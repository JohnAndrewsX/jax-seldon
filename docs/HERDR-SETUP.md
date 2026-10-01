# HERDR-SETUP.md — Running the Seldon team in Herdr

Companion to ORCHESTRATION.md. That file says *what* the orchestrator does;
this one says how to start it on the dev host. Machine names and private
paths are in `memory/local.md` (git-ignored).

## 1. One-time preparation (done 2026-10-01, see memory/host.md)

- Repo cloned, first commit pushed, GitHub remote set.
- Rust toolchain (rustup, musl target), `just`, `qmllint` verified.
- Test host reachable over SSH; it has cargo (glibc only) and `qmllint`.
- Herdr 0.9.1 with the Claude Code integration installed (`herdr integration status`).
- Repo harness: `.claude/settings.json` (deny list for the red zone,
  allow list for the usual tools, the guard hook) and `scripts/guard.sh`.
  The hook runs before every Bash call, in every permission mode, and
  blocks `sudo`, `pacman`, `yay`, `paru`, `systemctl` changes, mutating
  `omarchy` commands, writes under `/etc`, and writes under `~/.config`
  outside the plugin dev install. Worktrees inherit it.

Still manual, interactive (the operator):

```bash
# optional: register the test host as a Herdr machine (remote runs 0.8.2;
# Herdr asks for approval to update it)
herdr machine add <ssh-target> --label test
```

## 2. Starting the session

```bash
cd <repo>
herdr --session seldon          # or the workspace you already have
claude --model fable --effort high   # plus the permission mode you choose
```

The permission mode is the operator's call and is typed by the operator;
the orchestrator passes the same choice to its workers after `--`. The
guard hook (§1) applies in every mode.

Suggested layout inside the session — one workspace `seldon`, tabs:

| Tab | Agent / use | cwd |
|---|---|---|
| `orchestrator` | `claude` (long-running) | repo root |
| `engine` | worker started by the orchestrator | `wt/WP-NNN` |
| `plugin` | worker started by the orchestrator | `wt/WP-NNN` |
| `reviewer` | `claude` or `codex`, invoked per handover | repo root |
| `shell` | plain terminal for `just check`, `omarchy-shell` IPC, ssh to the test host | repo root |

Worktrees are created by the orchestrator with Herdr's helper, which also
opens them as a workspace:

```bash
herdr worktree create --cwd <repo> --branch wp/001-scaffold --path wt/WP-001 --label WP-001 --no-focus
```

`wt/` is git-ignored. Workers are started in a pane of that workspace:

```bash
herdr agent start engine-001 --kind claude --pane <pane-id> -- --model opus --effort high
herdr agent prompt engine-001 "<brief from ORCHESTRATION.md §3>" --wait --timeout 1800000
```

## 3. Orchestrator kickoff prompt

Paste this as the first prompt of the `orchestrator` agent:

```
You are the Seldon orchestrator (docs/ORCHESTRATION.md). Read, in order:
AGENTS.md, PROJECT.md, STATUS.md, docs/PLAN.md, docs/ORCHESTRATION.md
(all sections, especially 10-12), memory/host.md, memory/omarchy-shell.md,
memory/local.md.

You run on autopilot until the Phase 0 exit (G3). Kickoff checklist §9:
steps 1-3 are done. Continue with step 4: assign WP-001 (Scaffold) and
WP-002 (Schema Keeper) in parallel, each in its own worktree and Herdr
workspace, with the brief format of §3 and the model matrix of §12. Start
workers with the same permission mode this session was started with.

Decisions are settled by debate (§10); you may use a workflow for the
debate and for review fan-outs. After the first tick, start the /loop
skill in self-paced mode with the instruction "run the orchestration loop
(ORCHESTRATION.md §2)" and keep it running. Check the usage budget (§12)
every tick. Log every tick in work/ORCHESTRATOR-LOG.md.

Report to the operator in their language (German) only when a WP
completes, a WP blocks, a §10 escalation is needed, or the phase exits.
Never run anything red-zone (AGENTS.md §6).
```

## 4. Catching up as the operator

- `work/ORCHESTRATOR-LOG.md` — one line per tick.
- `STATUS.md` — active, queued, blocked, decisions.
- `herdr agent list` and the workspace sidebar — who is doing what.
- The orchestrator pane — questions addressed to you, in German.

## 5. Testing on the test host

The installable plugin repo (`jax-seldon-plugin`, ADR-0009) exists only
from the first release on. Until then, test by copying:

```bash
rsync -a --delete plugin/ test:~/.config/omarchy/plugins/jax.seldon/
ssh test 'omarchy plugin validate ~/.config/omarchy/plugins/jax.seldon && omarchy-shell shell rescanPlugins && omarchy plugin enable jax.seldon'
```

The engine builds on the test host with its pacman Rust (glibc), or copy
the static musl binary from the dev host:

```bash
just build-release
scp engine/target/x86_64-unknown-linux-musl/release/seldon test:~/.local/bin/
```

The dev host's own bar is also a test target (`~/.config/omarchy/plugins/`
is empty; the plugin dir is the only path under `~/.config` the workers may
write, AGENTS.md §6).
