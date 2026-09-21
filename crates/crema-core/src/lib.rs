//! Core logic for Crema: a calm pomodoro timer.
//!
//! This crate holds the parts worth testing on their own — the phase state
//! machine, the audio synthesis and the settings file — and depends on no
//! system libraries, so it builds and tests anywhere without a windowing or
//! audio toolkit present.

pub mod chime;
pub mod noise;
pub mod settings;
pub mod timer;

pub use chime::{frequency_for, ChimeVoice};
pub use noise::{NoiseKind, NoiseSource, DEFAULT_FADE_MS};
pub use settings::{Settings, Theme, CUSTOM_PRESET_ID};
pub use timer::{
    builtin_presets, default_preset, format_clock, preset_by_id, Behavior, Phase, Preset, RunState,
    Snapshot, Timer, Transition,
};
