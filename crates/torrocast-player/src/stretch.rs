//! Tempo without the chipmunk: WSOLA, waveform-similarity overlap-add.
//!
//! Playing faster by dropping samples raises the pitch. WSOLA instead cuts the
//! signal into short overlapping grains and lays them down closer together (or
//! further apart). Each grain is taken from where the signal best continues
//! the previous one, so the waves line up and a voice keeps its pitch.

/// Grain length. Long enough to hold two periods of a low voice.
const GRAIN_SECONDS: f32 = 0.040;
/// How far a grain may move from its ideal place to line up with the last one.
const SEARCH_SECONDS: f32 = 0.012;
/// The similarity search looks at every n-th candidate and sample: a quarter
/// of the work, and no audible difference for speech.
const STRIDE: usize = 2;

pub const MIN_SPEED: f32 = 0.5;
pub const MAX_SPEED: f32 = 3.0;

pub struct Stretcher {
    channels: usize,
    speed: f32,
    grain: usize,
    hop: usize,
    search: usize,
    window: Vec<f32>,
    /// Interleaved input not yet fully used; frame 0 is absolute frame `base`.
    input: Vec<f32>,
    base: usize,
    /// Where the next grain would ideally start, in absolute input frames.
    ideal: f64,
    /// Where the previous grain would naturally continue.
    natural: Option<usize>,
    /// The faded-out second half of the previous grain, waiting for its partner.
    tail: Vec<f32>,
}

impl Stretcher {
    #[must_use]
    pub fn new(sample_rate: u32, channels: usize) -> Self {
        let hop = ((sample_rate as f32 * GRAIN_SECONDS) as usize / 2).max(8);
        let grain = hop * 2;
        let window = (0..grain).map(|n| 0.5 - 0.5 * (std::f32::consts::TAU * n as f32 / grain as f32).cos()).collect();
        Self {
            channels: channels.max(1),
            speed: 1.0,
            grain,
            hop,
            search: (sample_rate as f32 * SEARCH_SECONDS) as usize,
            window,
            input: Vec::new(),
            base: 0,
            ideal: 0.0,
            natural: None,
            tail: vec![0.0; hop * channels.max(1)],
        }
    }

    #[must_use]
    pub fn speed(&self) -> f32 {
        self.speed
    }

    pub fn set_speed(&mut self, speed: f32) {
        self.speed = speed.clamp(MIN_SPEED, MAX_SPEED);
    }

    /// Forgets everything buffered — after a seek the old audio must not bleed in.
    pub fn reset(&mut self) {
        self.input.clear();
        self.base = 0;
        self.ideal = 0.0;
        self.natural = None;
        self.tail.fill(0.0);
    }

    /// Takes interleaved samples, appends the stretched result to `output`.
    /// At normal speed the input passes through untouched.
    pub fn process(&mut self, input: &[f32], output: &mut Vec<f32>) {
        if (self.speed - 1.0).abs() < 0.005 && self.natural.is_none() {
            output.extend_from_slice(input);
            return;
        }
        self.input.extend_from_slice(input);
        loop {
            let ideal = self.ideal.round() as usize;
            let furthest = ideal.max(self.natural.unwrap_or(0)) + self.search + self.grain;
            if furthest > self.base + self.input.len() / self.channels {
                break;
            }
            let start = match self.natural {
                Some(natural) => self.best_start(ideal, natural),
                None => ideal,
            };
            self.lay_down(start, output);
            self.natural = Some(start + self.hop);
            self.ideal += f64::from(self.hop as f32 * self.speed);
            self.forget_used();
        }
        // Back at normal speed and drained: return to passing through.
        if (self.speed - 1.0).abs() < 0.005 && self.input.len() / self.channels < self.grain {
            let rest = std::mem::take(&mut self.input);
            output.extend_from_slice(&rest);
            self.reset();
        }
    }

    fn mono(&self, frame: usize) -> f32 {
        let at = (frame - self.base) * self.channels;
        self.input[at..at + self.channels].iter().sum()
    }

    /// The start near `ideal` whose first half looks most like the audio that
    /// would have followed the previous grain.
    fn best_start(&self, ideal: usize, natural: usize) -> usize {
        let low = ideal.saturating_sub(self.search).max(self.base);
        let high = ideal + self.search;
        let mut best = (f32::MIN, ideal.max(self.base));
        for candidate in (low..=high).step_by(STRIDE) {
            let (mut correlation, mut energy) = (0.0f32, 1e-9f32);
            for offset in (0..self.hop).step_by(STRIDE) {
                let sample = self.mono(candidate + offset);
                correlation += sample * self.mono(natural + offset);
                energy += sample * sample;
            }
            let similarity = correlation / energy.sqrt();
            if similarity > best.0 {
                best = (similarity, candidate);
            }
        }
        best.1
    }

    /// Fades the grain at `start` in over the waiting tail and keeps its second half as the new tail.
    fn lay_down(&mut self, start: usize, output: &mut Vec<f32>) {
        let from = (start - self.base) * self.channels;
        for frame in 0..self.hop {
            for channel in 0..self.channels {
                let at = frame * self.channels + channel;
                output.push(self.tail[at] + self.input[from + at] * self.window[frame]);
            }
        }
        for frame in 0..self.hop {
            for channel in 0..self.channels {
                let at = frame * self.channels + channel;
                self.tail[at] = self.input[from + self.hop * self.channels + at] * self.window[self.hop + frame];
            }
        }
    }

    fn forget_used(&mut self) {
        let needed = (self.ideal as usize).saturating_sub(self.search).min(self.natural.unwrap_or(usize::MAX));
        if needed > self.base {
            let frames = (needed - self.base).min(self.input.len() / self.channels);
            self.input.drain(..frames * self.channels);
            self.base += frames;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Stretcher;

    const RATE: u32 = 44_100;

    fn tone(frequency: f32, seconds: f32) -> Vec<f32> {
        (0..(RATE as f32 * seconds) as usize)
            .map(|n| (std::f32::consts::TAU * frequency * n as f32 / RATE as f32).sin())
            .collect()
    }

    fn stretched(input: &[f32], speed: f32) -> Vec<f32> {
        let mut stretcher = Stretcher::new(RATE, 1);
        stretcher.set_speed(speed);
        let mut output = Vec::new();
        // In uneven pieces, as a decoder delivers them.
        for piece in input.chunks(1153) {
            stretcher.process(piece, &mut output);
        }
        output
    }

    /// Cycles per second, from the sign changes in the steady middle of the signal.
    fn frequency(signal: &[f32]) -> f32 {
        let middle = &signal[signal.len() / 4..signal.len() * 3 / 4];
        let crossings = middle.windows(2).filter(|pair| pair[0] <= 0.0 && pair[1] > 0.0).count();
        crossings as f32 / (middle.len() as f32 / RATE as f32)
    }

    #[test]
    fn faster_is_shorter_at_the_same_pitch() {
        let input = tone(220.0, 3.0);
        for speed in [1.5f32, 2.0, 0.8] {
            let output = stretched(&input, speed);
            let expected = input.len() as f32 / speed;
            let length_error = (output.len() as f32 - expected).abs() / expected;
            assert!(length_error < 0.05, "speed {speed}: {} samples, expected about {expected}", output.len());
            let pitch = frequency(&output);
            assert!((pitch - 220.0).abs() < 4.0, "speed {speed}: pitch moved to {pitch} Hz");
        }
    }

    #[test]
    fn the_level_survives() {
        let output = stretched(&tone(220.0, 2.0), 1.7);
        let middle = &output[output.len() / 4..output.len() * 3 / 4];
        let peak = middle.iter().fold(0.0f32, |peak, sample| peak.max(sample.abs()));
        assert!((0.85..1.15).contains(&peak), "grains must add up to the original level, got {peak}");
    }

    #[test]
    fn normal_speed_is_a_passthrough() {
        let input = tone(220.0, 0.5);
        assert_eq!(stretched(&input, 1.0), input);
    }

    #[test]
    fn stereo_stays_in_step() {
        let mono = tone(330.0, 1.0);
        let stereo: Vec<f32> = mono.iter().flat_map(|sample| [*sample, -*sample]).collect();
        let mut stretcher = Stretcher::new(RATE, 2);
        stretcher.set_speed(1.5);
        let mut output = Vec::new();
        stretcher.process(&stereo, &mut output);
        assert_eq!(output.len() % 2, 0);
        assert!(output.chunks(2).all(|frame| (frame[0] + frame[1]).abs() < 1e-4), "both channels get the same cuts");
    }
}
