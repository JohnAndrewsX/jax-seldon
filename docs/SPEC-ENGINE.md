# SPEC-ENGINE.md — `seldon`, the engine

Normative. Rust crate in `engine/`, binary `seldon`.

## 1. Principles

1. Single static binary, no runtime deps, no network, no async.
2. Only writer of the logbook and the index. Everything else reads.
3. Idempotent collectors with persistent cursors.
4. Every command has `--json`; human output is for terminals, JSON is for
   agents and the plugin.
5. Fast: `status` < 100 ms at 10 000 events; `hook` < 5 ms.
6. Never executes system changes. It may *print* commands.

## 2. Files the engine owns

| Path | Purpose |
|---|---|
| `~/.config/seldon/config.toml` | logbook path, language, collectors on/off, watched config paths, agent names, git.autocommit, redaction extras |
| `~/.local/state/seldon/index.json` | the contract output (see CONTRACT.md) |
| `~/.local/state/seldon/cursors.json` | per-collector cursors (pacman byte offset + inode, last snapper id, last plugin-list hash, config manifest hash) |
| `~/.local/state/seldon/manifest.json` | path → sha256 for watched config files |
| `~/.local/state/seldon/lock` | flock during writes |
| `<logbook>/.seldon/` | logbook.toml, active-case, templates/ |

## 3. Commands

```
seldon init [--path DIR] [--non-interactive] [--language de|en] [--obsidian]
            [--harness claude-code] [--git/--no-git]
seldon capture [--source pacman,snapper,omarchy,plugins,theme,config | --all] [--since TS]
seldon log "<text>" [--case ID] [--actor human|agent:NAME] [--tag T]
seldon event <source> <kind> --subject S [--detail D] [--case ID] [--actor A] [--meta k=v]
seldon plan new "<title>" [--zone Z] [--risk R] [--area A] [--priority P]
seldon plan start|verify|done|drop <ID> [--snapshot N] [--reason TEXT]
seldon plan list [--status S] [--area A]
seldon plan show <ID>
seldon drift [--crisis-only]
seldon drift link <EVENT> <CASE>
seldon drift explain <EVENT> "<intent>" [--zone --risk --area]
seldon drift dismiss <EVENT> --reason TEXT
seldon decide "<title>" [--case ID]            # creates ADR, opens $EDITOR unless --no-edit
seldon status                                  # regenerates STATUS.md + index
seldon index [--check]                         # rebuild index; --check validates against schema
seldon dossier [--section packages|services|omarchy|plugins|deviations|all]
seldon rebuild                                 # outputs/REBUILD.md
seldon update-impact [--target VERSION]        # outputs/UPDATE-IMPACT.md  (Phase 3)
seldon hook install <claude-code|generic> [--settings PATH]
seldon hook claude-code                        # stdin: Claude Code hook JSON
seldon hook generic                            # stdin: {"command":"…","actor":"…","cwd":"…"}
seldon hook session-start | session-stop       # context print / journal stub
seldon watch [--interval SECS]                 # feature "watch", optional
seldon doctor                                  # engine, config, logbook, omarchy, snapper, git checks
seldon open <case|journal|ledger|status>       # prints path; with --editor launches $EDITOR
seldon --version / seldon contract-version
```

Global flags: `--json`, `--logbook DIR` (overrides config and
`SELDON_LOGBOOK`), `--quiet`, `--no-commit`.

Exit codes: 0 ok · 1 user error (bad args, unknown case) · 2 engine error ·
3 logbook not initialised · 4 lock held. Argument-parse errors are user
errors (1), never 2; `--help` exits 0.

JSON shapes of the always-available commands (not part of `schema/`, so no
`contractVersion` bump when they change; the plugin's engine detection
parses them):

```
seldon --version --json          → {"name":"seldon","version":"0.1.0"}
seldon contract-version --json   → {"contractVersion":1}
any user error with --json       → {"error":{"code":1,"message":"<detail>"}}  (exit 1)
```

`message` carries the full detail (e.g. the unrecognised subcommand name),
not just the error kind. Detection of `--json` must not sniff raw argv for
the literal string, because free-text arguments (`seldon log "--json"`)
may contain it.

## 4. Collectors

Each collector implements `fn collect(ctx) -> Vec<Event>` and
`fn cursor(&self) -> Cursor`. Run order: snapper, pacman, omarchy, plugins,
theme, config. Rules:

- **pacman** — parse `/var/log/pacman.log` from the saved byte offset; verify
  inode; on rotation restart from 0 and dedupe by `(ts, kind, subject,
  version)`. Lines `[ALPM] installed|removed|upgraded|downgraded|reinstalled`.
  Transaction grouping: lines between `transaction started` and
  `transaction completed` share a `txId`; the `[PACMAN] Running 'pacman -S
  zed'` line gives `meta.command`; packages in the command are `explicit`,
  others in the same transaction are `dependency` and inherit the case of
  the explicit ones.
- **snapper** — `snapper --jsonout list`. New snapshot numbers become
  `snapshot` events with description; a `pre`/`post` pair is linked via
  `meta.pairOf`. Without `ALLOW_USERS` the command fails with a permission
  error; the collector then reports `ok: false` and the fix command, never
  sudo (ADR-0011).
- **omarchy** — version from `omarchy --version` (or
  `~/.local/share/omarchy/version`); git HEAD of the omarchy repo; change
  → `update` event with `from`/`to`.
- **plugins** — `omarchy plugin list --json`; diff against last snapshot
  (stored by hash) → `plugin-add|plugin-remove|plugin-enable|plugin-disable
  |plugin-update` with id and version.
- **theme** — current theme name (read how Omarchy stores it from
  `~/.local/share/omarchy/bin/omarchy-theme-set`); diff → `theme-set`.
  Optional hook script installed by the wizard into `theme-set.d/` calls
  `seldon event theme theme-set --subject "$1"`.
- **config** — sha256 manifest of files under `watchPaths` (default
  `~/.config/hypr`, `~/.config/omarchy` excluding `plugins/`, `~/.config/
  waybar` if present, `~/.bashrc`, `~/.zshrc`, user list). Changed/added/
  removed → `config-change` with path and both hashes. Binary files and
  files > 1 MB are skipped. Known secret-bearing files are listed in
  `config.toml [redaction] skipPaths` and never hashed.

All events get `actor: system` unless the collector can prove otherwise
(pacman's command line inside an agent-tagged time window is *not* proof;
only hooks set agent actors).

## 5. Reconciliation (drift)

After every capture:

1. For each new event without `case`: if `actor` is an agent and
   `.seldon/active-case` was set at `ts` (the hook writes the case into the
   event directly, so this only covers collector-found events), link.
2. If the event is a pacman `dependency` of an explicit event with a case,
   link.
3. If an active case lists the event's subject in *Affected paths* or the
   package name in *Steps*, propose (not link) — stored as `proposedCase`
   in the index for one-click confirmation.
4. Otherwise it is drift. Red zone → `crisis: true`.

Resolution events (`kind: resolution`, `refersTo`) are applied when the
index is built; an event with a resolution is not drift.

## 6. Index build

`seldon index` reads: all `ledger/*.jsonl`, all case files, journal file
for today and yesterday, `decisions/*.md` frontmatter, `system/*.md`
generated fences, `memory/*.md` frontmatter + first 20 lines, `areas/*/`.
Produces `index.json` per CONTRACT.md. Writes atomically (temp + rename).
`STATUS.md` is rendered from the same data.

Performance budget: 10 000 events, 300 cases, 365 journal files → < 100 ms
warm. Measured in `cargo bench` with the fixture logbook scaled ×10.

## 7. Redaction

Before writing any event, `meta.command` and `detail` are passed through
redaction: `--password`, `token=`, `Authorization:`, `AKIA[0-9A-Z]{16}`,
`ghp_[A-Za-z0-9]{36}`, `sk-[A-Za-z0-9]{20,}`, anything after `-p ` for
`mysql|psql|smbclient`, URLs with userinfo, and user-supplied patterns in
`config.toml [redaction] patterns`. Replacement: `‹redacted›`. The hook
never records stdin/stdout of commands, only the command line.

## 8. Hooks

`seldon hook claude-code` reads the PostToolUse JSON from stdin, extracts
`tool_input.command`, classifies with the same rules as the pacman command
parser plus `omarchy (pkg|plugin|theme|update|install)`, `systemctl
(enable|disable|start|stop|mask)`, `cp|mv|install|tee|sed -i` targeting a
watched path, `git` inside `~/.config` or the logbook. Non-mutating commands
produce no event. Output: nothing on stdout (hooks must stay silent), exit
0 always (a logging failure must never block an agent).

`seldon hook session-start` prints a compact context block to stdout:
STATUS summary, active case (id, title, plan steps), last 5 journal lines,
`memory/lessons.md` headings. Claude Code injects it as context.

`seldon hook session-stop` appends `## HH:MM · agent:NAME · CASE` with
"session ended; N events recorded" to today's journal, runs `capture --all`,
`status`, and commits.

## 9. Wizard (`seldon init`)

Interactive via `dialoguer` (no `gum` dependency; gum is optional eye candy
later). Steps: path (the options of ADR-0010: `~/Seldon`,
`~/Documents/Seldon`, a detected project folder, custom) → language → Obsidian config yes/no → collectors
(all on by default) → watched config paths (defaults shown) → agent
harnesses (Claude Code hooks; optional Omarchy-Agent kit guard/skills if
present as a template dir) → git init + first commit → run first capture →
print next steps. `--non-interactive` takes all defaults and flags.

The wizard writes templates from `engine/templates/{en,de}/` into the
logbook: `AGENTS.md`, `PROJECT.md`, `DECISIONS.md`, `areas/*/README.md`
for the default areas (`hyprland`, `themes`, `packages`, `dev-env`,
`plugins`, `shell`), `memory/lessons.md`, `system/*.md` skeletons.

## 10. Testing

- Unit: parsers (pacman log fixtures incl. rotation and malformed lines),
  redaction, case state machine, frontmatter round-trip.
- Golden: `fixtures/logbook/` → `seldon index` → compare with
  `fixtures/index.sample.json` (which the plugin uses). Any drift between
  them fails CI.
- Schema: every emitted JSON validated with `jsonschema` in tests.
- Idempotency: run every collector twice on the same fixtures; second run
  yields zero events.
- Bench: index build on scaled fixtures, asserted < 100 ms in CI (release).
