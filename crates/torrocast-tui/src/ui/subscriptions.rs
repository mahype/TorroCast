//! The podcasts the user follows.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::Line;
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
    let inner = block.inner(area);
    frame.render_widget(block, area);
    if subscriptions.is_empty() {
        let sentences = [
            lang.t("No subscriptions yet. Open a podcast and press s."),
            "",
            lang.t("They are kept in the library folder and appear on every device that shares it."),
        ];
        empty(frame, inner, &sentences);
        return;
    }
    let (width, height) = (usize::from(inner.width), usize::from(inner.height));
    let start = window(app.subscriptions_index, subscriptions.len(), height, 1);
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
