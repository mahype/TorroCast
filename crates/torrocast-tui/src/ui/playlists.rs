//! The user's own playlists, and the question "into which one?".

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Clear, Paragraph};

use super::discover::queue_label;
use super::{empty, panel};
use crate::app::App;
use crate::text::{duration, fit, window};
use crate::theme;

fn clock(milliseconds: u64) -> String {
    duration(u32::try_from(milliseconds / 1000).unwrap_or(u32::MAX))
}

fn name_input(app: &App, width: usize) -> Vec<Line<'static>> {
    let typed = app.playlist_name.as_ref().map(|(_, name)| name.clone()).unwrap_or_default();
    vec![
        Line::default(),
        Line::styled(format!(" {}", app.lang.t("Name of the new playlist")), theme::heading()),
        Line::from(vec![
            Span::styled(fit(&format!(" {typed}"), width.saturating_sub(1)).trim_end().to_owned(), theme::selected()),
            Span::styled("▏", Style::new().fg(theme::ACCENT)),
        ]),
    ]
}

pub fn draw(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let lang = app.lang;
    if app.playlist_name.is_some() {
        let block = panel(lang.t("Playlists"), true);
        let inner = block.inner(area);
        frame.render_widget(block, area);
        frame.render_widget(Paragraph::new(name_input(app, usize::from(inner.width))), inner);
        return;
    }
    let Some(playlist) = app.opened_playlist() else {
        draw_list(frame, area, app);
        return;
    };

    let total: u64 = playlist.items.iter().filter_map(|item| item.duration_ms).sum();
    let title = format!(
        "{} › {} · {} {} · {}",
        lang.t("Playlists"),
        playlist.name,
        playlist.items.len(),
        lang.t("episodes"),
        clock(total)
    );
    let block = panel(&title, true);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    if playlist.items.is_empty() {
        empty(frame, inner, &[lang.t("This playlist is empty. L on any episode puts it here.")]);
        return;
    }
    let (width, height) = (usize::from(inner.width), usize::from(inner.height));
    let start = window(app.playlist_item, playlist.items.len(), height, 3);
    super::clickable(app, inner, start, 3, playlist.items.len());
    let mut lines = Vec::new();
    for (index, item) in playlist.items.iter().enumerate().skip(start).take(height.div_ceil(3)) {
        let chosen = index == app.playlist_item;
        let background = if chosen { Style::new().bg(theme::SELECTION) } else { Style::new() };
        let (state, state_style) = match queue_label(app, &item.key()) {
            (label, style) if !label.is_empty() => (label, style),
            _ => (item.duration_ms.map(clock).unwrap_or_default(), theme::muted()),
        };
        let title_width = width.saturating_sub(state.chars().count() + 3);
        lines.push(Line::from(vec![
            Span::styled(
                format!(" {}", fit(&item.title, title_width)),
                if chosen { theme::selected() } else { Style::new() },
            ),
            Span::styled(format!(" {state} "), state_style.patch(background)),
        ]));
        lines.push(Line::styled(fit(&format!(" {}", item.podcast), width), background.fg(theme::MUTED)));
        lines.push(Line::default());
    }
    frame.render_widget(Paragraph::new(lines), inner);
}

fn draw_list(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let lang = app.lang;
    let playlists = &app.playlists;
    let title = if playlists.is_empty() {
        lang.t("Playlists").to_owned()
    } else {
        format!("{} · {}", lang.t("Playlists"), playlists.len())
    };
    let block = panel(&title, true);
    let mut inner = block.inner(area);
    frame.render_widget(block, area);
    if playlists.is_empty() {
        empty(frame, inner, &[lang.t("No playlists yet. N makes one; L on any episode puts it into a playlist.")]);
        return;
    }
    if let Some(notice) = &app.notice {
        let row =
            Rect { x: inner.x + 1, y: inner.y + inner.height - 1, width: inner.width.saturating_sub(2), height: 1 };
        frame.render_widget(Paragraph::new(notice.clone()).style(Style::new().fg(theme::AMBER)), row);
        inner.height = inner.height.saturating_sub(2);
    }
    let (width, height) = (usize::from(inner.width), usize::from(inner.height));
    let start = window(app.playlists_index, playlists.len(), height, 1);
    super::clickable(app, inner, start, 1, playlists.len());
    let lines: Vec<Line<'_>> = playlists
        .iter()
        .enumerate()
        .skip(start)
        .take(height)
        .map(|(index, playlist)| {
            let chosen = index == app.playlists_index;
            let background = if chosen { Style::new().bg(theme::SELECTION) } else { Style::new() };
            let total: u64 = playlist.items.iter().filter_map(|item| item.duration_ms).sum();
            let facts = format!("{} {} · {}", playlist.items.len(), lang.t("episodes"), clock(total));
            Line::from(vec![
                Span::styled(
                    fit(&format!(" {}", playlist.name), width.saturating_sub(facts.chars().count() + 2)),
                    if chosen { theme::selected() } else { Style::new() },
                ),
                Span::styled(format!("{facts}  "), background.fg(theme::MUTED)),
            ])
        })
        .collect();
    frame.render_widget(Paragraph::new(lines), inner);
}

/// "Into which playlist?" — a small box over the screen the episode was chosen on.
pub fn draw_picker(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let lang = app.lang;
    let rows = (app.playlists.len() as u16 + 6).min(area.height);
    let width = area.width.min(64);
    let place =
        Rect { x: area.x + (area.width - width) / 2, y: area.y + (area.height - rows) / 3, width, height: rows };
    frame.render_widget(Clear, place);
    let block = panel(lang.t("Into which playlist?"), true);
    let inner = block.inner(place);
    frame.render_widget(block, place);
    let inner_width = usize::from(inner.width);
    if app.playlist_name.is_some() {
        frame.render_widget(Paragraph::new(name_input(app, inner_width)), inner);
        return;
    }
    let Some((item, chosen)) = &app.picker else { return };
    let mut lines = vec![
        Line::styled(format!(" {}", fit(&item.title, inner_width.saturating_sub(2))), theme::muted()),
        Line::default(),
    ];
    let names = app
        .playlists
        .iter()
        .map(|playlist| playlist.name.clone())
        .chain(std::iter::once(lang.t("A new playlist …").to_owned()));
    for (index, name) in names.enumerate() {
        let text = fit(&format!(" {name}"), inner_width);
        lines.push(if index == *chosen { Line::styled(text, theme::selected()) } else { Line::raw(text) });
    }
    frame.render_widget(Paragraph::new(lines), inner);
}
