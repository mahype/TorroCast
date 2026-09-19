//! The sound card, or a stand-in that swallows audio where there is none.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::time::Duration;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use rtrb::{Consumer, Producer, RingBuffer};

/// What the playback thread and the audio callback share.
pub struct Controls {
    pub paused: AtomicBool,
    /// Ask the consuming side to throw away what is buffered (after a seek).
    pub flush: AtomicBool,
    volume: AtomicU32,
}

impl Default for Controls {
    fn default() -> Self {
        Self { paused: AtomicBool::new(false), flush: AtomicBool::new(false), volume: AtomicU32::new(1.0f32.to_bits()) }
    }
}

impl Controls {
    pub fn set_volume(&self, volume: f32) {
        self.volume.store(volume.clamp(0.0, 1.0).to_bits(), Ordering::Relaxed);
    }

    fn volume(&self) -> f32 {
        f32::from_bits(self.volume.load(Ordering::Relaxed))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputKind {
    /// The system's default output device.
    Device,
    /// No sound: audio is consumed as fast as it comes. For tests.
    Null,
    /// No sound, but at the pace of real playback. For machines without a sound card.
    Muted,
}

pub struct Output {
    pub sample_rate: u32,
    pub channels: usize,
    pub producer: Producer<f32>,
    /// Samples the ring holds when full.
    pub capacity: usize,
    // Dropping the stream stops the sound; it lives as long as the output.
    _stream: Option<cpal::Stream>,
}

fn fill<T: cpal::SizedSample + cpal::FromSample<f32>>(
    data: &mut [T],
    consumer: &mut Consumer<f32>,
    controls: &Controls,
) {
    if controls.flush.swap(false, Ordering::AcqRel) {
        while consumer.pop().is_ok() {}
    }
    let paused = controls.paused.load(Ordering::Relaxed);
    let volume = controls.volume();
    for slot in data {
        // An empty ring is silence, not an error: the network was slower than the music.
        let sample = if paused { 0.0 } else { consumer.pop().unwrap_or(0.0) * volume };
        *slot = T::from_sample(sample);
    }
}

impl Output {
    /// Opens an output as close to `wanted_rate` as the device offers.
    pub fn open(kind: OutputKind, wanted_rate: u32, controls: Arc<Controls>) -> Result<Self, String> {
        if kind != OutputKind::Device {
            let (producer, mut consumer) = RingBuffer::new(wanted_rate as usize);
            let shared = Arc::clone(&controls);
            // Stereo samples per millisecond, when the pace is to be real.
            let per_tick = (kind == OutputKind::Muted).then_some(wanted_rate as usize * 2 * 5 / 1000);
            std::thread::spawn(move || {
                while !consumer.is_abandoned() {
                    if shared.flush.swap(false, Ordering::AcqRel) {
                        while consumer.pop().is_ok() {}
                    }
                    if !shared.paused.load(Ordering::Relaxed) {
                        let mut taken = 0;
                        while per_tick.is_none_or(|limit| taken < limit) && consumer.pop().is_ok() {
                            taken += 1;
                        }
                    }
                    std::thread::sleep(Duration::from_millis(if per_tick.is_some() { 5 } else { 1 }));
                }
            });
            return Ok(Self {
                sample_rate: wanted_rate,
                channels: 2,
                producer,
                capacity: wanted_rate as usize,
                _stream: None,
            });
        }

        let device = cpal::default_host().default_output_device().ok_or_else(|| "no audio output device".to_owned())?;
        let exact = device.supported_output_configs().ok().and_then(|mut configs| {
            configs.find(|config| {
                config.channels() == 2
                    && config.sample_format() == cpal::SampleFormat::F32
                    && (config.min_sample_rate().0..=config.max_sample_rate().0).contains(&wanted_rate)
            })
        });
        let config = match exact {
            Some(config) => config.with_sample_rate(cpal::SampleRate(wanted_rate)),
            // The playback thread resamples to whatever the device prefers.
            None => device.default_output_config().map_err(|error| error.to_string())?,
        };
        let sample_rate = config.sample_rate().0;
        let channels = usize::from(config.channels());
        // Half a second: enough to ride out a hiccup, short enough that pause and seek feel immediate.
        let capacity = sample_rate as usize * channels / 2;
        let (producer, mut consumer) = RingBuffer::new(capacity);

        let shared = Arc::clone(&controls);
        let on_error = |_error: cpal::StreamError| {};
        let stream = match config.sample_format() {
            cpal::SampleFormat::F32 => device.build_output_stream(
                &config.config(),
                move |data: &mut [f32], _: &_| fill(data, &mut consumer, &shared),
                on_error,
                None,
            ),
            cpal::SampleFormat::I16 => device.build_output_stream(
                &config.config(),
                move |data: &mut [i16], _: &_| fill(data, &mut consumer, &shared),
                on_error,
                None,
            ),
            cpal::SampleFormat::U16 => device.build_output_stream(
                &config.config(),
                move |data: &mut [u16], _: &_| fill(data, &mut consumer, &shared),
                on_error,
                None,
            ),
            other => return Err(format!("unsupported sample format {other:?}")),
        }
        .map_err(|error| error.to_string())?;
        stream.play().map_err(|error| error.to_string())?;
        Ok(Self { sample_rate, channels, producer, capacity, _stream: Some(stream) })
    }
}
