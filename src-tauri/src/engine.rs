use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::Instant;

use crema_core::{
    chime::frequency_for,
    noise::NoiseKind,
    timer::{builtin_presets, Preset, Snapshot, Timer, Transition},
    Phase, RunState,
};
use serde::Serialize;

use crate::audio::Audio;
use crema_core::settings::{Settings, CUSTOM_PRESET_ID};

/// Everything the window needs in a single payload, so the UI never has to
/// stitch several calls together.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppState {
    pub timer: Snapshot,
    pub settings: Settings,
    pub presets: Vec<Preset>,
}

/// The noise configuration currently handed to the audio thread. Compared as
/// integers so float jitter never counts as a change.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct NoiseWish {
    enabled: bool,
    kind: NoiseKind,
    volume: u32,
    tone: u32,
}

pub struct Engine {
    timer: Mutex<Timer>,
    settings: Mutex<Settings>,
    last_noise: Mutex<Option<NoiseWish>>,
    audio: Audio,
    settings_path: PathBuf,
    started: Instant,
    ticks: AtomicU64,
}

impl Engine {
    pub fn new(settings: Settings, settings_path: PathBuf, audio: Audio) -> Self {
        let mut settings = settings.sanitized();
        settings.roll_over_day();

        let mut timer = Timer::new(settings.active_preset(), settings.behavior);
        timer.set_completed_focus(settings.completed_focus);

        Engine {
            timer: Mutex::new(timer),
            settings: Mutex::new(settings),
            last_noise: Mutex::new(None),
            audio,
            settings_path,
            started: Instant::now(),
            ticks: AtomicU64::new(0),
        }
    }

    fn now_ms(&self) -> u64 {
        self.started.elapsed().as_millis() as u64
    }

    pub fn settings(&self) -> Settings {
        self.settings.lock().unwrap().clone()
    }

    pub fn state(&self) -> AppState {
        let timer = self.timer.lock().unwrap().snapshot();
        let settings = self.settings.lock().unwrap().clone();
        let mut presets = builtin_presets();
        presets.push(settings.custom_preset.clone());
        AppState {
            timer,
            settings,
            presets,
        }
    }

    // --- transport -------------------------------------------------------

    pub fn start(&self) {
        {
            let mut timer = self.timer.lock().unwrap();
            // Re-anchor first: time spent idle or paused must not be charged
            // to the phase the moment it resumes.
            timer.sync_clock(self.now_ms());
            timer.start();
        }
        self.sync_noise();
    }

    pub fn pause(&self) {
        self.timer.lock().unwrap().pause();
        self.sync_noise();
    }

    pub fn toggle(&self) {
        let running = self.timer.lock().unwrap().is_running();
        if running {
            self.pause();
        } else {
            self.start();
        }
    }

    pub fn reset(&self) {
        self.timer.lock().unwrap().reset();
        self.sync_noise();
    }

    pub fn restart_phase(&self) {
        self.timer.lock().unwrap().restart_phase();
        self.sync_noise();
    }

    /// Ends the current phase by hand. No chime: the user already knows.
    pub fn skip(&self) {
        {
            let mut timer = self.timer.lock().unwrap();
            timer.sync_clock(self.now_ms());
            timer.skip();
        }
        self.sync_noise();
    }

    // --- configuration ---------------------------------------------------

    pub fn set_preset(&self, id: &str) {
        let preset = {
            let mut settings = self.settings.lock().unwrap();
            settings.preset_id = id.to_string();
            *settings = settings.clone().sanitized();
            settings.active_preset()
        };
        {
            let mut timer = self.timer.lock().unwrap();
            timer.sync_clock(self.now_ms());
            timer.set_preset(preset);
        }
        self.persist();
        self.sync_noise();
    }

    pub fn set_custom_preset(&self, focus_secs: u32, short_secs: u32, long_secs: u32, rounds: u32) {
        let preset = {
            let mut settings = self.settings.lock().unwrap();
            settings.custom_preset = Preset::new(
                CUSTOM_PRESET_ID,
                "Custom",
                focus_secs,
                short_secs,
                long_secs,
                rounds,
            )
            .sanitized();
            settings.preset_id = CUSTOM_PRESET_ID.to_string();
            settings.active_preset()
        };
        {
            let mut timer = self.timer.lock().unwrap();
            timer.sync_clock(self.now_ms());
            timer.set_preset(preset);
        }
        self.persist();
        self.sync_noise();
    }

    /// Applies a whole settings object from the UI. The running phase is left
    /// alone unless the active preset's durations actually moved.
    pub fn update_settings(&self, incoming: Settings) {
        let (behavior, preset, preset_changed) = {
            let mut settings = self.settings.lock().unwrap();
            let previous = settings.active_preset();
            let mut incoming = incoming.sanitized();
            // The session count belongs to the engine, not the form.
            incoming.completed_focus = settings.completed_focus;
            incoming.completed_date = settings.completed_date.clone();
            *settings = incoming;
            let preset = settings.active_preset();
            (settings.behavior, preset.clone(), preset != previous)
        };

        {
            let mut timer = self.timer.lock().unwrap();
            timer.set_behavior(behavior);
            if preset_changed {
                timer.sync_clock(self.now_ms());
                timer.set_preset(preset);
            }
        }

        self.persist();
        self.sync_noise();
    }

    pub fn preview_chime(&self) {
        let settings = self.settings.lock().unwrap();
        self.audio.chime(
            settings.chime_voice,
            frequency_for(Phase::Focus),
            settings.chime_volume,
        );
    }

    // --- running ---------------------------------------------------------

    /// Drives the clock forward. Returns the transition to announce, if any.
    pub fn tick(&self) -> Option<Transition> {
        let transition = self.timer.lock().unwrap().advance(self.now_ms());

        if let Some(transition) = transition {
            if transition.completed {
                let completed = self.timer.lock().unwrap().completed_focus();
                let settings = {
                    let mut settings = self.settings.lock().unwrap();
                    settings.completed_focus = completed;
                    settings.clone()
                };
                if settings.chime_enabled {
                    self.audio.chime(
                        settings.chime_voice,
                        frequency_for(transition.ended),
                        settings.chime_volume,
                    );
                }
                self.persist();
            }
            self.sync_noise();
        } else if self.ticks.fetch_add(1, Ordering::Relaxed) % 300 == 0 {
            // Roughly once a minute, so the day's count clears at midnight
            // even when the app is left running.
            self.roll_over_day();
        }

        transition
    }

    pub fn roll_over_day(&self) {
        let rolled = self.settings.lock().unwrap().roll_over_day();
        if rolled {
            self.timer.lock().unwrap().set_completed_focus(0);
            self.persist();
        }
    }

    /// Recomputes whether noise should be playing and tells the audio thread
    /// only when the answer has changed.
    pub fn sync_noise(&self) {
        let settings = self.settings.lock().unwrap().clone();
        let snapshot = self.timer.lock().unwrap().snapshot();

        let should_play = settings.noise_enabled
            && snapshot.state == RunState::Running
            && (snapshot.phase == Phase::Focus || settings.noise_during_breaks);

        let wish = NoiseWish {
            enabled: should_play,
            kind: settings.noise_kind,
            volume: (settings.noise_volume * 1000.0) as u32,
            tone: (settings.noise_tone * 1000.0) as u32,
        };

        let mut last = self.last_noise.lock().unwrap();
        if *last == Some(wish) {
            return;
        }
        *last = Some(wish);
        drop(last);

        self.audio.set_noise(
            should_play,
            settings.noise_kind,
            settings.noise_volume,
            settings.noise_tone,
        );
    }

    pub fn persist(&self) {
        let settings = self.settings.lock().unwrap().clone();
        if let Err(error) = settings.save(&self.settings_path) {
            eprintln!("crema: could not save settings: {error}");
        }
    }
}
