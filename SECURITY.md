# Security policy

## Supported versions

Seldon is before 1.0. Only the **latest release** gets security fixes;
the fix ships as a new patch release of the engine and the plugin
together (see [docs/VERSIONING.md](docs/VERSIONING.md)).

| Version | Supported |
|---|---|
| latest `0.x` release | yes |
| older releases | no — upgrade first |
| `main` between releases | best effort |

## Reporting a vulnerability

**Do not open a public issue for a vulnerability.**

1. **Preferred:** GitHub private vulnerability reporting —
   [Security → Report a vulnerability](https://github.com/JohnAndrewsX/jax-seldon/security/advisories/new)
   in this repository. The report stays private until an advisory is
   published.
2. **Alternative:** e-mail the maintainer at `e.andres+aur@ik.me` (the
   address in `packaging/PKGBUILD`).

This also covers the plugin repository
[`jax-seldon-plugin`](https://github.com/JohnAndrewsX/jax-seldon-plugin),
which is generated from `plugin/` here; report plugin issues to this
repository.

Please include the version (`seldon --version --json`, the plugin version
from its `manifest.json`), the steps to reproduce, and what an attacker
gains. Remove host names, user names, tokens and logbook content you do
not want to share; a minimal reproduction in a scratch logbook is best.

Seldon has a single maintainer. Expect an acknowledgement within 7 days
and an assessment within 30 days. Fixed issues are credited in the
advisory and in `CHANGELOG.md` unless you ask otherwise.

## Scope

How Seldon runs, and therefore what counts as a vulnerability:

- **The engine** (`seldon`) runs as the logged-in user and needs no
  root; it has **no network code**. It reads system logs and command
  output (`/var/log/pacman.log`, snapper, the `omarchy` CLI), reads the hook
  payloads that agent harnesses (Claude Code, Omarchy Agent) pipe into
  `seldon hook`, and writes only the logbook, its own config and state
  (`~/.config/seldon`, `~/.local/state/seldon`), and the hook scripts and
  harness settings the user asks it to install (an agent harness's
  `settings.json`, the theme hook under
  `~/.config/omarchy/hooks/theme-set.d/`). Git commits in the logbook are
  local; Seldon never pushes.
- **Redaction** (`docs/SPEC-ENGINE.md` §7): collectors and hooks redact
  secrets before an event is written. A secret that matches a documented
  rule and still reaches the logbook is a vulnerability.
- **The plugin** (`jax.seldon`) runs **unsandboxed inside the Omarchy
  shell process**. It reads one file (`index.json`) and runs the engine
  only with fixed argument lists (`docs/CONTRACT.md`). Anything in the
  logbook or the index that makes the plugin or the engine execute a
  command, open an unintended file, or evaluate content is a
  vulnerability.

In scope, for example: command or argument injection from logbook,
index, hook or log content; a path outside the logbook or Seldon's own
directories written by the engine; secrets leaking past redaction; the
release workflow or the AUR package shipping something other than the
tagged source.

Out of scope: an attacker who can already write to your home directory or
run code as your user (they can do anything Seldon can); bugs in Omarchy,
Quickshell, Hyprland, snapper or the agent harnesses (report upstream);
the content you put into your own logbook and push to a remote yourself;
denial of service by hand-editing your own logbook (that is a bug —
please open a normal issue).
