//! Every key, in one place.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use super::panel;
use crate::app::App;
use crate::theme;

const KEYS: &[(&str, &[(&str, &str)])] = &[
    (
        "Everywhere",
        &[
            ("↑ ↓  j k", "move the selection"),
            ("PgUp PgDn  g G", "page, first, last"),
            ("enter  →  l", "open the selected entry"),
            ("esc  ←  h", "one level back"),
            ("tab", "switch tab or panel"),
            ("/", "search, or filter a list"),
            ("1 2 3", "menu (in an episode the digits open links)"),
            ("r", "reload"),
            ("q  ctrl+c", "quit"),
        ],
    ),
    (
        "In lists",
        &[
            ("o", "reverse the order"),
            ("w", "open the website in the browser"),
            ("m", "unfold the description"),
        ],
    ),
    (
        "In an episode",
        &[
            ("1 … 9", "open the numbered link"),
            ("enter", "open the chapter's link"),
        ],
    ),
];

pub fn draw(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let lang = app.lang;
    let block = panel(&format!("{} › {}", lang.t("Help"), lang.t("Keys")), true);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let mut lines = Vec::new();
    for (group, keys) in KEYS {
        lines.push(Line::default());
        lines.push(Line::styled(format!(" {}", lang.t(group)), theme::heading()));
        for (key, meaning) in *keys {
            lines.push(Line::from(vec![
                Span::styled(format!("   {key:<18}"), Style::new().add_modifier(Modifier::BOLD)),
                Span::styled(lang.t(meaning), theme::muted()),
            ]));
        }
    }
    lines.push(Line::default());
    lines.push(Line::styled(
        format!(
            " {}",
            lang.t("The mouse works too: the wheel scrolls, a click picks a menu entry.")
        ),
        theme::faint(),
    ));
    frame.render_widget(Paragraph::new(lines), inner);
}
