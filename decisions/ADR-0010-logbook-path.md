# ADR-0010 — Default logbook path and the wizard's path options

**Status:** accepted · **Date:** 2026-10-01

## Context
The logbook is user data: a git repository, an Obsidian vault, Markdown
meant to be read and edited. Engine configuration and state already live
XDG-style in `~/.config/seldon` and `~/.local/state/seldon`. A hidden
folder (`~/.seldon`) would mix data with state, be skipped by file dialogs
and Obsidian's vault picker, and be forgotten in backups. Paths like
`~/Work/...` are one user's convention, not a default.

## Decision
`seldon init` offers three fixed options plus a free path:

1. `~/Seldon` — **default**. Top-level, visible, one logbook per machine.
2. `~/Documents/Seldon` — for tidy homes and Obsidian users who keep vaults
   there. Uses the XDG `DOCUMENTS` directory when it is configured.
3. A detected project folder: the wizard checks, in this order, for
   `~/Work`, `~/Projects`, `~/dev`, `~/src`, `~/code` and offers
   `<first existing>/seldon`. Not shown when none exists.
4. Custom path, typed by the user.

Spelling follows the surroundings: capitalised at the top level of the
home directory like `~/Documents`, lowercase inside a project folder like
other repositories. `--non-interactive` takes option 1 or `--path`.

## Consequences
ADR-0001's `~/Seldon` is confirmed. The chosen path is stored in
`~/.config/seldon/config.toml`; nothing inside the logbook depends on it
(SPEC-LOGBOOK §7). WP-003 implements the detection and the dialog.
