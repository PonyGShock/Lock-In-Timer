# Security policy

## Reporting a vulnerability

Please **do not** open a public issue for a security problem.

Use GitHub's private reporting instead:
[Security → Report a vulnerability](https://github.com/PonyGShock/Lock-In-Timer/security/advisories/new).

Tell us what you found, how to reproduce it, and what an attacker could do
with it. You will get an acknowledgement as soon as the maintainer sees it.
This is a volunteer project, so please allow a little time.

## Supported versions

The latest release is the supported one. There are no long-term support
branches.

## What the app actually does

Worth knowing before you go looking, because it narrows the attack surface a
lot:

- **No network code.** Lock In makes no HTTP requests. It has no server, no
  account system and no update checker. The only outbound action is opening
  the project's GitHub page in your browser when you tap that link in
  Settings.
- **No stored secrets.** The only thing written to disk is a settings file
  (`settings.json` in the platform config directory) containing timer lengths
  and preferences.
- **The webview has a restrictive CSP** and loads only bundled local assets —
  no remote scripts, fonts or images.

Interesting classes of bug would therefore be things like: a malicious
settings file causing a crash or worse, a path traversal in where settings are
written, or a way for a webview to reach a Tauri command it should not.

## Builds are not signed

Released binaries are unsigned, because code signing certificates cost money
and this app is free. That means your operating system cannot verify the
download came from us. If that matters to you, build from source — the
instructions are in the README and the whole thing takes a few minutes.
