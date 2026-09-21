import { CHIME_VOICES, type AppState, type Settings, type Theme } from "../types";
import { openExternal } from "../bridge";
import { BackIcon, Group, Row, Segmented, Slider, Stepper, Switch } from "./ui";

const REPOSITORY = "https://github.com/PonyGShock/crema";

const THEMES: { id: Theme; label: string }[] = [
  { id: "system", label: "System" },
  { id: "light", label: "Light" },
  { id: "dark", label: "Dark" },
];

const minutes = (secs: number) => `${Math.round(secs / 60)} min`;

export function SettingsSheet({
  open,
  state,
  onPatch,
  onCustomPreset,
  onPreviewChime,
  onClose,
  onQuit,
}: {
  open: boolean;
  state: AppState;
  onPatch: (patch: Partial<Settings>) => void;
  onCustomPreset: (preset: {
    focusSecs: number;
    shortBreakSecs: number;
    longBreakSecs: number;
    rounds: number;
  }) => void;
  onPreviewChime: () => void;
  onClose: () => void;
  onQuit: () => void;
}) {
  const { settings } = state;
  const custom = settings.customPreset;

  const patchCustom = (patch: Partial<typeof custom>) =>
    onCustomPreset({
      focusSecs: custom.focusSecs,
      shortBreakSecs: custom.shortBreakSecs,
      longBreakSecs: custom.longBreakSecs,
      rounds: custom.rounds,
      ...patch,
    });

  return (
    <div className="sheet" data-open={open} aria-hidden={!open}>
      <header className="header">
        <button className="icon-button" onClick={onClose} aria-label="Back to timer">
          <BackIcon />
        </button>
        <span className="wordmark">Settings</span>
        <span style={{ width: 30 }} />
      </header>

      <div className="sheet__body">
        <Group title="Sessions">
          <Row
            title="Start breaks automatically"
            hint="A break begins the moment focus ends."
          >
            <Switch
              label="Start breaks automatically"
              checked={settings.behavior.autoStartBreaks}
              onChange={(autoStartBreaks) =>
                onPatch({ behavior: { ...settings.behavior, autoStartBreaks } })
              }
            />
          </Row>
          <Row
            title="Start focus automatically"
            hint="Off by default, so a break ends when you say so."
          >
            <Switch
              label="Start focus automatically"
              checked={settings.behavior.autoStartFocus}
              onChange={(autoStartFocus) =>
                onPatch({ behavior: { ...settings.behavior, autoStartFocus } })
              }
            />
          </Row>
        </Group>

        <Group title="Custom lengths">
          <Row title="Focus">
            <Stepper
              label="focus length"
              value={custom.focusSecs}
              min={5 * 60}
              max={180 * 60}
              step={5 * 60}
              format={minutes}
              onChange={(focusSecs) => patchCustom({ focusSecs })}
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
              onChange={(shortBreakSecs) => patchCustom({ shortBreakSecs })}
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
              onChange={(longBreakSecs) => patchCustom({ longBreakSecs })}
            />
          </Row>
          <Row
            title="Rounds"
            hint="Focus sessions before the long break."
          >
            <Stepper
              label="rounds"
              value={custom.rounds}
              min={1}
              max={12}
              format={(value) => String(value)}
              onChange={(rounds) => patchCustom({ rounds })}
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
          <div className="row row--tappable" onClick={onPreviewChime}>
            <span className="link">Hear it</span>
          </div>
        </Group>

        <Group title="Noise">
          <Row title="Keep playing through breaks">
            <Switch
              label="Keep noise playing through breaks"
              checked={settings.noiseDuringBreaks}
              onChange={(noiseDuringBreaks) => onPatch({ noiseDuringBreaks })}
            />
          </Row>
          <Row title="Volume" stack>
            <Slider
              label="Noise volume"
              value={settings.noiseVolume}
              onChange={(noiseVolume) => onPatch({ noiseVolume })}
            />
          </Row>
          <Row title="Tone" hint="Muffled and distant on the left, open on the right." stack>
            <Slider
              label="Noise tone"
              value={settings.noiseTone}
              onChange={(noiseTone) => onPatch({ noiseTone })}
            />
          </Row>
        </Group>

        <Group title="App">
          <Row title="Countdown in the menu bar" hint="Hidden while the timer is idle.">
            <Switch
              label="Show the countdown in the menu bar"
              checked={settings.showClockInMenuBar}
              onChange={(showClockInMenuBar) => onPatch({ showClockInMenuBar })}
            />
          </Row>
          <Row title="Notifications">
            <Switch
              label="Show notifications"
              checked={settings.notificationsEnabled}
              onChange={(notificationsEnabled) => onPatch({ notificationsEnabled })}
            />
          </Row>
          <Row title="Open at login">
            <Switch
              label="Open at login"
              checked={settings.launchAtLogin}
              onChange={(launchAtLogin) => onPatch({ launchAtLogin })}
            />
          </Row>
          <Row title="Appearance" stack>
            <Segmented
              value={settings.theme}
              options={THEMES}
              onChange={(theme) => onPatch({ theme })}
            />
          </Row>
        </Group>

        <Group title="About">
          <div
            className="row row--tappable"
            onClick={() => void openExternal(REPOSITORY)}
          >
            <span className="link">Source code and issues</span>
          </div>
          <div className="row row--tappable" onClick={onQuit}>
            <span className="link">Quit Crema</span>
          </div>
        </Group>

        <p className="about">
          Crema is free and open source, for everyone, forever.
          <br />
          No account, no paywall, nothing sent anywhere.
        </p>
      </div>
    </div>
  );
}
