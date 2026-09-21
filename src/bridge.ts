// The one place that knows whether a real Tauri backend is behind the UI.

import { mockInvoke, mockListen } from "./mock";
import type { AppState } from "./types";

export const inTauri = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

const STATE_EVENT = "lockin://state";

export async function call<T = AppState>(
  command: string,
  args?: Record<string, unknown>,
): Promise<T> {
  if (!inTauri) return mockInvoke<T>(command, args);
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<T>(command, args);
}

/// Subscribes to engine pushes. Returns an unsubscribe function.
export async function onState(listener: (state: AppState) => void): Promise<() => void> {
  if (!inTauri) return mockListen(listener);
  const { listen } = await import("@tauri-apps/api/event");
  return listen<AppState>(STATE_EVENT, (event) => listener(event.payload));
}

export async function openExternal(url: string): Promise<void> {
  if (!inTauri) {
    window.open(url, "_blank", "noopener,noreferrer");
    return;
  }
  const { openUrl } = await import("@tauri-apps/plugin-opener");
  await openUrl(url);
}
