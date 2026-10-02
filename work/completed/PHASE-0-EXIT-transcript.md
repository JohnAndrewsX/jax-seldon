# Phase 0 exit (G3) — transcript

Date: 2026-10-02 · Operator's dev host (Omarchy 4.0.4-1) · engine `seldon 0.1.0`
(static musl build from main at 6c006dd, installed to `~/.local/bin/seldon`).
Private paths and host names are redacted (`<user>`, `<machine>`).

## 1. Preparation

```
rm -r ~/.config/seldon                      # stray config from the WP-024 pty test
just build-release                          # static musl binary, 5.1 MB
install -Dm755 engine/target/x86_64-unknown-linux-musl/release/seldon ~/.local/bin/seldon
seldon --version                            # seldon 0.1.0
```

## 2. Wizard

```
$ seldon init --path ~/Seldon --obsidian --harness claude-code
✓ Language of the logbook prose · Deutsch (de)
✓ Collectors · snapper, pacman, omarchy, plugins, theme, config
✓ Watched config paths · ~/.config/hypr, ~/.config/omarchy, ~/.config/waybar, ~/.bashrc, ~/.zshrc
✓ More paths · (none)
✓ Record theme switches the moment they happen? · yes
✓ Make the logbook a git repository with a first commit? · yes
✓ Backfill since · 2026-09-29
The backfill opened 22 drift item(s) (22 crisis): changes from before Seldon, none of them in a case.
✓ Mark them as the pre-Seldon baseline? · yes
Logbook created at ~/Seldon (machine <machine>, language de, 33 files).
Config: ~/.config/seldon/config.toml
Harness claude-code: .claude/settings.json: 3 hook(s) added, 0 already there
Git: repository initialised, first commit "seldon: init logbook"
Snapper: degraded — No permissions (ADR-0011)
First capture: 1230 event(s) since 2026-09-29T00:00:00+02:00; 22 drift item(s) dismissed as "pre-Seldon baseline"; 0 open drift item(s), 0 crisis
Dossier: Wrote system/hardware.md, system/omarchy.md, system/packages.md, system/plugins.md, system/services.md (7 fence(s) changed)
Theme hook: installed (~/.config/omarchy/hooks/theme-set.d/seldon-theme-set.sh)
Obsidian: open the folder as a vault.

$ seldon doctor
  ok        engine    seldon 0.1.0, contract 1
  ok        config    ~/.config/seldon/config.toml
  ok        logbook   ~/Seldon · machine <machine> · de · 0 cases, 0 decisions, 0 journal days
  ok        omarchy   Omarchy 4.0.4-1
  degraded  snapper   No permissions. fix: sudo snapper -c root set-config ALLOW_USERS=$USER SYNC_ACL=yes
  ok        git       git version 2.55.0; logbook is a repository; autocommit on
doctor: ok

$ sudo snapper -c root set-config ALLOW_USERS=$USER SYNC_ACL=yes     # operator's choice (ADR-0011)
$ seldon doctor
  ok        snapper   4 snapshots (config root)
doctor: ok
```

## 3. A real case with Claude Code

```
$ seldon plan new --zone yellow --risk R1 --area dev-env -- "Phase 0 exit: first real case"
Created C-2026-001 "Phase 0 exit: first real case" in work/queued/C-2026-001-phase-0-exit-first-real-case.md
$ seldon plan start C-2026-001
$ cd ~/Seldon && claude
```

Claude Code (with the installed PreToolUse/SessionEnd hooks) read the
logbook's AGENTS.md, filled the case's Intent and Plan, switched the theme
Osaka Jade → Flexoki Light → Gruvbox → Osaka Jade with `omarchy theme set`,
and moved the case to verification itself. Ledger lines written by the hook:

```
09:43:02  theme-set  flexoki-light  C-2026-001  agent:claude-code
09:43:23  theme-set  gruvbox        C-2026-001  agent:claude-code
09:43:43  theme-set  osaka-jade     C-2026-001  agent:claude-code
09:43:48  note                      C-2026-001  agent:claude-code
09:43:48  case-verified             C-2026-001  agent:claude-code
```

## 4. Capture, status, drift, close (run by the orchestrator on the operator's go)

```
$ seldon capture --all
Captured 1 new event(s).   config 1
$ seldon status
  cases   0 active · 1 in verification · 0 queued
  drift   1 open · 0 crisis
  events  11 today · 1241 in 7 days
$ seldon drift
yellow  2026-10-02 09:20  config/config-add  ~/.config/omarchy/hooks/theme-set.d/seldon-theme-set.sh  01M3XS7Z0PZFWA88Y83771YG2R
```

The one open item is the theme hook script that `seldon init --theme-hook`
installed itself (finding F1 below).

```
$ seldon drift explain 01M3XS7Z0PZFWA88Y83771YG2R -- "Installed by seldon init (theme hook opt-in); engine-owned file"
Explained 1 event(s) with the new completed case C-2026-002
$ seldon plan done C-2026-001
C-2026-001 verification → completed
$ seldon status
  cases   0 active · 0 in verification · 0 queued
  drift   0 open · 0 crisis
  events  14 today · 1244 in 7 days
```

## 5. Look

`~/Seldon/STATUS.md` (German prose, English headings per ADR-0007):

```
# Status — <machine>
Stand: 2026-10-02 · letztes Ereignis 09:46 · Omarchy 4.0.4-1 · Theme osaka-jade
## Overview
- Aktive Cases: 0 · in Prüfung: 0 · geplant: 0
- Offene Drift: 0, davon Krise: 0
- Ereignisse heute: 14 · letzte 7 Tage: 1244
## Open drift
- keine
```

Obsidian opens `~/Seldon` as a vault (operator).

## Result

G3 criteria (PLAN.md): `seldon init` on the operator's machine ✓ · one real
case worked with Claude Code ✓ (hook-attributed events, case lifecycle) ·
events ✓ (1244 in the ledger) · drift ✓ (backfill baseline, one real item
explained) · STATUS.md ✓ · logbook opens in Obsidian ✓ (operator).

**Gate G3 passed. Phase 0 exits.**

## Findings

- **F1** `seldon init --theme-hook` writes
  `~/.config/omarchy/hooks/theme-set.d/seldon-theme-set.sh` and the next
  capture reports that file as open yellow drift. The engine should
  attribute its own installs (actor `seldon`, an automatic explanation or
  a dismissed resolution) → small engine follow-up.
- **F2** The procedure in `work/PHASE-0-EXIT.md` predates WP-024/035:
  `seldon hook install claude-code` is no longer needed after `init
  --harness claude-code`, the wizard runs the first capture, baseline and
  dossier itself, and Omarchy 4 keeps the Hyprland config as Lua files
  (`~/.config/hypr/*.lua`), so the "edit hyprland.conf" example is stale.
- **F3** Claude Code, given only the logbook's AGENTS.md, planned the case,
  ran it and moved it to verification on its own — the agent-facing
  contract works without further instruction.
