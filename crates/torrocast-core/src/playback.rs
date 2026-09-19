//! What is playing, what comes next, and what a key press means for both.
//! Pure bookkeeping: every method answers with the [`Action`]s the audio
//! engine has to carry out, so all of this is testable without a sound card.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use torrocast_feed::Chapter;

pub const MIN_SPEED: f32 = 0.5;
pub const MAX_SPEED: f32 = 3.0;
/// Pressing "previous chapter" this far into a chapter restarts it instead.
const RESTART_AFTER_MS: u64 = 3_000;
/// Without chapters, "next chapter" still moves: this far.
const LEAP_MS: u64 = 5 * 60 * 1000;
/// So close to the end an episode counts as heard and is not resumed.
const HEARD_MARGIN_MS: u64 = 30_000;

/// An episode that can be played: everything needed without its feed at hand.
#[derive(Debug, Clone, PartialEq)]
pub struct QueueItem {
    pub title: String,
    pub podcast: String,
    pub feed_url: Option<String>,
    pub guid: Option<String>,
    pub audio_url: String,
    pub duration_ms: Option<u64>,
    pub chapters: Vec<Chapter>,
    /// Where further chapters may be found once the episode plays.
    pub chapters_url: Option<String>,
    pub is_mp3: bool,
    /// The episode's own picture, or its podcast's.
    pub artwork_url: Option<String>,
}

impl QueueItem {
    /// What makes two entries the same episode. The audio address changes with
    /// ad insertion and tracking prefixes, so the guid wins where there is one.
    #[must_use]
    pub fn key(&self) -> String {
        match (&self.feed_url, &self.guid) {
            (Some(feed), Some(guid)) => format!("{feed}\n{guid}"),
            _ => self.audio_url.clone(),
        }
    }

    /// The name the library files this episode under — the same on every device.
    #[must_use]
    pub fn library_id(&self) -> String {
        torrocast_library::episode_id(&self.key())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Status {
    Loading,
    Playing,
    Paused,
    Failed(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct NowPlaying {
    pub item: QueueItem,
    pub status: Status,
    pub position_ms: u64,
    pub duration_ms: Option<u64>,
}

impl NowPlaying {
    /// Chapters that are entries of a list; silent marks are not.
    #[must_use]
    pub fn chapters(&self) -> Vec<&Chapter> {
        self.item.chapters.iter().filter(|chapter| !chapter.hidden).collect()
    }

    /// Position of the chapter being heard within [`NowPlaying::chapters`].
    #[must_use]
    pub fn chapter_index(&self) -> Option<usize> {
        self.chapters().iter().rposition(|chapter| chapter.start_ms <= self.position_ms)
    }
}

/// The steps the sleep timer is set in, in minutes; after the last comes "end of the episode", then off.
pub const SLEEP_STEPS: [u32; 4] = [15, 30, 45, 60];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SleepTimer {
    At(Instant),
    EndOfEpisode,
}

/// The sleep timer as a listener wants to read it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sleep {
    /// Minutes left, rounded up.
    Minutes(u32),
    EndOfEpisode,
}

/// An order for the audio engine.
#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    Load {
        audio_url: String,
        start_ms: u64,
    },
    Pause,
    Resume,
    Seek(u64),
    Speed(f32),
    Stop,
    /// Look for chapters of the episode that just started.
    FindChapters {
        key: String,
        chapters_url: Option<String>,
        mp3_url: Option<String>,
    },
}

/// Where an episode was left — something worth writing down.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProgressNote {
    /// [`QueueItem::library_id`] of the episode.
    pub key: String,
    pub position_ms: u64,
    pub duration_ms: Option<u64>,
    /// Heard to the end.
    pub played: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Playback {
    pub now: Option<NowPlaying>,
    pub up_next: Vec<QueueItem>,
    pub speed: f32,
    /// Where episodes were left, by [`QueueItem::library_id`].
    positions: HashMap<String, u64>,
    /// Notes not yet collected by whoever keeps the library.
    notes: Vec<ProgressNote>,
    sleep: Option<SleepTimer>,
    /// How often the timer key was pressed since "off"; picks the next step.
    sleep_step: usize,
}

impl Default for Playback {
    fn default() -> Self {
        Self {
            now: None,
            up_next: Vec::new(),
            speed: 1.0,
            positions: HashMap::new(),
            notes: Vec::new(),
            sleep: None,
            sleep_step: 0,
        }
    }
}

impl Playback {
    /// What a library remembered from earlier sessions and other devices.
    pub fn restore(&mut self, up_next: Vec<QueueItem>, positions: HashMap<String, u64>) {
        let playing = self.now.as_ref().map(|now| now.item.key());
        self.up_next = up_next.into_iter().filter(|item| Some(item.key()) != playing).collect();
        self.positions = positions;
    }

    /// Where the library says an episode was left — on this device or another.
    pub fn hint_position(&mut self, item: &QueueItem, position_ms: Option<u64>) {
        match position_ms {
            Some(position_ms) => self.positions.insert(item.library_id(), position_ms),
            None => self.positions.remove(&item.library_id()),
        };
    }

    /// One press further: 15, 30, 45, 60 minutes, the end of the episode, off.
    pub fn cycle_sleep(&mut self, now: Instant) {
        self.sleep_step = (self.sleep_step + 1) % (SLEEP_STEPS.len() + 2);
        self.sleep = match self.sleep_step {
            0 => None,
            step if step <= SLEEP_STEPS.len() => {
                Some(SleepTimer::At(now + Duration::from_secs(u64::from(SLEEP_STEPS[step - 1]) * 60)))
            }
            _ => Some(SleepTimer::EndOfEpisode),
        };
    }

    #[must_use]
    pub fn sleep(&self, now: Instant) -> Option<Sleep> {
        match self.sleep? {
            SleepTimer::At(deadline) => {
                Some(Sleep::Minutes(deadline.saturating_duration_since(now).as_secs().div_ceil(60) as u32))
            }
            SleepTimer::EndOfEpisode => Some(Sleep::EndOfEpisode),
        }
    }

    fn sleep_off(&mut self) {
        (self.sleep, self.sleep_step) = (None, 0);
    }

    /// Called regularly: when the time is up, playback pauses — and stays where it is for tomorrow.
    pub fn sleep_due(&mut self, now: Instant) -> Vec<Action> {
        match self.sleep {
            Some(SleepTimer::At(deadline)) if now >= deadline => {
                self.sleep_off();
                if self.now.as_ref().is_some_and(|playing| playing.status == Status::Playing) {
                    self.toggle()
                } else {
                    Vec::new()
                }
            }
            _ => Vec::new(),
        }
    }

    /// The notes taken since the last call.
    pub fn take_notes(&mut self) -> Vec<ProgressNote> {
        std::mem::take(&mut self.notes)
    }

    /// A note about the episode playing right now, for the periodic save.
    #[must_use]
    pub fn note_now(&self) -> Option<ProgressNote> {
        let now = self.now.as_ref().filter(|now| now.status != Status::Loading && now.position_ms > 0)?;
        Some(ProgressNote {
            key: now.item.library_id(),
            position_ms: now.position_ms,
            duration_ms: now.duration_ms,
            played: false,
        })
    }

    fn remember(&mut self) {
        if let Some(now) = &self.now {
            let heard = now.duration_ms.is_some_and(|duration| now.position_ms + HEARD_MARGIN_MS >= duration);
            let key = now.item.library_id();
            if heard {
                self.positions.remove(&key);
            } else if now.position_ms > 0 {
                self.positions.insert(key.clone(), now.position_ms);
            } else {
                return;
            }
            self.notes.push(ProgressNote {
                key,
                position_ms: now.position_ms,
                duration_ms: now.duration_ms,
                played: heard,
            });
        }
    }

    fn start(&mut self, item: QueueItem) -> Vec<Action> {
        self.remember();
        self.up_next.retain(|queued| queued.key() != item.key());
        let start_ms = self.positions.get(&item.library_id()).copied().unwrap_or(0);
        let mut actions = vec![Action::Load { audio_url: item.audio_url.clone(), start_ms }];
        let mp3_url =
            (item.is_mp3 && item.chapters.is_empty() && item.chapters_url.is_none()).then(|| item.audio_url.clone());
        if item.chapters_url.is_some() || mp3_url.is_some() {
            actions.push(Action::FindChapters { key: item.key(), chapters_url: item.chapters_url.clone(), mp3_url });
        }
        self.now =
            Some(NowPlaying { duration_ms: item.duration_ms, item, status: Status::Loading, position_ms: start_ms });
        actions
    }

    /// Plays `item` now. What was playing is not lost: it goes to the top of Up Next.
    pub fn play_now(&mut self, item: QueueItem) -> Vec<Action> {
        if self.now.as_ref().is_some_and(|now| now.item.key() == item.key()) {
            return self.toggle();
        }
        if let Some(interrupted) = self.now.as_ref().map(|now| now.item.clone()) {
            self.remember();
            self.up_next.insert(0, interrupted);
        }
        self.start(item)
    }

    /// Queues `item` first or last. Asking for an episode that is already
    /// queued moves it; asking twice for the same place takes it out again.
    /// Returns where it went, or `None` if it was removed.
    pub fn enqueue(&mut self, item: QueueItem, first: bool) -> (Option<usize>, Vec<Action>) {
        if self.now.is_none() && self.up_next.is_empty() {
            return (Some(0), self.start(item));
        }
        if self.now.as_ref().is_some_and(|now| now.item.key() == item.key()) {
            return (None, Vec::new());
        }
        let was_at = self.up_next.iter().position(|queued| queued.key() == item.key());
        self.up_next.retain(|queued| queued.key() != item.key());
        let target = if first { 0 } else { self.up_next.len() };
        if was_at == Some(target) {
            return (None, Vec::new());
        }
        self.up_next.insert(target, item);
        (Some(target), Vec::new())
    }

    /// Queues a whole list, in its order, first or last. What plays is left out; what was queued already moves.
    pub fn enqueue_many(&mut self, items: Vec<QueueItem>, first: bool) -> Vec<Action> {
        let playing = self.now.as_ref().map(|now| now.item.key());
        let items: Vec<QueueItem> = items.into_iter().filter(|item| Some(item.key()) != playing).collect();
        self.up_next.retain(|queued| items.iter().all(|item| item.key() != queued.key()));
        let at = if first { 0 } else { self.up_next.len() };
        self.up_next.splice(at..at, items);
        // Into silence: the first of them starts.
        if self.now.is_none() { self.next_episode() } else { Vec::new() }
    }

    pub fn remove(&mut self, index: usize) {
        if index < self.up_next.len() {
            self.up_next.remove(index);
        }
    }

    /// Moves a queued episode up or down; returns where it ended.
    pub fn shift(&mut self, index: usize, down: bool) -> usize {
        let target = if down { index + 1 } else { index.saturating_sub(1) };
        if index < self.up_next.len() && target < self.up_next.len() {
            self.up_next.swap(index, target);
            return target;
        }
        index
    }

    pub fn clear(&mut self) {
        self.up_next.clear();
    }

    /// Plays the queued episode at `index` now.
    pub fn play_queued(&mut self, index: usize) -> Vec<Action> {
        if index >= self.up_next.len() {
            return Vec::new();
        }
        let item = self.up_next.remove(index);
        self.play_now(item)
    }

    pub fn toggle(&mut self) -> Vec<Action> {
        let Some(now) = &mut self.now else { return Vec::new() };
        match now.status {
            Status::Playing => {
                now.status = Status::Paused;
                // A pause is a likely moment to walk away, or to pick up another device.
                self.remember();
                vec![Action::Pause]
            }
            Status::Paused => {
                now.status = Status::Playing;
                vec![Action::Resume]
            }
            // Pressing play on an episode that failed tries it again.
            Status::Failed(_) => {
                let item = now.item.clone();
                self.now = None;
                self.start(item)
            }
            Status::Loading => Vec::new(),
        }
    }

    /// Remembers the place and lets go of the episode.
    pub fn stop(&mut self) -> Vec<Action> {
        self.remember();
        if self.now.take().is_some() { vec![Action::Stop] } else { Vec::new() }
    }

    /// The next queued episode, or silence when there is none.
    pub fn next_episode(&mut self) -> Vec<Action> {
        if self.up_next.is_empty() {
            return Vec::new();
        }
        let item = self.up_next.remove(0);
        self.start(item)
    }

    pub fn seek_to(&mut self, position_ms: u64) -> Vec<Action> {
        let Some(now) = &mut self.now else { return Vec::new() };
        let position_ms = now.duration_ms.map_or(position_ms, |duration| position_ms.min(duration));
        now.position_ms = position_ms;
        vec![Action::Seek(position_ms)]
    }

    pub fn seek_by(&mut self, delta_ms: i64) -> Vec<Action> {
        let Some(position) = self.now.as_ref().map(|now| now.position_ms) else { return Vec::new() };
        self.seek_to(position.saturating_add_signed(delta_ms))
    }

    pub fn next_chapter(&mut self) -> Vec<Action> {
        let Some(now) = &self.now else { return Vec::new() };
        let next = now.chapters().iter().map(|chapter| chapter.start_ms).find(|start| *start > now.position_ms + 1_000);
        match (next, now.chapters().is_empty()) {
            (Some(start), _) => self.seek_to(start),
            (None, true) => self.seek_by(LEAP_MS as i64),
            // After the last chapter comes the next episode.
            (None, false) => self.next_episode(),
        }
    }

    pub fn previous_chapter(&mut self) -> Vec<Action> {
        let Some(now) = &self.now else { return Vec::new() };
        let chapters = now.chapters();
        let target = match now.chapter_index() {
            Some(index) if now.position_ms - chapters[index].start_ms > RESTART_AFTER_MS || index == 0 => {
                chapters[index].start_ms
            }
            Some(index) => chapters[index - 1].start_ms,
            None => 0,
        };
        self.seek_to(target)
    }

    pub fn change_speed(&mut self, delta: f32) -> Vec<Action> {
        // Tenths, exactly: 1.0 + 0.1 + 0.1 must read 1.2, not 1.2000002.
        self.speed = ((self.speed + delta) * 10.0).round().clamp(MIN_SPEED * 10.0, MAX_SPEED * 10.0) / 10.0;
        vec![Action::Speed(self.speed)]
    }

    pub fn set_chapters(&mut self, key: &str, chapters: Vec<Chapter>) {
        if let Some(now) = self.now.as_mut().filter(|now| now.item.key() == key) {
            now.item.chapters = torrocast_feed::chapters::merge(std::mem::take(&mut now.item.chapters), chapters);
        }
    }

    // ── what the engine reports ─────────────────────────────────────────────

    pub fn on_started(&mut self, duration_ms: Option<u64>) {
        if let Some(now) = &mut self.now {
            now.status = Status::Playing;
            // The file knows its length better than the feed does.
            now.duration_ms = duration_ms.or(now.duration_ms);
        }
    }

    pub fn on_position(&mut self, position_ms: u64) {
        if let Some(now) = self.now.as_mut().filter(|now| now.status != Status::Loading) {
            now.position_ms = position_ms;
        }
    }

    pub fn on_failed(&mut self, reason: String) {
        if let Some(now) = &mut self.now {
            now.status = Status::Failed(reason);
        }
    }

    /// The episode ran out: on to the next one, without a pause.
    pub fn on_ended(&mut self) -> Vec<Action> {
        if let Some(now) = self.now.take() {
            let key = now.item.library_id();
            self.positions.remove(&key);
            let position_ms = now.duration_ms.unwrap_or(now.position_ms);
            self.notes.push(ProgressNote { key, position_ms, duration_ms: now.duration_ms, played: true });
        }
        // "Until the end of the episode" means exactly that: nothing follows.
        if self.sleep == Some(SleepTimer::EndOfEpisode) {
            self.sleep_off();
            return Vec::new();
        }
        self.next_episode()
    }
}

#[cfg(test)]
mod tests {
    use torrocast_feed::{Chapter, ChapterSource};

    use super::{Action, Playback, QueueItem, Status};

    fn item(name: &str) -> QueueItem {
        QueueItem {
            title: name.to_owned(),
            podcast: "Show".to_owned(),
            feed_url: Some("https://show.example/feed".to_owned()),
            guid: Some(name.to_owned()),
            audio_url: format!("https://cdn.example/{name}.mp3"),
            duration_ms: Some(3_600_000),
            chapters: Vec::new(),
            chapters_url: None,
            is_mp3: false,
            artwork_url: None,
        }
    }

    fn chapter(start_ms: u64) -> Chapter {
        Chapter {
            start_ms,
            title: Some(format!("at {start_ms}")),
            url: None,
            image: None,
            hidden: false,
            source: ChapterSource::Feed,
        }
    }

    fn titles(playback: &Playback) -> Vec<&str> {
        playback.up_next.iter().map(|item| item.title.as_str()).collect()
    }

    #[test]
    fn queueing_into_silence_starts_playing() {
        let mut playback = Playback::default();
        let (place, actions) = playback.enqueue(item("a"), false);
        assert_eq!(place, Some(0));
        assert_eq!(actions, vec![Action::Load { audio_url: "https://cdn.example/a.mp3".into(), start_ms: 0 }]);
        assert!(playback.up_next.is_empty());
    }

    #[test]
    fn first_last_move_and_take_out() {
        let mut playback = Playback::default();
        playback.enqueue(item("playing"), false);
        playback.enqueue(item("a"), false);
        playback.enqueue(item("b"), false);
        assert_eq!(playback.enqueue(item("c"), true).0, Some(0));
        assert_eq!(titles(&playback), vec!["c", "a", "b"]);
        assert_eq!(playback.enqueue(item("c"), false).0, Some(2), "asking for the other end moves it");
        assert_eq!(titles(&playback), vec!["a", "b", "c"]);
        assert_eq!(playback.enqueue(item("c"), false).0, None, "asking for the same end takes it out");
        assert_eq!(titles(&playback), vec!["a", "b"]);
        assert_eq!(playback.enqueue(item("playing"), true).0, None, "what plays is not queued");
    }

    #[test]
    fn a_whole_list_is_queued_in_its_order() {
        let mut playback = Playback::default();
        playback.enqueue(item("playing"), false);
        playback.enqueue(item("b"), false);
        playback.enqueue_many(vec![item("a"), item("playing"), item("b"), item("c")], true);
        assert_eq!(titles(&playback), vec!["a", "b", "c"], "what plays is left out, what was queued moves");

        let mut silent = Playback::default();
        let actions = silent.enqueue_many(vec![item("x"), item("y")], false);
        assert!(matches!(&actions[0], Action::Load { audio_url, .. } if audio_url.ends_with("x.mp3")));
        assert_eq!(titles(&silent), vec!["y"]);
    }

    #[test]
    fn play_now_keeps_what_was_interrupted() {
        let mut playback = Playback::default();
        playback.enqueue(item("a"), false);
        playback.on_started(None);
        playback.on_position(120_000);
        playback.enqueue(item("b"), false);
        playback.play_now(item("c"));
        assert_eq!(titles(&playback), vec!["a", "b"]);

        // Back to "a": it resumes where it was left.
        let actions = playback.play_queued(0);
        assert_eq!(actions[0], Action::Load { audio_url: "https://cdn.example/a.mp3".into(), start_ms: 120_000 });
        assert_eq!(titles(&playback), vec!["c", "b"]);
    }

    #[test]
    fn the_end_of_one_is_the_start_of_the_next() {
        let mut playback = Playback::default();
        playback.enqueue(item("a"), false);
        playback.enqueue(item("b"), false);
        let actions = playback.on_ended();
        assert!(matches!(&actions[0], Action::Load { audio_url, .. } if audio_url.ends_with("b.mp3")));
        assert!(playback.on_ended().is_empty());
        assert!(playback.now.is_none(), "an empty queue ends in silence");
    }

    #[test]
    fn an_episode_heard_to_the_end_starts_over() {
        let mut playback = Playback::default();
        playback.enqueue(item("a"), false);
        playback.on_started(Some(3_600_000));
        playback.on_position(3_590_000);
        playback.stop();
        let actions = playback.play_now(item("a"));
        assert!(matches!(actions[0], Action::Load { start_ms: 0, .. }));
    }

    #[test]
    fn chapters_forwards_and_backwards() {
        let mut playback = Playback::default();
        let mut episode = item("a");
        episode.chapters = vec![chapter(0), chapter(60_000), chapter(300_000)];
        playback.enqueue(episode, false);
        playback.on_started(None);
        playback.on_position(70_000);

        assert_eq!(playback.next_chapter(), vec![Action::Seek(300_000)]);
        playback.on_position(301_000);
        assert_eq!(playback.previous_chapter(), vec![Action::Seek(60_000)], "early in a chapter: the one before");
        playback.on_position(80_000);
        assert_eq!(playback.previous_chapter(), vec![Action::Seek(60_000)], "well into a chapter: its start");
        assert_eq!(playback.now.as_ref().and_then(super::NowPlaying::chapter_index), Some(1));
    }

    #[test]
    fn without_chapters_the_keys_still_move() {
        let mut playback = Playback::default();
        playback.enqueue(item("a"), false);
        playback.on_started(None);
        playback.on_position(60_000);
        assert_eq!(playback.next_chapter(), vec![Action::Seek(360_000)]);
        assert_eq!(playback.previous_chapter(), vec![Action::Seek(0)]);
        assert_eq!(playback.seek_by(-999_999_999), vec![Action::Seek(0)]);
        assert_eq!(playback.seek_by(i64::MAX), vec![Action::Seek(3_600_000)], "never past the end");
    }

    #[test]
    fn the_sleep_timer_pauses_and_the_end_of_episode_timer_lets_nothing_follow() {
        use std::time::{Duration, Instant};

        use super::Sleep;

        let start = Instant::now();
        let mut playback = Playback::default();
        playback.enqueue(item("a"), false);
        playback.enqueue(item("b"), false);
        playback.on_started(None);
        playback.on_position(60_000);

        playback.cycle_sleep(start);
        assert_eq!(playback.sleep(start + Duration::from_secs(61)), Some(Sleep::Minutes(14)));
        assert!(playback.sleep_due(start + Duration::from_secs(14 * 60)).is_empty());
        assert_eq!(playback.sleep_due(start + Duration::from_secs(15 * 60)), vec![Action::Pause]);
        assert_eq!(playback.sleep(start), None, "a timer that went off is off");
        assert_eq!(playback.take_notes().len(), 1, "the place is written down when the timer pauses");

        for _ in 0..5 {
            playback.cycle_sleep(start);
        }
        assert_eq!(playback.sleep(start), Some(Sleep::EndOfEpisode));
        assert!(playback.on_ended().is_empty());
        assert!(playback.now.is_none());
        assert_eq!(titles(&playback), vec!["b"], "the next episode waits for tomorrow");
        playback.cycle_sleep(start);
        playback.cycle_sleep(start);
        assert_eq!(playback.sleep(start), Some(Sleep::Minutes(30)), "after off the steps begin again");
    }

    #[test]
    fn speed_moves_in_clean_tenths() {
        let mut playback = Playback::default();
        for _ in 0..3 {
            playback.change_speed(0.1);
        }
        assert_eq!(playback.speed, 1.3);
        for _ in 0..40 {
            playback.change_speed(0.1);
        }
        assert_eq!(playback.speed, 3.0);
        for _ in 0..40 {
            playback.change_speed(-0.1);
        }
        assert_eq!(playback.speed, 0.5);
    }

    #[test]
    fn a_failed_episode_can_be_tried_again() {
        let mut playback = Playback::default();
        playback.enqueue(item("a"), false);
        playback.on_failed("no network".into());
        assert!(matches!(playback.now.as_ref().map(|now| &now.status), Some(Status::Failed(_))));
        assert!(matches!(playback.toggle()[0], Action::Load { .. }));
    }

    #[test]
    fn only_an_mp3_without_announced_chapters_is_searched() {
        let mut playback = Playback::default();
        let mut episode = item("a");
        episode.is_mp3 = true;
        let (_, actions) = playback.enqueue(episode, false);
        assert!(matches!(&actions[1], Action::FindChapters { mp3_url: Some(_), chapters_url: None, .. }));

        let mut announced = item("b");
        announced.is_mp3 = true;
        announced.chapters_url = Some("https://show.example/b.json".into());
        let actions = playback.play_now(announced);
        assert!(matches!(&actions[1], Action::FindChapters { mp3_url: None, chapters_url: Some(_), .. }));
    }
}
