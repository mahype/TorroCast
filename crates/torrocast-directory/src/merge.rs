//! Several directories, one list: the same show found twice appears once.

use std::collections::HashMap;

use crate::PodcastRef;

/// The feed address as an identity: two spellings of one address compare equal.
/// The query string stays — private feeds carry their token there.
#[must_use]
pub fn normalise_feed_url(url: &str) -> String {
    let trimmed = url.trim();
    let without_scheme = trimmed.split_once("://").map_or(trimmed, |(_, rest)| rest);
    let without_fragment = without_scheme.split('#').next().unwrap_or(without_scheme);
    let (location, query) = match without_fragment.split_once('?') {
        Some((location, query)) => (location, Some(query)),
        None => (without_fragment, None),
    };
    let (host, path) = match location.split_once('/') {
        Some((host, path)) => (host, path),
        None => (location, ""),
    };
    let host = host.to_lowercase();
    let host = host
        .strip_suffix(":443")
        .or_else(|| host.strip_suffix(":80"))
        .unwrap_or(&host);
    let host = host.strip_prefix("www.").unwrap_or(host);
    let path = path.trim_end_matches('/');
    let mut identity = if path.is_empty() {
        host.to_owned()
    } else {
        format!("{host}/{path}")
    };
    if let Some(query) = query.filter(|query| !query.is_empty()) {
        identity.push('?');
        identity.push_str(query);
    }
    identity
}

/// Interleaves the directories' lists by rank and folds duplicates together.
/// A show both directories name ranks above one only a single directory knows.
#[must_use]
pub fn merge(batches: &[Vec<PodcastRef>]) -> Vec<PodcastRef> {
    const K: f64 = 60.0;
    let mut merged: Vec<(f64, PodcastRef)> = Vec::new();
    let mut by_itunes: HashMap<u64, usize> = HashMap::new();
    let mut by_feed: HashMap<String, usize> = HashMap::new();

    for batch in batches {
        for (rank, candidate) in batch.iter().enumerate() {
            let score = 1.0 / (K + rank as f64);
            let feed_key = candidate.feed_url.as_deref().map(normalise_feed_url);
            let known = candidate
                .itunes_id
                .and_then(|id| by_itunes.get(&id).copied())
                .or_else(|| feed_key.as_ref().and_then(|key| by_feed.get(key).copied()));
            let index = match known {
                Some(index) => {
                    merged[index].0 += score;
                    fill(&mut merged[index].1, candidate);
                    index
                }
                None => {
                    merged.push((score, candidate.clone()));
                    merged.len() - 1
                }
            };
            if let Some(id) = merged[index].1.itunes_id {
                by_itunes.insert(id, index);
            }
            if let Some(key) = feed_key {
                by_feed.insert(key, index);
            }
        }
    }

    merged.sort_by(|left, right| right.0.total_cmp(&left.0));
    merged.into_iter().map(|(_, podcast)| podcast).collect()
}

/// What the first directory left open, the second may fill in.
fn fill(known: &mut PodcastRef, other: &PodcastRef) {
    fn take<T: Clone>(slot: &mut Option<T>, other: &Option<T>) {
        if slot.is_none() {
            slot.clone_from(other);
        }
    }
    take(&mut known.author, &other.author);
    take(&mut known.feed_url, &other.feed_url);
    take(&mut known.itunes_id, &other.itunes_id);
    take(&mut known.artwork_url, &other.artwork_url);
    take(&mut known.episode_count, &other.episode_count);
    take(&mut known.last_published, &other.last_published);
    take(&mut known.website, &other.website);
    take(&mut known.description, &other.description);
    take(&mut known.language, &other.language);
    if known.genres.is_empty() {
        known.genres.clone_from(&other.genres);
    }
    for source in &other.sources {
        if !known.sources.contains(source) {
            known.sources.push(*source);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{merge, normalise_feed_url};
    use crate::{PodcastRef, ProviderId};

    #[test]
    fn spellings_of_one_address_are_one_identity() {
        let expected = "feeds.lagedernation.org/feeds/ldn-mp3.xml";
        assert_eq!(
            normalise_feed_url("https://feeds.lagedernation.org/feeds/ldn-mp3.xml"),
            expected
        );
        assert_eq!(
            normalise_feed_url("http://WWW.feeds.lagedernation.org:80/feeds/ldn-mp3.xml/#top"),
            expected
        );
    }

    #[test]
    fn the_query_is_part_of_the_identity() {
        assert_ne!(
            normalise_feed_url("https://a.example/feed?token=1"),
            normalise_feed_url("https://a.example/feed?token=2")
        );
    }

    fn show(title: &str, feed: &str, source: ProviderId) -> PodcastRef {
        PodcastRef {
            title: title.into(),
            feed_url: Some(feed.into()),
            sources: vec![source],
            ..PodcastRef::default()
        }
    }

    #[test]
    fn a_show_both_directories_know_appears_once_and_first() {
        let apple = vec![
            show("Other", "https://o.example/feed", ProviderId::Apple),
            show("Lage", "https://l.example/feed", ProviderId::Apple),
        ];
        let mut described = show("Lage der Nation", "http://www.l.example/feed/", ProviderId::Fyyd);
        described.description = Some("Politik".into());
        let merged = merge(&[apple, vec![described]]);

        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0].title, "Lage");
        assert_eq!(merged[0].description.as_deref(), Some("Politik"));
        assert_eq!(merged[0].sources, vec![ProviderId::Apple, ProviderId::Fyyd]);
    }
}
