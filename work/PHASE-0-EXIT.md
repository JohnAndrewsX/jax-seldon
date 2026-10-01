# Phase 0 exit (G3) — procedure for the operator

PLAN.md: *"The operator runs `seldon init` on their machine, works one real
case with Claude Code, sees events, drift and a STATUS.md. Logbook opens in
Obsidian."* The orchestrator cannot do this: `seldon init` writes
`~/.config/seldon/config.toml` and the real logbook (AGENTS.md §6). The
transcript goes to `work/completed/PHASE-0-EXIT-transcript.md`.

Prerequisite: WP-008 merged (`just check` green on main).

```bash
cd ~/Work/johnandrewsx/jax-seldon
just build-release                                  # static musl binary
install -Dm755 engine/target/x86_64-unknown-linux-musl/release/seldon ~/.local/bin/seldon
seldon --version                                    # seldon 0.1.0

# 1. init (interactive wizard; or add --non-interactive for the defaults, ADR-0010)
seldon init --path ~/Seldon --obsidian --harness claude-code
seldon doctor                                       # all ok, snapper degraded is fine (ADR-0011)

# 2. a real case
seldon plan new --zone yellow --risk R1 --area dev-env -- "Phase 0 exit: first real case"
seldon plan start C-2026-001
seldon hook install claude-code                     # writes ~/Seldon/.claude/settings.json
cd ~/Seldon && claude                               # work the case: let Claude run a few commands
#   e.g. ask it to `omarchy theme set <some theme>` and set it back, or edit a file
#   under ~/.config/hypr — every mutating command becomes an agent/command event
seldon capture --all                                # collectors: pacman, snapper, omarchy, plugins, theme, config
seldon status                                       # STATUS.md + index.json
seldon drift                                        # anything the case did not cover
seldon plan verify C-2026-001 && seldon plan done C-2026-001

# 3. look
cat ~/Seldon/STATUS.md
obsidian ~/Seldon                                   # or open the vault from Obsidian's picker
```

Paste the terminal output (minus private paths) into
`work/completed/PHASE-0-EXIT-transcript.md`, or tell the orchestrator
and it writes the file from your summary. Then the loop stops and reports;
the operator restarts it for Phase 1 (ORCHESTRATION.md §11).

Known limits at this point (not blockers): `seldon init --json` still
prints the stale "no collectors" capture note (WP-024 wires the first
capture into the wizard); a backfill with `capture --since` turns past
package changes into drift by design (FINDINGS of WP-013).
