//! Keeps the library folder and playback in step: what changes while
//! listening is written down, and what other devices wrote is taken in.

use std::collections::HashMap;
use std::path::Path;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use torrocast_library::{Change, Folder, OpenError, StoredItem, Subscription, UP_NEXT};

use crate::playback::{Playback, ProgressNote, QueueItem};

/// While an episode plays its place is saved this often. Often enough to lose
/// little, rarely enough not to keep a sync service busy.
const SAVE_EVERY: Duration = Duration::from_secs(60);
/// How often the folder is looked at for what other devices have written.
const LOOK_EVERY: Duration = Duration::from_secs(30);

fn now_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |since| since.as_millis() as u64)
}

impl From<&QueueItem> for StoredItem {
    fn from(item: &QueueItem) -> Self {
        Self {
            title: item.title.clone(),
            podcast: item.podcast.clone(),
            feed_url: item.feed_url.clone(),
            guid: item.guid.clone(),
            audio_url: item.audio_url.clone(),
            duration_ms: item.duration_ms,
            chapters_url: item.chapters_url.clone(),
            is_mp3: item.is_mp3,
            artwork_url: item.artwork_url.clone(),
        }
    }
}

impl From<StoredItem> for QueueItem {
    fn from(item: StoredItem) -> Self {
        Self {
            title: item.title,
            podcast: item.podcast,
            feed_url: item.feed_url,
            guid: item.guid,
            audio_url: item.audio_url,
            duration_ms: item.duration_ms,
            // Chapters are looked up again when the episode plays.
            chapters: Vec::new(),
            chapters_url: item.chapters_url,
            is_mp3: item.is_mp3,
            artwork_url: item.artwork_url,
        }
    }
}

/// A playlist of the user's own.
#[derive(Debug, Clone, PartialEq)]
pub struct Playlist {
    pub id: String,
    pub name: String,
    pub items: Vec<QueueItem>,
}

pub struct Keeper {
    folder: Folder,
    device: String,
    device_name: String,
    saved: Instant,
    looked: Instant,
}

impl Keeper {
    pub fn open(directory: &Path, device: &str, device_name: &str) -> Result<Self, OpenError> {
        let folder = Folder::open(directory, device, device_name, now_ms())?;
        Ok(Self {
            folder,
            device: device.to_owned(),
            device_name: device_name.to_owned(),
            saved: Instant::now(),
            looked: Instant::now(),
        })
    }

    /// The same library in another folder — say, inside Dropbox. What this
    /// device has written goes along; what is already there is merged in, as if
    /// the two had always been one folder.
    pub fn relocate(&self, directory: &Path) -> Result<Self, OpenError> {
        let from = self.folder.root().join("devices").join(&self.device);
        let to = directory.join("devices").join(&self.device);
        if from != to {
            std::fs::create_dir_all(&to)?;
            for entry in std::fs::read_dir(&from)?.filter_map(Result::ok) {
                let target = to.join(entry.file_name());
                // Never over a file that is already there: it may hold more than ours.
                if entry.path().is_file() && !target.exists() {
                    std::fs::copy(entry.path(), target)?;
                }
            }
        }
        Self::open(directory, &self.device, &self.device_name)
    }

    #[must_use]
    pub fn directory(&self) -> &Path {
        self.folder.root()
    }

    #[must_use]
    pub fn subscriptions(&self) -> Vec<Subscription> {
        self.folder.state().subscriptions()
    }

    /// Hands playback what the library knows: the queue, and where episodes were left.
    pub fn restore(&self, playback: &mut Playback) {
        let state = self.folder.state();
        let up_next: Vec<QueueItem> = state.playlist(UP_NEXT).into_iter().map(|(_, _, item)| item.into()).collect();
        let positions: HashMap<String, u64> = up_next
            .iter()
            .map(QueueItem::library_id)
            .chain(playback.now.iter().map(|now| now.item.library_id()))
            .filter_map(|id| Some((id.clone(), state.progress(&id).filter(|progress| !progress.played)?.position_ms)))
            .collect();
        playback.restore(up_next, positions);
    }

    /// The place an episode was left at, on this device or any other.
    #[must_use]
    pub fn position_of(&self, item: &QueueItem) -> Option<u64> {
        self.folder
            .state()
            .progress(&item.library_id())
            .filter(|progress| !progress.played)
            .map(|progress| progress.position_ms)
    }

    pub fn set_subscribed(&mut self, podcast: String, feed_url: String, title: String, subscribed: bool) {
        let change =
            if subscribed { Change::Subscribed { podcast, feed_url, title } } else { Change::Unsubscribed { podcast } };
        // A library that cannot be written still works for this session.
        let _ = self.folder.record(change, now_ms());
    }

    /// Heard to the end, on this device or any other.
    #[must_use]
    pub fn is_played(&self, item: &QueueItem) -> bool {
        self.folder.state().progress(&item.library_id()).is_some_and(|progress| progress.played)
    }

    #[must_use]
    pub fn is_subscribed(&self, podcast: &str) -> bool {
        self.folder.state().is_subscribed(podcast)
    }

    pub fn save_notes(&mut self, notes: Vec<ProgressNote>) {
        for note in notes {
            let change = Change::PlaybackUpdated {
                episode: note.key,
                position_ms: note.position_ms,
                duration_ms: note.duration_ms,
                played: note.played,
            };
            let _ = self.folder.record(change, now_ms());
        }
    }

    /// The periodic save of the playing episode's place.
    pub fn save_now_and_then(&mut self, playback: &Playback) {
        if self.saved.elapsed() >= SAVE_EVERY {
            self.saved = Instant::now();
            self.save_notes(playback.note_now().into_iter().collect());
        }
    }

    /// Writes down how Up Next differs from what the library has. One entry
    /// added or taken out costs one line; only a reordering rewrites the list.
    pub fn save_queue(&mut self, up_next: &[QueueItem]) {
        let stored = self.folder.state().playlist(UP_NEXT);
        let wanted: Vec<String> = up_next.iter().map(QueueItem::library_id).collect();
        let mut changes = Vec::new();
        for (episode, _, _) in stored.iter().filter(|(episode, _, _)| !wanted.contains(episode)) {
            changes.push(Change::QueueItemRemoved { playlist: UP_NEXT.into(), episode: episode.clone() });
        }
        let kept: Vec<&(String, f64, StoredItem)> =
            stored.iter().filter(|(episode, _, _)| wanted.contains(episode)).collect();
        let order_held = wanted
            .iter()
            .filter(|id| kept.iter().any(|(episode, _, _)| episode == *id))
            .eq(kept.iter().map(|(episode, _, _)| episode));

        let sort_of = |id: &String| kept.iter().find(|(episode, _, _)| episode == id).map(|(_, sort, _)| *sort);
        let mut before: Option<f64> = None;
        for (index, (item, id)) in up_next.iter().zip(&wanted).enumerate() {
            let sort = match (order_held, sort_of(id)) {
                (true, Some(sort)) => sort,
                // A newcomer goes between its neighbours; nobody else has to move.
                (true, None) => {
                    let after = wanted[index + 1..].iter().find_map(sort_of);
                    let sort = match (before, after) {
                        (Some(before), Some(after)) => (before + after) / 2.0,
                        (Some(before), None) => before + 1.0,
                        (None, Some(after)) => after - 1.0,
                        (None, None) => 0.0,
                    };
                    changes.push(Change::QueueItemSet {
                        playlist: UP_NEXT.into(),
                        episode: id.clone(),
                        sort,
                        item: item.into(),
                    });
                    sort
                }
                (false, known) => {
                    let sort = index as f64;
                    if known != Some(sort) {
                        changes.push(Change::QueueItemSet {
                            playlist: UP_NEXT.into(),
                            episode: id.clone(),
                            sort,
                            item: item.into(),
                        });
                    }
                    sort
                }
            };
            before = Some(sort);
        }
        for change in changes {
            let _ = self.folder.record(change, now_ms());
        }
    }

    /// The user's own playlists with what is in them.
    #[must_use]
    pub fn playlists(&self) -> Vec<Playlist> {
        let state = self.folder.state();
        state
            .playlists()
            .into_iter()
            .map(|(id, name)| {
                let items = state.playlist(&id).into_iter().map(|(_, _, item)| item.into()).collect();
                Playlist { id, name, items }
            })
            .collect()
    }

    /// Creates a playlist and says what it is called in the library's files.
    pub fn create_playlist(&mut self, name: &str) -> String {
        let playlist = torrocast_library::new_playlist_id();
        let _ = self
            .folder
            .record(Change::PlaylistSet { playlist: playlist.clone(), name: name.trim().to_owned() }, now_ms());
        playlist
    }

    /// Deletes a playlist and what is in it.
    pub fn delete_playlist(&mut self, playlist: &str) {
        for (episode, _, _) in self.folder.state().playlist(playlist) {
            let _ = self.folder.record(Change::QueueItemRemoved { playlist: playlist.to_owned(), episode }, now_ms());
        }
        let _ = self.folder.record(Change::PlaylistRemoved { playlist: playlist.to_owned() }, now_ms());
    }

    /// Appends an episode. One that is already in the playlist stays where it is.
    pub fn add_to_playlist(&mut self, playlist: &str, item: &QueueItem) {
        let entries = self.folder.state().playlist(playlist);
        let episode = item.library_id();
        if entries.iter().any(|(known, _, _)| *known == episode) {
            return;
        }
        let sort = entries.last().map_or(0.0, |(_, sort, _)| sort + 1.0);
        let change = Change::QueueItemSet { playlist: playlist.to_owned(), episode, sort, item: item.into() };
        let _ = self.folder.record(change, now_ms());
    }

    pub fn remove_from_playlist(&mut self, playlist: &str, item: &QueueItem) {
        let change = Change::QueueItemRemoved { playlist: playlist.to_owned(), episode: item.library_id() };
        let _ = self.folder.record(change, now_ms());
    }

    /// Looks for what other devices wrote. `true` if there was something.
    pub fn look(&mut self) -> bool {
        if self.looked.elapsed() < LOOK_EVERY {
            return false;
        }
        self.looked = Instant::now();
        self.folder.rescan(now_ms()).unwrap_or(false)
    }
}

/// A name for this device in the library, made up once.
#[must_use]
pub fn new_device_id() -> String {
    torrocast_library::new_device_id()
}

/// The library id of a podcast, from what its feed declares or from its address.
#[must_use]
pub fn podcast_id(declared_guid: Option<&str>, feed_url: &str) -> String {
    torrocast_library::podcast_id(declared_guid, feed_url)
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use torrocast_library::{Folder, UP_NEXT};

    use super::Keeper;
    use crate::playback::{Playback, QueueItem};

    fn scratch(name: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!("torrocast-keeper-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        path
    }

    fn item(name: &str) -> QueueItem {
        QueueItem {
            title: name.to_owned(),
            podcast: "Show".to_owned(),
            feed_url: Some("https://show.example/feed".to_owned()),
            guid: Some(name.to_owned()),
            audio_url: format!("https://cdn.example/{name}.mp3"),
            duration_ms: Some(3_600_000),
            chapters: Vec::new(),
            chapters_url: Some(format!("https://show.example/{name}.json")),
            is_mp3: true,
            artwork_url: None,
        }
    }

    fn lines(directory: &std::path::Path) -> usize {
        std::fs::read_to_string(directory.join("devices/laptop/journal-000001.jsonl"))
            .map_or(0, |journal| journal.lines().count())
    }

    #[test]
    fn the_queue_and_the_place_survive_a_restart() {
        let directory = scratch("restart");
        {
            let mut keeper = Keeper::open(&directory, "laptop", "Laptop").expect("opens");
            let mut playback = Playback::default();
            playback.enqueue(item("playing"), false);
            playback.enqueue(item("a"), false);
            playback.enqueue(item("b"), false);
            playback.on_started(None);
            playback.on_position(754_000);
            playback.stop();
            keeper.save_queue(&playback.up_next);
            keeper.save_notes(playback.take_notes());
        }
        let keeper = Keeper::open(&directory, "laptop", "Laptop").expect("reopens");
        let mut playback = Playback::default();
        keeper.restore(&mut playback);
        let titles: Vec<&str> = playback.up_next.iter().map(|item| item.title.as_str()).collect();
        assert_eq!(titles, vec!["a", "b"]);
        assert_eq!(
            playback.up_next[0].chapters_url.as_deref(),
            Some("https://show.example/a.json"),
            "enough is kept to find chapters again"
        );
        assert_eq!(keeper.position_of(&item("playing")), Some(754_000));
        let _ = std::fs::remove_dir_all(directory);
    }

    #[test]
    fn what_was_playing_is_first_in_line_after_a_restart() {
        use std::sync::Arc;

        use torrocast_net::{Fetch, FetchError};

        use crate::{Command, Core, Event, OutputKind, Settings, Transport};

        struct Offline;
        impl Fetch for Offline {
            fn get(&self, _url: &str) -> Result<Vec<u8>, FetchError> {
                Err(FetchError::Status(503))
            }
            fn get_range(&self, _url: &str, _start: u64, _end: u64) -> Result<Vec<u8>, FetchError> {
                Err(FetchError::RangeIgnored)
            }
        }

        let directory = scratch("now");
        let open = || Keeper::open(&directory, "laptop", "Laptop").expect("opens");
        {
            let (mut core, _events) = Core::new(Arc::new(Offline), Settings::default(), OutputKind::Null, Some(open()));
            core.send(Command::Transport(Transport::PlayNow(item("playing"))));
            core.send(Command::Transport(Transport::Enqueue { item: item("later"), first: false }));
        }
        let (_core, events) = Core::new(Arc::new(Offline), Settings::default(), OutputKind::Null, Some(open()));
        let restored =
            events.try_iter().find_map(|event| if let Event::Playback(state) = event { Some(state) } else { None });
        let restored = restored.expect("the core reports its state at the start");
        assert!(restored.now.is_none(), "nothing starts by itself");
        let titles: Vec<&str> = restored.up_next.iter().map(|item| item.title.as_str()).collect();
        assert_eq!(titles, vec!["playing", "later"]);
        let _ = std::fs::remove_dir_all(directory);
    }

    #[test]
    fn moving_the_library_takes_everything_along_and_merges_what_is_there() {
        let (old, new) = (scratch("move-old"), scratch("move-new"));
        let mut laptop = Keeper::open(&old, "laptop", "Laptop").expect("opens");
        laptop.set_subscribed("alpha".into(), "https://alpha.example".into(), "Alpha".into(), true);
        // The new place is a shared folder another device already uses.
        let mut desktop = Keeper::open(&new, "desktop", "Desktop").expect("opens");
        desktop.set_subscribed("beta".into(), "https://beta.example".into(), "Beta".into(), true);

        let moved = laptop.relocate(&new).expect("the new folder is writable");
        let titles: Vec<String> = moved.subscriptions().into_iter().map(|subscription| subscription.title).collect();
        assert_eq!(titles, vec!["Alpha", "Beta"]);
        assert_eq!(moved.directory(), new.as_path());
        assert!(old.join("devices/laptop/journal-000001.jsonl").exists(), "the old folder is left as a backup");
        for directory in [old, new] {
            let _ = std::fs::remove_dir_all(directory);
        }
    }

    #[test]
    fn playlists_are_made_filled_and_deleted() {
        let directory = scratch("playlists");
        let id = {
            let mut keeper = Keeper::open(&directory, "laptop", "Laptop").expect("opens");
            let id = keeper.create_playlist("  Zum Einschlafen ");
            keeper.add_to_playlist(&id, &item("a"));
            keeper.add_to_playlist(&id, &item("b"));
            keeper.add_to_playlist(&id, &item("a"));
            let other = keeper.create_playlist("Andere");
            keeper.add_to_playlist(&other, &item("c"));
            keeper.delete_playlist(&other);
            id
        };
        let mut keeper = Keeper::open(&directory, "laptop", "Laptop").expect("reopens");
        let playlists = keeper.playlists();
        assert_eq!(playlists.len(), 1, "a deleted playlist stays deleted");
        assert_eq!(playlists[0].name, "Zum Einschlafen");
        let titles: Vec<&str> = playlists[0].items.iter().map(|item| item.title.as_str()).collect();
        assert_eq!(titles, vec!["a", "b"], "in the order added, and nothing twice");
        keeper.remove_from_playlist(&id, &item("a"));
        assert_eq!(keeper.playlists()[0].items.len(), 1);

        let mut playback = Playback::default();
        keeper.restore(&mut playback);
        assert!(playback.up_next.is_empty(), "a playlist is not Up Next");
        let _ = std::fs::remove_dir_all(directory);
    }

    #[test]
    fn small_changes_cost_single_lines() {
        let directory = scratch("lines");
        let mut keeper = Keeper::open(&directory, "laptop", "Laptop").expect("opens");
        let mut queue = vec![item("a"), item("b"), item("c")];
        keeper.save_queue(&queue);
        let base = lines(&directory);

        queue.insert(0, item("front"));
        keeper.save_queue(&queue);
        assert_eq!(lines(&directory), base + 1, "one episode to the front: one line");
        queue.insert(2, item("middle"));
        keeper.save_queue(&queue);
        assert_eq!(lines(&directory), base + 2, "one in between: still one line");
        queue.remove(1);
        keeper.save_queue(&queue);
        assert_eq!(lines(&directory), base + 3);
        keeper.save_queue(&queue);
        assert_eq!(lines(&directory), base + 3, "nothing changed, nothing written");

        queue.swap(0, 3);
        keeper.save_queue(&queue);
        let folder = Folder::open(&directory, "reader", "Reader", 0).expect("opens");
        let stored: Vec<String> = folder.state().playlist(UP_NEXT).into_iter().map(|(_, _, item)| item.title).collect();
        assert_eq!(stored, vec!["c", "middle", "b", "front"], "a reordering is stored as the new order");
        let _ = std::fs::remove_dir_all(directory);
    }
}
