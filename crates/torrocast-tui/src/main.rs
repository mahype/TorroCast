use std::collections::HashMap;
use std::io::stdout;
use std::sync::Arc;
use std::time::{Duration, Instant};

use ratatui::crossterm::event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyEventKind};
use ratatui::crossterm::execute;
use torrocast_core::keeper::Keeper;
use torrocast_core::settings::{Platform, config_file, default_library_dir};
use torrocast_core::{Core, OutputKind, Settings};
use torrocast_net::HttpClient;
use torrocast_tui::app::App;
use torrocast_tui::i18n::Lang;
use torrocast_tui::ui;

/// Short enough that an answer from the core shows up without a key press.
/// How often the subscribed feeds are fetched again while the program runs.
const REFRESH_EVERY: Duration = Duration::from_secs(30 * 60);
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
    // The library: in the folder the user chose, or in the platform's place for data.
    let mut settings = settings;
    if settings.device_id.is_none() {
        settings.device_id = Some(torrocast_core::keeper::new_device_id());
        if let Some(file) = file.as_deref() {
            let _ = settings.save(file);
        }
    }
    let directory = settings
        .library_dir
        .clone()
        .map(std::path::PathBuf::from)
        .or_else(|| default_library_dir(Platform::current(), &environment));
    let keeper = match (&directory, &settings.device_id) {
        (Some(directory), Some(device)) => {
            Keeper::open(directory, device, &device_name(&environment)).map_err(|error| error.to_string())
        }
        _ => Err(String::new()),
    };
    let library = match &keeper {
        Ok(keeper) => Ok(keeper.directory().display().to_string()),
        Err(reason) => Err(reason.clone()),
    };

    let (mut core, events) = Core::new(Arc::new(HttpClient::new()), settings.clone(), output, keeper.ok());
    let mut app = App::new(Lang::from_locale(&locale), settings);
    app.library = library;
    // The first look at what the subscriptions have published.
    core.send(torrocast_core::Command::RefreshSubscriptions);

    let mut terminal = ratatui::init();
    // The mouse is a convenience; a terminal that refuses it still works.
    let _ = execute!(stdout(), EnableMouseCapture);
    let mut refreshed = Instant::now();
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
        if refreshed.elapsed() >= REFRESH_EVERY {
            refreshed = Instant::now();
            core.send(torrocast_core::Command::RefreshSubscriptions);
        }
        core.pump();
        while let Ok(event) = events.try_recv() {
            app.on_event(event);
        }

        if let Some(wanted) = app.library_request.take() {
            // `~` is how people write their home folder; the file system does not know it.
            let home = environment.get("HOME").or_else(|| environment.get("USERPROFILE")).cloned().unwrap_or_default();
            let wanted = wanted.trim();
            let folder = match wanted.strip_prefix('~') {
                Some(rest) => format!("{home}{rest}"),
                None => wanted.to_owned(),
            };
            match core.move_library(std::path::Path::new(&folder)) {
                Ok(()) => {
                    app.settings.library_dir = Some(folder.clone());
                    app.settings_changed = true;
                    app.notice = Some(format!("{} {folder}.", app.lang.t("The library now lives in")));
                    app.library = Ok(folder);
                }
                Err(reason) => {
                    app.notice = Some(format!("{} {reason}", app.lang.t("The library could not be moved there:")))
                }
            }
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
    core.shutdown();
    let _ = execute!(stdout(), DisableMouseCapture);
    ratatui::restore();
    outcome
}

/// What this device is called in the library, for people reading the folder.
fn device_name(environment: &HashMap<String, String>) -> String {
    environment
        .get("HOSTNAME")
        .or_else(|| environment.get("COMPUTERNAME"))
        .cloned()
        .or_else(|| std::fs::read_to_string("/etc/hostname").ok())
        .map(|name| name.trim().to_owned())
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| std::env::consts::OS.to_owned())
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
