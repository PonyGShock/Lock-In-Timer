// A stand-in backend for running the UI in a plain browser.
//
// `npm run dev` opened outside Tauri would otherwise show a dead shell, which
// makes the interface impossible to work on without a full app build. It
// mirrors the rules in `lockin_core::Session` closely enough to design
// against. bridge.ts loads it lazily and only outside Tauri, so none of this —
// including its 5 Hz interval — ever runs inside the real app.
//
// Query parameters seed a starting state, which is how the screenshots in the
// README are produced:
//   ?state=running&remaining=1122&round=2&done=3&noise=1&theme=dark&platform=windows
//
// `&latency=150` delays every reply by that many milliseconds while still
// applying the command at once, the way real IPC behaves. Replies then arrive
// after newer changes, which is how the optimistic-update logic gets tested.

import { applyPatch, type SettingsPatch } from "./settings";
import type { AppState, Phase, Preset, RunState, Settings, Snapshot } from "./types";

const REPOSITORY = "https://github.com/PonyGShock/Lock-In-Timer";

const PRESETS: Preset[] = [
  { id: "espresso", label: "Espresso", focusSecs: 900, shortBreakSecs: 180, longBreakSecs: 600, rounds: 4 },
  { id: "classic", label: "Classic", focusSecs: 1500, shortBreakSecs: 300, longBreakSecs: 900, rounds: 4 },
  { id: "deep", label: "Deep", focusSecs: 3000, shortBreakSecs: 600, longBreakSecs: 1200, rounds: 3 },
  { id: "flow", label: "Flow", focusSecs: 5400, shortBreakSecs: 1200, longBreakSecs: 1800, rounds: 2 },
];

const PHASE_LABEL: Record<Phase, string> = {
  focus: "Focus",
  shortBreak: "Short break",
  longBreak: "Long break",
};

let settings: Settings = {
  presetId: "classic",
  customPreset: { id: "custom", label: "Custom", focusSecs: 1800, shortBreakSecs: 360, longBreakSecs: 1200, rounds: 4 },
  behavior: { autoStartBreaks: true, autoStartFocus: false },
  chimeEnabled: true,
  chimeVoice: "bell",
  chimeVolume: 0.7,
  noiseEnabled: false,
  noiseVolume: 0.35,
  noiseDuringBreaks: false,
  notificationsEnabled: true,
  showClockInMenuBar: true,
  launchAtLogin: false,
  theme: "system",
  completedFocus: 0,
  completedDate: new Date().toISOString().slice(0, 10),
};

let phase: Phase = "focus";
let runState: RunState = "idle";
let round = 1;
let remainingMs = 0;
let platform = "macos";
let latency = 0;
const listeners = new Set<(state: AppState) => void>();

function activePreset(): Preset {
  if (settings.presetId === "custom") return settings.customPreset;
  return PRESETS.find((p) => p.id === settings.presetId) ?? PRESETS[1];
}

function phaseMs(which: Phase = phase): number {
  const preset = activePreset();
  const secs =
    which === "focus" ? preset.focusSecs : which === "shortBreak" ? preset.shortBreakSecs : preset.longBreakSecs;
  return secs * 1000;
}

function clock(secs: number): string {
  const m = Math.floor(secs / 60);
  const s = secs % 60;
  return `${String(m).padStart(2, "0")}:${String(s).padStart(2, "0")}`;
}

function snapshot(): Snapshot {
  const total = phaseMs() / 1000;
  const remaining = Math.ceil(remainingMs / 1000);
  return {
    phase,
    phaseLabel: PHASE_LABEL[phase],
    state: runState,
    remainingSecs: remaining,
    totalSecs: total,
    progress: total === 0 ? 0 : Math.min(1, Math.max(0, (total - remaining) / total)),
    round,
    rounds: activePreset().rounds,
    completedFocus: settings.completedFocus,
    preset: activePreset(),
    clock: clock(remaining),
  };
}

function state(): AppState {
  return { timer: snapshot(), settings, presets: [...PRESETS, settings.customPreset], platform };
}

function publish() {
  // Real IPC serializes; sharing live objects with the UI would hide bugs.
  const current = structuredClone(state());
  listeners.forEach((listener) => listener(current));
}

function transition(completed: boolean) {
  if (phase === "focus") {
    if (completed) settings = { ...settings, completedFocus: settings.completedFocus + 1 };
    phase = round >= activePreset().rounds ? "longBreak" : "shortBreak";
  } else {
    round = phase === "shortBreak" ? round + 1 : 1;
    phase = "focus";
  }
  remainingMs = phaseMs();
  const auto = phase === "focus" ? settings.behavior.autoStartFocus : settings.behavior.autoStartBreaks;
  runState = auto ? "running" : "idle";
}

function reset() {
  phase = "focus";
  runState = "idle";
  round = 1;
  remainingMs = phaseMs();
}

/// New lengths for the running preset: keep the session, as Timer::retune does.
function retune(previousTotalMs: number) {
  const untouched = runState === "idle" && remainingMs === previousTotalMs;
  round = Math.min(round, activePreset().rounds);
  remainingMs = untouched ? phaseMs() : Math.min(remainingMs, phaseMs());
}

reset();

// Like the real tick loop: commands are answered by their reply alone, and an
// event goes out only when the visible clock, run state, phase, round or tally
// moves. Broadcasting after every command here would hide exactly the stale
// state bugs the browser is meant to catch.
let lastFingerprint = "";

setInterval(() => {
  if (runState === "running") {
    if (remainingMs <= 200) transition(true);
    else remainingMs -= 200;
  }
  const fingerprint = [Math.ceil(remainingMs / 1000), runState, phase, round, settings.completedFocus].join();
  if (fingerprint === lastFingerprint) return;
  lastFingerprint = fingerprint;
  publish();
}, 200);

function applyQuerySeed() {
  const params = new URLSearchParams(window.location.search);
  if (![...params.keys()].length) return;

  const preset = params.get("preset");
  if (preset) settings = { ...settings, presetId: preset };

  const seedPhase = params.get("phase") as Phase | null;
  if (seedPhase && seedPhase in PHASE_LABEL) phase = seedPhase;

  remainingMs = phaseMs();

  const remaining = params.get("remaining");
  if (remaining) remainingMs = Number(remaining) * 1000;

  const seedState = params.get("state");
  if (seedState === "running" || seedState === "paused" || seedState === "idle") runState = seedState;

  const seedRound = params.get("round");
  if (seedRound) round = Number(seedRound);

  const done = params.get("done");
  if (done) settings = { ...settings, completedFocus: Number(done) };

  if (params.get("noise") === "1") settings = { ...settings, noiseEnabled: true };

  const theme = params.get("theme");
  if (theme === "light" || theme === "dark" || theme === "system") settings = { ...settings, theme };

  platform = params.get("platform") ?? platform;
  latency = Number(params.get("latency") ?? 0);
}

applyQuerySeed();

export async function mockInvoke<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  switch (command) {
    case "timer_toggle":
      runState = runState === "running" ? "paused" : "running";
      break;
    case "timer_reset":
      reset();
      break;
    case "timer_skip":
      transition(false);
      break;
    case "select_preset": {
      const id = String(args?.id ?? "");
      // Reselecting the active preset must not cost the running session.
      if (id !== settings.presetId) {
        settings = { ...settings, presetId: id };
        reset();
      }
      break;
    }
    case "update_settings": {
      const before = settings;
      const previousActive = JSON.stringify(activePreset());
      const previousTotalMs = phaseMs();
      settings = {
        ...applyPatch(settings, (args?.patch ?? {}) as SettingsPatch),
        completedFocus: before.completedFocus,
        completedDate: before.completedDate,
      };
      if (settings.presetId !== before.presetId) reset();
      else if (JSON.stringify(activePreset()) !== previousActive) retune(previousTotalMs);
      break;
    }
    // These return nothing from Rust, so they return nothing here. Answering
    // with state would let a caller that wrongly expects state pass in the
    // browser and then crash in the real app.
    case "open_repository":
      window.open(REPOSITORY, "_blank", "noopener,noreferrer");
      return undefined as T;
    case "preview_chime":
    case "hide_window":
    case "quit_app":
      return undefined as T;
    case "get_state":
      break;
    default:
      throw new Error(`mock: unknown command ${command}`);
  }
  // Capture the reply now and deliver it late: the state it describes is the
  // state just after this command, however much has happened since.
  const reply = structuredClone(state());
  if (latency > 0) await new Promise((resolve) => setTimeout(resolve, latency));
  return reply as T;
}

export function mockListen(listener: (state: AppState) => void): () => void {
  listeners.add(listener);
  return () => listeners.delete(listener);
}
