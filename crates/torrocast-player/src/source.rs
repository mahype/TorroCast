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
        Ok(Self { shared, reader, position: 0, _file: file })
    }
}

impl Drop for HttpSource {
    fn drop(&mut self) {
        if let Ok(mut progress) = self.shared.progress.lock() {
            progress.abandoned = true;
        }
    }
}

impl Read for HttpSource {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
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
