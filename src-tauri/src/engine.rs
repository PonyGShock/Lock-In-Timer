//! The desktop app's handle on the session.
//!
//! The rules live in `lockin_core::Session`, where they are tested. This adds
//! what a running app needs around them: a clock, the audio thread, the
//! settings writer, and one lock so that every command sees and leaves the
//! session in a consistent state. Saves are queued while the lock is held, so
//! they reach the writer in the order the changes were made; the writing
//! itself happens on the writer's thread, so a slow disk never stalls the
//! timer. Sound is sent after the lock is released.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, MutexGuard, PoisonError};
use std::time::Instant;

use lockin_core::{
    builtin_presets, frequency_for, today, Fingerprint, Persister, Preset, Session, Settings,
    Snapshot, Transition,
};
use serde::Serialize;
use serde_json::Value;

use crate::audio::Audio;

/// Everything the window needs in a single payload, so the UI never has to
/// stitch several calls together.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppState {
    pub timer: Snapshot,
    pub settings: Settings,
    pub presets: Vec<Preset>,
    /// `macos`, `windows`, `linux`, ... Some settings only mean anything on
    /// one platform, and the interface hides them elsewhere.
    pub platform: &'static str,
}

/// The noise state last handed to the audio thread. Volume is compared as an
/// integer so float jitter never counts as a change.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct NoiseWish {
    enabled: bool,
    volume: u32,
}

/// Ticks between checks for a new calendar day: about once a minute.
const DAY_CHECK_TICKS: u64 = 300;

pub struct Engine {
    session: Mutex<Session>,
    noise: Mutex<Option<NoiseWish>>,
    audio: Audio,
    persister: Persister,
    started: Instant,
    ticks: AtomicU64,
}

impl Engine {
    pub fn new(settings: Settings, persister: Persister, audio: Audio) -> Self {
        Engine {
            session: Mutex::new(Session::new(settings, &today())),
            noise: Mutex::new(None),
            audio,
            persister,
            started: Instant::now(),
            ticks: AtomicU64::new(0),
        }
    }

    fn now_ms(&self) -> u64 {
        self.started.elapsed().as_millis() as u64
    }

    /// A panic while holding the lock leaves the session as it was, which is
    /// still a perfectly good session; refusing every later command over it
    /// would be worse.
    fn session(&self) -> MutexGuard<'_, Session> {
        self.session.lock().unwrap_or_else(PoisonError::into_inner)
    }

    pub fn state(&self) -> AppState {
        let session = self.session();
        let settings = session.settings().clone();
        let mut presets = builtin_presets();
        presets.push(settings.custom_preset.clone());
        AppState {
            timer: session.snapshot(),
            settings,
            presets,
            platform: std::env::consts::OS,
        }
    }

    pub fn settings(&self) -> Settings {
        self.session().settings().clone()
    }

    pub fn fingerprint(&self) -> Fingerprint {
        self.session().fingerprint()
    }

    // --- commands --------------------------------------------------------

    pub fn toggle(&self) {
        let now = self.now_ms();
        self.session().toggle(now);
        self.sync_noise();
    }

    pub fn reset(&self) {
        self.session().reset();
        self.sync_noise();
    }

    /// Ends the current phase by hand. No chime: the user already knows.
    pub fn skip(&self) {
        let now = self.now_ms();
        self.session().skip(now);
        self.sync_noise();
    }

    pub fn select_preset(&self, id: &str) {
        {
            let mut session = self.session();
            if session.select_preset(id) {
                self.persister.save(session.settings());
            }
        }
        self.sync_noise();
    }

    pub fn apply_patch(&self, patch: &Value) {
        {
            let mut session = self.session();
            if session.apply_patch(patch) {
                self.persister.save(session.settings());
            }
        }
        self.sync_noise();
    }

    pub fn preview_chime(&self) {
        let settings = self.settings();
        self.audio.chime(
            settings.chime_voice,
            frequency_for(lockin_core::Phase::Focus),
            settings.chime_volume,
        );
    }

    /// Writes any pending settings to disk and waits for it. Called on exit.
    pub fn flush(&self) {
        self.persister.flush();
    }

    // --- running ---------------------------------------------------------

    /// Drives the clock forward. Returns the transition to announce, if any.
    pub fn tick(&self) -> Option<Transition> {
        let now = self.now_ms();
        let check_day = self.ticks.fetch_add(1, Ordering::Relaxed) % DAY_CHECK_TICKS == 0;

        let (transition, chime) = {
            let mut session = self.session();
            let transition = session.advance(now);
            let finished = transition.filter(|t| t.completed);
            let rolled = check_day && session.roll_over_day(&today());

            let settings = session.settings();
            if finished.is_some() || rolled {
                // Under the lock: a copy taken here and queued after release
                // could land behind a newer one from a command, and the file
                // would keep the older settings.
                self.persister.save(settings);
            }
            let chime = finished.filter(|_| settings.chime_enabled).map(|t| {
                (
                    settings.chime_voice,
                    frequency_for(t.ended),
                    settings.chime_volume,
                )
            });
            (transition, chime)
        };

        if let Some((voice, frequency, volume)) = chime {
            self.audio.chime(voice, frequency, volume);
        }
        // Every tick, not only on a change: if a command to the audio thread
        // was ever dropped, the next tick notices and sends it again.
        self.sync_noise();
        transition
    }

    /// Tells the audio thread whether the rumble should play, when that has
    /// changed since it was last told.
    fn sync_noise(&self) {
        let (enabled, volume) = {
            let session = self.session();
            (session.noise_wanted(), session.settings().noise_volume)
        };
        let wish = NoiseWish {
            enabled,
            volume: (volume * 1000.0).round() as u32,
        };

        let mut last = self.noise.lock().unwrap_or_else(PoisonError::into_inner);
        if *last == Some(wish) {
            return;
        }
        // Remember the wish only once the audio thread has actually taken it.
        // Recording it first meant a dropped command left this cache claiming
        // a state the audio never reached, and noise played on through breaks.
        if self.audio.set_noise(enabled, volume) {
            *last = Some(wish);
        }
    }
}
