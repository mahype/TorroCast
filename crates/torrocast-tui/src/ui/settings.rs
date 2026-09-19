//! Where TorroCast looks for podcasts, and where it keeps the library.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use super::panel;
use crate::app::{App, Field, IndexState};
use crate::text::{fit, wrap};
use crate::theme;

struct Source {
    name: &'static str,
    about: &'static str,
    /// The mark and the word — never the colour alone.
    mark: (&'static str, Style),
    state: (&'static str, Style),
}

/// A field being typed: what is asked for, the text so far, and what the keys do.
fn draw_input(frame: &mut Frame<'_>, inner: Rect, app: &App, field: Field, typed: &str) {
    let lang = app.lang;
    let width = usize::from(inner.width);
    let (question, advice) = match field {
        Field::Library => (
            "Library folder",
            "enter moves the library there, with everything in it; esc leaves it where it is. ~ is your home folder.",
        ),
        Field::IndexKey => (
            "Podcast Index · API key",
            "A key and its secret are free at api.podcastindex.org. Paste the key, then press enter; esc cancels.",
        ),
        Field::IndexSecret => (
            "Podcast Index · API secret",
            "Paste the secret that came with the key, then press enter. It is shown masked.",
        ),
    };
    // A secret is never on the screen in full: only how long it is, and its last two characters.
    let shown = if field == Field::IndexSecret { mask(typed) } else { typed.to_owned() };
    let mut lines = vec![
        Line::default(),
        Line::styled(format!(" {}", lang.t(question)), theme::heading()),
        Line::from(vec![
            Span::styled(fit(&format!(" {shown}"), width.saturating_sub(1)).trim_end().to_owned(), theme::selected()),
            Span::styled("▏", Style::new().fg(theme::ACCENT)),
        ]),
        Line::default(),
    ];
    lines.extend(
        wrap(lang.t(advice), width.saturating_sub(2))
            .into_iter()
            .map(|row| Line::styled(format!(" {row}"), theme::muted())),
    );
    frame.render_widget(Paragraph::new(lines), inner);
}

fn mask(secret: &str) -> String {
    let count = secret.chars().count();
    let tail: String = secret.chars().skip(count.saturating_sub(2)).collect();
    if count <= 2 { "•".repeat(count) } else { format!("{}{tail}", "•".repeat(count - 2)) }
}

pub fn draw(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let lang = app.lang;
    let block = panel(&format!("{} › {}", lang.t("Settings"), lang.t("Where TorroCast looks for podcasts")), true);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let width = usize::from(inner.width);
    if let Some((field, typed)) = &app.settings_input {
        draw_input(frame, inner, app, *field, typed);
        return;
    }

    let on = (("●", Style::new().fg(theme::GREEN)), ("Active", Style::new()));
    let off = (("○", theme::muted()), ("Off", theme::muted()));
    let waiting = Style::new().fg(theme::AMBER);
    let sources_set = &app.settings.sources;
    let has_key = !sources_set.podcast_index_key.is_empty() && !sources_set.podcast_index_secret.is_empty();
    let index = match (has_key, sources_set.podcast_index, app.index_state) {
        (false, _, _) => (("○", waiting), ("No key stored", waiting)),
        (true, _, IndexState::Checking) => (("○", waiting), ("Checking the key …", waiting)),
        (true, _, IndexState::Rejected) => (("▲", waiting), ("The key is not accepted", waiting)),
        (true, _, IndexState::Unreachable) => (("▲", waiting), ("Not reachable", waiting)),
        (true, true, _) => on,
        (true, false, _) => off,
    };
    let sources = [
        Source {
            name: "Apple Podcasts",
            about: "Search, charts and categories. Needs no setup.",
            mark: on.0,
            state: on.1,
        },
        Source {
            name: "Podcast Index",
            about: if has_key {
                "Open directory. enter switches it, e enters another key."
            } else {
                "Open directory. Needs your own free key — enter to type it in."
            },
            mark: index.0,
            state: index.1,
        },
        Source {
            name: "fyyd",
            about: "German-language directory. Needs no setup.",
            mark: if sources_set.fyyd { on.0 } else { off.0 },
            state: if sources_set.fyyd { on.1 } else { off.1 },
        },
    ];

    let mut lines = vec![Line::default()];
    for (row, source) in sources.iter().enumerate() {
        let chosen = row == app.settings_index;
        let background = if chosen { Style::new().bg(theme::SELECTION) } else { Style::new() };
        let state = lang.t(source.state.0);
        let name_width = width.saturating_sub(state.chars().count() + 7);
        lines.push(Line::from(vec![
            Span::styled(format!(" {}  ", source.mark.0), source.mark.1.patch(background)),
            Span::styled(fit(source.name, name_width), if chosen { theme::selected() } else { theme::bold() }),
            Span::styled(format!(" {state}  "), source.state.1.patch(background)),
        ]));
        lines.push(Line::styled(fit(&format!("    {}", lang.t(source.about)), width), background.fg(theme::MUTED)));
        lines.push(Line::default());
    }

    let chosen = app.settings_index == 3;
    let label = lang.t("Country for search and charts");
    let country = format!("‹ {} ›", lang.country(&app.settings.country));
    let background = if chosen { Style::new().bg(theme::SELECTION) } else { Style::new() };
    lines.push(Line::from(vec![
        Span::styled(format!(" {label:<34}"), background.fg(theme::MUTED)),
        Span::styled(fit(&country, width.saturating_sub(35)), if chosen { theme::selected() } else { Style::new() }),
    ]));

    // Where the library lives.
    lines.push(Line::default());
    lines.push(Line::styled(format!(" {}", lang.t("Library folder")), theme::heading()));
    let chosen = app.settings_index == 4;
    match &app.library {
        Ok(directory) => {
            lines.push(Line::styled(
                fit(&format!(" {directory}"), width),
                if chosen { theme::selected() } else { Style::new() },
            ));
            let advice = lang.t("Subscriptions, Up Next and positions live here. Press enter to move it — into Dropbox, Syncthing or onto a NAS, to back it up and share it between devices.");
            lines.extend(
                wrap(advice, width.saturating_sub(2))
                    .into_iter()
                    .map(|row| Line::styled(format!(" {row}"), theme::muted())),
            );
        }
        Err(reason) => {
            let sentence =
                format!("{} {reason}", lang.t("The library could not be opened; nothing is kept beyond this session."));
            lines.extend(
                wrap(&sentence, width.saturating_sub(2))
                    .into_iter()
                    .map(|row| Line::styled(format!(" {row}"), waiting)),
            );
        }
    }
    lines.push(Line::default());
    let chosen = app.settings_index == 5;
    let background = if chosen { Style::new().bg(theme::SELECTION) } else { Style::new() };
    let covers = lang.t(if app.settings.covers { "on" } else { "off" });
    lines.push(Line::from(vec![
        Span::styled(format!(" {:<34}", lang.t("Podcast covers")), background.fg(theme::MUTED)),
        Span::styled(fit(covers, width.saturating_sub(35)), if chosen { theme::selected() } else { Style::new() }),
    ]));
    if let Some(notice) = &app.notice {
        lines.push(Line::default());
        lines.extend(
            wrap(notice, width.saturating_sub(2)).into_iter().map(|row| Line::styled(format!(" {row}"), waiting)),
        );
    }
    frame.render_widget(Paragraph::new(lines), inner);
}
