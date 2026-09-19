//! Reads what a podcast publishes: the feed, its chapters, its show notes.
//! Pure parsing — nothing here touches the network.

pub mod chapters;
mod entities;
pub mod notes;
mod parse;

use std::fmt;

use chrono::{DateTime, Utc};

pub use chapters::{Chapter, ChapterSource};
pub use parse::parse_feed;

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Podcast {
    pub title: String,
    pub author: Option<String>,
    /// Plain text; whatever markup the feed used is gone.
    pub description: Option<String>,
    pub website: Option<String>,
    pub image: Option<String>,
    pub language: Option<String>,
    pub categories: Vec<String>,
    pub explicit: bool,
    pub funding: Vec<Funding>,
    pub persons: Vec<Person>,
    /// `podcast:guid` — the show's identity across feed moves.
    pub guid: Option<String>,
    /// `itunes:new-feed-url`: the show has moved and says where to.
    pub new_feed_url: Option<String>,
    pub episodes: Vec<Episode>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Funding {
    pub url: String,
    pub label: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Person {
    pub name: String,
    pub role: Option<String>,
    pub url: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Episode {
    pub guid: Option<String>,
    pub title: String,
    pub published: Option<DateTime<Utc>>,
    pub duration_seconds: Option<u32>,
    pub season: Option<u32>,
    pub number: Option<u32>,
    pub enclosure: Option<Enclosure>,
    pub link: Option<String>,
    /// The show notes as the feed carries them, usually HTML. See [`notes::document`].
    pub notes_html: Option<String>,
    pub image: Option<String>,
    /// Podlove Simple Chapters, which travel inside the feed.
    pub chapters: Vec<Chapter>,
    /// `podcast:chapters`: chapters in a JSON file of their own.
    pub chapters_url: Option<String>,
    pub transcripts: Vec<Transcript>,
    pub persons: Vec<Person>,
}

impl Episode {
    /// Whether opening the episode can be expected to show chapters.
    #[must_use]
    pub fn announces_chapters(&self) -> bool {
        !self.chapters.is_empty() || self.chapters_url.is_some()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Enclosure {
    pub url: String,
    pub mime: Option<String>,
    pub bytes: Option<u64>,
}

impl Enclosure {
    /// MP3 is where ID3 chapter frames live.
    #[must_use]
    pub fn is_mp3(&self) -> bool {
        let path = self.url.split(['?', '#']).next().unwrap_or(&self.url).to_lowercase();
        path.ends_with(".mp3") || self.mime.as_deref().is_some_and(|mime| mime.eq_ignore_ascii_case("audio/mpeg"))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Transcript {
    pub url: String,
    pub mime: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FeedError {
    /// Not XML, even after repairing what feeds commonly get wrong.
    Malformed(String),
    /// XML, but not an RSS feed: a web page, an Atom feed, an error document.
    NotAFeed,
}

impl fmt::Display for FeedError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Malformed(reason) => write!(formatter, "malformed: {reason}"),
            Self::NotAFeed => write!(formatter, "not a podcast feed"),
        }
    }
}

impl std::error::Error for FeedError {}
