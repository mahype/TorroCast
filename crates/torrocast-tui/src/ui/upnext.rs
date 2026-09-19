//! Up Next: what plays after what is playing.

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use super::{empty, panel};
use crate::app::App;
use crate::text::{duration, fit, window};
use crate::theme;

fn clock(milliseconds: u64) -> String {
    duration(u32::try_from(milliseconds / 1000).unwrap_or(u32::MAX))
}

pub fn draw(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let lang = app.lang;
    let [head, list] = Layout::vertical([Constraint::Length(5), Constraint::Min(0)]).areas(area);

    let block = panel(lang.t("Now playing"), false);
    let inner = block.inner(head);
    frame.render_widget(block, head);
    match &app.playback.now {
        Some(now) => {
            let width = usize::from(inner.width.saturating_sub(4));
            let remaining = now
                .duration_ms
                .map(|total| format!(" · {} {}", lang.t("left"), clock(total.saturating_sub(now.position_ms))));
            let follows = if app.playback.up_next.is_empty() {
                "After this the player falls silent."
            } else {
                "After this, number 1 follows without a pause."
            };
            let lines = vec![
                Line::from(vec![
                    Span::styled(
                        if now.status == torrocast_core::Status::Playing { " ▶ " } else { " ▮▮" },
                        Style::new().fg(theme::ACCENT),
                    ),
                    Span::styled(fit(&now.item.title, width).trim_end().to_owned(), theme::bold()),
                ]),
                Line::styled(format!("   {}{}", now.item.podcast, remaining.unwrap_or_default()), theme::muted()),
                Line::styled(format!("   {}", lang.t(follows)), theme::faint()),
            ];
            frame.render_widget(Paragraph::new(lines), inner);
        }
        None => empty(frame, inner, &[lang.t("Nothing is playing. Press p on an episode.")]),
    }

    let queue = &app.playback.up_next;
    let total: u64 = queue.iter().filter_map(|item| item.duration_ms).sum();
    let title = if queue.is_empty() {
        lang.t("Up Next").to_owned()
    } else {
        format!("{} · {} {} · {}", lang.t("Up Next"), queue.len(), lang.t("episodes"), clock(total))
    };
    let block = panel(&title, true);
    let mut inner = block.inner(list);
    frame.render_widget(block, list);
    if queue.is_empty() {
        empty(
            frame,
            inner,
            &[lang.t("Nothing queued. In any list of episodes, a puts one at the end and A at the front.")],
        );
        return;
    }
    if let Some(notice) = &app.notice {
        let row =
            Rect { x: inner.x + 1, y: inner.y + inner.height - 1, width: inner.width.saturating_sub(2), height: 1 };
        frame.render_widget(Paragraph::new(notice.clone()).style(Style::new().fg(theme::AMBER)), row);
        inner.height = inner.height.saturating_sub(2);
    }

    let (width, height) = (usize::from(inner.width), usize::from(inner.height));
    let start = window(app.up_next_index, queue.len(), height, 3);
    let mut lines = Vec::new();
    for (index, item) in queue.iter().enumerate().skip(start).take(height.div_ceil(3)) {
        let chosen = index == app.up_next_index;
        let background = if chosen { Style::new().bg(theme::SELECTION) } else { Style::new() };
        let length = item.duration_ms.map(clock).unwrap_or_default();
        let title_width = width.saturating_sub(length.chars().count() + 8);
        lines.push(Line::from(vec![
            Span::styled(format!(" {:<3} ", index + 1), background.fg(theme::FAINT)),
            Span::styled(fit(&item.title, title_width), if chosen { theme::selected() } else { Style::new() }),
            Span::styled(format!(" {length}  "), background.fg(theme::MUTED)),
        ]));
        lines.push(Line::styled(fit(&format!("     {}", item.podcast), width), background.fg(theme::MUTED)));
        lines.push(Line::default());
    }
    frame.render_widget(Paragraph::new(lines), inner);
}
