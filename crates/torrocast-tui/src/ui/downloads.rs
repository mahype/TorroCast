//! Episodes kept on this machine.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Style;
use torrocast_core::DownloadState;

use super::discover::queue_label;
use super::{EpisodeRow, empty, episode_rows, panel};
use crate::app::App;
use crate::text::duration;
use crate::theme;

fn megabytes(bytes: u64) -> String {
    format!("{} MB", bytes.div_ceil(1_000_000))
}

pub fn draw(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let lang = app.lang;
    let downloads = &app.downloads;
    let total: u64 = downloads
        .iter()
        .map(|download| if let DownloadState::Done { bytes } = download.state { bytes } else { 0 })
        .sum();
    let title = if downloads.is_empty() {
        lang.t("Downloads").to_owned()
    } else {
        format!("{} · {} · {}", lang.t("Downloads"), downloads.len(), megabytes(total))
    };
    let block = panel(&title, true);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    if downloads.is_empty() {
        empty(
            frame,
            inner,
            &[lang.t("Nothing downloaded. D on an episode keeps it on this machine, to be heard without a network.")],
        );
        return;
    }

    let rows: Vec<EpisodeRow<'_>> = downloads
        .iter()
        .map(|download| {
            let state = match download.state {
                DownloadState::Loading { received, total: Some(total) } if total > 0 => {
                    (format!("↓ {} %", received * 100 / total), Style::new().fg(theme::SILVER))
                }
                DownloadState::Loading { received, .. } => {
                    (format!("↓ {}", megabytes(received)), Style::new().fg(theme::SILVER))
                }
                DownloadState::Failed => (lang.t("failed — D tries again").to_owned(), Style::new().fg(theme::AMBER)),
                DownloadState::Done { bytes } => match queue_label(app, &download.item.key()) {
                    (label, style) if !label.is_empty() => (label, style),
                    _ => (megabytes(bytes), theme::muted()),
                },
            };
            let mut facts = vec![download.item.podcast.clone()];
            facts.extend(download.item.duration_ms.map(|milliseconds| duration((milliseconds / 1000) as u32)));
            EpisodeRow { item: &download.item, number: None, state, facts: facts.join(" · ") }
        })
        .collect();
    episode_rows(frame, inner, app, app.downloads_index, &rows);
}
