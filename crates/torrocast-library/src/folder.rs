//! The folder on disk.
//!
//! ```text
//! torrocast-library/
//! ├── format.json
//! └── devices/
//!     ├── 7f3a9c2e11aa/journal-000001.jsonl    written by this device only
//!     └── c41d07b8e2f0/journal-000001.jsonl    written by that one
//! ```

use std::collections::HashMap;
use std::fmt;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::clock::{Clock, Hlc};
use crate::state::{Change, State};

const FORMAT: &str = "torrocast-library";
const FORMAT_VERSION: u32 = 1;
/// A journal is closed and a new one begun at this size, so a sync service
/// never has to move more than this for one more line.
const SEGMENT_BYTES: u64 = 256 * 1024;
/// When this device's journals have grown to this, they are folded into a snapshot at the next start.
const COMPACT_FROM_BYTES: u64 = 128 * 1024;

#[derive(Serialize, Deserialize)]
struct Format {
    format: String,
    format_version: u32,
    min_reader_version: u32,
    min_writer_version: u32,
}

#[derive(Serialize, Deserialize)]
struct Line {
    v: u32,
    /// `<device>:<sequence>` — unique, so a line copied twice by a sync service is recognisable.
    id: String,
    hlc: String,
    #[serde(flatten)]
    change: Change,
}

#[derive(Debug)]
pub enum OpenError {
    Io(io::Error),
    /// A newer TorroCast on another device has moved the folder to a format this version cannot read.
    TooNew,
    /// The folder holds something else.
    NotALibrary,
}

impl fmt::Display for OpenError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "{error}"),
            Self::TooNew => write!(formatter, "the library was written by a newer version"),
            Self::NotALibrary => write!(formatter, "the folder is not a TorroCast library"),
        }
    }
}

impl std::error::Error for OpenError {}

impl From<io::Error> for OpenError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

pub struct Folder {
    root: PathBuf,
    device: String,
    clock: Clock,
    sequence: u64,
    segment: u32,
    journal: File,
    state: State,
    /// How far each journal has been read. Journals only grow.
    read_to: HashMap<PathBuf, u64>,
    read_only: bool,
    /// This device's files hold something a newer version wrote. Folding them would lose it.
    own_unknown: bool,
    device_name: String,
}

fn journal_name(segment: u32) -> String {
    format!("journal-{segment:06}.jsonl")
}

/// Sync services leave their own files about; none of them is a journal.
fn is_journal(path: &Path) -> bool {
    let name = path.file_name().and_then(|name| name.to_str()).unwrap_or_default();
    name.ends_with(".jsonl") && !name.starts_with(['.', '~'])
}

impl Folder {
    /// Opens the library at `root`, creating it if the folder is empty, and reads what every device has written.
    pub fn open(root: &Path, device: &str, device_name: &str, now_ms: u64) -> Result<Self, OpenError> {
        fs::create_dir_all(root.join("devices"))?;
        let format_file = root.join("format.json");
        let mut read_only = false;
        match fs::read(&format_file) {
            Ok(bytes) => {
                let format: Format = serde_json::from_slice(&bytes).map_err(|_| OpenError::NotALibrary)?;
                if format.format != FORMAT {
                    return Err(OpenError::NotALibrary);
                }
                if format.min_reader_version > FORMAT_VERSION {
                    return Err(OpenError::TooNew);
                }
                read_only = format.min_writer_version > FORMAT_VERSION;
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                let format = Format {
                    format: FORMAT.into(),
                    format_version: FORMAT_VERSION,
                    min_reader_version: 1,
                    min_writer_version: 1,
                };
                write_whole(&format_file, &serde_json::to_vec_pretty(&format).map_err(io::Error::other)?)?;
            }
            Err(error) => return Err(error.into()),
        }

        let own = root.join("devices").join(device);
        let first_visit = !own.exists();
        fs::create_dir_all(&own)?;
        let segment = fs::read_dir(&own)?
            .filter_map(Result::ok)
            .filter_map(|entry| {
                entry.file_name().to_str()?.strip_prefix("journal-")?.strip_suffix(".jsonl")?.parse::<u32>().ok()
            })
            .max()
            .unwrap_or(1);
        let journal = OpenOptions::new().create(true).append(true).open(own.join(journal_name(segment)))?;

        let mut folder = Self {
            root: root.to_owned(),
            device: device.to_owned(),
            clock: Clock::new(device),
            sequence: 0,
            segment,
            journal,
            state: State::default(),
            read_to: HashMap::new(),
            read_only,
            own_unknown: false,
            device_name: device_name.to_owned(),
        };
        folder.rescan(now_ms)?;
        if first_visit {
            let app = concat!("torrocast ", env!("CARGO_PKG_VERSION")).to_owned();
            folder.record(Change::DeviceRegistered { name: device_name.to_owned(), app }, now_ms)?;
        }
        if folder.own_bytes() >= COMPACT_FROM_BYTES {
            folder.compact(now_ms)?;
        }
        Ok(folder)
    }

    #[must_use]
    pub fn state(&self) -> &State {
        &self.state
    }

    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Writes a change to this device's journal and applies it.
    pub fn record(&mut self, change: Change, now_ms: u64) -> io::Result<()> {
        let hlc = self.clock.tick(now_ms);
        if !self.read_only {
            self.sequence += 1;
            let line = Line {
                v: 1,
                id: format!("{}:{}", self.device, self.sequence),
                hlc: hlc.to_string(),
                change: change.clone(),
            };
            let mut bytes = serde_json::to_vec(&line).map_err(io::Error::other)?;
            bytes.push(b'\n');
            // One write per line: a reader elsewhere sees whole lines or none.
            self.journal.write_all(&bytes)?;
            self.journal.sync_data()?;
            let path = self.own_directory().join(journal_name(self.segment));
            *self.read_to.entry(path).or_default() += bytes.len() as u64;
            if self.journal.metadata()?.len() >= SEGMENT_BYTES {
                self.segment += 1;
                self.journal = OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(self.own_directory().join(journal_name(self.segment)))?;
            }
        }
        self.state.apply(&hlc, change);
        Ok(())
    }

    fn own_files(&self) -> Vec<PathBuf> {
        let files = fs::read_dir(self.own_directory()).into_iter().flatten().filter_map(Result::ok);
        files.map(|entry| entry.path()).filter(|path| is_journal(path)).collect()
    }

    fn own_bytes(&self) -> u64 {
        self.own_files().iter().filter_map(|path| fs::metadata(path).ok()).map(|metadata| metadata.len()).sum()
    }

    /// Folds this device's journals into one snapshot: for every fact this
    /// device was the last to speak about, its last word — with the time it was
    /// spoken. An hour of listening is sixty lines about one position; the
    /// snapshot keeps the last.
    ///
    /// Only this device's own files are ever touched. The snapshot is complete
    /// and in place before anything old is deleted, so a crash at any moment
    /// leaves the same library, only said twice.
    pub fn compact(&mut self, now_ms: u64) -> io::Result<()> {
        if self.read_only || self.own_unknown {
            return Ok(());
        }
        let old = self.own_files();
        let mut text = Vec::new();
        let app = concat!("torrocast ", env!("CARGO_PKG_VERSION")).to_owned();
        let registered = (self.clock.tick(now_ms), Change::DeviceRegistered { name: self.device_name.clone(), app });
        for (hlc, change) in self.state.last_words_of(&self.device).into_iter().chain(std::iter::once(registered)) {
            self.sequence += 1;
            let line = Line { v: 1, id: format!("{}:{}", self.device, self.sequence), hlc: hlc.to_string(), change };
            text.extend(serde_json::to_vec(&line).map_err(io::Error::other)?);
            text.push(b'\n');
        }
        let snapshot = self.own_directory().join(format!("snapshot-{:06}.jsonl", self.segment));
        let temporary = snapshot.with_extension("tmp");
        // Written and forced to disk through one handle: Windows refuses to flush a file opened for reading.
        let mut file = File::create(&temporary)?;
        file.write_all(&text)?;
        file.sync_all()?;
        drop(file);
        fs::rename(&temporary, &snapshot)?;

        self.segment += 1;
        self.journal =
            OpenOptions::new().create(true).append(true).open(self.own_directory().join(journal_name(self.segment)))?;
        for path in old.iter().filter(|path| **path != snapshot) {
            fs::remove_file(path)?;
            self.read_to.remove(path);
        }
        self.read_to.insert(snapshot, text.len() as u64);
        Ok(())
    }

    fn own_directory(&self) -> PathBuf {
        self.root.join("devices").join(&self.device)
    }

    /// Reads what has been added to any journal since the last look. `true` if the library changed.
    pub fn rescan(&mut self, now_ms: u64) -> io::Result<bool> {
        let mut changed = false;
        let mut journals = Vec::new();
        for device in fs::read_dir(self.root.join("devices"))?.filter_map(Result::ok) {
            if !device.path().is_dir() {
                continue;
            }
            // Everything that looks like a journal counts — a sync service's
            // "conflicted copy" of one too. Applying a change twice changes nothing.
            journals.extend(
                fs::read_dir(device.path())?
                    .filter_map(Result::ok)
                    .map(|entry| entry.path())
                    .filter(|path| is_journal(path)),
            );
        }
        journals.sort();
        for path in journals {
            let from = self.read_to.get(&path).copied().unwrap_or(0);
            let Ok(mut file) = File::open(&path) else { continue };
            let length = file.metadata()?.len();
            if length <= from {
                continue;
            }
            file.seek(SeekFrom::Start(from))?;
            let mut added = Vec::new();
            file.take(length - from).read_to_end(&mut added)?;
            // A last line without its newline is still being written, or still being synced.
            let complete = added.iter().rposition(|byte| *byte == b'\n').map_or(0, |end| end + 1);
            for text in added[..complete].split(|byte| *byte == b'\n').filter(|text| !text.is_empty()) {
                // A damaged line is skipped; it never stops the rest from being read.
                let Ok(line) = serde_json::from_slice::<Line>(text) else { continue };
                if line.change == Change::Unknown && path.starts_with(self.own_directory()) {
                    self.own_unknown = true;
                }
                let Ok(hlc) = line.hlc.parse::<Hlc>() else { continue };
                if let Some(sequence) =
                    line.id.strip_prefix(&format!("{}:", self.device)).and_then(|sequence| sequence.parse::<u64>().ok())
                {
                    self.sequence = self.sequence.max(sequence);
                }
                self.clock.observe(&hlc, now_ms);
                self.state.apply(&hlc, line.change);
                changed = true;
            }
            self.read_to.insert(path, from + complete as u64);
        }
        Ok(changed)
    }
}

/// Beside the target and renamed: a crash leaves the old file or the new one, never half of one.
fn write_whole(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let temporary = path.with_extension("tmp");
    fs::write(&temporary, bytes)?;
    fs::rename(temporary, path)
}
