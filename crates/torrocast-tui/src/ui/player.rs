//! The player: small at the foot of the menu column on every screen, large
//! over the content when asked for.

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use torrocast_core::{NowPlaying, Sleep, Status};

use super::{empty, panel};
use crate::app::{App, LEVELS, PLAYER_ROWS};
use crate::text::{duration, fit, timestamp, window};
use crate::theme;

const BARS: [char; 9] = [' ', '▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];
/// The red of the meter's lower half, a step below the accent.
const EMBER: Color = Color::Rgb(168, 50, 45);
/// Speech sits around an RMS of 0.1 to 0.2; this makes that fill the meter.
const METER_GAIN: f32 = 4.0;

fn clock(milliseconds: u64) -> String {
    duration(u32::try_from(milliseconds / 1000).unwrap_or(u32::MAX))
}

/// The level meter: one bar per moment, the newest on the right, two rows tall.
fn meter(app: &App, quiet: bool) -> [Line<'static>; 2] {
    let (mut upper, mut lower) = (String::new(), String::new());
    for slot in 0..LEVELS {
        let level = (slot + app.levels.len())
            .checked_sub(LEVELS)
            .and_then(|index| app.levels.get(index))
            .copied()
            .unwrap_or(0.0);
        let eighths = ((level * METER_GAIN).min(1.0) * 16.0).round() as usize;
        upper.push(BARS[eighths.saturating_sub(8)]);
        lower.push(BARS[eighths.clamp(1, 8)]);
    }
    let (high, low) = if quiet { (theme::FAINT, theme::FAINT) } else { (theme::ACCENT, EMBER) };
    [Line::styled(format!(" {upper}"), Style::new().fg(high)), Line::styled(format!(" {lower}"), Style::new().fg(low))]
}

/// Played and unplayed as a line, chapter starts as small gaps in it.
fn progress(now: &NowPlaying, width: usize, with_chapters: bool) -> Line<'static> {
    let total = now.duration_ms.unwrap_or(0).max(1);
    let cell =
        |milliseconds: u64| (milliseconds.min(total) as usize * width / total as usize).min(width.saturating_sub(1));
    let head = cell(now.position_ms);
    // In the narrow bar of the small player the marks would be most of the line.
    let marks: Vec<usize> = if with_chapters {
        now.chapters().iter().skip(1).map(|chapter| cell(chapter.start_ms)).collect()
    } else {
        Vec::new()
    };
    let spans = (0..width)
        .map(|at| {
            let colour = if at <= head && now.duration_ms.is_some() { theme::ACCENT } else { theme::LINE };
            let glyph = if at == head && now.duration_ms.is_some() {
                "●"
            } else if marks.contains(&at) {
                "╍"
            } else {
                "━"
            };
            Span::styled(glyph, Style::new().fg(colour))
        })
        .collect::<Vec<_>>();
    Line::from(spans)
}

/// "28 min", or where there is little room "28′"; "end of the episode" likewise.
fn sleep_label(app: &App, short: bool) -> Option<String> {
    Some(match (app.playback.sleep?, short) {
        (Sleep::Minutes(minutes), true) => format!("{minutes}′"),
        (Sleep::Minutes(minutes), false) => format!("{minutes} min"),
        (Sleep::EndOfEpisode, true) => app.lang.t("end").to_owned(),
        (Sleep::EndOfEpisode, false) => app.lang.t("until the end of the episode").to_owned(),
    })
}

fn status_mark(now: &NowPlaying) -> (&'static str, Style) {
    match now.status {
        Status::Playing => ("▶", Style::new().fg(theme::ACCENT)),
        Status::Paused => ("▮▮", theme::muted()),
        Status::Loading => ("…", theme::muted()),
        Status::Failed(_) => ("▲", Style::new().fg(theme::AMBER)),
    }
}

pub fn draw_mini(frame: &mut Frame<'_>, area: Rect, app: &App, now: &NowPlaying) {
    let lang = app.lang;
    let block = panel(&format!("0  {}", lang.t("Now playing")), false);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let width = usize::from(inner.width.saturating_sub(2));
    let paused = now.status != Status::Playing;

    let mut lines = Vec::new();
    if area.height >= PLAYER_ROWS {
        lines.extend(meter(app, paused));
        lines.push(Line::default());
    }
    lines.push(Line::styled(format!(" {}", fit(&now.item.title, width)), theme::bold()));
    lines.push(Line::styled(format!(" {}", fit(&now.item.podcast, width)), theme::muted()));
    let mut bar = progress(now, width, false);
    bar.spans.insert(0, Span::raw(" "));
    lines.push(bar);

    let total = now.duration_ms.map(clock).unwrap_or_default();
    let position = clock(now.position_ms);
    let gap = width.saturating_sub(position.chars().count() + total.chars().count());
    lines.push(Line::styled(format!(" {position}{}{total}", " ".repeat(gap)), theme::muted()));

    lines.push(match (&now.status, now.chapter_index()) {
        (Status::Loading, _) => Line::styled(format!(" {}", lang.t("Loading …")), theme::muted()),
        (Status::Failed(_), _) => Line::styled(
            format!(" {}", fit(lang.t("Cannot be played. ␣ tries again."), width)),
            Style::new().fg(theme::AMBER),
        ),
        (_, Some(index)) => {
            let chapters = now.chapters();
            let label = format!("{} {}  ", lang.t("Chapter"), index + 1);
            let title = chapters[index].title.clone().unwrap_or_default();
            Line::from(vec![
                Span::styled(format!(" {label}"), theme::muted()),
                Span::styled(fit(&title, width.saturating_sub(label.chars().count())), Style::new().fg(theme::SILVER)),
            ])
        }
        _ => Line::default(),
    });
    lines.push(Line::default());

    // Five buttons, and under each the key that does the same.
    let toggle = if now.status == Status::Playing { "▮▮" } else { "▶ " };
    lines.push(Line::styled(format!(" ◀◀   {toggle}   ■   ▶▶   ▶▮"), theme::bold()));
    let key = |label: &'static str| Span::styled(label, Style::new().bg(theme::KEY).fg(theme::MUTED));
    lines.push(Line::from(vec![
        Span::raw(" "),
        key(" , "),
        Span::raw("  "),
        key(" ␣ "),
        Span::raw(" "),
        key(" x "),
        Span::raw(" "),
        key(" . "),
        Span::raw("  "),
        key(" n "),
    ]));
    lines.push(Line::from(vec![
        Span::styled(format!(" {:.1}×", app.playback.speed).replace('.', lang.decimal()), theme::bold()),
        Span::styled(format!("  ≡ {}", app.playback.up_next.len()), theme::muted()),
        Span::styled(
            sleep_label(app, true).map(|label| format!("  ⏾ {label}")).unwrap_or_default(),
            Style::new().fg(theme::SILVER),
        ),
    ]));
    frame.render_widget(Paragraph::new(lines), inner);
}

pub fn draw(frame: &mut Frame<'_>, area: Rect, app: &App, now: &NowPlaying) {
    let lang = app.lang;
    let [head, list] = Layout::vertical([Constraint::Length(11), Constraint::Min(0)]).areas(area);
    let block = panel(lang.t("Now playing"), false);
    let inner = block.inner(head);
    frame.render_widget(block, head);
    let inner = Rect { x: inner.x + 1, width: inner.width.saturating_sub(2), ..inner };
    let width = usize::from(inner.width);

    let (mark, mark_style) = status_mark(now);
    let mut lines = vec![
        Line::from(vec![
            Span::styled(format!("{mark} "), mark_style.add_modifier(Modifier::BOLD)),
            Span::styled(fit(&now.item.title, width.saturating_sub(3)).trim_end().to_owned(), theme::bold()),
        ]),
        Line::styled(format!("   {}", now.item.podcast), theme::muted()),
        Line::default(),
    ];
    let chapters = now.chapters();
    lines.push(match (&now.status, now.chapter_index()) {
        (Status::Failed(_), _) => Line::styled(
            lang.t("This episode cannot be played right now. ␣ tries again."),
            Style::new().fg(theme::AMBER),
        ),
        (Status::Loading, _) => Line::styled(lang.t("Loading …"), theme::muted()),
        (_, Some(index)) => Line::from(vec![
            Span::styled(format!("{} {} / {}   ", lang.t("Chapter"), index + 1, chapters.len()), theme::muted()),
            Span::styled(chapters[index].title.clone().unwrap_or_default(), theme::heading()),
        ]),
        _ => Line::default(),
    });
    lines.push(Line::default());
    lines.push(Line::from(vec![
        Span::styled(format!("{}  ", lang.t("Tempo")), theme::muted()),
        Span::styled(format!("{:.1}×", app.playback.speed).replace('.', lang.decimal()), theme::bold()),
        Span::styled(format!("      {}  ", lang.t("Up Next")), theme::muted()),
        Span::raw(app.playback.up_next.len().to_string()),
        Span::styled(format!("      {}  ", lang.t("Sleep timer")), theme::muted()),
        Span::raw(sleep_label(app, false).unwrap_or_else(|| lang.t("off").to_owned())),
    ]));
    lines.push(Line::default());

    let position = timestamp(now.position_ms);
    let remaining =
        now.duration_ms.map(|total| format!("-{}", clock(total.saturating_sub(now.position_ms)))).unwrap_or_default();
    let bar_width = width.saturating_sub(position.chars().count() + remaining.chars().count() + 4);
    let mut bar = progress(now, bar_width, true);
    bar.spans.insert(0, Span::styled(format!("{position}  "), theme::bold()));
    bar.spans.push(Span::styled(format!("  {remaining}"), theme::muted()));
    lines.push(bar);
    frame.render_widget(Paragraph::new(lines), inner);

    let title = if chapters.is_empty() {
        lang.t("Chapters").to_owned()
    } else {
        format!("{} · {}", lang.t("Chapters"), chapters.len())
    };
    let block = panel(&title, true);
    let inner = block.inner(list);
    frame.render_widget(block, list);
    if chapters.is_empty() {
        empty(frame, inner, &[lang.t("No chapters.")]);
        return;
    }
    let (width, height) = (usize::from(inner.width), usize::from(inner.height));
    let current = now.chapter_index();
    let start = window(app.player_chapter, chapters.len(), height, 1);
    let lines: Vec<Line<'_>> = chapters
        .iter()
        .enumerate()
        .skip(start)
        .take(height)
        .map(|(index, chapter)| {
            let chosen = index == app.player_chapter;
            let heard = current.is_some_and(|current| index < current);
            let background = if chosen { Style::new().bg(theme::SELECTION) } else { Style::new() };
            let next_start = chapters.get(index + 1).map(|next| next.start_ms).or(now.duration_ms);
            let length = next_start.map(|next| clock(next.saturating_sub(chapter.start_ms))).unwrap_or_default();
            let text = if heard { theme::faint() } else { Style::new() };
            let text = if chosen { text.add_modifier(Modifier::BOLD) } else { text };
            Line::from(vec![
                Span::styled(if current == Some(index) { " ▶  " } else { "    " }, background.fg(theme::ACCENT)),
                Span::styled(
                    format!("{}  ", timestamp(chapter.start_ms)),
                    background.fg(if heard { theme::FAINT } else { theme::MUTED }),
                ),
                Span::styled(
                    fit(chapter.title.as_deref().unwrap_or("—"), width.saturating_sub(26)),
                    text.patch(background),
                ),
                Span::styled(format!("{length:>10}  "), background.fg(if heard { theme::FAINT } else { theme::MUTED })),
            ])
        })
        .collect();
    frame.render_widget(Paragraph::new(lines), inner);
}
