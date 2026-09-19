//! Drawing. The frame around every screen — brand bar, menu, key hints — is
//! here; each section draws its own content into what is left.

mod discover;
mod downloads;
mod episode;
mod fresh;
mod help;
mod player;
mod podcast;
mod settings;
mod subscriptions;
mod upnext;

use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Paragraph, Wrap};

use crate::app::{App, MENU_WIDTH, MIN_HEIGHT, MIN_WIDTH, Section, Tab};
use crate::text::fit;
use crate::theme;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
/// From this width on a list gets a preview beside it.
const WIDE: u16 = 100;

pub fn draw(frame: &mut Frame<'_>, app: &App) {
    let area = frame.area();
    if area.width < MIN_WIDTH || area.height < MIN_HEIGHT {
        draw_too_small(frame, area, app);
        return;
    }
    let [bar, body, hints] =
        Layout::vertical([Constraint::Length(1), Constraint::Min(0), Constraint::Length(1)]).areas(area);
    let [left, content] = Layout::horizontal([Constraint::Length(MENU_WIDTH), Constraint::Min(0)]).areas(body);
    // The player lives below the menu, in the same column, on every screen.
    let player_rows = if app.shows_mini_player() { app.player_rows() } else { 0 };
    let [menu, mini] = Layout::vertical([Constraint::Min(0), Constraint::Length(player_rows)]).areas(left);

    draw_bar(frame, bar);
    draw_menu(frame, menu, app);
    if let Some(now) = app.playback.now.as_ref().filter(|_| app.shows_mini_player()) {
        player::draw_mini(frame, mini, app, now);
    }
    frame.render_widget(Paragraph::new(key_hints(app)), hints);
    if let Some(now) = app.playback.now.as_ref().filter(|_| app.player_open) {
        player::draw(frame, content, app, now);
        return;
    }
    match app.section {
        Section::Discover => match (&app.episode, &app.podcast) {
            (Some(view), _) => episode::draw(frame, content, app, view),
            (None, Some(view)) => podcast::draw(frame, content, app, view),
            (None, None) => discover::draw(frame, content, app),
        },
        Section::Subscriptions => subscriptions::draw(frame, content, app),
        Section::NewEpisodes => fresh::draw(frame, content, app),
        Section::UpNext => upnext::draw(frame, content, app),
        Section::Downloads => downloads::draw(frame, content, app),
        Section::Settings => settings::draw(frame, content, app),
        Section::Help => help::draw(frame, content, app),
    }
}

/// As btop does it: nothing but the two sizes, each green once it suffices.
fn draw_too_small(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let lang = app.lang;
    let size = |value: u16, needed: u16| {
        let colour = if value >= needed { theme::GREEN } else { theme::ACCENT };
        Span::styled(value.to_string(), Style::new().fg(colour).add_modifier(Modifier::BOLD))
    };
    let enough =
        |value: u16| Span::styled(value.to_string(), Style::new().fg(theme::GREEN).add_modifier(Modifier::BOLD));
    let label = |text: &'static str| Span::styled(format!("{:<10}", lang.t(text)), theme::muted());
    let lines = vec![
        Line::styled(lang.t("The window is too small"), theme::bold()),
        Line::default(),
        Line::from(vec![
            label("Now"),
            Span::raw(format!("{} ", lang.t("Width"))),
            size(area.width, MIN_WIDTH),
            Span::raw(format!("   {} ", lang.t("Height"))),
            size(area.height, MIN_HEIGHT),
        ]),
        Line::from(vec![
            label("Needed"),
            Span::raw(format!("{} ", lang.t("Width"))),
            enough(MIN_WIDTH),
            Span::raw(format!("   {} ", lang.t("Height"))),
            enough(MIN_HEIGHT),
        ]),
        Line::default(),
        Line::styled(lang.t("Make the window larger — TorroCast keeps running."), theme::faint()),
    ];
    let height = lines.len() as u16;
    let top = area.y + area.height.saturating_sub(height) / 2;
    let middle = Rect { x: area.x, y: top, width: area.width, height: height.min(area.height) };
    frame.render_widget(Paragraph::new(lines).alignment(Alignment::Center).wrap(Wrap { trim: true }), middle);
}

fn draw_bar(frame: &mut Frame<'_>, area: Rect) {
    let on_red = Style::new().bg(theme::RED).add_modifier(Modifier::BOLD);
    let version = format!("v{VERSION}  ");
    let wordmark = "  \\_ TORROCAST _/";
    let gap = usize::from(area.width).saturating_sub(wordmark.len() + version.len());
    let line = Line::from(vec![
        Span::styled("  \\_ TORRO", on_red.fg(Color::White)),
        Span::styled("CAST", on_red.fg(theme::SILVER)),
        Span::styled(" _/", on_red.fg(Color::White)),
        Span::styled(" ".repeat(gap), on_red),
        Span::styled(version, Style::new().bg(theme::RED).fg(Color::Rgb(255, 210, 206))),
    ]);
    frame.render_widget(Paragraph::new(line), area);
}

fn draw_menu(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let block = panel(app.lang.t("Menu"), false);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let width = usize::from(inner.width);
    let mut lines = vec![Line::default()];
    for (index, section) in Section::ALL.iter().enumerate() {
        let title = app.lang.t(section.title());
        // What waits in Up Next is counted at its menu entry.
        let waiting = match section {
            Section::UpNext => app.playback.up_next.len(),
            Section::NewEpisodes => app.new_episodes.len(),
            _ => 0,
        };
        let badge = if waiting > 0 { format!(" {waiting} ") } else { String::new() };
        let room = width.saturating_sub(badge.chars().count() + usize::from(!badge.is_empty()));
        let chosen = *section == app.section && !app.player_open;
        let on_red = Style::new().bg(theme::RED).fg(Color::White).add_modifier(Modifier::BOLD);
        let mut spans = if chosen {
            vec![Span::styled(fit(&format!(" {}  {title}", index + 1), room), on_red)]
        } else {
            vec![
                Span::styled(format!(" {}  ", index + 1), theme::faint()),
                Span::raw(fit(title, room.saturating_sub(4))),
            ]
        };
        if !badge.is_empty() {
            spans.push(Span::styled(
                badge,
                Style::new().bg(theme::ACCENT).fg(Color::White).add_modifier(Modifier::BOLD),
            ));
            spans.push(Span::styled(" ", if chosen { on_red } else { Style::new() }));
        }
        lines.push(Line::from(spans));
    }
    frame.render_widget(Paragraph::new(lines), inner);

    if inner.height > 12 && !app.shows_mini_player() {
        let tagline =
            Rect { x: inner.x + 1, y: inner.y + inner.height - 2, width: inner.width.saturating_sub(2), height: 1 };
        frame.render_widget(Paragraph::new(app.lang.t("Podcasts in the terminal.")).style(theme::faint()), tagline);
    }
}

fn key_hints(app: &App) -> Line<'static> {
    let mut hints: Vec<(&'static str, &'static str)> = Vec::new();
    if app.player_open {
        hints.extend([("␣", "pause"), ("b f", "±30 s"), (", .", "chapter"), ("n", "next episode"), ("x", "stop")]);
        hints.extend([("- +", "tempo"), ("t", "sleep timer"), ("enter", "jump"), ("esc", "back")]);
        return hint_line(app, &hints);
    }
    match app.section {
        Section::Discover => match (&app.episode, &app.podcast) {
            (Some(view), _) => {
                if !view.listed_chapters().is_empty() {
                    hints.push(("tab", "notes/chapters"));
                }
                hints.extend([
                    ("p", "play"),
                    ("a", "to the end"),
                    ("A", "to the front"),
                    ("1-9", "open link"),
                    ("esc", "back"),
                ]);
            }
            (None, Some(view)) if view.filtering => hints.extend([("enter", "done"), ("esc", "back")]),
            (None, Some(_)) => {
                hints.extend([("enter", "open"), ("p", "play"), ("a", "to the end"), ("A", "to the front")]);
                hints.extend([("D", "download"), ("s", "subscribe"), ("/", "filter"), ("esc", "back")]);
            }
            (None, None) if app.search.editing && app.tab == Tab::Search => {
                hints.extend([
                    ("enter", "search now"),
                    ("tab", "tab"),
                    ("ctrl+e", "podcasts/episodes"),
                    ("esc", "leave input"),
                ]);
            }
            (None, None) if app.tab == Tab::Search && app.search.episodes_mode => {
                hints.extend([("enter", "open"), ("p", "play"), ("a", "to the end"), ("A", "to the front")]);
                hints.extend([("e", "podcasts"), ("/", "search"), ("tab", "tab")]);
            }
            (None, None) => {
                hints.extend([("↑↓", "select"), ("enter", "open"), ("/", "search"), ("tab", "tab")]);
                if app.tab == Tab::Search {
                    hints.push(("e", "episodes"));
                }
                if app.tab == Tab::Charts && app.charts.category.is_some() {
                    hints.push(("esc", "all charts"));
                }
                hints.extend([("1-7", "menu"), ("q", "quit")]);
            }
        },
        Section::Subscriptions => hints.extend([("↑↓", "select"), ("enter", "open"), ("1-7", "menu"), ("q", "quit")]),
        Section::NewEpisodes => {
            hints.extend([
                ("enter", "open"),
                ("p", "play"),
                ("a", "to the end"),
                ("A", "to the front"),
                ("r", "reload"),
            ]);
            hints.push(("1-7", "menu"));
        }
        Section::UpNext => {
            hints.extend([
                ("↑↓", "select"),
                ("J K", "move"),
                ("d", "remove"),
                ("p", "play"),
                ("C", "empty"),
                ("1-7", "menu"),
            ]);
        }
        Section::Downloads => {
            hints.extend([
                ("enter", "play"),
                ("a", "to the end"),
                ("A", "to the front"),
                ("d", "delete"),
                ("1-7", "menu"),
            ]);
        }
        Section::Settings => {
            hints.extend([("↑↓", "select"), ("enter", "on/off"), ("←→", "change"), ("1-7", "menu"), ("q", "quit")])
        }
        Section::Help => hints.extend([("esc", "back"), ("1-7", "menu"), ("q", "quit")]),
    }
    // While something plays, the way to its keys closes every hint line.
    if app.playback.now.is_some() && !app.is_typing() {
        hints.extend([("␣", "pause"), ("0", "player")]);
    }
    hint_line(app, &hints)
}

fn hint_line(app: &App, hints: &[(&'static str, &'static str)]) -> Line<'static> {
    let mut spans = vec![Span::raw(" ")];
    for (key, label) in hints {
        spans.push(Span::styled(format!(" {key} "), Style::new().bg(theme::KEY).add_modifier(Modifier::BOLD)));
        spans.push(Span::styled(format!(" {}  ", app.lang.t(label)), theme::muted()));
    }
    Line::from(spans)
}

/// A rounded panel. The one that has the user's attention wears the accent.
pub(crate) fn panel(title: &str, focused: bool) -> Block<'static> {
    let border = if focused { theme::ACCENT } else { theme::LINE };
    Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::new().fg(border))
        .title(Line::styled(format!(" {title} "), theme::bold()))
}

/// A label and its value on one line, the value starting at a fixed column.
pub(crate) fn field(label: &str, value: Span<'static>) -> Line<'static> {
    Line::from(vec![Span::styled(format!("{label:<14}"), theme::muted()), value])
}

/// Tabs as words; the active one bold with an accent rule beneath it.
pub(crate) fn tab_lines(titles: &[&str], active: usize) -> [Line<'static>; 2] {
    let mut names = vec![Span::raw("  ")];
    let mut rules = vec![Span::raw("  ")];
    for (index, title) in titles.iter().enumerate() {
        let width = title.chars().count();
        if index == active {
            names.push(Span::styled((*title).to_owned(), theme::bold()));
            rules.push(Span::styled("━".repeat(width), Style::new().fg(theme::ACCENT)));
        } else {
            names.push(Span::styled((*title).to_owned(), theme::muted()));
            rules.push(Span::raw(" ".repeat(width)));
        }
        names.push(Span::raw("    "));
        rules.push(Span::raw("    "));
    }
    [Line::from(names), Line::from(rules)]
}

/// A sentence in the middle of an otherwise empty panel.
pub(crate) fn empty(frame: &mut Frame<'_>, area: Rect, sentences: &[&str]) {
    let lines: Vec<Line<'_>> =
        sentences.iter().map(|sentence| Line::styled((*sentence).to_owned(), theme::faint())).collect();
    let inner = Rect {
        x: area.x + 1,
        y: area.y + 1,
        width: area.width.saturating_sub(2),
        height: area.height.saturating_sub(1),
    };
    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: true }), inner);
}

pub(crate) fn is_wide(area: Rect) -> bool {
    area.width + MENU_WIDTH >= WIDE
}
