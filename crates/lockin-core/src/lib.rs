//! Core logic for Lock In: a calm pomodoro timer.
//!
//! This crate holds everything worth testing on its own — the phase state
//! machine, the rules tying it to the settings, the settings file and the one
//! thread that writes it, and the audio synthesis — and depends on no system
//! libraries, so it builds and tests anywhere without a windowing or audio
//! toolkit present. The desktop app is a thin shell around it.

pub mod chime;
pub mod noise;
pub mod persist;
pub mod session;
pub mod settings;
pub mod timer;

pub use chime::{frequency_for, ChimeVoice};
pub use noise::{NoiseSource, DEFAULT_FADE_MS};
pub use persist::Persister;
pub use session::{Fingerprint, Session};
pub use settings::{today, Settings, Theme, CUSTOM_PRESET_ID};
pub use timer::{
    builtin_presets, default_preset, format_clock, preset_by_id, Behavior, Phase, Preset, RunState,
    Snapshot, Timer, Transition,
};
