import type { Settings } from "./types";

/// A partial settings update. Nested objects may be partial too: a patch can
/// name one field of the custom preset without restating the others.
export type SettingsPatch = {
  [K in keyof Settings]?: Settings[K] extends object ? Partial<Settings[K]> : Settings[K];
};

function isPlainObject(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

/// Lays a patch over settings. Nested objects merge rather than replace —
/// the same rule as `Settings::merged` on the Rust side, so what the interface
/// shows optimistically is what the engine will end up storing.
export function applyPatch(settings: Settings, patch: SettingsPatch): Settings {
  const next: Record<string, unknown> = { ...settings };
  for (const [key, value] of Object.entries(patch)) {
    const current = next[key];
    next[key] = isPlainObject(current) && isPlainObject(value) ? { ...current, ...value } : value;
  }
  return next as unknown as Settings;
}
