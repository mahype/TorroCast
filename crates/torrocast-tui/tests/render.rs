//! The screens, drawn into a buffer and read back as text.

use std::sync::Arc;
use std::time::{Duration, Instant};

use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use torrocast_core::{
    Chapter, ChapterSource, Command, EpisodeRef, Event, NewEpisode, NowPlaying, PlaybackState, PodcastRef, Problem,
    ProviderId, QueueItem, Settings, Status, Subscription, Transport,
};
use torrocast_tui::app::{App, SEARCH_DELAY, Section};
use torrocast_tui::i18n::Lang;
use torrocast_tui::ui;

const FEED: &str = include_str!("../../torrocast-feed/tests/fixtures/show.xml");

fn app() -> App {
    App::new(Lang::De, Settings::for_locale("de_DE"))
}

fn screen(app: &App, width: u16, height: u16) -> String {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).expect("a test backend always works");
    terminal.draw(|frame| ui::draw(frame, app)).expect("drawing into memory");
    let buffer = terminal.backend().buffer();
    (0..height)
        .map(|y| (0..width).map(|x| buffer[(x, y)].symbol()).collect::<String>().trim_end().to_owned())
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
    let Some(Command::Search { request, query }) = app.commands.pop() else { panic!("the pause starts a search") };
    assert_eq!(query, "beispiel");
    app.on_event(Event::SearchBatch { request, provider: ProviderId::Apple, outcome: Ok(results) });
}

fn opened_podcast(app: &mut App) {
    searched(app, vec![show("Beispielsendung", Some("https://beispiel.example/feed.xml"))]);
    press(app, KeyCode::Enter);
    press(app, KeyCode::Enter);
    let Some(Command::OpenFeed { request, .. }) = app.commands.pop() else { panic!("enter opens the feed") };
    let podcast = torrocast_feed::parse_feed(FEED).expect("the fixture is a feed");
    app.on_event(Event::Feed { request, outcome: Ok(Arc::new(podcast)) });
}

#[test]
fn the_first_screen_invites_a_search() {
    let text = screen(&app(), 104, 28);
    assert!(text.contains("\\_ TORROCAST _/"));
    assert!(text.contains("1  Entdecken"));
    assert!(text.contains("Noch keine Suche."));
    assert!(text.contains("Eingabe verlassen"), "the field has the keyboard from the start");
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
    searched(&mut app, vec![show("Beispielsendung", Some("https://b.example")), show("Nur bei Spotify", None)]);
    let text = screen(&app, 104, 28);
    assert!(text.contains("2 Treffer"));
    assert!(text.contains("Anna Beispiel · Politik"));
    assert!(text.contains("Vorschau"));
    assert!(!screen(&app, 90, 28).contains("Vorschau"), "no room for a preview below 100 columns");
}

#[test]
fn a_late_answer_to_an_old_search_is_ignored() {
    let mut app = app();
    searched(&mut app, vec![show("Beispielsendung", Some("https://b.example"))]);
    app.on_event(Event::SearchBatch { request: 0, provider: ProviderId::Apple, outcome: Ok(vec![show("Alt", None)]) });
    assert_eq!(app.search.results.len(), 1);
    assert_eq!(app.search.results[0].title, "Beispielsendung");
}

#[test]
fn a_failing_directory_is_a_sentence_not_a_code() {
    let mut app = app();
    type_text(&mut app, "beispiel");
    press(&mut app, KeyCode::Enter);
    let Some(Command::Search { request, .. }) = app.commands.pop() else { panic!("enter searches at once") };
    app.on_event(Event::SearchBatch { request, provider: ProviderId::Apple, outcome: Err(Problem::Unreachable) });
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
    let Some(Command::Chapters { request, chapters_url, mp3_url }) = app.commands.pop() else {
        panic!("the feed names a chapters file")
    };
    assert!(chapters_url.is_some());
    assert!(mp3_url.is_none(), "chapters are announced; the audio file is left alone");

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
    app.on_event(Event::Chapters { request, chapters: richer });
    assert!(screen(&app, 104, 28).contains("Kapitel · 3"), "the longer list wins");

    press(&mut app, KeyCode::Char('1'));
    assert_eq!(app.open_urls, vec!["https://beispiel.example/live"]);
    assert_eq!(app.section, Section::Discover, "inside an episode the digits open links, not the menu");
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
    press(&mut app, KeyCode::Char('5'));
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
    assert!(!screen(&app(), 80, 24).contains("zu klein"), "the minimum itself is enough");
}

#[test]
fn english_without_a_german_locale() {
    let app = App::new(Lang::from_locale("en_US.UTF-8"), Settings::for_locale("en_US.UTF-8"));
    let text = screen(&app, 104, 28);
    assert!(text.contains("1  Discover"));
    assert!(text.contains("Nothing searched yet."));
}

// ── playback ─────────────────────────────────────────────────────────────────

fn item(title: &str) -> QueueItem {
    QueueItem {
        title: title.into(),
        podcast: "Beispielsendung".into(),
        feed_url: Some("https://beispiel.example/feed.xml".into()),
        guid: Some(title.into()),
        audio_url: format!("https://cdn.beispiel.example/{title}.mp3"),
        duration_ms: Some(3_600_000),
        chapters: Vec::new(),
        chapters_url: None,
        is_mp3: true,
    }
}

fn chapter(start_ms: u64, title: &str) -> Chapter {
    Chapter { start_ms, title: Some(title.into()), url: None, image: None, hidden: false, source: ChapterSource::Feed }
}

fn playing(app: &mut App, queued: &[&str]) {
    let mut now = item("Haushalt und Rente");
    now.chapters = vec![chapter(0, "Begrüßung"), chapter(600_000, "Haushalt"), chapter(1_800_000, "Rente")];
    app.on_event(Event::Playback(Box::new(PlaybackState {
        now: Some(NowPlaying {
            item: now,
            status: Status::Playing,
            position_ms: 900_000,
            duration_ms: Some(3_600_000),
        }),
        up_next: queued.iter().map(|title| item(title)).collect(),
        speed: 1.3,
        sleep: Some(torrocast_core::Sleep::Minutes(28)),
    })));
}

fn transports(app: &mut App) -> Vec<Transport> {
    app.commands
        .drain(..)
        .filter_map(|command| if let Command::Transport(transport) = command { Some(transport) } else { None })
        .collect()
}

#[test]
fn an_episode_is_queued_from_the_list_it_is_seen_in() {
    let mut app = app();
    opened_podcast(&mut app);
    press(&mut app, KeyCode::Char('a'));
    press(&mut app, KeyCode::Char('A'));
    press(&mut app, KeyCode::Char('p'));
    let sent = transports(&mut app);
    assert!(matches!(&sent[0], Transport::Enqueue { item, first: false } if item.title == "BS 012 · Haushalt & Rente"));
    assert!(matches!(&sent[1], Transport::Enqueue { first: true, .. }));
    assert!(matches!(&sent[2], Transport::PlayNow(item) if item.chapters.len() == 2 && item.is_mp3));
    assert!(app.podcast.is_some(), "the selection stays where it was, to collect more");

    app.on_event(Event::Queued { title: "BS 012".into(), place: Some(0) });
    assert!(screen(&app, 104, 28).contains("„BS 012“ liegt jetzt am Anfang von Als Nächstes."));
}

#[test]
fn the_player_sits_under_the_menu_on_every_screen() {
    let mut app = app();
    press(&mut app, KeyCode::Esc);
    assert!(!screen(&app, 104, 32).contains("Läuft gerade"), "nothing plays, nothing shown");
    app.terminal_height = 32;
    playing(&mut app, &["Eins", "Zwei"]);
    for key in ['1', '2', '3', '4', '5', '6'] {
        press(&mut app, KeyCode::Char(key));
        let text = screen(&app, 104, 32);
        assert!(text.contains("0  Läuft gerade"), "missing on screen {key}");
        assert!(text.contains("Haushalt und Rente"));
        assert!(text.contains("Kapitel 2  Haushalt"));
        assert!(text.contains("◀◀") && text.contains("▶▮"));
        assert!(text.contains("1,3×"));
        assert!(text.contains("⏾ 28′"), "the sleep timer counts down in the player");
    }
    assert!(
        screen(&app, 104, 32).contains("Als Nächstes") && screen(&app, 104, 32).contains(" 2 "),
        "the queue is counted at its menu entry"
    );
}

#[test]
fn a_low_window_drops_the_meter_first() {
    let mut app = app();
    press(&mut app, KeyCode::Esc);
    playing(&mut app, &[]);
    app.on_event(Event::Level(0.25));
    app.terminal_height = 32;
    assert!(screen(&app, 104, 32).contains('█'));
    app.terminal_height = 24;
    let low = screen(&app, 104, 24);
    assert!(!low.contains('█'));
    assert!(low.contains("◀◀"), "the buttons stay");
}

#[test]
fn the_playback_keys_work_everywhere_but_not_while_typing() {
    let mut app = app();
    playing(&mut app, &["Eins"]);
    type_text(&mut app, "n x");
    assert!(transports(&mut app).is_empty(), "in the search field these are letters");
    assert_eq!(app.search.input, "n x");

    press(&mut app, KeyCode::Esc);
    for key in [' ', ',', '.', 'n', 'b', 'f', '+', '-', 't', 'x'] {
        press(&mut app, KeyCode::Char(key));
    }
    assert_eq!(
        transports(&mut app),
        vec![
            Transport::Toggle,
            Transport::PreviousChapter,
            Transport::NextChapter,
            Transport::NextEpisode,
            Transport::SeekBy(-30_000),
            Transport::SeekBy(30_000),
            Transport::SpeedBy(0.1),
            Transport::SpeedBy(-0.1),
            Transport::CycleSleep,
            Transport::Stop,
        ]
    );
}

#[test]
fn without_playback_the_space_bar_still_serves_the_settings() {
    let mut app = app();
    press(&mut app, KeyCode::Esc);
    press(&mut app, KeyCode::Char('5'));
    press(&mut app, KeyCode::Down);
    press(&mut app, KeyCode::Down);
    press(&mut app, KeyCode::Char(' '));
    assert!(app.settings.sources.fyyd);
    assert!(transports(&mut app).is_empty());
}

#[test]
fn up_next_is_reordered_and_emptied_with_care() {
    let mut app = app();
    press(&mut app, KeyCode::Esc);
    playing(&mut app, &["Eins", "Zwei", "Drei"]);
    press(&mut app, KeyCode::Char('4'));
    let text = screen(&app, 104, 32);
    assert!(text.contains("Als Nächstes · 3 Folgen · 3:00:00"));
    assert!(text.contains("Danach geht es ohne Pause mit Platz 1 weiter."));

    press(&mut app, KeyCode::Char('J'));
    press(&mut app, KeyCode::Char('d'));
    press(&mut app, KeyCode::Char('C'));
    assert_eq!(transports(&mut app), vec![Transport::Shift { index: 0, down: true }, Transport::Remove(1)]);
    assert!(screen(&app, 104, 32).contains("Drück noch einmal C"));
    press(&mut app, KeyCode::Char('C'));
    assert_eq!(transports(&mut app), vec![Transport::Clear]);
}

#[test]
fn the_large_player_lists_the_chapters_and_jumps_to_them() {
    let mut app = app();
    press(&mut app, KeyCode::Esc);
    playing(&mut app, &[]);
    press(&mut app, KeyCode::Char('0'));
    let text = screen(&app, 104, 32);
    assert!(text.contains("Kapitel 2 / 3"));
    assert!(text.contains("00:15:00"), "the position, with hours");
    assert!(text.contains("-45:00"), "and what is left");
    assert!(text.contains("▶  00:10:00  Haushalt"), "the chapter being heard is marked");
    assert!(!text.contains("0  Läuft gerade"), "the small player steps aside");

    press(&mut app, KeyCode::Down);
    press(&mut app, KeyCode::Enter);
    assert_eq!(transports(&mut app), vec![Transport::SeekTo(1_800_000)]);
    press(&mut app, KeyCode::Esc);
    assert!(!app.player_open);

    // When playback stops, the large player closes with it.
    press(&mut app, KeyCode::Char('0'));
    app.on_event(Event::Playback(Box::new(PlaybackState { speed: 1.0, ..PlaybackState::default() })));
    assert!(!app.player_open);
}

#[test]
fn episodes_can_be_searched_and_queued_without_opening_their_podcast() {
    let mut app = app();
    type_text(&mut app, "rente");
    app.on_key(KeyEvent::new(KeyCode::Char('e'), KeyModifiers::CONTROL));
    let Some(Command::SearchEpisodes { request, query }) = app.commands.pop() else {
        panic!("ctrl+e switches to episodes and searches")
    };
    assert_eq!(query, "rente");
    let found = EpisodeRef {
        title: "Aufstand gegen das Rentenpaket".into(),
        podcast: "Was jetzt?".into(),
        feed_url: Some("https://feeds.example/wj".into()),
        guid: Some("a68b".into()),
        audio_url: "https://cdn.example/wj.mp3?x=1".into(),
        duration_ms: Some(683_000),
        ..EpisodeRef::default()
    };
    app.on_event(Event::EpisodeResults { request, outcome: Ok(vec![found]) });
    press(&mut app, KeyCode::Esc);
    let text = screen(&app, 104, 28);
    assert!(text.contains("1 Folgen"));
    assert!(text.contains("Was jetzt? · 11:23"));

    press(&mut app, KeyCode::Char('A'));
    let sent = transports(&mut app);
    assert!(
        matches!(&sent[0], Transport::Enqueue { item, first: true } if item.is_mp3 && item.podcast == "Was jetzt?")
    );
}

#[test]
fn the_players_buttons_can_be_clicked() {
    let mut app = app();
    press(&mut app, KeyCode::Esc);
    app.terminal_height = 32;
    playing(&mut app, &["Eins"]);
    let text = screen(&app, 104, 32);
    let buttons_row = text.lines().position(|line| line.contains("◀◀")).expect("the buttons are drawn") as u16;
    let click = |column: u16, row: u16| MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column,
        row,
        modifiers: KeyModifiers::NONE,
    };
    app.on_mouse(click(7, buttons_row));
    app.on_mouse(click(22, buttons_row));
    app.on_mouse(click(3, buttons_row + 1));
    assert_eq!(transports(&mut app), vec![Transport::Toggle, Transport::NextEpisode, Transport::PreviousChapter]);
    assert_eq!(app.section, Section::Discover, "a click on the player is not a click on the menu");
}

// ── subscriptions ────────────────────────────────────────────────────────────

#[test]
fn a_podcast_is_subscribed_where_it_is_looked_at() {
    let mut app = app();
    opened_podcast(&mut app);
    press(&mut app, KeyCode::Char('s'));
    let Some(Command::SetSubscribed { feed_url, title, guid, subscribed }) = app.commands.pop() else {
        panic!("s subscribes")
    };
    assert_eq!(
        (feed_url.as_str(), title.as_str(), subscribed),
        ("https://beispiel.example/feed.xml", "Beispielsendung", true)
    );
    assert_eq!(
        guid.as_deref(),
        Some("917393e3-1b1e-5cef-ace4-edaa54e1f810"),
        "the feed's own guid names it in the library"
    );
    assert!(screen(&app, 104, 28).contains("„Beispielsendung“ ist jetzt abonniert."));

    // The core confirms; from then on the podcast shows it, and s ends the subscription.
    app.on_event(Event::Subscriptions(vec![Subscription {
        podcast: "917393e3".into(),
        feed_url: "https://beispiel.example/feed.xml".into(),
        title: "Beispielsendung".into(),
    }]));
    press(&mut app, KeyCode::Down);
    assert!(screen(&app, 104, 28).contains("✓ Abonniert"));
    press(&mut app, KeyCode::Char('s'));
    assert!(matches!(app.commands.pop(), Some(Command::SetSubscribed { subscribed: false, .. })));
}

#[test]
fn subscriptions_open_their_podcast_and_escape_leads_back() {
    let mut app = app();
    press(&mut app, KeyCode::Esc);
    press(&mut app, KeyCode::Char('2'));
    assert!(screen(&app, 104, 28).contains("Noch keine Abos."));

    app.on_event(Event::Subscriptions(vec![Subscription {
        podcast: "p".into(),
        feed_url: "https://beispiel.example/feed.xml".into(),
        title: "Beispielsendung".into(),
    }]));
    assert!(screen(&app, 104, 28).contains("Abos · 1"));
    press(&mut app, KeyCode::Enter);
    assert!(
        matches!(app.commands.pop(), Some(Command::OpenFeed { feed_url, .. }) if feed_url == "https://beispiel.example/feed.xml")
    );
    assert_eq!(app.section, Section::Discover);
    press(&mut app, KeyCode::Esc);
    assert_eq!(app.section, Section::Subscriptions, "back to where the podcast was opened from");
}

#[test]
fn the_settings_say_where_the_library_lives() {
    let mut app = app();
    press(&mut app, KeyCode::Esc);
    press(&mut app, KeyCode::Char('5'));
    app.library = Ok("/home/ada/Dropbox/torrocast".into());
    let text = screen(&app, 104, 30);
    assert!(text.contains("Bibliotheks-Ordner"));
    assert!(text.contains("/home/ada/Dropbox/torrocast"));
    app.library = Err("the library was written by a newer version".into());
    assert!(screen(&app, 104, 30).contains("Die Bibliothek ließ sich nicht öffnen"));
}

// ── new episodes ─────────────────────────────────────────────────────────────

fn new_episode(title: &str) -> NewEpisode {
    let published =
        chrono::DateTime::parse_from_rfc3339("2026-09-18T06:00:00Z").expect("a date").with_timezone(&chrono::Utc);
    NewEpisode { item: item(title), published }
}

#[test]
fn new_episodes_are_listed_counted_and_queued() {
    let mut app = app();
    press(&mut app, KeyCode::Esc);
    press(&mut app, KeyCode::Char('3'));
    assert!(screen(&app, 104, 28).contains("Hier erscheinen neue Folgen deiner Abos."));

    app.on_event(Event::NewEpisodes {
        episodes: vec![new_episode("Eins"), new_episode("Zwei")],
        pending: 3,
        failed: 0,
    });
    let text = screen(&app, 104, 28);
    assert!(text.contains("Neue Folgen · 2 · 3 Feeds laden noch"));
    assert!(text.contains("Beispielsendung · 18.09.2026 · 1:00:00"));
    let menu_row = text.lines().find(|line| line.contains("3  Neue Folgen")).expect("the menu entry");
    assert!(menu_row.contains(" 2 "), "counted at the menu entry: {menu_row}");

    press(&mut app, KeyCode::Down);
    press(&mut app, KeyCode::Char('A'));
    assert!(
        matches!(transports(&mut app).as_slice(), [Transport::Enqueue { item, first: true }] if item.title == "Zwei")
    );
    press(&mut app, KeyCode::Char('r'));
    assert_eq!(app.commands.pop(), Some(Command::RefreshSubscriptions));

    // Enter leads to the episode itself, by way of its podcast.
    press(&mut app, KeyCode::Enter);
    assert!(matches!(app.commands.pop(), Some(Command::OpenFeed { .. })));
    assert_eq!(app.section, Section::Discover);

    app.on_event(Event::NewEpisodes { episodes: Vec::new(), pending: 0, failed: 2 });
    press(&mut app, KeyCode::Esc);
    assert_eq!(app.section, Section::NewEpisodes);
    assert!(screen(&app, 104, 28).contains("2 Feeds haben nicht geantwortet"));
}
