// A stand-in backend for running the UI in a plain browser.
//
// `npm run dev` opened outside Tauri would otherwise show a dead shell, which
// makes the interface impossible to work on without a full app build. This
// mirrors the Rust engine closely enough to design against and is never
// bundled into a decision the real app makes: `inTauri` picks the real bridge
// whenever one is there.
//
// Query parameters seed a starting state, which is how the screenshots in the
// README are produced:
//   ?state=running&remaining=1122&round=2&done=3&noise=1&theme=dark

import type { AppState, Phase, Preset, RunState, Settings, Snapshot } from "./types";

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
  noiseKind: "brown",
  noiseVolume: 0.35,
  noiseTone: 0.55,
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
const listeners = new Set<(state: AppState) => void>();

function activePreset(): Preset {
  if (settings.presetId === "custom") return settings.customPreset;
  return PRESETS.find((p) => p.id === settings.presetId) ?? PRESETS[1];
}

function phaseSecs(which: Phase = phase): number {
  const preset = activePreset();
  if (which === "focus") return preset.focusSecs;
  if (which === "shortBreak") return preset.shortBreakSecs;
  return preset.longBreakSecs;
}

function clock(secs: number): string {
  const m = Math.floor(secs / 60);
  const s = secs % 60;
  return `${String(m).padStart(2, "0")}:${String(s).padStart(2, "0")}`;
}

function snapshot(): Snapshot {
  const total = phaseSecs();
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
  return { timer: snapshot(), settings, presets: [...PRESETS, settings.customPreset] };
}

function publish() {
  const current = state();
  listeners.forEach((listener) => listener(current));
}

function transition(completed: boolean) {
  if (phase === "focus") {
    if (completed) settings.completedFocus += 1;
    phase = round >= activePreset().rounds ? "longBreak" : "shortBreak";
  } else {
    round = phase === "shortBreak" ? round + 1 : 1;
    phase = "focus";
  }
  remainingMs = phaseSecs() * 1000;
  const auto = phase === "focus" ? settings.behavior.autoStartFocus : settings.behavior.autoStartBreaks;
  runState = auto ? "running" : "idle";
}

function reset() {
  phase = "focus";
  runState = "idle";
  round = 1;
  remainingMs = phaseSecs() * 1000;
}

reset();

setInterval(() => {
  if (runState !== "running") return;
  if (remainingMs <= 200) {
    transition(true);
  } else {
    remainingMs -= 200;
  }
  publish();
}, 200);

function applyQuerySeed() {
  const params = new URLSearchParams(window.location.search);
  if (![...params.keys()].length) return;

  const preset = params.get("preset");
  if (preset) settings.presetId = preset;

  const seedPhase = params.get("phase") as Phase | null;
  if (seedPhase && seedPhase in PHASE_LABEL) phase = seedPhase;

  remainingMs = phaseSecs() * 1000;

  const remaining = params.get("remaining");
  if (remaining) remainingMs = Number(remaining) * 1000;

  const seedState = params.get("state");
  if (seedState === "running" || seedState === "paused" || seedState === "idle") runState = seedState;

  const seedRound = params.get("round");
  if (seedRound) round = Number(seedRound);

  const done = params.get("done");
  if (done) settings.completedFocus = Number(done);

  if (params.get("noise") === "1") settings.noiseEnabled = true;

  const theme = params.get("theme");
  if (theme === "light" || theme === "dark" || theme === "system") settings.theme = theme;
}

applyQuerySeed();

export async function mockInvoke<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  switch (command) {
    case "timer_toggle":
      runState = runState === "running" ? "paused" : "running";
      break;
    case "timer_start":
      runState = "running";
      break;
    case "timer_pause":
      if (runState === "running") runState = "paused";
      break;
    case "timer_reset":
      reset();
      break;
    case "timer_skip":
      transition(false);
      break;
    case "timer_restart_phase":
      remainingMs = phaseSecs() * 1000;
      runState = "idle";
      break;
    case "set_preset":
      settings.presetId = String(args?.id ?? "classic");
      reset();
      break;
    case "set_custom_preset":
      settings.customPreset = {
        id: "custom",
        label: "Custom",
        focusSecs: Number(args?.focusSecs ?? 1800),
        shortBreakSecs: Number(args?.shortBreakSecs ?? 360),
        longBreakSecs: Number(args?.longBreakSecs ?? 1200),
        rounds: Number(args?.rounds ?? 4),
      };
      settings.presetId = "custom";
      reset();
      break;
    case "update_settings": {
      const incoming = args?.settings as Settings;
      const presetChanged =
        incoming.presetId !== settings.presetId ||
        JSON.stringify(incoming.customPreset) !== JSON.stringify(settings.customPreset);
      settings = { ...incoming, completedFocus: settings.completedFocus };
      if (presetChanged) reset();
      break;
    }
    case "preview_chime":
    case "hide_window":
    case "quit_app":
      break;
  }
  publish();
  return state() as T;
}

export function mockListen(listener: (state: AppState) => void): () => void {
  listeners.add(listener);
  return () => listeners.delete(listener);
}
