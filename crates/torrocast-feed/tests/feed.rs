use torrocast_feed::notes::{Block, document};
use torrocast_feed::{ChapterSource, FeedError, parse_feed};

const SHOW: &str = include_str!("fixtures/show.xml");

#[test]
fn the_show() {
    let show = parse_feed(SHOW).expect("the fixture is a feed");
    assert_eq!(show.title, "Beispielsendung");
    assert_eq!(show.author.as_deref(), Some("Anna Beispiel & Ben Muster"));
    assert_eq!(
        show.description.as_deref(),
        Some("Jede Woche Politik aus Berlin."),
        "markup does not survive"
    );
    assert_eq!(
        show.website.as_deref(),
        Some("https://beispiel.example"),
        "atom:link is not the website"
    );
    assert_eq!(show.image.as_deref(), Some("https://beispiel.example/cover.jpg"));
    assert_eq!(show.categories, vec!["News", "Politics", "Society & Culture"]);
    assert_eq!(show.guid.as_deref(), Some("917393e3-1b1e-5cef-ace4-edaa54e1f810"));
    assert_eq!(show.funding[0].url, "https://beispiel.example/plus");
    assert_eq!(show.funding[0].label.as_deref(), Some("Unterstützen"));
    assert_eq!(show.persons[0].role.as_deref(), Some("host"));
    assert!(!show.explicit);
    assert_eq!(show.episodes.len(), 2);
}

#[test]
fn an_episode_with_everything() {
    let show = parse_feed(SHOW).expect("the fixture is a feed");
    let episode = &show.episodes[0];
    assert_eq!(episode.guid.as_deref(), Some("bs-012"));
    assert_eq!(
        episode.title, "BS 012 · Haushalt & Rente",
        "titles escaped twice are decoded twice"
    );
    assert_eq!(
        episode.published.map(|date| date.to_rfc3339()).as_deref(),
        Some("2026-09-19T10:33:00+00:00")
    );
    assert_eq!(episode.duration_seconds, Some(5650));
    assert_eq!((episode.season, episode.number), (Some(2), Some(12)));

    let enclosure = episode.enclosure.as_ref().expect("has audio");
    assert!(enclosure.is_mp3());
    assert_eq!(enclosure.bytes, Some(67_890_123));

    assert_eq!(
        episode.chapters.len(),
        2,
        "a chapter without a readable time is dropped"
    );
    assert_eq!(episode.chapters[1].start_ms, 400_000);
    assert_eq!(
        episode.chapters[1].url.as_deref(),
        Some("https://beispiel.example/haushalt")
    );
    assert_eq!(episode.chapters[0].source, ChapterSource::Feed);
    assert_eq!(
        episode.chapters_url.as_deref(),
        Some("https://beispiel.example/012/chapters.json")
    );
    assert_eq!(episode.transcripts[0].mime.as_deref(), Some("text/vtt"));
    assert!(episode.announces_chapters());

    let notes = document(episode.notes_html.as_deref().expect("has notes"));
    assert_eq!(
        notes.links,
        vec!["https://beispiel.example/live"],
        "the full notes win over the teaser"
    );
    assert!(matches!(notes.blocks[1], Block::Heading(_)));
}

#[test]
fn html_entities_do_not_break_the_feed() {
    let show = parse_feed(SHOW).expect("repaired on the second attempt");
    let episode = &show.episodes[1];
    assert_eq!(episode.title, "BS 011 · Sommerpause\u{a0}– Hörerfragen");
    assert_eq!(
        episode.notes_html.as_deref(),
        Some("Fragen & Antworten, R&D inklusive.")
    );
    assert_eq!(episode.duration_seconds, Some(3480));
    let enclosure = episode.enclosure.as_ref().expect("has audio");
    assert!(!enclosure.is_mp3());
    assert_eq!(enclosure.bytes, None, "a length of zero is no length");
    assert!(!episode.announces_chapters());
}

#[test]
fn a_web_page_is_not_a_feed() {
    assert_eq!(parse_feed("<html><body>404</body></html>"), Err(FeedError::NotAFeed));
    assert!(matches!(parse_feed("not xml at all <"), Err(FeedError::Malformed(_))));
}
