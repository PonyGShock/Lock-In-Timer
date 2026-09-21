//! Pomodoro phase state machine.
//!
//! The engine never reads the system clock itself: callers pass a monotonic
//! millisecond value into [`Timer::advance`]. That keeps every transition
//! deterministic and testable, and lets the host decide what "now" means.

use serde::{Deserialize, Serialize};

/// Shortest phase the engine will accept, in seconds.
const MIN_PHASE_SECS: u32 = 10;
/// Longest phase the engine will accept, in seconds (4 hours).
const MAX_PHASE_SECS: u32 = 4 * 60 * 60;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Phase {
    Focus,
    ShortBreak,
    LongBreak,
}

impl Phase {
    pub fn is_break(self) -> bool {
        matches!(self, Phase::ShortBreak | Phase::LongBreak)
    }

    pub fn label(self) -> &'static str {
        match self {
            Phase::Focus => "Focus",
            Phase::ShortBreak => "Short break",
            Phase::LongBreak => "Long break",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RunState {
    Idle,
    Running,
    Paused,
}

/// A named set of phase durations.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Preset {
    pub id: String,
    pub label: String,
    pub focus_secs: u32,
    pub short_break_secs: u32,
    pub long_break_secs: u32,
    /// Focus sessions completed before a long break is offered.
    pub rounds: u32,
}

impl Preset {
    pub fn new(
        id: &str,
        label: &str,
        focus_secs: u32,
        short_break_secs: u32,
        long_break_secs: u32,
        rounds: u32,
    ) -> Self {
        Preset {
            id: id.to_string(),
            label: label.to_string(),
            focus_secs,
            short_break_secs,
            long_break_secs,
            rounds,
        }
    }

    /// Clamps every field into a range the state machine can run safely.
    /// A zero-length phase would make phase advancement spin, so this is
    /// applied to anything crossing the boundary from settings or the UI.
    pub fn sanitized(mut self) -> Self {
        self.focus_secs = self.focus_secs.clamp(MIN_PHASE_SECS, MAX_PHASE_SECS);
        self.short_break_secs = self.short_break_secs.clamp(MIN_PHASE_SECS, MAX_PHASE_SECS);
        self.long_break_secs = self.long_break_secs.clamp(MIN_PHASE_SECS, MAX_PHASE_SECS);
        self.rounds = self.rounds.clamp(1, 12);
        self
    }

    pub fn duration_secs(&self, phase: Phase) -> u32 {
        match phase {
            Phase::Focus => self.focus_secs,
            Phase::ShortBreak => self.short_break_secs,
            Phase::LongBreak => self.long_break_secs,
        }
    }
}

/// The built-in timer versions, longest-standing first.
pub fn builtin_presets() -> Vec<Preset> {
    vec![
        // Labels stay short so the whole row fits the popover without
        // scrolling, which is the only way the Custom chip stays findable.
        Preset::new("espresso", "Espresso", 15 * 60, 3 * 60, 10 * 60, 4),
        Preset::new("classic", "Classic", 25 * 60, 5 * 60, 15 * 60, 4),
        Preset::new("deep", "Deep", 50 * 60, 10 * 60, 20 * 60, 3),
        Preset::new("flow", "Flow", 90 * 60, 20 * 60, 30 * 60, 2),
    ]
}

pub fn default_preset() -> Preset {
    builtin_presets()
        .into_iter()
        .find(|p| p.id == "classic")
        .expect("classic is a built-in preset")
}

pub fn preset_by_id(id: &str) -> Option<Preset> {
    builtin_presets().into_iter().find(|p| p.id == id)
}

/// What the timer does on its own when a phase ends.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Behavior {
    pub auto_start_breaks: bool,
    pub auto_start_focus: bool,
}

impl Default for Behavior {
    fn default() -> Self {
        Behavior {
            auto_start_breaks: true,
            auto_start_focus: false,
        }
    }
}

/// Emitted when a phase boundary is crossed, so the host can chime and notify.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Transition {
    pub ended: Phase,
    pub next: Phase,
    pub auto_started: bool,
    /// True when the phase ran to zero rather than being skipped by hand.
    pub completed: bool,
}

/// Everything the UI and the tray need to render, in one flat struct.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub phase: Phase,
    pub phase_label: String,
    pub state: RunState,
    pub remaining_secs: u32,
    pub total_secs: u32,
    /// Fraction of the current phase already spent, 0.0 to 1.0.
    pub progress: f32,
    /// 1-based position of the current focus session inside the cycle.
    pub round: u32,
    pub rounds: u32,
    pub completed_focus: u32,
    pub preset: Preset,
    pub clock: String,
}

pub struct Timer {
    preset: Preset,
    behavior: Behavior,
    phase: Phase,
    state: RunState,
    remaining_ms: u64,
    round: u32,
    completed_focus: u32,
    clock_ms: u64,
}

impl Timer {
    pub fn new(preset: Preset, behavior: Behavior) -> Self {
        let preset = preset.sanitized();
        let remaining_ms = u64::from(preset.focus_secs) * 1000;
        Timer {
            preset,
            behavior,
            phase: Phase::Focus,
            state: RunState::Idle,
            remaining_ms,
            round: 1,
            completed_focus: 0,
            clock_ms: 0,
        }
    }

    pub fn behavior(&self) -> Behavior {
        self.behavior
    }

    pub fn set_behavior(&mut self, behavior: Behavior) {
        self.behavior = behavior;
    }

    pub fn preset(&self) -> &Preset {
        &self.preset
    }

    pub fn phase(&self) -> Phase {
        self.phase
    }

    pub fn state(&self) -> RunState {
        self.state
    }

    pub fn is_running(&self) -> bool {
        self.state == RunState::Running
    }

    pub fn completed_focus(&self) -> u32 {
        self.completed_focus
    }

    pub fn set_completed_focus(&mut self, count: u32) {
        self.completed_focus = count;
    }

    /// Swaps the durations and returns to an idle focus phase.
    pub fn set_preset(&mut self, preset: Preset) {
        self.preset = preset.sanitized();
        self.reset();
    }

    pub fn start(&mut self) {
        if self.state == RunState::Running {
            return;
        }
        if self.remaining_ms == 0 {
            self.remaining_ms = self.phase_total_ms();
        }
        self.state = RunState::Running;
    }

    pub fn pause(&mut self) {
        if self.state == RunState::Running {
            self.state = RunState::Paused;
        }
    }

    pub fn toggle(&mut self) {
        match self.state {
            RunState::Running => self.pause(),
            _ => self.start(),
        }
    }

    /// Returns to the top of the cycle: focus, round one, not running.
    pub fn reset(&mut self) {
        self.phase = Phase::Focus;
        self.state = RunState::Idle;
        self.round = 1;
        self.remaining_ms = u64::from(self.preset.focus_secs) * 1000;
    }

    /// Restarts the current phase without touching the cycle position.
    pub fn restart_phase(&mut self) {
        self.remaining_ms = self.phase_total_ms();
        self.state = RunState::Idle;
    }

    /// Ends the current phase by hand. A skipped focus session is not counted
    /// as completed — only time actually spent earns a round.
    pub fn skip(&mut self) -> Transition {
        self.transition(false)
    }

    /// Feeds monotonic time into the machine.
    ///
    /// At most one phase boundary is crossed per call: after the machine has
    /// been suspended (a closed laptop, a sleeping host) the leftover time is
    /// dropped rather than replayed, so waking up produces one chime and a
    /// fresh next phase instead of a burst of them.
    pub fn advance(&mut self, now_ms: u64) -> Option<Transition> {
        let delta = now_ms.saturating_sub(self.clock_ms);
        self.clock_ms = now_ms;

        if self.state != RunState::Running {
            return None;
        }

        if delta < self.remaining_ms {
            self.remaining_ms -= delta;
            return None;
        }

        self.remaining_ms = 0;
        Some(self.transition(true))
    }

    /// Resets the internal clock reference without advancing anything. Call
    /// this before resuming so time spent paused is not counted as elapsed.
    pub fn sync_clock(&mut self, now_ms: u64) {
        self.clock_ms = now_ms;
    }

    pub fn snapshot(&self) -> Snapshot {
        let total_secs = self.phase_total_secs();
        let remaining_secs = self.remaining_secs();
        let elapsed = total_secs.saturating_sub(remaining_secs) as f32;
        let progress = if total_secs == 0 {
            0.0
        } else {
            (elapsed / total_secs as f32).clamp(0.0, 1.0)
        };

        Snapshot {
            phase: self.phase,
            phase_label: self.phase.label().to_string(),
            state: self.state,
            remaining_secs,
            total_secs,
            progress,
            round: self.round,
            rounds: self.preset.rounds,
            completed_focus: self.completed_focus,
            preset: self.preset.clone(),
            clock: format_clock(remaining_secs),
        }
    }

    pub fn remaining_secs(&self) -> u32 {
        // Ceiling, so a fresh 25 minute phase reads 25:00 rather than 24:59
        // and the display only reaches 00:00 when the phase is truly over.
        self.remaining_ms.div_ceil(1000) as u32
    }

    fn phase_total_secs(&self) -> u32 {
        self.preset.duration_secs(self.phase)
    }

    fn phase_total_ms(&self) -> u64 {
        u64::from(self.phase_total_secs()) * 1000
    }

    fn transition(&mut self, completed: bool) -> Transition {
        let ended = self.phase;

        let next = match ended {
            Phase::Focus => {
                if completed {
                    self.completed_focus += 1;
                }
                if self.round >= self.preset.rounds {
                    Phase::LongBreak
                } else {
                    Phase::ShortBreak
                }
            }
            Phase::ShortBreak => {
                self.round += 1;
                Phase::Focus
            }
            Phase::LongBreak => {
                self.round = 1;
                Phase::Focus
            }
        };

        self.phase = next;
        self.remaining_ms = self.phase_total_ms();

        let auto_started = if next.is_break() {
            self.behavior.auto_start_breaks
        } else {
            self.behavior.auto_start_focus
        };
        self.state = if auto_started {
            RunState::Running
        } else {
            RunState::Idle
        };

        Transition {
            ended,
            next,
            auto_started,
            completed,
        }
    }
}

/// Formats seconds as the menu bar shows them: `25:00`, `90:00`, `04:07`.
pub fn format_clock(secs: u32) -> String {
    format!("{:02}:{:02}", secs / 60, secs % 60)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_preset() -> Preset {
        // Minimum-length phases keep the tests fast but still legal.
        Preset::new("test", "Test", 60, 20, 40, 2)
    }

    fn timer() -> Timer {
        Timer::new(test_preset(), Behavior::default())
    }

    #[test]
    fn starts_idle_on_a_full_focus_phase() {
        let t = timer();
        let snap = t.snapshot();
        assert_eq!(snap.phase, Phase::Focus);
        assert_eq!(snap.state, RunState::Idle);
        assert_eq!(snap.remaining_secs, 60);
        assert_eq!(snap.clock, "01:00");
        assert_eq!(snap.progress, 0.0);
    }

    #[test]
    fn idle_timer_does_not_drain() {
        let mut t = timer();
        assert!(t.advance(30_000).is_none());
        assert_eq!(t.remaining_secs(), 60);
    }

    #[test]
    fn running_timer_drains_by_elapsed_time() {
        let mut t = timer();
        t.start();
        t.advance(10_000);
        assert_eq!(t.remaining_secs(), 50);
        t.advance(25_000);
        assert_eq!(t.remaining_secs(), 35);
    }

    #[test]
    fn paused_time_is_not_counted() {
        let mut t = timer();
        t.start();
        t.advance(10_000);
        t.pause();
        t.advance(90_000);
        assert_eq!(t.remaining_secs(), 50);
        assert_eq!(t.state(), RunState::Paused);

        t.sync_clock(90_000);
        t.start();
        t.advance(95_000);
        assert_eq!(t.remaining_secs(), 45);
    }

    #[test]
    fn focus_rolls_into_a_short_break_and_auto_starts() {
        let mut t = timer();
        t.start();
        let transition = t.advance(60_000).expect("phase should end");
        assert_eq!(transition.ended, Phase::Focus);
        assert_eq!(transition.next, Phase::ShortBreak);
        assert!(transition.auto_started);
        assert!(transition.completed);
        assert_eq!(t.state(), RunState::Running);
        assert_eq!(t.remaining_secs(), 20);
        assert_eq!(t.completed_focus(), 1);
    }

    #[test]
    fn break_rolls_into_focus_and_waits_for_the_user_by_default() {
        let mut t = timer();
        t.start();
        t.advance(60_000);
        let transition = t.advance(80_000).expect("break should end");
        assert_eq!(transition.ended, Phase::ShortBreak);
        assert_eq!(transition.next, Phase::Focus);
        assert!(!transition.auto_started);
        assert_eq!(t.state(), RunState::Idle);
    }

    #[test]
    fn last_round_of_the_cycle_earns_a_long_break() {
        let mut t = timer();
        t.start();
        t.advance(60_000); // focus 1 ends -> short break
        t.advance(80_000); // short break ends -> focus 2 (idle)
        assert_eq!(t.snapshot().round, 2);

        t.start();
        let transition = t.advance(140_000).expect("focus 2 should end");
        assert_eq!(transition.next, Phase::LongBreak);
        assert_eq!(t.remaining_secs(), 40);
    }

    #[test]
    fn long_break_returns_to_round_one() {
        let mut t = timer();
        t.start();
        t.advance(60_000);
        t.advance(80_000);
        t.start();
        t.advance(140_000); // -> long break, auto started
        let transition = t.advance(180_000).expect("long break should end");
        assert_eq!(transition.next, Phase::Focus);
        assert_eq!(t.snapshot().round, 1);
        assert_eq!(t.completed_focus(), 2);
    }

    #[test]
    fn a_long_suspension_crosses_exactly_one_boundary() {
        let mut t = timer();
        t.start();
        // Eight hours of sleep would otherwise replay dozens of phases.
        let transition = t.advance(8 * 60 * 60 * 1000).expect("phase should end");
        assert_eq!(transition.next, Phase::ShortBreak);
        assert_eq!(t.remaining_secs(), 20, "next phase starts at full length");
        assert_eq!(t.completed_focus(), 1, "only one session is credited");
    }

    #[test]
    fn skipping_focus_does_not_credit_a_session() {
        let mut t = timer();
        t.start();
        t.advance(5_000);
        let transition = t.skip();
        assert!(!transition.completed);
        assert_eq!(transition.next, Phase::ShortBreak);
        assert_eq!(t.completed_focus(), 0);
    }

    #[test]
    fn reset_returns_to_the_top_of_the_cycle() {
        let mut t = timer();
        t.start();
        t.advance(60_000);
        t.advance(80_000);
        t.reset();
        let snap = t.snapshot();
        assert_eq!(snap.phase, Phase::Focus);
        assert_eq!(snap.state, RunState::Idle);
        assert_eq!(snap.round, 1);
        assert_eq!(snap.remaining_secs, 60);
        assert_eq!(t.completed_focus(), 1, "history survives a reset");
    }

    #[test]
    fn toggle_flips_between_running_and_paused() {
        let mut t = timer();
        t.toggle();
        assert_eq!(t.state(), RunState::Running);
        t.toggle();
        assert_eq!(t.state(), RunState::Paused);
        t.toggle();
        assert_eq!(t.state(), RunState::Running);
    }

    #[test]
    fn changing_preset_resets_to_its_focus_length() {
        let mut t = timer();
        t.start();
        t.advance(30_000);
        t.set_preset(preset_by_id("deep").unwrap());
        let snap = t.snapshot();
        assert_eq!(snap.remaining_secs, 50 * 60);
        assert_eq!(snap.state, RunState::Idle);
        assert_eq!(snap.preset.id, "deep");
    }

    #[test]
    fn progress_tracks_elapsed_fraction() {
        let mut t = timer();
        t.start();
        t.advance(15_000);
        assert!((t.snapshot().progress - 0.25).abs() < 0.001);
    }

    #[test]
    fn degenerate_presets_are_clamped_rather_than_trusted() {
        let wild = Preset::new("wild", "Wild", 0, 0, 0, 0).sanitized();
        assert_eq!(wild.focus_secs, MIN_PHASE_SECS);
        assert_eq!(wild.rounds, 1);

        let huge = Preset::new("huge", "Huge", u32::MAX, u32::MAX, u32::MAX, 999).sanitized();
        assert_eq!(huge.focus_secs, MAX_PHASE_SECS);
        assert_eq!(huge.rounds, 12);
    }

    #[test]
    fn clock_formatting_pads_both_fields() {
        assert_eq!(format_clock(0), "00:00");
        assert_eq!(format_clock(7), "00:07");
        assert_eq!(format_clock(60), "01:00");
        assert_eq!(format_clock(25 * 60), "25:00");
        assert_eq!(format_clock(90 * 60), "90:00");
    }

    #[test]
    fn builtin_presets_survive_sanitising_unchanged() {
        for preset in builtin_presets() {
            assert_eq!(preset.clone(), preset.sanitized());
        }
    }
}
