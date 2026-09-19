//! Podcast directories. A directory is only ever used to *find* a show; once
//! its feed address is known, everything else comes from the feed itself.

pub mod apple;
pub mod fyyd;
mod merge;
pub mod podcast_index;

use std::fmt;

use chrono::{DateTime, Utc};
use torrocast_net::{Fetch, FetchError};

pub use merge::{merge, normalise_feed_url};

/// Which directory an answer came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ProviderId {
    Apple,
    PodcastIndex,
    Fyyd,
}

impl ProviderId {
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Apple => "Apple",
            Self::PodcastIndex => "Podcast Index",
            Self::Fyyd => "fyyd",
        }
    }
}

/// A show as a directory describes it. Thin on purpose: directories disagree
/// about everything except where the feed is.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct PodcastRef {
    pub title: String,
    pub author: Option<String>,
    /// Missing for shows that exist only inside a closed platform.
    pub feed_url: Option<String>,
    pub itunes_id: Option<u64>,
    pub artwork_url: Option<String>,
    pub genres: Vec<String>,
    pub episode_count: Option<u32>,
    pub last_published: Option<DateTime<Utc>>,
    pub website: Option<String>,
    pub description: Option<String>,
    pub language: Option<String>,
    pub sources: Vec<ProviderId>,
}

/// A single episode as a directory's search finds it — enough to play it
/// without ever opening its feed.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct EpisodeRef {
    pub title: String,
    pub podcast: String,
    pub feed_url: Option<String>,
    pub guid: Option<String>,
    pub audio_url: String,
    pub duration_ms: Option<u64>,
    pub published: Option<DateTime<Utc>>,
    pub description: Option<String>,
}

/// One entry of a directory's category tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Category {
    pub id: u32,
    pub name: String,
    /// 0 for a main category, 1 for one below it.
    pub depth: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DirectoryError {
    /// We would exceed what the directory allows; nothing was sent.
    RateLimited,
    Fetch(FetchError),
    /// The directory answered with something we cannot read.
    Unreadable(String),
    Unsupported,
}

impl fmt::Display for DirectoryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RateLimited => write!(formatter, "rate limited"),
            Self::Fetch(error) => write!(formatter, "{error}"),
            Self::Unreadable(reason) => write!(formatter, "unreadable answer: {reason}"),
            Self::Unsupported => write!(formatter, "not supported by this directory"),
        }
    }
}

impl std::error::Error for DirectoryError {}

impl From<FetchError> for DirectoryError {
    fn from(error: FetchError) -> Self {
        Self::Fetch(error)
    }
}

pub trait DirectoryProvider: Send + Sync {
    fn id(&self) -> ProviderId;

    /// `country` is a two-letter code; directories without a notion of country ignore it.
    fn search(&self, fetch: &dyn Fetch, query: &str, country: &str) -> Result<Vec<PodcastRef>, DirectoryError>;

    /// Episodes, not shows. Not every directory can.
    fn search_episodes(
        &self,
        _fetch: &dyn Fetch,
        _query: &str,
        _country: &str,
    ) -> Result<Vec<EpisodeRef>, DirectoryError> {
        Err(DirectoryError::Unsupported)
    }

    /// The most popular shows, overall or within one category.
    fn charts(
        &self,
        _fetch: &dyn Fetch,
        _country: &str,
        _category: Option<u32>,
    ) -> Result<Vec<PodcastRef>, DirectoryError> {
        Err(DirectoryError::Unsupported)
    }

    fn categories(&self, _fetch: &dyn Fetch, _country: &str) -> Result<Vec<Category>, DirectoryError> {
        Err(DirectoryError::Unsupported)
    }
}

pub(crate) fn json(body: &[u8]) -> Result<serde_json::Value, DirectoryError> {
    serde_json::from_slice(body).map_err(|error| DirectoryError::Unreadable(error.to_string()))
}

pub(crate) fn text(value: &serde_json::Value) -> Option<String> {
    value.as_str().map(str::trim).filter(|text| !text.is_empty()).map(str::to_owned)
}
