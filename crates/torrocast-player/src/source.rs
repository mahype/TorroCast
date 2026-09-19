//! An episode on a server, readable like a file while it is still arriving.
//! One thread downloads front to back into a temporary file; the reader waits
//! whenever it gets ahead of the download.

use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

use symphonia::core::io::MediaSource;
use tempfile::NamedTempFile;
use torrocast_net::USER_AGENT;

/// A reader that has waited this long for bytes gives up: the connection is gone.
const STALLED: Duration = Duration::from_secs(30);
/// A read this far ahead of the download is not waited for: that part is fetched on its own.
const FAR_AHEAD: u64 = 1024 * 1024;
/// How much is fetched at a time for a place the download has not reached.
const SIDE_BYTES: u64 = 512 * 1024;

#[derive(Default)]
struct Progress {
    downloaded: u64,
    total: Option<u64>,
    finished: bool,
    failed: Option<String>,
    /// Set when nobody reads any more; the download stops at the next chunk.
    abandoned: bool,
}

struct Shared {
    progress: Mutex<Progress>,
    changed: Condvar,
}

pub struct HttpSource {
    shared: Arc<Shared>,
    reader: File,
    position: u64,
    url: String,
    agent: ureq::Agent,
    /// A piece of the episode from beyond the download: where it starts, and its bytes.
    side: Option<(u64, Vec<u8>)>,
    // Deletes the file when the source goes away.
    _file: NamedTempFile,
}

impl HttpSource {
    /// Connects and starts downloading. Returns once the server has answered.
    pub fn open(url: &str) -> io::Result<Self> {
        let agent = ureq::AgentBuilder::new()
            .user_agent(USER_AGENT)
            .timeout_connect(Duration::from_secs(15))
            .redirects(10)
            .build();
        let response = agent.get(url).call().map_err(io::Error::other)?;
        let total = response.header("Content-Length").and_then(|length| length.parse().ok());

        let file = NamedTempFile::with_prefix("torrocast-")?;
        let reader = file.reopen()?;
        let mut writer = file.reopen()?;
        let shared = Arc::new(Shared {
            progress: Mutex::new(Progress { total, ..Progress::default() }),
            changed: Condvar::new(),
        });

        let download = Arc::clone(&shared);
        std::thread::spawn(move || {
            let mut body = response.into_reader();
            let mut chunk = vec![0u8; 64 * 1024];
            let outcome = loop {
                let read = match body.read(&mut chunk) {
                    Ok(0) => break Ok(()),
                    Ok(read) => read,
                    Err(error) => break Err(error.to_string()),
                };
                if let Err(error) = writer.write_all(&chunk[..read]) {
                    break Err(error.to_string());
                }
                let Ok(mut progress) = download.progress.lock() else { return };
                if progress.abandoned {
                    return;
                }
                progress.downloaded += read as u64;
                download.changed.notify_all();
            };
            if let Ok(mut progress) = download.progress.lock() {
                progress.finished = true;
                progress.failed = outcome.err();
                download.changed.notify_all();
            }
        });
        Ok(Self { shared, reader, position: 0, url: url.to_owned(), agent, side: None, _file: file })
    }
}

impl Drop for HttpSource {
    fn drop(&mut self) {
        if let Ok(mut progress) = self.shared.progress.lock() {
            progress.abandoned = true;
        }
    }
}

impl HttpSource {
    /// Serves a read from beyond the download with a request of its own. A
    /// jump to the last hour of an episode, or a demuxer looking at the end of
    /// the file for its length, then costs one small request instead of the
    /// wait for everything before it. `None` if the server will not do ranges.
    fn read_far_ahead(&mut self, buffer: &mut [u8], total: u64) -> Option<usize> {
        let covered = self
            .side
            .as_ref()
            .is_some_and(|(start, bytes)| (*start..*start + bytes.len() as u64).contains(&self.position));
        if !covered {
            let end = (self.position + SIDE_BYTES).min(total) - 1;
            let response =
                self.agent.get(&self.url).set("Range", &format!("bytes={}-{end}", self.position)).call().ok()?;
            if response.status() != 206 {
                return None;
            }
            let mut bytes = Vec::new();
            response.into_reader().take(SIDE_BYTES).read_to_end(&mut bytes).ok()?;
            if bytes.is_empty() {
                return None;
            }
            self.side = Some((self.position, bytes));
        }
        let (start, bytes) = self.side.as_ref()?;
        let from = usize::try_from(self.position - start).ok()?;
        let count = buffer.len().min(bytes.len() - from);
        buffer[..count].copy_from_slice(&bytes[from..from + count]);
        self.position += count as u64;
        Some(count)
    }
}

impl Read for HttpSource {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        let far_ahead = self.shared.progress.lock().ok().and_then(|progress| {
            let total = progress.total?;
            (!progress.finished && self.position < total && self.position > progress.downloaded + FAR_AHEAD)
                .then_some(total)
        });
        if let Some(total) = far_ahead
            && let Some(count) = self.read_far_ahead(buffer, total)
        {
            return Ok(count);
        }
        let available = {
            let mut progress = self.shared.progress.lock().map_err(|_| io::Error::other("download thread panicked"))?;
            loop {
                if progress.downloaded > self.position {
                    break progress.downloaded - self.position;
                }
                if let Some(reason) = &progress.failed {
                    return Err(io::Error::other(reason.clone()));
                }
                if progress.finished {
                    return Ok(0);
                }
                let (next, waited) = self
                    .shared
                    .changed
                    .wait_timeout(progress, STALLED)
                    .map_err(|_| io::Error::other("download thread panicked"))?;
                progress = next;
                if waited.timed_out() && progress.downloaded <= self.position {
                    return Err(io::Error::new(io::ErrorKind::TimedOut, "the download stalled"));
                }
            }
        };
        let wanted = buffer.len().min(usize::try_from(available).unwrap_or(usize::MAX));
        self.reader.seek(SeekFrom::Start(self.position))?;
        let read = self.reader.read(&mut buffer[..wanted])?;
        self.position += read as u64;
        Ok(read)
    }
}

impl Seek for HttpSource {
    /// Seeking ahead of the download is allowed; the next read waits for it.
    fn seek(&mut self, to: SeekFrom) -> io::Result<u64> {
        let target = match to {
            SeekFrom::Start(offset) => Some(offset),
            SeekFrom::Current(delta) => self.position.checked_add_signed(delta),
            SeekFrom::End(delta) => {
                let total = self.shared.progress.lock().ok().and_then(|progress| progress.total);
                total.ok_or_else(|| io::Error::other("the server did not tell the size"))?.checked_add_signed(delta)
            }
        };
        self.position = target.ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "seek before the start"))?;
        Ok(self.position)
    }
}

impl MediaSource for HttpSource {
    fn is_seekable(&self) -> bool {
        true
    }

    fn byte_len(&self) -> Option<u64> {
        self.shared.progress.lock().ok().and_then(|progress| progress.total)
    }
}
