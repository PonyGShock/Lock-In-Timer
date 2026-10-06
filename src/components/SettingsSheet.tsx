import { useEffect, useRef } from "react";

import { call } from "../bridge";
import type { SettingsPatch } from "../settings";
import { CHIME_VOICES, type AppState, type Theme } from "../types";
import { BackIcon, Group, Row, Segmented, Slider, Stepper, Switch } from "./ui";

const THEMES: { id: Theme; label: string }[] = [
  { id: "system", label: "System" },
  { id: "light", label: "Light" },
  { id: "dark", label: "Dark" },
];

const DESKTOP = new Set(["macos", "windows", "linux"]);

const minutes = (secs: number) => `${Math.round(secs / 60)} min`;

export function SettingsSheet({
  open,
  state,
  onPatch,
  onClose,
}: {
  open: boolean;
  state: AppState;
  onPatch: (patch: SettingsPatch) => void;
  onClose: () => void;
}) {
  const { settings, platform } = state;
  const custom = settings.customPreset;
  const sheetRef = useRef<HTMLDivElement>(null);
  const backRef = useRef<HTMLButtonElement>(null);

  // Closed, the sheet is off-screen; inert keeps it out of the tab order and
  // away from assistive tech. Open, focus moves into it so the keyboard is
  // where the eyes are.
  useEffect(() => {
    sheetRef.current?.toggleAttribute("inert", !open);
    if (open) backRef.current?.focus();
  }, [open]);

  return (
    <div className="sheet" data-open={open} ref={sheetRef} role="dialog" aria-label="Settings">
      <header className="header">
        <button ref={backRef} className="icon-button" onClick={onClose} aria-label="Back to timer">
          <BackIcon />
        </button>
        <span className="brand">Settings</span>
        <span style={{ width: 30 }} />
      </header>

      <div className="sheet__body">
        <Group title="Sessions">
          <Row title="Start breaks automatically" hint="A break begins the moment focus ends.">
            <Switch
              label="Start breaks automatically"
              checked={settings.behavior.autoStartBreaks}
              onChange={(autoStartBreaks) => onPatch({ behavior: { autoStartBreaks } })}
            />
          </Row>
          <Row
            title="Start focus automatically"
            hint="Off by default, so a break ends when you say so."
          >
            <Switch
              label="Start focus automatically"
              checked={settings.behavior.autoStartFocus}
              onChange={(autoStartFocus) => onPatch({ behavior: { autoStartFocus } })}
            />
          </Row>
        </Group>

        <Group
          title="Custom lengths"
          note="Used when Custom is selected. Changing them never interrupts a session in progress."
        >
          <Row title="Focus">
            <Stepper
              label="focus length"
              value={custom.focusSecs}
              min={5 * 60}
              max={180 * 60}
              step={5 * 60}
              format={minutes}
              onChange={(focusSecs) => onPatch({ customPreset: { focusSecs } })}
            />
          </Row>
          <Row title="Short break">
            <Stepper
              label="short break length"
              value={custom.shortBreakSecs}
              min={60}
              max={60 * 60}
              step={60}
              format={minutes}
              onChange={(shortBreakSecs) => onPatch({ customPreset: { shortBreakSecs } })}
            />
          </Row>
          <Row title="Long break">
            <Stepper
              label="long break length"
              value={custom.longBreakSecs}
              min={5 * 60}
              max={120 * 60}
              step={5 * 60}
              format={minutes}
              onChange={(longBreakSecs) => onPatch({ customPreset: { longBreakSecs } })}
            />
          </Row>
          <Row title="Rounds" hint="Focus sessions before the long break.">
            <Stepper
              label="rounds"
              value={custom.rounds}
              min={1}
              max={12}
              format={(value) => String(value)}
              onChange={(rounds) => onPatch({ customPreset: { rounds } })}
            />
          </Row>
        </Group>

        <Group title="Chime">
          <Row title="Play a chime" hint="Sounds when a phase ends.">
            <Switch
              label="Play a chime"
              checked={settings.chimeEnabled}
              onChange={(chimeEnabled) => onPatch({ chimeEnabled })}
            />
          </Row>
          <Row title="Voice" stack>
            <Segmented
              value={settings.chimeVoice}
              options={CHIME_VOICES}
              onChange={(chimeVoice) => onPatch({ chimeVoice })}
            />
          </Row>
          <Row title="Volume" stack>
            <Slider
              label="Chime volume"
              value={settings.chimeVolume}
              onChange={(chimeVolume) => onPatch({ chimeVolume })}
            />
          </Row>
          <button className="row row--tappable" onClick={() => void call("preview_chime")}>
            <span className="link">Hear it</span>
          </button>
        </Group>

        <Group title="Noise">
          <Row
            title="Keep playing through breaks"
            hint="Volume and the on switch are on the main screen."
          >
            <Switch
              label="Keep noise playing through breaks"
              checked={settings.noiseDuringBreaks}
              onChange={(noiseDuringBreaks) => onPatch({ noiseDuringBreaks })}
            />
          </Row>
        </Group>

        <Group title="App">
          {platform === "macos" && (
            <Row title="Countdown in the menu bar" hint="Hidden while the timer is idle.">
              <Switch
                label="Show the countdown in the menu bar"
                checked={settings.showClockInMenuBar}
                onChange={(showClockInMenuBar) => onPatch({ showClockInMenuBar })}
              />
            </Row>
          )}
          <Row title="Notifications">
            <Switch
              label="Show notifications"
              checked={settings.notificationsEnabled}
              onChange={(notificationsEnabled) => onPatch({ notificationsEnabled })}
            />
          </Row>
          {DESKTOP.has(platform) && (
            <Row title="Open at login">
              <Switch
                label="Open at login"
                checked={settings.launchAtLogin}
                onChange={(launchAtLogin) => onPatch({ launchAtLogin })}
              />
            </Row>
          )}
          <Row title="Appearance" stack>
            <Segmented
              value={settings.theme}
              options={THEMES}
              onChange={(theme) => onPatch({ theme })}
            />
          </Row>
        </Group>

        <Group title="About">
          <button className="row row--tappable" onClick={() => void call("open_repository")}>
            <span className="link">Source code and issues</span>
          </button>
          <button className="row row--tappable" onClick={() => void call("quit_app")}>
            <span className="link">Quit Lock In</span>
          </button>
        </Group>

        <p className="about">
          Lock In is free and open source, for everyone, forever.
          <br />
          No account, no paywall, nothing sent anywhere.
        </p>
      </div>
    </div>
  );
}
