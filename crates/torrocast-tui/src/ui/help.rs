//! Every key, in one place.

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use super::panel;
use crate::app::App;
use crate::theme;

type Group = (&'static str, &'static [(&'static str, &'static str)]);

const LEFT: &[Group] = &[
    (
        "Everywhere",
        &[
            ("↑ ↓  j k", "move the selection"),
            ("PgUp PgDn  g G", "page, first, last"),
            ("enter  →  l", "open the selected entry"),
            ("esc  ←  h", "one level back"),
            ("tab", "switch tab or panel"),
            ("/", "search, or filter a list"),
            ("1 2 3 4", "menu (in an episode the digits open links)"),
            ("r", "reload"),
            ("q  ctrl+c", "quit"),
        ],
    ),
    (
        "In lists",
        &[("o", "reverse the order"), ("w", "open the website in the browser"), ("m", "unfold the description")],
    ),
    ("In an episode", &[("1 … 9", "open the numbered link"), ("enter", "open the chapter's link")]),
];

const RIGHT: &[Group] = &[
    (
        "Playback",
        &[
            ("space", "pause and resume"),
            ("x", "stop: remember the place, close the player"),
            (",  .", "previous and next chapter"),
            ("n", "next episode from Up Next"),
            ("b  f", "30 seconds back and forward"),
            ("-  +", "slower and faster, at the same pitch"),
            ("0", "open and close the large player"),
        ],
    ),
    (
        "On an episode",
        &[
            ("p", "play now; what was playing moves to Up Next"),
            ("a", "to the end of Up Next"),
            ("A", "to the front of Up Next"),
            ("e", "search for episodes instead of podcasts"),
        ],
    ),
    (
        "In Up Next",
        &[("J  K", "move the episode down and up"), ("d", "take the episode out"), ("C", "empty the list (asks once)")],
    ),
];

fn column(app: &App, groups: &[Group]) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    for (group, keys) in groups {
        lines.push(Line::default());
        lines.push(Line::styled(format!(" {}", app.lang.t(group)), theme::heading()));
        for (key, meaning) in *keys {
            lines.push(Line::from(vec![
                Span::styled(format!("   {key:<16}"), Style::new().add_modifier(Modifier::BOLD)),
                Span::styled(app.lang.t(meaning), theme::muted()),
            ]));
        }
    }
    lines
}

pub fn draw(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let lang = app.lang;
    let block = panel(&format!("{} › {}", lang.t("Help"), lang.t("Keys")), true);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let [keys, foot] = Layout::vertical([Constraint::Min(0), Constraint::Length(2)]).areas(inner);
    // Two columns where they fit; otherwise navigation first, then playback below it.
    if keys.width >= 110 {
        let [left, right] = Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)]).areas(keys);
        frame.render_widget(Paragraph::new(column(app, LEFT)), left);
        frame.render_widget(Paragraph::new(column(app, RIGHT)), right);
    } else {
        let mut lines = column(app, RIGHT);
        lines.extend(column(app, LEFT));
        frame.render_widget(Paragraph::new(lines), keys);
    }
    let note = lang.t("The mouse works too: the wheel scrolls, a click picks a menu entry or a button of the player.");
    frame.render_widget(Paragraph::new(format!(" {note}")).style(theme::faint()), foot);
}
