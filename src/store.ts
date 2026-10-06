import { useCallback, useEffect, useRef, useState } from "react";

import { call, onState } from "./bridge";
import { applyPatch, type SettingsPatch } from "./settings";
import type { AppState } from "./types";

/// The app's state, and the two ways the interface changes it.
///
/// Settings changes apply on screen immediately and are sent as partial
/// patches naming only what changed. While a patch is in flight, every state
/// that arrives from the engine — a command's reply, a tick's push — has the
/// pending patches laid back over it. Without that, the reply to a slider's
/// first step lands after its fifth and drags the thumb backwards, and a
/// stepper clicked three times quickly reads the same stale value three times.
export function useLockIn() {
  const [state, setState] = useState<AppState | null>(null);
  const pending = useRef(new Map<number, SettingsPatch>());
  const nextPatchId = useRef(0);

  const reconcile = useCallback((server: AppState): AppState => {
    let settings = server.settings;
    for (const patch of pending.current.values()) settings = applyPatch(settings, patch);
    return { ...server, settings };
  }, []);

  const receive = useCallback((server: AppState) => setState(reconcile(server)), [reconcile]);

  useEffect(() => {
    let live = true;
    let unlisten: (() => void) | undefined;

    void call<AppState>("get_state").then((next) => live && receive(next));
    void onState((next) => live && receive(next)).then((off) => {
      if (live) unlisten = off;
      else off();
    });

    return () => {
      live = false;
      unlisten?.();
    };
  }, [receive]);

  /// Runs a command and shows the state it reports back.
  const command = useCallback(
    (name: string, args?: Record<string, unknown>) => {
      void call<AppState>(name, args).then(receive, () => resync(receive));
    },
    [receive],
  );

  /// Changes settings. Only the keys in `patch` are sent.
  const patchSettings = useCallback(
    (patch: SettingsPatch) => {
      const id = nextPatchId.current++;
      pending.current.set(id, patch);
      // A pure updater: React may run it twice, so it must not send anything.
      setState((current) => current && { ...current, settings: applyPatch(current.settings, patch) });

      void call<AppState>("update_settings", { patch }).then(
        (next) => {
          // Retire this patch before reconciling, so the engine's word on it
          // stands — it may have clamped a value or refused a change.
          pending.current.delete(id);
          receive(next);
        },
        () => {
          pending.current.delete(id);
          resync(receive);
        },
      );
    },
    [receive],
  );

  return { state, command, patchSettings };
}

/// After a failed call, ask for the truth rather than keep showing a guess.
function resync(receive: (state: AppState) => void) {
  void call<AppState>("get_state").then(receive, () => {});
}
