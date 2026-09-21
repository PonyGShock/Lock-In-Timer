//! Noise synthesis.
//!
//! Everything is generated sample by sample, so there is no audio file to ship,
//! no licence to track and no loop point to hear. The source runs forever and
//! fades in and out under its own envelope so starting and stopping never
//! clicks.

use serde::{Deserialize, Serialize};

pub const DEFAULT_FADE_MS: u32 = 450;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum NoiseKind {
    /// Flat spectrum. Bright and hissy, the classic masking noise.
    White,
    /// Equal energy per octave. Softer, close to rain on a window.
    Pink,
    /// Steep low-frequency tilt. Deep and rumbling, like distant surf.
    Brown,
}

impl NoiseKind {
    pub fn id(self) -> &'static str {
        match self {
            NoiseKind::White => "white",
            NoiseKind::Pink => "pink",
            NoiseKind::Brown => "brown",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            NoiseKind::White => "White",
            NoiseKind::Pink => "Pink",
            NoiseKind::Brown => "Brown",
        }
    }

    pub fn all() -> [NoiseKind; 3] {
        [NoiseKind::White, NoiseKind::Pink, NoiseKind::Brown]
    }
}

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
        let bits = (self.next_u64() >> 40) as u32; // 24 bits of mantissa
        (bits as f32 / 8_388_608.0) - 1.0
    }
}

#[derive(Debug, Clone)]
struct ChannelState {
    rng: Xorshift64,
    pink: [f32; 7],
    brown: f32,
    lowpass: f32,
}

impl ChannelState {
    fn new(seed: u64) -> Self {
        ChannelState {
            rng: Xorshift64::new(seed),
            pink: [0.0; 7],
            brown: 0.0,
            lowpass: 0.0,
        }
    }

    fn white(&mut self) -> f32 {
        self.rng.next_bipolar()
    }

    /// Paul Kellett's refined pink-noise filter: a bank of one-pole filters
    /// whose sum approximates a -3 dB per octave tilt.
    fn pink(&mut self) -> f32 {
        let white = self.white();
        let b = &mut self.pink;
        b[0] = 0.99886 * b[0] + white * 0.0555179;
        b[1] = 0.99332 * b[1] + white * 0.0750759;
        b[2] = 0.96900 * b[2] + white * 0.153852;
        b[3] = 0.86650 * b[3] + white * 0.3104856;
        b[4] = 0.55000 * b[4] + white * 0.5329522;
        b[5] = -0.7616 * b[5] - white * 0.0168980;
        let out = b[0] + b[1] + b[2] + b[3] + b[4] + b[5] + b[6] + white * 0.5362;
        b[6] = white * 0.115926;
        out * 0.11
    }

    /// A leaky integrator of white noise, which gives the -6 dB per octave
    /// tilt. The leak keeps it from wandering away from zero.
    fn brown(&mut self) -> f32 {
        let white = self.white();
        self.brown = (self.brown + 0.02 * white) / 1.02;
        (self.brown * 3.5).clamp(-1.0, 1.0)
    }
}

/// An endless stereo noise generator with its own fade envelope.
#[derive(Debug, Clone)]
pub struct NoiseSource {
    kind: NoiseKind,
    sample_rate: u32,
    channels: [ChannelState; 2],
    gain: f32,
    target_gain: f32,
    fade_step: f32,
    lowpass_coeff: f32,
}

impl NoiseSource {
    /// Starts silent. Call [`NoiseSource::fade_to`] to bring it up.
    pub fn new(kind: NoiseKind, sample_rate: u32, seed: u64) -> Self {
        let mut source = NoiseSource {
            kind,
            sample_rate: sample_rate.max(8_000),
            // Independent states per channel give a wide, decorrelated field
            // rather than a single point of noise in the middle of the head.
            channels: [
                ChannelState::new(seed),
                ChannelState::new(seed ^ 0xA5A5_5A5A_1234_9876),
            ],
            gain: 0.0,
            target_gain: 0.0,
            fade_step: 0.0,
            lowpass_coeff: 1.0,
        };
        source.set_tone(0.6);
        source
    }

    pub fn kind(&self) -> NoiseKind {
        self.kind
    }

    pub fn set_kind(&mut self, kind: NoiseKind) {
        self.kind = kind;
    }

    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    /// Softens the top end. 0.0 is muffled and distant, 1.0 is fully open.
    pub fn set_tone(&mut self, tone: f32) {
        let tone = tone.clamp(0.0, 1.0);
        // Map to cutoff logarithmically so the control feels even to the ear.
        let cutoff = 350.0 * (18_000.0f32 / 350.0).powf(tone);
        let nyquist = self.sample_rate as f32 / 2.0;
        let cutoff = cutoff.min(nyquist * 0.98);
        let x = -std::f32::consts::TAU * cutoff / self.sample_rate as f32;
        self.lowpass_coeff = 1.0 - x.exp();
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
        // generators entirely rather than shaping noise down to zero.
        if self.is_silent() {
            out.fill(0.0);
            return;
        }
        for frame in out.chunks_mut(2) {
            self.step_gain();
            for (channel, slot) in frame.iter_mut().enumerate() {
                let raw = match self.kind {
                    NoiseKind::White => self.channels[channel].white(),
                    NoiseKind::Pink => self.channels[channel].pink(),
                    NoiseKind::Brown => self.channels[channel].brown(),
                };
                let state = &mut self.channels[channel];
                state.lowpass += self.lowpass_coeff * (raw - state.lowpass);
                *slot = (state.lowpass * self.gain).clamp(-1.0, 1.0);
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

#[cfg(test)]
mod tests {
    use super::*;

    const RATE: u32 = 48_000;

    fn rendered(kind: NoiseKind, frames: usize) -> Vec<f32> {
        let mut source = NoiseSource::new(kind, RATE, 42);
        source.set_tone(1.0); // bypass the tone shaping for spectral comparisons
        source.fade_to(1.0, 0);
        let mut buf = vec![0.0; frames * 2];
        source.fill(&mut buf);
        buf
    }

    /// Mean absolute difference between neighbouring samples: a cheap proxy
    /// for how much high-frequency energy a signal carries.
    fn roughness(samples: &[f32]) -> f32 {
        let left: Vec<f32> = samples.iter().step_by(2).copied().collect();
        let sum: f32 = left.windows(2).map(|w| (w[1] - w[0]).abs()).sum();
        sum / (left.len() - 1) as f32
    }

    fn rms(samples: &[f32]) -> f32 {
        (samples.iter().map(|s| s * s).sum::<f32>() / samples.len() as f32).sqrt()
    }

    #[test]
    fn every_kind_stays_inside_the_sample_range() {
        for kind in NoiseKind::all() {
            for sample in rendered(kind, 20_000) {
                assert!(
                    (-1.0..=1.0).contains(&sample),
                    "{kind:?} produced {sample} outside [-1, 1]"
                );
            }
        }
    }

    #[test]
    fn every_kind_actually_makes_sound() {
        for kind in NoiseKind::all() {
            assert!(rms(&rendered(kind, 20_000)) > 0.01, "{kind:?} was silent");
        }
    }

    #[test]
    fn spectral_tilt_goes_white_then_pink_then_brown() {
        let white = roughness(&rendered(NoiseKind::White, 40_000));
        let pink = roughness(&rendered(NoiseKind::Pink, 40_000));
        let brown = roughness(&rendered(NoiseKind::Brown, 40_000));
        assert!(
            pink < white,
            "pink ({pink}) should be softer than white ({white})"
        );
        assert!(
            brown < pink,
            "brown ({brown}) should be softer than pink ({pink})"
        );
    }

    #[test]
    fn channels_are_decorrelated() {
        let samples = rendered(NoiseKind::White, 20_000);
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
    fn the_same_seed_gives_the_same_noise() {
        assert_eq!(
            rendered(NoiseKind::Pink, 500),
            rendered(NoiseKind::Pink, 500)
        );
    }

    #[test]
    fn a_fresh_source_is_silent_until_faded_up() {
        let mut source = NoiseSource::new(NoiseKind::Brown, RATE, 7);
        assert!(source.is_silent());
        let mut buf = vec![0.0; 2_000];
        source.fill(&mut buf);
        assert!(buf.iter().all(|s| *s == 0.0));
    }

    #[test]
    fn fading_in_ramps_rather_than_jumping() {
        let mut source = NoiseSource::new(NoiseKind::White, RATE, 7);
        source.fade_to(1.0, 100);
        let mut buf = vec![0.0; 2 * RATE as usize / 10]; // exactly the fade length
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
        let mut source = NoiseSource::new(NoiseKind::White, RATE, 7);
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
    fn tone_control_removes_high_frequencies() {
        let mut dark = NoiseSource::new(NoiseKind::White, RATE, 11);
        dark.set_tone(0.0);
        dark.fade_to(1.0, 0);
        let mut dark_buf = vec![0.0; 40_000];
        dark.fill(&mut dark_buf);

        let mut open = NoiseSource::new(NoiseKind::White, RATE, 11);
        open.set_tone(1.0);
        open.fade_to(1.0, 0);
        let mut open_buf = vec![0.0; 40_000];
        open.fill(&mut open_buf);

        assert!(roughness(&dark_buf) < roughness(&open_buf));
    }

    #[test]
    fn brown_noise_does_not_drift_away_from_zero() {
        let samples = rendered(NoiseKind::Brown, 400_000);
        let mean = samples.iter().sum::<f32>() / samples.len() as f32;
        assert!(mean.abs() < 0.05, "DC offset drifted to {mean}");
    }
}
