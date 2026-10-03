# SPEC-ENGINE.md — `seldon`, the engine

Normative. Rust crate in `engine/`, binary `seldon`.

## 1. Principles

1. Single static binary, no runtime deps, no network, no async.
2. Only writer of the logbook and the index. Everything else reads.
3. Idempotent collectors with persistent cursors.
4. Every command has `--json`; human output is for terminals, JSON is for
   agents and the plugin.
5. Fast: `status` < 100 ms at 10 000 events; `hook` < 5 ms for a call
   it does not record (about 1 ms). A recorded command returns only
   after its ledger line and the updated case file are synced to disk,
   so the case file survives a crash: about 3 ms on tmpfs and 13 ms on
   a btrfs disk (release build, measured in WP-064).
6. Never executes system changes. It may *print* commands.

## 2. Files the engine owns

| Path | Purpose |
|---|---|
| `~/.config/seldon/config.toml` | keys (WP-003): `logbook`, `language`, `watchPaths`, `harnesses`; `[collectors] snapper|pacman|omarchy|plugins|theme|config` (bool); `[git] autocommit`; `[redaction] patterns, skipPaths`; `[drift] alwaysRed` (ADR-0013; package globs, default `linux`, `linux-lts`, `linux-zen`, `linux-hardened`, `linux-rt`, `linux-rt-lts`, `linux-omarchy`, `systemd`, `glibc`, `hyprland`, `omarchy`, `omarchy-settings`, `quickshell`, `limine*`, `grub`, `mkinitcpio*`, `filesystem`, `pam`, `sddm`, `uwsm` — the R3 subjects of ADR-0023 as packages: the kernels only (firmware and headers are not R3; another kernel package is added by hand), the login path `pam`/`sddm`/`uwsm`, `/etc` through `omarchy-settings` and `filesystem`; WP-050. `init` writes the list into the file, so an existing config keeps its own); `[agent] launcher` (argv list with `{prompt}`) and `[agent.launchers] NAME = [...]` (WP-022; the section is omitted on save while it is the default); `[hooks] scope` (`"logbook"` or `"all"`, which agent sessions the hooks serve, §8; WP-063; omitted on save while it is the default `"logbook"`). `$XDG_STATE_HOME/seldon/agent-launch.log` holds the launcher's stderr; `$XDG_STATE_HOME/seldon/hooks/` the installed hook scripts (WP-024). Unknown keys survive a save; comments and key order do not (toml crate; the header says so). Precedence for the logbook path: `--logbook` > `SELDON_LOGBOOK` > config > `~/Seldon`. A global `--config FILE` / `SELDON_CONFIG` override lands in WP-006 so tests and the test host never touch the real file |
| `~/.local/state/seldon/index.json` | the contract output (see CONTRACT.md) |
| `~/.local/state/seldon/cursors.json` | `{logbook, collectors: {name: {cursor, ok, message, fix, lastRun, events}}}`, bound to the canonical logbook path (another logbook re-baselines every collector). Cursors: pacman byte offset + inode; snapper = the set of known snapshots (number, type, description — a delete event needs what was deleted); omarchy = last version; plugins = last list hash + versions; config = manifest hash. `index.state.collectors` is derived from `ok`/`message`/`lastRun` (the schema object is closed and has no `fix`; `fix` stays in `cursors.json`, `capture --json` and `doctor`) |
| `~/.local/state/seldon/manifest.json` | `{hash, files: {"~/path": sha256}, skipped: [paths], previous?}` for watched config files; written by the config collector during `collect`, with `previous` = the generation the cursor names so a failed ledger write never loses or duplicates a change (WP-005); per state dir, so switching logbooks re-baselines config with a message |
| `~/.local/state/seldon/owned.json` | `{"~/path": {hash, by, op?}}`: files the engine wrote or deleted itself under a watched path (`init --theme-hook`, `hook install`; WP-049: `init --remove-theme-hook`, `hook uninstall`) whose config event the next capture has not seen yet (§5 rule 7, WP-038); `op` is `remove` (Seldon's part taken out, the file stays) or `delete` (`hash` = the content deleted), absent for an install; written under the lock, removed by the next capture that runs the config collector successfully |
| `~/.local/state/seldon/lock` | flock during writes |
| `<logbook>/.seldon/` | logbook.toml, active-case, templates/ |

File modes (WP-064): a directory the engine creates (the logbook and its folders, the config and state directories) is 0700 and a new file 0600, whatever the umask; existing files and directories keep their mode, the engine never tightens them. Every rewrite goes through `sys::write_atomic`: a temp file `.<name>.tmp-<pid>` next to the target (new logbooks ignore `.*.tmp-*` in `.gitignore`) with the target's permission bits, synced, renamed over the target, the directory synced; the temp file is removed when any step fails. Files the engine rebuilds from the ledger and the logbook (`index.json`, `STATUS.md`, the `ledger/*.md` views, `outputs/REBUILD.md`) are written the same way without the two syncs (`sys::write_generated`): the next build writes them again. Ledger lines are synced when appended. A symbolic link at the path is followed: the link stays and its target is replaced (a link to a missing file creates the target). The theme hook script is written 0755. `.git/` is written by git under its own rules.

## 3. Commands

```
seldon init [--path DIR] [--non-interactive] [--language de|en] [--obsidian]
            [--harness claude-code|omarchy-agent]… [--git/--no-git]
            [--since TS [--baseline]] [--no-capture] [--theme-hook]
            # --since: YYYY-MM-DD (local midnight) or RFC 3339; --baseline requires
            # --since; --no-capture conflicts with --since (WP-024)
seldon init --remove-theme-hook                # WP-049: undoes --theme-hook (§9); conflicts with
                                               # every other init flag, needs no logbook
seldon agent start <caseId> [--launcher NAME] [--json]   # WP-022: active case only; the prompt
                                                         # names the case, no logbook text (WP-058)
seldon capture [--source pacman,snapper,omarchy,plugins,theme,config | --all] [--since TS]
seldon log "<text>" [--case ID] [--actor human|agent:NAME] [--tag T]
seldon event <source> <kind> --subject S [--detail D] [--case ID] [--actor A] [--meta k=v]
seldon plan new "<title>" [--zone Z] [--risk R] [--area A] [--priority P]
seldon plan start|verify|done|drop <ID> [--snapshot N] [--reason TEXT] [--actor A]
# --snapshot: `plan start` only (WP-049: the other steps do not offer it; clap
# refuses it, exit 1). Starting an R2 or R3 case with no snapshot
# (no --snapshot, no snapshotBefore) prints a warning and never refuses
# (ADR-0023); R3's also asks for the human's explicit go per step (WP-050)
seldon plan list [--status S] [--area A]
seldon plan show <ID>                            # the case file's path and text as quoted lines (`> `, as
                                                 # hook session-start, §8), under one note line; --json unquoted
seldon drift [--crisis-only] [--json]            # read-only: index items, crises first; totals count all
seldon drift show <EVENT> --json                 # {event, open, item, txId, members} — full member list of a group
seldon drift link <EVENT> <CASE> [--only] [--actor A]
seldon drift explain <EVENT> [--only] [--zone Z] [--risk R] [--area A] [--actor A] -- <intent>
seldon drift dismiss <EVENT> [--only] [--actor A] -- <reason>
# resolving commands: one lock, one ledger write (one `resolution` line per open
# member of the group, same ts/actor/detail/case, meta.txId on fan-out), case
# `events:` updated oldest-first, autocommit `seldon: drift <verb>: N event(s)`,
# index rebuilt; a re-run writes nothing (exit 0, resolved 0); ids and the case
# are checked before any write (exit 1). `explain` creates a completed
# retroactive case (ADR-0021). --json → {eventId, resolution, only, txId,
# resolved, events, case, areaCreated, git}
seldon decide "<title>" [--case ID] [--no-edit] # creates ADR, opens $EDITOR unless --no-edit
seldon status                                  # regenerates STATUS.md + index
# decide and status (WP-050) fill the `decisions.index` fence of the logbook's
# DECISIONS.md from decisions/*.md frontmatter: `| [[id]] | title | status |
# date |`, newest id first, `|` in a title escaped; the table head inside the
# fence is kept when it has one (a translated head stays), else `| ID | Title |
# Status | Date |`. Text outside the fence is never changed; a missing fence is
# appended under `## Index`, a missing file created; a begin marker without an
# end marker of its own leaves the file alone (warning). Written only on change; decide commits
# it with the new ADR, status lists it in `files`.
seldon index [--check]                         # rebuild index; --check validates against schema
seldon dossier [--section packages|services|omarchy|hardware|plugins|deviations|all] [--json]
                                               # WP-035: rewrites only the bodies of the selected generated
                                               # fences of system/*.md (comma list or repeated; default all):
                                               # packages.summary (explicit/total/aur), packages.history (a row
                                               # for today when the counts changed; today's row is replaced),
                                               # packages.explicit (sorted, `- <name> · repo|aur · omarchy-base|user
                                               # · since <date> [[C-…]]` from the ledger's latest install, else
                                               # `pre-logbook`; WP-036: class `omarchy-base` when Omarchy's
                                               # omarchy-base.packages or omarchy-other.packages names it, read
                                               # as files from $SELDON_OMARCHY_PACKAGES, else $OMARCHY_PATH/install,
                                               # else /usr/share/omarchy/install; `#` comments skipped; a list
                                               # not readable → its packages `user`, one warning),
                                               # services.enabled (system then user units; case from the ledger:
                                               # a user unit file's config event or an agent's cased `systemctl
                                               # [--user] enable`, else the old row's, else —), omarchy.summary
                                               # (version, theme, lastUpdate), hardware.summary (cpu, memory,
                                               # machine, rootfs from /proc and /sys files only), plugins.list,
                                               # deviations.table (old lines kept byte for byte, a row added per
                                               # cased config path without one, reason left empty; WP-036: a row
                                               # whose case cell is empty, blank, — or -, gets the case of its
                                               # path's latest cased config event when that event is not older
                                               # than the row's date; only that cell changes). Text outside
                                               # the fences is never changed; a missing fence is appended to its
                                               # default file under a heading in the logbook language. A failed
                                               # query skips its fences (warning, fence kept); so does a fence
                                               # whose begin marker has no end marker of its own (WP-050: no
                                               # second fence is appended; `import` reports it as an error). Files written
                                               # atomically and only on change, autocommit `seldon: dossier`,
                                               # index rebuilt; no ledger write. `init` runs it once after the
                                               # first capture; `capture` and `status` never do. --json →
                                               # {files, sections: {<fence>: written|unchanged|skipped}, counts:
                                               # {explicit, preLogbook, omarchyBase, total, aur, units, plugins},
                                               # git, warnings}
seldon rebuild [--json]                        # outputs/REBUILD.md (WP-032): 1 base, 2 explicit packages by
                                               # case (`omarchy pkg add|aur add` from meta.command: pacman -S →
                                               # repo, -U → AUR, else "repository unknown"; "Before the logbook":
                                               # the `pre-logbook` lines of class `user` of `packages.explicit` as
                                               # one `omarchy pkg add` and one `omarchy pkg aur add` block, WP-035,
                                               # then one line "N more come with Omarchy <version>" counting the
                                               # `pre-logbook` lines of class `omarchy-base` (only when N > 0; the
                                               # version from `omarchy.summary`, else section 1's base), WP-036; an empty fence
                                               # counts as none), 3 deviations, 4 plugins,
                                               # 5 theme, 6 units (incl. cased `services.enabled` rows; system scope
                                               # separately), 7 open drift (marked in place too) + dismissed
                                               # ("deliberately not reproduced"); English headings, prose in the
                                               # logbook language. A name goes into a command only after its check
                                               # (`rebuild::shell_arg`: package name, unit name, theme slug, plugin
                                               # id, https URL without user info, query or fragment; none with a
                                               # leading `-`) and is single-quoted unless it is plain; an item
                                               # whose name fails is listed as "not reproduced: invalid name" with
                                               # no command and a warning (WP-059). Code spans show control
                                               # characters and U+2028/U+2029 as escapes (`\n`, `\u{1b}`), so a
                                               # value stays on its line. Fence `rebuild`, text outside kept; written
                                               # atomically and only on change, autocommit `seldon: rebuild`; no
                                               # ledger write, no index rebuild. --json → {path, sections:
                                               # {packages, deviations, plugins, units, open}, files, git, warnings}
seldon update-impact [--target VERSION]        # outputs/UPDATE-IMPACT.md  (Phase 3)
seldon import omarchy-agent <VAULT> [--dry-run|--apply] [--json]
                                               # WP-043: the omarchy-agent kit's vault, read only; dry run is the
                                               # default. Both modes plan under the lock and write
                                               # outputs/IMPORT-omarchy-agent.md (fence `import-omarchy-agent`, text
                                               # outside kept, only on change): counts, the id mapping, collisions,
                                               # id rewrites per file, assumptions, journal days, memory files,
                                               # deviation rows, files not imported with the reason, errors,
                                               # redaction hits (file, line, rule; never the text).
                                               # Mapping: kit cases (pipeline/cases, archive/cases) keep their id and
                                               # file slug unless the logbook (case file or work/C-… folder) or an
                                               # earlier kit case has it: then the next free id of the year after the
                                               # highest of logbook and kit, tag `omarchy-agent/<old id>` and a line
                                               # under the title; every imported case is tagged `omarchy-agent`.
                                               # new|planned|in-progress|verification → queued (never active; the
                                               # last two say so in the Log), done → completed, dropped → dropped;
                                               # zone, risk, priority, created, closed kept (done/dropped without
                                               # closed: closed = created, listed under assumptions). Auftrag →
                                               # Intent, Plan → Plan, Ergebnis → Result, every other section (and
                                               # text before the first) under `## History`, headings one level
                                               # deeper. A kit id the logbook had is rewritten in all imported text
                                               # (case bodies, journal sessions, memory sections, deviation reasons):
                                               # `[[C-OLD…` and bare `C-OLD` → `C-NEW`, one pass (an id two kit
                                               # files share is not rewritten); the old id stays in the tag, the
                                               # line under the title and meta.originalId; one Log line names the
                                               # source; one ledger `manual/note` per case, `ts` = created at local
                                               # midnight, actor human, meta {import: omarchy-agent, originalId,
                                               # originalStatus, source}, its id in the case's `events`. Journal
                                               # journal/YYYY-MM.md split at `## YYYY-MM-DD…` headings outside code
                                               # fences; each day's sessions (headings one level deeper) under
                                               # `## Imported from omarchy-agent` in journal/YYYY/YYYY-MM-DD.md,
                                               # appended when the day exists, linked kit cases added to `cases:`.
                                               # knowledge/<topic>/*.md → a `## <title>` section each (source line,
                                               # headings one level deeper) appended to memory/<topic>.md (lessons →
                                               # memory/lessons.md; a new file gets `type: memory` frontmatter, an
                                               # existing one `updated` = the import day),
                                               # knowledge/<name>.md → memory/<name>.md. system/deviations.md: each
                                               # `### ` entry not resolved (✅/"aufgelöst"/an "Aufgelöst" section)
                                               # whose heading, else body, has a `~/…` or `/…` code span → a
                                               # deviations.table row `| path | <heading> (omarchy-agent) | <date> |
                                               # — |` when the path is not listed (a table whose separator an editor
                                               # padded, `| --- |`, `|:--|`, keeps its rows; `seldon dossier` reads
                                               # `packages.history` and `deviations.table` the same way). Inbox, Dashboard, templates,
                                               # STRUCTURE.md, the rest of system/, .obsidian/ and symlinks are listed,
                                               # not imported. Every imported line passes §7 redaction (config
                                               # patterns included) and `/home/<user>` at the start of a path becomes
                                               # `~`. A case the kit layout says to import but that cannot be mapped
                                               # (no or invalid frontmatter, unknown status/zone/risk/priority, bad
                                               # date, not UTF-8) or a day file with invalid frontmatter is an error.
                                               # Dry run: only the report, autocommit `seldon: import omarchy-agent
                                               # (dry run)`. --apply: refused (exit 1, report written) while there
                                               # are errors; before its first write it autocommits the logbook's
                                               # pending changes as `seldon: before import omarchy-agent` and is
                                               # refused (exit 1, nothing written) while the work tree is still not
                                               # clean (--no-commit, autocommit off, a failed commit; WP-061); then
                                               # ledger first, then cases (never overwritten), days,
                                               # memory, deviations, the report ("applied"), the marker
                                               # .seldon/imports/omarchy-agent.json {source, vault, importedAt, cases:
                                               # {old: new}, counts}; one autocommit `seldon: import omarchy-agent`;
                                               # index rebuilt. A write that fails after the ledger append is an
                                               # error that says nothing was committed and how to undo it with the
                                               # import's own files only (`git --literal-pathspecs checkout <commit before the import>
                                               # -- <files it changed> && rm -f -- <files it created>` in the
                                               # logbook, also kept in .seldon/imports/omarchy-agent.undo.json, which
                                               # the undo removes too). The marker makes
                                               # every later run write nothing ("Nothing changed", changed: false);
                                               # import notes in the ledger without the marker (an apply that did
                                               # not finish, or a deleted .seldon/) are a user error (exit 1) with
                                               # the kept undo, offered only with a hash base whenever it restores and
                                               # paths in the import's folders (ledger/ work/ journal/ memory/ system/
                                               # outputs/ .seldon/imports/), no `.git`, no glob or pathspec magic,
                                               # no existing part of a path a symbolic link
                                               # (otherwise, and without it: `git -C <logbook> status` lists what to
                                               # take back), never "nothing changed" and never a second import. Not a directory / not a vault → exit 1.
                                               # --json → {mode: dry-run|apply, changed, alreadyImported: null|{by,
                                               # importedAt}, vault, report, errors, counts: {cases, renumbered,
                                               # journalSessions, journalDays, memorySections, memoryFiles,
                                               # deviationRows, notImported, errors, redactedLines, privatePaths,
                                               # rewrittenLinks, rewrittenIds, assumptions},
                                               # cases: [{from, to, kitStatus, status, path, source, renumbered}],
                                               # collisions: [{from, to, takenBy}], rewrites: [{file, wikilinks,
                                               # ids}], assumptions: [{case, source, assumption}], skipped: [{path,
                                               # reason, error}],
                                               # files, marker, git}
seldon hook install claude-code [--settings FILE]
                                               # WP-050: `generic` dropped from the synopsis: it has no
                                               # settings file to merge into; other agents pipe into
                                               # `hook generic` themselves (§8)
seldon hook uninstall claude-code [--settings FILE]
                                               # WP-049: the inverse of install (§8); `generic` has
                                               # nothing installed, so nothing to uninstall
seldon hook claude-code                        # stdin: Claude Code hook JSON
seldon hook generic                            # stdin: {"command":"…","actor":"…","cwd":"…"}
seldon hook session-start | session-stop       # context print / journal stub
seldon watch [--interval SECS] [--json]        # feature "watch" (off by default, ADR-0005; without it: exit 1
                                               # "built without the watch feature"). Watches ledger/ work/ journal/
                                               # decisions/ system/ memory/ (recursive) and .seldon/logbook.toml;
                                               # one rebuild at start, then reacts to changes: after SECS quiet
                                               # (default and minimum 2; at most 5×SECS into a burst) rebuilds
                                               # index.json under the lock (a held lock delays, retried every
                                               # 250 ms). Writes index.json only: no capture, no views, no
                                               # STATUS.md, no commit. Ignores reads, ledger/*.md, STATUS.md,
                                               # hidden/temp/backup files. One line per rebuild on stderr; --json:
                                               # JSON lines on stdout {status: watching|rebuilt|error|stopped}.
                                               # SIGTERM/SIGINT → exit 0 after the rebuild in progress; exit 3 when
                                               # the logbook is not initialised; watcher error at start (inotify
                                               # limit) → exit 2; a failing re-watch later is an error line. RSS
                                               # budget: < 10 MB on the ×10 fixture (`just check-rss`). User unit:
                                               # engine/systemd/ (WP-034); the Phase 4 package ships the feature.
seldon doctor                                  # engine, config, logbook, cases, ledger, fences, state,
                                               # omarchy, snapper, git checks (read-only)
seldon open <case|journal|ledger|status|logbook|C-…|ADR-…> [--editor] [--json]
# prints the path; --editor on a terminal runs $VISUAL/$EDITOR attached with the
# path as one argument; without a terminal (the plugin) it launches
# `omarchy-launch-editor <path>` DETACHED (null stdio, own process group, never
# killed or waited for): a non-zero exit within 200 ms is an error (exit 1),
# otherwise {"launched": true, "program": …} (WP-008 fix of the 10 s kill)
seldon --version / seldon contract-version
seldon completions bash|zsh|fish               # WP-049: the completion script (clap_complete) on stdout;
                                               # --json → {shell, script}
seldon mangen                                  # WP-049: seldon(1) in roff on stdout (clap_mangen: name,
                                               # synopsis, global options; then COMMANDS with every command's
                                               # usage, sentence and arguments, EXIT STATUS, ENVIRONMENT,
                                               # FILES); --json → {manPage}. Both are generated from the clap
                                               # definition, read no config, logbook or home, and are what the
                                               # package (/usr/share/man/man1, bash-completion, zsh
                                               # site-functions, fish vendor_completions.d) and install.sh
                                               # (under the prefix) install
```

Help texts (WP-049): every command's `--help` starts with one sentence;
values are named by what they are (`<ID>` a case id, `<EVENT>` an event
id, `<ACTOR>` `human` or `agent:NAME`, `<ZONE>`, `<RISK>`, `<AREA>`,
`<TS>` a time, `<DIR>`, `<FILE>`); every positional has a line; a flag
that takes a form and free text after `--` have an example (`Examples:`
under the options). `docs/user/*/05-cli-reference.md` carries them
verbatim (`scripts/docs-check.sh`).

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
seldon doctor --json             → {"ok":bool,"logbook":"<path>"|null,
                                     "checks":[{"name","status":"ok|degraded|error","message","fix"?}]}
                                    exit 0 (no error), 1 (a check is error), 3 (not initialised)
```

`doctor --path DIR` is an alias of the global `--logbook DIR`. The plugin's
banner states parse the doctor shape; it is not part of `schema/`.

doctor's checks (WP-070), each `error` or `degraded` with a `fix` line
where one exists. `config`: `config.toml` parses and its `[redaction]
patterns` compile (an invalid pattern makes every writing command
refuse: error). When `config.toml` does not parse, the logbook path it
names is not known: the `logbook` row is "not checked: config.toml is
invalid", `"logbook"` is `null`, and doctor exits 1, never 3 with the
default path (a path from `--logbook`, `--path` or `SELDON_LOGBOOK` is
still checked; the exit code stays 1). With an open logbook: `cases`, a
case id in two files (error, the `index --check` rule); `ledger`, lines
that are not events, per month with the count and the first line
numbers (degraded: every reader skips them); `fences`, the generated
fence of `STATUS.md` or `DECISIONS.md` that `status` leaves alone (no end
marker of its own, or `STATUS.md` with the header but without the
fence; degraded, the fix names the marker lines), an end marker
that closes no fence (degraded), and a file that cannot be read as text
(error: `status` stops on it). doctor cannot tell an intact fence from
one whose own end marker was removed while a later end marker remains:
the writer then takes the text up to that marker as the fence body and
replaces it. The row says so when it finds a stray end marker; with only
one end marker left, nothing in the file shows it. `state`:
`cursors.json`, `manifest.json` and `owned.json` in the state directory
parse (missing is fine); each corrupt or unreadable one is an error row
with what it breaks and the fix (`mv <file> <file>.bad`). The `snapper`
probe runs the program the collector runs (`SELDON_SNAPPER`, default
`snapper`).

```
seldon capture --json  → {"ok":true,"logbook":"<path>","written":N,"files":["ledger/2026-10.jsonl"],
                          "collectors":[{"name","enabled","ran","ok","events","message"?,"fix"?}],
                          "sinceIgnored":[…],"explainedOwn":N}   # explainedOwn: §5 rule 7
                         exit 0 also when a collector is degraded (ok:false + fix, ADR-0011);
                         1 unknown source or --source with --all; 3 not initialised; 4 lock held
```

`log`, `event`, `plan *`, `open` with `--json` return `{"event":
<ledger line>, "git": {...}}` plus, for plan steps, `from`, `to`,
`movedFrom`, `activeCase`, `journal` (WP-006) and `warnings` (a list of
strings, empty unless `plan start` warned; WP-050); `decide --json` returns
`{"decision": {id, title, status, date, cases, path}, "editor", "git",
"warnings"}` (`warnings`: the `decisions.index` fill, WP-050)
(no ledger event). `plan new` defaults:
`--zone yellow --risk R1 --priority normal`; `--actor` is accepted on every
plan step so agents identify themselves; `log --tag T` stores `meta.tags`
(comma-joined) and a `#tag` line in the journal; `open` also takes
`logbook`, a case id or an ADR id; `seldon log --case` does not add a Log
line to the case (the fixture agrees). `seldon log` with `--actor agent:…` refuses
a note that contains a line break (`\n`, `\r`, vertical tab, form feed,
NEL, U+2028, U+2029; leading and trailing ones are trimmed first) with exit 1 and one
line, before anything is read or written; a person's note may have
several lines (WP-058). `SELDON_NOW=<RFC 3339>` overrides
the clock for tests and demos; `SELDON_CONFIG=FILE` is the config
override. `decide` writes no ledger event (no fitting kind; revisit with
WP-008). `.seldon/active-case` names the case started last; `done`/`drop`
clear it only when it names that case.

```
seldon init --json   → {logbook, config, machineId, language, files, obsidian, collectors, watchPaths,
                        harnesses, harnessSetup:{<name>:{…}}, git, snapper,
                        capture:{ran, since, written, files, collectors, sinceIgnored, openDrift, crisis,
                                 baseline:{reason, items, events}|null, git} | {ran:false, reason|error},
                        dossier:{ran:true, files, sections, counts, git, warnings} | {ran:false, reason|error},
                        themeHook:{requested, installed, already?, script?, hook?, error?, fix?,
                                   ownWrites?: ["~/path"] | {error}}, nextSteps}
seldon agent start <caseId> --json → {launched, launcher, program, argv (with the "{prompt}" placeholder,
                        never the prompt), case, cwd, previousActiveCase}; exit 1 for a case that is not
                        active (queued → hint `seldon plan start`), an unknown launcher, or a launcher
                        that fails within 200 ms; 3 not initialised; 4 lock held. The launcher argv
                        comes from config.toml `[agent] launcher` / `[agent.launchers] NAME`, default
                        ["omarchy","agent","prompt","{prompt}"] (no --inline: detached with null stdio,
                        --inline would run the agent without a terminal); `{prompt}` is exactly one
                        element; first element a program name without `/` or an absolute path; before
                        `{prompt}`, programs known to run their arguments as code are refused — shells,
                        interpreters, `script`, `watch`, `flock`, `su`, `ssh`, `tmux`, `screen`, `xargs`,
                        `parallel`, the compositors' exec messages, `hyprctl`, the Omarchy launchers that
                        build `bash -c` strings, `env -S`, `sudo -s|-i`; names compare without a version
                        suffix (`python3.12` is `python`); a heuristic by program name, not a sandbox, and
                        the error says so (WP-058). The prompt holds no logbook text: it names the case id
                        and the logbook path and tells the agent to run `seldon hook session-start` and
                        `seldon plan show <id>` (WP-058). As an argument it is visible in the process list
                        (`ps`) and in a session journal that logs the launch; stderr goes to
                        `$XDG_STATE_HOME/seldon/agent-launch.log`; `.seldon/active-case` is set and
                        restored on failure; no ledger event (WP-022).
```

`capture` selection: no flag or `--all` = every collector enabled in
`config.toml [collectors]`; `--source a,b` = exactly those, even if disabled.
Baseline: a collector without a cursor emits only events at or after the
logbook's `created` (or `--since TS`); diff collectors record their first
state silently. `--since` has no effect on a collector that already has a
cursor (one notice line in human output). Dedupe against the ledger runs on
every capture, not only after a rotation, so a lost `cursors.json` never
duplicates events. `capture` runs the shared attribution pass before the
append (ADR-0017), then rebuilds the index (CONTRACT rule 2); the commit
helper and reconciliation (WP-008) follow.

```
seldon index --json   → {"ok","logbook","index":"<path>","generatedAt","events":N,"summary":{…},
                         "files":["ledger/2026-10.md",…],"warnings":[…],"valid":bool}
seldon status --json  → the index shape above plus "status","state","git":{"committed":bool,"reason"?}
```

`index --check` validates before writing and refuses an invalid index
(exit 2). A case id found in two files (a stale copy, WP-057) is not a
schema error but the user's to fix: `--check` refuses with exit 1 and
names both files ("keep one file"); `index` and `status` write the index
and warn. Before `init` both commands write the `notInitialised` index and
exit 3 (what the plugin's banner expects). `status` autocommits as
`seldon: status` only when a logbook file changed; `index` never commits.
Every writing command calls `index::rebuild_if_initialised` after its
autocommit and before releasing the lock; a failed rebuild is a warning,
never a command failure. Broken files (torn ledger line, invalid case
frontmatter) are skipped with a warning that `--json` and stderr surface.
A `cursors.json` that cannot be read (every capture fails on it) is a
warning too, and every enabled collector row in `state.collectors` is
`ok: false` with that message and no `lastRun` (WP-070).

The autocommit (WP-061): `git add -A` and `git commit -m "seldon:
<summary>"` in the logbook, when `[git] autocommit` is on, `--no-commit`
is not given and the logbook has its own `.git`. Every git command runs in
the logbook with the variables that point git at another repository
removed (`GIT_DIR`, `GIT_WORK_TREE`, `GIT_INDEX_FILE`,
`GIT_OBJECT_DIRECTORY`, `GIT_ALTERNATE_OBJECT_DIRECTORIES`,
`GIT_COMMON_DIR`, `GIT_NAMESPACE` and the rest of `git rev-parse
--local-env-vars`) and `GIT_CEILING_DIRECTORIES` set to the logbook's
parent, so a `seldon` started from a git hook or with an exported
`GIT_DIR` still commits only into the logbook, and an empty or broken
`.git` never lets git walk up into a repository around it. Before it
writes, the autocommit checks that `git rev-parse --show-toplevel` is the
logbook and that its git directory (`--absolute-git-dir`) is
`<logbook>/.git`, or a `worktrees/<name>` entry whose `gitdir` file names
`<logbook>/.git` (a linked work tree); a `.git` file that points at
another repository's git directory is not committed to. `index`'s `logbook.git` reads
git with the same environment. The
user's git configuration applies (hooks, `commit.gpgsign`, a passphrase
prompt on the terminal; git runs in the engine's process group, other
programs in their own, WP-064). A detached HEAD (`git symbolic-ref -q HEAD`
fails) is not committed and nothing is staged. A commit that is not made
(detached HEAD, a stale `.git/index.lock`, a refusing hook) is one line on
stderr, `seldon: warning: git: not committed: <reason>`, and
`"git": {"committed": false, "error": "<reason>"}` in `--json`; the
command still exits 0, because the data is written. `doctor`'s `git` check
is degraded, with a fix line, when the logbook's `.git` is not its own
usable repository, and, while autocommit is on, when a `.git/index.lock`
exists, `.git` is read-only, HEAD is detached or names no commit, or git
cannot resolve the committer and author (`git var`, with the fallback
identity the autocommit uses). doctor takes no lock and writes nothing
into `.git` (file metadata and read-only git queries only).

`message` carries the full detail (e.g. the unrecognised subcommand name),
not just the error kind. Detection of `--json` must not sniff raw argv for
the literal string, because free-text arguments (`seldon log "--json"`)
may contain it.

## 4. Collectors

Each collector implements `fn collect(ctx) -> Vec<Event>` and
`fn cursor(&self) -> Cursor`. Run order: snapper, pacman, omarchy, plugins,
theme, config. `seldon dossier` (§3) is not a collector and writes no
events; it queries pacman read-only (`-Qqe`, `-Qqm`, `-Q`) and systemd
read-only (`list-unit-files --state=enabled`), and reads Omarchy's
package lists (`omarchy-base.packages`, `omarchy-other.packages`) as
plain files; no package manager or `systemctl` is ever invoked with a
mutating verb. Every program the engine runs with a timeout (collectors,
dossier queries, `omarchy hook install`) starts in its own process group
with stdin closed and its output captured; the timeout covers the output
pipes too: at the deadline the whole group is killed, including a helper
the program started that still holds a pipe, and the pipes get up to
200 ms more before the call counts as timed out (WP-064); a terminal
Ctrl-C stops the engine, not the program. git on the logbook (autocommit,
the git state in the index) stays in the engine's process group, because git,
its hooks or a signing prompt may read the terminal: at the deadline only
git itself is killed, with the same bounded pipe wait. Rules:

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
  sharpened by ADR-0017 §2–§5, with this reading of "named the subject": a
  cause that names a package attributes only members whose own transaction
  command names it or that have no logged command (`explicit` not
  `false`); `explicit: false` members are reached only through the
  full-upgrade path or by inheritance from an attributed explicit member of
  their own transaction. So an agent's `yay -S zed` never claims a human's
  later `-Syu` that happens to upgrade `zed`.
- **snapper** — `snapper --jsonout list`. New snapshot numbers become
  `snapshot` events with description; a `pre`/`post` pair is linked via
  `meta.pairOf`. Without `ALLOW_USERS` the command fails with a permission
  error; the collector then reads the snapshots from the info files
  (`/.snapshots/<number>/info.xml`, `SELDON_SNAPSHOTS_DIR`; under
  `SELDON_TEST_GUARD` without it `<guard>/.snapshots`) with the same events
  and cursor, so switching between list and info files adds no events (an
  info file that cannot be read is skipped and named in the message; its
  snapshot is neither new nor deleted). When the info files cannot be read
  either, or none is found (an empty directory, e.g. right after booting
  into a snapshot), it reports `ok: false` and the fix command, writes no
  events and keeps the cursor, never sudo (ADR-0011); `doctor` and `init`
  say that the fix's `ALLOW_USERS` entry also lets the user create, change
  and delete root snapshots without a password, and when listing works,
  `doctor`'s `ok` message adds that this user may use the snapper config,
  which also lets it create, change and delete snapshots without a
  password. snapper is run with `LC_ALL=C` (and without
  `LANGUAGE`); its messages are matched in English, whatever the user's
  locale (`doctor` and `init` use the same argv and locale).
- **omarchy** — version from `omarchy-version` (prints e.g. `4.0.4-1`;
  `omarchy --version` does not exist and `$OMARCHY_PATH/version` is
  stale); `repoHead` (7-character short hash, like `logbook.git.head`)
  only when `$OMARCHY_PATH` is a git checkout, omitted on package
  installs; change → `update` event with `from`/`to`.
- **plugins** — `omarchy plugin list --json` (shell IPC; fails when the
  shell is not running → `ok: false`); diff against last snapshot (stored
  by hash) → `plugin-add|plugin-remove|plugin-enable|plugin-disable
  |plugin-update` with id and version. Neither `list` nor `catalog`
  carries a version: read it from the manifest at the plugin's
  `manifestPath`, else `~/.config/omarchy/plugins/<id>/manifest.json`, else
  the plugin directory's own git HEAD short hash, else the last version
  seen (ADR-0014 §3). `plugin-update` only for `firstParty: false`
  (ADR-0018); enable/disable fire for every plugin. `plugin-add|update`
  `ts` = the plugin directory's mtime clamped to [last check, now], like
  theme and config, so the ADR-0017 window can match the agent's command.
  A non-zero exit, a timeout, non-JSON output or an empty list after a
  non-empty one → degraded, cursor kept.
- **theme** — current theme name (read how Omarchy stores it from
  `~/.local/share/omarchy/bin/omarchy-theme-set`); diff → `theme-set`.
  Optional hook script installed by the wizard into `theme-set.d/` calls
  `seldon event theme theme-set --subject "$1"`.
- **config** — sha256 manifest of files under `watchPaths` (default
  `~/.config/hypr`, `~/.config/omarchy` excluding `plugins/`, `~/.config/
  waybar` if present, `~/.bashrc`, `~/.zshrc`, user list). Changed/added/
  removed → `config-add|config-change|config-remove` with the path written
  with `~` and both hashes (`detail` `sha256 <8> → <8>`). Binary files (a
  NUL in the first 8000 bytes) and files over 1 MiB are listed as
  `skipped` without a hash, so growing past the limit is not a removal;
  `.git` directories are never walked, symlinked directories are not
  followed, `~/.config/omarchy/plugins/` is excluded. Known secret-bearing
  files are listed in `config.toml [redaction] skipPaths` and are never
  opened, hashed or named (pattern rules in §7). Event `ts` is the file's
  mtime clamped to [last check, now]; a removal gets the capture time.

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
   `config.toml [drift] alwaysRed` — default in §2), else red. Other
   sources are never grouped.
6. Red zone → `crisis: true`.
7. **The engine's own writes (WP-038).** A file the engine writes itself
   under a watched path — the theme hook script that `init --theme-hook`
   has `omarchy hook install` copy to
   `~/.config/omarchy/hooks/theme-set.d/seldon-theme-set.sh`, a Claude
   Code settings file `hook install` writes under a watched path — is
   recorded right after the write, under the lock, in
   `$XDG_STATE_HOME/seldon/owned.json`: its `~`-path, the sha256 of its
   content, and the command (`by`). Only paths the config collector
   hashes are recorded (under `watchPaths`, not in `skipPaths`). The next
   capture that runs the config collector successfully appends, after the collector
   events and under the same lock, one `explained` resolution per new
   `config-add|config-change` without a case whose subject and
   `meta.hashTo` match a record: `source: seldon`, actor `system`, no
   case, detail `installed by <by>` (e.g. `installed by seldon init
   --theme-hook`). The removal commands (WP-049) record the same way:
   `hook uninstall` that leaves the file records the new content with
   `op: remove`, and its `config-change` is explained; a file the engine
   deletes (`init --remove-theme-hook`, `hook uninstall` of a file that
   held only Seldon's hooks) is recorded *before* the deletion with
   `op: delete` and the sha256 of the content it had, and a
   `config-remove` whose `meta.hashFrom` matches it is explained; both
   with detail `removed by <by>` (e.g. `removed by seldon hook uninstall
   claude-code`). A file someone changed after the last capture and
   before the deletion does not match (the manifest's hash is the older
   one): its removal stays drift. Then it forgets every record, matched or not: the
   collector has seen each file — as an event, in its baseline, or with
   content someone else wrote, which stays drift. The config event stays
   in the ledger with its own actor; the index folds the resolution
   (ADR-0021: no case, so none is folded) and the event is no drift. A
   capture without the config collector keeps the records. Files the
   wizard writes before its first capture (the harnesses inside the
   logbook) are part of that capture's config baseline and need no record.

Resolution events (`kind: resolution`, `refersTo`) are applied when the
index is built; an event with a resolution is not drift. `seldon drift
link|explain|dismiss <id>` on a group member resolves every member that is
open at that moment, one resolution line per member in one write with
`meta.txId`; `--only` resolves the named event alone (ADR-0013 §4) and
does not fan out to the explicit event's dependencies (rule 2 is
satisfied at capture and by the fan-out without `--only`). A resolved
event is no longer a group handle: after `--only` on a leader, the rest
is a new item with its own leader and the old id resolves nothing (exit
0, `resolved: 0`). `drift link` accepts a completed or dropped case as
target (retro-links). `drift explain` creates a completed retroactive
case and the index folds its `case` onto the explained event (ADR-0021).

## 6. Index build

`seldon index` reads: all `ledger/*.jsonl`, all case files, journal file
for today and yesterday, `decisions/*.md` frontmatter, `system/*.md`
generated fences, `memory/*.md` frontmatter + first 20 lines, `areas/*/`.
Produces `index.json` per CONTRACT.md. Writes atomically (temp + rename).
A ledger month is read as bytes and decoded line by line: a line that is
not UTF-8 (a write torn inside a multi-byte character), not an event, or
names an actor or a case that the ledger would refuse to write is skipped,
and one warning per month names the month, the count and the first line
numbers (`--json` `warnings`, stderr); `Ledger::bad_lines` gives the same
per month (WP-065).
`STATUS.md` is rendered from the same data into its `status` fence; text
outside the fence is the user's. A damaged fence (a begin marker without
an end marker of its own) or a file with the generated header but no
fence leaves `STATUS.md` byte for byte as it is, with a warning; only the
`init` template is replaced. Fence markers end in `\n` or `\r\n`; a CRLF
file gets CRLF in its new body. In the fence bodies of `STATUS.md`,
`DECISIONS.md` and `outputs/*.md` the engine writes, `<!-- seldon:` gets a
zero-width space (U+200B) after `<!--`, so a title, subject or message
can neither end nor open a fence (WP-065). The dossier fences of
`system/*.md` are not neutralised yet (follow-up, WP-075).

Performance budget: 10 000 events, 300 cases, 365 journal files → < 100 ms
warm. Measured in `cargo bench` with the fixture logbook scaled ×10.

## 7. Redaction

Before writing any event, `subject`, `detail` and every string value of
`meta` are passed through redaction. The commands that write free text
into the logbook (`log` with its tags, `plan new` and the `plan` step
reasons, `decide`, `drift explain|dismiss`) pass that text through the
same redaction before
the first write, so the ledger, the journal, case and decision files,
`STATUS.md` and the index hold the same redacted text (WP-062). The rules
(`redact::BUILTIN`, in this order): URLs with userinfo; `--password`;
`--token`, `--with-token`, `--secret`, `--client-secret`, `--passphrase`
and similar options; `--api-key`, `--access-key`, `--secret-key`;
`token=`; `…SECRET=`, `…PASSWORD=`, `…PASSWD=`, `…PASSPHRASE=`, `…_PWD=`,
`…_PASS=`, `SSHPASS=` assignments (also `PGPASSWORD=`); `…KEY=`
assignments (also `?api_key=`); `Authorization:`; headers whose name ends in
a credential word (`X-…-Key:`, `X-…-Token:`, `X-…-Secret:`, `X-Auth:`,
`X-…-Auth:`, `Api-Key:`, `Private-Token:`; not `X-Author:`);
`(AKIA|ASIA)[0-9A-Z]{16}`; `gh[pousr]_[A-Za-z0-9]{36,}` and
`github_pat_…`; `glpat-…`; `xox[abposr]-…`; `sk-`/`sk_` keys
(`\bsk[-_][A-Za-z0-9_-]{20,}`); anything after `-p ` for
`mysql|psql|smbclient`; the value after `curl -u`/`--user`; after
`sshpass -p`; after `-p` of `docker|podman|buildah|nerdctl|helm registry
login`; and user-supplied patterns in `config.toml [redaction] patterns`.
Replacement: `‹redacted›`. The hook never records stdin/stdout of
commands, only the command line. A name that can only mean a
credential masks any non-empty value: `--password`, `--token`,
`--with-token`, `--secret`, `--client-secret`, `--passphrase`
(`password-option`, `secret-option`) and `token=`, `…SECRET=`,
`…PASSWORD=`, `…PASSWD=`, `…PASSPHRASE=`, `…_PWD=`, `…_PASS=`,
`SSHPASS=` (`token-assignment`, `secret-assignment`). A name that ends
in `key` (`--api-key`, `--access-key`, `--secret-key`: `key-option`;
`…KEY=`: `key-assignment`) masks a value only when it looks like a
credential: without its quotes, at least 16 characters, or at least 8
that mix two of lower case, upper case, digits and other characters
(`redact::looks_like_credential`), so `sort --key=2`, `hotkey=Super` and
`the key=value pairs` stay as they are. Redaction over-matches by design
(WP-004): the `sk` rule also matches `sk_`/`sk-proj-`/`sk_live_` (at a
word start, so `task-…` is not cut), `token=` and the assignments are
case-insensitive, quoted values are redacted whole, mysql's attached
`-pSECRET` and psql's `-p` port are both redacted. URL userinfo that
holds a `:` is cut from `://` up to the last `@` before the next white
space or quote, so a password may contain `/ ? # : @`; userinfo without
a `:` (a bare token) is cut up to the last `@` before the path. Redacting
twice gives the same text for the built-in rules: a match that lies
inside an existing `‹redacted›` is left alone (a user pattern that
matches across the marker's edge is applied as written). Each built-in
rule is compiled once per process, and only when the text holds one of
its literal triggers (`redact::triggers`), checked on the text in lower
case with the Kelvin sign and the long s folded onto `k` and `s`, as
case-insensitive matching folds them (`redact::trigger_text`). An invalid user pattern is a user error
(exit 1): Seldon writes nothing rather than unredacted text. `subject` is
cut at 512 and `detail` at 4096 characters after redaction. Files written
before a rule existed are not rewritten.

`[redaction] skipPaths` (config collector and the hook, ADR-0014 §4): a
pattern with `/` matches the full path (`~/` = home), as a file or as a
directory with everything below it; a relative pattern matches at any
directory boundary; a pattern without `/` matches a file or directory
name; `*` and `?` stay within one path component, `**` crosses them.
Matching files are never opened, hashed or named; a hook path that matches
is recorded redacted.

## 8. Hooks

`seldon hook claude-code` reads the **PreToolUse** JSON from stdin
(ADR-0017: the event carries the start time, `meta.toolUseId` and
`meta.sessionId`; a PostToolUse for a recorded `tool_use_id` writes
nothing, checked under the state lock so that two PostToolUse calls for
one tool call write one event; a PostToolUse-only install records with
the arrival time and rarely attributes), extracts `tool_input.command` (or `file_path` for
`Edit|Write|MultiEdit`, ADR-0014 §4), and classifies with the shared
command parser (`pkgcmd.rs`): package managers via `is_mutating` (query
sub-commands never, ADR-0017 §5; `pacman -Sy` counts), `omarchy
(update|pkg add|aur add|drop|remove|install|remove|reinstall|plugin
add|clone|enable|disable|remove|update|theme set|install|remove|update)`
and the `omarchy-*` scripts, `systemctl (enable|disable|start|stop|mask|
unmask)`, `cp|mv|install|ln|tee|sed -i|rm|rmdir|unlink|truncate` and
redirections whose target — or `mv` source — lies in a watched path
(`watchPaths` plus `~/.config/systemd`, always), `git` changing
sub-commands inside the XDG config home or the logbook; the string of
`bash|sh|zsh -c` and `eval` is re-parsed. Relative paths resolve against
the payload's `cwd` and earlier `cd`s in the line; heredoc bodies are the
command's stdin and are cut from the record; redaction runs before the
4096-character cut. Green events per ADR-0019 only while a case is set.
Non-mutating commands produce no event. A command line that names a path
`[redaction] skipPaths` matches (§7) is recorded as `<program>
‹redacted›` (the event's subject), as an `Edit` of such a file is
recorded as `Edit ‹redacted›`; the line is read for such paths more
widely than the shell would open them: its write targets, every word of
its commands (`sh -c` and `eval` strings opened), every token of the line
between blanks, quotes, shell operators, `:` and `,` (files read with
`<`, words inside `$(…)`, the parts of a list such as `PATH=a:b`), each
also after its first `=`, relative paths against `cwd` and every
directory a `cd` or `pushd` in the line moves to or a `-C DIR` names
(`git -C`, `make -C`). Paths built from shell variables or globs are read
as written (WP-063).
Output: nothing on stdout (hooks must stay silent), exit 0 always, even
on malformed stdin, a missing logbook, a broken config or a held lock
(waited for up to 8 s, below the 10 s timeout `hook install` sets; a
capture holds the lock while its collectors run; after the wait, stderr
says the lock is held and the command was not recorded). The hook takes the state lock before it reads the case
(`--case`, `.seldon/active-case`) and holds it until the case file is
written, like every other case writer; only green records with no case
at all are dropped before the lock. Only after it wrote an event, it
rebuilds the index the cheap way (no git spawn, `.git/HEAD` read
directly), after releasing the lock and without waiting for it again
(another writer that holds it rebuilds after its own write), and only
while the ledger has at most 1000 lines; above that the next `capture`
or `status` brings the index up to date. WP-062's redaction is not
slower than before it: measured 2026-10-03 on a loaded dev host (load
average 3 to 8), release builds interleaved with a build of the code
before it, median of 21 runs each, a recorded command took 19 to 36 %
less time with an empty ledger, near 1000 lines with the rebuild, above
1000 lines without it, and for a command with secrets in it. A
non-mutating command compiles no redaction rule.
`seldon hook generic` takes `{"command","actor","cwd",
"startedAt"?}` with the same rules.

Session scope (WP-063): the hooks serve the sessions inside the logbook.
A session's directory is `CLAUDE_PROJECT_DIR` when that is set in the
hook's environment (Claude Code sets it for its hook commands; it stays
the project while the agent's `cwd` moves), else the payload's `cwd`;
relative paths in a command still resolve against `cwd`. `hook
claude-code`, `hook generic`, `hook session-start` and `hook
session-stop` do nothing — no event, no context block, no journal line,
no capture, no commit, exit 0 — when that directory is not the logbook
or a directory below it (compared as written and with symbolic links
resolved; a directory that is not an absolute path counts as outside).
A call with neither (an agent or a person running the hook itself) is
served. `config.toml [hooks] scope` sets this: `"logbook"`
(the default; not written to the file) or `"all"`, which serves every
session wherever it works, as the hooks did before WP-063. A settings
file that only the logbook's sessions read (the default
`<logbook>/.claude/settings.json`) gives the same result under both; the
setting matters for a settings file outside the logbook, such as the
user-wide `~/.claude/settings.json`, whose hooks Claude Code runs in every
session of the user.

`seldon hook install claude-code
[--settings FILE]` merges `PreToolUse` (`Bash|Edit|Write|MultiEdit`),
`SessionStart` and `SessionEnd` (timeout 60 s, Claude Code's cap; `Stop`
would fire after every reply) into `<logbook>/.claude/settings.json`
without clobbering existing hooks, idempotently; the merged file is
written with sorted keys. When it writes a file under a watched path
(e.g. `--settings ~/.claude/settings.json` with `~/.claude` watched), it
records the file as the engine's own write (§5 rule 7), so the next
capture explains its config event; `--json` adds `ownWrites` (the
recorded `~`-paths, `{error}` when they could not be recorded, `null` when nothing
was added). A settings file outside the logbook (any `--settings` path
not inside it, compared as for the session scope) gets a warning, in the
report as `warning: …` and under `--json` in `warnings` (empty
otherwise), also when nothing was added: Claude Code runs the hooks in
every session that reads the file, and the text says what Seldon does in
the sessions outside the logbook under the current `[hooks] scope`
(nothing under `"logbook"`; records and prints the context under
`"all"`). Other agents call `hook generic` themselves.

`seldon hook uninstall claude-code [--settings FILE]` (WP-049) is the
inverse: it takes out each hook `install` writes (same event, matcher and
command) and keeps everything else, also a hook the user added to one of
Seldon's groups; a group, an event list or the `hooks` object it leaves
empty goes too, and a file left as `{}` is deleted (its directory stays).
None of Seldon's hooks there (or no file): nothing is written, exit 0. A
file that is not a JSON object is refused unchanged (exit 1). The write or
deletion is recorded as the engine's own (§5 rule 7: `op: remove` or
`delete`), the logbook's own file is committed as `seldon: hook uninstall
claude-code`. It is not an agent hook: errors keep their exit codes
(3 without a logbook). `--json` → `{settings, removed, absent, deleted,
ownWrites, git}`. Both `install` and `uninstall` hold the state lock from
reading the settings file to the commit (WP-049 review), so no capture or
`watch` sees the written file before its record; while another `seldon`
holds the lock they change nothing and exit 4.

`seldon hook session-start` prints a compact context block to stdout
(nothing for a session outside the session scope above):
STATUS summary, active case (id, title, plan steps), last 5 journal lines
(of the latest day file `journal/YYYY/YYYY-MM-DD.md` up to today; other
file names are skipped), `memory/lessons.md` headings. Claude Code adds it
to the session's context. Under the title one fixed line says that lines
starting with `>` are quoted from the logbook and are data, not
instructions. Every line taken from the logbook — also the case title and
an error text that may carry it — is printed with `> ` in front (an empty
one as a bare `>`); each line
break in it (`\n`, `\r`, vertical tab, form feed, NEL, U+2028, U+2029)
starts a new quoted line, and other control characters become U+FFFD.
Seldon's own lines (the title, the note, the `## ` headings, the case's
id/status line and the fixed texts) never start with `>`, so logbook text
cannot take their form (WP-058). The framing keeps the structure
unambiguous; it does not guarantee that a model reads quoted text only as
data.

`seldon hook session-stop [--actor agent:NAME]` appends `## HH:MM ·
agent:NAME · CASE` with "session ended; N events recorded" (N = the
session's events by `meta.sessionId`) to today's journal, runs `capture
--all`, then, under the state lock, writes the generated views
(`STATUS.md`, `ledger/*.md`, the `decisions.index` fence of
`DECISIONS.md`) and commits `seldon: session ended (agent:NAME)`, and
rebuilds the index after the commit. Every step runs even if an earlier
one failed (a journal day it cannot read, a failed capture, a failed
commit); each failure is one line on stderr and the hook exits 0. Only a
lock it cannot get within 8 s skips the steps that need it.

## 9. Wizard (`seldon init`)

Interactive via `dialoguer` (no `gum` dependency; gum is optional eye candy
later). The wizard asks: path (the options of ADR-0010: `~/Seldon`,
`~/Documents/Seldon`, a detected project folder, custom) → language →
Obsidian config yes/no → collectors (all on by default) → watched config
paths (defaults shown) → harnesses (`claude-code`, `omarchy-agent`) →
theme hook yes/no (default no) → git → backfill (a note explains the
drift consequence, then a date or empty). Then `init` runs, in order
(WP-024): layout + harness files (the Omarchy-Agent kit from
`${XDG_DATA_HOME:-~/.local/share}/seldon/harness/omarchy-agent/` or
`$SELDON_OMARCHY_AGENT_KIT`, copied into `<logbook>/.claude/` without
overwriting, symlinks skipped, modes kept, before the Claude Code hook
merge of §8; without the dir `init` reports what it would copy) → git init
+ first commit → `capture --all [--since]` in-process, pinned to the new
logbook → pre-Seldon baseline (`--baseline`, or asked interactively with
the item count, default yes): one `dismissed` resolution per open drift
*member*, reason `pre-Seldon baseline`, actor `human`, `meta.txId` on
groups, one ledger write over every open item (not capped), events stay
→ commit `seldon: first capture[ and pre-Seldon baseline]` → index
rebuild → `seldon dossier` once, all sections, its own commit `seldon:
dossier` (only after a first capture; `--no-capture` adds `seldon
dossier` to the next steps, WP-035) → theme hook on opt-in only: the
embedded script is written to
`$XDG_STATE_HOME/seldon/hooks/seldon-theme-set.sh` and `omarchy hook
install theme-set <script>` runs once (skipped when
`~/.config/omarchy/hooks/theme-set.d/seldon-theme-set.sh` exists; a
failure prints the manual command); the installed copy is recorded as
the engine's own write (§5 rule 7), so the next capture's `config-add`
for it is explained and opens no drift (WP-038) → next steps (`seldon
drift` when drift stays open). A failure after the layout is reported, never fatal.

`seldon init --remove-theme-hook` (WP-049) undoes the theme hook: it
deletes `~/.config/omarchy/hooks/theme-set.d/seldon-theme-set.sh` (a name
of Seldon's own; Omarchy has no `hook remove`) under the lock, recorded
first as the engine's own deletion (§5 rule 7: `op: delete`), so the next
capture's `config-remove` is explained `removed by seldon init
--remove-theme-hook`, and deletes `$XDG_STATE_HOME/seldon/hooks/
seldon-theme-set.sh`; directories stay. Nothing installed: nothing
changes, exit 0. It runs no wizard, needs no logbook (the config's
`watchPaths` decide what is recorded; defaults without a config) and
conflicts with every other `init` flag. The flag sits on `init`, not on a
new `setup` command, because `init --theme-hook` is the only way the hook
is installed: the undo is next to it in the same help. `--json` →
`{hook, removed, script, scriptRemoved, ownWrites}`.
`--non-interactive`: flags, then the existing config, then: `~/Seldon`,
language from the locale, all collectors, default watched paths,
harnesses from the config (none on a fresh machine), git on, first
capture from now (no backfill, no baseline), no Obsidian, no theme hook.
`SELDON_TEST_GUARD=<dir>`: the engine refuses to run (exit 2) when its
resolved home/config/state dirs lie outside that dir — set by every test
harness and manual recipe so a lost environment can never reach the real
home. `--non-interactive` takes flags, then the existing
config, then the defaults. The default language is deterministic:
`--language` > config > `LC_ALL`/`LC_MESSAGES`/`LANG` (`de*` → `de`) >
`en`; the interactive wizard pre-selects the same. Every flag skips its
step. `init` refuses an existing logbook or a non-empty directory (exit 1)
and never overwrites a file; only `config.toml` is rewritten, with unknown
keys preserved. Empty layout directories get a `.gitkeep`.

The wizard writes templates from `engine/templates/{en,de}/` into the
logbook: `AGENTS.md` (the rules for agents, the short form of
`docs/AGENT-GUIDE.md`: session start, engine is the only writer, cases,
zones, commands, journal and memory, drift, hooks, ending a session,
never; no `CLAUDE.md`, WP-047), `PROJECT.md`, `STATUS.md`,
`DECISIONS.md`, `areas/*/README.md` for the default areas (`hyprland`,
`themes`, `packages`, `dev-env`, `plugins`, `shell`), `memory/lessons.md`,
`system/*.md` skeletons and `.seldon/templates/{case,decision}.md`.
English keys and headings in every language, prose per language
(ADR-0007); `engine/tests/golden/init-skeleton.txt` pins the skeleton.

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
