# Contributing

Thanks for looking. Lock In is meant to stay small, free and pleasant, and
help with any of those three is welcome.

## Getting set up

You need [Rust](https://rustup.rs) and [Node 20+](https://nodejs.org). Then:

```sh
npm install
npm run icons   # generates src-tauri/icons from app-icon.png
npm run app     # runs the real app
```

On Linux you also need the GTK and audio headers:

```sh
sudo apt install libwebkit2gtk-4.1-dev libasound2-dev libgtk-3-dev librsvg2-dev
```

### Working on the interface without building the app

The UI runs in an ordinary browser against a stand-in backend in
`src/mock.ts`, which is far quicker than rebuilding for a spacing change:

```sh
npm run dev     # http://localhost:1420
```

Query parameters seed a state to design against:

```
?state=running&remaining=1122&round=2&done=3&noise=1&theme=dark
```

If you change something the backend owns — a new command, a new settings
field — update `src/mock.ts` too, or the browser view quietly drifts away from
the real app.

## Before you open a pull request

```sh
cargo fmt --all
cargo test -p lockin-core
cargo clippy --workspace --all-targets -- -D warnings
npm run build
```

CI runs exactly these, plus a real macOS and Windows build.

## How the code is laid out

`crates/lockin-core` holds everything that can be tested without a window or a
sound card: the timer state machine, the noise and chime synthesis, and the
settings file. If you are adding logic, try to put it here — it is the part
that is cheap to test and hard to get subtly wrong unnoticed.

`src-tauri` is the app: tray, audio device, window, and the commands the UI
calls. `src` is the React interface.

Two rules that keep the rest of it honest:

- **The timer never reads the clock itself.** `Timer::advance` takes the
  current time as an argument. That is what makes every transition testable,
  including awkward ones like a laptop sleeping through the end of a session.
- **No audio files.** Noise and chimes are synthesised. This keeps the app
  small, avoids sample licensing entirely, and means the noise never loops.

## What is likely to be accepted

- Bug fixes, with a test if the bug was in `lockin-core`.
- Platform work — Windows polish, and the Android and iOS ports.
- Accessibility and localisation.
- Anything that removes code without removing the feature.

## What is likely to be declined

- Accounts, sync, analytics, telemetry or crash reporting that phones home.
- Paid tiers, upsells, "pro" features, or anything that makes a feature
  conditional on payment. This is the entire reason the project exists.
- Task lists, calendars, habit tracking. There are good apps for those. This
  one is a timer.

If you are unsure whether something fits, open an issue before writing it.
Nobody enjoys having a finished pull request turned down.

## Commits

Write the message for someone reading `git log` in a year: what changed and
why, not what file you touched. Small, focused commits are easier to review
than one large one.
