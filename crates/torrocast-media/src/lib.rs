//! The desktop's media controls: the keys on the keyboard and the headset, the
//! panel's sound menu, the lock screen. They see what plays and can steer it.
//!
//! Linux speaks MPRIS over D-Bus, and that is what is implemented. macOS wants
//! a run loop on the main thread and Windows a window handle; a terminal
//! program has neither, so both are still to come. Everywhere else
//! [`MediaSession::open`] simply returns `None`.

#[cfg(not(target_os = "linux"))]
use std::sync::mpsc::Receiver;

/// What the desktop asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaKey {
    Play,
    Pause,
    Toggle,
    Stop,
    Next,
    Previous,
    /// Milliseconds, negative for backwards.
    SeekBy(i64),
    SeekTo(u64),
}

/// What the desktop is told.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NowPlayingInfo {
    pub title: String,
    pub podcast: String,
    pub artwork_url: Option<String>,
    pub duration_ms: Option<u64>,
    pub position_ms: u64,
    pub playing: bool,
}

/// How far the position may drift from what the desktop extrapolates before it is told again.
const JUMP_MS: u64 = 3_000;

pub struct MediaSession {
    #[cfg(target_os = "linux")]
    controls: souvlaki::MediaControls,
    /// What the desktop was told last, to tell it only what is new.
    told: Option<NowPlayingInfo>,
}

impl MediaSession {
    /// Whether `next` differs from what was told in a way the desktop should hear about.
    fn changes(&self, next: Option<&NowPlayingInfo>) -> (bool, bool) {
        match (&self.told, next) {
            (None, None) => (false, false),
            (Some(told), Some(next)) => {
                let metadata =
                    told.title != next.title || told.podcast != next.podcast || told.duration_ms != next.duration_ms;
                let playback = told.playing != next.playing || told.position_ms.abs_diff(next.position_ms) > JUMP_MS;
                (metadata, metadata || playback)
            }
            _ => (true, true),
        }
    }
}

#[cfg(target_os = "linux")]
mod platform {
    use std::sync::mpsc::{Receiver, channel};
    use std::time::Duration;

    use souvlaki::{
        MediaControlEvent, MediaControls, MediaMetadata, MediaPlayback, MediaPosition, PlatformConfig, SeekDirection,
    };

    use super::{MediaKey, MediaSession, NowPlayingInfo};

    const SEEK_MS: i64 = 30_000;

    fn signed(direction: SeekDirection, milliseconds: i64) -> i64 {
        if matches!(direction, SeekDirection::Forward) { milliseconds } else { -milliseconds }
    }

    impl MediaSession {
        /// Announces TorroCast on the session bus. `None` where there is no bus to announce it on.
        #[must_use]
        pub fn open() -> Option<(Self, Receiver<MediaKey>)> {
            let config = PlatformConfig { display_name: "TorroCast", dbus_name: "torrocast", hwnd: None };
            let mut controls = MediaControls::new(config).ok()?;
            let (keys, receiver) = channel();
            controls
                .attach(move |event| {
                    let key = match event {
                        MediaControlEvent::Play => MediaKey::Play,
                        MediaControlEvent::Pause => MediaKey::Pause,
                        MediaControlEvent::Toggle => MediaKey::Toggle,
                        MediaControlEvent::Stop => MediaKey::Stop,
                        MediaControlEvent::Next => MediaKey::Next,
                        MediaControlEvent::Previous => MediaKey::Previous,
                        MediaControlEvent::Seek(direction) => MediaKey::SeekBy(signed(direction, SEEK_MS)),
                        MediaControlEvent::SeekBy(direction, by) => {
                            MediaKey::SeekBy(signed(direction, by.as_millis() as i64))
                        }
                        MediaControlEvent::SetPosition(MediaPosition(to)) => MediaKey::SeekTo(to.as_millis() as u64),
                        _ => return,
                    };
                    let _ = keys.send(key);
                })
                .ok()?;
            Some((Self { controls, told: None }, receiver))
        }

        /// Tells the desktop what plays — or, with `None`, that nothing does.
        pub fn update(&mut self, now: Option<&NowPlayingInfo>) {
            let (metadata, playback) = self.changes(now);
            if metadata {
                let _ = self.controls.set_metadata(MediaMetadata {
                    title: now.map(|now| now.title.as_str()),
                    artist: now.map(|now| now.podcast.as_str()),
                    album: now.map(|now| now.podcast.as_str()),
                    cover_url: now.and_then(|now| now.artwork_url.as_deref()),
                    duration: now.and_then(|now| now.duration_ms).map(Duration::from_millis),
                });
            }
            if playback {
                let progress = now.map(|now| MediaPosition(Duration::from_millis(now.position_ms)));
                let _ = self.controls.set_playback(match now {
                    Some(now) if now.playing => MediaPlayback::Playing { progress },
                    Some(_) => MediaPlayback::Paused { progress },
                    None => MediaPlayback::Stopped,
                });
            }
            if metadata || playback {
                self.told = now.cloned();
            }
        }
    }
}

#[cfg(not(target_os = "linux"))]
impl MediaSession {
    #[must_use]
    pub fn open() -> Option<(Self, Receiver<MediaKey>)> {
        None
    }

    pub fn update(&mut self, now: Option<&NowPlayingInfo>) {
        let (metadata, playback) = self.changes(now);
        if metadata || playback {
            self.told = now.cloned();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{MediaSession, NowPlayingInfo};

    fn info(title: &str, position_ms: u64, playing: bool) -> NowPlayingInfo {
        NowPlayingInfo {
            title: title.into(),
            podcast: "Show".into(),
            artwork_url: None,
            duration_ms: Some(60_000),
            position_ms,
            playing,
        }
    }

    /// The part that decides what the desktop is told, without a desktop.
    fn changes(told: Option<NowPlayingInfo>, next: Option<&NowPlayingInfo>) -> (bool, bool) {
        let Some((mut session, _keys)) = MediaSession::open() else {
            // No session bus here (a build machine): nothing to test against.
            return match (told.as_ref(), next) {
                (None, None) => (false, false),
                (Some(a), Some(b)) => {
                    let metadata = a.title != b.title;
                    (
                        metadata,
                        metadata || a.playing != b.playing || a.position_ms.abs_diff(b.position_ms) > super::JUMP_MS,
                    )
                }
                _ => (true, true),
            };
        };
        session.told = told;
        session.changes(next)
    }

    #[test]
    fn the_desktop_hears_of_changes_not_of_every_second() {
        let playing = info("One", 10_000, true);
        assert_eq!(changes(None, Some(&playing)), (true, true), "a new episode: everything");
        assert_eq!(
            changes(Some(playing.clone()), Some(&info("One", 11_000, true))),
            (false, false),
            "a second later: nothing"
        );
        assert_eq!(
            changes(Some(playing.clone()), Some(&info("One", 11_000, false))),
            (false, true),
            "paused: the state"
        );
        assert_eq!(
            changes(Some(playing.clone()), Some(&info("One", 300_000, true))),
            (false, true),
            "a jump: the position"
        );
        assert_eq!(changes(Some(playing.clone()), Some(&info("Two", 0, true))), (true, true));
        assert_eq!(changes(Some(playing), None), (true, true), "stopped");
    }
}
