//! New episodes: what the subscriptions published lately and is still unheard.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use super::discover::queue_label;
use super::{EpisodeRow, empty, episode_rows, panel};
use crate::app::App;
use crate::text::{date, duration};
use crate::theme;

pub fn draw(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let lang = app.lang;
    let episodes = &app.new_episodes;
    let mut title = if episodes.is_empty() {
        lang.t("New Episodes").to_owned()
    } else {
        format!("{} · {}", lang.t("New Episodes"), episodes.len())
    };
    if app.refresh_pending > 0 {
        title.push_str(&format!(" · {} {}", app.refresh_pending, lang.t("still loading feeds")));
    } else if app.refresh_failed > 0 {
        title.push_str(&format!(" · {} {}", app.refresh_failed, lang.t("feeds did not answer")));
    }
    let block = panel(&title, true);
    let mut inner = block.inner(area);
    frame.render_widget(block, area);

    if episodes.is_empty() {
        let sentence = if app.subscriptions.is_empty() {
            "New episodes of your subscriptions appear here. Subscribe to a podcast with s first."
        } else if app.refresh_pending > 0 {
            "Looking through your subscriptions …"
        } else {
            "Nothing new in the last two weeks. r looks again."
        };
        empty(frame, inner, &[lang.t(sentence)]);
        return;
    }
    if let Some(notice) = &app.notice {
        let row =
            Rect { x: inner.x + 1, y: inner.y + inner.height - 1, width: inner.width.saturating_sub(2), height: 1 };
        let line = Line::from(vec![
            Span::styled("✓ ", Style::new().fg(theme::GREEN)),
            Span::styled(notice.clone(), theme::muted()),
        ]);
        frame.render_widget(Paragraph::new(line), row);
        inner.height = inner.height.saturating_sub(2);
    }

    let rows: Vec<EpisodeRow<'_>> = episodes
        .iter()
        .map(|episode| {
            let mut facts = vec![episode.item.podcast.clone(), date(lang, episode.published)];
            facts.extend(episode.item.duration_ms.map(|milliseconds| duration((milliseconds / 1000) as u32)));
            EpisodeRow {
                item: &episode.item,
                number: None,
                state: queue_label(app, &episode.item.key()),
                facts: facts.join(" · "),
            }
        })
        .collect();
    episode_rows(frame, inner, app, app.new_index, &rows);
}
