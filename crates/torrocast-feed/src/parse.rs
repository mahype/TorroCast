//! RSS 2.0 with the extensions podcasts actually use.

use chrono::{DateTime, Utc};
use roxmltree::{Document, Node, ParsingOptions};

use crate::chapters::{Chapter, ChapterSource, parse_npt};
use crate::notes::plain_text;
use crate::{Enclosure, Episode, FeedError, Funding, Person, Podcast, Transcript, entities};

/// The vocabularies a podcast feed mixes. Feeds spell the namespace addresses
/// with varying case and trailing slashes, so they are matched loosely.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Vocabulary {
    Rss,
    Itunes,
    Content,
    Podlove,
    Podcast,
    Other,
}

fn vocabulary(node: Node<'_, '_>) -> Vocabulary {
    let Some(namespace) = node.tag_name().namespace() else {
        return Vocabulary::Rss;
    };
    let namespace = namespace.to_lowercase();
    if namespace.contains("itunes.com/dtds/podcast") {
        Vocabulary::Itunes
    } else if namespace.contains("purl.org/rss/1.0/modules/content") {
        Vocabulary::Content
    } else if namespace.contains("podlove.org/simple-chapters") {
        Vocabulary::Podlove
    } else if namespace.contains("podcastindex.org/namespace") || namespace.contains("podcast-namespace") {
        Vocabulary::Podcast
    } else {
        Vocabulary::Other
    }
}

fn children<'a, 'input>(
    node: Node<'a, 'input>,
    wanted: Vocabulary,
    name: &'static str,
) -> impl Iterator<Item = Node<'a, 'input>> {
    node.children()
        .filter(move |child| child.is_element() && child.tag_name().name() == name && vocabulary(*child) == wanted)
}

fn child<'a, 'input>(node: Node<'a, 'input>, wanted: Vocabulary, name: &'static str) -> Option<Node<'a, 'input>> {
    children(node, wanted, name).next()
}

/// All text below `node`, CDATA included, or `None` when that is nothing.
fn content(node: Node<'_, '_>) -> Option<String> {
    let text: String = node.descendants().filter(Node::is_text).filter_map(|text| text.text()).collect();
    let trimmed = text.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_owned())
}

fn text(node: Node<'_, '_>, wanted: Vocabulary, name: &'static str) -> Option<String> {
    child(node, wanted, name).and_then(content)
}

/// A title or a name. Many feeds escape these twice, so `&amp;` survives the
/// XML parser; nobody means to show that.
fn label(node: Node<'_, '_>, wanted: Vocabulary, name: &'static str) -> Option<String> {
    text(node, wanted, name).map(|label| entities::decode(&label))
}

fn attribute(node: Node<'_, '_>, name: &str) -> Option<String> {
    node.attribute(name).map(str::trim).filter(|value| !value.is_empty()).map(str::to_owned)
}

pub fn parse_feed(xml: &str) -> Result<Podcast, FeedError> {
    let xml = xml.trim_start_matches('\u{feff}').trim_start();
    let options = ParsingOptions { allow_dtd: true, ..ParsingOptions::default() };
    match Document::parse_with_options(xml, options) {
        Ok(document) => read(&document),
        Err(first) => {
            // Feeds written by hand or by old plugins use HTML's entity names.
            let repaired = entities::repair_xml(xml);
            let document = Document::parse_with_options(&repaired, options)
                .map_err(|_| FeedError::Malformed(first.to_string()))?;
            read(&document)
        }
    }
}

fn read(document: &Document<'_>) -> Result<Podcast, FeedError> {
    let root = document.root_element();
    if root.tag_name().name() != "rss" {
        return Err(FeedError::NotAFeed);
    }
    let channel = child(root, Vocabulary::Rss, "channel").ok_or(FeedError::NotAFeed)?;

    let description = text(channel, Vocabulary::Rss, "description")
        .or_else(|| text(channel, Vocabulary::Itunes, "summary"))
        .map(|description| plain_text(&description))
        .filter(|description| !description.is_empty());
    let image = child(channel, Vocabulary::Itunes, "image")
        .and_then(|image| attribute(image, "href"))
        .or_else(|| child(channel, Vocabulary::Rss, "image").and_then(|image| text(image, Vocabulary::Rss, "url")));

    let mut categories = Vec::new();
    for category in children(channel, Vocabulary::Itunes, "category") {
        for level in std::iter::once(category).chain(children(category, Vocabulary::Itunes, "category")) {
            if let Some(name) = attribute(level, "text").filter(|name| !categories.contains(name)) {
                categories.push(name);
            }
        }
    }

    Ok(Podcast {
        title: label(channel, Vocabulary::Rss, "title").unwrap_or_default(),
        author: label(channel, Vocabulary::Itunes, "author"),
        description,
        website: text(channel, Vocabulary::Rss, "link"),
        image,
        language: text(channel, Vocabulary::Rss, "language"),
        categories,
        explicit: text(channel, Vocabulary::Itunes, "explicit").is_some_and(|value| is_yes(&value)),
        funding: children(channel, Vocabulary::Podcast, "funding")
            .filter_map(|funding| Some(Funding { url: attribute(funding, "url")?, label: content(funding) }))
            .collect(),
        persons: persons(channel),
        guid: text(channel, Vocabulary::Podcast, "guid"),
        new_feed_url: text(channel, Vocabulary::Itunes, "new-feed-url"),
        episodes: children(channel, Vocabulary::Rss, "item").map(episode).collect(),
    })
}

fn is_yes(value: &str) -> bool {
    matches!(value.to_lowercase().as_str(), "yes" | "true" | "explicit")
}

fn persons(node: Node<'_, '_>) -> Vec<Person> {
    children(node, Vocabulary::Podcast, "person")
        .filter_map(|person| {
            Some(Person { name: content(person)?, role: attribute(person, "role"), url: attribute(person, "href") })
        })
        .collect()
}

fn episode(item: Node<'_, '_>) -> Episode {
    let title =
        label(item, Vocabulary::Rss, "title").or_else(|| label(item, Vocabulary::Itunes, "title")).unwrap_or_default();
    // The richest text wins: content:encoded is the full notes, description often a teaser.
    let notes_html = text(item, Vocabulary::Content, "encoded")
        .or_else(|| text(item, Vocabulary::Rss, "description"))
        .or_else(|| text(item, Vocabulary::Itunes, "summary"));
    let enclosure = child(item, Vocabulary::Rss, "enclosure").and_then(|enclosure| {
        Some(Enclosure {
            url: attribute(enclosure, "url")?,
            mime: attribute(enclosure, "type"),
            bytes: attribute(enclosure, "length").and_then(|length| length.parse().ok()).filter(|bytes| *bytes > 0),
        })
    });
    let chapters = child(item, Vocabulary::Podlove, "chapters")
        .map(|list| {
            children(list, Vocabulary::Podlove, "chapter")
                .filter_map(|chapter| {
                    Some(Chapter {
                        start_ms: parse_npt(chapter.attribute("start")?)?,
                        title: attribute(chapter, "title"),
                        url: attribute(chapter, "href"),
                        image: attribute(chapter, "image"),
                        hidden: false,
                        source: ChapterSource::Feed,
                    })
                })
                .collect()
        })
        .unwrap_or_default();

    Episode {
        guid: text(item, Vocabulary::Rss, "guid"),
        title,
        published: text(item, Vocabulary::Rss, "pubDate").and_then(|date| parse_date(&date)),
        duration_seconds: text(item, Vocabulary::Itunes, "duration").and_then(|duration| parse_duration(&duration)),
        season: text(item, Vocabulary::Itunes, "season").and_then(|season| season.parse().ok()),
        number: text(item, Vocabulary::Itunes, "episode").and_then(|number| number.parse().ok()),
        enclosure,
        link: text(item, Vocabulary::Rss, "link"),
        notes_html,
        image: child(item, Vocabulary::Itunes, "image").and_then(|image| attribute(image, "href")),
        chapters,
        chapters_url: child(item, Vocabulary::Podcast, "chapters").and_then(|chapters| attribute(chapters, "url")),
        transcripts: children(item, Vocabulary::Podcast, "transcript")
            .filter_map(|transcript| {
                Some(Transcript { url: attribute(transcript, "url")?, mime: attribute(transcript, "type") })
            })
            .collect(),
        persons: persons(item),
    }
}

/// RFC 2822 as RSS demands, RFC 3339 as some feeds send instead.
fn parse_date(date: &str) -> Option<DateTime<Utc>> {
    let date = date.trim();
    DateTime::parse_from_rfc2822(date)
        .or_else(|_| DateTime::parse_from_rfc3339(date))
        .ok()
        .map(|date| date.with_timezone(&Utc))
}

/// `1:34:10`, `94:10`, `5650` and `5650.4` all occur in the wild.
fn parse_duration(duration: &str) -> Option<u32> {
    let mut seconds: u32 = 0;
    for part in duration.trim().split(':') {
        let value = part.trim().split('.').next()?.parse::<u32>().ok()?;
        seconds = seconds.checked_mul(60)?.checked_add(value)?;
    }
    (seconds > 0).then_some(seconds)
}

#[cfg(test)]
mod tests {
    use super::{parse_date, parse_duration};

    #[test]
    fn durations_in_every_spelling() {
        assert_eq!(parse_duration("1:34:10"), Some(5650));
        assert_eq!(parse_duration("94:10"), Some(5650));
        assert_eq!(parse_duration("5650"), Some(5650));
        assert_eq!(parse_duration("5650.4"), Some(5650));
        assert_eq!(parse_duration("0"), None);
        assert_eq!(parse_duration("soon"), None);
    }

    #[test]
    fn dates_in_both_standards() {
        let expected = "2026-09-19T10:33:00+00:00";
        assert_eq!(
            parse_date("Sat, 19 Sep 2026 12:33:00 +0200").map(|date| date.to_rfc3339()).as_deref(),
            Some(expected)
        );
        assert_eq!(parse_date("2026-09-19T10:33:00Z").map(|date| date.to_rfc3339()).as_deref(), Some(expected));
    }
}
