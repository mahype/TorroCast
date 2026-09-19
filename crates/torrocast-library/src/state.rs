//! What the library says, given every change any device ever recorded.
//! Each fact is a register that the latest change wins; merging is therefore
//! the same in any order, any number of times.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::clock::Hlc;

/// The playlist the player consumes. Others may follow; the format already has room.
pub const UP_NEXT: &str = "up-next";

/// An episode with all that is needed to play it on a device that has never seen its feed.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StoredItem {
    pub title: String,
    pub podcast: String,
    pub feed_url: Option<String>,
    pub guid: Option<String>,
    pub audio_url: String,
    pub duration_ms: Option<u64>,
    /// Where chapters can be found once it plays.
    #[serde(default)]
    pub chapters_url: Option<String>,
    #[serde(default)]
    pub is_mp3: bool,
    #[serde(default)]
    pub artwork_url: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "t")]
pub enum Change {
    DeviceRegistered {
        name: String,
        app: String,
    },
    Subscribed {
        podcast: String,
        feed_url: String,
        title: String,
    },
    Unsubscribed {
        podcast: String,
    },
    PlaybackUpdated {
        episode: String,
        position_ms: u64,
        duration_ms: Option<u64>,
        played: bool,
    },
    QueueItemSet {
        playlist: String,
        episode: String,
        sort: f64,
        item: StoredItem,
    },
    QueueItemRemoved {
        playlist: String,
        episode: String,
    },
    /// Written by a newer version. Skipped, never rewritten, so nothing is lost.
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Subscription {
    pub podcast: String,
    pub feed_url: String,
    pub title: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Progress {
    pub position_ms: u64,
    pub duration_ms: Option<u64>,
    pub played: bool,
}

/// A place in a playlist: the playlist's name and the episode's id.
type Entry = (String, String);

#[derive(Debug, Clone, PartialEq)]
struct Register<T> {
    hlc: Hlc,
    value: T,
}

/// `None` is a tombstone: the fact that something was taken away, which must
/// outlive the thing itself or an old device would bring it back.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct State {
    subscriptions: HashMap<String, Register<Option<Subscription>>>,
    progress: HashMap<String, Register<Progress>>,
    queue: HashMap<Entry, Register<Option<(f64, StoredItem)>>>,
}

fn write<K: std::hash::Hash + Eq, T>(registers: &mut HashMap<K, Register<T>>, key: K, hlc: &Hlc, value: T) {
    if registers.get(&key).is_none_or(|known| known.hlc < *hlc) {
        registers.insert(key, Register { hlc: hlc.clone(), value });
    }
}

impl State {
    pub fn apply(&mut self, hlc: &Hlc, change: Change) {
        match change {
            Change::Subscribed { podcast, feed_url, title } => {
                let value = Some(Subscription { podcast: podcast.clone(), feed_url, title });
                write(&mut self.subscriptions, podcast, hlc, value);
            }
            Change::Unsubscribed { podcast } => write(&mut self.subscriptions, podcast, hlc, None),
            Change::PlaybackUpdated { episode, position_ms, duration_ms, played } => {
                write(&mut self.progress, episode, hlc, Progress { position_ms, duration_ms, played });
            }
            Change::QueueItemSet { playlist, episode, sort, item } => {
                write(&mut self.queue, (playlist, episode), hlc, Some((sort, item)))
            }
            Change::QueueItemRemoved { playlist, episode } => write(&mut self.queue, (playlist, episode), hlc, None),
            Change::DeviceRegistered { .. } | Change::Unknown => {}
        }
    }

    /// Alphabetical, as a person would look for a show.
    #[must_use]
    pub fn subscriptions(&self) -> Vec<Subscription> {
        let mut subscriptions: Vec<Subscription> =
            self.subscriptions.values().filter_map(|register| register.value.clone()).collect();
        subscriptions.sort_by_key(|subscription| subscription.title.to_lowercase());
        subscriptions
    }

    #[must_use]
    pub fn is_subscribed(&self, podcast: &str) -> bool {
        self.subscriptions.get(podcast).is_some_and(|register| register.value.is_some())
    }

    #[must_use]
    pub fn progress(&self, episode: &str) -> Option<Progress> {
        self.progress.get(episode).map(|register| register.value)
    }

    /// A playlist in its order, each entry with its episode id and sort value.
    #[must_use]
    pub fn playlist(&self, playlist: &str) -> Vec<(String, f64, StoredItem)> {
        let mut entries: Vec<(String, f64, StoredItem)> = self
            .queue
            .iter()
            .filter(|((list, _), _)| list == playlist)
            .filter_map(|((_, episode), register)| {
                register.value.clone().map(|(sort, item)| (episode.clone(), sort, item))
            })
            .collect();
        // Two devices may have chosen the same sort value; the id settles it the same way everywhere.
        entries.sort_by(|left, right| left.1.total_cmp(&right.1).then_with(|| left.0.cmp(&right.0)));
        entries
    }
}

#[cfg(test)]
mod tests {
    use super::{Change, State, StoredItem, UP_NEXT};
    use crate::clock::Hlc;

    fn hlc(ms: u64, device: &str) -> Hlc {
        Hlc { ms, counter: 0, device: device.into() }
    }

    fn item(title: &str) -> StoredItem {
        StoredItem {
            title: title.into(),
            podcast: "Show".into(),
            feed_url: None,
            guid: None,
            audio_url: format!("https://a.example/{title}.mp3"),
            duration_ms: None,
            chapters_url: None,
            is_mp3: true,
            artwork_url: None,
        }
    }

    fn history() -> Vec<(Hlc, Change)> {
        vec![
            (
                hlc(10, "laptop"),
                Change::Subscribed {
                    podcast: "p1".into(),
                    feed_url: "https://one.example".into(),
                    title: "One".into(),
                },
            ),
            (
                hlc(20, "desktop"),
                Change::Subscribed {
                    podcast: "p2".into(),
                    feed_url: "https://two.example".into(),
                    title: "Two".into(),
                },
            ),
            (hlc(30, "laptop"), Change::Unsubscribed { podcast: "p1".into() }),
            (
                hlc(25, "desktop"),
                Change::Subscribed {
                    podcast: "p1".into(),
                    feed_url: "https://one.example/new".into(),
                    title: "One".into(),
                },
            ),
            (
                hlc(40, "laptop"),
                Change::PlaybackUpdated { episode: "e1".into(), position_ms: 60_000, duration_ms: None, played: false },
            ),
            (
                hlc(50, "desktop"),
                Change::PlaybackUpdated { episode: "e1".into(), position_ms: 30_000, duration_ms: None, played: false },
            ),
            (
                hlc(60, "laptop"),
                Change::QueueItemSet { playlist: UP_NEXT.into(), episode: "e1".into(), sort: 1.0, item: item("a") },
            ),
            (
                hlc(60, "desktop"),
                Change::QueueItemSet { playlist: UP_NEXT.into(), episode: "e2".into(), sort: 1.0, item: item("b") },
            ),
            (
                hlc(70, "desktop"),
                Change::QueueItemSet { playlist: UP_NEXT.into(), episode: "e3".into(), sort: 0.0, item: item("c") },
            ),
            (hlc(80, "laptop"), Change::QueueItemRemoved { playlist: UP_NEXT.into(), episode: "e3".into() }),
            (hlc(90, "phone"), Change::Unknown),
        ]
    }

    fn folded(changes: &[(Hlc, Change)]) -> State {
        let mut state = State::default();
        for (hlc, change) in changes {
            state.apply(hlc, change.clone());
        }
        state
    }

    #[test]
    fn the_latest_change_wins_each_fact() {
        let state = folded(&history());
        let titles: Vec<String> = state.subscriptions().into_iter().map(|subscription| subscription.title).collect();
        assert_eq!(titles, vec!["Two"], "unsubscribed at 30 beats subscribed at 25, whatever arrived last");
        assert!(!state.is_subscribed("p1"));
        assert_eq!(
            state.progress("e1").map(|progress| progress.position_ms),
            Some(30_000),
            "rewinding is a wish, not an error"
        );
        let queue: Vec<String> = state.playlist(UP_NEXT).into_iter().map(|(episode, _, _)| episode).collect();
        assert_eq!(queue, vec!["e1", "e2"], "both devices' additions survive; the tie is settled by id");
    }

    #[test]
    fn any_order_any_number_of_times_gives_the_same_library() {
        let changes = history();
        let expected = folded(&changes);
        // A small generator instead of a dependency: enough to shuffle a dozen changes many ways.
        let mut seed = 0x2545_f491_4f6c_dd1du64;
        for _ in 0..200 {
            let mut shuffled = changes.clone();
            shuffled.extend(changes.iter().take(4).cloned());
            for index in (1..shuffled.len()).rev() {
                seed = seed.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1_442_695_040_888_963_407);
                shuffled.swap(index, (seed >> 33) as usize % (index + 1));
            }
            assert_eq!(folded(&shuffled), expected);
        }
    }
}
