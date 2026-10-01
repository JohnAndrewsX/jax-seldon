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
| `~/.config/seldon/config.toml` | keys (WP-003): `logbook`, `language`, `watchPaths`, `harnesses`; `[collectors] snapper|pacman|omarchy|plugins|theme|config` (bool); `[git] autocommit`; `[redaction] patterns, skipPaths`; `[drift] alwaysRed` (ADR-0013). Unknown keys survive a save; comments and key order do not (toml crate; the header says so). Precedence for the logbook path: `--logbook` > `SELDON_LOGBOOK` > config > `~/Seldon`. A global `--config FILE` / `SELDON_CONFIG` override lands in WP-006 so tests and the test host never touch the real file |
| `~/.local/state/seldon/index.json` | the contract output (see CONTRACT.md) |
| `~/.local/state/seldon/cursors.json` | `{logbook, collectors: {name: {cursor, ok, message, fix, lastRun, events}}}`, bound to the canonical logbook path (another logbook re-baselines every collector). Cursors: pacman byte offset + inode; snapper = the set of known snapshots (number, type, description — a delete event needs what was deleted); omarchy = last version; plugins = last list hash + versions; config = manifest hash. `index.state.collectors` is derived from `ok`/`message`/`fix`/`lastRun` (WP-007) |
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
seldon doctor --json             → {"ok":bool,"logbook":"<path>",
                                     "checks":[{"name","status":"ok|degraded|error","message","fix"?}]}
                                    exit 0 (no error), 1 (a check is error), 3 (not initialised)
```

`doctor --path DIR` is an alias of the global `--logbook DIR`. The plugin's
banner states parse the doctor shape; it is not part of `schema/`.

```
seldon capture --json  → {"ok":true,"logbook":"<path>","written":N,"files":["ledger/2026-10.jsonl"],
                          "collectors":[{"name","enabled","ran","ok","events","message"?,"fix"?}]}
                         exit 0 also when a collector is degraded (ok:false + fix, ADR-0011);
                         1 unknown source or --source with --all; 3 not initialised; 4 lock held
```

`capture` selection: no flag or `--all` = every collector enabled in
`config.toml [collectors]`; `--source a,b` = exactly those, even if disabled.
Baseline: a collector without a cursor emits only events at or after the
logbook's `created` (or `--since TS`); diff collectors record their first
state silently. `--since` has no effect on a collector that already has a
cursor (one notice line in human output). Dedupe against the ledger runs on
every capture, not only after a rotation, so a lost `cursors.json` never
duplicates events. `capture` does not commit, reconcile or rebuild the
index by itself until WP-006/007/008 wire those steps in.

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
  the explicit ones. A transaction is emitted only after `transaction
  completed`, the next `transaction started`, or when
  `/var/lib/pacman/db.lck` is absent at capture time; until then the
  cursor stays at the transaction's `[PACMAN] Running` line, else
  `transaction started` (ADR-0013 §5). `meta.command` is parsed as argv,
  never matched as a substring; the parser (`command_intent`,
  `parse_command`, `is_plain_full_upgrade`) is shared with the hook (§8)
  and the drift routine rule (§5). Rotation: the tail of `<log>.1` with the
  old inode is read first, then the new file from 0; a rotation to another
  name loses the lines between the old offset and the rotation (never
  duplicates, thanks to dedupe). Attribution follows ADR-0014 §1 as
  sharpened by ADR-0017 §2–§5.
- **snapper** — `snapper --jsonout list`. New snapshot numbers become
  `snapshot` events with description; a `pre`/`post` pair is linked via
  `meta.pairOf`. Without `ALLOW_USERS` the command fails with a permission
  error; the collector then reports `ok: false` and the fix command, never
  sudo (ADR-0011).
- **omarchy** — version from `omarchy-version` (prints e.g. `4.0.4-1`;
  `omarchy --version` does not exist and `$OMARCHY_PATH/version` is
  stale); `repoHead` only when `$OMARCHY_PATH` is a git checkout, omitted
  on package installs; change → `update` event with `from`/`to`.
- **plugins** — `omarchy plugin list --json` (shell IPC; fails when the
  shell is not running → `ok: false`); diff against last snapshot (stored
  by hash) → `plugin-add|plugin-remove|plugin-enable|plugin-disable
  |plugin-update` with id and version. Neither `list` nor `catalog`
  carries a version: read it from the manifest at the plugin's
  `manifestPath`, else the plugin directory's git HEAD short hash
  (ADR-0014 §3).
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

All events get `actor: system` unless the collector can prove otherwise.
Proof is an agent hook `command` event that (a) named the subject
(package, path, or a full upgrade — `-Syu`/`-Su`/`omarchy update` — for
every member of the resulting transaction) and (b) precedes the collector
event within the same pacman transaction or by at most 10 minutes; then
the collector event inherits that command's `actor` and `case`. Time
proximity alone is never proof (ADR-0014 §1).

Zones (ADR-0014 §2): red = `pacman`, `omarchy`, systemd units including
`config-change` under `~/.config/systemd/`; yellow = other `config`,
`theme`, `plugins`; none = `snapper`, `seldon` notes and case events,
`resolution`. Hook `command` events take the zone of what the command
would produce. `crisis` derives from the zone (ADR-0008, ADR-0013).

## 5. Reconciliation (drift)

After every capture:

1. For each new event without `case`: if `actor` is an agent and
   `.seldon/active-case` was set at `ts` (the hook writes the case into the
   event directly, so this only covers collector-found events), link.
2. If the event is a pacman `dependency` of an explicit event with a case,
   link.
3. If an **open** case (queued, active, verification) lists the event's
   subject as a whole-word token in its `## Plan` section, propose (not
   link) — stored as `proposedCase` in the index for one-click
   confirmation; the lowest case id wins (ADR-0012 §7, §13).
4. Otherwise it is drift, if the event is drift-eligible: only `pacman`,
   `omarchy`, `plugins`, `theme` and `config` events can be drift;
   snapshots, notes, hook `command` events and case events never are
   (ADR-0012 §6).
5. **Grouping (ADR-0013).** Open drift `pacman` events that share a `txId`
   form one drift item keyed by the leader's event id (lowest-id explicit
   member, else lowest-id member); the index row carries `txId` and
   `members`. The item's zone is yellow iff every member is *routine*
   (kind `upgrade`/`reinstall`, not explicit, transaction command is `-S`
   with `-u`/`--sysupgrade` naming no package, subject not matching
   `config.toml [drift] alwaysRed` — default `linux*`, `systemd`, `glibc`,
   `hyprland`, `omarchy`, `quickshell`), else red. Other sources are
   never grouped.
6. Red zone → `crisis: true`.

Resolution events (`kind: resolution`, `refersTo`) are applied when the
index is built; an event with a resolution is not drift. `seldon drift
link|explain|dismiss <id>` on a group member resolves every member that is
open at that moment, one resolution line per member in one write with
`meta.txId`; `--only` resolves the named event alone (ADR-0013 §4).

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
print next steps. `--non-interactive` takes flags, then the existing
config, then the defaults. The default language is deterministic:
`--language` > config > `LC_ALL`/`LC_MESSAGES`/`LANG` (`de*` → `de`) >
`en`; the interactive wizard pre-selects the same. Every flag skips its
step. `init` refuses an existing logbook or a non-empty directory (exit 1)
and never overwrites a file; only `config.toml` is rewritten, with unknown
keys preserved. Empty layout directories get a `.gitkeep`.

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
