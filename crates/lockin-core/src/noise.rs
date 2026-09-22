//! Ambient sound.
//!
//! One sound: a deep, low rumble, generated sample by sample. There is no
//! audio file to ship, no licence to track and no loop point to hear. The
//! source runs forever and fades in and out under its own envelope so
//! starting and stopping never clicks.
//!
//! This used to offer a choice. White noise went first: flat power per hertz
//! puts most of its energy in the top two octaves, which is exactly the part
//! that hurts after twenty minutes. Pink went with it for being hissy, and
//! synthesised rain and a low drone were tried and judged worse than nothing.
//! What is left is the one people actually keep switched on.

pub const DEFAULT_FADE_MS: u32 = 450;

/// The rumble is gently rolled off up here. There is little energy above it
/// to begin with; this is the tuning the sound was settled on, kept fixed
/// rather than exposed as a control nobody could hear working.
const TOP_END_HZ: f32 = 3_000.0;

/// A small, fast, deterministic PRNG. Audio noise needs speed and a flat
/// distribution, not cryptographic strength.
#[derive(Debug, Clone)]
struct Xorshift64 {
    state: u64,
}

impl Xorshift64 {
    fn new(seed: u64) -> Self {
        Xorshift64 {
            // A zero state would lock the generator at zero forever.
            state: if seed == 0 {
                0x9E37_79B9_7F4A_7C15
            } else {
                seed
            },
        }
    }

    fn next_u64(&mut self) -> u64 {
        let mut x = self.state;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.state = x;
        x
    }

    /// Uniform in [-1.0, 1.0).
    fn next_bipolar(&mut self) -> f32 {
        ((self.next_u64() >> 40) as f32 / 16_777_216.0) * 2.0 - 1.0
    }
}

#[derive(Debug, Clone)]
struct ChannelState {
    rng: Xorshift64,
    brown: f32,
    lowpass: f32,
}

impl ChannelState {
    fn new(seed: u64) -> Self {
        ChannelState {
            rng: Xorshift64::new(seed),
            brown: 0.0,
            lowpass: 0.0,
        }
    }

    /// A leaky integrator of white noise, which gives the -6 dB per octave
    /// tilt that makes it a rumble rather than a hiss. The leak keeps it from
    /// wandering away from zero.
    fn next(&mut self, top_end: f32) -> f32 {
        let white = self.rng.next_bipolar();
        self.brown = (self.brown + 0.02 * white) / 1.02;
        let raw = (self.brown * 3.5).clamp(-1.0, 1.0);
        self.lowpass += top_end * (raw - self.lowpass);
        self.lowpass
    }
}

/// An endless stereo rumble with its own fade envelope.
#[derive(Debug, Clone)]
pub struct NoiseSource {
    sample_rate: u32,
    channels: [ChannelState; 2],
    gain: f32,
    target_gain: f32,
    fade_step: f32,
    top_end: f32,
}

impl NoiseSource {
    /// Starts silent. Call [`NoiseSource::fade_to`] to bring it up.
    pub fn new(sample_rate: u32, seed: u64) -> Self {
        let sample_rate = sample_rate.max(8_000);
        NoiseSource {
            sample_rate,
            // Independent states per channel give a wide, decorrelated field
            // rather than a single point of sound in the middle of the head.
            channels: [
                ChannelState::new(seed),
                ChannelState::new(seed ^ 0xA5A5_5A5A_1234_9876),
            ],
            gain: 0.0,
            target_gain: 0.0,
            fade_step: 0.0,
            top_end: one_pole(TOP_END_HZ, sample_rate as f32),
        }
    }

    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    /// Ramps the output level over `fade_ms`. A zero fade jumps immediately.
    pub fn fade_to(&mut self, target: f32, fade_ms: u32) {
        self.target_gain = target.clamp(0.0, 1.0);
        if fade_ms == 0 {
            self.gain = self.target_gain;
            self.fade_step = 0.0;
            return;
        }
        let samples = (u64::from(fade_ms) * u64::from(self.sample_rate) / 1000).max(1) as f32;
        self.fade_step = (self.target_gain - self.gain).abs() / samples;
    }

    pub fn gain(&self) -> f32 {
        self.gain
    }

    pub fn target_gain(&self) -> f32 {
        self.target_gain
    }

    /// True once a fade-out has fully landed, so the host can drop the stream.
    pub fn is_silent(&self) -> bool {
        self.gain <= f32::EPSILON && self.target_gain <= f32::EPSILON
    }

    /// Fills an interleaved stereo buffer. `out.len()` must be even.
    pub fn fill(&mut self, out: &mut [f32]) {
        // A fully faded-out source is the common idle case; skip the
        // generator entirely rather than shaping silence.
        if self.is_silent() {
            out.fill(0.0);
            return;
        }

        for frame in out.chunks_mut(2) {
            self.step_gain();
            for (channel, slot) in frame.iter_mut().enumerate() {
                let sample = self.channels[channel].next(self.top_end);
                *slot = (sample * self.gain).clamp(-1.0, 1.0);
            }
        }
    }

    fn step_gain(&mut self) {
        if self.fade_step == 0.0 {
            self.gain = self.target_gain;
            return;
        }
        if self.gain < self.target_gain {
            self.gain = (self.gain + self.fade_step).min(self.target_gain);
        } else if self.gain > self.target_gain {
            self.gain = (self.gain - self.fade_step).max(self.target_gain);
        }
    }
}

/// One-pole low-pass coefficient for a cutoff in hertz.
fn one_pole(cutoff: f32, sample_rate: f32) -> f32 {
    let nyquist = sample_rate / 2.0;
    let cutoff = cutoff.min(nyquist * 0.98);
    1.0 - (-std::f32::consts::TAU * cutoff / sample_rate).exp()
}

#[cfg(test)]
mod tests {
    use super::*;

    const RATE: u32 = 48_000;

    fn rendered(frames: usize) -> Vec<f32> {
        let mut source = NoiseSource::new(RATE, 42);
        source.fade_to(1.0, 0);
        let mut buf = vec![0.0; frames * 2];
        source.fill(&mut buf);
        buf
    }

    fn left(samples: &[f32]) -> Vec<f32> {
        samples.iter().step_by(2).copied().collect()
    }

    fn rms(samples: &[f32]) -> f32 {
        (samples.iter().map(|s| s * s).sum::<f32>() / samples.len() as f32).sqrt()
    }

    /// Mean absolute step between neighbouring samples, divided by level: a
    /// loudness-independent proxy for how much treble a sound carries. This is
    /// the number that corresponds to "it hurts my ears".
    fn brightness(samples: &[f32]) -> f32 {
        let steps: f32 = samples.windows(2).map(|w| (w[1] - w[0]).abs()).sum();
        (steps / (samples.len() - 1) as f32) / rms(samples)
    }

    #[test]
    fn stays_inside_the_sample_range() {
        for sample in rendered(80_000) {
            assert!((-1.0..=1.0).contains(&sample), "{sample} is out of range");
        }
    }

    #[test]
    fn actually_makes_sound() {
        assert!(rms(&rendered(80_000)) > 0.05);
    }

    #[test]
    fn leaves_headroom_below_the_clamp() {
        let observed = rendered(200_000)
            .iter()
            .fold(0.0f32, |acc, s| acc.max(s.abs()));
        assert!(observed < 0.95, "peaked at {observed}");
    }

    #[test]
    fn is_far_gentler_than_raw_white_noise() {
        // The reason this crate offers a rumble and not a hiss.
        let mut rng = Xorshift64::new(42);
        let white: Vec<f32> = (0..80_000).map(|_| rng.next_bipolar()).collect();
        let rumble = brightness(&left(&rendered(80_000)));
        let hiss = brightness(&white);
        assert!(
            rumble < hiss / 3.0,
            "rumble at {rumble} is not much gentler than white at {hiss}"
        );
    }

    #[test]
    fn channels_are_decorrelated() {
        let samples = rendered(20_000);
        let identical = samples
            .chunks(2)
            .filter(|frame| (frame[0] - frame[1]).abs() < f32::EPSILON)
            .count();
        assert!(
            identical < 100,
            "channels look duplicated: {identical} identical frames"
        );
    }

    #[test]
    fn the_same_seed_gives_the_same_sound() {
        assert_eq!(rendered(2_000), rendered(2_000));
    }

    #[test]
    fn a_fresh_source_is_silent_until_faded_up() {
        let mut source = NoiseSource::new(RATE, 7);
        assert!(source.is_silent());
        let mut buf = vec![0.0; 2_000];
        source.fill(&mut buf);
        assert!(buf.iter().all(|s| *s == 0.0));
    }

    #[test]
    fn fading_in_ramps_rather_than_jumping() {
        let mut source = NoiseSource::new(RATE, 7);
        source.fade_to(1.0, 100);
        let mut buf = vec![0.0; 2 * RATE as usize / 10];
        source.fill(&mut buf);

        let first = rms(&buf[..2_000]);
        let last = rms(&buf[buf.len() - 2_000..]);
        assert!(
            first < last * 0.25,
            "start ({first}) should be much quieter than end ({last})"
        );
        assert!((source.gain() - 1.0).abs() < 0.01);
    }

    #[test]
    fn fading_out_reaches_true_silence() {
        let mut source = NoiseSource::new(RATE, 7);
        source.fade_to(1.0, 0);
        source.fade_to(0.0, 50);
        let mut buf = vec![0.0; 2 * RATE as usize / 10];
        source.fill(&mut buf);

        assert!(source.is_silent());
        let tail = &buf[buf.len() - 1_000..];
        assert!(
            tail.iter().all(|s| *s == 0.0),
            "tail should be digital silence"
        );
    }

    #[test]
    fn does_not_drift_away_from_zero() {
        let samples = rendered(400_000);
        let mean = samples.iter().sum::<f32>() / samples.len() as f32;
        assert!(mean.abs() < 0.05, "DC offset drifted to {mean}");
    }
}
