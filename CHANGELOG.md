# Changelog

What changed in each version of Lock In, newest first. Downloads are on the
[releases page](https://github.com/PonyGShock/Lock-In-Timer/releases).

## 0.2.0

### New look

- A new icon: a cup seen from above, its crema drawn as a timer ring.
- The menu bar icon is that cup, and it fills as the session runs. It dims
  while paused. On Windows it is a small full-colour tile, so it no longer
  disappears on a dark taskbar.
- The right-click menu opens with the phase, the time left and the round.
- The name and the phase are set in a soft serif, the dial sits in a faint
  saucer, and the window has a light paper grain. Text on the accent colour
  is easier to read in dark mode.
- A website: https://ponygshock.github.io/Lock-In-Timer/

### Fixed

- Tapping the preset that was already selected reset a running session.
- Changing any Custom length switched to the Custom preset and reset the
  timer. It now only edits Custom, and if Custom is in use the running
  session is adjusted rather than restarted.
- "Source code and issues" did nothing in the installed app.
- One unrecognised value in the settings file reset every setting. Now only
  that value falls back to its default.
- Two saves at once could corrupt the settings file, and dragging a slider
  rewrote it about a hundred times. Saves are now batched and made in order,
  and nothing pending is lost when you quit.
- Pressing "Hear it" several times queued the chimes, which could delay the
  real end-of-session chime by half a minute.
- A dropped message to the audio thread could leave noise playing through a
  break.
- "Open at login" could claim to be on when it was not.
- Clicking the menu bar icon while the window was open closed it and opened
  it again.
- The menu bar countdown switch is only shown on macOS, where it works.
- Quick repeated clicks on the length steppers were lost, and the volume
  slider could jump backwards.
- Holding Space toggled the timer many times a second, and Space on a
  focused button pressed the timer instead of that button.
- Tab could reach the hidden timer behind the settings screen, and three
  settings rows could not be reached by keyboard at all.
- The noise volume no longer appears twice.

## 0.1.0

The first release: five timer lengths, synthesised chimes, a deep ambient
rumble, light and dark themes, for the macOS menu bar and the Windows tray.
