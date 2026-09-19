//! The whole chain — file, decoder, tempo, output — against a generated tone,
//! with an output that swallows audio as fast as it arrives.

use std::io::Write;
use std::path::PathBuf;
use std::sync::mpsc::Receiver;
use std::time::Duration;

use torrocast_player::{Media, OutputKind, Player, PlayerEvent};

const RATE: u32 = 22_050;

/// Ten seconds of a 300 Hz tone as a 16-bit mono WAV.
fn tone_file(name: &str) -> PathBuf {
    let samples: Vec<i16> = (0..RATE * 10)
        .map(|n| ((std::f32::consts::TAU * 300.0 * n as f32 / RATE as f32).sin() * 12_000.0) as i16)
        .collect();
    let data_bytes = (samples.len() * 2) as u32;
    let mut wav = Vec::new();
    wav.extend_from_slice(b"RIFF");
    wav.extend_from_slice(&(36 + data_bytes).to_le_bytes());
    wav.extend_from_slice(b"WAVEfmt ");
    wav.extend_from_slice(&16u32.to_le_bytes());
    wav.extend_from_slice(&1u16.to_le_bytes()); // PCM
    wav.extend_from_slice(&1u16.to_le_bytes()); // mono
    wav.extend_from_slice(&RATE.to_le_bytes());
    wav.extend_from_slice(&(RATE * 2).to_le_bytes());
    wav.extend_from_slice(&2u16.to_le_bytes());
    wav.extend_from_slice(&16u16.to_le_bytes());
    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&data_bytes.to_le_bytes());
    for sample in samples {
        wav.extend_from_slice(&sample.to_le_bytes());
    }
    let path = std::env::temp_dir().join(format!("torrocast-{}-{name}.wav", std::process::id()));
    std::fs::File::create(&path).and_then(|mut file| file.write_all(&wav)).expect("temp dir is writable");
    path
}

fn until(events: &Receiver<PlayerEvent>, wanted: impl Fn(&PlayerEvent) -> bool) -> Vec<PlayerEvent> {
    let mut seen = Vec::new();
    loop {
        let event = events.recv_timeout(Duration::from_secs(20)).expect("the player keeps talking");
        let done = wanted(&event);
        seen.push(event);
        if done {
            return seen;
        }
    }
}

#[test]
fn plays_a_file_to_its_end() {
    let path = tone_file("end");
    let (player, events) = Player::new(OutputKind::Null);
    player.load(Media::File(path.clone()), 0);
    let seen = until(&events, |event| *event == PlayerEvent::Ended);

    assert_eq!(seen[0], PlayerEvent::Loading);
    assert!(matches!(seen[1], PlayerEvent::Started { duration_ms: Some(10_000) }), "got {:?}", seen[1]);
    let levels: Vec<f32> = seen
        .iter()
        .filter_map(|event| if let PlayerEvent::Level(level) = event { Some(*level) } else { None })
        .collect();
    assert!(levels.len() > 100, "about twenty levels a second, got {}", levels.len());
    // A sine at 12000/32768 has an RMS of 0.26.
    assert!(levels.iter().all(|level| (0.2..0.32).contains(level)), "levels follow the audio: {:?}", &levels[..5]);
    let _ = std::fs::remove_file(path);
}

#[test]
fn starts_where_it_is_told_and_seeks() {
    let path = tone_file("seek");
    let (player, events) = Player::new(OutputKind::Null);
    player.pause();
    player.load(Media::File(path.clone()), 6_000);
    until(&events, |event| matches!(event, PlayerEvent::Started { .. }));
    // Loading un-pauses; hold it again so the positions can be inspected.
    player.pause();
    player.seek(2_000);
    let seen = until(
        &events,
        |event| matches!(event, PlayerEvent::Position { position_ms, .. } if (1_900..=2_100).contains(position_ms)),
    );
    assert!(!seen.is_empty());
    player.resume();
    until(&events, |event| *event == PlayerEvent::Ended);
    let _ = std::fs::remove_file(path);
}

#[test]
fn a_missing_file_is_a_failure_not_a_crash() {
    let (player, events) = Player::new(OutputKind::Null);
    player.load(Media::File("/nonexistent/episode.mp3".into()), 0);
    let seen = until(&events, |event| matches!(event, PlayerEvent::Failed(_)));
    assert_eq!(seen[0], PlayerEvent::Loading);
    // And the player is still there for the next episode.
    let path = tone_file("after-failure");
    player.set_speed(2.0);
    player.load(Media::File(path.clone()), 0);
    until(&events, |event| *event == PlayerEvent::Ended);
    let _ = std::fs::remove_file(path);
}
