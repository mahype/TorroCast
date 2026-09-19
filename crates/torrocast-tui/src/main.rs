use std::collections::HashMap;
use std::io::stdout;
use std::sync::Arc;
use std::time::{Duration, Instant};

use ratatui::crossterm::event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyEventKind};
use ratatui::crossterm::execute;
use torrocast_core::keeper::Keeper;
use torrocast_core::settings::{
    Platform, cache_dir, config_file, default_download_dir, default_library_dir, socket_file,
};
use torrocast_core::{Command, Core, Event as CoreEvent, OutputKind, Settings, Transport};
use torrocast_net::HttpClient;
use torrocast_tui::app::App;
use torrocast_tui::i18n::Lang;
use torrocast_tui::ui;

mod cli;

/// Short enough that an answer from the core shows up without a key press.
/// How often the subscribed feeds are fetched again while the program runs.
const REFRESH_EVERY: Duration = Duration::from_secs(30 * 60);
const POLL: Duration = Duration::from_millis(50);

/// Everything a TorroCast that plays needs, whether it draws a terminal interface or not.
struct Started {
    environment: HashMap<String, String>,
    lang: Lang,
    file: Option<std::path::PathBuf>,
    settings: Settings,
    library: Result<String, String>,
    core: Core,
    events: std::sync::mpsc::Receiver<CoreEvent>,
}

fn locale_of(environment: &HashMap<String, String>) -> String {
    ["LC_ALL", "LC_MESSAGES", "LANG"]
        .iter()
        .filter_map(|name| environment.get(*name))
        .find(|value| !value.is_empty())
        .cloned()
        .unwrap_or_default()
}

fn start(environment: HashMap<String, String>) -> Started {
    let locale = locale_of(&environment);
    // Without a place for the file the settings last for this run only.
    let file = config_file(Platform::current(), &environment);
    let mut settings =
        file.as_deref().map_or_else(|| Settings::for_locale(&locale), |file| Settings::load(file, &locale));

    // `TORROCAST_OUTPUT=muted` plays without a sound card, in real time — for trying things out in silence.
    let output = match environment.get("TORROCAST_OUTPUT").map(String::as_str) {
        Some("muted") => OutputKind::Muted,
        _ => OutputKind::Device,
    };
    // The library: in the folder the user chose, or in the platform's place for data.
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
    let downloads = settings
        .download_dir
        .clone()
        .map(std::path::PathBuf::from)
        .or_else(|| default_download_dir(Platform::current(), &environment));
    if let Some(directory) = cache_dir(Platform::current(), &environment) {
        core.set_cache_directory(&directory);
    }
    if let Some(directory) = &downloads {
        core.set_download_directory(directory);
    }
    // The first look at what the subscriptions have published.
    core.send(Command::RefreshSubscriptions);
    Started { lang: Lang::from_locale(&locale), environment, file, settings, library, core, events }
}

fn main() -> std::io::Result<()> {
    let environment: HashMap<String, String> = std::env::vars().collect();
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let code = match arguments.first().map(String::as_str) {
        None => return run_tui(environment),
        Some("--version" | "-V") => {
            println!("torrocast {}", ui::VERSION);
            0
        }
        Some("daemon") => cli::daemon(environment),
        Some("status") => cli::status(&environment),
        Some("ctl") => cli::ctl(&environment, &arguments[1..]),
        Some("--help" | "-h" | "help") => {
            print!("{}", cli::HELP);
            0
        }
        Some(other) => {
            eprintln!("torrocast: unknown argument “{other}”\n\n{}", cli::HELP);
            2
        }
    };
    std::process::exit(code)
}

fn run_tui(environment: HashMap<String, String>) -> std::io::Result<()> {
    // One TorroCast plays at a time. One without a window makes room and hands over what it played;
    // another terminal interface is left alone.
    let socket = socket_file(Platform::current(), &environment);
    let lang = Lang::from_locale(&locale_of(&environment));
    let (remote, resume) = match socket.as_deref().map(|socket| cli::take_over(socket, lang)) {
        Some(Ok(taken)) => taken,
        Some(Err(sentence)) => {
            eprintln!("{sentence}");
            std::process::exit(1)
        }
        None => (None, false),
    };
    let mut remote = remote;
    let Started { environment, lang, file, settings, library, mut core, events } = start(environment);
    if resume {
        core.send(Command::Transport(Transport::PlayQueued(0)));
    }
    let mut app = App::new(lang, settings);
    app.library = library;

    let mut terminal = ratatui::init();
    // What pictures the terminal can show is read from its environment and its size — see `covers::protocol_of`.
    let window = ratatui::crossterm::terminal::window_size().ok();
    let pixels = window.as_ref().map_or((0, 0), |window| (window.width, window.height));
    let cells = window.as_ref().map_or((0, 0), |window| (window.columns, window.rows));
    app.covers =
        torrocast_tui::covers::Covers::new(Some(torrocast_tui::covers::picker_for(&environment, pixels, cells)));
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
            core.send(Command::RefreshSubscriptions);
        }
        core.pump();
        while let Ok(event) = events.try_recv() {
            if let Some(remote) = &mut remote {
                remote.view.observe(&event);
            }
            app.on_event(event);
        }
        // A widget or a script may ask what plays, steer it, or ask this window to close.
        if remote.as_mut().is_some_and(|remote| remote.serve(&mut core)) {
            app.should_quit = true;
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
    drop(remote);
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
