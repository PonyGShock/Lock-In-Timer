use std::fs;
use std::path::{Path, PathBuf};

use crate::chime::ChimeVoice;
use crate::noise::NoiseKind;
use crate::timer::{default_preset, preset_by_id, Behavior, Preset};
use chrono::Local;
use serde::{Deserialize, Serialize};

pub const CUSTOM_PRESET_ID: &str = "custom";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Settings {
    pub preset_id: String,
    pub custom_preset: Preset,
    pub behavior: Behavior,

    pub chime_enabled: bool,
    pub chime_voice: ChimeVoice,
    pub chime_volume: f32,

    pub noise_enabled: bool,
    pub noise_kind: NoiseKind,
    pub noise_volume: f32,
    pub noise_tone: f32,
    pub noise_during_breaks: bool,

    pub notifications_enabled: bool,
    pub show_clock_in_menu_bar: bool,
    pub launch_at_login: bool,
    pub theme: Theme,

    /// Focus sessions finished on `completed_date`, so the count on the face
    /// is today's rather than all time.
    pub completed_focus: u32,
    pub completed_date: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum Theme {
    #[default]
    System,
    Light,
    Dark,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            preset_id: default_preset().id,
            custom_preset: Preset::new(CUSTOM_PRESET_ID, "Custom", 30 * 60, 6 * 60, 20 * 60, 4),
            behavior: Behavior::default(),

            chime_enabled: true,
            chime_voice: ChimeVoice::Bell,
            chime_volume: 0.7,

            noise_enabled: false,
            noise_kind: NoiseKind::Deep,
            noise_volume: 0.35,
            noise_tone: 0.55,
            noise_during_breaks: false,

            notifications_enabled: true,
            show_clock_in_menu_bar: true,
            launch_at_login: false,
            theme: Theme::System,

            completed_focus: 0,
            completed_date: today(),
        }
    }
}

impl Settings {
    /// Brings anything that arrived from disk or the UI back into range.
    pub fn sanitized(mut self) -> Self {
        self.chime_volume = self.chime_volume.clamp(0.0, 1.0);
        self.noise_volume = self.noise_volume.clamp(0.0, 1.0);
        self.noise_tone = self.noise_tone.clamp(0.0, 1.0);
        self.custom_preset = self.custom_preset.sanitized();
        self.custom_preset.id = CUSTOM_PRESET_ID.to_string();
        if preset_by_id(&self.preset_id).is_none() && self.preset_id != CUSTOM_PRESET_ID {
            self.preset_id = default_preset().id;
        }
        self
    }

    /// The preset the timer should actually run.
    pub fn active_preset(&self) -> Preset {
        if self.preset_id == CUSTOM_PRESET_ID {
            self.custom_preset.clone().sanitized()
        } else {
            preset_by_id(&self.preset_id).unwrap_or_else(default_preset)
        }
    }

    /// Zeroes the session count when the calendar day has moved on.
    pub fn roll_over_day(&mut self) -> bool {
        let today = today();
        if self.completed_date != today {
            self.completed_date = today;
            self.completed_focus = 0;
            return true;
        }
        false
    }

    pub fn load(path: &Path) -> Self {
        let Ok(raw) = fs::read_to_string(path) else {
            return Settings::default();
        };
        // A corrupt or older file should never stop the app from opening;
        // falling back to defaults loses preferences, not the app.
        serde_json::from_str::<Settings>(&raw)
            .unwrap_or_default()
            .sanitized()
    }

    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string_pretty(self)?;
        // Write beside the target and rename, so a crash mid-write cannot
        // leave a half-written settings file behind.
        let tmp = temp_path(path);
        fs::write(&tmp, json)?;
        fs::rename(&tmp, path)
    }
}

fn temp_path(path: &Path) -> PathBuf {
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(".tmp");
    path.with_file_name(name)
}

fn today() -> String {
    Local::now().format("%Y-%m-%d").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_already_in_range() {
        let defaults = Settings::default();
        assert_eq!(defaults.clone(), defaults.sanitized());
    }

    #[test]
    fn out_of_range_values_are_pulled_back() {
        let settings = Settings {
            chime_volume: 9.0,
            noise_volume: -3.0,
            noise_tone: 5.0,
            preset_id: "nonsense".to_string(),
            ..Settings::default()
        }
        .sanitized();

        assert_eq!(settings.chime_volume, 1.0);
        assert_eq!(settings.noise_volume, 0.0);
        assert_eq!(settings.noise_tone, 1.0);
        assert_eq!(settings.preset_id, default_preset().id);
    }

    #[test]
    fn custom_preset_selection_is_preserved() {
        let settings = Settings {
            preset_id: CUSTOM_PRESET_ID.to_string(),
            ..Settings::default()
        }
        .sanitized();
        assert_eq!(settings.preset_id, CUSTOM_PRESET_ID);
        assert_eq!(settings.active_preset().focus_secs, 30 * 60);
    }

    #[test]
    fn a_missing_file_yields_defaults() {
        let settings = Settings::load(Path::new("/definitely/not/here/settings.json"));
        assert_eq!(settings, Settings::default());
    }

    #[test]
    fn partial_files_fill_in_the_rest() {
        let dir = std::env::temp_dir().join(format!("lockin-partial-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("settings.json");
        fs::write(&path, r#"{"noiseKind":"rain","chimeVolume":0.25}"#).unwrap();

        let settings = Settings::load(&path);
        assert_eq!(settings.noise_kind, NoiseKind::Rain);
        assert_eq!(settings.chime_volume, 0.25);
        assert_eq!(settings.preset_id, default_preset().id);

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_corrupt_file_does_not_break_startup() {
        let dir = std::env::temp_dir().join(format!("lockin-corrupt-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("settings.json");
        fs::write(&path, "}{ not json at all").unwrap();

        assert_eq!(Settings::load(&path), Settings::default());
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn settings_survive_a_save_and_load_round_trip() {
        let dir = std::env::temp_dir().join(format!("lockin-roundtrip-{}", std::process::id()));
        let path = dir.join("nested").join("settings.json");

        let original = Settings {
            noise_enabled: true,
            noise_kind: NoiseKind::Rain,
            chime_voice: ChimeVoice::Bowl,
            preset_id: "deep".to_string(),
            ..Settings::default()
        };
        original.save(&path).unwrap();

        assert_eq!(Settings::load(&path), original);
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn the_day_counter_resets_when_the_date_changes() {
        let mut settings = Settings {
            completed_focus: 6,
            completed_date: "2001-01-01".to_string(),
            ..Settings::default()
        };
        assert!(settings.roll_over_day());
        assert_eq!(settings.completed_focus, 0);
        assert_eq!(settings.completed_date, today());
        assert!(!settings.roll_over_day(), "a second call is a no-op");
    }
}
