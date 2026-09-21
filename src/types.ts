// Mirrors the serde representation of the Rust types. Keep the two in step:
// everything crosses the bridge in camelCase.

export type Phase = "focus" | "shortBreak" | "longBreak";
export type RunState = "idle" | "running" | "paused";
export type NoiseKind = "white" | "pink" | "brown";
export type ChimeVoice = "bell" | "bowl" | "wood";
export type Theme = "system" | "light" | "dark";

export interface Preset {
  id: string;
  label: string;
  focusSecs: number;
  shortBreakSecs: number;
  longBreakSecs: number;
  rounds: number;
}

export interface Behavior {
  autoStartBreaks: boolean;
  autoStartFocus: boolean;
}

export interface Snapshot {
  phase: Phase;
  phaseLabel: string;
  state: RunState;
  remainingSecs: number;
  totalSecs: number;
  progress: number;
  round: number;
  rounds: number;
  completedFocus: number;
  preset: Preset;
  clock: string;
}

export interface Settings {
  presetId: string;
  customPreset: Preset;
  behavior: Behavior;

  chimeEnabled: boolean;
  chimeVoice: ChimeVoice;
  chimeVolume: number;

  noiseEnabled: boolean;
  noiseKind: NoiseKind;
  noiseVolume: number;
  noiseTone: number;
  noiseDuringBreaks: boolean;

  notificationsEnabled: boolean;
  showClockInMenuBar: boolean;
  launchAtLogin: boolean;
  theme: Theme;

  completedFocus: number;
  completedDate: string;
}

export interface AppState {
  timer: Snapshot;
  settings: Settings;
  presets: Preset[];
}

export const NOISE_KINDS: { id: NoiseKind; label: string; hint: string }[] = [
  { id: "brown", label: "Brown", hint: "Deep and rumbling, like distant surf" },
  { id: "pink", label: "Pink", hint: "Soft and even, close to rain on a window" },
  { id: "white", label: "White", hint: "Bright and full, the classic masking hiss" },
];

export const CHIME_VOICES: { id: ChimeVoice; label: string }[] = [
  { id: "bell", label: "Bell" },
  { id: "bowl", label: "Bowl" },
  { id: "wood", label: "Wood" },
];
