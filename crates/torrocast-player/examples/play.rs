//! Plays an address or a file: `cargo run -p torrocast-player --example play -- <url> [volume] [speed] [seconds] [start-ms]`

use std::time::{Duration, Instant};

use torrocast_player::{Media, OutputKind, Player, PlayerEvent};

fn main() {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let Some(target) = arguments.first() else {
        eprintln!("usage: play <url-or-file> [volume 0..1] [speed] [seconds] [start-ms]");
        return;
    };
    let number =
        |index: usize, default: f32| arguments.get(index).and_then(|value| value.parse().ok()).unwrap_or(default);
    let media = if target.starts_with("http") { Media::Url(target.clone()) } else { Media::File(target.into()) };

    let (player, events) = Player::new(OutputKind::Device);
    player.set_volume(number(1, 0.5));
    player.set_speed(number(2, 1.0));
    player.load(media, number(4, 0.0) as u64);

    let stop_at = Instant::now() + Duration::from_secs_f32(number(3, 10.0));
    let mut levels = 0;
    while Instant::now() < stop_at {
        match events.recv_timeout(Duration::from_millis(500)) {
            Ok(PlayerEvent::Level(_)) => levels += 1,
            Ok(event @ (PlayerEvent::Ended | PlayerEvent::Failed(_))) => {
                println!("{event:?}");
                break;
            }
            Ok(event) => println!("{event:?}"),
            Err(_) => {}
        }
    }
    println!("levels received: {levels}");
}
