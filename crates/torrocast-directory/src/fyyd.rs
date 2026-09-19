//! fyyd.de — a directory with its heart in the German-speaking podcast scene.
//! Run by one person, without published terms: treat every failure as normal.

use chrono::{DateTime, Utc};
use serde_json::Value;
use torrocast_net::{Fetch, encode};

use crate::{DirectoryError, DirectoryProvider, PodcastRef, ProviderId, json, text};

#[derive(Default)]
pub struct Fyyd;

impl DirectoryProvider for Fyyd {
    fn id(&self) -> ProviderId {
        ProviderId::Fyyd
    }

    fn search(&self, fetch: &dyn Fetch, query: &str, _country: &str) -> Result<Vec<PodcastRef>, DirectoryError> {
        let url = format!("https://api.fyyd.de/0.2/search/podcast?count=30&title={}", encode(query));
        Ok(parse_search(&json(&fetch.get(&url)?)?))
    }
}

#[must_use]
pub fn parse_search(answer: &Value) -> Vec<PodcastRef> {
    let Some(shows) = answer["data"].as_array() else {
        return Vec::new();
    };
    shows
        .iter()
        .filter_map(|show| {
            Some(PodcastRef {
                title: text(&show["title"])?,
                author: text(&show["author"]),
                feed_url: text(&show["xmlURL"]),
                artwork_url: text(&show["layoutImageURL"]).or_else(|| text(&show["imgURL"])),
                episode_count: show["episode_count"].as_u64().and_then(|count| u32::try_from(count).ok()),
                last_published: text(&show["lastpub"])
                    .and_then(|date| DateTime::parse_from_rfc3339(&date).ok())
                    .map(|date| date.with_timezone(&Utc)),
                website: text(&show["htmlURL"]),
                description: text(&show["description"]).or_else(|| text(&show["subtitle"])),
                language: text(&show["language"]),
                sources: vec![ProviderId::Fyyd],
                ..PodcastRef::default()
            })
        })
        .collect()
}
