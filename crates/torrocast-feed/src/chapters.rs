//! Chapter marks reach a client three ways: inside the feed (Podlove), in a
//! JSON file the feed points to (Podcasting 2.0), and inside the audio file
//! (ID3). All three end up as the same [`Chapter`].

use std::io::Cursor;

use id3::TagLike;
use serde::Deserialize;

use crate::FeedError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChapterSource {
    /// Podlove Simple Chapters in the feed.
    Feed,
    /// A `podcast:chapters` JSON file.
    Json,
    /// ID3 chapter frames at the head of the audio file.
    AudioFile,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Chapter {
    pub start_ms: u64,
    pub title: Option<String>,
    pub url: Option<String>,
    pub image: Option<String>,
    /// `toc: false` — a mark that changes artwork or a link but is no entry in the list.
    pub hidden: bool,
    pub source: ChapterSource,
}

/// Normal Play Time as Podlove writes it: `HH:MM:SS.mmm`, `MM:SS`, `SS`.
#[must_use]
pub fn parse_npt(time: &str) -> Option<u64> {
    let mut milliseconds: u64 = 0;
    let parts: Vec<&str> = time.trim().split(':').collect();
    if parts.is_empty() || parts.len() > 3 {
        return None;
    }
    for (index, part) in parts.iter().enumerate() {
        let is_seconds = index == parts.len() - 1;
        let (whole, fraction) = match part.split_once('.') {
            Some((whole, fraction)) if is_seconds => (whole, fraction),
            Some(_) => return None,
            None => (*part, ""),
        };
        let whole: u64 = whole.parse().ok()?;
        milliseconds = milliseconds.checked_mul(60)?.checked_add(whole.checked_mul(1000)?)?;
        if !fraction.is_empty() {
            let digits: String = fraction.chars().chain("000".chars()).take(3).collect();
            milliseconds = milliseconds.checked_add(digits.parse().ok()?)?;
        }
    }
    Some(milliseconds)
}

#[derive(Deserialize)]
struct JsonFile {
    chapters: Vec<JsonChapter>,
}

#[derive(Deserialize)]
struct JsonChapter {
    #[serde(rename = "startTime")]
    start_time: f64,
    title: Option<String>,
    img: Option<String>,
    url: Option<String>,
    toc: Option<bool>,
}

fn filled(value: Option<String>) -> Option<String> {
    value.map(|value| value.trim().to_owned()).filter(|value| !value.is_empty())
}

/// `application/json+chapters`, version 1.x.
pub fn parse_json(body: &[u8]) -> Result<Vec<Chapter>, FeedError> {
    let file: JsonFile = serde_json::from_slice(body).map_err(|error| FeedError::Malformed(error.to_string()))?;
    let mut chapters: Vec<Chapter> = file
        .chapters
        .into_iter()
        .filter(|chapter| chapter.start_time.is_finite() && chapter.start_time >= 0.0)
        .map(|chapter| Chapter {
            start_ms: (chapter.start_time * 1000.0).round() as u64,
            title: filled(chapter.title),
            url: filled(chapter.url),
            image: filled(chapter.img),
            hidden: chapter.toc == Some(false),
            source: ChapterSource::Json,
        })
        .collect();
    chapters.sort_by_key(|chapter| chapter.start_ms);
    Ok(chapters)
}

/// How many bytes the ID3v2 tag at the head of a file spans, read from its
/// first ten bytes — so exactly the tag can be fetched, and not the episode.
#[must_use]
pub fn id3_tag_size(head: &[u8]) -> Option<u64> {
    let header = head.get(..10)?;
    if &header[..3] != b"ID3" || header[6..].iter().any(|byte| byte & 0x80 != 0) {
        return None;
    }
    let size = header[6..].iter().fold(0u64, |size, byte| (size << 7) | u64::from(*byte));
    let footer = if header[5] & 0x10 != 0 { 10 } else { 0 };
    Some(10 + size + footer)
}

/// The chapter frames of an ID3v2 tag. `tag` is the head of an MP3, at least
/// [`id3_tag_size`] bytes long.
#[must_use]
pub fn parse_id3(tag: &[u8]) -> Vec<Chapter> {
    let Ok(tag) = id3::Tag::read_from2(Cursor::new(tag)) else {
        return Vec::new();
    };
    let mut chapters: Vec<Chapter> = tag
        .chapters()
        .map(|chapter| Chapter {
            start_ms: u64::from(chapter.start_time),
            title: filled(chapter.title().map(str::to_owned)),
            url: chapter
                .frames
                .iter()
                .find_map(|frame| {
                    frame
                        .content()
                        .extended_link()
                        .map(|link| link.link.clone())
                        .or_else(|| frame.content().link().map(str::to_owned))
                })
                .and_then(|url| filled(Some(url))),
            image: None,
            hidden: false,
            source: ChapterSource::AudioFile,
        })
        .collect();
    chapters.sort_by_key(|chapter| chapter.start_ms);
    chapters
}

fn richness(chapters: &[Chapter]) -> usize {
    chapters
        .iter()
        .map(|chapter| {
            usize::from(chapter.title.is_some())
                + usize::from(chapter.url.is_some())
                + usize::from(chapter.image.is_some())
        })
        .sum()
}

/// Two lists for one episode become one, after AntennaPod's rule: the longer
/// list wins; of two equally long lists that disagree about the times, the
/// richer one wins; lists that agree fill each other's gaps, `preferred` first.
#[must_use]
pub fn merge(fallback: Vec<Chapter>, preferred: Vec<Chapter>) -> Vec<Chapter> {
    const TOLERANCE_MS: u64 = 1000;
    if preferred.is_empty() {
        return fallback;
    }
    if fallback.is_empty() {
        return preferred;
    }
    if fallback.len() != preferred.len() {
        return if fallback.len() > preferred.len() { fallback } else { preferred };
    }
    let agree =
        fallback.iter().zip(&preferred).all(|(left, right)| left.start_ms.abs_diff(right.start_ms) <= TOLERANCE_MS);
    if !agree {
        return if richness(&fallback) > richness(&preferred) { fallback } else { preferred };
    }
    preferred
        .into_iter()
        .zip(fallback)
        .map(|(mut chapter, other)| {
            chapter.title = chapter.title.or(other.title);
            chapter.url = chapter.url.or(other.url);
            chapter.image = chapter.image.or(other.image);
            chapter
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{Chapter, ChapterSource, id3_tag_size, merge, parse_json, parse_npt};

    #[test]
    fn normal_play_time() {
        assert_eq!(parse_npt("01:02:03.5"), Some(3_723_500));
        assert_eq!(parse_npt("02:03"), Some(123_000));
        assert_eq!(parse_npt("7"), Some(7_000));
        assert_eq!(parse_npt("00:00:01.0625"), Some(1_062));
        assert_eq!(parse_npt("1:2:3:4"), None);
        assert_eq!(parse_npt("1.5:00"), None);
    }

    #[test]
    fn json_chapters_sorted_with_silent_marks() {
        let body = br#"{"version":"1.2.0","chapters":[
            {"startTime":94.5,"title":"Thema","url":"https://example.org"},
            {"startTime":0,"title":"Intro"},
            {"startTime":120,"img":"https://example.org/a.jpg","toc":false}]}"#;
        let chapters = parse_json(body).expect("valid chapters");
        assert_eq!(chapters.iter().map(|chapter| chapter.start_ms).collect::<Vec<_>>(), vec![0, 94_500, 120_000]);
        assert_eq!(chapters[1].url.as_deref(), Some("https://example.org"));
        assert!(chapters[2].hidden && chapters[2].title.is_none());
    }

    #[test]
    fn tag_size_is_syncsafe() {
        assert_eq!(id3_tag_size(b"ID3\x04\x00\x00\x00\x00\x02\x01"), Some(10 + 257));
        assert_eq!(id3_tag_size(b"ID3\x04\x00\x10\x00\x00\x02\x01"), Some(10 + 257 + 10), "a footer counts");
        assert_eq!(id3_tag_size(b"\xff\xfb\x90\x00\x00\x00\x00\x00\x00\x00"), None, "audio without a tag");
        assert_eq!(id3_tag_size(b"ID3"), None);
    }

    fn chapter(start_ms: u64, title: Option<&str>, url: Option<&str>, source: ChapterSource) -> Chapter {
        Chapter {
            start_ms,
            title: title.map(str::to_owned),
            url: url.map(str::to_owned),
            image: None,
            hidden: false,
            source,
        }
    }

    #[test]
    fn agreeing_lists_fill_each_other() {
        let feed = vec![chapter(0, Some("Intro"), Some("https://example.org"), ChapterSource::Feed)];
        let json = vec![chapter(400, Some("Begrüßung"), None, ChapterSource::Json)];
        let merged = merge(feed, json);
        assert_eq!(merged[0].title.as_deref(), Some("Begrüßung"));
        assert_eq!(merged[0].url.as_deref(), Some("https://example.org"));
    }

    #[test]
    fn the_longer_list_wins() {
        let feed = vec![
            chapter(0, Some("A"), None, ChapterSource::Feed),
            chapter(60_000, Some("B"), None, ChapterSource::Feed),
        ];
        let json = vec![chapter(0, Some("Only"), None, ChapterSource::Json)];
        assert_eq!(merge(feed.clone(), json), feed);
    }
}
