//! The user's library — subscriptions, playback positions, playlists — kept
//! in a folder the user chooses, so Dropbox, Syncthing, a NAS or git can carry
//! it between devices.
//!
//! File sync services are not databases: two devices writing one file means a
//! "conflicted copy" and lost data. So no file here ever has two writers.
//! Every device appends to a journal of its own, and the library is what all
//! journals together say.

pub mod clock;
mod folder;
pub mod state;

use uuid::Uuid;

pub use folder::{Folder, OpenError};
pub use state::{Change, Progress, State, StoredItem, Subscription, UP_NEXT};

/// The namespace the Podcasting 2.0 specification defines for podcast guids.
const PODCAST_NAMESPACE: Uuid =
    Uuid::from_bytes([0xea, 0xd4, 0xc2, 0x36, 0xbf, 0x58, 0x58, 0xc6, 0xa2, 0xc6, 0xa6, 0xb2, 0x8d, 0x12, 0x8c, 0xb6]);

/// A podcast's identity: the guid its feed declares, or the one the
/// specification derives from the feed address — the same on every device,
/// with no network involved.
#[must_use]
pub fn podcast_id(declared_guid: Option<&str>, feed_url: &str) -> String {
    if let Some(guid) = declared_guid.map(str::trim).filter(|guid| !guid.is_empty()) {
        return guid.to_lowercase();
    }
    let address = feed_url.trim();
    let address = address.split_once("://").map_or(address, |(_, rest)| rest).trim_end_matches('/');
    Uuid::new_v5(&PODCAST_NAMESPACE, address.as_bytes()).to_string()
}

/// An episode's identity from whatever identifies it to the player. The
/// prefix versions the scheme.
#[must_use]
pub fn episode_id(key: &str) -> String {
    format!("ep1:{}", Uuid::new_v5(&Uuid::NAMESPACE_URL, key.as_bytes()))
}

/// A name for a new playlist in the library's files.
#[must_use]
pub fn new_playlist_id() -> String {
    format!("pl-{}", &Uuid::new_v4().simple().to_string()[..12])
}

#[must_use]
pub fn new_device_id() -> String {
    Uuid::new_v4().simple().to_string()[..12].to_owned()
}

#[cfg(test)]
mod tests {
    use super::{episode_id, podcast_id};

    #[test]
    fn the_specifications_own_example() {
        // podcast-namespace, docs/tags/guid.md
        assert_eq!(podcast_id(None, "https://podnews.net/rss"), "9b024349-ccf0-5f69-a609-6b82873eab3c");
        assert_eq!(podcast_id(None, "http://podnews.net/rss/"), "9b024349-ccf0-5f69-a609-6b82873eab3c");
        assert_eq!(
            podcast_id(Some(" 917393E3-1b1e-5cef-ace4-edaa54e1f810 "), "https://x.example"),
            "917393e3-1b1e-5cef-ace4-edaa54e1f810"
        );
    }

    #[test]
    fn episodes_get_the_same_id_everywhere() {
        assert_eq!(episode_id("feed\nguid"), episode_id("feed\nguid"));
        assert_ne!(episode_id("feed\nguid"), episode_id("feed\nother"));
        assert!(episode_id("x").starts_with("ep1:"));
    }
}
