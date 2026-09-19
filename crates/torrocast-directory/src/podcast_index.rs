//! podcastindex.org — the open index. Free, but every user brings a key of
//! their own: the terms forbid shipping one inside an open-source project.

use std::time::{SystemTime, UNIX_EPOCH};

use chrono::DateTime;
use serde_json::Value;
use sha1::{Digest, Sha1};
use torrocast_net::{Fetch, USER_AGENT, encode};

use crate::{DirectoryError, DirectoryProvider, PodcastRef, ProviderId, json, text};

const API: &str = "https://api.podcastindex.org/api/1.0";

pub struct PodcastIndex {
    key: String,
    secret: String,
}

impl PodcastIndex {
    #[must_use]
    pub fn new(key: &str, secret: &str) -> Self {
        Self { key: key.trim().to_owned(), secret: secret.trim().to_owned() }
    }

    /// The four headers every call needs. The signature is the SHA-1 of key, secret and the time.
    #[must_use]
    pub fn headers(&self, unix_time: u64) -> Vec<(&'static str, String)> {
        let mut hasher = Sha1::new();
        hasher.update(format!("{}{}{unix_time}", self.key, self.secret));
        let signature: String = hasher.finalize().iter().map(|byte| format!("{byte:02x}")).collect();
        vec![
            // The index refuses generic agents.
            ("User-Agent", USER_AGENT.to_owned()),
            ("X-Auth-Key", self.key.clone()),
            ("X-Auth-Date", unix_time.to_string()),
            ("Authorization", signature),
        ]
    }

    fn call(&self, fetch: &dyn Fetch, path: &str) -> Result<Value, DirectoryError> {
        let now = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |since| since.as_secs());
        json(&fetch.get_with(&format!("{API}{path}"), &self.headers(now))?)
    }

    /// Whether the index accepts the key — asked once when it is entered.
    pub fn verify(&self, fetch: &dyn Fetch) -> Result<(), DirectoryError> {
        self.call(fetch, "/search/byterm?q=podcast&max=1").map(|_| ())
    }
}

impl DirectoryProvider for PodcastIndex {
    fn id(&self) -> ProviderId {
        ProviderId::PodcastIndex
    }

    fn search(&self, fetch: &dyn Fetch, query: &str, _country: &str) -> Result<Vec<PodcastRef>, DirectoryError> {
        Ok(parse_feeds(&self.call(fetch, &format!("/search/byterm?max=40&q={}", encode(query)))?))
    }
}

#[must_use]
pub fn parse_feeds(answer: &Value) -> Vec<PodcastRef> {
    let Some(feeds) = answer["feeds"].as_array() else {
        return Vec::new();
    };
    feeds
        .iter()
        // The index keeps feeds that stopped answering long ago; nobody searches for those.
        .filter(|feed| feed["dead"].as_u64().unwrap_or(0) == 0)
        .filter_map(|feed| {
            Some(PodcastRef {
                title: text(&feed["title"])?,
                author: text(&feed["author"]).or_else(|| text(&feed["ownerName"])),
                feed_url: text(&feed["url"]).or_else(|| text(&feed["originalUrl"])),
                itunes_id: feed["itunesId"].as_u64(),
                artwork_url: text(&feed["artwork"]).or_else(|| text(&feed["image"])),
                genres: feed["categories"]
                    .as_object()
                    .map(|names| names.values().filter_map(text).collect())
                    .unwrap_or_default(),
                episode_count: feed["episodeCount"].as_u64().and_then(|count| u32::try_from(count).ok()),
                last_published: feed["newestItemPubdate"]
                    .as_i64()
                    .filter(|time| *time > 0)
                    .and_then(|time| DateTime::from_timestamp(time, 0)),
                website: text(&feed["link"]),
                description: text(&feed["description"]),
                language: text(&feed["language"]),
                sources: vec![ProviderId::PodcastIndex],
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{PodcastIndex, parse_feeds};

    #[test]
    fn requests_are_signed_as_the_index_demands() {
        // sha1("key" + "secret" + "1700000000"), computed independently with sha1sum.
        let headers = PodcastIndex::new(" key ", "secret\n").headers(1_700_000_000);
        let signature = &headers.iter().find(|(name, _)| *name == "Authorization").expect("signed").1;
        assert_eq!(signature, "abaf71c02050c31e4d4e6b08c1625173af0445ba", "lower-case hex, as the index demands");
        assert_eq!(headers[1].1, "key", "stray spaces from pasting are gone");
        assert_eq!(headers[2].1, "1700000000");
    }

    #[test]
    fn feeds_become_shows_and_dead_ones_are_dropped() {
        let answer: serde_json::Value = serde_json::from_str(
            r#"{"status":"true","feeds":[
            {"id":1,"title":"Lage der Nation","url":"https://feeds.example/ldn","link":"https://lagedernation.org",
             "description":"Politik","author":"Banse & Buermeyer","artwork":"https://img.example/a.jpg","itunesId":1092957894,
             "language":"de","categories":{"55":"News","59":"Politics"},"episodeCount":497,"newestItemPubdate":1789813980,"dead":0},
            {"id":2,"title":"Gone","url":"https://gone.example","dead":1}]}"#,
        )
        .expect("valid JSON");
        let shows = parse_feeds(&answer);
        assert_eq!(shows.len(), 1);
        assert_eq!(shows[0].itunes_id, Some(1_092_957_894));
        assert_eq!(shows[0].language.as_deref(), Some("de"));
        assert_eq!(shows[0].episode_count, Some(497));
        assert!(shows[0].genres.contains(&"Politics".to_owned()));
        assert!(shows[0].last_published.is_some());
    }
}
