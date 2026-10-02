# Phase 0 exit (G3) — procedure for a fresh machine

PLAN.md: *"The operator runs `seldon init` on their machine, works one real
case with Claude Code, sees events, drift and a STATUS.md. Logbook opens in
Obsidian."* Gate G3 passed on 2026-10-02
(`work/completed/PHASE-0-EXIT-transcript.md`). This file is the
reference for doing it again on a fresh machine with the shipped wizard
(WP-024, WP-035, WP-038). The orchestrator cannot run it: `seldon init`
writes `~/.config/seldon/config.toml`, `~/.local/state/seldon/` and the
real logbook, and `--theme-hook` writes under `~/.config/omarchy/hooks/`
(AGENTS.md §6). The operator runs every command below.

**You need:** Omarchy 4 (tested on 4.0.4-1), `git`, Claude Code (`claude`)
and, optionally, Obsidian. Use an engine that contains WP-038, the release
after 0.1.0 or a build from `main` (step 1). With 0.1.0 the theme hook file shows
up as one open drift item after the first capture; explain it as in
the transcript (finding F1).

## 1. Install the engine

Pick one:

```bash
# a) the AUR package, once it is registered (ADR-0016)
omarchy pkg aur add jax-seldon

# b) the static release binary from GitHub (until then)
V=<version>                                          # e.g. 0.1.1
curl -LO https://github.com/JohnAndrewsX/jax-seldon/releases/download/v$V/seldon-$V-x86_64-unknown-linux-musl.tar.gz
tar xzf seldon-$V-x86_64-unknown-linux-musl.tar.gz
install -Dm755 seldon-$V-x86_64-unknown-linux-musl/seldon ~/.local/bin/seldon

# c) a build from a checkout of this repository
just build-release
install -Dm755 engine/target/x86_64-unknown-linux-musl/release/seldon ~/.local/bin/seldon
```

```bash
seldon --version                                     # seldon <version>
```

Optional, the bar plugin (ADR-0009):

```bash
omarchy plugin add https://github.com/JohnAndrewsX/jax-seldon-plugin.git
omarchy plugin list                                  # jax.seldon; enable it if it is listed as disabled
```

## 2. `seldon init` — one command sets everything up

```bash
seldon init --path ~/Seldon --obsidian --harness claude-code --theme-hook
```

The wizard asks for the rest. The answers from the G3 run:

| Question | Answer |
|---|---|
| Language of the logbook prose | your language (e.g. Deutsch) |
| Collectors | all (snapper, pacman, omarchy, plugins, theme, config) |
| Watched config paths | the defaults: `~/.config/hypr`, `~/.config/omarchy`, `~/.config/waybar`, `~/.bashrc`, `~/.zshrc` |
| More paths | none |
| Make the logbook a git repository with a first commit? | yes |
| Backfill since | a few days back (`YYYY-MM-DD`), or empty to record from now on |
| Mark them as the pre-Seldon baseline? | yes (asked only after a backfill) |

`--harness claude-code` and `--theme-hook` answer the harness and theme hook
questions; `--obsidian` adds `.obsidian/`.

Without questions, the same in one line:

```bash
seldon init --non-interactive --path ~/Seldon --language de --obsidian \
  --harness claude-code --theme-hook --since "$(date -d '-3 days' +%F)" --baseline
```

`init` then does all of this itself, in order. You do not run any of it by
hand:

1. writes the logbook and `~/.config/seldon/config.toml`;
2. merges the Claude Code hooks into `~/Seldon/.claude/settings.json`.
   No separate `seldon hook install claude-code` is needed;
3. creates the git repository with the first commit;
4. runs the first capture (`capture --all`, backfilled with `--since`);
5. dismisses the backfill's drift as the pre-Seldon baseline when you
   said yes. The events stay in the ledger;
6. fills `system/*.md` once (`seldon dossier`);
7. installs the theme hook with `omarchy hook install theme-set`. The
   engine records the hook file as its own write. The next capture
   explains it, so it never shows up as drift (SPEC-ENGINE §5 rule 7).

Expected output (G3 run, abridged):

```
Logbook created at ~/Seldon (machine <machine>, language de, 33 files).
Harness claude-code: .claude/settings.json: 3 hook(s) added, 0 already there
Git: repository initialised, first commit "seldon: init logbook"
Snapper: degraded — No permissions (ADR-0011)
First capture: 1230 event(s) since …; 22 drift item(s) dismissed as "pre-Seldon baseline"; 0 open drift item(s), 0 crisis
Dossier: Wrote system/hardware.md, system/omarchy.md, system/packages.md, system/plugins.md, system/services.md (…)
Theme hook: installed (~/.config/omarchy/hooks/theme-set.d/seldon-theme-set.sh)
Obsidian: open the folder as a vault.
```

## 3. Doctor, and snapshots (optional)

```bash
seldon doctor                     # everything ok; snapper "degraded" is fine (ADR-0011)
```

Omarchy does not let a user list snapper snapshots by default. The
timeline works without them. If you want them, run this yourself. It is
your decision, not the engine's (ADR-0011). It changes the root snapper
config:

```bash
sudo snapper -c root set-config ALLOW_USERS=$USER SYNC_ACL=yes
seldon doctor                     # snapper: ok, N snapshots (config root)
```

## 4. The first capture after `init`

```bash
seldon capture --all
#   Captured 1 new event(s).
#     snapper 0 · pacman 0 · omarchy 0 · plugins 0 · theme 0 · config 1   (one line per collector)
#   note: 1 config event(s) explained as written by seldon itself
#   (more events appear when the machine changed between init and capture)
seldon drift                      # 0 open drift item(s)
```

The one event is the `config-add` of the theme hook script. It stays in
the ledger, explained as "installed by seldon init --theme-hook". Without
`--theme-hook` the capture writes nothing.

## 5. A real case with Claude Code

```bash
seldon plan new --zone yellow --risk R1 --area themes -- "Phase 0 exit: first real case"
seldon plan start C-2026-001      # the id plan new printed
cd ~/Seldon && claude             # accept the workspace trust prompt: the Seldon hooks live in .claude/settings.json
```

Claude Code reads the logbook's `AGENTS.md` by itself. In G3 it filled the
case's Intent and Plan, did the work, and moved the case to verification
on its own (finding F3). Give it a task you can undo. Two examples:

- **Theme round trip** (the G3 case). Tell it: *"Work case C-2026-001:
  note the current theme (`omarchy theme current`), switch to two other
  themes with `omarchy theme set <name>`, then switch back to the original
  one. Then move the case to verification."* Each switch becomes a
  `theme-set` event with the case and `agent:claude-code`, recorded by the
  theme hook the moment it happens.
- **A Hyprland config edit.** Omarchy 4 keeps the Hyprland config as Lua
  files: `~/.config/hypr/hyprland.lua`, `bindings.lua`, `looknfeel.lua`,
  `input.lua`, `monitors.lua`, `autostart.lua`. There is no
  `hyprland.conf` to edit. Tell it: *"Work case C-2026-001: append the
  comment line `-- seldon phase-0 test` to ~/.config/hypr/looknfeel.lua."*
  Run `seldon capture --all` in another terminal now, before the revert:
  you get a `config-change` attributed to the case through the Edit hook
  (after the revert the file is unchanged again, and a capture would see
  nothing). Then: *"Remove that line again and move the case to
  verification."* A Lua comment changes nothing on screen, and the file
  ends as it began.

Leave Claude Code with `/exit`. The `SessionEnd` hook adds a line to
today's journal, runs `capture --all` and commits.

## 6. Capture, status, drift, close

```bash
seldon capture --all
seldon status                     # cases 0 active · 1 in verification; drift 0 open
seldon drift                      # anything the case did not cover
seldon plan verify C-2026-001     # only if Claude Code did not move it already
seldon plan done C-2026-001
```

If `seldon drift` lists something, resolve it, one item per id:

```bash
seldon drift link <EVENT> C-2026-001                 # it belongs to the case
seldon drift explain <EVENT> -- "<why it happened>"  # creates a completed retroactive case
seldon drift dismiss <EVENT> -- "<reason>"           # noise
```

## 7. Look

```bash
cat ~/Seldon/STATUS.md            # 0 active cases, 0 open drift, today's events
git -C ~/Seldon log --oneline     # every engine step is a commit
obsidian ~/Seldon                 # or open the folder as a vault from Obsidian's picker
```

With the bar plugin enabled, the pill shows the same counts as STATUS.md.

## 8. Report

Paste the terminal output, without private paths and host names
(`<user>`, `<machine>`), into `work/completed/PHASE-0-EXIT-transcript.md`.
Or tell the orchestrator and it writes the file from your summary.

## 9. Start over (only if you want to repeat this)

These commands delete what `init` created: the logbook, the engine's
config and state, and the theme hook. Nothing else on the system depends
on them.

```bash
rm ~/.config/omarchy/hooks/theme-set.d/seldon-theme-set.sh
rm -r ~/Seldon ~/.config/seldon ~/.local/state/seldon
```

Known limits (not blockers):
- A backfill opens every older package change as drift, mostly as
  crises. The baseline question handles that (WP-013 FINDINGS §2.2).
- Snapper stays degraded until step 3's opt-in.
- Without the theme hook, a theme switch reaches the ledger only with the
  next capture, through the theme collector.
