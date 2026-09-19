use std::collections::HashMap;
use std::io::stdout;
use std::sync::Arc;
use std::time::{Duration, Instant};

use ratatui::crossterm::event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyEventKind};
use ratatui::crossterm::execute;
use torrocast_core::settings::{Platform, config_file};
use torrocast_core::{Core, OutputKind, Settings};
use torrocast_net::HttpClient;
use torrocast_tui::app::App;
use torrocast_tui::i18n::Lang;
use torrocast_tui::ui;

/// Short enough that an answer from the core shows up without a key press.
const POLL: Duration = Duration::from_millis(50);

fn main() -> std::io::Result<()> {
    if std::env::args().any(|argument| argument == "--version" || argument == "-V") {
        println!("torrocast {}", ui::VERSION);
        return Ok(());
    }
    let environment: HashMap<String, String> = std::env::vars().collect();
    let locale = ["LC_ALL", "LC_MESSAGES", "LANG"]
        .iter()
        .filter_map(|name| environment.get(*name))
        .find(|value| !value.is_empty())
        .cloned()
        .unwrap_or_default();
    // Without a place for the file the settings last for this run only.
    let file = config_file(Platform::current(), &environment);
    let settings = file.as_deref().map_or_else(|| Settings::for_locale(&locale), |file| Settings::load(file, &locale));

    // `TORROCAST_OUTPUT=muted` plays without a sound card, in real time — for trying things out in silence.
    let output = match environment.get("TORROCAST_OUTPUT").map(String::as_str) {
        Some("muted") => OutputKind::Muted,
        _ => OutputKind::Device,
    };
    let (mut core, events) = Core::new(Arc::new(HttpClient::new()), settings.clone(), output);
    let mut app = App::new(Lang::from_locale(&locale), settings);

    let mut terminal = ratatui::init();
    // The mouse is a convenience; a terminal that refuses it still works.
    let _ = execute!(stdout(), EnableMouseCapture);
    let mut shown = app.screen();
    let outcome = loop {
        // Terminals disagree with us about the width of some emoji, and what
        // they drew too wide outlives the screen it belonged to. A new screen
        // therefore starts from a cleared terminal.
        if app.screen() != shown {
            shown = app.screen();
            if let Err(error) = terminal.clear() {
                break Err(error);
            }
        }
        app.terminal_height = terminal.size().map_or(0, |size| size.height);
        if let Err(error) = terminal.draw(|frame| ui::draw(frame, &app)) {
            break Err(error);
        }
        match event::poll(POLL) {
            // Windows reports the release of a key as well; only the press counts.
            Ok(true) => match event::read() {
                Ok(Event::Key(key)) if key.kind == KeyEventKind::Press => app.on_key(key),
                Ok(Event::Mouse(mouse)) => app.on_mouse(mouse),
                Ok(_) => {}
                Err(error) => break Err(error),
            },
            Ok(false) => {}
            Err(error) => break Err(error),
        }
        app.tick(Instant::now());
        core.pump();
        while let Ok(event) = events.try_recv() {
            app.on_event(event);
        }

        if app.settings_changed {
            app.settings_changed = false;
            core.set_settings(app.settings.clone());
            app.providers = core.active_providers();
            let saved = file.as_deref().is_none_or(|file| app.settings.save(file).is_ok());
            if !saved {
                app.notice = Some(app.lang.t("The settings could not be saved.").to_owned());
            }
        }
        for command in app.commands.drain(..) {
            core.send(command);
        }
        for url in app.open_urls.drain(..) {
            open_in_browser(&url);
        }
        if app.should_quit {
            break Ok(());
        }
    };
    let _ = execute!(stdout(), DisableMouseCapture);
    ratatui::restore();
    outcome
}

/// Hands an address to the system. Only web addresses: a feed is someone
/// else's text and must not be able to start anything else.
fn open_in_browser(url: &str) {
    if !(url.starts_with("https://") || url.starts_with("http://")) {
        return;
    }
    let mut command = if cfg!(target_os = "macos") {
        std::process::Command::new("open")
    } else if cfg!(target_os = "windows") {
        let mut command = std::process::Command::new("rundll32");
        command.arg("url.dll,FileProtocolHandler");
        command
    } else {
        std::process::Command::new("xdg-open")
    };
    let _ = command
        .arg(url)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn();
}
