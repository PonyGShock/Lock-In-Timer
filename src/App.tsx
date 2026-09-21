import { useCallback, useEffect, useState } from "react";

import { call, onState } from "./bridge";
import { Ring } from "./components/Ring";
import { SettingsSheet } from "./components/SettingsSheet";
import { GearIcon, Segmented, Slider, Switch, WaveIcon } from "./components/ui";
import { NOISE_KINDS, type AppState, type NoiseKind, type Settings } from "./types";

export default function App() {
  const [state, setState] = useState<AppState | null>(null);
  const [settingsOpen, setSettingsOpen] = useState(false);

  useEffect(() => {
    let live = true;
    let unlisten: (() => void) | undefined;

    void call<AppState>("get_state").then((next) => live && setState(next));
    void onState((next) => live && setState(next)).then((off) => {
      if (live) unlisten = off;
      else off();
    });

    return () => {
      live = false;
      unlisten?.();
    };
  }, []);

  const theme = state?.settings.theme ?? "system";
  useEffect(() => {
    const root = document.documentElement;
    if (theme === "system") root.removeAttribute("data-theme");
    else root.dataset.theme = theme;
  }, [theme]);

  const run = useCallback((command: string, args?: Record<string, unknown>) => {
    void call<AppState>(command, args).then(setState);
  }, []);

  const patch = useCallback(
    (changes: Partial<Settings>) => {
      setState((current) => {
        if (!current) return current;
        const settings = { ...current.settings, ...changes };
        void call<AppState>("update_settings", { settings }).then(setState);
        // Apply locally straight away so sliders and switches never lag
        // behind the finger that moved them.
        return { ...current, settings };
      });
    },
    [],
  );

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        if (settingsOpen) setSettingsOpen(false);
        else void call("hide_window");
      }
      if (event.code === "Space" && !settingsOpen) {
        event.preventDefault();
        run("timer_toggle");
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [settingsOpen, run]);

  if (!state) return <div className="card" data-phase="focus" />;

  const { timer, settings, presets } = state;

  const primaryLabel =
    timer.state === "running"
      ? "Pause"
      : timer.state === "paused"
        ? "Resume"
        : `Start ${timer.phaseLabel.toLowerCase()}`;

  // A round counts as done once its focus session is behind us, which during
  // a break means the round number itself.
  const roundsDone = timer.phase === "focus" ? timer.round - 1 : timer.round;
  const untouched =
    timer.state === "idle" && timer.round === 1 && timer.progress === 0 && timer.phase === "focus";

  const noiseKind = NOISE_KINDS.find((kind) => kind.id === settings.noiseKind);

  return (
    <div className="card" data-phase={timer.phase}>
      <header className="header">
        <span className="wordmark">Crema</span>
        <button
          className="icon-button"
          onClick={() => setSettingsOpen(true)}
          aria-label="Settings"
        >
          <GearIcon />
        </button>
      </header>

      <div className="presets">
        {presets.map((preset) => (
          <button
            key={preset.id}
            className="chip"
            aria-pressed={settings.presetId === preset.id}
            onClick={() => run("set_preset", { id: preset.id })}
          >
            {preset.label}
          </button>
        ))}
      </div>

      <div className="dial">
        <Ring timer={timer} />
        <div className="dots" aria-label={`Round ${timer.round} of ${timer.rounds}`}>
          {Array.from({ length: timer.rounds }, (_, index) => (
            <span
              key={index}
              className="dot"
              data-done={index < roundsDone}
              data-current={timer.phase === "focus" && index === roundsDone}
            />
          ))}
        </div>
      </div>

      <div className="transport">
        <button className="primary" onClick={() => run("timer_toggle")}>
          {primaryLabel}
        </button>
        <div className="secondary-row">
          <button className="secondary" onClick={() => run("timer_skip")}>
            Skip
          </button>
          <button
            className="secondary"
            disabled={untouched}
            onClick={() => run("timer_reset")}
          >
            Reset
          </button>
        </div>
      </div>

      <div className="shelf">
        <div className="shelf__row">
          <WaveIcon />
          <div className="shelf__label">
            <span className="shelf__title">Ambient noise</span>
            <span className="shelf__hint">
              {settings.noiseEnabled ? noiseKind?.hint : "Off"}
            </span>
          </div>
          <Switch
            label="Ambient noise"
            checked={settings.noiseEnabled}
            onChange={(noiseEnabled) => patch({ noiseEnabled })}
          />
        </div>
        {settings.noiseEnabled && (
          <>
            <div className="shelf__kinds">
              <Segmented<NoiseKind>
                value={settings.noiseKind}
                options={NOISE_KINDS}
                onChange={(noiseKind) => patch({ noiseKind })}
              />
            </div>
            <div className="shelf__row" style={{ height: 30 }}>
              <Slider
                label="Noise volume"
                value={settings.noiseVolume}
                onChange={(noiseVolume) => patch({ noiseVolume })}
              />
            </div>
          </>
        )}
      </div>

      <p className="footer">
        {timer.completedFocus === 0
          ? "No sessions finished today"
          : `${timer.completedFocus} session${timer.completedFocus === 1 ? "" : "s"} today`}
      </p>

      <SettingsSheet
        open={settingsOpen}
        state={state}
        onPatch={patch}
        onCustomPreset={(preset) => run("set_custom_preset", preset)}
        onPreviewChime={() => void call("preview_chime")}
        onClose={() => setSettingsOpen(false)}
        onQuit={() => void call("quit_app")}
      />
    </div>
  );
}
