//! The timer and the settings that drive it, kept consistent with each other.
//!
//! These rules used to live in the desktop app, next to the audio device and
//! the window, where nothing tested them, and that is where the two worst bugs
//! were: tapping the preset that was already selected threw away the session
//! in progress, and editing the custom lengths silently switched to the Custom
//! preset and threw it away too. Here they are plain functions over plain
//! data, and the tests below pin each of them down.

use serde_json::Value;

use crate::settings::{Settings, CUSTOM_PRESET_ID};
use crate::timer::{preset_by_id, Phase, RunState, Snapshot, Timer, Transition};

pub struct Session {
    timer: Timer,
    settings: Settings,
}

/// The few timer facts that decide whether the interface needs redrawing.
/// Copy-only, so checking it every tick costs no allocation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Fingerprint {
    pub remaining_secs: u32,
    pub running: bool,
    pub paused: bool,
    pub phase: Phase,
    pub round: u32,
    pub completed_focus: u32,
}

impl Session {
    pub fn new(settings: Settings, today: &str) -> Self {
        let mut settings = settings.sanitized();
        settings.roll_over_day(today);
        let mut timer = Timer::new(settings.active_preset(), settings.behavior);
        timer.set_completed_focus(settings.completed_focus);
        Session { timer, settings }
    }

    pub fn settings(&self) -> &Settings {
        &self.settings
    }

    pub fn snapshot(&self) -> Snapshot {
        self.timer.snapshot()
    }

    pub fn fingerprint(&self) -> Fingerprint {
        let state = self.timer.state();
        Fingerprint {
            remaining_secs: self.timer.remaining_secs(),
            running: state == RunState::Running,
            paused: state == RunState::Paused,
            phase: self.timer.phase(),
            round: self.timer.round(),
            completed_focus: self.timer.completed_focus(),
        }
    }

    pub fn toggle(&mut self, now_ms: u64) {
        self.timer.toggle(now_ms);
    }

    pub fn reset(&mut self) {
        self.timer.reset();
    }

    pub fn skip(&mut self, now_ms: u64) -> Transition {
        self.timer.skip(now_ms)
    }

    /// Selects a preset. Selecting the one already active does nothing at
    /// all: the chip for the current preset looks like a toggle, and tapping
    /// it must never cost the session that is running. Returns whether the
    /// settings changed.
    pub fn select_preset(&mut self, id: &str) -> bool {
        let known = id == CUSTOM_PRESET_ID || preset_by_id(id).is_some();
        if !known || self.settings.preset_id == id {
            return false;
        }
        self.settings.preset_id = id.to_string();
        self.timer.set_preset(self.settings.active_preset());
        true
    }

    /// Applies a partial update from the interface. Only the values the
    /// patch names change; see [`Settings::merged`].
    ///
    /// What happens to the timer depends on what changed:
    /// - a different preset id is a deliberate switch, so the cycle restarts;
    /// - new lengths for the preset already running retune it in place and
    ///   keep the session going;
    /// - anything else — including the custom lengths while another preset is
    ///   running — leaves the timer alone.
    ///
    /// Returns whether the settings changed.
    pub fn apply_patch(&mut self, patch: &Value) -> bool {
        let before = self.settings.clone();
        let mut next = before.merged(patch);
        // The day's count belongs to the session, not to the form.
        next.completed_focus = before.completed_focus;
        next.completed_date = before.completed_date.clone();
        if next == before {
            return false;
        }

        let old_active = before.active_preset();
        self.settings = next;
        let new_active = self.settings.active_preset();

        self.timer.set_behavior(self.settings.behavior);
        if self.settings.preset_id != before.preset_id {
            self.timer.set_preset(new_active);
        } else if new_active != old_active {
            self.timer.retune(new_active);
        }
        true
    }

    /// Feeds time to the timer and credits any finished focus session to the
    /// day's count.
    pub fn advance(&mut self, now_ms: u64) -> Option<Transition> {
        let transition = self.timer.advance(now_ms)?;
        self.settings.completed_focus = self.timer.completed_focus();
        Some(transition)
    }

    /// Clears the day's count once the date moves on. Returns whether it did.
    pub fn roll_over_day(&mut self, today: &str) -> bool {
        if self.settings.roll_over_day(today) {
            self.timer.set_completed_focus(0);
            return true;
        }
        false
    }

    /// Whether the rumble should be audible right now.
    pub fn noise_wanted(&self) -> bool {
        self.settings.noise_enabled
            && self.timer.is_running()
            && (self.timer.phase() == Phase::Focus || self.settings.noise_during_breaks)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const TODAY: &str = "2026-03-04";

    fn session() -> Session {
        Session::new(Settings::default(), TODAY)
    }

    /// Twenty minutes into a 25 minute Classic focus session.
    fn mid_session() -> Session {
        let mut s = session();
        s.toggle(0);
        s.advance(20 * 60 * 1000);
        assert_eq!(s.snapshot().remaining_secs, 5 * 60);
        s
    }

    #[test]
    fn reselecting_the_running_preset_keeps_the_session() {
        let mut s = mid_session();
        let changed = s.select_preset("classic");
        assert!(!changed);
        let snap = s.snapshot();
        assert_eq!(
            snap.remaining_secs,
            5 * 60,
            "twenty minutes of work survive"
        );
        assert_eq!(snap.state, RunState::Running);
    }

    #[test]
    fn selecting_a_different_preset_starts_its_cycle_fresh() {
        let mut s = mid_session();
        assert!(s.select_preset("deep"));
        let snap = s.snapshot();
        assert_eq!(snap.preset.id, "deep");
        assert_eq!(snap.remaining_secs, 50 * 60);
        assert_eq!(snap.state, RunState::Idle);
    }

    #[test]
    fn an_unknown_preset_id_is_ignored() {
        let mut s = mid_session();
        assert!(!s.select_preset("does-not-exist"));
        assert_eq!(s.snapshot().remaining_secs, 5 * 60);
        assert_eq!(s.settings().preset_id, "classic");
    }

    #[test]
    fn editing_custom_lengths_leaves_another_running_preset_alone() {
        // The other half of the reported bug: changing Rounds while Deep was
        // running used to switch to Custom and reset to 30:00.
        let mut s = session();
        s.select_preset("deep");
        s.toggle(0);
        s.advance(40 * 60 * 1000);

        assert!(s.apply_patch(&json!({ "customPreset": { "rounds": 5 } })));
        let snap = s.snapshot();
        assert_eq!(s.settings().preset_id, "deep", "still on Deep");
        assert_eq!(snap.remaining_secs, 10 * 60, "forty minutes kept");
        assert_eq!(snap.state, RunState::Running);
        assert_eq!(
            s.settings().custom_preset.rounds,
            5,
            "and the edit was saved"
        );
    }

    #[test]
    fn editing_custom_lengths_while_custom_runs_keeps_the_session() {
        let mut s = session();
        s.select_preset(CUSTOM_PRESET_ID);
        s.toggle(0);
        s.advance(10 * 60 * 1000); // 20 of 30 minutes left
        s.apply_patch(&json!({ "customPreset": { "focusSecs": 45 * 60 } }));
        let snap = s.snapshot();
        assert_eq!(snap.remaining_secs, 20 * 60, "time left is kept");
        assert_eq!(snap.total_secs, 45 * 60, "under the new length");
        assert_eq!(snap.state, RunState::Running);
    }

    #[test]
    fn editing_custom_lengths_before_starting_shows_the_new_length() {
        let mut s = session();
        s.select_preset(CUSTOM_PRESET_ID);
        s.apply_patch(&json!({ "customPreset": { "focusSecs": 35 * 60 } }));
        assert_eq!(s.snapshot().remaining_secs, 35 * 60);
    }

    #[test]
    fn a_patch_naming_a_different_preset_switches_deliberately() {
        let mut s = mid_session();
        s.apply_patch(&json!({ "presetId": "espresso" }));
        let snap = s.snapshot();
        assert_eq!(snap.preset.id, "espresso");
        assert_eq!(snap.remaining_secs, 15 * 60);
    }

    #[test]
    fn an_unrelated_patch_does_not_touch_the_timer() {
        let mut s = mid_session();
        assert!(s.apply_patch(&json!({ "noiseEnabled": true, "chimeVolume": 0.3 })));
        assert_eq!(s.snapshot().remaining_secs, 5 * 60);
        assert_eq!(s.snapshot().state, RunState::Running);
    }

    #[test]
    fn a_patch_that_changes_nothing_reports_no_change() {
        let mut s = session();
        assert!(!s.apply_patch(&json!({ "chimeVolume": 0.7 })));
        assert!(!s.apply_patch(&json!({})));
    }

    #[test]
    fn a_patch_cannot_rewrite_the_days_count() {
        let mut s = session();
        s.toggle(0);
        s.advance(25 * 60 * 1000); // one focus session done
        s.apply_patch(&json!({ "completedFocus": 99, "completedDate": "1999-01-01" }));
        assert_eq!(s.settings().completed_focus, 1);
        assert_eq!(s.settings().completed_date, TODAY);
    }

    #[test]
    fn a_bad_value_in_a_patch_is_dropped_and_the_rest_applied() {
        let mut s = session();
        s.apply_patch(&json!({ "chimeVoice": "gong", "noiseVolume": 0.9 }));
        assert_eq!(s.settings().noise_volume, 0.9);
    }

    #[test]
    fn changing_auto_start_takes_effect_on_the_running_timer() {
        let mut s = session();
        s.apply_patch(&json!({ "behavior": { "autoStartBreaks": false } }));
        s.toggle(0);
        let transition = s.advance(25 * 60 * 1000).unwrap();
        assert!(!transition.auto_started);
        assert_eq!(s.snapshot().state, RunState::Idle);
    }

    #[test]
    fn finishing_focus_credits_the_days_count() {
        let mut s = session();
        s.toggle(0);
        s.advance(25 * 60 * 1000);
        assert_eq!(s.settings().completed_focus, 1);
        assert_eq!(s.snapshot().completed_focus, 1);
    }

    #[test]
    fn a_new_day_clears_the_count_in_both_places() {
        let mut s = session();
        s.toggle(0);
        s.advance(25 * 60 * 1000);
        assert!(s.roll_over_day("2026-03-05"));
        assert_eq!(s.settings().completed_focus, 0);
        assert_eq!(s.snapshot().completed_focus, 0);
        assert!(!s.roll_over_day("2026-03-05"));
    }

    #[test]
    fn starting_on_a_later_day_does_not_carry_yesterdays_count() {
        let settings = Settings {
            completed_focus: 7,
            completed_date: "2026-03-03".to_string(),
            ..Settings::default()
        };
        let s = Session::new(settings, TODAY);
        assert_eq!(s.snapshot().completed_focus, 0);
    }

    #[test]
    fn noise_plays_only_while_focus_is_running_unless_asked() {
        let mut s = session();
        s.apply_patch(&json!({ "noiseEnabled": true }));
        assert!(!s.noise_wanted(), "not while idle");

        s.toggle(0);
        assert!(s.noise_wanted(), "during running focus");

        s.toggle(1);
        assert!(!s.noise_wanted(), "not while paused");

        s.toggle(1);
        s.advance(25 * 60 * 1000 + 1); // into the auto-started break
        assert!(!s.noise_wanted(), "not during a break by default");

        s.apply_patch(&json!({ "noiseDuringBreaks": true }));
        assert!(s.noise_wanted(), "during a break when asked");
    }

    #[test]
    fn the_fingerprint_moves_only_when_the_display_would() {
        let mut s = session();
        s.toggle(0);
        let first = s.fingerprint();
        s.advance(400);
        assert_eq!(s.fingerprint(), first, "under a second: same clock face");
        s.advance(1_000);
        assert_ne!(s.fingerprint(), first);
    }
}
