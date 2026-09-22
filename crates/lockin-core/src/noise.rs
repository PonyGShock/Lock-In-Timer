//! Ambient sound synthesis.
//!
//! Everything is generated sample by sample, so there is no audio file to ship,
//! no licence to track and no loop point to hear. The source runs forever and
//! fades in and out under its own envelope so starting and stopping never
//! clicks.
//!
//! White and pink noise were tried and dropped. Flat power per hertz puts most
//! of a white signal's energy in the top two octaves, which is precisely the
//! range that grates after twenty minutes; pink is gentler but still hissy.
//! What survives here is the quiet end: a deep rumble, rain, and a low drone.

use serde::{Deserialize, Serialize};

pub const DEFAULT_FADE_MS: u32 = 450;

/// Droplet voices per channel. Enough overlap to sound like rainfall rather
/// than a dripping tap, without turning into a wash of its own.
const DROPLETS: usize = 8;
/// Droplet onsets per second, per channel.
const DROPLET_RATE: f32 = 45.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum NoiseKind {
    /// Brown noise: a steep low-frequency tilt, deep and rumbling.
    // The aliases keep settings files written before the sound set changed
    // loading instead of being thrown away for one unknown word.
    #[serde(alias = "brown", alias = "white", alias = "pink")]
    Deep,
    /// Steady rainfall: a filtered wash under scattered droplets.
    Rain,
    /// A low drone with a slow binaural pulse.
    Hum,
}

impl NoiseKind {
    pub fn id(self) -> &'static str {
        match self {
            NoiseKind::Deep => "deep",
            NoiseKind::Rain => "rain",
            NoiseKind::Hum => "hum",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            NoiseKind::Deep => "Deep",
            NoiseKind::Rain => "Rain",
            NoiseKind::Hum => "Hum",
        }
    }

    pub fn all() -> [NoiseKind; 3] {
        [NoiseKind::Deep, NoiseKind::Rain, NoiseKind::Hum]
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

    /// Uniform in [0.0, 1.0).
    fn next_unit(&mut self) -> f32 {
        (self.next_u64() >> 40) as f32 / 16_777_216.0
    }

    /// Uniform in [-1.0, 1.0).
    fn next_bipolar(&mut self) -> f32 {
        self.next_unit() * 2.0 - 1.0
    }

    fn range(&mut self, low: f32, high: f32) -> f32 {
        low + self.next_unit() * (high - low)
    }
}

/// One decaying "plink". Several run at once to build up rainfall.
#[derive(Debug, Clone, Default)]
struct Droplet {
    envelope: f32,
    decay: f32,
    phase: f32,
    step: f32,
    amplitude: f32,
}

impl Droplet {
    fn active(&self) -> bool {
        // Roughly -48 dB. Anything quieter is inaudible under the wash, and
        // holding the voice open past that point only starves the pool.
        self.envelope > 0.004
    }

    fn strike(&mut self, rng: &mut Xorshift64, sample_rate: f32) {
        let frequency = rng.range(900.0, 3600.0);
        let tau = rng.range(0.008, 0.026);
        self.envelope = 1.0;
        self.decay = (-1.0 / (tau * sample_rate)).exp();
        self.phase = 0.0;
        self.step = std::f32::consts::TAU * frequency / sample_rate;
        self.amplitude = rng.range(0.05, 0.20);
    }

    fn next(&mut self) -> f32 {
        if !self.active() {
            self.envelope = 0.0;
            return 0.0;
        }
        let value = self.amplitude * self.envelope * self.phase.sin();
        self.phase += self.step;
        self.envelope *= self.decay;
        value
    }
}

#[derive(Debug, Clone)]
struct ChannelState {
    rng: Xorshift64,
    pink: [f32; 7],
    brown: f32,
    tone_lp: f32,
    rain_lp1: f32,
    rain_lp2: f32,
    rain_hp: f32,
    droplets: [Droplet; DROPLETS],
    drone_phase: f32,
    harmonic_phase: f32,
    binaural_phase: f32,
    breath_phase: f32,
}

impl ChannelState {
    fn new(seed: u64) -> Self {
        ChannelState {
            rng: Xorshift64::new(seed),
            pink: [0.0; 7],
            brown: 0.0,
            tone_lp: 0.0,
            rain_lp1: 0.0,
            rain_lp2: 0.0,
            rain_hp: 0.0,
            droplets: Default::default(),
            drone_phase: 0.0,
            harmonic_phase: 0.0,
            binaural_phase: 0.0,
            breath_phase: 0.0,
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
    fn deep(&mut self) -> f32 {
        let white = self.white();
        self.brown = (self.brown + 0.02 * white) / 1.02;
        (self.brown * 3.5).clamp(-1.0, 1.0)
    }

    /// A filtered wash for the body of the rain, plus scattered droplets for
    /// the texture. Two cascaded poles roll the wash off at -12 dB per octave,
    /// which is what keeps it sounding like water and not like hiss.
    fn rain(&mut self, coeffs: RainCoeffs, sample_rate: f32) -> f32 {
        let wash = self.pink();
        self.rain_lp1 += coeffs.wash * (wash - self.rain_lp1);
        self.rain_lp2 += coeffs.wash * (self.rain_lp1 - self.rain_lp2);
        // Rain on a window carries almost no bass. Subtracting a low-passed
        // copy high-passes the wash, which is the difference between rainfall
        // and a rumble with ticks on top.
        self.rain_hp += coeffs.rumble * (self.rain_lp2 - self.rain_hp);
        let mut out = (self.rain_lp2 - self.rain_hp) * 1.8;

        if self.rng.next_unit() < coeffs.droplet {
            if let Some(voice) = self.droplets.iter_mut().find(|d| !d.active()) {
                voice.strike(&mut self.rng, sample_rate);
            }
        }
        for voice in &mut self.droplets {
            out += voice.next();
        }
        out.clamp(-1.0, 1.0)
    }

    /// A low drone with a quiet, slightly detuned partner an octave and a half
    /// up. The detuning is what produces the slow pulse; it only reads as one
    /// when each ear gets its own channel, so headphones matter here.
    fn hum(&mut self, steps: &HumSteps) -> f32 {
        self.drone_phase += steps.fundamental;
        self.harmonic_phase += steps.harmonic;
        self.binaural_phase += steps.binaural;
        self.breath_phase += steps.breath;

        let breath = 0.85 + 0.15 * self.breath_phase.sin();
        let body = self.drone_phase.sin() * 0.26
            + self.harmonic_phase.sin() * 0.10
            + self.binaural_phase.sin() * 0.16;
        // A whisper of noise underneath; a bare sine sounds like equipment.
        let bed = self.deep() * 0.05;
        ((body * breath) + bed).clamp(-1.0, 1.0)
    }
}

/// Filter coefficients for the rain generator, precomputed once.
#[derive(Debug, Clone, Copy)]
struct RainCoeffs {
    wash: f32,
    rumble: f32,
    droplet: f32,
}

/// Per-sample phase increments for the drone, precomputed per channel.
#[derive(Debug, Clone, Copy, Default)]
struct HumSteps {
    fundamental: f32,
    harmonic: f32,
    binaural: f32,
    breath: f32,
}

/// An endless stereo generator with its own fade envelope.
#[derive(Debug, Clone)]
pub struct NoiseSource {
    kind: NoiseKind,
    sample_rate: u32,
    channels: [ChannelState; 2],
    gain: f32,
    target_gain: f32,
    fade_step: f32,
    tone_coeff: f32,
    rain: RainCoeffs,
    hum: [HumSteps; 2],
}

impl NoiseSource {
    /// Starts silent. Call [`NoiseSource::fade_to`] to bring it up.
    pub fn new(kind: NoiseKind, sample_rate: u32, seed: u64) -> Self {
        let sample_rate = sample_rate.max(8_000);
        let rate = sample_rate as f32;

        let mut source = NoiseSource {
            kind,
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
            tone_coeff: 1.0,
            rain: RainCoeffs {
                wash: one_pole(5_000.0, rate),
                rumble: one_pole(180.0, rate),
                droplet: DROPLET_RATE / rate,
            },
            hum: hum_steps(rate),
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
        self.tone_coeff = one_pole(cutoff, self.sample_rate as f32);
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
        // generators entirely rather than shaping silence.
        if self.is_silent() {
            out.fill(0.0);
            return;
        }

        let rate = self.sample_rate as f32;
        for frame in out.chunks_mut(2) {
            self.step_gain();
            for (channel, slot) in frame.iter_mut().enumerate() {
                let raw = match self.kind {
                    NoiseKind::Deep => self.channels[channel].deep(),
                    NoiseKind::Rain => self.channels[channel].rain(self.rain, rate),
                    NoiseKind::Hum => self.channels[channel].hum(&self.hum[channel]),
                };
                let state = &mut self.channels[channel];
                state.tone_lp += self.tone_coeff * (raw - state.tone_lp);
                *slot = (state.tone_lp * self.gain).clamp(-1.0, 1.0);
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

/// The drone's per-channel phase increments. The 7 Hz offset between the two
/// binaural carriers is the pulse rate; everything else is shared.
fn hum_steps(sample_rate: f32) -> [HumSteps; 2] {
    let step = |hz: f32| std::f32::consts::TAU * hz / sample_rate;
    let carrier = 220.0;
    let beat = 7.0;
    [
        HumSteps {
            fundamental: step(68.0),
            harmonic: step(136.0),
            binaural: step(carrier),
            breath: step(0.06),
        },
        HumSteps {
            fundamental: step(68.0),
            harmonic: step(136.0),
            binaural: step(carrier + beat),
            breath: step(0.06),
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    const RATE: u32 = 48_000;

    fn rendered(kind: NoiseKind, frames: usize) -> Vec<f32> {
        let mut source = NoiseSource::new(kind, RATE, 42);
        source.set_tone(1.0); // bypass tone shaping for spectral comparisons
        source.fade_to(1.0, 0);
        let mut buf = vec![0.0; frames * 2];
        source.fill(&mut buf);
        buf
    }

    fn left(samples: &[f32]) -> Vec<f32> {
        samples.iter().step_by(2).copied().collect()
    }

    fn right(samples: &[f32]) -> Vec<f32> {
        samples.iter().skip(1).step_by(2).copied().collect()
    }

    /// What a listener hears when the two channels reach the same ear, which
    /// is what happens on speakers.
    fn summed(samples: &[f32]) -> Vec<f32> {
        samples.chunks(2).map(|f| (f[0] + f[1]) * 0.5).collect()
    }

    fn rms(samples: &[f32]) -> f32 {
        (samples.iter().map(|s| s * s).sum::<f32>() / samples.len() as f32).sqrt()
    }

    fn peak(samples: &[f32]) -> f32 {
        samples.iter().fold(0.0f32, |acc, s| acc.max(s.abs()))
    }

    /// Mean absolute step between neighbouring samples, divided by level: a
    /// cheap, loudness-independent proxy for how much treble a sound carries.
    /// This is the number that corresponds to "it hurts my ears".
    fn brightness(samples: &[f32]) -> f32 {
        let steps: f32 = samples.windows(2).map(|w| (w[1] - w[0]).abs()).sum();
        (steps / (samples.len() - 1) as f32) / rms(samples)
    }

    #[test]
    fn every_kind_stays_inside_the_sample_range() {
        for kind in NoiseKind::all() {
            for sample in rendered(kind, 40_000) {
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
            assert!(
                rms(&rendered(kind, 40_000)) > 0.05,
                "{kind:?} was too quiet"
            );
        }
    }

    #[test]
    fn nothing_reaches_the_clamp() {
        // Hitting +/-1.0 means the sum is being squashed, which is audible as
        // grit. Every kind should leave headroom at full gain.
        for kind in NoiseKind::all() {
            let observed = peak(&rendered(kind, 80_000));
            assert!(observed < 0.95, "{kind:?} peaked at {observed}");
        }
    }

    #[test]
    fn the_three_sounds_sit_at_the_same_level() {
        // Switching kind should change the character, not the volume.
        let levels: Vec<f32> = NoiseKind::all()
            .iter()
            .map(|k| rms(&rendered(*k, 80_000)))
            .collect();
        let loudest = levels.iter().cloned().fold(0.0f32, f32::max);
        let quietest = levels.iter().cloned().fold(f32::MAX, f32::min);
        assert!(loudest / quietest < 1.35, "levels are uneven: {levels:?}");
    }

    #[test]
    fn every_sound_is_far_gentler_than_the_white_noise_that_was_dropped() {
        // The reason this crate no longer offers white noise: measured against
        // it, everything here is several times less trebly.
        let mut reference = ChannelState::new(42);
        let white: Vec<f32> = (0..80_000).map(|_| reference.white()).collect();
        let harshest = brightness(&white);

        for kind in NoiseKind::all() {
            let measured = brightness(&left(&rendered(kind, 80_000)));
            assert!(
                measured < harshest / 3.0,
                "{kind:?} at {measured} is not much gentler than white at {harshest}"
            );
        }
    }

    #[test]
    fn rain_is_brighter_than_deep_and_hum_is_darkest() {
        let deep = brightness(&left(&rendered(NoiseKind::Deep, 80_000)));
        let rain = brightness(&left(&rendered(NoiseKind::Rain, 80_000)));
        let hum = brightness(&left(&rendered(NoiseKind::Hum, 80_000)));
        assert!(
            rain > deep,
            "rain ({rain}) should be brighter than deep ({deep})"
        );
        assert!(
            hum < deep,
            "hum ({hum}) should be darker than deep ({deep})"
        );
    }

    #[test]
    fn a_droplet_strikes_then_decays_to_nothing() {
        let mut rng = Xorshift64::new(9);
        let mut droplet = Droplet::default();
        assert!(!droplet.active(), "a fresh droplet is silent");

        droplet.strike(&mut rng, RATE as f32);
        assert!(droplet.active());

        let opening: f32 = (0..64).map(|_| droplet.next().abs()).sum();
        assert!(opening > 0.0, "a struck droplet should make sound");

        // Longest tau is 26 ms and the gate is about six time constants down,
        // so a third of a second is comfortably past the end.
        for _ in 0..RATE / 3 {
            droplet.next();
        }
        assert!(!droplet.active(), "a droplet should not ring forever");
        assert_eq!(droplet.next(), 0.0);
    }

    #[test]
    fn hum_pulses_when_the_two_channels_meet() {
        // The carriers are 7 Hz apart. Neither channel wavers on its own —
        // a binaural beat is something the listener's head does — but the two
        // together swell and fade, which is what speakers reproduce.
        let samples = rendered(NoiseKind::Hum, 96_000);
        assert_ne!(left(&samples), right(&samples), "channels should differ");

        let mixed = summed(&samples);
        let levels: Vec<f32> = mixed.chunks(800).map(rms).collect();
        let loudest = levels.iter().cloned().fold(0.0f32, f32::max);
        let quietest = levels.iter().cloned().fold(f32::MAX, f32::min);
        assert!(
            loudest > quietest * 1.15,
            "hum should pulse, measured {quietest} to {loudest}"
        );
    }

    #[test]
    fn channels_are_decorrelated() {
        let samples = rendered(NoiseKind::Rain, 20_000);
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
        for kind in NoiseKind::all() {
            assert_eq!(rendered(kind, 2_000), rendered(kind, 2_000), "{kind:?}");
        }
    }

    #[test]
    fn a_fresh_source_is_silent_until_faded_up() {
        let mut source = NoiseSource::new(NoiseKind::Deep, RATE, 7);
        assert!(source.is_silent());
        let mut buf = vec![0.0; 2_000];
        source.fill(&mut buf);
        assert!(buf.iter().all(|s| *s == 0.0));
    }

    #[test]
    fn fading_in_ramps_rather_than_jumping() {
        let mut source = NoiseSource::new(NoiseKind::Deep, RATE, 7);
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
        let mut source = NoiseSource::new(NoiseKind::Deep, RATE, 7);
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
        let mut dark = NoiseSource::new(NoiseKind::Rain, RATE, 11);
        dark.set_tone(0.0);
        dark.fade_to(1.0, 0);
        let mut dark_buf = vec![0.0; 80_000];
        dark.fill(&mut dark_buf);

        let mut open = NoiseSource::new(NoiseKind::Rain, RATE, 11);
        open.set_tone(1.0);
        open.fade_to(1.0, 0);
        let mut open_buf = vec![0.0; 80_000];
        open.fill(&mut open_buf);

        assert!(brightness(&left(&dark_buf)) < brightness(&left(&open_buf)));
    }

    #[test]
    fn deep_does_not_drift_away_from_zero() {
        let samples = rendered(NoiseKind::Deep, 400_000);
        let mean = samples.iter().sum::<f32>() / samples.len() as f32;
        assert!(mean.abs() < 0.05, "DC offset drifted to {mean}");
    }

    #[test]
    fn settings_written_before_the_sound_set_changed_still_load() {
        for old in ["\"brown\"", "\"white\"", "\"pink\""] {
            let kind: NoiseKind = serde_json::from_str(old).expect("should migrate");
            assert_eq!(kind, NoiseKind::Deep, "{old} should fall back to Deep");
        }
        for (text, expected) in [
            ("\"deep\"", NoiseKind::Deep),
            ("\"rain\"", NoiseKind::Rain),
            ("\"hum\"", NoiseKind::Hum),
        ] {
            assert_eq!(serde_json::from_str::<NoiseKind>(text).unwrap(), expected);
        }
    }
}
