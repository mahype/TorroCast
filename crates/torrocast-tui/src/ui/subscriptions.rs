//! The podcasts the user follows.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use super::{empty, panel};
use crate::app::App;
use crate::text::{fit, window};
use crate::theme;

pub fn draw(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let lang = app.lang;
    let subscriptions = &app.subscriptions;
    let title = if subscriptions.is_empty() {
        lang.t("Subscriptions").to_owned()
    } else {
        format!("{} · {}", lang.t("Subscriptions"), subscriptions.len())
    };
    let block = panel(&title, true);
    let mut inner = block.inner(area);
    frame.render_widget(block, area);
    if let Some((import, path)) = &app.opml_path {
        let question =
            if *import { "OPML file to import subscriptions from" } else { "OPML file to write the subscriptions to" };
        let lines = vec![
            Line::default(),
            Line::styled(format!(" {}", lang.t(question)), theme::heading()),
            Line::from(vec![
                Span::styled(format!(" {path}"), theme::selected()),
                Span::styled("▏", Style::new().fg(theme::ACCENT)),
            ]),
            Line::default(),
            Line::styled(
                format!(" {}", lang.t("~ is your home folder. Every podcast client can write and read such a file.")),
                theme::muted(),
            ),
        ];
        frame.render_widget(Paragraph::new(lines), inner);
        return;
    }
    // What an import or export came to is said in the last row.
    if let Some(notice) = &app.notice {
        let row = Rect {
            x: inner.x + 1,
            y: inner.y + inner.height.saturating_sub(1),
            width: inner.width.saturating_sub(2),
            height: 1,
        };
        frame.render_widget(Paragraph::new(notice.clone()).style(Style::new().fg(theme::SILVER)), row);
        inner.height = inner.height.saturating_sub(2);
    }
    if subscriptions.is_empty() {
        let sentences = [
            lang.t("No subscriptions yet. Open a podcast and press s — or press I to bring them along from another client."),
            "",
            lang.t("They are kept in the library folder and appear on every device that shares it."),
        ];
        empty(frame, inner, &sentences);
        return;
    }
    let (width, height) = (usize::from(inner.width), usize::from(inner.height));
    let start = window(app.subscriptions_index, subscriptions.len(), height, 1);
    super::clickable(app, inner, start, 1, subscriptions.len());
    let lines: Vec<Line<'_>> = subscriptions
        .iter()
        .enumerate()
        .skip(start)
        .take(height)
        .map(|(index, subscription)| {
            let text = fit(&format!(" {}", subscription.title), width);
            if index == app.subscriptions_index {
                Line::styled(text, theme::selected())
            } else {
                Line::styled(text, Style::new())
            }
        })
        .collect();
    frame.render_widget(Paragraph::new(lines), inner);
}
