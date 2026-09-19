//! Fitting text into cells: truncating, padding, wrapping — by display width,
//! because a terminal counts columns and not characters.

use chrono::{DateTime, Local, Utc};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use torrocast_core::{Block, Document, Inline};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use crate::i18n::Lang;
use crate::theme;

/// `text` in exactly `width` columns: cut with an ellipsis or padded with spaces.
#[must_use]
pub fn fit(text: &str, width: usize) -> String {
    if text.width() <= width {
        return format!("{text}{}", " ".repeat(width - text.width()));
    }
    let mut fitted = String::new();
    let mut used = 0;
    for character in text.chars() {
        let columns = character.width().unwrap_or(0);
        if used + columns + 1 > width {
            break;
        }
        fitted.push(character);
        used += columns;
    }
    if width > 0 {
        fitted.push('…');
        used += 1;
    }
    format!("{fitted}{}", " ".repeat(width.saturating_sub(used)))
}

/// `1:34:10`, or `58:02` below an hour.
#[must_use]
pub fn duration(seconds: u32) -> String {
    let (hours, minutes, seconds) = (seconds / 3600, seconds / 60 % 60, seconds % 60);
    if hours > 0 { format!("{hours}:{minutes:02}:{seconds:02}") } else { format!("{minutes}:{seconds:02}") }
}

/// A chapter's start: always with hours, so a column of them lines up.
#[must_use]
pub fn timestamp(milliseconds: u64) -> String {
    let seconds = milliseconds / 1000;
    format!("{:02}:{:02}:{:02}", seconds / 3600, seconds / 60 % 60, seconds % 60)
}

#[must_use]
pub fn date(lang: Lang, moment: DateTime<Utc>) -> String {
    moment.with_timezone(&Local).format(lang.date_format()).to_string()
}

/// Plain text wrapped at word boundaries.
#[must_use]
pub fn wrap(text: &str, width: usize) -> Vec<String> {
    let mut lines = Vec::new();
    for paragraph in text.lines() {
        let mut line = String::new();
        for word in paragraph.split_whitespace() {
            if !line.is_empty() && line.width() + 1 + word.width() > width {
                lines.push(std::mem::take(&mut line));
            }
            if !line.is_empty() {
                line.push(' ');
            }
            line.push_str(word);
        }
        lines.push(line);
    }
    lines
}

struct Word {
    text: String,
    style: Style,
    /// Whether a space separated this word from the one before it.
    spaced: bool,
}

fn words(inlines: &[Inline], base: Style) -> Vec<Word> {
    let mut words = Vec::new();
    let mut spaced = false;
    let mut push = |text: &str, style: Style, spaced: &mut bool| {
        for (index, piece) in text.split(' ').enumerate() {
            if index > 0 {
                *spaced = true;
            }
            if piece.is_empty() {
                continue;
            }
            words.push(Word { text: piece.to_owned(), style, spaced: *spaced });
            *spaced = false;
        }
    };
    for inline in inlines {
        match inline {
            Inline::Text(text) => push(text, base, &mut spaced),
            Inline::Link { text, target } => {
                push(text, theme::link(), &mut spaced);
                spaced = true;
                push(&format!("[{}]", target + 1), theme::link().add_modifier(Modifier::BOLD), &mut spaced);
            }
        }
    }
    words
}

fn flow(words: Vec<Word>, width: usize, first_prefix: &str, prefix: &str, lines: &mut Vec<Line<'static>>) {
    let mut spans: Vec<Span<'static>> = vec![Span::styled(first_prefix.to_owned(), theme::muted())];
    let mut used = first_prefix.width();
    let mut empty = true;
    for word in words {
        let columns = word.text.width();
        let gap = usize::from(word.spaced && !empty);
        if !empty && used + gap + columns > width {
            lines.push(Line::from(std::mem::take(&mut spans)));
            spans.push(Span::raw(prefix.to_owned()));
            used = prefix.width();
            empty = true;
        }
        if word.spaced && !empty {
            spans.push(Span::raw(" "));
            used += 1;
        }
        spans.push(Span::styled(word.text, word.style));
        used += columns;
        empty = false;
    }
    lines.push(Line::from(spans));
}

/// Show notes laid out for `width` columns, with the list of links at the end.
#[must_use]
pub fn notes(lang: Lang, document: &Document, width: usize) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    let mut previous_was_item = false;
    for block in &document.blocks {
        let is_item = matches!(block, Block::Item(_));
        if !lines.is_empty() && !(is_item && previous_was_item) {
            lines.push(Line::default());
        }
        match block {
            Block::Paragraph(inlines) => flow(words(inlines, Style::new()), width, "", "", &mut lines),
            Block::Heading(inlines) => flow(words(inlines, theme::heading()), width, "", "", &mut lines),
            Block::Item(inlines) => flow(words(inlines, Style::new()), width, "• ", "  ", &mut lines),
        }
        previous_was_item = is_item;
    }
    if !document.links.is_empty() {
        if !lines.is_empty() {
            lines.push(Line::default());
        }
        lines.push(Line::styled(lang.t("Links"), theme::heading()));
        for (index, link) in document.links.iter().enumerate() {
            let number = format!("[{}] ", index + 1);
            let address = link.split_once("://").map_or(link.as_str(), |(_, rest)| rest);
            let room = width.saturating_sub(number.width());
            lines.push(Line::from(vec![
                Span::styled(number, theme::link().add_modifier(Modifier::BOLD)),
                Span::styled(fit(address, room).trim_end().to_owned(), theme::muted()),
            ]));
        }
    }
    lines
}

/// The first row to show so that `index` stays visible in `height` rows of
/// entries `rows_each` tall.
#[must_use]
pub fn window(index: usize, count: usize, height: usize, rows_each: usize) -> usize {
    let capacity = (height / rows_each.max(1)).max(1);
    if count <= capacity {
        return 0;
    }
    // Keep the selection around the middle, as a pager would.
    index.saturating_sub(capacity / 2).min(count - capacity)
}

#[cfg(test)]
mod tests {
    use super::{duration, fit, timestamp, window, wrap};

    #[test]
    fn fit_counts_columns() {
        assert_eq!(fit("Lage", 6), "Lage  ");
        assert_eq!(fit("Lage der Nation", 8), "Lage de…");
        assert_eq!(fit("日本語のポッドキャスト", 7), "日本語…");
        assert_eq!(fit("anything", 0), "");
    }

    #[test]
    fn times() {
        assert_eq!(duration(5650), "1:34:10");
        assert_eq!(duration(3482), "58:02");
        assert_eq!(timestamp(3_527_000), "00:58:47");
    }

    #[test]
    fn wrapping_breaks_at_words() {
        assert_eq!(wrap("Jede Woche Politik aus Berlin", 12), vec!["Jede Woche", "Politik aus", "Berlin"]);
    }

    #[test]
    fn the_window_follows_the_selection() {
        assert_eq!(window(0, 100, 10, 1), 0);
        assert_eq!(window(50, 100, 10, 1), 45);
        assert_eq!(window(99, 100, 10, 1), 90);
        assert_eq!(window(3, 5, 10, 1), 0, "everything fits");
        assert_eq!(window(4, 10, 9, 3), 3, "three-row entries: three fit");
    }
}
