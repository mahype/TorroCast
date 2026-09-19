//! The portable heart of TorroCast. A user interface sends [`Command`]s and
//! reads [`Event`]s; everything slow happens on worker threads in between.
//! Nothing here knows what a terminal is.

pub mod downloads;
pub mod fresh;
pub mod keeper;
pub mod playback;
pub mod settings;

use std::collections::HashMap;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex, RwLock};
use std::thread;

use downloads::Downloads;
use keeper::Keeper;
use playback::{Action, Playback};
use torrocast_media::{MediaKey, MediaSession, NowPlayingInfo};
use torrocast_player::{Media, Player, PlayerEvent};

use torrocast_directory::apple::Apple;
use torrocast_directory::fyyd::Fyyd;
use torrocast_directory::podcast_index::PodcastIndex;
use torrocast_directory::{DirectoryError, DirectoryProvider};
use torrocast_feed::{chapters, opml};
use torrocast_net::{Fetch, FetchError};

pub use downloads::{Download, DownloadState};
pub use fresh::NewEpisode;
pub use keeper::Playlist;
pub use playback::{NowPlaying, QueueItem, Sleep, Status};
pub use settings::Settings;
pub use torrocast_directory::{Category, EpisodeRef, PodcastRef, ProviderId, merge, normalise_feed_url};
pub use torrocast_feed::chapters::merge as merge_chapters;
pub use torrocast_feed::notes::{self, Block, Document, Inline};
pub use torrocast_feed::{Chapter, ChapterSource, Episode, Podcast};
pub use torrocast_library::{Progress, Subscription};
pub use torrocast_player::OutputKind;

/// Tags larger than this are cover art with chapters attached; not worth the traffic.
const MAX_TAG_BYTES: u64 = 3 * 1024 * 1024;

/// Why something did not work, in terms a user interface can put into words.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Problem {
    /// No answer: offline, or the server is down.
    Unreachable,
    /// The server answered with an error.
    Refused(u16),
    /// We held a request back to stay within a directory's limits.
    RateLimited,
    /// The address does not lead to a podcast feed.
    NotAFeed,
    /// An answer arrived that cannot be read.
    Unreadable,
}

impl From<FetchError> for Problem {
    fn from(error: FetchError) -> Self {
        match error {
            FetchError::Status(code) => Self::Refused(code),
            FetchError::Unreachable(_) => Self::Unreachable,
            FetchError::TooLarge | FetchError::RangeIgnored => Self::Unreadable,
        }
    }
}

impl From<DirectoryError> for Problem {
    fn from(error: DirectoryError) -> Self {
        match error {
            DirectoryError::RateLimited => Self::RateLimited,
            DirectoryError::Fetch(error) => error.into(),
            DirectoryError::Unreadable(_) | DirectoryError::Unsupported => Self::Unreadable,
        }
    }
}

/// `request` numbers are chosen by the caller and come back on the answer, so
/// an answer that arrives after the user has moved on can be recognised.
#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    Search {
        request: u64,
        query: String,
    },
    /// Episodes instead of shows. Apple only.
    SearchEpisodes {
        request: u64,
        query: String,
    },
    Charts {
        request: u64,
        category: Option<u32>,
    },
    Categories,
    OpenFeed {
        request: u64,
        feed_url: String,
        reload: bool,
    },
    /// Looks for chapters outside the feed: the JSON file it points to, and —
    /// only if nothing else announced chapters — the head of the MP3.
    Chapters {
        request: u64,
        chapters_url: Option<String>,
        mp3_url: Option<String>,
    },
    Transport(Transport),
    /// Fetches a picture — a podcast's cover. Kept on disk once fetched.
    Cover {
        url: String,
    },
    Playlist(PlaylistCommand),
    /// Subscribes to every feed of an OPML file. `~` is the home folder.
    ImportOpml {
        path: String,
    },
    /// Writes all subscriptions to an OPML file.
    ExportOpml {
        path: String,
    },
    /// Keeps an episode on this machine, to be heard without a network.
    Download(QueueItem),
    /// Deletes a downloaded episode, by its library id.
    DeleteDownload(String),
    /// Asks Podcast Index whether it accepts the key in the settings.
    VerifyPodcastIndex,
    /// Fetches every subscribed feed again; the list of new episodes follows as the answers come in.
    RefreshSubscriptions,
    /// Subscribes to a podcast, or ends the subscription. `guid` is the feed's `podcast:guid`, if it has one.
    SetSubscribed {
        feed_url: String,
        title: String,
        guid: Option<String>,
        subscribed: bool,
    },
}

/// The user's own playlists. Up Next is not one of them; it is what the player consumes.
#[derive(Debug, Clone, PartialEq)]
pub enum PlaylistCommand {
    /// A new playlist, optionally with its first episode.
    Create {
        name: String,
        first: Option<QueueItem>,
    },
    Delete(String),
    Add {
        playlist: String,
        item: QueueItem,
    },
    Remove {
        playlist: String,
        item: QueueItem,
    },
    /// The whole playlist into Up Next, at its front or its end.
    Queue {
        playlist: String,
        first: bool,
    },
}

/// Everything that concerns what is heard and what comes next.
#[derive(Debug, Clone, PartialEq)]
pub enum Transport {
    /// Play this now; what was playing moves to the top of Up Next.
    PlayNow(QueueItem),
    /// To the top of Up Next (`first`) or to its end.
    Enqueue {
        item: QueueItem,
        first: bool,
    },
    PlayQueued(usize),
    Remove(usize),
    Shift {
        index: usize,
        down: bool,
    },
    Clear,
    Toggle,
    Stop,
    NextEpisode,
    NextChapter,
    PreviousChapter,
    SeekBy(i64),
    SeekTo(u64),
    SpeedBy(f32),
    /// One step further on the sleep timer: 15, 30, 45, 60 minutes, end of the episode, off.
    CycleSleep,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    /// One directory's answer; a search yields one of these per active directory.
    SearchBatch {
        request: u64,
        provider: ProviderId,
        outcome: Result<Vec<PodcastRef>, Problem>,
    },
    EpisodeResults {
        request: u64,
        outcome: Result<Vec<EpisodeRef>, Problem>,
    },
    Charts {
        request: u64,
        outcome: Result<Vec<PodcastRef>, Problem>,
    },
    Categories {
        outcome: Result<Vec<Category>, Problem>,
    },
    Feed {
        request: u64,
        outcome: Result<Arc<Podcast>, Problem>,
    },
    Chapters {
        request: u64,
        chapters: Vec<Chapter>,
    },
    /// The whole truth about playback, sent whenever any of it changed.
    Playback(Box<PlaybackState>),
    /// How loud the moment being heard is, 0 to 1 — about twenty a second.
    Level(f32),
    /// An episode went to Up Next at this place, or (`None`) came out of it.
    Queued {
        title: String,
        place: Option<usize>,
    },
    /// What the subscriptions published lately and is still unheard. `pending` feeds
    /// are yet to answer; `failed` ones did not.
    NewEpisodes {
        episodes: Vec<NewEpisode>,
        pending: usize,
        failed: usize,
    },
    /// A picture asked for with [`Command::Cover`]; `None` if it could not be had.
    Cover {
        url: String,
        bytes: Option<Arc<Vec<u8>>>,
    },
    Opml(OpmlOutcome),
    /// The user's playlists, by name — at the start and whenever one changes, here or on another device.
    Playlists(Vec<Playlist>),
    /// How far every episode has been heard, by [`QueueItem::library_id`]. Sent at the start and whenever a
    /// place is written down. The episode playing right now is ahead of this; its place is in [`Event::Playback`].
    Progress(HashMap<String, Progress>),
    /// Every download, the newest last — whenever one starts, moves on, ends or is deleted.
    Downloads(Vec<Download>),
    /// Whether Podcast Index accepted the user's key. `Refused(401)` is a wrong key.
    PodcastIndexVerified(Result<(), Problem>),
    /// All subscriptions, alphabetically — at the start and whenever they change, here or on another device.
    Subscriptions(Vec<Subscription>),
}

/// What came of an OPML import or export.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OpmlOutcome {
    /// `new` feeds were subscribed to; `known` were subscriptions already.
    Imported {
        new: usize,
        known: usize,
    },
    Exported {
        count: usize,
        path: String,
    },
    /// The file could not be read, written, or understood.
    Failed,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct PlaybackState {
    pub now: Option<NowPlaying>,
    pub up_next: Vec<QueueItem>,
    pub speed: f32,
    pub sleep: Option<Sleep>,
}

struct Shared {
    fetch: Arc<dyn Fetch>,
    apple: Apple,
    fyyd: Fyyd,
    /// Present while the user has switched it on and given a key.
    podcast_index: RwLock<Option<PodcastIndex>>,
    feeds: Mutex<HashMap<String, Arc<Podcast>>>,
    /// Where fetched pictures are kept between runs, if anywhere.
    covers: RwLock<Option<std::path::PathBuf>>,
}

fn podcast_index(settings: &Settings) -> Option<PodcastIndex> {
    let sources = &settings.sources;
    sources.uses_podcast_index().then(|| PodcastIndex::new(&sources.podcast_index_key, &sources.podcast_index_secret))
}

/// Chapters found for the playing episode, by [`QueueItem::key`].
type Found = (String, Vec<Chapter>);
/// A subscribed feed, fetched again: its address and what came of it.
type Refreshed = (String, Result<Arc<Podcast>, Problem>);

/// Feeds fetched side by side during a refresh. Polite to hosters, quick enough for a long list.
const REFRESH_WORKERS: usize = 4;

pub struct Core {
    shared: Arc<Shared>,
    settings: Settings,
    events: Sender<Event>,
    output: OutputKind,
    playback: Playback,
    /// Opened with the first episode, so browsing never touches the sound card.
    player: Option<(Player, Receiver<PlayerEvent>)>,
    found: (Sender<Found>, Receiver<Found>),
    refreshed: (Sender<Refreshed>, Receiver<Refreshed>),
    /// The state of the running or last refresh.
    latest: Vec<(String, Arc<Podcast>)>,
    refresh_pending: usize,
    refresh_failed: usize,
    /// `None` when the library folder could not be opened; everything then lasts for the session.
    keeper: Option<Keeper>,
    downloads: Option<Downloads>,
    /// The desktop's media controls, opened with the first episode. `None` where there are none.
    media: Option<(MediaSession, Receiver<MediaKey>)>,
    media_tried: bool,
}

impl Core {
    /// The core and the channel its events arrive on.
    #[must_use]
    pub fn new(
        fetch: Arc<dyn Fetch>,
        settings: Settings,
        output: OutputKind,
        keeper: Option<Keeper>,
    ) -> (Self, Receiver<Event>) {
        let (events, receiver) = channel();
        let shared = Shared {
            fetch,
            apple: Apple::default(),
            fyyd: Fyyd,
            podcast_index: RwLock::new(podcast_index(&settings)),
            feeds: Mutex::new(HashMap::new()),
            covers: RwLock::new(None),
        };
        let core = Self {
            shared: Arc::new(shared),
            settings,
            events,
            output,
            playback: Playback::default(),
            player: None,
            found: channel(),
            refreshed: channel(),
            latest: Vec::new(),
            refresh_pending: 0,
            refresh_failed: 0,
            keeper,
            downloads: None,
            media: None,
            media_tried: false,
        };
        let mut core = core;
        if let Some(keeper) = &core.keeper {
            keeper.restore(&mut core.playback);
            let _ = core.events.send(Event::Subscriptions(keeper.subscriptions()));
            let _ = core.events.send(Event::Playlists(keeper.playlists()));
            let _ = core.events.send(Event::Progress(keeper.progress()));
        }
        core.publish();
        (core, receiver)
    }

    /// Says where pictures may be cached.
    pub fn set_cache_directory(&mut self, directory: &std::path::Path) {
        if let Ok(mut covers) = self.shared.covers.write() {
            *covers = Some(directory.join("covers"));
        }
    }

    /// Says where downloads are kept; what is already there is reported at once.
    pub fn set_download_directory(&mut self, directory: &std::path::Path) {
        let downloads = Downloads::open(directory);
        let _ = self.events.send(Event::Downloads(downloads.list()));
        self.downloads = Some(downloads);
    }

    pub fn set_settings(&mut self, settings: Settings) {
        if let Ok(mut index) = self.shared.podcast_index.write() {
            *index = podcast_index(&settings);
        }
        self.settings = settings;
    }

    /// The directories a search goes to right now.
    #[must_use]
    pub fn active_providers(&self) -> Vec<ProviderId> {
        let mut providers = vec![ProviderId::Apple];
        if self.settings.sources.uses_podcast_index() {
            providers.push(ProviderId::PodcastIndex);
        }
        if self.settings.sources.fyyd {
            providers.push(ProviderId::Fyyd);
        }
        providers
    }

    /// Returns at once; the answer arrives as an [`Event`].
    pub fn send(&mut self, command: Command) {
        let country = self.settings.country.clone();
        match command {
            Command::Search { request, query } => {
                // One thread per directory: a slow one never holds back a fast one.
                for provider in self.active_providers() {
                    let (query, country) = (query.clone(), country.clone());
                    self.spawn(move |shared| {
                        let fetch = shared.fetch.as_ref();
                        let outcome = match provider {
                            ProviderId::Apple => shared.apple.search(fetch, &query, &country),
                            ProviderId::Fyyd => shared.fyyd.search(fetch, &query, &country),
                            ProviderId::PodcastIndex => match shared.podcast_index.read().ok().as_deref() {
                                Some(Some(index)) => index.search(fetch, &query, &country),
                                _ => Err(DirectoryError::Unsupported),
                            },
                        }
                        .map_err(Problem::from);
                        Event::SearchBatch { request, provider, outcome }
                    });
                }
            }
            Command::SearchEpisodes { request, query } => self.spawn(move |shared| {
                let outcome =
                    shared.apple.search_episodes(shared.fetch.as_ref(), &query, &country).map_err(Problem::from);
                Event::EpisodeResults { request, outcome }
            }),
            Command::Transport(transport) => self.transport(transport),
            Command::RefreshSubscriptions => self.refresh(),
            Command::Playlist(command) => self.playlist(command),
            Command::ImportOpml { path } => {
                let outcome = self.import_opml(&home_expanded(&path));
                let _ = self.events.send(Event::Opml(outcome));
            }
            Command::ExportOpml { path } => {
                let path = home_expanded(&path);
                let outlines: Vec<opml::Outline> = self
                    .keeper
                    .iter()
                    .flat_map(Keeper::subscriptions)
                    .map(|subscription| opml::Outline { title: subscription.title, feed_url: subscription.feed_url })
                    .collect();
                let written = std::fs::write(&path, opml::write("TorroCast", &outlines));
                let outcome = if written.is_ok() {
                    OpmlOutcome::Exported { count: outlines.len(), path }
                } else {
                    OpmlOutcome::Failed
                };
                let _ = self.events.send(Event::Opml(outcome));
            }
            Command::Cover { url } => self.spawn(move |shared| {
                let bytes = cover(shared, &url).map(Arc::new);
                Event::Cover { url, bytes }
            }),
            Command::Download(item) => {
                if let Some(downloads) = &mut self.downloads {
                    downloads.start(item, Arc::clone(&self.shared.fetch));
                    let _ = self.events.send(Event::Downloads(downloads.list()));
                }
            }
            Command::DeleteDownload(id) => {
                if let Some(downloads) = &mut self.downloads {
                    downloads.remove(&id);
                    let _ = self.events.send(Event::Downloads(downloads.list()));
                }
            }
            Command::VerifyPodcastIndex => self.spawn(|shared| {
                let outcome = match shared.podcast_index.read().ok().as_deref() {
                    Some(Some(index)) => index.verify(shared.fetch.as_ref()).map_err(Problem::from),
                    _ => Err(Problem::Unreadable),
                };
                Event::PodcastIndexVerified(outcome)
            }),
            Command::SetSubscribed { feed_url, title, guid, subscribed } => {
                if let Some(keeper) = &mut self.keeper {
                    keeper.set_subscribed(keeper::podcast_id(guid.as_deref(), &feed_url), feed_url, title, subscribed);
                    let _ = self.events.send(Event::Subscriptions(keeper.subscriptions()));
                }
                // The list of new episodes follows the subscriptions.
                self.refresh();
            }
            Command::Charts { request, category } => self.spawn(move |shared| {
                let outcome = shared.apple.charts(shared.fetch.as_ref(), &country, category).map_err(Problem::from);
                Event::Charts { request, outcome }
            }),
            Command::Categories => self.spawn(move |shared| {
                let outcome = shared.apple.categories(shared.fetch.as_ref(), &country).map_err(Problem::from);
                Event::Categories { outcome }
            }),
            Command::OpenFeed { request, feed_url, reload } => {
                self.spawn(move |shared| Event::Feed { request, outcome: open_feed(shared, &feed_url, reload) });
            }
            Command::Chapters { request, chapters_url, mp3_url } => self.spawn(move |shared| Event::Chapters {
                request,
                chapters: find_chapters(shared.fetch.as_ref(), chapters_url, mp3_url),
            }),
        }
    }

    fn transport(&mut self, transport: Transport) {
        // The library may know where this episode was left, even if this device never played it.
        if let (Some(keeper), Transport::PlayNow(item) | Transport::Enqueue { item, .. }) = (&self.keeper, &transport) {
            self.playback.hint_position(item, keeper.position_of(item));
        }
        let actions = match transport {
            Transport::PlayNow(item) => self.playback.play_now(item),
            Transport::Enqueue { item, first } => {
                let title = item.title.clone();
                let (place, actions) = self.playback.enqueue(item, first);
                let _ = self.events.send(Event::Queued { title, place });
                actions
            }
            Transport::PlayQueued(index) => self.playback.play_queued(index),
            Transport::Remove(index) => {
                self.playback.remove(index);
                Vec::new()
            }
            Transport::Shift { index, down } => {
                self.playback.shift(index, down);
                Vec::new()
            }
            Transport::Clear => {
                self.playback.clear();
                Vec::new()
            }
            Transport::Toggle => self.playback.toggle(),
            Transport::Stop => self.playback.stop(),
            Transport::NextEpisode => self.playback.next_episode(),
            Transport::NextChapter => self.playback.next_chapter(),
            Transport::PreviousChapter => self.playback.previous_chapter(),
            Transport::SeekBy(delta_ms) => self.playback.seek_by(delta_ms),
            Transport::SeekTo(position_ms) => self.playback.seek_to(position_ms),
            Transport::SpeedBy(delta) => self.playback.change_speed(delta),
            Transport::CycleSleep => {
                self.playback.cycle_sleep(std::time::Instant::now());
                Vec::new()
            }
        };
        self.carry_out(actions);
        self.keep();
        self.publish();
    }

    fn import_opml(&mut self, path: &str) -> OpmlOutcome {
        let Some(keeper) = &mut self.keeper else { return OpmlOutcome::Failed };
        let Some(outlines) = std::fs::read(path).ok().and_then(|bytes| opml::parse(&decode_body(&bytes)).ok()) else {
            return OpmlOutcome::Failed;
        };
        let (mut new, mut known) = (0, 0);
        for outline in outlines {
            // The feed's own guid is not known yet; the address names the podcast until the feed says otherwise.
            let podcast = keeper::podcast_id(None, &outline.feed_url);
            if keeper.is_subscribed(&podcast) {
                known += 1;
            } else {
                keeper.set_subscribed(podcast, outline.feed_url, outline.title, true);
                new += 1;
            }
        }
        let _ = self.events.send(Event::Subscriptions(keeper.subscriptions()));
        self.refresh();
        OpmlOutcome::Imported { new, known }
    }

    fn playlist(&mut self, command: PlaylistCommand) {
        let Some(keeper) = &mut self.keeper else { return };
        let mut queued = None;
        match command {
            PlaylistCommand::Create { name, first } => {
                let playlist = keeper.create_playlist(&name);
                if let Some(item) = first {
                    keeper.add_to_playlist(&playlist, &item);
                }
            }
            PlaylistCommand::Delete(playlist) => keeper.delete_playlist(&playlist),
            PlaylistCommand::Add { playlist, item } => keeper.add_to_playlist(&playlist, &item),
            PlaylistCommand::Remove { playlist, item } => keeper.remove_from_playlist(&playlist, &item),
            PlaylistCommand::Queue { playlist, first } => {
                queued =
                    keeper.playlists().into_iter().find(|known| known.id == playlist).map(|known| (known.items, first));
            }
        }
        let _ = self.events.send(Event::Playlists(keeper.playlists()));
        if let Some((items, first)) = queued {
            let actions = self.playback.enqueue_many(items, first);
            self.carry_out(actions);
            self.keep();
            self.publish();
        }
    }

    /// Fetches all subscribed feeds again, a few at a time.
    fn refresh(&mut self) {
        let Some(keeper) = &self.keeper else { return };
        if self.refresh_pending > 0 {
            return;
        }
        let feeds: Vec<String> = keeper.subscriptions().into_iter().map(|subscription| subscription.feed_url).collect();
        self.latest.clear();
        self.refresh_pending = feeds.len();
        self.refresh_failed = 0;
        let queue = Arc::new(Mutex::new(feeds));
        for _ in 0..REFRESH_WORKERS {
            let (shared, queue, results) = (Arc::clone(&self.shared), Arc::clone(&queue), self.refreshed.0.clone());
            thread::spawn(move || {
                loop {
                    let Some(feed_url) = queue.lock().ok().and_then(|mut queue| queue.pop()) else { return };
                    let outcome = open_feed(&shared, &feed_url, true);
                    if results.send((feed_url, outcome)).is_err() {
                        return;
                    }
                }
            });
        }
        self.publish_new();
    }

    fn publish_new(&self) {
        let is_played = |item: &QueueItem| self.keeper.as_ref().is_some_and(|keeper| keeper.is_played(item));
        let episodes = fresh::newest(&self.latest, chrono::Utc::now(), is_played);
        let _ = self.events.send(Event::NewEpisodes {
            episodes,
            pending: self.refresh_pending,
            failed: self.refresh_failed,
        });
    }

    /// Writes down what playback changed.
    fn keep(&mut self) {
        let notes = self.playback.take_notes();
        let heard_one = notes.iter().any(|note| note.played);
        let noted = !notes.is_empty();
        if let Some(keeper) = &mut self.keeper {
            keeper.save_notes(notes);
            if noted {
                let _ = self.events.send(Event::Progress(keeper.progress()));
            }
            // What plays is kept at the head of the stored list. Should the
            // program end without warning, the episode is still there next time.
            let stored: Vec<QueueItem> = self
                .playback
                .now
                .iter()
                .map(|now| now.item.clone())
                .chain(self.playback.up_next.iter().cloned())
                .collect();
            keeper.save_queue(&stored);
        }
        // An episode heard to the end is no longer new.
        if heard_one {
            self.publish_new();
        }
    }

    /// Call before the program ends: the place in the playing episode is written down.
    pub fn shutdown(&mut self) {
        let note = self.playback.note_now();
        if let Some(keeper) = &mut self.keeper {
            keeper.save_notes(note.into_iter().collect());
        }
    }

    fn carry_out(&mut self, actions: Vec<Action>) {
        for action in actions {
            if let Action::FindChapters { key, chapters_url, mp3_url } = action {
                let (shared, found) = (Arc::clone(&self.shared), self.found.0.clone());
                thread::spawn(move || {
                    let _ = found.send((key, find_chapters(shared.fetch.as_ref(), chapters_url, mp3_url)));
                });
                continue;
            }
            // An episode that is on this machine is played from there.
            let local = self.playback.now.as_ref().map(|now| now.item.library_id()).and_then(|id| {
                self.downloads.as_ref().and_then(|downloads| downloads.file_of(&id).map(std::path::Path::to_owned))
            });
            let output = self.output;
            let (player, _) = self.player.get_or_insert_with(|| Player::new(output));
            match action {
                Action::Load { audio_url, start_ms } => {
                    player.set_speed(self.playback.speed);
                    player.load(local.map_or(Media::Url(audio_url), Media::File), start_ms);
                }
                Action::Pause => player.pause(),
                Action::Resume => player.resume(),
                Action::Seek(position_ms) => player.seek(position_ms),
                Action::Speed(speed) => player.set_speed(speed),
                Action::Stop => player.stop(),
                Action::FindChapters { .. } => {}
            }
        }
    }

    /// Tells the desktop's media controls what plays, opening them when first needed.
    fn tell_desktop(&mut self) {
        // Automated tests have no business on the session bus.
        if self.output == OutputKind::Null {
            return;
        }
        if self.playback.now.is_some() && !self.media_tried {
            self.media_tried = true;
            self.media = MediaSession::open();
        }
        let info = self.playback.now.as_ref().map(|now| NowPlayingInfo {
            title: now.item.title.clone(),
            podcast: now.item.podcast.clone(),
            artwork_url: now.item.artwork_url.clone(),
            duration_ms: now.duration_ms,
            position_ms: now.position_ms,
            playing: now.status == Status::Playing,
        });
        if let Some((session, _)) = &mut self.media {
            session.update(info.as_ref());
        }
    }

    fn publish(&mut self) {
        self.tell_desktop();
        let state = PlaybackState {
            now: self.playback.now.clone(),
            up_next: self.playback.up_next.clone(),
            speed: self.playback.speed,
            sleep: self.playback.sleep(std::time::Instant::now()),
        };
        let _ = self.events.send(Event::Playback(Box::new(state)));
    }

    /// Takes in what the audio engine has reported since the last call. The
    /// interface calls this on every turn of its loop.
    pub fn pump(&mut self) {
        // Media keys, the headset's button, the panel's sound menu.
        let keys: Vec<MediaKey> = self.media.as_ref().map(|(_, keys)| keys.try_iter().collect()).unwrap_or_default();
        for key in keys {
            let playing = self.playback.now.as_ref().is_some_and(|now| now.status == Status::Playing);
            let transport = match key {
                MediaKey::Toggle => Some(Transport::Toggle),
                MediaKey::Play if !playing => Some(Transport::Toggle),
                MediaKey::Pause if playing => Some(Transport::Toggle),
                MediaKey::Play | MediaKey::Pause => None,
                MediaKey::Stop => Some(Transport::Stop),
                MediaKey::Next => Some(Transport::NextChapter),
                MediaKey::Previous => Some(Transport::PreviousChapter),
                MediaKey::SeekBy(delta_ms) => Some(Transport::SeekBy(delta_ms)),
                MediaKey::SeekTo(position_ms) => Some(Transport::SeekTo(position_ms)),
            };
            if let Some(transport) = transport {
                self.transport(transport);
            }
        }

        let reports: Vec<PlayerEvent> =
            self.player.as_ref().map(|(_, events)| events.try_iter().collect()).unwrap_or_default();
        let mut changed = false;
        let asleep = self.playback.sleep_due(std::time::Instant::now());
        if !asleep.is_empty() {
            self.carry_out(asleep);
            changed = true;
        }
        for report in reports {
            match report {
                PlayerEvent::Level(level) => {
                    let _ = self.events.send(Event::Level(level));
                    continue;
                }
                PlayerEvent::Started { duration_ms } => self.playback.on_started(duration_ms),
                PlayerEvent::Position { position_ms, .. } => self.playback.on_position(position_ms),
                PlayerEvent::Failed(reason) => self.playback.on_failed(reason),
                PlayerEvent::Ended => {
                    let actions = self.playback.on_ended();
                    self.carry_out(actions);
                }
                PlayerEvent::Loading | PlayerEvent::Paused | PlayerEvent::Resumed | PlayerEvent::Stopped => {}
            }
            changed = true;
        }
        if let Some(downloads) = &mut self.downloads
            && downloads.pump()
        {
            let _ = self.events.send(Event::Downloads(downloads.list()));
        }
        let answers: Vec<Refreshed> = self.refreshed.1.try_iter().collect();
        if !answers.is_empty() {
            for (feed_url, outcome) in answers {
                self.refresh_pending = self.refresh_pending.saturating_sub(1);
                match outcome {
                    Ok(podcast) => self.latest.push((feed_url, podcast)),
                    Err(_) => self.refresh_failed += 1,
                }
            }
            self.publish_new();
        }
        for (key, chapters) in self.found.1.try_iter().collect::<Vec<_>>() {
            self.playback.set_chapters(&key, chapters);
            changed = true;
        }
        if changed {
            self.keep();
        }
        if let Some(keeper) = &mut self.keeper {
            keeper.save_now_and_then(&self.playback);
            // Another device subscribed, queued or listened: take it over.
            if keeper.look() {
                keeper.restore(&mut self.playback);
                let _ = self.events.send(Event::Subscriptions(keeper.subscriptions()));
                let _ = self.events.send(Event::Playlists(keeper.playlists()));
                let _ = self.events.send(Event::Progress(keeper.progress()));
                changed = true;
            }
        }
        if changed {
            self.publish();
        }
    }

    /// Moves the library to `directory`. On success everything the interface shows is sent again.
    pub fn move_library(&mut self, directory: &std::path::Path) -> Result<(), String> {
        let keeper = match &self.keeper {
            Some(keeper) => keeper.relocate(directory),
            None => return Err("no library is open".to_owned()),
        }
        .map_err(|error| error.to_string())?;
        keeper.restore(&mut self.playback);
        let _ = self.events.send(Event::Subscriptions(keeper.subscriptions()));
        let _ = self.events.send(Event::Playlists(keeper.playlists()));
        self.keeper = Some(keeper);
        self.keep();
        self.publish();
        self.refresh();
        Ok(())
    }

    /// Where the library lives, if it could be opened.
    #[must_use]
    pub fn library_directory(&self) -> Option<&std::path::Path> {
        self.keeper.as_ref().map(Keeper::directory)
    }

    fn spawn(&self, work: impl FnOnce(&Shared) -> Event + Send + 'static) {
        let (shared, events) = (Arc::clone(&self.shared), self.events.clone());
        thread::spawn(move || {
            // A closed channel means the interface is gone; nothing left to tell.
            let _ = events.send(work(&shared));
        });
    }
}

fn open_feed(shared: &Shared, feed_url: &str, reload: bool) -> Result<Arc<Podcast>, Problem> {
    if !reload
        && let Ok(feeds) = shared.feeds.lock()
        && let Some(known) = feeds.get(feed_url)
    {
        return Ok(Arc::clone(known));
    }
    let body = shared.fetch.get(feed_url)?;
    let podcast = torrocast_feed::parse_feed(&decode_body(&body)).map_err(|error| match error {
        torrocast_feed::FeedError::NotAFeed => Problem::NotAFeed,
        torrocast_feed::FeedError::Malformed(_) => Problem::Unreadable,
    })?;
    let podcast = Arc::new(podcast);
    if let Ok(mut feeds) = shared.feeds.lock() {
        feeds.insert(feed_url.to_owned(), Arc::clone(&podcast));
    }
    Ok(podcast)
}

/// A picture, from the disk if it was fetched before.
fn cover(shared: &Shared, url: &str) -> Option<Vec<u8>> {
    // Covers of several megabytes exist; they are of no use to a terminal and not worth keeping.
    const LARGEST: usize = 12 * 1024 * 1024;
    let file = shared.covers.read().ok()?.as_ref().map(|directory| {
        let name: String =
            torrocast_library::episode_id(url).chars().filter(|character| character.is_ascii_alphanumeric()).collect();
        directory.join(name)
    });
    if let Some(known) = file.as_ref().and_then(|file| std::fs::read(file).ok()) {
        return Some(known);
    }
    let bytes = shared.fetch.get(url).ok().filter(|bytes| bytes.len() <= LARGEST)?;
    if let Some(file) = &file {
        let _ = file.parent().map(std::fs::create_dir_all);
        let _ = std::fs::write(file, &bytes);
    }
    Some(bytes)
}

/// `~/podcasts.opml` as the file system wants it.
fn home_expanded(path: &str) -> String {
    let path = path.trim();
    let home = std::env::var("HOME").or_else(|_| std::env::var("USERPROFILE")).unwrap_or_default();
    match path.strip_prefix('~') {
        Some(rest) if !home.is_empty() => format!("{home}{rest}"),
        _ => path.to_owned(),
    }
}

/// Chapters from outside the feed: the JSON file first, the head of the MP3 otherwise.
fn find_chapters(fetch: &dyn Fetch, chapters_url: Option<String>, mp3_url: Option<String>) -> Vec<Chapter> {
    let found = chapters_url
        .and_then(|url| fetch.get(&url).ok())
        .and_then(|body| chapters::parse_json(&body).ok())
        .unwrap_or_default();
    match mp3_url {
        Some(url) if found.is_empty() => embedded_chapters(fetch, &url),
        _ => found,
    }
}

impl QueueItem {
    /// An episode of a feed, ready to be played. `None` when it has no audio.
    #[must_use]
    pub fn from_feed(podcast: &Podcast, feed_url: Option<&str>, episode: &Episode) -> Option<Self> {
        let enclosure = episode.enclosure.as_ref()?;
        Some(Self {
            title: episode.title.clone(),
            podcast: podcast.title.clone(),
            feed_url: feed_url.map(str::to_owned),
            guid: episode.guid.clone(),
            audio_url: enclosure.url.clone(),
            duration_ms: episode.duration_seconds.map(|seconds| u64::from(seconds) * 1000),
            chapters: episode.chapters.clone(),
            chapters_url: episode.chapters_url.clone(),
            is_mp3: enclosure.is_mp3(),
            artwork_url: episode.image.clone().or_else(|| podcast.image.clone()),
        })
    }

    /// An episode found by a directory's search.
    #[must_use]
    pub fn from_search(episode: &EpisodeRef) -> Self {
        let path = episode.audio_url.split(['?', '#']).next().unwrap_or_default().to_lowercase();
        Self {
            title: episode.title.clone(),
            podcast: episode.podcast.clone(),
            feed_url: episode.feed_url.clone(),
            guid: episode.guid.clone(),
            audio_url: episode.audio_url.clone(),
            duration_ms: episode.duration_ms,
            chapters: Vec::new(),
            chapters_url: None,
            is_mp3: path.ends_with(".mp3"),
            artwork_url: episode.artwork_url.clone(),
        }
    }
}

/// Feeds are UTF-8 with few exceptions, and the exceptions say so up front.
fn decode_body(body: &[u8]) -> String {
    match std::str::from_utf8(body) {
        Ok(text) => text.to_owned(),
        Err(_) => {
            let declaration = String::from_utf8_lossy(&body[..body.len().min(200)]).to_lowercase();
            if declaration.contains("iso-8859-1") || declaration.contains("windows-1252") {
                body.iter().map(|byte| char::from(*byte)).collect()
            } else {
                String::from_utf8_lossy(body).into_owned()
            }
        }
    }
}

/// Reads the ID3 tag at the head of an MP3 — ten bytes to learn its size, then
/// exactly the tag. The audio itself is never requested.
fn embedded_chapters(fetch: &dyn Fetch, url: &str) -> Vec<Chapter> {
    let Some(size) = fetch.get_range(url, 0, 9).ok().and_then(|head| chapters::id3_tag_size(&head)) else {
        return Vec::new();
    };
    if size > MAX_TAG_BYTES {
        return Vec::new();
    }
    fetch.get_range(url, 0, size - 1).map(|tag| chapters::parse_id3(&tag)).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::Mutex;
    use std::time::Duration;

    use torrocast_net::{Fetch, FetchError};

    use super::{Command, Core, Event, Problem, ProviderId, Settings};

    const FEED: &str =
        r#"<rss version="2.0"><channel><title>Show</title><item><title>One</title></item></channel></rss>"#;

    struct Canned {
        asked: Mutex<Vec<String>>,
    }

    impl Fetch for Canned {
        fn get(&self, url: &str) -> Result<Vec<u8>, FetchError> {
            self.asked.lock().expect("no poisoning in tests").push(url.to_owned());
            match url {
                "https://show.example/feed" => Ok(FEED.as_bytes().to_vec()),
                "https://show.example/page" => Ok(b"<html/>".to_vec()),
                url if url.starts_with("https://api.fyyd.de") => {
                    Ok(br#"{"data":[{"title":"Found","xmlURL":"https://f.example"}]}"#.to_vec())
                }
                _ => Err(FetchError::Status(503)),
            }
        }

        fn get_range(&self, _url: &str, _start: u64, _end: u64) -> Result<Vec<u8>, FetchError> {
            Err(FetchError::RangeIgnored)
        }
    }

    fn core(fyyd: bool) -> (Core, std::sync::mpsc::Receiver<Event>, Arc<Canned>) {
        let canned = Arc::new(Canned { asked: Mutex::new(Vec::new()) });
        let mut settings = Settings::for_locale("de_DE");
        settings.sources.fyyd = fyyd;
        let (core, events) = Core::new(Arc::clone(&canned) as Arc<dyn Fetch>, settings, super::OutputKind::Null, None);
        (core, events, canned)
    }

    /// The next answer to a command; the core's reports about itself are passed over.
    fn next(events: &std::sync::mpsc::Receiver<Event>) -> Event {
        loop {
            match events.recv_timeout(Duration::from_secs(5)).expect("the worker answers") {
                Event::Playback(_) | Event::Subscriptions(_) | Event::Playlists(_) | Event::Progress(_) => {}
                event => return event,
            }
        }
    }

    #[test]
    fn a_feed_is_fetched_once() {
        let (mut core, events, canned) = core(false);
        for request in 1..=2 {
            core.send(Command::OpenFeed { request, feed_url: "https://show.example/feed".into(), reload: false });
            let Event::Feed { request: answered, outcome } = next(&events) else { panic!("expected a feed") };
            assert_eq!(answered, request);
            assert_eq!(outcome.expect("parses").episodes.len(), 1);
        }
        assert_eq!(canned.asked.lock().expect("no poisoning in tests").len(), 1);
    }

    #[test]
    fn problems_have_names() {
        let (mut core, events, _) = core(false);
        core.send(Command::OpenFeed { request: 1, feed_url: "https://show.example/page".into(), reload: false });
        assert_eq!(next(&events), Event::Feed { request: 1, outcome: Err(Problem::NotAFeed) });
        core.send(Command::OpenFeed { request: 2, feed_url: "https://down.example".into(), reload: false });
        assert_eq!(next(&events), Event::Feed { request: 2, outcome: Err(Problem::Refused(503)) });
    }

    #[test]
    fn one_directory_failing_does_not_silence_the_other() {
        let (mut core, events, _) = core(true);
        core.send(Command::Search { request: 7, query: "found".into() });
        let mut answers = [next(&events), next(&events)];
        answers.sort_by_key(|event| match event {
            Event::SearchBatch { provider, .. } => *provider,
            _ => ProviderId::Apple,
        });
        assert!(matches!(
            &answers[0],
            Event::SearchBatch { provider: ProviderId::Apple, outcome: Err(Problem::Refused(503)), .. }
        ));
        assert!(
            matches!(&answers[1], Event::SearchBatch { provider: ProviderId::Fyyd, outcome: Ok(found), .. } if found[0].title == "Found")
        );
    }
}
