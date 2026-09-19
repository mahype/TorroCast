//! The screens, drawn into a buffer and read back as text.

use std::sync::Arc;
use std::time::{Duration, Instant};

use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use torrocast_core::{Chapter, ChapterSource, Command, Event, PodcastRef, Problem, ProviderId, Settings};
use torrocast_tui::app::{App, SEARCH_DELAY, Section};
use torrocast_tui::i18n::Lang;
use torrocast_tui::ui;

const FEED: &str = include_str!("../../torrocast-feed/tests/fixtures/show.xml");

fn app() -> App {
    App::new(Lang::De, Settings::for_locale("de_DE"))
}

fn screen(app: &App, width: u16, height: u16) -> String {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).expect("a test backend always works");
    terminal
        .draw(|frame| ui::draw(frame, app))
        .expect("drawing into memory");
    let buffer = terminal.backend().buffer();
    (0..height)
        .map(|y| {
            (0..width)
                .map(|x| buffer[(x, y)].symbol())
                .collect::<String>()
                .trim_end()
                .to_owned()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn press(app: &mut App, code: KeyCode) {
    app.on_key(KeyEvent::new(code, KeyModifiers::NONE));
}

fn type_text(app: &mut App, text: &str) {
    for character in text.chars() {
        press(app, KeyCode::Char(character));
    }
}

fn show(title: &str, feed: Option<&str>) -> PodcastRef {
    PodcastRef {
        title: title.into(),
        author: Some("Anna Beispiel".into()),
        feed_url: feed.map(str::to_owned),
        genres: vec!["Politik".into()],
        sources: vec![ProviderId::Apple],
        ..PodcastRef::default()
    }
}

/// Types a query, lets the pause pass, and answers the search the app then asks for.
fn searched(app: &mut App, results: Vec<PodcastRef>) {
    type_text(app, "beispiel");
    app.tick(Instant::now() + SEARCH_DELAY + Duration::from_millis(1));
    let Some(Command::Search { request, query }) = app.commands.pop() else {
        panic!("the pause starts a search")
    };
    assert_eq!(query, "beispiel");
    app.on_event(Event::SearchBatch {
        request,
        provider: ProviderId::Apple,
        outcome: Ok(results),
    });
}

fn opened_podcast(app: &mut App) {
    searched(
        app,
        vec![show("Beispielsendung", Some("https://beispiel.example/feed.xml"))],
    );
    press(app, KeyCode::Enter);
    press(app, KeyCode::Enter);
    let Some(Command::OpenFeed { request, .. }) = app.commands.pop() else {
        panic!("enter opens the feed")
    };
    let podcast = torrocast_feed::parse_feed(FEED).expect("the fixture is a feed");
    app.on_event(Event::Feed {
        request,
        outcome: Ok(Arc::new(podcast)),
    });
}

#[test]
fn the_first_screen_invites_a_search() {
    let text = screen(&app(), 104, 28);
    assert!(text.contains("\\_ TORROCAST _/"));
    assert!(text.contains("1  Entdecken"));
    assert!(text.contains("Noch keine Suche."));
    assert!(
        text.contains("Eingabe verlassen"),
        "the field has the keyboard from the start"
    );
}

#[test]
fn typing_searches_only_after_a_pause() {
    let mut app = app();
    type_text(&mut app, "be");
    app.tick(Instant::now() + SEARCH_DELAY * 2);
    assert!(app.commands.is_empty(), "two letters are no query");
    type_text(&mut app, "ispiel");
    app.tick(Instant::now());
    assert!(app.commands.is_empty(), "still typing");
    app.tick(Instant::now() + SEARCH_DELAY + Duration::from_millis(1));
    assert_eq!(app.commands.len(), 1);
    app.tick(Instant::now() + SEARCH_DELAY * 3);
    assert_eq!(app.commands.len(), 1, "the same words are not searched twice");
}

#[test]
fn results_and_preview() {
    let mut app = app();
    searched(
        &mut app,
        vec![
            show("Beispielsendung", Some("https://b.example")),
            show("Nur bei Spotify", None),
        ],
    );
    let text = screen(&app, 104, 28);
    assert!(text.contains("2 Treffer"));
    assert!(text.contains("Anna Beispiel · Politik"));
    assert!(text.contains("Vorschau"));
    assert!(
        !screen(&app, 90, 28).contains("Vorschau"),
        "no room for a preview below 100 columns"
    );
}

#[test]
fn a_late_answer_to_an_old_search_is_ignored() {
    let mut app = app();
    searched(&mut app, vec![show("Beispielsendung", Some("https://b.example"))]);
    app.on_event(Event::SearchBatch {
        request: 0,
        provider: ProviderId::Apple,
        outcome: Ok(vec![show("Alt", None)]),
    });
    assert_eq!(app.search.results.len(), 1);
    assert_eq!(app.search.results[0].title, "Beispielsendung");
}

#[test]
fn a_failing_directory_is_a_sentence_not_a_code() {
    let mut app = app();
    type_text(&mut app, "beispiel");
    press(&mut app, KeyCode::Enter);
    let Some(Command::Search { request, .. }) = app.commands.pop() else {
        panic!("enter searches at once")
    };
    app.on_event(Event::SearchBatch {
        request,
        provider: ProviderId::Apple,
        outcome: Err(Problem::Unreachable),
    });
    let text = screen(&app, 104, 28);
    assert!(text.contains("Keine Verbindung."));
    assert!(!text.contains("503") && !text.contains("HTTP"));
}

#[test]
fn a_show_without_a_feed_says_why_it_cannot_be_opened() {
    let mut app = app();
    searched(&mut app, vec![show("Nur bei Spotify", None)]);
    press(&mut app, KeyCode::Enter);
    press(&mut app, KeyCode::Enter);
    assert!(app.podcast.is_none());
    assert!(app.commands.is_empty());
    // The sentence wraps inside the list; its start is enough to know it is there.
    assert!(screen(&app, 104, 28).contains("Dieser Podcast hat keinen"));
}

#[test]
fn an_address_in_the_search_field_opens_the_feed() {
    let mut app = app();
    type_text(&mut app, "https://beispiel.example/feed.xml");
    press(&mut app, KeyCode::Enter);
    assert!(
        matches!(app.commands.as_slice(), [Command::OpenFeed { feed_url, .. }] if feed_url == "https://beispiel.example/feed.xml")
    );
}

#[test]
fn the_podcast_screen() {
    let mut app = app();
    opened_podcast(&mut app);
    let text = screen(&app, 104, 28);
    assert!(text.contains("Entdecken › Beispielsendung"));
    assert!(text.contains("Anna Beispiel & Ben Muster"));
    assert!(text.contains("♥ Unterstützen"));
    assert!(text.contains("BS 012 · Haushalt & Rente"));
    assert!(text.contains("1:34:10"));
    assert!(text.contains("▤ Kapitel"), "the marks are explained");

    press(&mut app, KeyCode::Char('/'));
    type_text(&mut app, "sommer");
    let filtered = screen(&app, 104, 28);
    assert!(filtered.contains("Filter: sommer"));
    assert!(!filtered.contains("BS 012"));
    assert!(filtered.contains("BS 011"));
}

#[test]
fn the_episode_screen_with_notes_links_and_chapters() {
    let mut app = app();
    opened_podcast(&mut app);
    press(&mut app, KeyCode::Enter);
    let Some(Command::Chapters {
        request,
        chapters_url,
        mp3_url,
    }) = app.commands.pop()
    else {
        panic!("the feed names a chapters file")
    };
    assert!(chapters_url.is_some());
    assert!(
        mp3_url.is_none(),
        "chapters are announced; the audio file is left alone"
    );

    let text = screen(&app, 104, 28);
    assert!(text.contains("Staffel 2, Folge 12"));
    assert!(text.contains("Karten gibt es hier [1]."));
    assert!(text.contains("[1] beispiel.example/live"));
    assert!(text.contains("00:06:40  Haushalt"));
    assert!(text.contains("Aus dem Feed"));

    let extra = Chapter {
        start_ms: 900_000,
        title: Some("Rente".into()),
        url: None,
        image: None,
        hidden: false,
        source: ChapterSource::Json,
    };
    let mut richer: Vec<Chapter> = app.episode.as_ref().expect("an episode is open").chapters.clone();
    richer.push(extra);
    app.on_event(Event::Chapters {
        request,
        chapters: richer,
    });
    assert!(screen(&app, 104, 28).contains("Kapitel · 3"), "the longer list wins");

    press(&mut app, KeyCode::Char('1'));
    assert_eq!(app.open_urls, vec!["https://beispiel.example/live"]);
    assert_eq!(
        app.section,
        Section::Discover,
        "inside an episode the digits open links, not the menu"
    );
}

#[test]
fn escape_walks_back_one_level_at_a_time() {
    let mut app = app();
    opened_podcast(&mut app);
    press(&mut app, KeyCode::Enter);
    assert!(app.episode.is_some());
    press(&mut app, KeyCode::Esc);
    assert!(app.episode.is_none() && app.podcast.is_some());
    press(&mut app, KeyCode::Esc);
    assert!(app.podcast.is_none());
    assert_eq!(app.search.results.len(), 1, "the results are still there");
}

#[test]
fn sources_are_switched_in_the_settings() {
    let mut app = app();
    press(&mut app, KeyCode::Esc);
    press(&mut app, KeyCode::Char('2'));
    assert!(screen(&app, 104, 28).contains("Ausgeschaltet"));
    press(&mut app, KeyCode::Down);
    press(&mut app, KeyCode::Down);
    press(&mut app, KeyCode::Char(' '));
    assert!(app.settings.sources.fyyd && app.settings_changed);
    press(&mut app, KeyCode::Down);
    press(&mut app, KeyCode::Right);
    assert_eq!(app.settings.country, "at");
    assert!(screen(&app, 104, 28).contains("‹ Österreich ›"));
}

#[test]
fn a_small_window_shows_only_the_two_sizes() {
    let text = screen(&app(), 70, 20);
    assert!(text.contains("Das Fenster ist zu klein"));
    assert!(text.contains("Breite 70") && text.contains("Höhe 20"));
    assert!(text.contains("Breite 80") && text.contains("Höhe 24"));
    assert!(!text.contains("TORROCAST"), "nothing else is drawn");
    assert!(
        !screen(&app(), 80, 24).contains("zu klein"),
        "the minimum itself is enough"
    );
}

#[test]
fn english_without_a_german_locale() {
    let app = App::new(Lang::from_locale("en_US.UTF-8"), Settings::for_locale("en_US.UTF-8"));
    let text = screen(&app, 104, 28);
    assert!(text.contains("1  Discover"));
    assert!(text.contains("Nothing searched yet."));
}
