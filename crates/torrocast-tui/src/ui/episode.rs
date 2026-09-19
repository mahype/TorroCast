//! One episode: its show notes and, where there are any, its chapters.

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use torrocast_core::ChapterSource;

use super::{empty, is_wide, panel};
use crate::app::{App, EpisodeView, Focus, Looking};
use crate::text::{date, duration, fit, notes, timestamp, window};
use crate::theme;

pub fn draw(frame: &mut Frame<'_>, area: Rect, app: &App, view: &EpisodeView) {
    let lang = app.lang;
    let episode = view.episode();
    let [head, body] = Layout::vertical([Constraint::Length(4), Constraint::Min(0)]).areas(area);

    let path = format!(
        "{} › {} › {}",
        lang.t("Discover"),
        fit(&view.podcast.title, 28).trim_end(),
        fit(&episode.title, 28).trim_end()
    );
    let block = panel(&path, false);
    let inner = block.inner(head);
    frame.render_widget(block, head);
    let mut facts = Vec::new();
    facts.extend(episode.published.map(|moment| date(lang, moment)));
    facts.extend(episode.duration_seconds.map(duration));
    match (episode.season, episode.number) {
        (Some(season), Some(number)) => {
            facts.push(format!("{} {season}, {} {number}", lang.t("Season"), lang.t("Episode")))
        }
        (None, Some(number)) => facts.push(format!("{} {number}", lang.t("Episode"))),
        _ => {}
    }
    let width = usize::from(inner.width.saturating_sub(2));
    let lines = vec![
        Line::styled(fit(&episode.title, width).trim_end().to_owned(), theme::bold()),
        Line::styled(facts.join(" · "), theme::muted()),
    ];
    frame.render_widget(Paragraph::new(lines), Rect { x: inner.x + 1, width: inner.width.saturating_sub(2), ..inner });

    let has_chapters = !view.listed_chapters().is_empty() || view.looking.is_some();
    let (notes_area, chapters_area) = match (has_chapters, is_wide(area)) {
        (false, _) => (Some(body), None),
        (true, true) => {
            // Chapter titles are sentences; give them nearly half, within reason.
            let chapters_width = (body.width * 45 / 100).clamp(36, 64);
            let [left, right] =
                Layout::horizontal([Constraint::Min(0), Constraint::Length(chapters_width)]).areas(body);
            (Some(left), Some(right))
        }
        // No room for both: the panel with the focus gets the space.
        (true, false) if view.focus == Focus::Chapters => (None, Some(body)),
        (true, false) => (Some(body), None),
    };
    if let Some(area) = notes_area {
        draw_notes(frame, area, app, view);
    }
    if let Some(area) = chapters_area {
        draw_chapters(frame, area, app, view);
    }
}

fn draw_notes(frame: &mut Frame<'_>, area: Rect, app: &App, view: &EpisodeView) {
    let lang = app.lang;
    let block = panel(lang.t("Show notes"), view.focus == Focus::Notes);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let inner = Rect { x: inner.x + 1, width: inner.width.saturating_sub(2), ..inner };
    if view.notes.blocks.is_empty() {
        empty(frame, Rect { y: inner.y.saturating_sub(1), ..inner }, &[lang.t("This episode has no show notes.")]);
        return;
    }
    let lines = notes(lang, &view.notes, usize::from(inner.width));
    // The app scrolls without knowing the length; the end is enforced here.
    let furthest = lines.len().saturating_sub(usize::from(inner.height));
    let scroll = usize::from(view.scroll).min(furthest);
    let shown: Vec<Line<'_>> = lines.into_iter().skip(scroll).take(usize::from(inner.height)).collect();
    frame.render_widget(Paragraph::new(shown), inner);
}

fn draw_chapters(frame: &mut Frame<'_>, area: Rect, app: &App, view: &EpisodeView) {
    let lang = app.lang;
    let chapters = view.listed_chapters();
    let title = if chapters.is_empty() {
        lang.t("Chapters").to_owned()
    } else {
        format!("{} · {}", lang.t("Chapters"), chapters.len())
    };
    let block = panel(&title, view.focus == Focus::Chapters);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    if chapters.is_empty() {
        let sentence = match view.looking {
            Some(Looking::AudioFile) => "Looking for chapters in the audio file …",
            Some(Looking::ChaptersFile) => "Loading the chapters …",
            None => "No chapters.",
        };
        empty(frame, inner, &[lang.t(sentence)]);
        return;
    }

    let height = usize::from(inner.height.saturating_sub(2));
    let width = usize::from(inner.width);
    let start = window(view.chapter_index, chapters.len(), height, 1);
    let mut lines: Vec<Line<'_>> = chapters
        .iter()
        .enumerate()
        .skip(start)
        .take(height)
        .map(|(index, chapter)| {
            let chosen = index == view.chapter_index && view.focus == Focus::Chapters;
            let background = if chosen { Style::new().bg(theme::SELECTION) } else { Style::new() };
            let title_width = width.saturating_sub(14);
            Line::from(vec![
                Span::styled(format!(" {}  ", timestamp(chapter.start_ms)), background.fg(theme::MUTED)),
                Span::styled(
                    fit(chapter.title.as_deref().unwrap_or("—"), title_width),
                    if chosen { theme::selected() } else { Style::new() },
                ),
                Span::styled(if chapter.url.is_some() { " ↗ " } else { "   " }, background.fg(theme::CYAN)),
            ])
        })
        .collect();
    while lines.len() <= height {
        lines.push(Line::default());
    }
    let origin = match chapters.first().map(|chapter| chapter.source) {
        Some(ChapterSource::Feed) => "From the feed",
        Some(ChapterSource::Json) => "From the chapters file",
        Some(ChapterSource::AudioFile) | None => "From the audio file",
    };
    lines.push(Line::styled(format!(" {}", lang.t(origin)), theme::faint()));
    frame.render_widget(Paragraph::new(lines), inner);
}
