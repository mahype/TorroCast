//! Playback. A [`Player`] takes commands and reports [`PlayerEvent`]s; one
//! thread behind it streams, decodes, keeps the tempo and feeds the sound card.
//! No user interface, no knowledge of podcasts — only of audio.

mod decode;
mod output;
mod source;
pub mod stretch;

use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::time::{Duration, Instant};

use decode::Decoder;
use output::{Controls, Output};
use stretch::Stretcher;

pub use output::OutputKind;
pub use stretch::{MAX_SPEED, MIN_SPEED};

const POSITION_EVERY: Duration = Duration::from_millis(200);
/// One level value per this much audio: twenty bars a second.
const LEVEL_SPAN_MS: u64 = 50;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Media {
    Url(String),
    File(PathBuf),
}

#[derive(Debug, Clone, PartialEq)]
pub enum PlayerEvent {
    /// Connecting and reading the first bytes.
    Loading,
    Started {
        duration_ms: Option<u64>,
    },
    Position {
        position_ms: u64,
        duration_ms: Option<u64>,
    },
    /// How loud the audio being heard right now is, 0 to 1.
    Level(f32),
    Paused,
    Resumed,
    /// The episode played to its end.
    Ended,
    Stopped,
    Failed(String),
}

enum Command {
    Load { media: Media, start_ms: u64 },
    Pause,
    Resume,
    Seek(u64),
    Speed(f32),
    Stop,
}

pub struct Player {
    commands: Sender<Command>,
    controls: Arc<Controls>,
}

impl Player {
    /// The player and the channel its events arrive on.
    #[must_use]
    pub fn new(kind: OutputKind) -> (Self, Receiver<PlayerEvent>) {
        let (commands, inbox) = channel();
        let (events, receiver) = channel();
        let controls = Arc::new(Controls::default());
        let shared = Arc::clone(&controls);
        std::thread::spawn(move || Engine { kind, controls: shared, events, speed: 1.0, session: None }.run(&inbox));
        (Self { commands, controls }, receiver)
    }

    fn send(&self, command: Command) {
        // The thread only ends with the player; a failed send has nobody to tell.
        let _ = self.commands.send(command);
    }

    pub fn load(&self, media: Media, start_ms: u64) {
        self.send(Command::Load { media, start_ms });
    }

    /// Takes effect in the audio callback at once, even while the thread waits for the network.
    pub fn pause(&self) {
        self.controls.paused.store(true, Ordering::Relaxed);
        self.send(Command::Pause);
    }

    pub fn resume(&self) {
        self.controls.paused.store(false, Ordering::Relaxed);
        self.send(Command::Resume);
    }

    pub fn seek(&self, position_ms: u64) {
        self.send(Command::Seek(position_ms));
    }

    pub fn set_speed(&self, speed: f32) {
        self.send(Command::Speed(speed));
    }

    pub fn set_volume(&self, volume: f32) {
        self.controls.set_volume(volume);
    }

    pub fn stop(&self) {
        self.send(Command::Stop);
    }
}

/// Straight-line interpolation. Only used when the device cannot run at the
/// episode's own rate; for speech the difference is not audible.
struct Resampler {
    /// Input frames per output frame.
    step: f64,
    phase: f64,
    last: Vec<f32>,
}

impl Resampler {
    fn process(&mut self, input: &[f32], channels: usize, output: &mut Vec<f32>) {
        let frames = input.len() / channels;
        let frame = |index: isize, channel: usize, last: &[f32]| {
            if index < 0 { last[channel] } else { input[index as usize * channels + channel] }
        };
        while self.phase < frames as f64 {
            let index = self.phase.floor() as isize - 1;
            let fraction = (self.phase - self.phase.floor()) as f32;
            for channel in 0..channels {
                let (before, after) = (frame(index, channel, &self.last), frame(index + 1, channel, &self.last));
                output.push(before + (after - before) * fraction);
            }
            self.phase += self.step;
        }
        self.phase -= frames as f64;
        if frames > 0 {
            self.last.copy_from_slice(&input[(frames - 1) * channels..]);
        }
    }
}

struct Session {
    decoder: Decoder,
    output: Output,
    stretcher: Stretcher,
    resampler: Option<Resampler>,
    /// Device-ready samples the ring had no room for yet.
    pending: VecDeque<f32>,
    /// Where in the episode the decoded audio ends.
    decoded_ms: u64,
    ended: bool,
    /// Levels waiting for their moment: the ring plays half a second behind the decoder.
    levels: VecDeque<(u64, f32)>,
    level_sum: f32,
    level_count: usize,
    level_from_ms: u64,
    reported: Instant,
}

impl Session {
    /// The moment being heard: what was decoded, minus what still waits to be played.
    fn position_ms(&self, speed: f32) -> u64 {
        let waiting = self.output.capacity - self.output.producer.slots() + self.pending.len();
        let waiting_ms = waiting as f64 * 1000.0 / (self.output.sample_rate as f64 * self.output.channels as f64);
        self.decoded_ms.saturating_sub((waiting_ms * f64::from(speed)) as u64)
    }

    fn drained(&self) -> bool {
        self.pending.is_empty() && self.output.producer.slots() == self.output.capacity
    }
}

struct Engine {
    kind: OutputKind,
    controls: Arc<Controls>,
    events: Sender<PlayerEvent>,
    speed: f32,
    session: Option<Session>,
}

impl Engine {
    fn emit(&self, event: PlayerEvent) {
        let _ = self.events.send(event);
    }

    fn run(mut self, inbox: &Receiver<Command>) {
        loop {
            // Idle: wait for a command. Playing: look for one, then get on with the audio.
            let command = if self.session.is_some() {
                match inbox.try_recv() {
                    Ok(command) => Some(command),
                    Err(std::sync::mpsc::TryRecvError::Empty) => None,
                    Err(std::sync::mpsc::TryRecvError::Disconnected) => return,
                }
            } else {
                match inbox.recv() {
                    Ok(command) => Some(command),
                    Err(_) => return,
                }
            };
            if let Some(command) = command {
                self.handle(command);
                continue;
            }
            if !self.work() {
                std::thread::sleep(Duration::from_millis(4));
            }
        }
    }

    fn handle(&mut self, command: Command) {
        match command {
            Command::Load { media, start_ms } => {
                self.session = None;
                self.emit(PlayerEvent::Loading);
                match self.open(&media, start_ms) {
                    Ok(session) => {
                        self.controls.paused.store(false, Ordering::Relaxed);
                        self.emit(PlayerEvent::Started { duration_ms: session.decoder.duration_ms });
                        self.session = Some(session);
                    }
                    Err(reason) => self.emit(PlayerEvent::Failed(reason)),
                }
            }
            Command::Pause => self.emit(PlayerEvent::Paused),
            Command::Resume => self.emit(PlayerEvent::Resumed),
            Command::Speed(speed) => {
                self.speed = speed.clamp(MIN_SPEED, MAX_SPEED);
                if let Some(session) = &mut self.session {
                    session.stretcher.set_speed(self.speed);
                }
            }
            Command::Seek(position_ms) => self.seek(position_ms),
            Command::Stop => {
                self.session = None;
                self.emit(PlayerEvent::Stopped);
            }
        }
    }

    fn open(&self, media: &Media, start_ms: u64) -> Result<Session, String> {
        let (source, name): (Box<dyn symphonia::core::io::MediaSource>, &str) = match media {
            Media::Url(url) => {
                (Box::new(source::HttpSource::open(url).map_err(|error| error.to_string())?), url.as_str())
            }
            Media::File(path) => (
                Box::new(std::fs::File::open(path).map_err(|error| error.to_string())?),
                path.to_str().unwrap_or_default(),
            ),
        };
        let path = name.split(['?', '#']).next().unwrap_or(name);
        let extension = path
            .rsplit_once('.')
            .map(|(_, extension)| extension.to_lowercase())
            .filter(|extension| extension.len() <= 4);
        let mut decoder = Decoder::open(source, extension.as_deref())?;
        let output = Output::open(self.kind, decoder.sample_rate, Arc::clone(&self.controls))?;
        let decoded_ms = if start_ms > 0 { decoder.seek(start_ms).unwrap_or(0) } else { 0 };

        let mut stretcher = Stretcher::new(decoder.sample_rate, decoder.channels);
        stretcher.set_speed(self.speed);
        let resampler = (output.sample_rate != decoder.sample_rate).then(|| Resampler {
            step: f64::from(decoder.sample_rate) / f64::from(output.sample_rate),
            phase: 0.0,
            last: vec![0.0; decoder.channels],
        });
        Ok(Session {
            decoder,
            output,
            stretcher,
            resampler,
            pending: VecDeque::new(),
            decoded_ms,
            ended: false,
            levels: VecDeque::new(),
            level_sum: 0.0,
            level_count: 0,
            level_from_ms: decoded_ms,
            reported: Instant::now(),
        })
    }

    fn seek(&mut self, position_ms: u64) {
        let Some(session) = &mut self.session else { return };
        let target =
            session.decoder.duration_ms.map_or(position_ms, |duration| position_ms.min(duration.saturating_sub(1000)));
        match session.decoder.seek(target) {
            Ok(landed) => {
                // What is buffered belongs to the old place. Wait until the
                // callback has thrown it away, or new audio would go with it.
                self.controls.flush.store(true, Ordering::Release);
                let asked = Instant::now();
                while self.controls.flush.load(Ordering::Acquire) && asked.elapsed() < Duration::from_millis(250) {
                    std::thread::sleep(Duration::from_millis(2));
                }
                session.pending.clear();
                session.levels.clear();
                session.stretcher.reset();
                session.decoded_ms = landed;
                session.level_from_ms = landed;
                session.ended = false;
                let duration_ms = session.decoder.duration_ms;
                self.emit(PlayerEvent::Position { position_ms: landed, duration_ms });
            }
            Err(reason) => self.emit(PlayerEvent::Failed(reason)),
        }
    }

    /// One step of playback. `false` means there was nothing to do right now.
    fn work(&mut self) -> bool {
        let speed = self.speed;
        let Some(session) = &mut self.session else { return false };
        let mut busy = false;

        while session.output.producer.slots() > 0 {
            let Some(sample) = session.pending.pop_front() else { break };
            if session.output.producer.push(sample).is_err() {
                session.pending.push_front(sample);
                break;
            }
            busy = true;
        }

        let mut failure = None;
        if session.pending.is_empty() && !session.ended {
            busy = true;
            match session.decoder.next() {
                Ok(Some(piece)) => {
                    let source_channels = piece.channels;
                    if let Some(ends_at) = piece.ends_at_ms {
                        session.decoded_ms = ends_at;
                    }
                    for sample in piece.samples {
                        session.level_sum += sample * sample;
                    }
                    session.level_count += piece.samples.len();
                    if session.decoded_ms >= session.level_from_ms + LEVEL_SPAN_MS {
                        let rms = (session.level_sum / session.level_count.max(1) as f32).sqrt();
                        session.levels.push_back((session.level_from_ms, rms));
                        (session.level_sum, session.level_count, session.level_from_ms) = (0.0, 0, session.decoded_ms);
                    }

                    let mut stretched = Vec::with_capacity(piece.samples.len());
                    session.stretcher.process(piece.samples, &mut stretched);
                    let resampled = match &mut session.resampler {
                        Some(resampler) => {
                            let mut resampled = Vec::with_capacity(stretched.len());
                            resampler.process(&stretched, source_channels, &mut resampled);
                            resampled
                        }
                        None => stretched,
                    };
                    // Mono to every speaker, surround down to its front pair.
                    let device_channels = session.output.channels;
                    for frame in resampled.chunks_exact(source_channels) {
                        for channel in 0..device_channels {
                            let sample = match (source_channels, channel) {
                                (1, _) => frame[0],
                                (_, 0 | 1) => frame[channel],
                                _ => 0.0,
                            };
                            session.pending.push_back(sample);
                        }
                    }
                }
                Ok(None) => session.ended = true,
                Err(reason) => failure = Some(reason),
            }
        }

        let position_ms = session.position_ms(speed);
        let mut events = Vec::new();
        while session.levels.front().is_some_and(|(at_ms, _)| *at_ms <= position_ms) {
            events.extend(session.levels.pop_front().map(|(_, level)| PlayerEvent::Level(level)));
        }
        if session.reported.elapsed() >= POSITION_EVERY {
            session.reported = Instant::now();
            events.push(PlayerEvent::Position { position_ms, duration_ms: session.decoder.duration_ms });
        }
        let finished = session.ended && session.drained();
        for event in events {
            self.emit(event);
        }
        if let Some(reason) = failure {
            self.session = None;
            self.emit(PlayerEvent::Failed(reason));
        } else if finished {
            self.session = None;
            self.emit(PlayerEvent::Ended);
        }
        busy
    }
}
