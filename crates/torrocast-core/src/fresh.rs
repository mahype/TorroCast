//! New episodes: what the subscribed podcasts published lately and the user
//! has not heard — the list a listener opens first.

use std::sync::Arc;

use chrono::{DateTime, Duration, Utc};
use torrocast_feed::Podcast;

use crate::playback::QueueItem;

/// How far back "new" reaches. Two weeks, as Pocket Casts' New Releases.
pub const NEW_FOR_DAYS: i64 = 14;
/// A feed that publishes daily must not crowd out everyone else.
const PER_PODCAST: usize = 5;

#[derive(Debug, Clone, PartialEq)]
pub struct NewEpisode {
    pub item: QueueItem,
    pub published: DateTime<Utc>,
}

/// The recent, unheard episodes of `feeds`, newest first.
#[must_use]
pub fn newest(
    feeds: &[(String, Arc<Podcast>)],
    now: DateTime<Utc>,
    is_played: impl Fn(&QueueItem) -> bool,
) -> Vec<NewEpisode> {
    let since = now - Duration::days(NEW_FOR_DAYS);
    let mut episodes: Vec<NewEpisode> = feeds
        .iter()
        .flat_map(|(feed_url, podcast)| {
            let mut recent: Vec<NewEpisode> = podcast
                .episodes
                .iter()
                .filter_map(|episode| {
                    let published = episode.published.filter(|published| *published >= since)?;
                    let item = QueueItem::from_feed(podcast, Some(feed_url), episode)?;
                    (!is_played(&item)).then_some(NewEpisode { item, published })
                })
                .collect();
            recent.sort_by_key(|episode| std::cmp::Reverse(episode.published));
            recent.truncate(PER_PODCAST);
            recent
        })
        .collect();
    episodes.sort_by(|left, right| {
        right.published.cmp(&left.published).then_with(|| left.item.title.cmp(&right.item.title))
    });
    episodes
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use chrono::{DateTime, Duration, Utc};
    use torrocast_feed::{Enclosure, Episode, Podcast};

    use super::newest;

    fn now() -> DateTime<Utc> {
        DateTime::parse_from_rfc3339("2026-09-19T12:00:00Z").expect("a date").with_timezone(&Utc)
    }

    fn episode(title: &str, days_ago: i64, with_audio: bool) -> Episode {
        Episode {
            title: title.into(),
            guid: Some(title.into()),
            published: Some(now() - Duration::days(days_ago)),
            enclosure: with_audio.then(|| Enclosure {
                url: format!("https://cdn.example/{title}.mp3"),
                mime: None,
                bytes: None,
            }),
            ..Episode::default()
        }
    }

    fn feed(title: &str, episodes: Vec<Episode>) -> (String, Arc<Podcast>) {
        (
            format!("https://{title}.example/feed"),
            Arc::new(Podcast { title: title.into(), episodes, ..Podcast::default() }),
        )
    }

    #[test]
    fn recent_unheard_playable_newest_first() {
        let feeds = vec![
            feed("alpha", vec![episode("a-new", 1, true), episode("a-old", 40, true), episode("a-heard", 2, true)]),
            feed("beta", vec![episode("b-newest", 0, true), episode("b-silent", 0, false)]),
        ];
        let found = newest(&feeds, now(), |item| item.title == "a-heard");
        let titles: Vec<&str> = found.iter().map(|episode| episode.item.title.as_str()).collect();
        assert_eq!(titles, vec!["b-newest", "a-new"]);
        assert_eq!(found[1].item.podcast, "alpha");
        assert_eq!(found[1].item.feed_url.as_deref(), Some("https://alpha.example/feed"));
    }

    #[test]
    fn a_daily_show_does_not_bury_the_others() {
        let daily: Vec<_> = (0..12).map(|day| episode(&format!("daily-{day:02}"), day, true)).collect();
        let feeds = vec![feed("daily", daily), feed("weekly", vec![episode("weekly-1", 6, true)])];
        let found = newest(&feeds, now(), |_| false);
        assert_eq!(found.iter().filter(|episode| episode.item.podcast == "daily").count(), 5);
        assert!(found.iter().any(|episode| episode.item.title == "weekly-1"));
    }
}
