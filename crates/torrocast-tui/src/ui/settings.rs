//! Where TorroCast looks for podcasts.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use super::panel;
use crate::app::App;
use crate::text::fit;
use crate::theme;

struct Source {
    name: &'static str,
    about: &'static str,
    /// The mark and the word — never the colour alone.
    mark: (&'static str, Style),
    state: (&'static str, Style),
}

pub fn draw(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let lang = app.lang;
    let block = panel(&format!("{} › {}", lang.t("Settings"), lang.t("Where TorroCast looks for podcasts")), true);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let width = usize::from(inner.width);

    let on = (("●", Style::new().fg(theme::GREEN)), ("Active", Style::new()));
    let off = (("○", theme::muted()), ("Off", theme::muted()));
    let fyyd = if app.settings.sources.fyyd { on } else { off };
    let sources = [
        Source {
            name: "Apple Podcasts",
            about: "Search, charts and categories. Needs no setup.",
            mark: on.0,
            state: on.1,
        },
        Source {
            name: "Podcast Index",
            about: "Open directory with trending shows. Needs your own free key.",
            mark: ("○", theme::faint()),
            state: ("Comes with a later version", theme::faint()),
        },
        Source { name: "fyyd", about: "German-language directory. Needs no setup.", mark: fyyd.0, state: fyyd.1 },
    ];

    let mut lines = vec![Line::default()];
    for (index, source) in sources.iter().enumerate() {
        let chosen = index == app.settings_index;
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
    // Where the library lives: said, not yet changeable from here.
    lines.push(Line::default());
    lines.push(Line::styled(format!(" {}", lang.t("Library folder")), theme::heading()));
    let chosen = app.settings_index == 4;
    if let Some(typed) = &app.library_input {
        lines.push(Line::from(vec![
            Span::styled(fit(&format!(" {typed}"), width.saturating_sub(1)).trim_end().to_owned(), theme::selected()),
            Span::styled("▏", Style::new().fg(theme::ACCENT)),
        ]));
        let advice = lang.t(
            "enter moves the library there, with everything in it; esc leaves it where it is. ~ is your home folder.",
        );
        lines.extend(
            crate::text::wrap(advice, width.saturating_sub(2))
                .into_iter()
                .map(|row| Line::styled(format!(" {row}"), theme::muted())),
        );
        frame.render_widget(Paragraph::new(lines), inner);
        return;
    }
    match &app.library {
        Ok(directory) => {
            lines.push(Line::styled(
                fit(&format!(" {directory}"), width),
                if chosen { theme::selected() } else { Style::new() },
            ));
            let advice = lang.t("Subscriptions, Up Next and positions live here. Press enter to move it — into Dropbox, Syncthing or onto a NAS, to back it up and share it between devices.");
            lines.extend(
                crate::text::wrap(advice, width.saturating_sub(2))
                    .into_iter()
                    .map(|row| Line::styled(format!(" {row}"), theme::muted())),
            );
        }
        Err(reason) => {
            let sentence =
                format!("{} {reason}", lang.t("The library could not be opened; nothing is kept beyond this session."));
            lines.extend(
                crate::text::wrap(&sentence, width.saturating_sub(2))
                    .into_iter()
                    .map(|row| Line::styled(format!(" {row}"), Style::new().fg(theme::AMBER))),
            );
        }
    }
    if let Some(notice) = &app.notice {
        lines.push(Line::default());
        lines.push(Line::styled(format!(" {notice}"), Style::new().fg(theme::AMBER)));
    }
    frame.render_widget(Paragraph::new(lines), inner);
}
