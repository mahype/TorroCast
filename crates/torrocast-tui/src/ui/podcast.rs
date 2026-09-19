//! One podcast: what it is, and its episodes.

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use super::{empty, panel};
use crate::app::{App, Load, PodcastView};
use crate::text::{date, duration, fit, window, wrap};
use crate::theme;

const FOLDED_ROWS: usize = 2;
const UNFOLDED_ROWS: usize = 10;

pub fn draw(frame: &mut Frame<'_>, area: Rect, app: &App, view: &PodcastView) {
    let lang = app.lang;
    let podcast = view.podcast.as_deref();
    let title = podcast.map_or(view.reference.title.as_str(), |podcast| podcast.title.as_str());
    let has_cover = app.settings.covers && app.covers.get(view.reference.artwork_url.as_deref()).is_some();
    let beside = if has_cover { usize::from(crate::covers::SMALL.width) + 2 } else { 0 };
    let text_width = usize::from(area.width.saturating_sub(4)).saturating_sub(beside);

    // Until the feed is here, the directory's words stand in.
    let author = podcast.and_then(|podcast| podcast.author.clone()).or_else(|| view.reference.author.clone());
    let description =
        podcast.and_then(|podcast| podcast.description.clone()).or_else(|| view.reference.description.clone());
    let description_rows = description.as_deref().map(|text| wrap(text, text_width)).unwrap_or_default();
    let shown_rows = description_rows.len().min(if view.expanded { UNFOLDED_ROWS } else { FOLDED_ROWS });

    let mut lines = vec![Line::styled(fit(title, text_width).trim_end().to_owned(), theme::bold())];
    let subscribed = view.reference.feed_url.as_deref().is_some_and(|feed_url| app.is_subscribed(feed_url));
    let mut byline = vec![Span::styled(author.unwrap_or_default(), theme::muted())];
    if subscribed {
        byline.push(Span::styled(format!("   ✓ {}", lang.t("Subscribed")), Style::new().fg(theme::GREEN)));
    }
    lines.push(Line::from(byline));
    lines.push(Line::default());

    let mut facts: Vec<String> = match podcast {
        Some(podcast) => podcast.categories.iter().take(3).cloned().collect(),
        None => view.reference.genres.iter().take(3).cloned().collect(),
    };
    if let Some(language) =
        podcast.and_then(|podcast| podcast.language.clone()).or_else(|| view.reference.language.clone())
    {
        facts.push(language);
    }
    if let Some(count) =
        podcast.map(|podcast| podcast.episodes.len()).or(view.reference.episode_count.map(|count| count as usize))
    {
        facts.push(format!("{count} {}", lang.t("episodes")));
    }
    if podcast.is_some_and(|podcast| podcast.explicit) {
        facts.push(lang.t("explicit").to_owned());
    }
    lines.push(Line::raw(fit(&facts.join(" · "), text_width).trim_end().to_owned()));

    let website = podcast.and_then(|podcast| podcast.website.clone()).or_else(|| view.reference.website.clone());
    if let Some(website) = website {
        lines.push(Line::styled(short(&website, text_width), theme::link()));
    }
    if let Some(funding) = podcast.and_then(|podcast| podcast.funding.first()) {
        let label = funding.label.clone().unwrap_or_else(|| lang.t("Support").to_owned());
        lines.push(Line::from(vec![
            Span::styled("♥ ", Style::new().fg(theme::ACCENT)),
            Span::styled(format!("{label}  "), theme::muted()),
            Span::styled(short(&funding.url, text_width.saturating_sub(label.chars().count() + 4)), theme::link()),
        ]));
    }
    if shown_rows > 0 {
        lines.push(Line::default());
        for (index, row) in description_rows.iter().take(shown_rows).enumerate() {
            let last_shown = index + 1 == shown_rows;
            let folds = description_rows.len() > FOLDED_ROWS;
            if last_shown && folds {
                let label = lang.t(if view.expanded { "less" } else { "more" });
                let room = text_width.saturating_sub(label.chars().count() + 6);
                let cut = if description_rows.len() > shown_rows {
                    format!("{} …", fit(row, room.saturating_sub(2)).trim_end())
                } else {
                    row.clone()
                };
                lines.push(Line::from(vec![
                    Span::raw(fit(&cut, room + 1)),
                    Span::styled(" m ", Style::new().bg(theme::KEY).add_modifier(Modifier::BOLD)),
                    Span::styled(format!(" {label}"), theme::muted()),
                ]));
            } else {
                lines.push(Line::raw(row.clone()));
            }
        }
    }

    // With a cover beside it the text needs at least the cover's height.
    let cover = app.covers.get(view.reference.artwork_url.as_deref()).filter(|_| app.settings.covers);
    while cover.is_some() && lines.len() < usize::from(crate::covers::SMALL.height) {
        lines.push(Line::default());
    }
    let head_height = (lines.len() as u16 + 2).min(area.height.saturating_sub(6));
    let [head, list] = Layout::vertical([Constraint::Length(head_height), Constraint::Min(0)]).areas(area);
    let block = panel(&format!("{} › {}", lang.t("Discover"), fit(title, 40).trim_end()), false);
    let inner = block.inner(head);
    frame.render_widget(block, head);
    let mut text = Rect { x: inner.x + 1, width: inner.width.saturating_sub(2), ..inner };
    if let Some(cover) = cover {
        let room = crate::covers::SMALL;
        let picture =
            Rect { x: text.x, y: text.y, width: room.width.min(text.width), height: room.height.min(text.height) };
        frame.render_widget(ratatui_image::Image::new(&cover.small), picture);
        text.x += room.width + 2;
        text.width = text.width.saturating_sub(room.width + 2);
    }
    frame.render_widget(Paragraph::new(lines), text);

    draw_episodes(frame, list, app, view);
}

fn short(address: &str, width: usize) -> String {
    let address = address.split_once("://").map_or(address, |(_, rest)| rest).trim_end_matches('/');
    fit(address, width).trim_end().to_owned()
}

fn draw_episodes(frame: &mut Frame<'_>, area: Rect, app: &App, view: &PodcastView) {
    let lang = app.lang;
    let order = lang.t(if view.newest_first { "newest first" } else { "oldest first" });
    let mut title = format!("{} · {order}", lang.t("Episodes"));
    if view.filtering || !view.filter.is_empty() {
        title.push_str(&format!(" · {}: {}{}", lang.t("Filter"), view.filter, if view.filtering { "▏" } else { "" }));
    }
    let block = panel(&title, true);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let Some(podcast) = view.podcast.as_deref() else {
        match view.load {
            Load::Failed(problem) => empty(frame, inner, &[lang.problem(problem)]),
            _ => empty(frame, inner, &[lang.t("Loading the episodes …")]),
        }
        return;
    };
    let visible = view.visible();
    if visible.is_empty() {
        let sentence = if podcast.episodes.is_empty() {
            "This feed has no episodes yet."
        } else {
            "No episode matches the filter."
        };
        empty(frame, inner, &[lang.t(sentence)]);
        return;
    }

    // The last row explains the marks: they do not explain themselves.
    let height = usize::from(inner.height.saturating_sub(1));
    let width = usize::from(inner.width);
    const DATE: usize = 10;
    const LENGTH: usize = 8;
    const MARKS: usize = 5;
    let title_width = width.saturating_sub(DATE + LENGTH + MARKS + 5);

    let start = window(view.index, visible.len(), height, 1);

    super::clickable(app, inner, start, 1, visible.len());
    let mut lines: Vec<Line<'_>> = visible
        .iter()
        .enumerate()
        .skip(start)
        .take(height)
        .map(|(index, position)| {
            let episode = &podcast.episodes[*position];
            let published = episode.published.map(|moment| date(lang, moment)).unwrap_or_default();
            let length = episode.duration_seconds.map(duration).unwrap_or_default();
            let marks = format!(
                "{} {}",
                if episode.announces_chapters() { "▤" } else { " " },
                if episode.transcripts.is_empty() { " " } else { "¶" }
            );
            let chosen = index == view.index;
            // The first column says where the episode stands in the queue.
            let key = torrocast_core::QueueItem::from_feed(podcast, view.reference.feed_url.as_deref(), episode)
                .map(|item| item.key());
            let (queue_mark, queue_colour) = match key.and_then(|key| app.queue_state(&key)) {
                Some(crate::app::QueueState::Playing) => ("▶", theme::ACCENT),
                Some(crate::app::QueueState::Queued(_)) => ("✓", theme::GREEN),
                None => (" ", theme::MUTED),
            };
            // Queue state first; an episode that is simply on this machine shows that.
            let here = torrocast_core::QueueItem::from_feed(podcast, view.reference.feed_url.as_deref(), episode)
                .and_then(|item| app.download_state(&item.key()));
            let (queue_mark, queue_colour) = match (queue_mark, here) {
                (" ", Some(torrocast_core::DownloadState::Done { .. })) => ("↓", theme::SILVER),
                (" ", Some(torrocast_core::DownloadState::Loading { .. })) => ("↓", theme::FAINT),
                (mark, _) => (mark, queue_colour),
            };
            let (base, quiet, mark) = if chosen {
                let background = Style::new().bg(theme::SELECTION);
                (theme::selected(), background.fg(theme::MUTED), background.fg(theme::CYAN))
            } else {
                (Style::new(), theme::muted(), theme::link())
            };
            Line::from(vec![
                Span::styled(
                    queue_mark,
                    if chosen {
                        Style::new().bg(theme::SELECTION).fg(queue_colour)
                    } else {
                        Style::new().fg(queue_colour)
                    },
                ),
                Span::styled(format!("{} ", fit(&episode.title, title_width)), base),
                Span::styled(format!("{published:>DATE$}  {length:>LENGTH$} "), quiet),
                Span::styled(fit(&format!(" {marks}"), MARKS + 1), mark),
            ])
        })
        .collect();
    while lines.len() < height {
        lines.push(Line::default());
    }
    // What was just queued is said here; otherwise the row explains the marks.
    if let Some(notice) = &app.notice {
        lines.push(Line::from(vec![
            Span::styled(" ✓ ", Style::new().fg(theme::GREEN)),
            Span::styled(notice.clone(), theme::muted()),
        ]));
        frame.render_widget(Paragraph::new(lines), inner);
        return;
    }
    lines.push(Line::from(vec![
        Span::styled(" ▤", theme::link()),
        Span::styled(format!(" {}   ", lang.t("Chapters")), theme::faint()),
        Span::styled("¶", theme::link()),
        Span::styled(format!(" {}", lang.t("Transcript")), theme::faint()),
    ]));
    frame.render_widget(Paragraph::new(lines), inner);
}
