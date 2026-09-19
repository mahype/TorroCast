//! Search, charts and categories: three ways to a podcast.

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Wrap};
use torrocast_core::PodcastRef;

use super::{empty, field, is_wide, panel, tab_lines};
use crate::app::{App, Load, Tab};
use crate::text::{date, fit, window, wrap};
use crate::theme;

const LIST_WIDTH: u16 = 42;

pub fn draw(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let lang = app.lang;
    let head_height = if app.tab == Tab::Search { 6 } else { 4 };
    let [head, body] = Layout::vertical([Constraint::Length(head_height), Constraint::Min(0)]).areas(area);

    let editing = app.tab == Tab::Search && app.search.editing;
    let block = panel(lang.t("Discover"), editing);
    let inner = block.inner(head);
    frame.render_widget(block, head);
    let titles: Vec<&str> = Tab::ALL.iter().map(|tab| lang.t(tab.title())).collect();
    let active = Tab::ALL.iter().position(|tab| *tab == app.tab).unwrap_or(0);
    let mut lines: Vec<Line<'_>> = tab_lines(&titles, active).into();
    if app.tab == Tab::Search {
        let mut input = vec![
            Span::styled("  ⌕ ", theme::muted()),
            Span::raw(app.search.input.clone()),
        ];
        if editing {
            input.push(Span::styled("▏", Style::new().fg(theme::ACCENT)));
        }
        lines.push(Line::default());
        lines.push(Line::from(input));
    }
    frame.render_widget(Paragraph::new(lines), inner);

    match app.tab {
        Tab::Search => draw_search(frame, body, app),
        Tab::Charts => draw_charts(frame, body, app),
        Tab::Categories => draw_categories(frame, body, app),
    }
}

fn draw_search(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let lang = app.lang;
    let search = &app.search;
    let mut title = if search.sent.is_empty() {
        lang.t("Results").to_owned()
    } else {
        format!("{} {}", search.results.len(), lang.t("Results"))
    };
    for provider in &search.pending {
        title.push_str(&format!(" · {} {}", provider.name(), lang.t("still loading")));
    }
    let focused = !search.editing;
    // A directory that failed is said once, below the list; the others' results stay.
    let mut problems: Vec<String> = search
        .failures
        .iter()
        .map(|(provider, problem)| lang.provider_problem(*provider, *problem))
        .collect();
    if let [(_, problem)] = search.failures.as_slice()
        && search.pending.is_empty()
        && search.results.is_empty()
    {
        problems = vec![lang.problem(*problem).to_owned()];
    }

    if search.results.is_empty() {
        let block = panel(&title, focused);
        let inner = block.inner(area);
        frame.render_widget(block, area);
        let note = app.notice.clone().or_else(|| problems.first().cloned());
        if let Some(note) = note {
            empty(frame, inner, &[&note]);
        } else if search.sent.is_empty() {
            empty(
                frame,
                inner,
                &[
                    lang.t("Nothing searched yet. Type a word, or switch to the charts with tab."),
                    "",
                    lang.t("A feed address works here too."),
                ],
            );
        } else if search.pending.is_empty() {
            empty(frame, inner, &[lang.t("Nothing found for this.")]);
        } else {
            empty(frame, inner, &[lang.t("Searching …")]);
        }
        return;
    }
    let footer = app.notice.clone().or_else(|| problems.first().cloned());
    draw_shows(
        frame,
        area,
        app,
        &title,
        &search.results,
        search.index,
        focused,
        footer.as_deref(),
    );
}

fn draw_charts(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let lang = app.lang;
    let charts = &app.charts;
    let country = lang.country(&app.settings.country);
    let title = match &charts.category {
        Some(category) => format!("{} · {} · {country}", lang.t("Charts"), category.name),
        None => format!("{} · {country}", lang.t("Charts")),
    };
    match charts.load {
        Load::Ready if !charts.list.is_empty() => {
            draw_shows(
                frame,
                area,
                app,
                &title,
                &charts.list,
                charts.index,
                true,
                app.notice.as_deref(),
            );
        }
        state => {
            let block = panel(&title, true);
            let inner = block.inner(area);
            frame.render_widget(block, area);
            match state {
                Load::Failed(problem) => empty(frame, inner, &[lang.problem(problem)]),
                Load::Ready => empty(frame, inner, &[lang.t("Nothing found for this.")]),
                Load::Idle | Load::Loading => empty(frame, inner, &[lang.t("Loading the charts …")]),
            }
        }
    }
}

fn draw_categories(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let lang = app.lang;
    let categories = &app.categories;
    let block = panel(lang.t("Categories"), true);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    match categories.load {
        Load::Failed(problem) => empty(frame, inner, &[lang.problem(problem)]),
        Load::Idle | Load::Loading => empty(frame, inner, &[lang.t("Loading the categories …")]),
        Load::Ready => {
            let (width, height) = (usize::from(inner.width), usize::from(inner.height));
            let start = window(categories.index, categories.list.len(), height, 1);
            let lines: Vec<Line<'_>> = categories
                .list
                .iter()
                .enumerate()
                .skip(start)
                .take(height)
                .map(|(index, category)| {
                    let label = format!(" {}{}", "   ".repeat(usize::from(category.depth)), category.name);
                    if index == categories.index {
                        Line::styled(fit(&label, width), theme::selected())
                    } else if category.depth == 0 {
                        Line::raw(label)
                    } else {
                        Line::styled(label, theme::muted())
                    }
                })
                .collect();
            frame.render_widget(Paragraph::new(lines), inner);
        }
    }
}

/// A list of shows, two rows each, with a preview beside it where there is room.
#[allow(clippy::too_many_arguments)]
fn draw_shows(
    frame: &mut Frame<'_>,
    area: Rect,
    app: &App,
    title: &str,
    shows: &[PodcastRef],
    selected: usize,
    focused: bool,
    footer: Option<&str>,
) {
    let (list_area, preview_area) = if is_wide(area) {
        let [list, preview] = Layout::horizontal([Constraint::Length(LIST_WIDTH), Constraint::Min(0)]).areas(area);
        (list, Some(preview))
    } else {
        (area, None)
    };

    let block = panel(title, focused);
    let mut inner = block.inner(list_area);
    frame.render_widget(block, list_area);
    if let Some(footer) = footer {
        let rows = wrap(footer, usize::from(inner.width.saturating_sub(2)));
        let height = (rows.len() as u16).min(inner.height.saturating_sub(3));
        let note = Rect {
            x: inner.x + 1,
            y: inner.y + inner.height - height,
            width: inner.width.saturating_sub(2),
            height,
        };
        let lines: Vec<Line<'_>> = rows.into_iter().map(|row| Line::styled(row, theme::muted())).collect();
        frame.render_widget(Paragraph::new(lines), note);
        inner.height = inner.height.saturating_sub(height + 1);
    }

    let (width, height) = (usize::from(inner.width), usize::from(inner.height));
    let start = window(selected, shows.len(), height, 3);
    let mut lines = Vec::new();
    for (index, show) in shows.iter().enumerate().skip(start).take(height.div_ceil(3)) {
        let mut meta: Vec<&str> = show.author.as_deref().into_iter().collect();
        meta.extend(show.genres.first().map(String::as_str));
        let meta = meta.join(" · ");
        if index == selected {
            lines.push(Line::styled(fit(&format!(" {}", show.title), width), theme::selected()));
            lines.push(Line::styled(
                fit(&format!(" {meta}"), width),
                Style::new().bg(theme::SELECTION).fg(theme::MUTED),
            ));
        } else {
            lines.push(Line::raw(fit(&format!(" {}", show.title), width)));
            lines.push(Line::styled(fit(&format!(" {meta}"), width), theme::muted()));
        }
        lines.push(Line::default());
    }
    frame.render_widget(Paragraph::new(lines), inner);

    if let (Some(preview_area), Some(show)) = (preview_area, shows.get(selected)) {
        draw_preview(frame, preview_area, app, show);
    }
}

fn draw_preview(frame: &mut Frame<'_>, area: Rect, app: &App, show: &PodcastRef) {
    let lang = app.lang;
    let block = panel(lang.t("Preview"), false);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let inner = Rect {
        x: inner.x + 1,
        width: inner.width.saturating_sub(2),
        ..inner
    };
    let width = usize::from(inner.width);

    let mut lines: Vec<Line<'_>> = wrap(&show.title, width)
        .into_iter()
        .map(|row| Line::styled(row, theme::bold()))
        .collect();
    if let Some(author) = &show.author {
        lines.extend(
            wrap(author, width)
                .into_iter()
                .map(|row| Line::styled(row, theme::muted())),
        );
    }
    lines.push(Line::default());
    if !show.genres.is_empty() {
        lines.extend(wrap(&show.genres.join(" · "), width).into_iter().map(Line::raw));
    }
    let mut facts = Vec::new();
    if let Some(count) = show.episode_count {
        facts.push(format!("{count} {}", lang.t("episodes")));
    }
    if let Some(last) = show.last_published {
        facts.push(format!("{} {}", lang.t("last"), date(lang, last)));
    }
    if !facts.is_empty() {
        lines.push(Line::raw(facts.join(" · ")));
    }
    lines.push(Line::default());
    let sources: Vec<&str> = show.sources.iter().map(|source| source.name()).collect();
    lines.push(field(lang.t("Source"), Span::raw(sources.join(", "))));
    if let Some(language) = &show.language {
        lines.push(field(lang.t("Language"), Span::raw(language.clone())));
    }
    if let Some(website) = &show.website {
        let address = website
            .split_once("://")
            .map_or(website.as_str(), |(_, rest)| rest)
            .trim_end_matches('/');
        lines.push(field(
            lang.t("Website"),
            Span::styled(
                fit(address, width.saturating_sub(14)).trim_end().to_owned(),
                theme::link(),
            ),
        ));
    }
    lines.push(Line::default());
    match &show.description {
        Some(description) => lines.extend(wrap(description, width).into_iter().map(Line::raw)),
        None if show.feed_url.is_none() => lines.extend(
            wrap(
                lang.t("This podcast has no open feed. It can only be heard inside the platform that hosts it."),
                width,
            )
            .into_iter()
            .map(|row| Line::styled(row, Style::new().fg(theme::AMBER))),
        ),
        None => lines.extend(
            wrap(
                lang.t("Description and episodes appear once you open the podcast."),
                width,
            )
            .into_iter()
            .map(|row| Line::styled(row, theme::faint())),
        ),
    }
    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), inner);
}
