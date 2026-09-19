//! Episodes kept on this machine, to be heard without a network.
//!
//! One folder, two files per episode: the audio, and beside it a small JSON
//! file saying which episode it is. The folder is therefore its own index — it
//! can be looked through, cleaned up by hand, or copied to another machine.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::time::{Duration, Instant};

use torrocast_library::StoredItem;
use torrocast_net::Fetch;

use crate::playback::QueueItem;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DownloadState {
    /// `total` is what the server announced, if it did.
    Loading {
        received: u64,
        total: Option<u64>,
    },
    Done {
        bytes: u64,
    },
    Failed,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Download {
    pub item: QueueItem,
    pub state: DownloadState,
}

enum Report {
    Progress(String, u64, Option<u64>),
    Finished(String, Result<u64, ()>),
}

pub struct Downloads {
    directory: PathBuf,
    /// In the order they were asked for; the newest last.
    entries: Vec<(String, Download)>,
    files: HashMap<String, PathBuf>,
    reports: (Sender<Report>, Receiver<Report>),
}

/// A file name from an episode's library id: nothing in it but what every file system takes.
fn file_stem(id: &str) -> String {
    id.chars()
        .map(|character| if character.is_ascii_alphanumeric() || character == '-' { character } else { '_' })
        .collect()
}

fn extension(audio_url: &str) -> &str {
    let path = audio_url.split(['?', '#']).next().unwrap_or(audio_url);
    match path.rsplit_once('.') {
        Some((_, extension))
            if extension.len() <= 4 && extension.chars().all(|character| character.is_ascii_alphanumeric()) =>
        {
            extension
        }
        _ => "audio",
    }
}

impl Downloads {
    /// Opens the folder and takes stock of what is in it.
    #[must_use]
    pub fn open(directory: &Path) -> Self {
        let mut found: Vec<(std::time::SystemTime, String, Download, PathBuf)> = Vec::new();
        for entry in std::fs::read_dir(directory).into_iter().flatten().filter_map(Result::ok) {
            let path = entry.path();
            // What a download left behind when the program ended under it: nothing can continue it.
            if path.extension().is_some_and(|extension| extension == "part") {
                let _ = std::fs::remove_file(&path);
                continue;
            }
            if path.extension().is_none_or(|extension| extension != "json") {
                continue;
            }
            let Some(item) =
                std::fs::read(&path).ok().and_then(|bytes| serde_json::from_slice::<StoredItem>(&bytes).ok())
            else {
                continue;
            };
            let item: QueueItem = item.into();
            let audio = path.with_extension(extension(&item.audio_url));
            // A description without its audio is a download that never finished.
            let Ok(metadata) = std::fs::metadata(&audio) else { continue };
            let added = metadata.modified().unwrap_or(std::time::UNIX_EPOCH);
            found.push((
                added,
                item.library_id(),
                Download { item, state: DownloadState::Done { bytes: metadata.len() } },
                audio,
            ));
        }
        found.sort_by_key(|(added, ..)| *added);
        let files = found.iter().map(|(_, id, _, audio)| (id.clone(), audio.clone())).collect();
        let entries = found.into_iter().map(|(_, id, download, _)| (id, download)).collect();
        Self { directory: directory.to_owned(), entries, files, reports: channel() }
    }

    #[must_use]
    pub fn list(&self) -> Vec<Download> {
        self.entries.iter().map(|(_, download)| download.clone()).collect()
    }

    /// The audio on disk, if the episode has been downloaded completely.
    #[must_use]
    pub fn file_of(&self, id: &str) -> Option<&Path> {
        let done = self
            .entries
            .iter()
            .any(|(known, download)| known == id && matches!(download.state, DownloadState::Done { .. }));
        self.files.get(id).filter(|_| done).map(PathBuf::as_path)
    }

    /// Starts downloading, unless the episode is already here or on its way.
    pub fn start(&mut self, item: QueueItem, fetch: Arc<dyn Fetch>) {
        let id = item.library_id();
        if self.entries.iter().any(|(known, download)| *known == id && download.state != DownloadState::Failed) {
            return;
        }
        self.entries.retain(|(known, _)| *known != id);
        let audio = self.directory.join(format!("{}.{}", file_stem(&id), extension(&item.audio_url)));
        let description = audio.with_extension("json");
        let partial = audio.with_extension("part");
        self.files.insert(id.clone(), audio.clone());
        self.entries.push((
            id.clone(),
            Download { item: item.clone(), state: DownloadState::Loading { received: 0, total: None } },
        ));

        let (reports, directory) = (self.reports.0.clone(), self.directory.clone());
        std::thread::spawn(move || {
            let mut told = Instant::now();
            let outcome = std::fs::create_dir_all(&directory).map_err(|_| ()).and_then(|()| {
                let mut progress = |received: u64, total: Option<u64>| {
                    if told.elapsed() >= Duration::from_millis(250) {
                        told = Instant::now();
                        let _ = reports.send(Report::Progress(id.clone(), received, total));
                    }
                };
                fetch.download(&item.audio_url, &partial, &mut progress).map_err(|_| ())
            });
            // Only a complete file gets its real name and its description.
            let outcome = outcome.and_then(|bytes| {
                let stored = serde_json::to_vec_pretty(&StoredItem::from(&item)).map_err(|_| ())?;
                std::fs::rename(&partial, &audio).map_err(|_| ())?;
                std::fs::write(&description, stored).map_err(|_| ())?;
                Ok(bytes)
            });
            if outcome.is_err() {
                let _ = std::fs::remove_file(&partial);
            }
            let _ = reports.send(Report::Finished(id, outcome));
        });
    }

    /// Deletes a finished download. One still running is left alone.
    pub fn remove(&mut self, id: &str) {
        let running = self
            .entries
            .iter()
            .any(|(known, download)| known == id && matches!(download.state, DownloadState::Loading { .. }));
        if running {
            return;
        }
        if let Some(audio) = self.files.remove(id) {
            let _ = std::fs::remove_file(audio.with_extension("json"));
            let _ = std::fs::remove_file(audio);
        }
        self.entries.retain(|(known, _)| known != id);
    }

    /// Takes in what the download threads have reported. `true` if anything changed.
    pub fn pump(&mut self) -> bool {
        let mut changed = false;
        for report in self.reports.1.try_iter().collect::<Vec<_>>() {
            let (id, state) = match report {
                Report::Progress(id, received, total) => (id, DownloadState::Loading { received, total }),
                Report::Finished(id, Ok(bytes)) => (id, DownloadState::Done { bytes }),
                Report::Finished(id, Err(())) => (id, DownloadState::Failed),
            };
            if let Some((_, download)) = self.entries.iter_mut().find(|(known, _)| *known == id) {
                download.state = state;
                changed = true;
            }
        }
        changed
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::sync::Arc;
    use std::time::Duration;

    use torrocast_net::{Fetch, FetchError};

    use super::{DownloadState, Downloads};
    use crate::playback::QueueItem;

    struct Canned;
    impl Fetch for Canned {
        fn get(&self, url: &str) -> Result<Vec<u8>, FetchError> {
            if url.contains("missing") { Err(FetchError::Status(404)) } else { Ok(vec![7u8; 4096]) }
        }
        fn get_range(&self, _url: &str, _start: u64, _end: u64) -> Result<Vec<u8>, FetchError> {
            Err(FetchError::RangeIgnored)
        }
    }

    fn scratch(name: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!("torrocast-downloads-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        path
    }

    fn item(name: &str) -> QueueItem {
        QueueItem {
            title: name.to_owned(),
            podcast: "Show".to_owned(),
            feed_url: Some("https://show.example/feed".to_owned()),
            guid: Some(name.to_owned()),
            audio_url: format!("https://cdn.example/{name}.mp3?token=1"),
            duration_ms: Some(60_000),
            chapters: Vec::new(),
            chapters_url: None,
            is_mp3: true,
            artwork_url: None,
        }
    }

    fn settle(downloads: &mut Downloads) {
        for _ in 0..200 {
            downloads.pump();
            if downloads.list().iter().all(|download| !matches!(download.state, DownloadState::Loading { .. })) {
                return;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        panic!("downloads did not finish");
    }

    #[test]
    fn downloaded_found_again_and_deleted() {
        let directory = scratch("cycle");
        let mut downloads = Downloads::open(&directory);
        downloads.start(item("one"), Arc::new(Canned));
        downloads.start(item("one"), Arc::new(Canned));
        assert_eq!(downloads.list().len(), 1, "asking twice downloads once");
        assert!(downloads.file_of(&item("one").library_id()).is_none(), "not playable before it is complete");
        settle(&mut downloads);
        assert_eq!(downloads.list()[0].state, DownloadState::Done { bytes: 4096 });
        let file = downloads.file_of(&item("one").library_id()).expect("complete").to_owned();
        assert!(file.to_string_lossy().ends_with(".mp3"), "the extension survives the query string");

        // A new start finds it by the description beside it — and clears away what an interrupted download left.
        std::fs::write(directory.join("ep1_interrupted.part"), b"half an episode").expect("writable");
        let mut reopened = Downloads::open(&directory);
        assert_eq!(reopened.list()[0].item.title, "one");
        assert!(!directory.join("ep1_interrupted.part").exists());
        assert_eq!(reopened.file_of(&item("one").library_id()), Some(file.as_path()));
        reopened.remove(&item("one").library_id());
        assert!(!file.exists() && reopened.list().is_empty());
        let _ = std::fs::remove_dir_all(directory);
    }

    #[test]
    fn a_failed_download_leaves_nothing_behind_and_can_be_tried_again() {
        let directory = scratch("failed");
        let mut downloads = Downloads::open(&directory);
        downloads.start(item("missing"), Arc::new(Canned));
        settle(&mut downloads);
        assert_eq!(downloads.list()[0].state, DownloadState::Failed);
        let leftovers = std::fs::read_dir(&directory).map(Iterator::count).unwrap_or(0);
        assert_eq!(leftovers, 0);
        downloads.start(item("missing"), Arc::new(Canned));
        assert!(
            matches!(downloads.list()[0].state, DownloadState::Loading { .. }),
            "a failed one may be asked for again"
        );
        let _ = std::fs::remove_dir_all(directory);
    }
}
