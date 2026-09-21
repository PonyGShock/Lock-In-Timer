# Getting Lock In to people

Shipping a free app is not free. This is what each route actually costs and
demands, so the decision can be made with open eyes rather than discovered
halfway through.

> Fees and store policies change. Everything below was accurate as far as we
> knew when it was written — **check the current terms before paying anyone.**

## Where things stand

| Platform | Status |
| --- | --- |
| macOS | Builds and bundles. Needs testing on real hardware. |
| Windows | Builds in CI. Untested on a real desktop. |
| Linux | Builds from source. Not released. |
| Android | Not started. Tauri supports it; the app needs real work first. |
| iOS | Not started. Same. |

## The free routes, which are the point

### GitHub Releases — macOS, Windows, Linux

Already wired up: push a `v*` tag and `.github/workflows/release.yml` builds
and drafts a release. Costs nothing. The catch is that unsigned builds get
scary warnings:

- **macOS** says the app "is damaged and can't be opened". It is not damaged;
  it is unsigned. `xattr -cr "/Applications/Lock In.app"` clears it.
- **Windows** shows a SmartScreen "unrecognised app" wall. "More info" then
  "Run anyway" gets past it.

Both are documented in the README. Plenty of well-known free software ships
exactly like this.

### F-Droid — Android

The natural home for an app like this, and the only store route that costs
nothing at all. F-Droid builds from source themselves and verifies the app is
genuinely free software, which Lock In is.

Requirements: a GPL-compatible licence (have it), a reproducible build from a
tagged commit, no proprietary dependencies or analytics (have none), and
metadata in the `fastlane/metadata/android` layout — already in this repo, so
the listing text is ready when the Android build is.

This should be the first Android target.

## The paid routes

### Apple — macOS notarisation, Mac App Store, iOS

Everything Apple needs the **Apple Developer Program at $99/year**. There is
no free path onto an iPhone for other people, which is simply Apple's rule.

That one membership covers three different things:

1. **Notarising the macOS build.** Removes the "damaged" warning. This is the
   single highest-value thing the money buys — it makes the free GitHub
   download work without a terminal command.
2. **Mac App Store.** Optional; the direct download already works.
3. **iOS App Store.** The only way to reach iPhones at scale.

If the budget ever stretches to one paid thing, make it this, and use it for
notarisation first.

To add notarisation later: set `APPLE_CERTIFICATE`,
`APPLE_CERTIFICATE_PASSWORD`, `APPLE_SIGNING_IDENTITY`, `APPLE_ID`,
`APPLE_PASSWORD` and `APPLE_TEAM_ID` as repository secrets. `tauri-action`
picks them up on its own; the workflow does not need restructuring.

### Google Play — Android

**$25, once, ever.** Cheap, and the only realistic way to reach most Android
users who have never heard of F-Droid.

Two things to know before starting:

- Personal developer accounts created in recent years have had to run a closed
  test with a minimum number of testers for a minimum period before being
  allowed into production. Budget weeks, not an afternoon.
- You must publish a privacy policy URL and fill in the Data Safety form.
  [PRIVACY.md](../PRIVACY.md) is written for this; the honest answer to every
  question on the form is "no data collected".

### Microsoft Store — Windows

An individual developer account has historically cost a small one-time fee,
and Microsoft has at times waived it. Worth checking, because store
distribution also sidesteps the SmartScreen warning.

Separately, signing the Windows build directly needs a code signing
certificate. Traditional certificates run to hundreds of dollars a year;
Microsoft's cloud signing service is much cheaper on a monthly basis. Either
way this is the least urgent spend — "Run anyway" works.

## A suggested order

1. **Tag `v0.1.0` and ship the GitHub release.** Free, today, once macOS has
   been tested on real hardware.
2. **Test and fix Windows.** The code is there; nobody has run it.
3. **Do the Android port properly**, then submit to F-Droid. Still free.
4. **Pay Google the $25** when the Android build has been in F-Droid long
   enough to have its rough edges filed off.
5. **Pay Apple the $99** when either notarisation is worth it or iOS is real.

## What the mobile ports actually need

Not a recompile. The desktop-only pieces are already gated behind
`#[cfg(desktop)]` and the interface has a phone layout, so it should build —
but building is the easy part. The real work:

- **Surviving the background.** A phone suspends your process. The timer must
  finish correctly when the app has not been running, which means scheduling a
  local notification for the end of the phase up front and reconciling the
  state on resume, rather than counting down in a thread.
- **Audio in the background.** Playing noise while the screen is off needs a
  foreground service on Android and the correct background audio mode on iOS.
  Both stores scrutinise this.
- **No tray.** The whole menu bar interaction has no meaning on a phone. The
  app needs its own way to start and stop from outside — a home screen widget,
  a Live Activity, a notification with actions.
- **Store assets.** Screenshots at each required size, a feature graphic for
  Play, and the listing text (drafted in `fastlane/metadata/android`).
