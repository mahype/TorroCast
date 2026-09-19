//! The running TorroCast, asked and steered from outside — by a desktop widget, a script, or a second terminal.
//!
//! One socket per user, one line in, one line out, both JSON. The program that holds the socket is the one
//! that plays; a second one finds the socket answered and knows it is not alone. The socket is polled from
//! the loop that already turns [`Core::pump`], so there is no thread and nothing to lock.
//!
//! Episodes are named by [`QueueItem::key`], never by their place in a list: between a widget's last look
//! and its click the list may have moved.

use std::collections::HashMap;
use std::io;
use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use crate::playback::{QueueItem, Sleep, Status};
use crate::{Command, Core, Event, NewEpisode, PlaybackState, Progress, Transport};

/// Who holds the socket: the terminal interface, or the program without a window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Tui,
    Daemon,
}

impl Mode {
    fn name(self) -> &'static str {
        match self {
            Self::Tui => "tui",
            Self::Daemon => "daemon",
        }
    }
}

/// What the core last said about itself — all a status answer is made of.
#[derive(Debug, Default)]
pub struct View {
    playback: PlaybackState,
    new_episodes: Vec<NewEpisode>,
    new_pending: usize,
    new_failed: usize,
    subscriptions: usize,
    progress: HashMap<String, Progress>,
}

/// What a request asks of the program around the core.
#[derive(Debug, Default, PartialEq)]
pub struct Asked {
    pub commands: Vec<Command>,
    pub quit: bool,
}

impl View {
    pub fn observe(&mut self, event: &Event) {
        match event {
            Event::Playback(state) => self.playback = (**state).clone(),
            Event::NewEpisodes { episodes, pending, failed } => {
                self.new_episodes.clone_from(episodes);
                self.new_pending = *pending;
                self.new_failed = *failed;
            }
            Event::Subscriptions(subscriptions) => self.subscriptions = subscriptions.len(),
            Event::Progress(progress) => self.progress.clone_from(progress),
            _ => {}
        }
    }

    fn heard(&self, item: &QueueItem) -> f64 {
        let fraction = |position_ms: u64, duration_ms: Option<u64>| match duration_ms.or(item.duration_ms) {
            Some(duration) if duration > 0 => (position_ms as f64 / duration as f64).clamp(0.0, 1.0),
            _ => 0.0,
        };
        match self.progress.get(&item.library_id()) {
            Some(progress) if progress.played => 1.0,
            Some(progress) => fraction(progress.position_ms, progress.duration_ms),
            None => 0.0,
        }
    }

    fn entry(&self, item: &QueueItem) -> Value {
        json!({
            "key": item.key(),
            "title": item.title,
            "podcast": item.podcast,
            "duration_ms": item.duration_ms,
            "artwork_url": item.artwork_url,
            "heard": (self.heard(item) * 1000.0).round() / 1000.0,
        })
    }

    /// The whole state as one document. `version` and `language` are the caller's to know.
    #[must_use]
    pub fn status(&self, mode: Mode, version: &str, language: &str) -> Value {
        let now = self.playback.now.as_ref().map(|now| {
            let (status, problem) = match &now.status {
                Status::Loading => ("loading", None),
                Status::Playing => ("playing", None),
                Status::Paused => ("paused", None),
                Status::Failed(reason) => ("failed", Some(reason.clone())),
            };
            let chapters = now.chapters();
            let chapter = now.chapter_index().and_then(|index| {
                let chapter = chapters.get(index)?;
                Some(json!({ "index": index, "count": chapters.len(), "title": chapter.title }))
            });
            json!({
                "key": now.item.key(),
                "title": now.item.title,
                "podcast": now.item.podcast,
                "artwork_url": now.item.artwork_url,
                "status": status,
                "problem": problem,
                "position_ms": now.position_ms,
                "duration_ms": now.duration_ms.or(now.item.duration_ms),
                "chapter": chapter,
            })
        });
        let sleep = match self.playback.sleep {
            None => Value::Null,
            Some(Sleep::Minutes(minutes)) => json!({ "minutes": minutes }),
            Some(Sleep::EndOfEpisode) => json!("end-of-episode"),
        };
        let new_episodes: Vec<Value> = self
            .new_episodes
            .iter()
            .map(|episode| {
                let mut entry = self.entry(&episode.item);
                entry["published"] = json!(episode.published.to_rfc3339());
                entry["queued"] = json!(self.playback.up_next.iter().any(|item| item.key() == episode.item.key()));
                entry
            })
            .collect();
        json!({
            "running": true,
            "mode": mode.name(),
            "version": version,
            "language": language,
            "now": now,
            "speed": (f64::from(self.playback.speed) * 100.0).round() / 100.0,
            "sleep": sleep,
            "up_next": self.playback.up_next.iter().map(|item| self.entry(item)).collect::<Vec<_>>(),
            "new_episodes": new_episodes,
            "new_pending": self.new_pending,
            "new_failed": self.new_failed,
            "subscriptions": self.subscriptions,
        })
    }

    fn queued(&self, key: &str) -> Option<usize> {
        self.playback.up_next.iter().position(|item| item.key() == key)
    }

    fn known(&self, key: &str) -> Option<QueueItem> {
        let new = self.new_episodes.iter().map(|episode| &episode.item);
        self.playback.up_next.iter().chain(new).find(|item| item.key() == key).cloned()
    }

    /// What one request line asks for. `Err` is a sentence for whoever sent it.
    pub fn ask(&self, line: &str) -> Result<Asked, String> {
        let request: Value = serde_json::from_str(line).map_err(|_| "The request is not JSON.".to_owned())?;
        let verb = request["do"].as_str().ok_or("The request names nothing to do.")?;
        let key = || request["key"].as_str().ok_or_else(|| format!("“{verb}” needs the key of an episode."));
        let unknown = || "TorroCast no longer has this episode in a list.".to_owned();
        let playing = self.playback.now.as_ref().is_some_and(|now| now.status != Status::Paused);
        let transport = match verb {
            "status" => return Ok(Asked::default()),
            "quit" => return Ok(Asked { commands: Vec::new(), quit: true }),
            "refresh" => return Ok(Asked { commands: vec![Command::RefreshSubscriptions], quit: false }),
            "toggle" => match (&self.playback.now, self.playback.up_next.is_empty()) {
                // Nothing in the player yet: a press on play means the head of Up Next.
                (None, false) => Transport::PlayQueued(0),
                _ => Transport::Toggle,
            },
            "pause" if !playing => return Ok(Asked::default()),
            "pause" => Transport::Toggle,
            "play" => match request["key"].as_str() {
                Some(key) if self.playback.now.as_ref().is_some_and(|now| now.item.key() == key) => {
                    if playing {
                        return Ok(Asked::default());
                    }
                    Transport::Toggle
                }
                Some(key) => match self.queued(key) {
                    Some(index) => Transport::PlayQueued(index),
                    None => Transport::PlayNow(self.known(key).ok_or_else(unknown)?),
                },
                None if playing => return Ok(Asked::default()),
                None if self.playback.now.is_some() => Transport::Toggle,
                None if self.playback.up_next.is_empty() => return Err("Up Next is empty.".to_owned()),
                None => Transport::PlayQueued(0),
            },
            "stop" => Transport::Stop,
            "next" => Transport::NextEpisode,
            "next-chapter" => Transport::NextChapter,
            "previous-chapter" => Transport::PreviousChapter,
            "seek-by" => {
                let seconds = request["seconds"].as_i64().ok_or("“seek-by” needs seconds.")?;
                Transport::SeekBy(seconds.saturating_mul(1000))
            }
            "seek-to" => Transport::SeekTo(request["ms"].as_u64().ok_or("“seek-to” needs ms.")?),
            "speed-by" => Transport::SpeedBy(request["step"].as_f64().ok_or("“speed-by” needs a step.")? as f32),
            "sleep" => Transport::CycleSleep,
            "enqueue" => {
                let key = key()?;
                if self.queued(key).is_some() {
                    return Ok(Asked::default());
                }
                let first = request["first"].as_bool().unwrap_or(false);
                Transport::Enqueue { item: self.known(key).ok_or_else(unknown)?, first }
            }
            "remove" => Transport::Remove(self.queued(key()?).ok_or_else(unknown)?),
            "up" | "down" => Transport::Shift { index: self.queued(key()?).ok_or_else(unknown)?, down: verb == "down" },
            other => return Err(format!("TorroCast does not know “{other}”.")),
        };
        Ok(Asked { commands: vec![Command::Transport(transport)], quit: false })
    }
}

/// The document a status question gets when nobody holds the socket.
#[must_use]
pub fn not_running(version: &str, language: &str) -> Value {
    json!({ "running": false, "version": version, "language": language })
}

/// The listening end. Dropping it takes the socket file away.
pub struct Remote {
    #[cfg(unix)]
    listener: std::os::unix::net::UnixListener,
    path: PathBuf,
    mode: Mode,
    version: String,
    language: String,
    pub view: View,
}

/// Why the socket could not be had.
#[derive(Debug)]
pub enum OpenError {
    /// Another TorroCast answers on it; this is what it said about itself.
    Taken(Value),
    Io(io::Error),
}

impl Remote {
    /// Takes the socket at `path`. A file left behind by a program that died is cleared away; one that
    /// answers is respected.
    #[cfg(unix)]
    pub fn open(path: &Path, mode: Mode, version: &str, language: &str) -> Result<Self, OpenError> {
        if let Ok(answer) = ask(path, &json!({ "do": "status" })) {
            return Err(OpenError::Taken(answer));
        }
        let _ = std::fs::remove_file(path);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(OpenError::Io)?;
        }
        let listener = std::os::unix::net::UnixListener::bind(path).map_err(OpenError::Io)?;
        listener.set_nonblocking(true).map_err(OpenError::Io)?;
        Ok(Self {
            listener,
            path: path.to_owned(),
            mode,
            version: version.to_owned(),
            language: language.to_owned(),
            view: View::default(),
        })
    }

    #[cfg(not(unix))]
    pub fn open(_path: &Path, _mode: Mode, _version: &str, _language: &str) -> Result<Self, OpenError> {
        Err(OpenError::Io(io::Error::new(io::ErrorKind::Unsupported, "no remote control on this system yet")))
    }

    /// Answers everyone who is waiting. `true` when one of them asked the program to end.
    #[cfg(unix)]
    pub fn serve(&mut self, core: &mut Core) -> bool {
        use std::io::{BufRead, BufReader, Write};
        let mut quit = false;
        while let Ok((stream, _)) = self.listener.accept() {
            // Whoever connects and then says nothing must not hold up the player.
            let patient = stream
                .set_nonblocking(false)
                .and_then(|()| stream.set_read_timeout(Some(std::time::Duration::from_millis(200))))
                .and_then(|()| stream.set_write_timeout(Some(std::time::Duration::from_millis(200))));
            if patient.is_err() {
                continue;
            }
            let mut line = String::new();
            if BufReader::new(&stream).read_line(&mut line).is_err() {
                continue;
            }
            let answer = match self.view.ask(line.trim()) {
                Ok(asked) => {
                    quit |= asked.quit;
                    for command in asked.commands {
                        core.send(command);
                    }
                    self.view.status(self.mode, &self.version, &self.language)
                }
                Err(sentence) => json!({ "error": sentence }),
            };
            let _ = writeln!(&stream, "{answer}");
        }
        quit
    }

    #[cfg(not(unix))]
    pub fn serve(&mut self, _core: &mut Core) -> bool {
        false
    }
}

impl Drop for Remote {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

/// Sends one request to whoever holds the socket and returns the answer.
#[cfg(unix)]
pub fn ask(path: &Path, request: &Value) -> io::Result<Value> {
    use std::io::{BufRead, BufReader, Write};
    let stream = std::os::unix::net::UnixStream::connect(path)?;
    stream.set_read_timeout(Some(std::time::Duration::from_secs(2)))?;
    stream.set_write_timeout(Some(std::time::Duration::from_secs(2)))?;
    writeln!(&stream, "{request}")?;
    let mut line = String::new();
    BufReader::new(&stream).read_line(&mut line)?;
    serde_json::from_str(&line).map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
}

#[cfg(not(unix))]
pub fn ask(_path: &Path, _request: &Value) -> io::Result<Value> {
    Err(io::Error::new(io::ErrorKind::Unsupported, "no remote control on this system yet"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::playback::NowPlaying;

    fn item(name: &str) -> QueueItem {
        QueueItem {
            title: name.to_owned(),
            podcast: "Beispielsendung".to_owned(),
            feed_url: Some("https://beispiel.example/feed.xml".to_owned()),
            guid: Some(name.to_owned()),
            audio_url: format!("https://beispiel.example/{name}.mp3"),
            duration_ms: Some(600_000),
            chapters: Vec::new(),
            chapters_url: None,
            is_mp3: true,
            artwork_url: None,
        }
    }

    fn view(now: Option<(QueueItem, Status)>, up_next: &[&str]) -> View {
        let mut view = View::default();
        let now = now.map(|(item, status)| NowPlaying { item, status, position_ms: 60_000, duration_ms: None });
        let up_next = up_next.iter().map(|name| item(name)).collect();
        view.observe(&Event::Playback(Box::new(PlaybackState { now, up_next, speed: 1.0, sleep: None })));
        view
    }

    fn transport(view: &View, line: &str) -> Transport {
        match view.ask(line).expect("the request is understood").commands.pop() {
            Some(Command::Transport(transport)) => transport,
            other => panic!("expected a transport command, got {other:?}"),
        }
    }

    #[test]
    fn play_with_an_empty_player_starts_the_head_of_up_next() {
        let view = view(None, &["eins", "zwei"]);
        assert_eq!(transport(&view, r#"{"do":"play"}"#), Transport::PlayQueued(0));
        assert_eq!(transport(&view, r#"{"do":"toggle"}"#), Transport::PlayQueued(0));
    }

    #[test]
    fn play_never_pauses_and_pause_never_plays() {
        let playing = view(Some((item("eins"), Status::Playing)), &[]);
        assert_eq!(playing.ask(r#"{"do":"play"}"#), Ok(Asked::default()));
        assert_eq!(transport(&playing, r#"{"do":"pause"}"#), Transport::Toggle);
        let paused = view(Some((item("eins"), Status::Paused)), &[]);
        assert_eq!(paused.ask(r#"{"do":"pause"}"#), Ok(Asked::default()));
        assert_eq!(transport(&paused, r#"{"do":"play"}"#), Transport::Toggle);
    }

    #[test]
    fn episodes_are_found_by_key_wherever_they_stand_now() {
        let view = view(None, &["eins", "zwei", "drei"]);
        let key = item("drei").key();
        assert_eq!(transport(&view, &json!({"do": "play", "key": key}).to_string()), Transport::PlayQueued(2));
        assert_eq!(transport(&view, &json!({"do": "remove", "key": key}).to_string()), Transport::Remove(2));
        assert_eq!(
            transport(&view, &json!({"do": "up", "key": key}).to_string()),
            Transport::Shift { index: 2, down: false }
        );
        let gone = json!({"do": "remove", "key": "nirgends"}).to_string();
        assert!(view.ask(&gone).is_err());
    }

    #[test]
    fn a_new_episode_can_be_played_or_queued_once() {
        let mut view = view(None, &["eins"]);
        let fresh = NewEpisode { item: item("neu"), published: chrono::Utc::now() };
        view.observe(&Event::NewEpisodes { episodes: vec![fresh], pending: 0, failed: 0 });
        let key = item("neu").key();
        assert_eq!(transport(&view, &json!({"do": "play", "key": key}).to_string()), Transport::PlayNow(item("neu")));
        assert_eq!(
            transport(&view, &json!({"do": "enqueue", "key": key, "first": true}).to_string()),
            Transport::Enqueue { item: item("neu"), first: true }
        );
        // What is queued already is not queued again.
        let queued = json!({"do": "enqueue", "key": item("eins").key()}).to_string();
        assert_eq!(view.ask(&queued), Ok(Asked::default()));
    }

    #[test]
    fn the_status_names_what_plays_and_what_waits() {
        let view = view(Some((item("eins"), Status::Playing)), &["zwei"]);
        let status = view.status(Mode::Daemon, "0.1.0", "de");
        assert_eq!(status["running"], true);
        assert_eq!(status["mode"], "daemon");
        assert_eq!(status["now"]["title"], "eins");
        assert_eq!(status["now"]["status"], "playing");
        assert_eq!(status["now"]["duration_ms"], 600_000);
        assert_eq!(status["up_next"][0]["title"], "zwei");
        assert_eq!(status["up_next"][0]["key"], item("zwei").key());
    }

    #[test]
    fn nonsense_gets_a_sentence_back() {
        let view = view(None, &[]);
        assert!(view.ask("kein json").is_err());
        assert!(view.ask(r#"{"do":"tanzen"}"#).is_err());
        assert!(view.ask(r#"{"do":"play"}"#).is_err(), "nothing to play");
        assert_eq!(view.ask(r#"{"do":"quit"}"#), Ok(Asked { commands: Vec::new(), quit: true }));
    }

    struct Offline;

    impl torrocast_net::Fetch for Offline {
        fn get(&self, _url: &str) -> Result<Vec<u8>, torrocast_net::FetchError> {
            Err(torrocast_net::FetchError::Status(503))
        }

        fn get_range(&self, _url: &str, _start: u64, _end: u64) -> Result<Vec<u8>, torrocast_net::FetchError> {
            Err(torrocast_net::FetchError::RangeIgnored)
        }
    }

    #[cfg(unix)]
    #[test]
    fn the_socket_answers_clears_a_leftover_and_is_held_by_one_program_only() {
        let directory = std::env::temp_dir().join(format!("torrocast-remote-{}", std::process::id()));
        std::fs::create_dir_all(&directory).expect("a scratch folder");
        let path = directory.join("torrocast.sock");
        // A file nobody answers on is a leftover of a program that died.
        std::fs::write(&path, b"").expect("a leftover");
        let mut remote = Remote::open(&path, Mode::Daemon, "0.1.0", "en").expect("the leftover is cleared");
        let settings = crate::Settings::for_locale("en_US");
        let (mut core, _events) = Core::new(std::sync::Arc::new(Offline), settings, crate::OutputKind::Null, None);

        let asking = {
            let path = path.clone();
            std::thread::spawn(move || {
                let status = ask(&path, &json!({"do": "status"})).expect("an answer");
                let taken = Remote::open(&path, Mode::Tui, "0.1.0", "en");
                let quit = ask(&path, &json!({"do": "quit"})).expect("an answer");
                (status, taken.is_err_and(|error| matches!(error, OpenError::Taken(_))), quit)
            })
        };
        let mut quit = false;
        let started = std::time::Instant::now();
        while !quit && started.elapsed() < std::time::Duration::from_secs(10) {
            quit = remote.serve(&mut core);
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        let (status, taken, last) = asking.join().expect("the asking thread ends");
        assert!(quit, "quit reaches the program that serves");
        assert_eq!(status["mode"], "daemon");
        assert_eq!(status["now"], Value::Null);
        assert!(taken, "a second program is told the socket is taken");
        assert_eq!(last["running"], true);
        drop(remote);
        assert!(!path.exists(), "the socket leaves with its holder");
        let _ = std::fs::remove_dir_all(directory);
    }
}
