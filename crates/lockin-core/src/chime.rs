//! Chime synthesis.
//!
//! Each chime is a short additive render: a handful of partials, each with its
//! own decay, summed and normalised. Inharmonic ratios are what make a struck
//! metal object sound like one rather than like an organ.

use serde::{Deserialize, Serialize};

use crate::timer::Phase;

/// Rounded onset, long enough to remove the click of a hard start.
const ATTACK_MS: f32 = 9.0;
/// Guaranteed silence at the tail so the buffer never ends mid-cycle.
const RELEASE_MS: f32 = 60.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ChimeVoice {
    /// A soft struck bell. The default: clear, but with the edge taken off.
    Bell,
    /// A singing bowl. Slow, wide and very long, with a gentle beating.
    Bowl,
    /// A wooden block. Dry and brief, for anyone who finds tails distracting.
    Wood,
}

impl ChimeVoice {
    pub fn id(self) -> &'static str {
        match self {
            ChimeVoice::Bell => "bell",
            ChimeVoice::Bowl => "bowl",
            ChimeVoice::Wood => "wood",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            ChimeVoice::Bell => "Bell",
            ChimeVoice::Bowl => "Singing bowl",
            ChimeVoice::Wood => "Wood",
        }
    }

    pub fn all() -> [ChimeVoice; 3] {
        [ChimeVoice::Bell, ChimeVoice::Bowl, ChimeVoice::Wood]
    }

    /// `(frequency ratio, amplitude, decay seconds, detune in Hz)`.
    fn partials(self) -> &'static [(f32, f32, f32, f32)] {
        match self {
            ChimeVoice::Bell => &[
                (1.00, 1.00, 3.20, 0.0),
                (2.00, 0.50, 2.30, 0.0),
                (2.98, 0.32, 1.60, 0.0),
                (4.03, 0.18, 1.10, 0.0),
                (5.43, 0.09, 0.80, 0.0),
                (6.79, 0.05, 0.55, 0.0),
            ],
            ChimeVoice::Bowl => &[
                (1.00, 1.00, 6.00, 0.0),
                // The detuned twin beats slowly against its partner, which is
                // what gives a real bowl its breathing quality.
                (1.00, 0.85, 6.00, 0.7),
                (2.72, 0.42, 4.00, 0.0),
                (2.72, 0.30, 4.00, 1.1),
                (5.38, 0.14, 2.40, 0.0),
            ],
            ChimeVoice::Wood => &[
                (1.00, 1.00, 0.34, 0.0),
                (3.12, 0.44, 0.20, 0.0),
                (6.71, 0.16, 0.12, 0.0),
            ],
        }
    }

    fn longest_decay(self) -> f32 {
        self.partials()
            .iter()
            .map(|(_, _, decay, _)| *decay)
            .fold(0.0, f32::max)
    }
}

/// The pitch each boundary rings at. Ending focus resolves downward and ending
/// a break lifts, so the two are distinguishable without looking at the screen.
pub fn frequency_for(phase_ended: Phase) -> f32 {
    match phase_ended {
        Phase::Focus => 396.0,
        Phase::ShortBreak | Phase::LongBreak => 528.0,
    }
}

/// Renders one strike as an interleaved stereo buffer, peak-normalised to
/// `gain`. Returns an empty buffer when `gain` is zero.
pub fn render(voice: ChimeVoice, sample_rate: u32, frequency: f32, gain: f32) -> Vec<f32> {
    let gain = gain.clamp(0.0, 1.0);
    if gain == 0.0 {
        return Vec::new();
    }

    let sample_rate = sample_rate.max(8_000);
    let frequency = frequency.clamp(40.0, 4_000.0);
    let rate = sample_rate as f32;

    // Run each partial until it has decayed well below audibility.
    let duration_secs = voice.longest_decay() * 1.6 + RELEASE_MS / 1000.0;
    let frames = (duration_secs * rate) as usize;
    let attack_frames = (ATTACK_MS / 1000.0 * rate).max(1.0);
    let release_frames = (RELEASE_MS / 1000.0 * rate).max(1.0);
    let nyquist = rate / 2.0;

    let mut mono = vec![0.0f32; frames];
    for (ratio, amplitude, decay, detune) in voice.partials() {
        let partial_hz = frequency * ratio + detune;
        // Anything above Nyquist would alias back down as a metallic buzz.
        if partial_hz >= nyquist {
            continue;
        }
        let step = std::f32::consts::TAU * partial_hz / rate;
        for (index, slot) in mono.iter_mut().enumerate() {
            let t = index as f32 / rate;
            *slot += amplitude * (-t / decay).exp() * (step * index as f32).sin();
        }
    }

    let peak = mono.iter().fold(0.0f32, |acc, s| acc.max(s.abs()));
    let normalise = if peak > 0.0 { gain / peak } else { 0.0 };

    let mut out = Vec::with_capacity(frames * 2);
    for (index, sample) in mono.iter().enumerate() {
        let position = index as f32;
        let attack = (position / attack_frames).min(1.0);
        // Raised cosine rather than a straight line: no corner to hear.
        let attack = 0.5 - 0.5 * (std::f32::consts::PI * attack).cos();
        let remaining = (frames - 1 - index) as f32;
        let release = (remaining / release_frames).min(1.0);
        let value = sample * normalise * attack * release;
        let value = value.clamp(-1.0, 1.0);
        out.push(value);
        out.push(value);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const RATE: u32 = 48_000;

    fn rms(samples: &[f32]) -> f32 {
        if samples.is_empty() {
            return 0.0;
        }
        (samples.iter().map(|s| s * s).sum::<f32>() / samples.len() as f32).sqrt()
    }

    fn peak(samples: &[f32]) -> f32 {
        samples.iter().fold(0.0f32, |acc, s| acc.max(s.abs()))
    }

    #[test]
    fn every_voice_renders_audible_stereo_audio() {
        for voice in ChimeVoice::all() {
            let buf = render(voice, RATE, 440.0, 0.8);
            assert!(!buf.is_empty(), "{voice:?} rendered nothing");
            assert_eq!(buf.len() % 2, 0, "{voice:?} is not interleaved stereo");
            assert!(rms(&buf) > 0.005, "{voice:?} is inaudible");
        }
    }

    #[test]
    fn output_is_normalised_to_the_requested_gain() {
        for voice in ChimeVoice::all() {
            let buf = render(voice, RATE, 440.0, 0.5);
            let observed = peak(&buf);
            assert!(observed <= 0.5 + 1e-4, "{voice:?} peaked at {observed}");
            assert!(observed > 0.4, "{voice:?} only reached {observed}");
        }
    }

    #[test]
    fn silence_is_requested_by_zero_gain() {
        assert!(render(ChimeVoice::Bell, RATE, 440.0, 0.0).is_empty());
    }

    #[test]
    fn the_onset_is_ramped_not_clicked() {
        let buf = render(ChimeVoice::Bell, RATE, 440.0, 1.0);
        assert!(buf[0].abs() < 1e-6, "first sample is {}", buf[0]);
        // Within the first millisecond the envelope is still well below peak.
        let first_ms = &buf[..2 * RATE as usize / 1000];
        assert!(peak(first_ms) < 0.3);
    }

    #[test]
    fn the_tail_lands_on_exact_silence() {
        for voice in ChimeVoice::all() {
            let buf = render(voice, RATE, 440.0, 1.0);
            let last = buf.last().copied().unwrap();
            assert!(last.abs() < 1e-6, "{voice:?} ended at {last}");
        }
    }

    #[test]
    fn energy_decays_over_the_render() {
        for voice in ChimeVoice::all() {
            let buf = render(voice, RATE, 440.0, 1.0);
            let quarter = buf.len() / 4;
            let head = rms(&buf[quarter..quarter * 2]);
            let tail = rms(&buf[quarter * 3..]);
            assert!(tail < head, "{voice:?} did not decay: {head} then {tail}");
        }
    }

    #[test]
    fn wood_is_much_shorter_than_bowl() {
        let wood = render(ChimeVoice::Wood, RATE, 440.0, 1.0);
        let bowl = render(ChimeVoice::Bowl, RATE, 440.0, 1.0);
        assert!(wood.len() * 4 < bowl.len());
    }

    #[test]
    fn partials_above_nyquist_are_dropped_rather_than_aliased() {
        // At 8 kHz the upper bell partials sit above Nyquist.
        let buf = render(ChimeVoice::Bell, 8_000, 3_500.0, 1.0);
        assert!(!buf.is_empty());
        assert!(peak(&buf) <= 1.0);
    }

    #[test]
    fn the_two_boundaries_ring_at_different_pitches() {
        assert_ne!(
            frequency_for(Phase::Focus),
            frequency_for(Phase::ShortBreak)
        );
        assert_eq!(
            frequency_for(Phase::ShortBreak),
            frequency_for(Phase::LongBreak)
        );
    }

    #[test]
    fn extreme_frequencies_are_clamped_into_range() {
        assert!(!render(ChimeVoice::Bell, RATE, 0.0, 1.0).is_empty());
        assert!(!render(ChimeVoice::Bell, RATE, 100_000.0, 1.0).is_empty());
    }
}
