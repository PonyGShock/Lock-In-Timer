# Privacy policy

**Last updated:** 21 September 2026
**Applies to:** the Lock In app on macOS, Windows, Android and iOS.

## The short version

Lock In does not collect anything. There is no account, no analytics, no
crash reporting, no advertising and no tracking of any kind. Nothing you do in
the app is transmitted anywhere, because the app contains no code that talks
to a network.

## What is stored, and where

One file, on your own device: `settings.json`, in the standard configuration
directory for your platform. It contains:

- your chosen timer preset and any custom lengths you set
- your sound preferences (chime on or off, which voice, volume; noise type,
  volume and tone)
- your app preferences (theme, notifications, menu bar countdown, open at
  login)
- the number of focus sessions you have finished today, and today's date, so
  the count can reset at midnight

That is the whole file. It never leaves your device. Deleting the app removes
it, and you can delete it yourself at any time — the app will simply start
again from its defaults.

## What is never collected

No name, email address or account. No contacts, photos, files, location,
calendar or device identifiers. No usage statistics, session history,
screenshots or diagnostics. No advertising identifier. Nothing is sold or
shared with anybody, because nothing is gathered in the first place.

## Permissions the app asks for

- **Notifications** — to tell you a focus session or break has ended. Optional;
  the app works without it.
- **Open at login** (desktop only) — to start the app when you sign in.
  Optional and off by default.

## Network access

The app makes no network requests. The single exception is that tapping
"Source code and issues" in Settings asks your operating system to open
`github.com/PonyGShock/Lock-In-Timer` in your normal browser. At that point you
are on GitHub's website, under GitHub's privacy policy, and Lock In is not
involved.

If you download the app from an app store, that store will have its own record
of the download under its own privacy policy. That is between you and them.

## Children

The app is suitable for all ages and collects no data from anyone, including
children.

## Verifying any of this

You do not have to take our word for it. The complete source code is at
<https://github.com/PonyGShock/Lock-In-Timer> under the GPL-3.0 licence. You
can read it, search it for network calls, and build the app yourself.

## Changes

If this policy ever changes, the new version will be committed to the
repository, so the full history of what was promised and when is public in
`git log`.

## Contact

Open an issue at
<https://github.com/PonyGShock/Lock-In-Timer/issues>.
