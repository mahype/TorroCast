//! TorroCast without its terminal interface: the player that runs in the background, and the two commands
//! that ask and steer whichever TorroCast is running. A desktop widget is built from exactly these.

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use serde_json::{Value, json};
use torrocast_core::remote::{self, Mode, OpenError, Remote};
use torrocast_core::settings::{Platform, socket_file};
use torrocast_core::{Command, Event};
use torrocast_tui::i18n::Lang;
use torrocast_tui::ui::VERSION;

pub const HELP: &str = "\
torrocast                 the terminal interface
torrocast daemon          play without a window, steered by `ctl` and the desktop's media keys
torrocast status          what plays, Up Next and the new episodes, as JSON
torrocast ctl <verb>      steer the TorroCast that is running:
    start                 start one without a window, unless one runs already
    play [key]            resume, or play the episode with this key (starts TorroCast if need be)
    pause | toggle | stop | next | next-chapter | previous-chapter | sleep
    seek <seconds>        forward, or back with a minus
    seek-to <ms>
    speed <step>          0.1 is one step faster, -0.1 one slower
    enqueue <key> [--first] | remove <key> | up <key> | down <key>
    refresh               fetch the subscribed feeds again
    quit                  save the place and end
Keys are the `key` fields of `torrocast status`.
";

/// How long a TorroCast that was just started, or asked to leave, is waited for.
const PATIENCE: Duration = Duration::from_secs(5);

fn code(lang: Lang) -> &'static str {
    match lang {
        Lang::De => "de",
        Lang::En => "en",
    }
}

/// The socket for the terminal interface. A TorroCast without a window makes room; `true` then says it was
/// playing, and the interface carries on where it stopped. Another interface is left alone: `Err` is the
/// sentence to leave with. Without a socket the interface runs all the same, only unheard from outside.
pub fn take_over(socket: &Path, lang: Lang) -> Result<(Option<Remote>, bool), String> {
    match Remote::open(socket, Mode::Tui, VERSION, code(lang)) {
        Ok(remote) => Ok((Some(remote), false)),
        Err(OpenError::Io(_)) => Ok((None, false)),
        Err(OpenError::Taken(status)) if status["mode"] == "daemon" => {
            let resume = matches!(status["now"]["status"].as_str(), Some("playing" | "loading"));
            let _ = remote::ask(socket, &json!({ "do": "quit" }));
            let asked = Instant::now();
            while remote::ask(socket, &json!({ "do": "status" })).is_ok() && asked.elapsed() < PATIENCE {
                std::thread::sleep(Duration::from_millis(50));
            }
            match Remote::open(socket, Mode::Tui, VERSION, code(lang)) {
                Ok(remote) => Ok((Some(remote), resume)),
                Err(_) => Ok((None, false)),
            }
        }
        Err(OpenError::Taken(_)) => Err(lang.t("TorroCast is open in another window already.").to_owned()),
    }
}

/// The player without a window. Runs until it is asked to quit, or told to by the system.
pub fn daemon(environment: HashMap<String, String>) -> i32 {
    let lang = Lang::from_locale(&super::locale_of(&environment));
    let Some(socket) = socket_file(Platform::current(), &environment) else {
        eprintln!("torrocast: no place for the socket");
        return 1;
    };
    // The socket first: whoever holds it owns the library's journal and the sound card.
    let mut remote = match Remote::open(&socket, Mode::Daemon, VERSION, code(lang)) {
        Ok(remote) => remote,
        Err(OpenError::Taken(_)) => {
            eprintln!("{}", lang.t("TorroCast is running already."));
            return 1;
        }
        Err(OpenError::Io(error)) => {
            eprintln!("torrocast: {error}");
            return 1;
        }
    };
    // Logging out or `kill` must not cost the place in the episode.
    let ending = Arc::new(AtomicBool::new(false));
    {
        let ending = Arc::clone(&ending);
        let _ = ctrlc::set_handler(move || ending.store(true, Ordering::Relaxed));
    }

    let super::Started { mut core, events, .. } = super::start(environment);
    let mut refreshed = Instant::now();
    let mut playing = false;
    while !ending.load(Ordering::Relaxed) {
        core.pump();
        while let Ok(event) = events.try_recv() {
            if let Event::Playback(state) = &event {
                playing = state.now.is_some();
            }
            remote.view.observe(&event);
        }
        if remote.serve(&mut core) {
            break;
        }
        if refreshed.elapsed() >= super::REFRESH_EVERY {
            refreshed = Instant::now();
            core.send(Command::RefreshSubscriptions);
        }
        // With nothing in the player there is only the socket to look after.
        std::thread::sleep(if playing { super::POLL } else { Duration::from_millis(150) });
    }
    core.shutdown();
    0
}

pub fn status(environment: &HashMap<String, String>) -> i32 {
    let lang = Lang::from_locale(&super::locale_of(environment));
    let answer = socket_file(Platform::current(), environment)
        .and_then(|socket| remote::ask(&socket, &json!({ "do": "status" })).ok())
        .unwrap_or_else(|| remote::not_running(VERSION, code(lang)));
    println!("{answer}");
    0
}

/// The request a `ctl` line stands for.
fn request(arguments: &[String]) -> Result<Value, String> {
    let verb = arguments.first().map(String::as_str).ok_or("ctl needs a verb; see torrocast --help")?;
    let word = |what: &str| arguments.get(1).cloned().ok_or_else(|| format!("“{verb}” needs {what}"));
    let number = |what: &str| word(what)?.parse::<f64>().map_err(|_| format!("“{verb}” needs {what}"));
    Ok(match verb {
        "start" => json!({ "do": "status" }),
        "play" => match arguments.get(1) {
            Some(key) => json!({ "do": "play", "key": key }),
            None => json!({ "do": "play" }),
        },
        "seek" => json!({ "do": "seek-by", "seconds": number("seconds")? as i64 }),
        "seek-to" => json!({ "do": "seek-to", "ms": number("milliseconds")?.max(0.0) as u64 }),
        "speed" => json!({ "do": "speed-by", "step": number("a step such as 0.1")? }),
        "enqueue" => {
            let first = arguments.iter().any(|argument| argument == "--first");
            let key = arguments[1..].iter().find(|argument| *argument != "--first");
            json!({ "do": "enqueue", "key": key.ok_or("“enqueue” needs a key")?, "first": first })
        }
        "remove" | "up" | "down" => json!({ "do": verb, "key": word("a key")? }),
        _ => json!({ "do": verb }),
    })
}

fn start_daemon(socket: &Path) -> bool {
    let Ok(program) = std::env::current_exe() else { return false };
    let mut command = std::process::Command::new(program);
    command
        .arg("daemon")
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    // Its own process group: closing the terminal or the widget that started it does not take it along.
    #[cfg(unix)]
    std::os::unix::process::CommandExt::process_group(&mut command, 0);
    if command.spawn().is_err() {
        return false;
    }
    let started = Instant::now();
    while started.elapsed() < PATIENCE {
        if remote::ask(socket, &json!({ "do": "status" })).is_ok() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    false
}

pub fn ctl(environment: &HashMap<String, String>, arguments: &[String]) -> i32 {
    let request = match request(arguments) {
        Ok(request) => request,
        Err(sentence) => {
            eprintln!("torrocast: {sentence}");
            return 2;
        }
    };
    let Some(socket) = socket_file(Platform::current(), environment) else { return 1 };
    let mut answer = remote::ask(&socket, &request);
    // Asking for sound is reason enough to start; asking for silence is not.
    let starts = matches!(arguments.first().map(String::as_str), Some("start" | "play" | "toggle"));
    if answer.is_err() && starts && start_daemon(&socket) {
        answer = remote::ask(&socket, &request);
    }
    match answer {
        Ok(answer) => {
            println!("{answer}");
            i32::from(answer.get("error").is_some())
        }
        Err(_) => {
            println!("{}", remote::not_running(VERSION, "en"));
            3
        }
    }
}

#[cfg(test)]
mod tests {
    use super::request;
    use serde_json::json;

    fn line(words: &[&str]) -> Vec<String> {
        words.iter().map(|word| (*word).to_owned()).collect()
    }

    #[test]
    fn ctl_lines_become_requests() {
        assert_eq!(request(&line(&["toggle"])), Ok(json!({"do": "toggle"})));
        assert_eq!(request(&line(&["play", "k"])), Ok(json!({"do": "play", "key": "k"})));
        assert_eq!(request(&line(&["seek", "-30"])), Ok(json!({"do": "seek-by", "seconds": -30})));
        assert_eq!(request(&line(&["speed", "0.1"])), Ok(json!({"do": "speed-by", "step": 0.1})));
        assert_eq!(
            request(&line(&["enqueue", "--first", "k"])),
            Ok(json!({"do": "enqueue", "key": "k", "first": true}))
        );
        assert!(request(&line(&["seek"])).is_err());
        assert!(request(&line(&["remove"])).is_err());
        assert!(request(&[]).is_err());
    }
}
