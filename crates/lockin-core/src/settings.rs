use std::fs;
use std::path::{Path, PathBuf};

use crate::chime::ChimeVoice;
use crate::timer::{default_preset, preset_by_id, Behavior, Preset};
use chrono::Local;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

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
    pub noise_volume: f32,
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
            noise_volume: 0.35,
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
    pub fn roll_over_day(&mut self, today: &str) -> bool {
        if self.completed_date != today {
            self.completed_date = today.to_string();
            self.completed_focus = 0;
            return true;
        }
        false
    }

    /// Lays `overlay` over these settings, one value at a time.
    ///
    /// Each value is kept only if the settings still make sense with it in
    /// place, so a single bad entry — an unknown chime voice, text where a
    /// number belongs — costs that one entry rather than everything around
    /// it. Nested objects merge rather than replace, so an overlay naming one
    /// field of the custom preset leaves the others alone.
    ///
    /// The same rules serve a settings file from an older or newer build and
    /// a partial update from the interface.
    pub fn merged(&self, overlay: &Value) -> Settings {
        let mut accepted = serde_json::to_value(self).expect("settings always serialise");
        let mut changes = Vec::new();
        collect_changes(&accepted, overlay, &mut Vec::new(), &mut changes);

        for (path, value) in changes {
            let mut candidate = accepted.clone();
            set_at(&mut candidate, &path, value);
            if serde_json::from_value::<Settings>(candidate.clone()).is_ok() {
                accepted = candidate;
            }
        }

        serde_json::from_value::<Settings>(accepted)
            .map(Settings::sanitized)
            .unwrap_or_else(|_| self.clone())
    }

    pub fn load(path: &Path) -> Self {
        let defaults = Settings::default();
        let Ok(raw) = fs::read_to_string(path) else {
            return defaults;
        };
        // A corrupt file should never stop the app from opening. Text that is
        // not JSON at all costs the preferences; anything that parses keeps
        // every value it can.
        match serde_json::from_str::<Value>(&raw) {
            Ok(value) => defaults.merged(&value),
            Err(_) => defaults,
        }
    }

    /// Writes atomically: beside the target first, then renamed over it, so a
    /// crash mid-write cannot leave a half-written file behind.
    ///
    /// The temporary name is fixed, which is only safe with a single writer.
    /// [`crate::persist::Persister`] is that writer; nothing else should call
    /// this outside tests.
    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string_pretty(self)?;
        let tmp = temp_path(path);
        fs::write(&tmp, json)?;
        fs::rename(&tmp, path)
    }
}

/// Flattens an overlay into individual changes. Recursion stops wherever the
/// existing value is not an object: there the overlay replaces it wholesale.
fn collect_changes(
    base: &Value,
    overlay: &Value,
    path: &mut Vec<String>,
    out: &mut Vec<(Vec<String>, Value)>,
) {
    match (base, overlay) {
        (Value::Object(base), Value::Object(overlay)) => {
            for (key, value) in overlay {
                path.push(key.clone());
                let existing = base.get(key).unwrap_or(&Value::Null);
                collect_changes(existing, value, path, out);
                path.pop();
            }
        }
        _ if !path.is_empty() => out.push((path.clone(), overlay.clone())),
        // A top-level overlay that is not an object has nothing to apply.
        _ => {}
    }
}

fn set_at(target: &mut Value, path: &[String], value: Value) {
    let Some((last, parents)) = path.split_last() else {
        return;
    };
    let mut node = target;
    for key in parents {
        if !node.is_object() {
            *node = Value::Object(Map::new());
        }
        node = node
            .as_object_mut()
            .expect("just ensured")
            .entry(key.clone())
            .or_insert(Value::Null);
    }
    if !node.is_object() {
        *node = Value::Object(Map::new());
    }
    node.as_object_mut()
        .expect("just ensured")
        .insert(last.clone(), value);
}

fn temp_path(path: &Path) -> PathBuf {
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(".tmp");
    path.with_file_name(name)
}

/// Today's date in the user's own time zone, as the day counter stores it.
pub fn today() -> String {
    Local::now().format("%Y-%m-%d").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// A scratch directory unique to one test, so parallel tests never share
    /// a settings file.
    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("lockin-{name}-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn load_text(name: &str, text: &str) -> Settings {
        let dir = scratch(name);
        let path = dir.join("settings.json");
        fs::write(&path, text).unwrap();
        let settings = Settings::load(&path);
        fs::remove_dir_all(&dir).ok();
        settings
    }

    /// Defaults with the date pinned, so a comparison cannot straddle midnight.
    fn defaults_dated(date: &str) -> Settings {
        Settings {
            completed_date: date.to_string(),
            ..Settings::default()
        }
    }

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
            preset_id: "nonsense".to_string(),
            ..Settings::default()
        }
        .sanitized();

        assert_eq!(settings.chime_volume, 1.0);
        assert_eq!(settings.noise_volume, 0.0);
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
        assert_eq!(settings, defaults_dated(&settings.completed_date));
    }

    #[test]
    fn a_file_that_is_not_json_at_all_yields_defaults() {
        let settings = load_text("garbage", "}{ not json at all");
        assert_eq!(settings, defaults_dated(&settings.completed_date));
    }

    #[test]
    fn fields_this_version_does_not_know_are_ignored() {
        let settings = load_text(
            "unknown-field",
            r#"{"noiseKind":"rain","chimeVolume":0.25}"#,
        );
        assert_eq!(settings.chime_volume, 0.25);
        assert_eq!(settings.preset_id, default_preset().id);
    }

    #[test]
    fn one_unknown_value_costs_only_that_setting() {
        // Previously this whole file loaded as defaults, because "gong" is not
        // a chime voice and one failed field failed everything.
        let settings = load_text(
            "unknown-value",
            r#"{"chimeVoice":"gong","presetId":"deep","chimeVolume":0.25,"noiseEnabled":true}"#,
        );
        assert_eq!(
            settings.chime_voice,
            ChimeVoice::Bell,
            "the bad value falls back"
        );
        assert_eq!(settings.preset_id, "deep", "the rest survive");
        assert_eq!(settings.chime_volume, 0.25);
        assert!(settings.noise_enabled);
    }

    #[test]
    fn a_partial_nested_object_keeps_its_other_fields() {
        // Previously this also loaded as defaults: the custom preset had no
        // default for the fields it did not mention.
        let settings = load_text(
            "partial-nested",
            r#"{"presetId":"deep","customPreset":{"focusSecs":2700}}"#,
        );
        assert_eq!(settings.preset_id, "deep");
        assert_eq!(settings.custom_preset.focus_secs, 2700);
        assert_eq!(settings.custom_preset.short_break_secs, 6 * 60);
        assert_eq!(settings.custom_preset.rounds, 4);
    }

    #[test]
    fn a_wrong_type_is_ignored_rather_than_coerced() {
        let settings = load_text(
            "wrong-type",
            r#"{"chimeVolume":"loud","noiseVolume":0.5,"customPreset":{"rounds":"many","focusSecs":1200}}"#,
        );
        assert_eq!(settings.chime_volume, 0.7);
        assert_eq!(settings.noise_volume, 0.5);
        assert_eq!(settings.custom_preset.rounds, 4);
        assert_eq!(settings.custom_preset.focus_secs, 1200);
    }

    #[test]
    fn a_merge_changes_only_what_it_names() {
        let base = Settings {
            preset_id: "deep".to_string(),
            noise_enabled: true,
            ..Settings::default()
        };
        let merged = base.merged(&json!({ "chimeVolume": 0.2 }));
        assert_eq!(merged.chime_volume, 0.2);
        assert_eq!(
            merged.preset_id, "deep",
            "unmentioned settings are untouched"
        );
        assert!(merged.noise_enabled);
    }

    #[test]
    fn a_merged_value_is_still_brought_into_range() {
        let merged = Settings::default().merged(&json!({ "noiseVolume": 7.5 }));
        assert_eq!(merged.noise_volume, 1.0);
    }

    #[test]
    fn an_overlay_that_is_not_an_object_changes_nothing() {
        let base = Settings::default();
        for overlay in [json!(null), json!(3), json!("x"), json!([1, 2])] {
            assert_eq!(base.merged(&overlay), base, "{overlay}");
        }
    }

    #[test]
    fn the_custom_preset_cannot_be_renamed_through_a_merge() {
        let merged = Settings::default().merged(&json!({ "customPreset": { "id": "classic" } }));
        assert_eq!(merged.custom_preset.id, CUSTOM_PRESET_ID);
    }

    #[test]
    fn settings_survive_a_save_and_load_round_trip() {
        let dir = scratch("roundtrip");
        let path = dir.join("nested").join("settings.json");

        let original = Settings {
            noise_enabled: true,
            chime_voice: ChimeVoice::Bowl,
            preset_id: "deep".to_string(),
            completed_date: "2026-01-02".to_string(),
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
        assert!(settings.roll_over_day("2001-01-02"));
        assert_eq!(settings.completed_focus, 0);
        assert_eq!(settings.completed_date, "2001-01-02");
        assert!(
            !settings.roll_over_day("2001-01-02"),
            "a second call is a no-op"
        );
    }
}
