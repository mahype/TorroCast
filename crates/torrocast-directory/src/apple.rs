//! Apple's podcast directory: search, country charts and the genre tree.
//! No key, but roughly twenty calls a minute — see [`Budget`].

use std::collections::{HashMap, VecDeque};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use chrono::{DateTime, Utc};
use serde_json::Value;
use torrocast_net::{Fetch, encode};

use crate::{Category, DirectoryError, DirectoryProvider, EpisodeRef, PodcastRef, ProviderId, json, text};

const PODCASTS_GENRE: u32 = 26;
const CHART_SIZE: u32 = 50;

/// Apple documents "approximately 20 calls per minute". We stay below it
/// ourselves rather than find out what happens above.
struct Budget {
    calls: Mutex<VecDeque<Instant>>,
}

impl Budget {
    const CALLS: usize = 18;
    const WINDOW: Duration = Duration::from_secs(60);

    fn spend(&self) -> Result<(), DirectoryError> {
        let Ok(mut calls) = self.calls.lock() else {
            return Ok(());
        };
        let now = Instant::now();
        while calls.front().is_some_and(|call| now.duration_since(*call) > Self::WINDOW) {
            calls.pop_front();
        }
        if calls.len() >= Self::CALLS {
            return Err(DirectoryError::RateLimited);
        }
        calls.push_back(now);
        Ok(())
    }
}

pub struct Apple {
    budget: Budget,
}

impl Default for Apple {
    fn default() -> Self {
        Self { budget: Budget { calls: Mutex::new(VecDeque::new()) } }
    }
}

impl Apple {
    fn call(&self, fetch: &dyn Fetch, url: &str) -> Result<Value, DirectoryError> {
        self.budget.spend()?;
        json(&fetch.get(url)?)
    }

    /// The charts name shows by id only; one lookup turns the ids into feeds.
    fn resolve(
        &self,
        fetch: &dyn Fetch,
        ranked: Vec<PodcastRef>,
        country: &str,
    ) -> Result<Vec<PodcastRef>, DirectoryError> {
        let ids: Vec<String> = ranked.iter().filter_map(|podcast| podcast.itunes_id).map(|id| id.to_string()).collect();
        if ids.is_empty() {
            return Ok(ranked);
        }
        let url =
            format!("https://itunes.apple.com/lookup?id={}&country={}&entity=podcast", ids.join(","), encode(country));
        let answer = self.call(fetch, &url)?;
        let mut details: HashMap<u64, PodcastRef> = parse_results(&answer)
            .into_iter()
            .filter_map(|podcast| podcast.itunes_id.map(|id| (id, podcast)))
            .collect();
        // Chart order is the point of a chart; the lookup answers in its own order.
        Ok(ranked
            .into_iter()
            .map(|entry| entry.itunes_id.and_then(|id| details.remove(&id)).unwrap_or(entry))
            .collect())
    }
}

impl DirectoryProvider for Apple {
    fn id(&self) -> ProviderId {
        ProviderId::Apple
    }

    fn search(&self, fetch: &dyn Fetch, query: &str, country: &str) -> Result<Vec<PodcastRef>, DirectoryError> {
        let url = format!(
            "https://itunes.apple.com/search?media=podcast&entity=podcast&limit=50&term={}&country={}",
            encode(query),
            encode(country)
        );
        Ok(parse_results(&self.call(fetch, &url)?))
    }

    fn search_episodes(
        &self,
        fetch: &dyn Fetch,
        query: &str,
        country: &str,
    ) -> Result<Vec<EpisodeRef>, DirectoryError> {
        let url = format!(
            "https://itunes.apple.com/search?media=podcast&entity=podcastEpisode&limit=50&term={}&country={}",
            encode(query),
            encode(country)
        );
        Ok(parse_episodes(&self.call(fetch, &url)?))
    }

    fn charts(
        &self,
        fetch: &dyn Fetch,
        country: &str,
        category: Option<u32>,
    ) -> Result<Vec<PodcastRef>, DirectoryError> {
        let country = country.to_lowercase();
        let ranked = match category {
            // The documented feed has no genre filter; the older one does.
            Some(genre) => {
                let url =
                    format!("https://itunes.apple.com/{country}/rss/toppodcasts/limit={CHART_SIZE}/genre={genre}/json");
                parse_genre_chart(&self.call(fetch, &url)?)
            }
            None => {
                let url = format!(
                    "https://rss.marketingtools.apple.com/api/v2/{country}/podcasts/top/{CHART_SIZE}/podcasts.json"
                );
                parse_chart(&self.call(fetch, &url)?)
            }
        };
        self.resolve(fetch, ranked, &country)
    }

    fn categories(&self, fetch: &dyn Fetch, country: &str) -> Result<Vec<Category>, DirectoryError> {
        let url = format!(
            "https://itunes.apple.com/WebObjects/MZStoreServices.woa/ws/genres?id={PODCASTS_GENRE}&cc={}",
            encode(&country.to_lowercase())
        );
        Ok(parse_genres(&self.call(fetch, &url)?))
    }
}

fn number(value: &Value) -> Option<u64> {
    value.as_u64().or_else(|| value.as_str().and_then(|text| text.parse().ok()))
}

/// The answer of `search` and `lookup`.
#[must_use]
pub fn parse_results(answer: &Value) -> Vec<PodcastRef> {
    let Some(results) = answer["results"].as_array() else {
        return Vec::new();
    };
    results
        .iter()
        .filter(|entry| entry["kind"].as_str().is_none_or(|kind| kind == "podcast"))
        .filter_map(|entry| {
            let title = text(&entry["collectionName"]).or_else(|| text(&entry["trackName"]))?;
            let genres = entry["genres"]
                .as_array()
                .map(|genres| genres.iter().filter_map(text).filter(|genre| genre != "Podcasts").collect())
                .unwrap_or_default();
            Some(PodcastRef {
                title,
                author: text(&entry["artistName"]),
                feed_url: text(&entry["feedUrl"]),
                itunes_id: number(&entry["collectionId"]),
                artwork_url: text(&entry["artworkUrl600"]).or_else(|| text(&entry["artworkUrl100"])),
                genres,
                episode_count: number(&entry["trackCount"]).and_then(|count| u32::try_from(count).ok()),
                last_published: text(&entry["releaseDate"])
                    .and_then(|date| DateTime::parse_from_rfc3339(&date).ok())
                    .map(|date| date.with_timezone(&Utc)),
                website: None,
                description: None,
                language: None,
                sources: vec![ProviderId::Apple],
            })
        })
        .collect()
}

/// The answer of `search` with `entity=podcastEpisode`. Episodes without an
/// audio address are left out: they could be listed but never played.
#[must_use]
pub fn parse_episodes(answer: &Value) -> Vec<EpisodeRef> {
    let Some(results) = answer["results"].as_array() else {
        return Vec::new();
    };
    results
        .iter()
        .filter_map(|entry| {
            Some(EpisodeRef {
                title: text(&entry["trackName"])?,
                podcast: text(&entry["collectionName"]).unwrap_or_default(),
                feed_url: text(&entry["feedUrl"]),
                guid: text(&entry["episodeGuid"]),
                audio_url: text(&entry["episodeUrl"])?,
                duration_ms: number(&entry["trackTimeMillis"]),
                published: text(&entry["releaseDate"])
                    .and_then(|date| DateTime::parse_from_rfc3339(&date).ok())
                    .map(|date| date.with_timezone(&Utc)),
                description: text(&entry["shortDescription"]).or_else(|| text(&entry["description"])),
            })
        })
        .collect()
}

/// `rss.marketingtools.apple.com/api/v2/…/podcasts.json`
#[must_use]
pub fn parse_chart(answer: &Value) -> Vec<PodcastRef> {
    let Some(results) = answer["feed"]["results"].as_array() else {
        return Vec::new();
    };
    results
        .iter()
        .filter_map(|entry| {
            Some(PodcastRef {
                title: text(&entry["name"])?,
                author: text(&entry["artistName"]),
                itunes_id: number(&entry["id"]),
                artwork_url: text(&entry["artworkUrl100"]),
                genres: entry["genres"]
                    .as_array()
                    .map(|genres| genres.iter().filter_map(|genre| text(&genre["name"])).collect())
                    .unwrap_or_default(),
                sources: vec![ProviderId::Apple],
                ..PodcastRef::default()
            })
        })
        .collect()
}

/// `itunes.apple.com/{cc}/rss/toppodcasts/…/json`
#[must_use]
pub fn parse_genre_chart(answer: &Value) -> Vec<PodcastRef> {
    let entries = &answer["feed"]["entry"];
    // A chart of one is an object, not a list of one.
    let entries: Vec<&Value> = match entries {
        Value::Array(entries) => entries.iter().collect(),
        Value::Object(_) => vec![entries],
        _ => Vec::new(),
    };
    entries
        .into_iter()
        .filter_map(|entry| {
            Some(PodcastRef {
                title: text(&entry["im:name"]["label"])?,
                author: text(&entry["im:artist"]["label"]),
                itunes_id: number(&entry["id"]["attributes"]["im:id"]),
                artwork_url: entry["im:image"]
                    .as_array()
                    .and_then(|images| images.last())
                    .and_then(|image| text(&image["label"])),
                genres: text(&entry["category"]["attributes"]["label"]).into_iter().collect(),
                description: text(&entry["summary"]["label"]),
                sources: vec![ProviderId::Apple],
                ..PodcastRef::default()
            })
        })
        .collect()
}

/// The localised genre tree below "Podcasts", flattened in reading order.
#[must_use]
pub fn parse_genres(answer: &Value) -> Vec<Category> {
    fn walk(node: &Value, depth: u8, into: &mut Vec<Category>) {
        let Some(children) = node["subgenres"].as_object() else {
            return;
        };
        let mut children: Vec<&Value> = children.values().collect();
        children.sort_by_key(|child| text(&child["name"]).unwrap_or_default().to_lowercase());
        for child in children {
            let (Some(id), Some(name)) = (number(&child["id"]), text(&child["name"])) else {
                continue;
            };
            let Ok(id) = u32::try_from(id) else { continue };
            into.push(Category { id, name, depth });
            if depth == 0 {
                walk(child, 1, into);
            }
        }
    }
    let mut categories = Vec::new();
    walk(&answer[PODCASTS_GENRE.to_string()], 0, &mut categories);
    categories
}
