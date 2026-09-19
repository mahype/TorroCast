//! From bytes to samples: symphonia behind the three things a player needs —
//! the next piece of audio, a seek, and the length.

use std::io::ErrorKind;

use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::{CODEC_TYPE_NULL, Decoder as Codec, DecoderOptions};
use symphonia::core::errors::Error;
use symphonia::core::formats::{FormatOptions, FormatReader, SeekMode, SeekTo};
use symphonia::core::io::{MediaSource, MediaSourceStream};
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;
use symphonia::core::units::{Time, TimeBase};

pub struct Decoder {
    format: Box<dyn FormatReader>,
    codec: Box<dyn Codec>,
    track: u32,
    time_base: Option<TimeBase>,
    pub sample_rate: u32,
    pub channels: usize,
    pub duration_ms: Option<u64>,
    samples: Option<SampleBuffer<f32>>,
}

/// One decoded packet: interleaved samples and where in the episode they end.
pub struct Piece<'a> {
    pub samples: &'a [f32],
    pub channels: usize,
    pub ends_at_ms: Option<u64>,
}

impl Decoder {
    /// `extension` helps telling the format apart; the content decides.
    pub fn open(source: Box<dyn MediaSource>, extension: Option<&str>) -> Result<Self, String> {
        let mut hint = Hint::new();
        if let Some(extension) = extension {
            hint.with_extension(extension);
        }
        let stream = MediaSourceStream::new(source, Default::default());
        let options = FormatOptions { enable_gapless: true, ..FormatOptions::default() };
        let probed = symphonia::default::get_probe()
            .format(&hint, stream, &options, &MetadataOptions::default())
            .map_err(|error| error.to_string())?;
        let format = probed.format;
        let track = format
            .tracks()
            .iter()
            .find(|track| track.codec_params.codec != CODEC_TYPE_NULL)
            .ok_or_else(|| "no audio in this file".to_owned())?;
        let parameters = track.codec_params.clone();
        let codec = symphonia::default::get_codecs()
            .make(&parameters, &DecoderOptions::default())
            .map_err(|error| error.to_string())?;
        let sample_rate = parameters.sample_rate.ok_or_else(|| "unknown sample rate".to_owned())?;
        let duration_ms = parameters.n_frames.map(|frames| frames * 1000 / u64::from(sample_rate));
        Ok(Self {
            track: track.id,
            time_base: parameters.time_base,
            format,
            codec,
            sample_rate,
            // Known for certain only after the first packet; stereo is the safe guess.
            channels: parameters.channels.map_or(2, |channels| channels.count()),
            duration_ms,
            samples: None,
        })
    }

    fn milliseconds(&self, timestamp: u64) -> Option<u64> {
        let time = self.time_base?.calc_time(timestamp);
        Some(time.seconds * 1000 + (time.frac * 1000.0) as u64)
    }

    /// The next audio, or `None` at the end. Damaged packets are skipped: a
    /// crackle is better than a stop.
    pub fn next(&mut self) -> Result<Option<Piece<'_>>, String> {
        let ends_at_ms = loop {
            let packet = match self.format.next_packet() {
                Ok(packet) => packet,
                Err(Error::IoError(error)) if error.kind() == ErrorKind::UnexpectedEof => return Ok(None),
                Err(Error::ResetRequired) => return Ok(None),
                Err(error) => return Err(error.to_string()),
            };
            if packet.track_id() != self.track {
                continue;
            }
            let ends_at_ms = self.milliseconds(packet.ts() + packet.dur());
            let decoded = match self.codec.decode(&packet) {
                Ok(decoded) => decoded,
                Err(Error::DecodeError(_)) => continue,
                Err(Error::IoError(error)) if error.kind() == ErrorKind::UnexpectedEof => return Ok(None),
                Err(error) => return Err(error.to_string()),
            };
            let spec = *decoded.spec();
            self.channels = spec.channels.count();
            let needed = decoded.capacity() * self.channels;
            if self.samples.as_ref().is_none_or(|buffer| buffer.capacity() < needed) {
                self.samples = Some(SampleBuffer::new(decoded.capacity() as u64, spec));
            }
            if let Some(buffer) = self.samples.as_mut() {
                buffer.copy_interleaved_ref(decoded);
            }
            break ends_at_ms;
        };
        let samples = self.samples.as_ref().map_or(&[][..], |buffer| buffer.samples());
        Ok(Some(Piece { samples, channels: self.channels, ends_at_ms }))
    }

    /// Jumps near `position_ms` and says where it landed.
    pub fn seek(&mut self, position_ms: u64) -> Result<u64, String> {
        let time = Time::new(position_ms / 1000, (position_ms % 1000) as f64 / 1000.0);
        let landed = self
            .format
            .seek(SeekMode::Coarse, SeekTo::Time { time, track_id: Some(self.track) })
            .map_err(|error| error.to_string())?;
        self.codec.reset();
        Ok(self.milliseconds(landed.actual_ts).unwrap_or(position_ms))
    }
}
