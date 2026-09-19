//! What the user is looking at, and what a key does to it. The app never
//! fetches: it queues [`Command`]s for the core and is told the [`Event`]s.

use std::cell::RefCell;
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::{Duration, Instant};

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::{Position, Rect};
use torrocast_core::settings::COUNTRIES;
use torrocast_core::{
    Category, Chapter, Command, Document, Download, DownloadState, Episode, EpisodeRef, Event, NewEpisode, OpmlOutcome,
    PlaybackState, Playlist, PlaylistCommand, Podcast, PodcastRef, Problem, Progress, ProviderId, QueueItem, Settings,
    Subscription, Transport, merge, merge_chapters, normalise_feed_url, notes,
};

use crate::covers::Covers;
use crate::i18n::Lang;

/// Apple allows about twenty searches a minute; waiting for a pause in the
/// typing keeps a whole word to a single search.
pub const SEARCH_DELAY: Duration = Duration::from_millis(600);
const MIN_QUERY: usize = 3;
pub const MIN_WIDTH: u16 = 80;
pub const MIN_HEIGHT: u16 = 24;
const MENU_FIRST_ROW: u16 = 3;
pub const MENU_WIDTH: u16 = 26;
/// Rows of the player at the foot of the menu column, with and without the level meter.
pub const PLAYER_ROWS: u16 = 14;
pub const PLAYER_ROWS_SHORT: u16 = 11;
/// Below this height the level meter is the first thing to go.
pub const METER_FROM_HEIGHT: u16 = 30;
pub const LEVELS: usize = 22;
const SEEK_BACK_MS: i64 = -30_000;
const SEEK_FORWARD_MS: i64 = 30_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Section {
    Discover,
    Subscriptions,
    NewEpisodes,
    UpNext,
    Playlists,
    Downloads,
    Settings,
    Help,
}

impl Section {
    pub const ALL: [Self; 8] = [
        Self::Discover,
        Self::Subscriptions,
        Self::NewEpisodes,
        Self::UpNext,
        Self::Playlists,
        Self::Downloads,
        Self::Settings,
        Self::Help,
    ];

    #[must_use]
    pub fn title(self) -> &'static str {
        match self {
            Self::Discover => "Discover",
            Self::Subscriptions => "Subscriptions",
            Self::NewEpisodes => "New Episodes",
            Self::UpNext => "Up Next",
            Self::Playlists => "Playlists",
            Self::Downloads => "Downloads",
            Self::Settings => "Settings",
            Self::Help => "Help",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Search,
    Charts,
    Categories,
}

impl Tab {
    pub const ALL: [Self; 3] = [Self::Search, Self::Charts, Self::Categories];

    #[must_use]
    pub fn title(self) -> &'static str {
        match self {
            Self::Search => "Search",
            Self::Charts => "Charts",
            Self::Categories => "Categories",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Load {
    Idle,
    Loading,
    Ready,
    Failed(Problem),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Search {
    pub input: String,
    /// The input field has the keyboard: every character lands in it.
    pub editing: bool,
    typed_at: Option<Instant>,
    /// The query the current results belong to.
    pub sent: String,
    request: u64,
    pub pending: Vec<ProviderId>,
    batches: Vec<(ProviderId, Vec<PodcastRef>)>,
    pub failures: Vec<(ProviderId, Problem)>,
    pub results: Vec<PodcastRef>,
    pub index: usize,
    /// Searching for episodes instead of shows.
    pub episodes_mode: bool,
    pub episodes: Vec<EpisodeRef>,
    pub episode_index: usize,
    pub episodes_load: Load,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Charts {
    request: u64,
    /// `None` is the country's overall chart.
    pub category: Option<Category>,
    pub list: Vec<PodcastRef>,
    pub index: usize,
    pub load: Load,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Categories {
    pub list: Vec<Category>,
    pub index: usize,
    pub load: Load,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PodcastView {
    /// What the directory said; shown until the feed has arrived.
    pub reference: PodcastRef,
    request: u64,
    pub load: Load,
    pub podcast: Option<Arc<Podcast>>,
    /// Position within [`PodcastView::visible`].
    pub index: usize,
    pub newest_first: bool,
    pub filter: String,
    pub filtering: bool,
    pub expanded: bool,
    /// Opened from an episode found by search: go on to this episode once the feed is here.
    wanted_guid: Option<String>,
    /// Where `esc` leads back to.
    origin: Section,
}

impl PodcastView {
    /// The episodes the list shows, as positions in the feed, in display order.
    #[must_use]
    pub fn visible(&self) -> Vec<usize> {
        let Some(podcast) = &self.podcast else {
            return Vec::new();
        };
        let needle = self.filter.to_lowercase();
        let mut visible: Vec<usize> = podcast
            .episodes
            .iter()
            .enumerate()
            .filter(|(_, episode)| needle.is_empty() || episode.title.to_lowercase().contains(&needle))
            .map(|(position, _)| position)
            .collect();
        // Feeds list the newest episode first; that is the order we keep by default.
        if !self.newest_first {
            visible.reverse();
        }
        visible
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    Notes,
    Chapters,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EpisodeView {
    pub podcast: Arc<Podcast>,
    pub feed_url: Option<String>,
    pub position: usize,
    pub notes: Document,
    pub chapters: Vec<Chapter>,
    /// Chapters may still arrive from outside the feed.
    pub looking: Option<Looking>,
    request: u64,
    pub focus: Focus,
    pub scroll: u16,
    pub chapter_index: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Looking {
    ChaptersFile,
    AudioFile,
}

impl EpisodeView {
    #[must_use]
    pub fn episode(&self) -> &Episode {
        &self.podcast.episodes[self.position]
    }

    /// Chapters that are entries of the list; silent marks are not.
    #[must_use]
    pub fn listed_chapters(&self) -> Vec<&Chapter> {
        self.chapters.iter().filter(|chapter| !chapter.hidden).collect()
    }
}

/// Something on screen a click can mean. Noted by the drawing code, frame by frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Hit {
    pub area: Rect,
    pub target: HitTarget,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HitTarget {
    /// The rows of a list: `first` is the entry in the top row, each entry `rows_each` rows tall.
    Rows {
        first: usize,
        rows_each: u16,
        count: usize,
    },
    Tab(Tab),
}

/// A text being typed in the settings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Field {
    Library,
    IndexKey,
    IndexSecret,
}

/// What is known about the user's Podcast Index key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IndexState {
    Unknown,
    Checking,
    Accepted,
    Rejected,
    Unreachable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueueState {
    Playing,
    /// Position in Up Next, counted from 0.
    Queued(usize),
}

#[derive(Debug)]
pub struct App {
    pub lang: Lang,
    pub settings: Settings,
    pub section: Section,
    pub tab: Tab,
    pub search: Search,
    pub charts: Charts,
    pub categories: Categories,
    pub podcast: Option<PodcastView>,
    pub episode: Option<EpisodeView>,
    pub settings_index: usize,
    /// A sentence for the bottom of the content area; gone with the next key.
    pub notice: Option<String>,
    /// The directories a search reaches right now, told by whoever owns the core.
    pub providers: Vec<ProviderId>,
    pub subscriptions: Vec<Subscription>,
    pub subscriptions_index: usize,
    pub covers: Covers,
    /// How far episodes have been heard, by library id; the playing one is ahead of this.
    pub progress: HashMap<String, Progress>,
    /// What the last frame drew that can be clicked.
    pub hits: RefCell<Vec<Hit>>,
    pub playlists: Vec<Playlist>,
    pub playlists_index: usize,
    /// The playlist being looked into, by its id, and the selection within it.
    pub open_playlist: Option<String>,
    pub playlist_item: usize,
    /// `L` was pressed on this episode: which playlist shall it go to? The number is the selection.
    pub picker: Option<(QueueItem, usize)>,
    /// The name of a new playlist while it is typed, and the episode that goes into it first.
    pub playlist_name: Option<(Option<QueueItem>, String)>,
    confirm_delete: bool,
    /// The path of an OPML file while it is typed: `true` to import from it, `false` to export to it.
    pub opml_path: Option<(bool, String)>,
    pub downloads: Vec<Download>,
    pub downloads_index: usize,
    pub new_episodes: Vec<NewEpisode>,
    pub new_index: usize,
    /// Feeds of the running refresh still to answer, and those that did not.
    pub refresh_pending: usize,
    pub refresh_failed: usize,
    /// Where the library lives, or why there is none. Told by the main loop.
    pub library: Result<String, String>,
    /// The text being typed in the settings, and what it is for.
    pub settings_input: Option<(Field, String)>,
    pub index_state: IndexState,
    /// A folder the user has asked the library to move to; the main loop carries it out.
    pub library_request: Option<String>,
    pub playback: PlaybackState,
    /// The last moments' loudness, oldest first, for the meter in the player.
    pub levels: VecDeque<f32>,
    pub up_next_index: usize,
    /// The large player covers the content area.
    pub player_open: bool,
    pub player_chapter: usize,
    /// `C` was pressed once in Up Next; the second press clears the list.
    confirm_clear: bool,
    /// Told by the main loop, so a click can be matched to the player's buttons.
    pub terminal_height: u16,
    requests: u64,
    // What the main loop has to do on the app's behalf.
    pub commands: Vec<Command>,
    pub open_urls: Vec<String>,
    pub settings_changed: bool,
    pub should_quit: bool,
}

impl App {
    #[must_use]
    pub fn new(lang: Lang, settings: Settings) -> Self {
        let mut providers = vec![ProviderId::Apple];
        if settings.sources.uses_podcast_index() {
            providers.push(ProviderId::PodcastIndex);
        }
        if settings.sources.fyyd {
            providers.push(ProviderId::Fyyd);
        }
        Self {
            lang,
            settings,
            section: Section::Discover,
            tab: Tab::Search,
            search: Search {
                input: String::new(),
                // A search tool that starts ready to be typed into.
                editing: true,
                typed_at: None,
                sent: String::new(),
                request: 0,
                pending: Vec::new(),
                batches: Vec::new(),
                failures: Vec::new(),
                results: Vec::new(),
                index: 0,
                episodes_mode: false,
                episodes: Vec::new(),
                episode_index: 0,
                episodes_load: Load::Idle,
            },
            charts: Charts { request: 0, category: None, list: Vec::new(), index: 0, load: Load::Idle },
            categories: Categories { list: Vec::new(), index: 0, load: Load::Idle },
            podcast: None,
            episode: None,
            settings_index: 0,
            notice: None,
            providers,
            subscriptions: Vec::new(),
            subscriptions_index: 0,
            covers: Covers::default(),
            progress: HashMap::new(),
            hits: RefCell::new(Vec::new()),
            playlists: Vec::new(),
            playlists_index: 0,
            open_playlist: None,
            playlist_item: 0,
            picker: None,
            playlist_name: None,
            confirm_delete: false,
            opml_path: None,
            downloads: Vec::new(),
            downloads_index: 0,
            new_episodes: Vec::new(),
            new_index: 0,
            refresh_pending: 0,
            refresh_failed: 0,
            library: Err(String::new()),
            settings_input: None,
            index_state: IndexState::Unknown,
            library_request: None,
            playback: PlaybackState { speed: 1.0, ..PlaybackState::default() },
            levels: VecDeque::new(),
            up_next_index: 0,
            player_open: false,
            player_chapter: 0,
            confirm_clear: false,
            terminal_height: 0,
            requests: 0,
            commands: Vec::new(),
            open_urls: Vec::new(),
            settings_changed: false,
            should_quit: false,
        }
    }

    fn next_request(&mut self) -> u64 {
        self.requests += 1;
        self.requests
    }

    /// Which screen is up — equal values mean the same screen, whatever its content.
    #[must_use]
    pub fn screen(&self) -> (Section, Tab, bool, bool, bool) {
        (self.section, self.tab, self.podcast.is_some(), self.episode.is_some(), self.player_open)
    }

    fn transport(&mut self, transport: Transport) {
        self.commands.push(Command::Transport(transport));
    }

    /// Whether the player at the foot of the menu column is on screen.
    #[must_use]
    pub fn shows_mini_player(&self) -> bool {
        self.playback.now.is_some() && !self.player_open
    }

    /// What the queue says about an episode: playing, or its place in Up Next.
    #[must_use]
    pub fn queue_state(&self, item_key: &str) -> Option<QueueState> {
        if self.playback.now.as_ref().is_some_and(|now| now.item.key() == item_key) {
            return Some(QueueState::Playing);
        }
        self.playback.up_next.iter().position(|item| item.key() == item_key).map(QueueState::Queued)
    }

    /// Whether characters typed right now go into a text field.
    #[must_use]
    pub fn is_typing(&self) -> bool {
        match self.section {
            _ if self.playlist_name.is_some() || self.opml_path.is_some() => true,
            _ if self.player_open => false,
            Section::Settings => self.settings_input.is_some(),
            Section::Discover if self.episode.is_some() => false,
            Section::Discover => match &self.podcast {
                Some(view) => view.filtering,
                None => self.tab == Tab::Search && self.search.editing,
            },
            _ => false,
        }
    }

    // ── time ────────────────────────────────────────────────────────────────

    /// Called regularly; starts the search once the typing has paused.
    pub fn tick(&mut self, now: Instant) {
        let due = self.search.typed_at.is_some_and(|typed| now.duration_since(typed) >= SEARCH_DELAY);
        if due {
            self.start_search();
        }
    }

    fn start_search(&mut self) {
        self.search.typed_at = None;
        let query = self.search.input.trim().to_owned();
        if query == self.search.sent || query.chars().count() < MIN_QUERY {
            return;
        }
        // An address is not a search: it is the feed itself.
        if query.starts_with("http://") || query.starts_with("https://") {
            self.search.editing = false;
            let reference = PodcastRef { title: query.clone(), feed_url: Some(query), ..PodcastRef::default() };
            self.open_podcast(reference);
            return;
        }
        let request = self.next_request();
        self.search.request = request;
        self.search.sent.clone_from(&query);
        if self.search.episodes_mode {
            self.search.episodes.clear();
            self.search.episode_index = 0;
            self.search.episodes_load = Load::Loading;
            self.commands.push(Command::SearchEpisodes { request, query });
            return;
        }
        self.search.pending.clone_from(&self.providers);
        self.search.batches.clear();
        self.search.failures.clear();
        self.search.results.clear();
        self.search.index = 0;
        self.commands.push(Command::Search { request, query });
    }

    // ── events from the core ────────────────────────────────────────────────

    pub fn on_event(&mut self, event: Event) {
        match event {
            Event::SearchBatch { request, provider, outcome } if request == self.search.request => {
                self.search.pending.retain(|pending| *pending != provider);
                match outcome {
                    Ok(batch) => self.search.batches.push((provider, batch)),
                    Err(problem) => self.search.failures.push((provider, problem)),
                }
                // Apple first, whoever answered first: the order must not jump.
                self.search.batches.sort_by_key(|(provider, _)| *provider);
                let selected = self.search.results.get(self.search.index).cloned();
                let batches: Vec<Vec<PodcastRef>> =
                    self.search.batches.iter().map(|(_, batch)| batch.clone()).collect();
                self.search.results = merge(&batches);
                self.search.index = selected
                    .and_then(|selected| self.search.results.iter().position(|result| same_show(result, &selected)))
                    .unwrap_or(0);
            }
            Event::EpisodeResults { request, outcome } if request == self.search.request => match outcome {
                Ok(episodes) => {
                    let found: Vec<QueueItem> = episodes.iter().map(QueueItem::from_search).collect();
                    self.want_covers_of(found.iter());
                    self.search.episodes = episodes;
                    self.search.episodes_load = Load::Ready;
                }
                Err(problem) => self.search.episodes_load = Load::Failed(problem),
            },
            Event::Playback(state) => {
                if state.now.is_none() {
                    self.levels.clear();
                    self.player_open = false;
                }
                let artwork = state.now.as_ref().and_then(|now| now.item.artwork_url.clone());
                self.want_cover(artwork.as_deref());
                self.up_next_index = self.up_next_index.min(state.up_next.len().saturating_sub(1));
                self.want_covers_of(state.up_next.iter());
                self.playback = *state;
            }
            Event::NewEpisodes { episodes, pending, failed } => {
                self.new_index = self.new_index.min(episodes.len().saturating_sub(1));
                self.want_covers_of(episodes.iter().map(|episode| &episode.item));
                self.new_episodes = episodes;
                (self.refresh_pending, self.refresh_failed) = (pending, failed);
            }
            Event::PodcastIndexVerified(outcome) => {
                self.index_state = match outcome {
                    Ok(()) => IndexState::Accepted,
                    Err(Problem::Refused(401 | 403)) => IndexState::Rejected,
                    Err(_) => IndexState::Unreachable,
                };
                // A key the index refuses would only make every search complain.
                if self.index_state == IndexState::Rejected {
                    self.settings.sources.podcast_index = false;
                    self.settings_changed = true;
                }
            }
            Event::Cover { url, bytes } => {
                if let Some(bytes) = bytes {
                    self.covers.arrived(&url, &bytes);
                }
            }
            Event::Progress(progress) => self.progress = progress,
            Event::Opml(outcome) => {
                self.notice = Some(match (self.lang, outcome) {
                    (Lang::De, OpmlOutcome::Imported { new, known }) => {
                        format!("{new} neue Abos übernommen, {known} waren schon da.")
                    }
                    (Lang::En, OpmlOutcome::Imported { new, known }) => {
                        format!("{new} new subscriptions taken over, {known} were there already.")
                    }
                    (Lang::De, OpmlOutcome::Exported { count, path }) => {
                        format!("{count} Abos nach {path} geschrieben.")
                    }
                    (Lang::En, OpmlOutcome::Exported { count, path }) => {
                        format!("{count} subscriptions written to {path}.")
                    }
                    (lang, OpmlOutcome::Failed) => {
                        lang.t("The file could not be read or written. Check the path.").to_owned()
                    }
                });
            }
            Event::Playlists(playlists) => {
                self.playlists_index = self.playlists_index.min(playlists.len().saturating_sub(1));
                self.want_covers_of(playlists.iter().flat_map(|playlist| playlist.items.iter()));
                if self.open_playlist.as_ref().is_some_and(|open| playlists.iter().all(|playlist| playlist.id != *open))
                {
                    self.open_playlist = None;
                }
                self.playlists = playlists;
                let count = self.opened_playlist().map_or(0, |playlist| playlist.items.len());
                self.playlist_item = self.playlist_item.min(count.saturating_sub(1));
            }
            Event::Downloads(downloads) => {
                self.downloads_index = self.downloads_index.min(downloads.len().saturating_sub(1));
                self.want_covers_of(downloads.iter().map(|download| &download.item));
                self.downloads = downloads;
            }
            Event::Subscriptions(subscriptions) => {
                self.subscriptions_index = self.subscriptions_index.min(subscriptions.len().saturating_sub(1));
                self.subscriptions = subscriptions;
            }
            Event::Level(level) => {
                self.levels.push_back(level);
                while self.levels.len() > LEVELS {
                    self.levels.pop_front();
                }
            }
            Event::Queued { title, place } => {
                let title: String = title.chars().take(40).collect();
                self.notice = Some(match (self.lang, place) {
                    (Lang::De, Some(0)) => format!("„{title}“ liegt jetzt am Anfang von Als Nächstes."),
                    (Lang::De, Some(place)) => {
                        format!("„{title}“ liegt jetzt auf Platz {} von Als Nächstes.", place + 1)
                    }
                    (Lang::De, None) => format!("„{title}“ ist nicht mehr in Als Nächstes."),
                    (Lang::En, Some(0)) => format!("“{title}” is now first in Up Next."),
                    (Lang::En, Some(place)) => format!("“{title}” is now number {} in Up Next.", place + 1),
                    (Lang::En, None) => format!("“{title}” is no longer in Up Next."),
                });
            }
            Event::Charts { request, outcome } if request == self.charts.request => match outcome {
                Ok(list) => {
                    self.charts.list = list;
                    self.charts.index = 0;
                    self.charts.load = Load::Ready;
                }
                Err(problem) => self.charts.load = Load::Failed(problem),
            },
            Event::Categories { outcome } => match outcome {
                Ok(list) => {
                    self.categories.list = list;
                    self.categories.load = Load::Ready;
                }
                Err(problem) => self.categories.load = Load::Failed(problem),
            },
            Event::Feed { request, outcome } => {
                let mut wanted = None;
                let mut cover = None;
                if let Some(view) = self.podcast.as_mut().filter(|view| view.request == request) {
                    match outcome {
                        Ok(podcast) => {
                            // The directory's picture if it gave one; the feed's own otherwise.
                            cover = view.reference.artwork_url.clone().or_else(|| podcast.image.clone());
                            view.reference.artwork_url.clone_from(&cover);
                            view.podcast = Some(podcast);
                            view.load = Load::Ready;
                            view.index = 0;
                            wanted = view.wanted_guid.take();
                        }
                        Err(problem) => view.load = Load::Failed(problem),
                    }
                }
                self.want_cover(cover.as_deref());
                // Came here for one episode: go straight on to it.
                if let (Some(guid), Some(view)) = (wanted, self.podcast.as_mut()) {
                    let position = view.visible().iter().position(|position| {
                        view.podcast
                            .as_ref()
                            .is_some_and(|podcast| podcast.episodes[*position].guid.as_deref() == Some(&guid))
                    });
                    if let Some(position) = position {
                        view.index = position;
                        self.open_episode();
                    }
                }
            }
            Event::Chapters { request, chapters } => {
                if let Some(view) = self.episode.as_mut().filter(|view| view.request == request) {
                    view.looking = None;
                    view.chapters = merge_chapters(std::mem::take(&mut view.chapters), chapters);
                }
            }
            // An answer to a question nobody is asking any more.
            Event::SearchBatch { .. } | Event::Charts { .. } | Event::EpisodeResults { .. } => {}
        }
    }

    // ── navigation ──────────────────────────────────────────────────────────

    fn open_found_episode(&mut self) {
        let Some(found) = self.search.episodes.get(self.search.episode_index).cloned() else { return };
        let reference =
            PodcastRef { title: found.podcast.clone(), feed_url: found.feed_url.clone(), ..PodcastRef::default() };
        self.open_podcast(reference);
        if let Some(view) = &mut self.podcast {
            view.wanted_guid = found.guid;
        }
    }

    /// The episode the selection is on, wherever that is, as something playable.
    fn selected_item(&self) -> Option<QueueItem> {
        if self.section == Section::NewEpisodes {
            return self.new_episodes.get(self.new_index).map(|episode| episode.item.clone());
        }
        if self.section == Section::Downloads {
            return self.downloads.get(self.downloads_index).map(|download| download.item.clone());
        }
        if self.section == Section::Playlists {
            return self.opened_playlist()?.items.get(self.playlist_item).cloned();
        }
        if self.section != Section::Discover {
            return None;
        }
        if let Some(view) = &self.episode {
            return QueueItem::from_feed(&view.podcast, view.feed_url.as_deref(), view.episode());
        }
        if let Some(view) = &self.podcast {
            let podcast = view.podcast.as_ref()?;
            let position = *view.visible().get(view.index)?;
            return QueueItem::from_feed(podcast, view.reference.feed_url.as_deref(), &podcast.episodes[position]);
        }
        if self.tab == Tab::Search && self.search.episodes_mode {
            return self.search.episodes.get(self.search.episode_index).map(QueueItem::from_search);
        }
        None
    }

    /// How much of an episode has been heard, from 0 to 1.
    #[must_use]
    pub fn heard(&self, item: &QueueItem) -> f32 {
        let fraction = |position_ms: u64, duration_ms: Option<u64>| match duration_ms.or(item.duration_ms) {
            Some(duration) if duration > 0 => (position_ms as f32 / duration as f32).clamp(0.0, 1.0),
            _ => 0.0,
        };
        // What plays right now is further than the last place written down.
        if let Some(now) = self.playback.now.as_ref().filter(|now| now.item.key() == item.key()) {
            return fraction(now.position_ms, now.duration_ms);
        }
        match self.progress.get(&item.library_id()) {
            Some(progress) if progress.played => 1.0,
            Some(progress) => fraction(progress.position_ms, progress.duration_ms),
            None => 0.0,
        }
    }

    /// Asks for the pictures of a list's episodes. Lists can be long; the first screenfuls are enough.
    fn want_covers_of<'a>(&mut self, items: impl Iterator<Item = &'a QueueItem>) {
        let urls: Vec<String> = items.take(60).filter_map(|item| item.artwork_url.clone()).collect();
        for url in urls {
            self.want_cover(Some(&url));
        }
    }

    /// Asks for a picture, unless covers are off, it is known, or there is none.
    fn want_cover(&mut self, url: Option<&str>) {
        if let Some(url) = url.filter(|_| self.settings.covers)
            && self.covers.want(url)
        {
            self.commands.push(Command::Cover { url: url.to_owned() });
        }
    }

    fn open_podcast(&mut self, reference: PodcastRef) {
        let Some(feed_url) = reference.feed_url.clone() else {
            self.notice = Some(
                self.lang
                    .t("This podcast has no open feed. It can only be heard inside the platform that hosts it.")
                    .to_owned(),
            );
            return;
        };
        let request = self.next_request();
        self.want_cover(reference.artwork_url.as_deref());
        self.commands.push(Command::OpenFeed { request, feed_url, reload: false });
        self.podcast = Some(PodcastView {
            reference,
            request,
            load: Load::Loading,
            podcast: None,
            index: 0,
            newest_first: true,
            filter: String::new(),
            filtering: false,
            expanded: false,
            wanted_guid: None,
            origin: self.section,
        });
        // A podcast is always looked at within Discover, wherever it was opened from.
        self.section = Section::Discover;
        self.player_open = false;
    }

    fn open_episode(&mut self) {
        let Some(view) = &self.podcast else { return };
        let (Some(podcast), Some(position)) = (view.podcast.clone(), view.visible().get(view.index).copied()) else {
            return;
        };
        let feed_url = view.reference.feed_url.clone();
        let episode = &podcast.episodes[position];
        let notes = episode.notes_html.as_deref().map(notes::document).unwrap_or_default();
        let chapters = episode.chapters.clone();
        let chapters_url = episode.chapters_url.clone();
        // The audio file is only asked when nothing else promises chapters.
        let mp3_url = episode
            .enclosure
            .as_ref()
            .filter(|enclosure| enclosure.is_mp3() && !episode.announces_chapters())
            .map(|enclosure| enclosure.url.clone());
        let looking = if chapters_url.is_some() {
            Some(Looking::ChaptersFile)
        } else if mp3_url.is_some() {
            Some(Looking::AudioFile)
        } else {
            None
        };
        let request = self.next_request();
        if looking.is_some() {
            self.commands.push(Command::Chapters { request, chapters_url, mp3_url });
        }
        self.episode = Some(EpisodeView {
            podcast,
            feed_url,
            position,
            notes,
            chapters,
            looking,
            request,
            focus: Focus::Notes,
            scroll: 0,
            chapter_index: 0,
        });
    }

    fn load_charts(&mut self, category: Option<Category>) {
        let request = self.next_request();
        self.charts.request = request;
        self.charts.load = Load::Loading;
        self.charts.list.clear();
        self.commands.push(Command::Charts { request, category: category.as_ref().map(|category| category.id) });
        self.charts.category = category;
    }

    fn enter_tab(&mut self, tab: Tab) {
        self.tab = tab;
        match tab {
            Tab::Charts if self.charts.load == Load::Idle => self.load_charts(None),
            Tab::Categories if self.categories.load == Load::Idle => {
                self.categories.load = Load::Loading;
                self.commands.push(Command::Categories);
            }
            _ => {}
        }
    }

    fn cycle_tab(&mut self, step: isize) {
        let position = Tab::ALL.iter().position(|tab| *tab == self.tab).unwrap_or(0);
        let next = (position as isize + step).rem_euclid(Tab::ALL.len() as isize) as usize;
        self.enter_tab(Tab::ALL[next]);
    }

    // ── keys ────────────────────────────────────────────────────────────────

    pub fn on_key(&mut self, key: KeyEvent) {
        self.notice = None;
        if key.modifiers.contains(KeyModifiers::CONTROL) {
            match key.code {
                KeyCode::Char('c' | 'q') => self.should_quit = true,
                KeyCode::Char('u') if self.is_typing() => self.edit(|text| text.clear()),
                KeyCode::Char('e') => self.toggle_episode_search(),
                _ => {}
            }
            return;
        }
        if self.is_typing() {
            self.on_typing_key(key.code);
            return;
        }
        if !matches!(key.code, KeyCode::Char('C')) {
            self.confirm_clear = false;
        }
        if !matches!(key.code, KeyCode::Char('d')) {
            self.confirm_delete = false;
        }
        if self.picker.is_some() {
            self.on_picker_key(key.code);
            return;
        }
        if self.on_transport_key(key.code) {
            return;
        }
        if self.player_open {
            self.on_player_key(key.code);
            return;
        }
        match key.code {
            KeyCode::Char('q') => self.should_quit = true,
            KeyCode::Char('?') => self.section = Section::Help,
            // Inside an episode the digits belong to the links.
            KeyCode::Char(digit @ '1'..='8') if !(self.section == Section::Discover && self.episode.is_some()) => {
                self.section = Section::ALL[digit as usize - '1' as usize];
            }
            code => match self.section {
                Section::Discover => self.on_discover_key(code),
                Section::Subscriptions => self.on_subscriptions_key(code),
                Section::NewEpisodes => self.on_new_episodes_key(code),
                Section::UpNext => self.on_up_next_key(code),
                Section::Playlists => self.on_playlists_key(code),
                Section::Downloads => self.on_downloads_key(code),
                Section::Settings => self.on_settings_key(code),
                Section::Help => {
                    if matches!(code, KeyCode::Esc | KeyCode::Backspace | KeyCode::Left) {
                        self.section = Section::Discover;
                    }
                }
            },
        }
    }

    /// The keys that drive playback work on every screen. `true` if `code` was one.
    fn on_transport_key(&mut self, code: KeyCode) -> bool {
        if code == KeyCode::Char('0') {
            if self.playback.now.is_some() {
                self.player_open = !self.player_open;
                self.player_chapter = self.playback.now.as_ref().and_then(|now| now.chapter_index()).unwrap_or(0);
            }
            return true;
        }
        if code == KeyCode::Char('L')
            && !self.player_open
            && let Some(item) = self.selected_item()
        {
            self.picker = Some((item, 0));
            return true;
        }
        if code == KeyCode::Char('D')
            && !self.player_open
            && let Some(item) = self.selected_item()
        {
            self.commands.push(Command::Download(item));
            return true;
        }
        // Queueing works wherever an episode is selected.
        if let KeyCode::Char(key @ ('a' | 'A' | 'p')) = code
            && !self.player_open
            && let Some(item) = self.selected_item()
        {
            self.transport(match key {
                'p' => Transport::PlayNow(item),
                key => Transport::Enqueue { item, first: key == 'A' },
            });
            return true;
        }
        if self.playback.now.is_none() {
            return false;
        }
        let transport = match code {
            KeyCode::Char(' ') => Transport::Toggle,
            KeyCode::Char('x') => Transport::Stop,
            KeyCode::Char(',') => Transport::PreviousChapter,
            KeyCode::Char('.') => Transport::NextChapter,
            KeyCode::Char('n') => Transport::NextEpisode,
            KeyCode::Char('b') => Transport::SeekBy(SEEK_BACK_MS),
            KeyCode::Char('f') => Transport::SeekBy(SEEK_FORWARD_MS),
            KeyCode::Char('-') => Transport::SpeedBy(-0.1),
            KeyCode::Char('+' | '=') => Transport::SpeedBy(0.1),
            KeyCode::Char('t') => Transport::CycleSleep,
            _ => return false,
        };
        self.transport(transport);
        true
    }

    fn on_player_key(&mut self, code: KeyCode) {
        let chapters: Vec<u64> = self
            .playback
            .now
            .as_ref()
            .map(|now| now.chapters().iter().map(|chapter| chapter.start_ms).collect())
            .unwrap_or_default();
        match code {
            KeyCode::Esc | KeyCode::Left | KeyCode::Char('h') | KeyCode::Backspace => self.player_open = false,
            KeyCode::Char('q') => self.should_quit = true,
            KeyCode::Char('?') => (self.section, self.player_open) = (Section::Help, false),
            // The menu stays one key away, as on every other screen.
            KeyCode::Char(digit @ '1'..='8') => {
                self.section = Section::ALL[digit as usize - '1' as usize];
                self.player_open = false;
            }
            KeyCode::Enter => {
                if let Some(start) = chapters.get(self.player_chapter) {
                    self.transport(Transport::SeekTo(*start));
                }
            }
            code => move_selection(&mut self.player_chapter, chapters.len(), code),
        }
    }

    fn on_subscriptions_key(&mut self, code: KeyCode) {
        match code {
            // Bring subscriptions along from another client, or take them elsewhere.
            KeyCode::Char('I') => self.opml_path = Some((true, "~/".to_owned())),
            KeyCode::Char('E') => self.opml_path = Some((false, "~/torrocast-abos.opml".to_owned())),
            KeyCode::Enter | KeyCode::Right | KeyCode::Char('l') => {
                if let Some(subscription) = self.subscriptions.get(self.subscriptions_index) {
                    let reference = PodcastRef {
                        title: subscription.title.clone(),
                        feed_url: Some(subscription.feed_url.clone()),
                        ..PodcastRef::default()
                    };
                    self.open_podcast(reference);
                }
            }
            code => move_selection(&mut self.subscriptions_index, self.subscriptions.len(), code),
        }
    }

    fn on_new_episodes_key(&mut self, code: KeyCode) {
        match code {
            KeyCode::Char('r') => self.commands.push(Command::RefreshSubscriptions),
            // The episode with its notes and chapters lives in its podcast: open that, then the episode.
            KeyCode::Enter | KeyCode::Right | KeyCode::Char('l') => {
                if let Some(episode) = self.new_episodes.get(self.new_index).cloned() {
                    let reference = PodcastRef {
                        title: episode.item.podcast.clone(),
                        feed_url: episode.item.feed_url.clone(),
                        ..PodcastRef::default()
                    };
                    self.open_podcast(reference);
                    if let Some(view) = &mut self.podcast {
                        view.wanted_guid = episode.item.guid;
                    }
                }
            }
            code => move_selection(&mut self.new_index, self.new_episodes.len(), code),
        }
    }

    /// Whether the podcast on screen is one the user follows.
    #[must_use]
    pub fn is_subscribed(&self, feed_url: &str) -> bool {
        // Directories spell one address in several ways: with http or https, with or without a closing slash.
        let wanted = normalise_feed_url(feed_url);
        self.subscriptions.iter().any(|subscription| normalise_feed_url(&subscription.feed_url) == wanted)
    }

    fn toggle_subscription(&mut self) {
        let Some(view) = &self.podcast else { return };
        let (Some(feed_url), Some(podcast)) = (view.reference.feed_url.clone(), view.podcast.as_ref()) else { return };
        let subscribed = !self.is_subscribed(&feed_url);
        let title = podcast.title.clone();
        self.notice = Some(match (self.lang, subscribed) {
            (Lang::De, true) => format!("„{title}“ ist jetzt abonniert."),
            (Lang::De, false) => format!("„{title}“ ist nicht mehr abonniert."),
            (Lang::En, true) => format!("Subscribed to “{title}”."),
            (Lang::En, false) => format!("No longer subscribed to “{title}”."),
        });
        self.commands.push(Command::SetSubscribed { feed_url, title, guid: podcast.guid.clone(), subscribed });
    }

    /// Whether an episode is on this machine, on its way, or neither.
    #[must_use]
    pub fn download_state(&self, item_key: &str) -> Option<DownloadState> {
        self.downloads.iter().find(|download| download.item.key() == item_key).map(|download| download.state)
    }

    /// The playlist whose episodes are on screen.
    #[must_use]
    pub fn opened_playlist(&self) -> Option<&Playlist> {
        let open = self.open_playlist.as_ref()?;
        self.playlists.iter().find(|playlist| playlist.id == *open)
    }

    /// Choosing a playlist for an episode: the playlists, and below them "a new one".
    fn on_picker_key(&mut self, code: KeyCode) {
        let Some((item, index)) = self.picker.clone() else { return };
        match code {
            KeyCode::Esc | KeyCode::Char('q') => self.picker = None,
            KeyCode::Enter => {
                self.picker = None;
                match self.playlists.get(index) {
                    Some(playlist) => {
                        let (name, title): (String, String) =
                            (playlist.name.clone(), item.title.chars().take(40).collect());
                        self.notice = Some(match self.lang {
                            Lang::De => format!("„{title}“ liegt jetzt in „{name}“."),
                            Lang::En => format!("“{title}” is now in “{name}”."),
                        });
                        self.commands
                            .push(Command::Playlist(PlaylistCommand::Add { playlist: playlist.id.clone(), item }));
                    }
                    None => self.playlist_name = Some((Some(item), String::new())),
                }
            }
            code => {
                let mut index = index;
                move_selection(&mut index, self.playlists.len() + 1, code);
                self.picker = Some((item, index));
            }
        }
    }

    fn on_playlists_key(&mut self, code: KeyCode) {
        if let Some(playlist) = self.opened_playlist().cloned() {
            match code {
                KeyCode::Esc | KeyCode::Left | KeyCode::Char('h') | KeyCode::Backspace => self.open_playlist = None,
                KeyCode::Enter => {
                    if let Some(item) = self.selected_item() {
                        self.transport(Transport::PlayNow(item));
                    }
                }
                KeyCode::Char('d') | KeyCode::Delete => {
                    if let Some(item) = self.selected_item() {
                        self.commands.push(Command::Playlist(PlaylistCommand::Remove { playlist: playlist.id, item }));
                    }
                }
                code => move_selection(&mut self.playlist_item, playlist.items.len(), code),
            }
            return;
        }
        let chosen = self.playlists.get(self.playlists_index).map(|playlist| playlist.id.clone());
        match (code, chosen) {
            (KeyCode::Char('N'), _) => self.playlist_name = Some((None, String::new())),
            (KeyCode::Enter | KeyCode::Right | KeyCode::Char('l'), Some(playlist)) => {
                self.open_playlist = Some(playlist);
                self.playlist_item = 0;
            }
            (KeyCode::Char(key @ ('a' | 'A')), Some(playlist)) => {
                self.commands.push(Command::Playlist(PlaylistCommand::Queue { playlist, first: key == 'A' }));
            }
            // A playlist is more than one keystroke's worth of work: ask once.
            (KeyCode::Char('d') | KeyCode::Delete, Some(playlist)) => {
                if self.confirm_delete {
                    self.confirm_delete = false;
                    self.commands.push(Command::Playlist(PlaylistCommand::Delete(playlist)));
                } else {
                    self.confirm_delete = true;
                    self.notice = Some(self.lang.t("Press d again to delete the playlist.").to_owned());
                }
            }
            (code, _) => move_selection(&mut self.playlists_index, self.playlists.len(), code),
        }
    }

    fn on_downloads_key(&mut self, code: KeyCode) {
        match code {
            KeyCode::Enter => {
                if let Some(item) = self.selected_item() {
                    self.transport(Transport::PlayNow(item));
                }
            }
            KeyCode::Char('d') | KeyCode::Delete => {
                if let Some(download) = self.downloads.get(self.downloads_index) {
                    self.commands.push(Command::DeleteDownload(download.item.library_id()));
                }
            }
            code => move_selection(&mut self.downloads_index, self.downloads.len(), code),
        }
    }

    fn on_up_next_key(&mut self, code: KeyCode) {
        let count = self.playback.up_next.len();
        let index = self.up_next_index;
        match code {
            KeyCode::Enter | KeyCode::Char('p') if index < count => self.transport(Transport::PlayQueued(index)),
            KeyCode::Char('d') | KeyCode::Delete if index < count => self.transport(Transport::Remove(index)),
            KeyCode::Char('J') if index + 1 < count => {
                self.transport(Transport::Shift { index, down: true });
                self.up_next_index += 1;
            }
            KeyCode::Char('K') if index > 0 && index < count => {
                self.transport(Transport::Shift { index, down: false });
                self.up_next_index -= 1;
            }
            KeyCode::Char('C') if count > 0 => {
                if self.confirm_clear {
                    self.transport(Transport::Clear);
                    self.confirm_clear = false;
                } else {
                    self.confirm_clear = true;
                    self.notice = Some(self.lang.t("Press C again to empty the list.").to_owned());
                }
            }
            code => move_selection(&mut self.up_next_index, count, code),
        }
    }

    fn toggle_episode_search(&mut self) {
        if self.section == Section::Discover && self.podcast.is_none() && self.tab == Tab::Search {
            self.search.episodes_mode = !self.search.episodes_mode;
            self.search.sent.clear();
            self.start_search();
        }
    }

    /// Applies `change` to whichever text field has the keyboard.
    fn edit(&mut self, change: impl FnOnce(&mut String)) {
        if let Some((_, path)) = &mut self.opml_path {
            change(path);
        } else if let Some((_, name)) = &mut self.playlist_name {
            change(name);
        } else if let Some((_, text)) = &mut self.settings_input {
            change(text);
        } else if let Some(view) = self.podcast.as_mut().filter(|view| view.filtering) {
            change(&mut view.filter);
            view.index = 0;
        } else {
            change(&mut self.search.input);
            self.search.typed_at = Some(Instant::now());
        }
    }

    fn on_typing_key(&mut self, code: KeyCode) {
        if self.opml_path.is_some() {
            match code {
                KeyCode::Char(character) => self.edit(|text| text.push(character)),
                KeyCode::Backspace => self.edit(|text| {
                    text.pop();
                }),
                KeyCode::Esc => self.opml_path = None,
                KeyCode::Enter => {
                    if let Some((import, path)) = self.opml_path.take().filter(|(_, path)| !path.trim().is_empty()) {
                        self.commands.push(if import {
                            Command::ImportOpml { path }
                        } else {
                            Command::ExportOpml { path }
                        });
                    }
                }
                _ => {}
            }
            return;
        }
        if self.playlist_name.is_some() {
            match code {
                KeyCode::Char(character) => self.edit(|text| text.push(character)),
                KeyCode::Backspace => self.edit(|text| {
                    text.pop();
                }),
                KeyCode::Esc => self.playlist_name = None,
                KeyCode::Enter => {
                    if let Some((first, name)) = self.playlist_name.take().filter(|(_, name)| !name.trim().is_empty()) {
                        self.commands.push(Command::Playlist(PlaylistCommand::Create { name, first }));
                    }
                }
                _ => {}
            }
            return;
        }
        if self.settings_input.is_some() {
            match code {
                KeyCode::Char(character) => self.edit(|text| text.push(character)),
                KeyCode::Backspace => self.edit(|text| {
                    text.pop();
                }),
                KeyCode::Enter => self.finish_settings_input(),
                KeyCode::Esc => self.settings_input = None,
                _ => {}
            }
            return;
        }
        match code {
            KeyCode::Char(character) => self.edit(|text| text.push(character)),
            KeyCode::Backspace => self.edit(|text| {
                text.pop();
            }),
            KeyCode::Enter | KeyCode::Down | KeyCode::Esc => {
                if let Some(view) = self.podcast.as_mut().filter(|view| view.filtering) {
                    view.filtering = false;
                    if code == KeyCode::Esc {
                        view.filter.clear();
                    }
                } else {
                    self.search.editing = false;
                    if code == KeyCode::Enter {
                        self.start_search();
                    }
                }
            }
            KeyCode::Tab if self.podcast.is_none() => {
                self.search.editing = false;
                self.cycle_tab(1);
            }
            KeyCode::BackTab if self.podcast.is_none() => {
                self.search.editing = false;
                self.cycle_tab(-1);
            }
            _ => {}
        }
    }

    fn on_discover_key(&mut self, code: KeyCode) {
        if self.episode.is_some() {
            self.on_episode_key(code);
        } else if self.podcast.is_some() {
            self.on_podcast_key(code);
        } else {
            self.on_root_key(code);
        }
    }

    fn on_root_key(&mut self, code: KeyCode) {
        match code {
            KeyCode::Char('/') => {
                self.tab = Tab::Search;
                self.search.editing = true;
            }
            KeyCode::Tab => self.cycle_tab(1),
            KeyCode::BackTab => self.cycle_tab(-1),
            KeyCode::Char('e') => self.toggle_episode_search(),
            KeyCode::Char('r') => match self.tab {
                Tab::Charts => self.load_charts(self.charts.category.clone()),
                Tab::Categories => {
                    self.categories.load = Load::Loading;
                    self.commands.push(Command::Categories);
                }
                Tab::Search => {
                    self.search.sent.clear();
                    self.start_search();
                }
            },
            KeyCode::Esc | KeyCode::Backspace if self.tab == Tab::Charts && self.charts.category.is_some() => {
                self.load_charts(None)
            }
            KeyCode::Up | KeyCode::Char('k') if self.tab == Tab::Search && self.search_selection() == 0 => {
                self.search.editing = true
            }
            KeyCode::Enter | KeyCode::Right | KeyCode::Char('l') => match self.tab {
                Tab::Search if self.search.episodes_mode => self.open_found_episode(),
                Tab::Search => {
                    if let Some(reference) = self.search.results.get(self.search.index).cloned() {
                        self.open_podcast(reference);
                    }
                }
                Tab::Charts => {
                    if let Some(reference) = self.charts.list.get(self.charts.index).cloned() {
                        self.open_podcast(reference);
                    }
                }
                Tab::Categories => {
                    if let Some(category) = self.categories.list.get(self.categories.index).cloned() {
                        self.load_charts(Some(category));
                        self.tab = Tab::Charts;
                    }
                }
            },
            code => {
                let (index, count) = match self.tab {
                    Tab::Search if self.search.episodes_mode => {
                        (&mut self.search.episode_index, self.search.episodes.len())
                    }
                    Tab::Search => (&mut self.search.index, self.search.results.len()),
                    Tab::Charts => (&mut self.charts.index, self.charts.list.len()),
                    Tab::Categories => (&mut self.categories.index, self.categories.list.len()),
                };
                move_selection(index, count, code);
            }
        }
    }

    fn search_selection(&self) -> usize {
        if self.search.episodes_mode { self.search.episode_index } else { self.search.index }
    }

    fn on_podcast_key(&mut self, code: KeyCode) {
        let Some(view) = self.podcast.as_mut() else { return };
        match code {
            KeyCode::Esc | KeyCode::Left | KeyCode::Char('h') | KeyCode::Backspace => {
                self.section = view.origin;
                self.podcast = None;
            }
            KeyCode::Char('s') => self.toggle_subscription(),
            KeyCode::Enter | KeyCode::Right | KeyCode::Char('l') => self.open_episode(),
            KeyCode::Char('/') => view.filtering = true,
            KeyCode::Char('o') => {
                view.newest_first = !view.newest_first;
                view.index = 0;
            }
            KeyCode::Char('m') => view.expanded = !view.expanded,
            KeyCode::Char('w') => {
                let website = view
                    .podcast
                    .as_ref()
                    .and_then(|podcast| podcast.website.clone())
                    .or_else(|| view.reference.website.clone());
                self.open_urls.extend(website);
            }
            KeyCode::Char('r') => {
                if let Some(feed_url) = view.reference.feed_url.clone() {
                    view.load = Load::Loading;
                    let request = view.request;
                    self.commands.push(Command::OpenFeed { request, feed_url, reload: true });
                }
            }
            code => {
                let count = view.visible().len();
                move_selection(&mut view.index, count, code);
            }
        }
    }

    fn on_episode_key(&mut self, code: KeyCode) {
        let Some(view) = self.episode.as_mut() else { return };
        let chapters = view.listed_chapters().len();
        match code {
            KeyCode::Esc | KeyCode::Left | KeyCode::Char('h') | KeyCode::Backspace => self.episode = None,
            KeyCode::Tab | KeyCode::BackTab if chapters > 0 => {
                view.focus = if view.focus == Focus::Notes { Focus::Chapters } else { Focus::Notes };
            }
            KeyCode::Char(digit @ '1'..='9') => {
                let link = view.notes.links.get(digit as usize - '1' as usize).cloned();
                self.open_urls.extend(link);
            }
            KeyCode::Char('w') => {
                let link = view.episode().link.clone();
                self.open_urls.extend(link);
            }
            KeyCode::Enter if view.focus == Focus::Chapters => {
                let link = view.listed_chapters().get(view.chapter_index).and_then(|chapter| chapter.url.clone());
                self.open_urls.extend(link);
            }
            code if view.focus == Focus::Chapters => move_selection(&mut view.chapter_index, chapters, code),
            // The end of the notes is known only to the drawing code, which clamps.
            KeyCode::Up | KeyCode::Char('k') => view.scroll = view.scroll.saturating_sub(1),
            KeyCode::Down | KeyCode::Char('j') => view.scroll = view.scroll.saturating_add(1),
            KeyCode::PageUp => view.scroll = view.scroll.saturating_sub(10),
            KeyCode::PageDown => view.scroll = view.scroll.saturating_add(10),
            KeyCode::Home | KeyCode::Char('g') => view.scroll = 0,
            KeyCode::End | KeyCode::Char('G') => view.scroll = u16::MAX,
            _ => {}
        }
    }

    /// The rows of the settings: 0 Apple, 1 Podcast Index, 2 fyyd, 3 country, 4 library folder, 5 covers.
    pub const SETTINGS_ROWS: usize = 6;

    fn on_settings_key(&mut self, code: KeyCode) {
        match (self.settings_index, code) {
            (2, KeyCode::Char(' ') | KeyCode::Enter) => {
                self.settings.sources.fyyd = !self.settings.sources.fyyd;
                self.settings_changed = true;
            }
            (3, KeyCode::Char(' ') | KeyCode::Enter | KeyCode::Right | KeyCode::Char('l')) => self.cycle_country(1),
            (3, KeyCode::Left | KeyCode::Char('h')) => self.cycle_country(-1),
            (5, KeyCode::Enter | KeyCode::Char(' ')) => {
                self.settings.covers = !self.settings.covers;
                self.settings_changed = true;
            }
            (4, KeyCode::Enter) => {
                self.settings_input = Some((Field::Library, self.library.clone().unwrap_or_default()))
            }
            // Podcast Index: without a key enter asks for one; with a key it switches, and e asks for another.
            (1, KeyCode::Enter | KeyCode::Char(' ')) if self.has_index_key() => {
                self.settings.sources.podcast_index = !self.settings.sources.podcast_index;
                self.settings_changed = true;
                if self.settings.sources.podcast_index {
                    self.index_state = IndexState::Checking;
                    self.commands.push(Command::VerifyPodcastIndex);
                }
            }
            (1, KeyCode::Enter | KeyCode::Char('e')) => {
                self.settings_input = Some((Field::IndexKey, self.settings.sources.podcast_index_key.clone()));
            }
            (_, code) => move_selection(&mut self.settings_index, Self::SETTINGS_ROWS, code),
        }
    }

    fn has_index_key(&self) -> bool {
        let sources = &self.settings.sources;
        !sources.podcast_index_key.is_empty() && !sources.podcast_index_secret.is_empty()
    }

    fn finish_settings_input(&mut self) {
        let Some((field, text)) = self.settings_input.take() else { return };
        let text = text.trim().to_owned();
        match field {
            Field::Library => self.library_request = Some(text).filter(|folder| !folder.is_empty()),
            Field::IndexKey | Field::IndexSecret if text.is_empty() => {}
            Field::IndexKey => {
                self.settings.sources.podcast_index_key = text;
                self.settings_input = Some((Field::IndexSecret, String::new()));
            }
            // Key and secret are there: switch the directory on and ask it whether it agrees.
            Field::IndexSecret => {
                self.settings.sources.podcast_index_secret = text;
                self.settings.sources.podcast_index = true;
                self.settings_changed = true;
                self.index_state = IndexState::Checking;
                self.commands.push(Command::VerifyPodcastIndex);
            }
        }
    }

    fn cycle_country(&mut self, step: isize) {
        let position = COUNTRIES.iter().position(|country| *country == self.settings.country);
        let next =
            position.map_or(0, |position| (position as isize + step).rem_euclid(COUNTRIES.len() as isize) as usize);
        self.settings.country = COUNTRIES[next].to_owned();
        self.settings_changed = true;
        // Charts and categories belong to the country they were loaded for.
        self.charts = Charts { request: 0, category: None, list: Vec::new(), index: 0, load: Load::Idle };
        self.categories = Categories { list: Vec::new(), index: 0, load: Load::Idle };
        self.search.sent.clear();
    }

    // ── mouse ───────────────────────────────────────────────────────────────

    pub fn on_mouse(&mut self, mouse: MouseEvent) {
        match mouse.kind {
            MouseEventKind::ScrollUp => self.on_wheel(KeyCode::Up),
            MouseEventKind::ScrollDown => self.on_wheel(KeyCode::Down),
            MouseEventKind::Down(MouseButton::Left) if mouse.column >= MENU_WIDTH => {
                // Later things are drawn over earlier ones; the last hit is the one on top.
                let at = Position { x: mouse.column, y: mouse.row };
                let hit = self.hits.borrow().iter().rev().find(|hit| hit.area.contains(at)).copied();
                match hit.map(|hit| (hit.area, hit.target)) {
                    Some((_, HitTarget::Tab(tab))) if !self.player_open && self.podcast.is_none() => {
                        self.search.editing = false;
                        self.enter_tab(tab);
                    }
                    Some((area, HitTarget::Rows { first, rows_each, count })) => {
                        let index = first + usize::from((mouse.row - area.y) / rows_each.max(1));
                        if index < count {
                            self.click_row(index);
                        }
                    }
                    _ => {}
                }
            }
            MouseEventKind::Down(MouseButton::Left) if mouse.column < MENU_WIDTH => {
                if self.on_player_click(mouse.column, mouse.row) {
                    return;
                }
                let entry = usize::from(mouse.row.saturating_sub(MENU_FIRST_ROW));
                if mouse.row >= MENU_FIRST_ROW && entry < Section::ALL.len() {
                    self.section = Section::ALL[entry];
                    self.player_open = false;
                }
            }
            _ => {}
        }
    }

    /// The selection of whichever list has the keyboard right now.
    fn selection(&mut self) -> Option<&mut usize> {
        if let Some((_, index)) = &mut self.picker {
            return Some(index);
        }
        if self.player_open {
            return Some(&mut self.player_chapter);
        }
        Some(match self.section {
            Section::Discover => match (&mut self.episode, &mut self.podcast) {
                (Some(view), _) => {
                    // The only list in an episode is its chapters; a click there takes the focus along.
                    view.focus = Focus::Chapters;
                    &mut view.chapter_index
                }
                (None, Some(view)) => &mut view.index,
                (None, None) => match self.tab {
                    Tab::Search if self.search.episodes_mode => &mut self.search.episode_index,
                    Tab::Search => &mut self.search.index,
                    Tab::Charts => &mut self.charts.index,
                    Tab::Categories => &mut self.categories.index,
                },
            },
            Section::Subscriptions => &mut self.subscriptions_index,
            Section::NewEpisodes => &mut self.new_index,
            Section::UpNext => &mut self.up_next_index,
            Section::Playlists if self.open_playlist.is_some() => &mut self.playlist_item,
            Section::Playlists => &mut self.playlists_index,
            Section::Downloads => &mut self.downloads_index,
            Section::Settings | Section::Help => return None,
        })
    }

    /// A click on a row selects it; a click on the selected row opens it, as enter would.
    fn click_row(&mut self, index: usize) {
        self.search.editing = false;
        let Some(selection) = self.selection() else { return };
        if *selection == index {
            self.on_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        } else {
            *selection = index;
        }
    }

    /// Rows of the small player for the current terminal height.
    #[must_use]
    pub fn player_rows(&self) -> u16 {
        if self.terminal_height >= METER_FROM_HEIGHT { PLAYER_ROWS } else { PLAYER_ROWS_SHORT }
    }

    /// A click on the small player: its buttons, its progress bar, or the player itself.
    fn on_player_click(&mut self, column: u16, row: u16) -> bool {
        if !self.shows_mini_player() || self.terminal_height == 0 {
            return false;
        }
        let rows = self.player_rows();
        let top = self.terminal_height.saturating_sub(1 + rows);
        if row < top || row >= top + rows {
            return false;
        }
        // Rows inside the block, counted from its last: tempo, keys, buttons, gap, chapter, times, bar.
        let from_bottom = top + rows - 1 - row;
        match from_bottom {
            2 | 3 => self.transport(match column {
                0..=4 => Transport::PreviousChapter,
                5..=9 => Transport::Toggle,
                10..=13 => Transport::Stop,
                14..=18 => Transport::NextChapter,
                _ => Transport::NextEpisode,
            }),
            7 => {
                let duration = self.playback.now.as_ref().and_then(|now| now.duration_ms);
                if let (Some(duration), 2..=23) = (duration, column) {
                    self.transport(Transport::SeekTo(duration * u64::from(column - 2) / 22));
                }
            }
            _ => {
                self.player_open = true;
                self.player_chapter = self.playback.now.as_ref().and_then(|now| now.chapter_index()).unwrap_or(0);
            }
        }
        true
    }

    fn on_wheel(&mut self, code: KeyCode) {
        if self.player_open {
            self.on_player_key(code);
            return;
        }
        match self.section {
            Section::Discover => {
                // The wheel moves what is shown, never the text being typed.
                if self.search.editing && self.podcast.is_none() {
                    self.search.editing = false;
                }
                self.on_discover_key(code);
            }
            Section::Subscriptions => self.on_subscriptions_key(code),
            Section::NewEpisodes => self.on_new_episodes_key(code),
            Section::UpNext => self.on_up_next_key(code),
            Section::Playlists => self.on_playlists_key(code),
            Section::Downloads => self.on_downloads_key(code),
            Section::Settings => self.on_settings_key(code),
            Section::Help => {}
        }
    }
}

fn same_show(left: &PodcastRef, right: &PodcastRef) -> bool {
    (left.itunes_id.is_some() && left.itunes_id == right.itunes_id)
        || (left.feed_url.is_some() && left.feed_url == right.feed_url)
}

fn move_selection(index: &mut usize, count: usize, code: KeyCode) {
    let last = count.saturating_sub(1);
    *index = match code {
        KeyCode::Up | KeyCode::Char('k') => index.saturating_sub(1),
        KeyCode::Down | KeyCode::Char('j') => index.saturating_add(1),
        KeyCode::PageUp => index.saturating_sub(10),
        KeyCode::PageDown => index.saturating_add(10),
        KeyCode::Home | KeyCode::Char('g') => 0,
        KeyCode::End | KeyCode::Char('G') => last,
        _ => *index,
    }
    .min(last);
}
