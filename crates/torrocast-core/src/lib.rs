//! The portable heart of TorroCast. A user interface sends [`Command`]s and
//! reads [`Event`]s; everything slow happens on worker threads in between.
//! Nothing here knows what a terminal is.

pub mod settings;

use std::collections::HashMap;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex};
use std::thread;

use torrocast_directory::apple::Apple;
use torrocast_directory::fyyd::Fyyd;
use torrocast_directory::{DirectoryError, DirectoryProvider};
use torrocast_feed::chapters;
use torrocast_net::{Fetch, FetchError};

pub use settings::Settings;
pub use torrocast_directory::{Category, PodcastRef, ProviderId, merge};
pub use torrocast_feed::chapters::merge as merge_chapters;
pub use torrocast_feed::notes::{self, Block, Document, Inline};
pub use torrocast_feed::{Chapter, ChapterSource, Episode, Podcast};

/// Tags larger than this are cover art with chapters attached; not worth the traffic.
const MAX_TAG_BYTES: u64 = 3 * 1024 * 1024;

/// Why something did not work, in terms a user interface can put into words.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Problem {
    /// No answer: offline, or the server is down.
    Unreachable,
    /// The server answered with an error.
    Refused(u16),
    /// We held a request back to stay within a directory's limits.
    RateLimited,
    /// The address does not lead to a podcast feed.
    NotAFeed,
    /// An answer arrived that cannot be read.
    Unreadable,
}

impl From<FetchError> for Problem {
    fn from(error: FetchError) -> Self {
        match error {
            FetchError::Status(code) => Self::Refused(code),
            FetchError::Unreachable(_) => Self::Unreachable,
            FetchError::TooLarge | FetchError::RangeIgnored => Self::Unreadable,
        }
    }
}

impl From<DirectoryError> for Problem {
    fn from(error: DirectoryError) -> Self {
        match error {
            DirectoryError::RateLimited => Self::RateLimited,
            DirectoryError::Fetch(error) => error.into(),
            DirectoryError::Unreadable(_) | DirectoryError::Unsupported => Self::Unreadable,
        }
    }
}

/// `request` numbers are chosen by the caller and come back on the answer, so
/// an answer that arrives after the user has moved on can be recognised.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    Search {
        request: u64,
        query: String,
    },
    Charts {
        request: u64,
        category: Option<u32>,
    },
    Categories,
    OpenFeed {
        request: u64,
        feed_url: String,
        reload: bool,
    },
    /// Looks for chapters outside the feed: the JSON file it points to, and —
    /// only if nothing else announced chapters — the head of the MP3.
    Chapters {
        request: u64,
        chapters_url: Option<String>,
        mp3_url: Option<String>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    /// One directory's answer; a search yields one of these per active directory.
    SearchBatch {
        request: u64,
        provider: ProviderId,
        outcome: Result<Vec<PodcastRef>, Problem>,
    },
    Charts {
        request: u64,
        outcome: Result<Vec<PodcastRef>, Problem>,
    },
    Categories {
        outcome: Result<Vec<Category>, Problem>,
    },
    Feed {
        request: u64,
        outcome: Result<Arc<Podcast>, Problem>,
    },
    Chapters {
        request: u64,
        chapters: Vec<Chapter>,
    },
}

struct Shared {
    fetch: Arc<dyn Fetch>,
    apple: Apple,
    fyyd: Fyyd,
    feeds: Mutex<HashMap<String, Arc<Podcast>>>,
}

pub struct Core {
    shared: Arc<Shared>,
    settings: Settings,
    events: Sender<Event>,
}

impl Core {
    /// The core and the channel its events arrive on.
    #[must_use]
    pub fn new(fetch: Arc<dyn Fetch>, settings: Settings) -> (Self, Receiver<Event>) {
        let (events, receiver) = channel();
        let shared = Shared {
            fetch,
            apple: Apple::default(),
            fyyd: Fyyd,
            feeds: Mutex::new(HashMap::new()),
        };
        (
            Self {
                shared: Arc::new(shared),
                settings,
                events,
            },
            receiver,
        )
    }

    pub fn set_settings(&mut self, settings: Settings) {
        self.settings = settings;
    }

    /// The directories a search goes to right now.
    #[must_use]
    pub fn active_providers(&self) -> Vec<ProviderId> {
        let mut providers = vec![ProviderId::Apple];
        if self.settings.sources.fyyd {
            providers.push(ProviderId::Fyyd);
        }
        providers
    }

    /// Returns at once; the answer arrives as an [`Event`].
    pub fn send(&self, command: Command) {
        let country = self.settings.country.clone();
        match command {
            Command::Search { request, query } => {
                // One thread per directory: a slow one never holds back a fast one.
                for provider in self.active_providers() {
                    let (query, country) = (query.clone(), country.clone());
                    self.spawn(move |shared| {
                        let directory: &dyn DirectoryProvider = match provider {
                            ProviderId::Apple => &shared.apple,
                            ProviderId::Fyyd => &shared.fyyd,
                        };
                        let outcome = directory
                            .search(shared.fetch.as_ref(), &query, &country)
                            .map_err(Problem::from);
                        Event::SearchBatch {
                            request,
                            provider,
                            outcome,
                        }
                    });
                }
            }
            Command::Charts { request, category } => self.spawn(move |shared| {
                let outcome = shared
                    .apple
                    .charts(shared.fetch.as_ref(), &country, category)
                    .map_err(Problem::from);
                Event::Charts { request, outcome }
            }),
            Command::Categories => self.spawn(move |shared| {
                let outcome = shared
                    .apple
                    .categories(shared.fetch.as_ref(), &country)
                    .map_err(Problem::from);
                Event::Categories { outcome }
            }),
            Command::OpenFeed {
                request,
                feed_url,
                reload,
            } => {
                self.spawn(move |shared| Event::Feed {
                    request,
                    outcome: open_feed(shared, &feed_url, reload),
                });
            }
            Command::Chapters {
                request,
                chapters_url,
                mp3_url,
            } => self.spawn(move |shared| {
                let fetch = shared.fetch.as_ref();
                let mut found = chapters_url
                    .and_then(|url| fetch.get(&url).ok())
                    .and_then(|body| chapters::parse_json(&body).ok())
                    .unwrap_or_default();
                if found.is_empty()
                    && let Some(url) = mp3_url
                {
                    found = embedded_chapters(fetch, &url);
                }
                Event::Chapters {
                    request,
                    chapters: found,
                }
            }),
        }
    }

    fn spawn(&self, work: impl FnOnce(&Shared) -> Event + Send + 'static) {
        let (shared, events) = (Arc::clone(&self.shared), self.events.clone());
        thread::spawn(move || {
            // A closed channel means the interface is gone; nothing left to tell.
            let _ = events.send(work(&shared));
        });
    }
}

fn open_feed(shared: &Shared, feed_url: &str, reload: bool) -> Result<Arc<Podcast>, Problem> {
    if !reload
        && let Ok(feeds) = shared.feeds.lock()
        && let Some(known) = feeds.get(feed_url)
    {
        return Ok(Arc::clone(known));
    }
    let body = shared.fetch.get(feed_url)?;
    let podcast = torrocast_feed::parse_feed(&decode_body(&body)).map_err(|error| match error {
        torrocast_feed::FeedError::NotAFeed => Problem::NotAFeed,
        torrocast_feed::FeedError::Malformed(_) => Problem::Unreadable,
    })?;
    let podcast = Arc::new(podcast);
    if let Ok(mut feeds) = shared.feeds.lock() {
        feeds.insert(feed_url.to_owned(), Arc::clone(&podcast));
    }
    Ok(podcast)
}

/// Feeds are UTF-8 with few exceptions, and the exceptions say so up front.
fn decode_body(body: &[u8]) -> String {
    match std::str::from_utf8(body) {
        Ok(text) => text.to_owned(),
        Err(_) => {
            let declaration = String::from_utf8_lossy(&body[..body.len().min(200)]).to_lowercase();
            if declaration.contains("iso-8859-1") || declaration.contains("windows-1252") {
                body.iter().map(|byte| char::from(*byte)).collect()
            } else {
                String::from_utf8_lossy(body).into_owned()
            }
        }
    }
}

/// Reads the ID3 tag at the head of an MP3 — ten bytes to learn its size, then
/// exactly the tag. The audio itself is never requested.
fn embedded_chapters(fetch: &dyn Fetch, url: &str) -> Vec<Chapter> {
    let Some(size) = fetch
        .get_range(url, 0, 9)
        .ok()
        .and_then(|head| chapters::id3_tag_size(&head))
    else {
        return Vec::new();
    };
    if size > MAX_TAG_BYTES {
        return Vec::new();
    }
    fetch
        .get_range(url, 0, size - 1)
        .map(|tag| chapters::parse_id3(&tag))
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::Mutex;
    use std::time::Duration;

    use torrocast_net::{Fetch, FetchError};

    use super::{Command, Core, Event, Problem, ProviderId, Settings};

    const FEED: &str =
        r#"<rss version="2.0"><channel><title>Show</title><item><title>One</title></item></channel></rss>"#;

    struct Canned {
        asked: Mutex<Vec<String>>,
    }

    impl Fetch for Canned {
        fn get(&self, url: &str) -> Result<Vec<u8>, FetchError> {
            self.asked.lock().expect("no poisoning in tests").push(url.to_owned());
            match url {
                "https://show.example/feed" => Ok(FEED.as_bytes().to_vec()),
                "https://show.example/page" => Ok(b"<html/>".to_vec()),
                url if url.starts_with("https://api.fyyd.de") => {
                    Ok(br#"{"data":[{"title":"Found","xmlURL":"https://f.example"}]}"#.to_vec())
                }
                _ => Err(FetchError::Status(503)),
            }
        }

        fn get_range(&self, _url: &str, _start: u64, _end: u64) -> Result<Vec<u8>, FetchError> {
            Err(FetchError::RangeIgnored)
        }
    }

    fn core(fyyd: bool) -> (Core, std::sync::mpsc::Receiver<Event>, Arc<Canned>) {
        let canned = Arc::new(Canned {
            asked: Mutex::new(Vec::new()),
        });
        let mut settings = Settings::for_locale("de_DE");
        settings.sources.fyyd = fyyd;
        let (core, events) = Core::new(Arc::clone(&canned) as Arc<dyn Fetch>, settings);
        (core, events, canned)
    }

    fn next(events: &std::sync::mpsc::Receiver<Event>) -> Event {
        events.recv_timeout(Duration::from_secs(5)).expect("the worker answers")
    }

    #[test]
    fn a_feed_is_fetched_once() {
        let (core, events, canned) = core(false);
        for request in 1..=2 {
            core.send(Command::OpenFeed {
                request,
                feed_url: "https://show.example/feed".into(),
                reload: false,
            });
            let Event::Feed {
                request: answered,
                outcome,
            } = next(&events)
            else {
                panic!("expected a feed")
            };
            assert_eq!(answered, request);
            assert_eq!(outcome.expect("parses").episodes.len(), 1);
        }
        assert_eq!(canned.asked.lock().expect("no poisoning in tests").len(), 1);
    }

    #[test]
    fn problems_have_names() {
        let (core, events, _) = core(false);
        core.send(Command::OpenFeed {
            request: 1,
            feed_url: "https://show.example/page".into(),
            reload: false,
        });
        assert_eq!(
            next(&events),
            Event::Feed {
                request: 1,
                outcome: Err(Problem::NotAFeed)
            }
        );
        core.send(Command::OpenFeed {
            request: 2,
            feed_url: "https://down.example".into(),
            reload: false,
        });
        assert_eq!(
            next(&events),
            Event::Feed {
                request: 2,
                outcome: Err(Problem::Refused(503))
            }
        );
    }

    #[test]
    fn one_directory_failing_does_not_silence_the_other() {
        let (core, events, _) = core(true);
        core.send(Command::Search {
            request: 7,
            query: "found".into(),
        });
        let mut answers = [next(&events), next(&events)];
        answers.sort_by_key(|event| match event {
            Event::SearchBatch { provider, .. } => *provider,
            _ => ProviderId::Apple,
        });
        assert!(matches!(
            &answers[0],
            Event::SearchBatch {
                provider: ProviderId::Apple,
                outcome: Err(Problem::Refused(503)),
                ..
            }
        ));
        assert!(
            matches!(&answers[1], Event::SearchBatch { provider: ProviderId::Fyyd, outcome: Ok(found), .. } if found[0].title == "Found")
        );
    }
}
