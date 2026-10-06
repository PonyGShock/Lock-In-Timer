// The one place that knows whether a real Tauri backend is behind the UI.

import type { AppState } from "./types";

export const inTauri = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

const STATE_EVENT = "lockin://state";

// Loaded on demand and only outside Tauri. A static import put the mock in the
// shipped bundle, where its timer woke the webview five times a second forever.
const mock = () => import("./mock");

export async function call<T = AppState>(
  command: string,
  args?: Record<string, unknown>,
): Promise<T> {
  if (!inTauri) return (await mock()).mockInvoke<T>(command, args);
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<T>(command, args);
}

/// Subscribes to engine pushes. Returns an unsubscribe function.
export async function onState(listener: (state: AppState) => void): Promise<() => void> {
  if (!inTauri) return (await mock()).mockListen(listener);
  const { listen } = await import("@tauri-apps/api/event");
  return listen<AppState>(STATE_EVENT, (event) => listener(event.payload));
}
