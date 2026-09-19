//! The directories' real answers, recorded on 2026-09-19.

use std::collections::HashMap;
use std::sync::Mutex;

use torrocast_directory::apple::{self, Apple};
use torrocast_directory::fyyd;
use torrocast_directory::{DirectoryProvider, ProviderId};
use torrocast_net::{Fetch, FetchError};

const SEARCH: &str = include_str!("../../../docs/research/fixtures/apple-search-lage-der-nation.json");
const CHART: &str = include_str!("../../../docs/research/fixtures/apple-charts-de-top10.json");
const GENRE_CHART: &str = include_str!("../../../docs/research/fixtures/apple-charts-legacy-de.json");
const FYYD: &str = include_str!("../../../docs/research/fixtures/fyyd-search-lage-der-nation.json");

fn value(json: &str) -> serde_json::Value {
    serde_json::from_str(json).expect("fixture is JSON")
}

#[test]
fn apple_search_finds_the_feed() {
    let results = apple::parse_results(&value(SEARCH));
    let show = results
        .iter()
        .find(|show| show.title.starts_with("Lage der Nation"))
        .expect("the show is in the fixture");
    assert_eq!(
        show.feed_url.as_deref(),
        Some("https://feeds.lagedernation.org/feeds/ldn-mp3.xml")
    );
    assert_eq!(show.itunes_id, Some(1_092_957_894));
    assert!(show.genres.contains(&"Politik".to_owned()));
    assert!(
        !show.genres.contains(&"Podcasts".to_owned()),
        "the genre every podcast has says nothing"
    );
    assert_eq!(show.sources, vec![ProviderId::Apple]);
}

#[test]
fn both_chart_formats_name_the_same_leader() {
    let documented = apple::parse_chart(&value(CHART));
    let by_genre = apple::parse_genre_chart(&value(GENRE_CHART));
    assert_eq!(documented[0].itunes_id, Some(1_700_432_142));
    assert_eq!(by_genre[0].itunes_id, Some(1_700_432_142));
    assert!(
        documented.iter().all(|entry| entry.feed_url.is_none()),
        "charts carry no feeds; the lookup adds them"
    );
}

#[test]
fn fyyd_brings_what_apple_leaves_out() {
    let results = fyyd::parse_search(&value(FYYD));
    assert_eq!(
        results[0].feed_url.as_deref(),
        Some("https://feeds.lagedernation.org/feeds/ldn-mp3.xml")
    );
    assert_eq!(results[0].language.as_deref(), Some("de"));
    assert!(results[0].description.is_some());
}

/// Answers from a table, and remembers what was asked.
struct Canned {
    answers: HashMap<&'static str, &'static str>,
    asked: Mutex<Vec<String>>,
}

impl Fetch for Canned {
    fn get(&self, url: &str) -> Result<Vec<u8>, FetchError> {
        self.asked.lock().expect("no poisoning in tests").push(url.to_owned());
        self.answers
            .iter()
            .find(|(prefix, _)| url.starts_with(**prefix))
            .map(|(_, body)| body.as_bytes().to_vec())
            .ok_or(FetchError::Status(404))
    }

    fn get_range(&self, _url: &str, _start: u64, _end: u64) -> Result<Vec<u8>, FetchError> {
        Err(FetchError::RangeIgnored)
    }
}

#[test]
fn charts_keep_their_order_and_gain_feeds() {
    let lookup = r#"{"results":[
        {"kind":"podcast","collectionId":1092957894,"collectionName":"Lage der Nation","feedUrl":"https://l.example/feed"},
        {"kind":"podcast","collectionId":1700432142,"collectionName":"RONZHEIMER.","feedUrl":"https://r.example/feed"}]}"#;
    let canned = Canned {
        answers: HashMap::from([
            ("https://rss.marketingtools.apple.com/", CHART),
            ("https://itunes.apple.com/lookup", lookup),
        ]),
        asked: Mutex::new(Vec::new()),
    };
    let chart = Apple::default().charts(&canned, "DE", None).expect("canned answers");

    assert_eq!(chart[0].title, "RONZHEIMER.");
    assert_eq!(chart[0].feed_url.as_deref(), Some("https://r.example/feed"));
    assert!(
        chart[1].feed_url.is_none(),
        "a show the lookup does not know keeps its chart entry"
    );
    let asked = canned.asked.lock().expect("no poisoning in tests");
    assert_eq!(asked.len(), 2, "one call for the chart, one for all feeds");
    assert!(asked[0].contains("/de/"), "Apple wants the country in lower case");
}

#[test]
fn apple_stops_before_the_limit() {
    let canned = Canned {
        answers: HashMap::from([("https://itunes.apple.com/search", SEARCH)]),
        asked: Mutex::new(Vec::new()),
    };
    let apple = Apple::default();
    let refused = (0..25).filter(|_| apple.search(&canned, "lage", "de").is_err()).count();
    assert_eq!(refused, 7);
    assert_eq!(
        canned.asked.lock().expect("no poisoning in tests").len(),
        18,
        "a refused search sends nothing"
    );
}
