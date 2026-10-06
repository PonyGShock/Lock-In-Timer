import { useEffect, useRef, useState } from "react";

import { call } from "./bridge";
import { Ring } from "./components/Ring";
import { SettingsSheet } from "./components/SettingsSheet";
import { CupMark, GearIcon, Slider, Switch, WaveIcon } from "./components/ui";
import { useLockIn } from "./store";

export default function App() {
  const { state, command, patchSettings } = useLockIn();
  const [settingsOpen, setSettingsOpen] = useState(false);
  const faceRef = useRef<HTMLDivElement>(null);
  const gearRef = useRef<HTMLButtonElement>(null);

  const theme = state?.settings.theme ?? "system";
  useEffect(() => {
    const root = document.documentElement;
    if (theme === "system") root.removeAttribute("data-theme");
    else root.dataset.theme = theme;
  }, [theme]);

  // The sheet covers the timer but does not remove it, so without this Tab
  // walks into buttons nobody can see and Enter presses them.
  useEffect(() => {
    faceRef.current?.toggleAttribute("inert", settingsOpen);
  }, [settingsOpen]);

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        if (settingsOpen) closeSettings();
        else void call("hide_window");
        return;
      }
      if (event.code !== "Space" || settingsOpen) return;
      // Holding the key would otherwise toggle twenty times a second.
      if (event.repeat) return;
      // A focused button or field answers Space itself. Handling it here as
      // well toggled twice — once from this handler, once from the button.
      // The target is the window itself when nothing has focus.
      const target = event.target;
      if (target instanceof Element && target.closest("button, input, [role='switch']")) return;
      event.preventDefault();
      command("timer_toggle");
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  });

  function closeSettings() {
    setSettingsOpen(false);
    gearRef.current?.focus();
  }

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

  return (
    <div className="card" data-phase={timer.phase}>
      <div className="face" ref={faceRef}>
        <header className="header">
          <span className="brand">
            <CupMark />
            Lock In
          </span>
          <button
            ref={gearRef}
            className="icon-button"
            onClick={() => setSettingsOpen(true)}
            aria-label="Settings"
          >
            <GearIcon />
          </button>
        </header>

        <div className="presets" role="group" aria-label="Timer length">
          {presets.map((preset) => (
            <button
              key={preset.id}
              className="chip"
              aria-pressed={settings.presetId === preset.id}
              onClick={() => command("select_preset", { id: preset.id })}
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
          <button className="primary" onClick={() => command("timer_toggle")}>
            {primaryLabel}
          </button>
          <div className="secondary-row">
            <button className="secondary" onClick={() => command("timer_skip")}>
              Skip
            </button>
            <button
              className="secondary"
              disabled={untouched}
              onClick={() => command("timer_reset")}
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
                {settings.noiseEnabled ? "Low and rumbling, like distant surf" : "Off"}
              </span>
            </div>
            <Switch
              label="Ambient noise"
              checked={settings.noiseEnabled}
              onChange={(noiseEnabled) => patchSettings({ noiseEnabled })}
            />
          </div>
          {settings.noiseEnabled && (
            <div className="shelf__row" style={{ height: 30 }}>
              <Slider
                label="Noise volume"
                value={settings.noiseVolume}
                onChange={(noiseVolume) => patchSettings({ noiseVolume })}
              />
            </div>
          )}
        </div>

        <p className="footer">
          {timer.completedFocus === 0
            ? "No sessions finished today"
            : `${timer.completedFocus} session${timer.completedFocus === 1 ? "" : "s"} today`}
        </p>
      </div>

      <SettingsSheet
        open={settingsOpen}
        state={state}
        onPatch={patchSettings}
        onClose={closeSettings}
      />
    </div>
  );
}
