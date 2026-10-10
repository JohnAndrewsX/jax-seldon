# CONCEPT.md — Seldon, end to end

## The problem

An Omarchy developer machine changes every day: packages, dotfiles, themes,
shell plugins, Hyprland rules, agent harnesses. Several agents and one human
make those changes. Three months in, nobody can answer "why is this here?",
"what did the agent do on Tuesday?", or "which of my overrides will the next
Omarchy update break?". Hand-written changelogs die after two weeks. System
logs record facts without intent.

## The idea in one picture

```
   planned ──────────────┐                       ┌────────── observed
   work/ (cases)         │                       │   ledger/ (events)
   "install Zed, R1"     │        reconcile      │   pacman: +zed 0.198
   "switch theme, R1"    ├──────── (engine) ─────┤   theme-set: tokyo-night
                         │                       │   config: hyprland.conf
                         │                       │   pacman: +ollama   ← no case
                         ▼                       ▼
                   matched: trace ◄──────────► unmatched: DRIFT
                   "C-2026-004 done,              "ollama installed by
                    verified, snapshot 112"        agent:codex, 14:02 —
                                                   link / explain / dismiss"
```

Seldon captures what happened (the **ledger**), records what was meant to
happen (**cases** in `work/`), and shows the difference (**drift**). The
narrative — why, what was learned — lives in the **journal**. Decisions get
**ADRs**. The machine's documented state is the **dossier** (`system/`).
What agents learn goes to **memory**.

## Three layers

| Layer | Role | Writes | Reads |
|---|---|---|---|
| **Logbook** (`~/Seldon`) | the record; Markdown + JSONL; git repo; Obsidian vault | — | humans, agents, engine |
| **Engine** (`seldon`, Rust) | the only writer; wizard, collectors, commands, index, hooks | logbook, index, STATUS.md | logs, omarchy CLI, logbook |
| **Plugin** (`jax.seldon`, QML) | the face; bar pill, panel, Prime Radiant | nothing (calls engine) | `index.json` |

The engine's `index.json` is a cache; delete it and `seldon index` rebuilds
it. The plugin never touches Markdown. Agents never touch the index.

## Ledger and journal, and why both

**Ledger** — automatic, complete, boring. Sources in v1:

| Source | What becomes an event | How |
|---|---|---|
| pacman | install, remove, upgrade, downgrade | parse `/var/log/pacman.log` from a saved cursor |
| snapper | snapshot created/deleted | `snapper --jsonout list` or Omarchy's wrapper |
| omarchy | version change, `omarchy-update` run | `omarchy --version`, update log, repo HEAD |
| plugins | plugin added/removed/enabled/disabled/updated | diff `omarchy plugin list --json` snapshots |
| theme | theme switched | diff current theme; optional `theme-set.d` hook calls `seldon event theme` |
| config | file changed under watched paths (`~/.config/hypr`, `~/.config/omarchy`, user-defined) | hash manifest diff |
| agent | mutating command run by an agent | hooks (`seldon hook claude-code`) |
| manual | `seldon log`, `seldon event` | user or agent |

**Journal** — written, narrative, in the user's language. Daily notes
`journal/YYYY/YYYY-MM-DD.md`. An agent session ends with a journal entry;
the human adds the why. Obsidian daily-notes compatible.

A ledger without a journal is a syslog. A journal without a ledger is
fiction. Seldon needs both, and keeps them linked by case IDs and event IDs.

## Drift and Crisis

Every change is recorded. Whether a change without a case also **needs
attention** is decided by what a wrong one would cost (ADR-0028), not by
who made it:

- **Routine** — history, not drift: a theme switch, a plugin toggle, a
  plain full upgrade (`pacman -Syu`, `omarchy update`, kernels included),
  Omarchy's own copy of a file. It shows in the Changelog; nobody is asked
  for a reason.
- **Attention** — quiet drift: a package installed or removed by name, a
  third-party plugin added or updated, an override under a watched path.
  The panel lists it, the bar does not count it, nothing is asked. An
  agent explains it only with evidence: its own Log, a hook event or the
  user's words.
- **Crisis** — it can break boot, login, the shell or security, and nobody
  asked for it in a case: an `alwaysRed` package installed or removed by
  name, a new file in a persistence path (`~/.config/systemd/user`,
  Omarchy's hooks, autostart). The only thing that colours the bar; the
  user is told once and never has to act. An agent may not explain or
  dismiss a crisis.

`seldon drift` lists attention and crises (`--all` adds routine). Each can
be:

- **linked** to an existing case (`seldon drift link <event> <case>`),
- **explained** — Seldon creates a retroactive case from the event
  (`seldon drift explain <event> -- "wanted ollama for local models"`),
- **dismissed** (`seldon drift dismiss <event> -- "dependency pull"`).

The user may, never must. Dependencies pulled in by an explicit install
are auto-linked to that install's case (pacman's log tells us). The
session-start context lists the crises and attention items of the last
seven days for every agent session. `config.toml [drift] attention =
"all"` restores the louder picture (every change without a case is drift,
crisis iff red zone).

## Cases: planning that agents can follow

A case is one Markdown file in `work/queued|active|completed/` with
frontmatter (id, status, zone, risk, area, snapshot, agents, events) and
four sections: *Intent*, *Plan* (steps, affected paths, rollback,
verification), *Log* (append-only), *Result*. The template is the
Omarchy-Agent kit's case, extended with the fields Seldon fills in
automatically: linked events, agent sessions, snapshot before, verification
evidence.

Agents get a case the same way humans do: `seldon plan start C-2026-004`
writes `.seldon/active-case` in the logbook, the hook reads it, every event
the agent causes carries the ID. When the agent is done: `seldon plan verify`
and `seldon plan done`, which also moves the file and writes the journal
stub. The result is a **trace** — an ordered list of what the agent actually
did, next to what it was asked to do.

## The plugin surfaces

Two surfaces from 0.2.0 (ADR-0034; the 0.1 bar popup and the fullscreen
overlay are gone):

- **Bar pill** — the Seldon mark, then `2 · 1`: active cases, crises
  (hidden at 0; `driftInBar` shows all changes without a case or none).
  Colour by worst state: accent while cases are active, urgent on a
  crisis. Left click → the desk, middle click → the desk at the Prime
  Radiant, right click → capture. `SUPER+SHIFT+L` toggles the desk
  (suggested, user-configurable).
- **The desk** — one window over the desktop, on the focused monitor, 50
  to 100 % of the screen wide (`deskWidth`, default 100). A header with
  the mark, the machine, the status chip and the counts (active,
  verification, queued, crises, attention); notices with their one-click
  fixes; a sidebar with nine sections and a search; a list and a detail
  with the actions. The sections, each on a key:
  - *1 Today* — the day: what needs you (the crises), the journal and the
    note field, the active cases as tiles, a new case from one sentence;
    until Seldon is set up, the setup card.
  - *2 Changelog* — the ledger's events by day with the chips open,
    crisis, attention, routine, in case, all; resolve a change (link,
    explain, dismiss), or let the agent propose it for all open changes
    and apply the proposal yourself (ADR-0036).
  - *3 Work* — the cases by status; start, hand to an agent, verify,
    complete, drop, reopen; import tasks.
  - *4 Decisions*, *5 System* (the dossier as tiles), *6 Memory*.
  - *7 Prime Radiant* — the picture: activity heatmap, package count
    series, drift curve, risk distribution, timeline of Omarchy releases,
    snapshots and crises, and the current Plan (active cases with
    progress) over 30 d, 90 d, 365 d or all. This is the screenshot people
    share.
  - *8 Graph* — areas, cases, decisions and changes as a graph that grows
    day by day.
  - *Settings* (`,`) — the desk's width and sidebar; what is in force and
    where it is set.
  The desk only reads `index.json` and asks the engine for every write;
  writing actions arm on the first press and run on the second.
- **Empty and error states** — engine missing, logbook not initialised,
  index missing, index stale, a contract mismatch — each a notice with
  one command and one button, named in the header's chip; until Seldon is
  set up, one setup card on Today takes their place.

## Agents and the logbook

The logbook is a Hermes-style project. Its `AGENTS.md` carries the rules
(zones, gates, language, "never execute red-zone commands"); any agent that
reads AGENTS.md can work there. Harnesses live inside and are excluded from
Obsidian: `.claude/` (hooks, skills, commands — the Omarchy-Agent kit's
guard and skills can be installed here by the wizard), `.codex/`, `.seldon/`
(active-case marker, cursors are *not* here — they live in state).

Hook lifecycle for Claude Code (installed by `seldon hook install claude-code`):

| Hook | Does |
|---|---|
| `SessionStart` | prints `STATUS.md` + active case summary as context |
| `PreToolUse` (Bash, Edit, Write, MultiEdit) | `seldon hook claude-code` parses the command (or the edited file) *before* it runs, so the event carries the start time (ADR-0017), and writes an `agent` event if it changes the system (pacman/yay/omarchy/systemctl, writes into watched paths), tagged `actor: agent:claude-code`, `case: <active>` |
| `SessionEnd` | appends a journal stub for the session, runs `seldon capture --all`, commits the logbook (`Stop` would fire after every reply) |

Codex and the Omarchy default agent use the generic form: any program can
pipe `{"command": "...", "actor": "agent:x", "cwd": "..."}` into `seldon hook
generic` before the command runs (SPEC-ENGINE §8).

## Outputs that make the system reproducible

- `seldon rebuild` → `outputs/REBUILD.md`: explicit packages (repo and AUR
  separated, with the case that introduced each), deviations from Omarchy
  defaults with their reasons, enabled plugins, theme, user services. A
  fresh install can be walked through it. Later: a generated script.
- `seldon update-impact` → compares the Omarchy release notes between the
  installed and the available version against `system/deviations.md` and
  lists overrides that touch changed areas. Runs before `omarchy-update`.

## What Seldon deliberately is not

Not an executor (it never changes the system). Not a sandbox (that is the
harness's job). Not cloud. Not an Obsidian plugin. Not a general PKM tool.

## Glossary

| Term | Meaning |
|---|---|
| Logbook | the project folder, `~/Seldon` by default |
| Ledger | machine-captured events, `ledger/*.jsonl` + generated `.md` view |
| Journal | written daily notes |
| Case | a planned unit of change, `C-YYYY-NNN`, lives in `work/` |
| Plan | all open cases (Foundation flavour, used in the UI) |
| Drift | an event with no case and no resolution that is worth a look (ADR-0028: attention or crisis; routine is history) |
| Crisis | a change without a case that can break boot, login, the shell or security (ADR-0028) |
| Trace | the ordered events and sessions linked to a case |
| Dossier | `system/` — documented actual state |
| Memory | `memory/` — what agents learned about this machine |
| Desk | the plugin's one window from 0.2.0: sidebar, list, detail (ADR-0034) |
| Prime Radiant | the desk's section 7, the charts (the full-screen overlay until 0.1.x) |
| Index | `~/.local/state/seldon/index.json`, the plugin's only input |
