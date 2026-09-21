<div align="center">

<img src="app-icon.png" width="112" alt="Crema" />

# Crema

**A calm pomodoro timer that lives in your menu bar.**

Free and open source. No account, no subscription, no paywall, no telemetry.
Every feature is in the app the moment you install it.

</div>

<div align="center">
<img src="docs/screenshots/idle-light.png" width="240" alt="Idle timer" />
<img src="docs/screenshots/running-light.png" width="240" alt="Running focus session with brown noise" />
<img src="docs/screenshots/break-light.png" width="240" alt="Short break" />
</div>

## Why

Focus timers are about the simplest software there is, and somehow most of them
want a subscription, an account, or your attention. The alternative — a timer on
a website — means opening a browser, which is where focus goes to die.

Crema is a small thing that sits in the menu bar, counts down, and makes a
pleasant sound. That is the whole product, and it should not cost anything.

## What it does

- **Five timer lengths.** Espresso (15/3), Classic (25/5), Deep (50/10),
  Flow (90/20) and a Custom one you set yourself.
- **A countdown in the menu bar**, shown only while a session is actually
  running. An idle timer is a quiet one.
- **A chime at each boundary**, synthesised rather than sampled: a soft bell,
  a singing bowl or a wooden block. Focus ends on a lower note than a break, so
  you can tell them apart without looking.
- **White, pink or brown noise** while you work, with volume and a tone control
  that runs from muffled and distant to fully open. It fades in and out, never
  loops, and stops on its own when the session does.
- **Breaks that start themselves** (and focus that does not, unless you ask).
- **Light and dark**, following the system or pinned either way.

Nothing leaves your machine. There is no network code in the app at all — the
only outbound link is the one to this repository.

## Install

> [!NOTE]
> Crema is at version 0.1 and only macOS is built so far. Windows, iOS and
> Android are the plan — see [Roadmap](#roadmap).

Download the `.dmg` from [Releases](../../releases), drag Crema to
Applications, and launch it. The icon appears in the menu bar; there is no dock
icon and no window in the app switcher, which is intentional.

**The first launch needs one extra step.** Releases are not signed with an Apple
Developer certificate, because that costs $99 a year and this app is free. macOS
will therefore say Crema "is damaged and can't be opened", which is Gatekeeper
being unhelpfully vague about an unsigned app. Clear the quarantine flag once:

```sh
xattr -cr /Applications/Crema.app
```

If you would rather not take that on faith, build it yourself — it takes about
five minutes.

## Build from source

You need [Rust](https://rustup.rs), [Node 20+](https://nodejs.org) and, on
macOS, the Xcode command line tools (`xcode-select --install`).

```sh
git clone https://github.com/PonyGShock/crema.git
cd crema
npm install
npm run icons      # renders the platform icon set from app-icon.png
npm run app        # development build with hot reload
npm run app:build  # release build, output in src-tauri/target/release/bundle
```

### Working on the interface

The UI runs in an ordinary browser against a stand-in backend, which is much
faster than rebuilding the app for a spacing change:

```sh
npm run dev        # then open http://localhost:1420
```

Query parameters seed a state to design against, which is also how the
screenshots above were made:

```
http://localhost:1420/?state=running&remaining=1122&round=2&done=3&noise=1&theme=dark
```

### Tests

```sh
cargo test -p crema-core   # timer, synthesis and settings
npm run build              # typecheck and bundle the UI
```

## How it works

```
crates/crema-core     no system dependencies, all the logic worth testing
  timer.rs            the phase state machine, driven by an injected clock
  noise.rs            white / pink / brown generators and the fade envelope
  chime.rs            additive synthesis of the boundary chimes
  settings.rs         the settings file, its clamping and its migrations
src-tauri             the macOS app
  engine.rs           owns the timer and decides what should be audible
  audio.rs            a thread owning the output device, fed over a channel
  tray.rs             menu bar icon, title and menu
src                   the React popover
```

Two decisions are worth knowing about if you plan to contribute:

**The timer lives in Rust, not in the webview.** The popover is a view onto
state the backend owns, so the countdown keeps perfect time whether the window
is open, hidden, or has never been opened at all. The engine takes `now` as an
argument rather than reading the clock, which is what makes its behaviour
testable down to the millisecond — including what happens when a laptop sleeps
through the end of a session.

**All audio is synthesised at runtime.** There are no audio files in this
repository. Noise is generated sample by sample, so it never loops and the
installer stays small; the chimes are sums of decaying partials at inharmonic
ratios, which is what makes a struck bell sound struck. It also means there is
no sample licence for anyone to trip over when they fork this.

## Roadmap

In rough order:

- **Windows 11** — the tray, audio and UI are already cross-platform; this is
  mostly positioning the popover against the taskbar and testing it.
- **iOS and Android** — Tauri 2 builds both from this codebase. The timer needs
  to survive backgrounding, which is real work rather than a recompile.
- Keyboard shortcut to start and pause from anywhere.
- A gentle heads-up a minute before a session ends.

Issues and pull requests are welcome. If you want a feature, say so — the point
of this project is that nobody has to pay to get their timer to work properly.

## License

[GPL-3.0-or-later](LICENSE). You can use, study, change and share it. If you
distribute a modified version, it has to stay free in the same way, which is the
entire reason for choosing this licence over something more permissive: nobody
should be able to take this, close it, and start charging for it.
