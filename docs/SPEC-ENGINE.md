# SPEC-ENGINE.md — `seldon`, the engine

Normative. Rust crate in `engine/`, binary `seldon`.

## 1. Principles

1. Single static binary, no runtime deps, no network, no async.
2. Only writer of the logbook and the index. Everything else reads.
3. Idempotent collectors with persistent cursors.
4. Every command has `--json`; human output is for terminals, JSON is for
   agents and the plugin.
5. Fast: `status` < 100 ms at 10 000 events; `hook` < 5 ms for a call
   it does not record (about 0.6 ms, WP-092). A recorded command returns only
   after its ledger line and the updated case file are synced to disk,
   so the case file survives a crash: about 3 ms on tmpfs and 13 ms on
   a btrfs disk (release build, measured in WP-064).
6. Never executes system changes. It may *print* commands.

## 2. Files the engine owns

| Path | Purpose |
|---|---|
| `~/.config/seldon/config.toml` | keys (WP-003): `logbook`, `language` (the language `init` gives a new logbook; no key = the locale, §9; the logbook keeps its own in `.seldon/logbook.toml`, which every later command reads, so changing the key later leaves an existing logbook as it is; WP-074), `watchPaths`, `harnesses`; `[collectors] snapper|pacman|omarchy|plugins|theme|config` (bool); `[git] autocommit`; `[redaction] patterns, skipPaths` (`skipPaths` default: the plugin state files `~/.config/omarchy/**/history.json`, `**/history/`, `**/state.json`, `**/cache/`, `**/*.log`; WP-069; an empty list, as `init` wrote it before, also means the defaults, a non-empty list replaces them; `init` writes the defaults into a new file and names `skipPaths` in its output); `[drift] alwaysRed` (ADR-0013; package globs, default `linux`, `linux-lts`, `linux-zen`, `linux-hardened`, `linux-rt`, `linux-rt-lts`, `linux-omarchy`, `systemd`, `glibc`, `hyprland`, `omarchy`, `omarchy-settings`, `quickshell`, `limine*`, `grub`, `mkinitcpio*`, `filesystem`, `pam`, `sddm`, `uwsm` — the R3 subjects of ADR-0023 as packages: the kernels only (firmware and headers are not R3; another kernel package is added by hand), the login path `pam`/`sddm`/`uwsm`, `/etc` through `omarchy-settings` and `filesystem`; WP-050. `init` writes the list into the file, so an existing config keeps its own); `[drift] attention` (`"normal"`, default: the classification of ADR-0028 §2, §5; `"all"`: every drift-eligible event without a case is open drift with the pacman zone computed and `crisis` iff red, the derivation before ADR-0028 and its rollback), `routine` (the routine rule ids that apply, default all: `sysupgrade`, `upgrade`, `keyring`, `omarchy-update`, `plugin-toggle`, `theme`, `omarchy-default`, `system-link`, `routine-paths`, `theme-assets`, `theme-repo`; a rule left out does not apply and its events fall to the next row, usually attention), `routinePaths` (default `~/.config/omarchy/shell.json`, `**/*.bak.*`), `routinePackages` (default `archlinux-keyring`, `omarchy-keyring`), `alwaysRedPaths` (default `~/.config/systemd/user/**`, `~/.config/omarchy/hooks/**`, `~/.config/autostart/**`, `~/.config/environment.d/**`, `~/.config/uwsm/**`, `~/.profile`, `~/.bash_profile`); the path lists take the `skipPaths` glob syntax (§7) against the `~`-path subject; these five keys are read at index time and written only when they differ from the default, so a later engine's defaults reach a config `init` wrote (ADR-0028 §4c; WP-109); `[agent] launcher` (argv list with `{prompt}`) and `[agent.launchers] NAME = [...]` (WP-022; the section is omitted on save while it is the default); `[hooks] scope` (`"logbook"` or `"all"`, which agent sessions the hooks serve, §8; WP-063; omitted on save while it is the default `"logbook"`). `$XDG_STATE_HOME/seldon/agent-launch.log` holds the launcher's stderr; `$XDG_STATE_HOME/seldon/hooks/` the installed hook scripts (WP-024). Unknown keys survive a save; comments and key order do not (toml crate; the header says so). Precedence for the logbook path: `--logbook` > `SELDON_LOGBOOK` > config > `~/Seldon`. Path values in the file (`logbook`, `watchPaths`): `~`, `~/…`, `$HOME/…`, `${HOME}/…` and a relative value lie under the home directory, never the current directory (the plugin and the hooks run the engine from different directories; WP-069), `.`/`..` folded, an empty value ignored; the wizard stores typed watch paths as `~/…`. `--logbook`, `SELDON_LOGBOOK`, `--config` and `SELDON_CONFIG` stay relative to the current directory. A global `--config FILE` / `SELDON_CONFIG` override lands in WP-006 so tests and the test host never touch the real file |
| `~/.local/state/seldon/index.json` | the contract output (see CONTRACT.md) |
| `~/.local/state/seldon/cursors.json` | `{logbook, collectors: {name: {cursor, ok, message, fix, lastRun, events, pendingBaseline}}, pendingNotes}` (`pendingBaseline`: `cursors` or `logbook`, only while set, §3 state reset; an entry without `lastRun` and `cursor`, only `ok: true`, `events: 0` and the mark, is a collector that was not run in the capture that lost its state, WP-091; `pendingNotes`: the times of `seldon` notes a capture was about to append, only while set, §3 state reset, WP-099; `silentBaselines`: per canonical logbook path, the sources whose baseline a capture took or left waiting without a note, only while set, §3 state reset, WP-104), bound to the canonical logbook path (another logbook re-baselines every collector). Cursors: pacman byte offset + inode; snapper = the set of known snapshots (number, type, description — a delete event needs what was deleted); omarchy = last version; plugins = last list hash + versions; config = manifest hash, check time and the marker `atCheck` (§4, WP-107). `index.state.collectors` is derived from `ok`/`message`/`lastRun`, and from an entry with only the mark as from no entry (`ok: true`, no message, `lastRun: null`) (the schema object is closed and has no `fix`; `fix` stays in `cursors.json`, `capture --json` and `doctor`) |
| `~/.local/state/seldon/manifest.json` | `{hash, files: {"~/path": sha256}, skipped: [paths], scope: {watch, exclude, skip}, stats: {"~/path": [size, mtimeNs, ctimeNs, inode]}, previous?}` for watched config files; written by the config collector during `collect`, with `previous` = the generation the cursor names so a failed ledger write never loses or duplicates a change (WP-005); per state dir, so switching logbooks re-baselines config with a message. `hash` covers `files` and `skipped` only. `scope` (WP-069) is the scope the generation was taken in: the watch paths and excluded folders and files as `~`-paths and the `skipPaths` patterns as configured, sorted (a generation written before WP-069 has none). `stats` holds the size, mtime and ctime (ns) and inode of each hashed file of the current generation, except files modified less than 2 s before the walk started |
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
# log, event, the agent hooks (claude-code, generic) and `drift link` change a
# case after their ledger line: its save is checked first, and a save that would
# be refused (WP-066) fails the command before the ledger or the journal changes
# (exit 1; a hook records nothing and says so on stderr) (WP-077)
seldon plan new "<title>" [--zone Z] [--risk R] [--area A] [--priority P]
seldon plan start|verify|done|drop <ID> [--snapshot N] [--reason TEXT] [--actor A]
# --snapshot: `plan start` only (WP-049: the other steps do not offer it; clap
# refuses it, exit 1). Starting an R2 or R3 case with no snapshot
# (no --snapshot, no snapshotBefore) prints a warning and never refuses
# (ADR-0023); R3's also asks for the human's explicit go per step (WP-050)
seldon plan list [--status S] [--area A]          # a case file that does not load is a warning line
                                                 # (`<path>: invalid case: …; skipped`, as the index's;
                                                 # --json `warnings`), the others are listed, exit 0 (WP-077)
seldon plan show <ID>                            # the case file's path and text as quoted lines (`> `, as
                                                 # hook session-start, §8), under one note line; --json unquoted
seldon drift [--crisis-only] [--all] [--json]    # read-only: index items, crises first; totals count all; --all:
                                                 # every item that can still be resolved, routine ones too,
                                                 # uncapped (ADR-0028 §4c). Each item adds `class`
                                                 # (routine|attention|crisis) and `rule` (§5) to the index's
                                                 # fields; --json → {drift, openDrift, crisis, routine}
seldon drift show <EVENT> --json                 # {event, open, class, rule, item, txId, members} — the item's
                                                 # members that can still be resolved (open or routine)
seldon drift link <EVENT> <CASE> [--only] [--actor A]
seldon drift explain <EVENT> [--only] [--zone Z] [--risk R] [--area A] [--actor A] -- <intent>
seldon drift dismiss <EVENT> [--only] [--actor A] -- <reason>
# resolving commands: one lock, one ledger write (one `resolution` line per open
# member of the group, same ts/actor/detail/case, meta.txId on fan-out), case
# `events:` updated oldest-first, autocommit `seldon: drift <verb>: N event(s)`,
# index rebuilt; a re-run writes nothing (exit 0, resolved 0); ids and the case
# are checked before any write (exit 1). `explain` creates a completed
# retroactive case (ADR-0021). --json → {eventId, resolution, only, txId,
# resolved, events, case, areaCreated, git}. ADR-0028 §3: `link` also takes a
# routine event (its whole transaction, or one event with --only);
# `explain|dismiss` of a routine event exit 1 ("routine", the rule, `drift
# link` named). An agent actor (--actor or SELDON_ACTOR) may not explain or
# dismiss a crisis and may link one only to an active case whose `agents`
# lists it (exit 1, before any write); a human is never refused
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
                                               # second fence is appended; `import` reports it as an error). A
                                               # system/*.md that is not UTF-8 or not readable is skipped (the
                                               # index's `cannot read …; skipped` warning) and never written; a
                                               # fence no readable file has is then not appended while its default
                                               # file is such a file, or one that may hold it (its lossy bytes have
                                               # the begin marker, or it could not be read at all): warning, fence
                                               # kept (WP-075). Files written
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
                                               # originalStatus, source}, its id in the case's `events`; before
                                               # them, in the same write, one note for the apply itself (subject
                                               # `omarchy-agent`, no case, `ts` = the apply, meta {import:
                                               # omarchy-agent}), so a vault without cases is guarded too (WP-075). Journal
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
                                               # date, not UTF-8, a file or folder name that is not UTF-8 (shown
                                               # with U+FFFD), not readable) or a day file with invalid frontmatter
                                               # is an error; the plan goes on (WP-075). A listed file is listed
                                               # whatever its name. Frontmatter may start after a UTF-8 BOM and its
                                               # `---` lines may end in spaces or tabs, as in the logbook (WP-066).
                                               # VAULT is a path: `~/` is the home, other bytes are kept.
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
seldon hook generic                            # stdin: {"command":"…","actor":"…"?,"cwd":"…"}
seldon hook session-start | session-stop       # context print / journal stub
seldon watch [--interval SECS] [--json]        # feature "watch" (off by default, ADR-0005; without it: exit 1
                                               # "built without the watch feature"). Watches ledger/ work/ journal/
                                               # decisions/ system/ memory/ areas/ (recursive; areas/ since WP-075)
                                               # and .seldon/logbook.toml;
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
seldon doctor                                  # engine, config, logbook, cases, ledger, fences, rules,
                                               # collectors, state, omarchy, snapper, git, watch, drift
                                               # checks (read-only)
seldon rules update [--replace] [--json]       # WP-100, ADR-0027: the rules block of the logbook's AGENTS.md
                                               # (`<!-- seldon:begin rules vN -->` … `<!-- seldon:end -->`,
                                               # marker lines as whole lines) becomes this engine's v2 block.
                                               # Fenced file: the block is rewritten, every byte outside it
                                               # kept (a CRLF block keeps CRLF); a block that is no block
                                               # Seldon wrote (this engine's in any language, or a released
                                               # one) was edited, so the file is archived first. Unfenced
                                               # file (v1): exactly as a release wrote it (sha256 of the
                                               # v0.1.0 and v0.1.1–v0.1.3 templates, en and de) → the
                                               # template; otherwise the file is archived, the template
                                               # written, and only its lines that occur in no v1 text Seldon
                                               # wrote (those four and the pre-release renderings of WP-003,
                                               # WP-024, WP-047, engine/templates/rules-v1/; order kept,
                                               # blank runs as one) follow below `## Your rules (kept)`;
                                               # none left (also an empty file) → the template alone. No
                                               # file: the template. Archive and --replace: the old file's
                                               # bytes go to archive/AGENTS-<date>.md (`-2`, `-3`, … when
                                               # taken; never overwritten); a blank file is not archived.
                                               # Refused, file untouched (exit 1): a damaged block
                                               # (no end marker line, a marker inside it, a begin marker
                                               # without a version), a block newer than v2, a file that is not
                                               # UTF-8 (all three: --replace takes them). Prints a `-U0` diff;
                                               # autocommit `seldon: rules update`; a current file is "nothing
                                               # changed" (exit 0, no write, no commit). Never runs on its own.
                                               # --json → {file, action: unchanged|created|rewritten|kept|
                                               # replaced, from: "vN"|null, version, archived, diff, git}
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

Agent rules (the v2 block of the logbook's `AGENTS.md`, ADR-0027,
WP-100; normative for the templates in `engine/templates/{en,de}/`):
the R3 check resolves every package transaction read-only before it
runs — `pacman -Sp --print-format %n <package>…` (the whole set,
dependencies included), and for a PKGBUILD, the project's or an AUR
package's, the same over its `depends` and `makedepends`; the agent never
refreshes the sync database for an install (`-Sy`, `-Syy`), so what was
checked is what runs. A system upgrade (`pacman -Syu`, `omarchy update`,
an AUR helper's `-Syu`) and any transaction that cannot be resolved
read-only are R3 as such (one go, with the list `checkupdates` shows);
an AUR install as such is not. This is stricter than ADR-0027 §2c's
recipe, which stays as accepted. The user's rules and area rules only
add limits, and an agent edits no `AGENTS.md` unasked. A child agent
process, job or timer runs with `SELDON_ATTENDED` unset and its own
`SELDON_ACTOR`. Two deviations from ADR-0027 §3's wording: the snapshot
description is the case id only (`-d "<ID>"`; the title is logbook text
and never goes into a shell command), and the agent starts the case
first, then snapshots, and records the number in a *Log* line
`snapshot <N> (<config>) before <step>` until WP-101 adds
`plan snapshot`; `plan start --snapshot N` stays the human's (snapshot
before the start). Every `seldon …` the templates name is checked
against `--help` by `tests/init.rs`.

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
                                     "checks":[{"name","status":"ok|degraded|error","message","fix"?}],
                                     "drift":{"attention","routine","routinePaths","routinePackages",
                                              "alwaysRedPaths","alwaysRed","nonDefault"}}
                                    exit 0 (no error), 1 (a check is error), 3 (not initialised)
```

`doctor --path DIR` is an alias of the global `--logbook DIR`. The doctor
shape is not part of `schema/`, and the plugin does not run `doctor`: its
banners come from `index.json`, its `seldon --version --json` probe and the
results of its engine calls.

doctor's checks (WP-070), each `error` or `degraded` with a `fix` line
where one exists (an `ok` row has a fix only for the old snapper opt-in,
§4). `config`: `config.toml` can be read, parses and its
`[redaction] patterns` compile (an invalid pattern makes every writing
command refuse: error). A parse error's fix says to correct the file
(the message names the key) or move it away and run `seldon init`; an
unreadable file is a config error with a `chmod` fix, not a bare exit
2. When `config.toml` cannot be read or parsed, the logbook path it
names is not known: the `logbook` row is "not checked: config.toml is
invalid" (or "cannot be read"), `"logbook"` is `null`, and doctor exits
1, never 3 with the default path (a path from `--logbook`, `--path` or
`SELDON_LOGBOOK` is still checked; the exit code stays 1, and an `init`
fix starts "after fixing config.toml:"). With an open logbook: `cases`, a
case id in two files (error, the `index --check` rule); `ledger`, lines
that are not events, per month with the count and the first line
numbers (degraded: every reader skips them); `rules` (WP-100), the
rules block of `AGENTS.md` against this engine's in the logbook's
language: `current (v2)` ok; `outdated (v1)` (no block, a file from
before ADR-0027), `outdated (vN)`, `outdated (v2, its text differs …)`
and `missing`, degraded with the fix `seldon rules update` (for the
changed v2 block: "(archives your copy)"); `invalid (not UTF-8)`,
degraded, fix `seldon rules update --replace`; a damaged block degraded
with the fix to restore the marker lines or run
`seldon rules update --replace`; a newer block degraded, fix: update
seldon; `fences`, the generated
fence of `STATUS.md` or `DECISIONS.md` that `status` leaves alone (no end
marker of its own, or `STATUS.md` with the header but without the
fence; degraded, the fix names the marker lines), an end marker
that closes no fence (degraded), and a file that cannot be read as text
(error: `status` stops on it); `collectors`, every enabled collector
whose last capture failed according to `cursors.json` (only when the
cursors belong to this logbook), with its message and the fixes the
collectors stored (degraded). doctor cannot tell an intact fence from
one whose own end marker was removed while a later end marker remains:
the writer then takes the text up to that marker as the fence body and
replaces it. The row says so when it finds a stray end marker; with only
one end marker left, nothing in the file shows it. `state`:
`cursors.json`, `manifest.json` and `owned.json` in the state directory
parse (missing is fine); each corrupt or unreadable one is an error row
with what it breaks and the fix (`mv <file> <file>.bad`, or `chmod` for
an unreadable one). A corrupt `manifest.json` or `owned.json` is read as
empty by the collector; the row says that the next capture records it as
a state reset (§3 `capture`, WP-081) when the ledger holds config events.
A second `state` row, degraded, appears while the ledger's newest
`state-reset` note is as new as the newest `lastRun` in `cursors.json`
for this logbook (the last capture recorded it): it names the sources and
files from the note's `meta`, and its fix is to restore a backup of the
state directory and run `seldon capture` (without a backup, the next
capture clears the row); with `logbook` among the files the fix says that
nothing can be restored, because the state belonged to another logbook
path. A `state` row, degraded, also appears *before* the capture that
would record a state reset (WP-083), while a restored backup still
prevents the gap: doctor applies the capture's own rule to
`cursors.json` as it is (`capture::pending_reset`). Each collector a
plain `seldon capture` would run (enabled in `config.toml`) whose cursor
for this logbook is missing or does not read as its cursor (the
collector's `cursor_reads`) would take a baseline; that baseline passes
the same binding gate (no binding: `cursors missing`; another logbook:
`bound to another logbook`; this logbook: only a cursor that is there
and does not read, `cursors unreadable`, or a `pendingBaseline` mark,
below)
and the same ledger rule (at
least one event of its source). Message `the next capture will record a
state reset for <sources>: cursors missing in <state dir>, so changes
made since the last capture may not be recorded` (or `cursors unreadable
in <state dir>`, or `cursors in <state dir> bound to another logbook`);
fix: restore the state directory from a backup now (user guide), or run
`seldon capture` to accept the new baseline (`seldon --logbook <path>
capture` when doctor's logbook came from `--path` or `--logbook`); for
another logbook, that there is nothing to restore. The rule leaves out
the sources in `silentBaselines` for this logbook, as the capture does
(§3 `capture`, WP-104). A loss whose `state-reset` note the
ledger already holds at a time in `pendingNotes` (a capture recorded it
and stopped before saving its state, WP-099) is not in this row: the
next capture does not record it again, it only warns, so doctor gives a
second row with the same reason and fix (WP-104): `the next capture will
warn of the state reset for <sources> that a capture recorded before it
stopped without saving its state: cursors missing in <state dir>, so
changes made since the last completed capture may not be recorded`. No row for a fresh
logbook (no event of a collector source), for a collector that never ran
successfully here, when `cursors.json` cannot be read (its error row), or
when the ledger cannot be read (the `ledger` row). Not predicted: a
collector that degrades in that capture takes no baseline (its baseline
waits, `pendingBaseline`), so the row can name more collectors than the
note; `manifest` and `owned` losses show in their error rows. After that
capture the row is gone and the reset row takes over. A collector whose
baseline waits (enabled, cursors bound to this logbook, marked
`pendingBaseline` because it degraded or was not run in the capture that
lost its state) has its own degraded `state` row instead (WP-091):
`<collectors> degraded or not run since a state reset (cursors missing or
unreadable in <state dir>); its next successful capture records the gap,
so changes made in between may not be recorded`, with `the state in
<state dir> belonged to another logbook` for a `logbook` mark (each
collector with its own reason when the marks differ); fix `run seldon
capture --source <collectors> once it can run (a degraded collector: the
collectors row's fix first)`. A capture clears it only by running the
collector successfully; a restore is not offered, as the other
collectors took their new baseline already. The `omarchy`
and `snapper` probes run the programs the collectors run
(`SELDON_OMARCHY_VERSION`, `SELDON_SNAPPER`).

doctor's `watch` and `drift` rows (ADR-0028 §4c, §4d, §6; WP-109; last,
so the earlier rows keep their places, and only with a readable
`config.toml`): `watch` is `ok` when `watchPaths` holds every default
path, `ok` with "the next capture adds …" when it is an earlier engine's
default list, and `degraded` for a list of the user's own that lacks
default paths, naming them, with the fix `add to watchPaths in
config.toml: "…", …`. `drift` prints the effective `[drift]` set
(`attention`, the `routine` rule ids, the counts of `routinePaths`,
`routinePackages`, `alwaysRedPaths` and `alwaysRed`) and ends in `all
defaults` or `non-default: <keys>`: the config can silence rules, so the
change is shown, never refused; an unknown routine rule id is
`degraded` with the list of valid ids. `--json` carries the full set as
`drift`.

```
seldon capture --json  → {"ok":true,"logbook":"<path>","written":N,"files":["ledger/2026-10.jsonl"],
                          "collectors":[{"name","enabled","ran","ok","events","message"?,"fix"?}],
                          "sinceIgnored":[…],"explainedOwn":N,"explainedSelf":N,
                          "watchPathsAdded":[…],"warnings":[…]}
                                                                    # explainedOwn: §5 rule 7;
                                                                    # watchPathsAdded: §4 config;
                                                                    # explainedSelf: §5 rule 8;
                                                                    # warnings: the state reset (WP-081)
                         exit 0 also when a collector is degraded (ok:false + fix, ADR-0026);
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
plan step so agents identify themselves; without `--actor`, `plan`, `log`,
`drift` and `event` take `$SELDON_ACTOR` (WP-096, ADR-0027 §5; `seldon agent
start` sets it), checked like the flag (`plan`, `log`, `drift`: human or
`agent:<name>`; `event` also `system`) before anything is read or written: a
value it refuses is exit 1 naming the variable and the allowed form, an empty
value counts as unset, and with `--actor` the variable is not read. `event`
takes the variable after the ledger attribution of a hook-recorded change
(the agent command found names actor and case, as before) and only while the
actor is still `system`; `log`'s one-line rule holds for an agent from the
variable too; `log --tag T` stores `meta.tags`
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
                        never the prompt), actor, case, cwd, previousActiveCase}; exit 1 for a case that is not
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
                        restored on failure; no ledger event (WP-022). The launcher runs with
                        SELDON_LOGBOOK, SELDON_ACTOR=agent:<launcher name> (`actor`; the name
                        lowercased, every run of characters other than a-z and 0-9 one `-`, none at
                        either end, so `default` → agent:default, `Claude Code` → agent:claude-code;
                        a name with nothing left is exit 1 before anything changes) and
                        SELDON_ATTENDED=1, replacing the caller's values (WP-096, §8).
```

`capture` selection: no flag or `--all` = every collector enabled in
`config.toml [collectors]`; `--source a,b` = exactly those, even if disabled.
Baseline: a collector without a cursor emits only events at or after the
logbook's `created` (or `--since TS`); diff collectors record their first
state silently. `--since` has no effect on a collector that already has a
cursor (one notice line in human output). Diff collectors check the
ledger on every capture (the config collector by replaying the config
events after its cursor's marker onto the cursor's generation, §4), so
a restored older state directory or a failed cursor save never
duplicates events; a lost `cursors.json` is a baseline (state reset,
below). `capture` runs the shared attribution pass before the append
(ADR-0017), then rebuilds the index (CONTRACT rule 2); the commit helper
and reconciliation (WP-008) follow.

State reset (WP-081). A collector that takes a baseline because its state
was missing or unreadable says which state file it missed: `cursors` (no
cursor for this logbook in `cursors.json`, or one that does not read as
its cursor), `manifest` (config: `manifest.json` missing, corrupt, or
without the generation the cursor names) or `owned` (config: `owned.json`
corrupt). `capture` turns `cursors` into a loss by what `cursors.json` was
bound to before the capture: to no logbook (no file: a new or lost state
directory) → `cursors`; to another logbook (`bind` drops its cursors) →
`logbook`; to this logbook → only a cursor that is there and does not read
is a loss, while a collector without one (every run degraded so far,
disabled until now, never selected) never ran successfully here and takes
its first baseline without a note, even when the ledger holds events of
its source that the theme hook or an agent wrote. When the ledger already
holds at least one event of the losing collector's source, the changes
since its last capture may be lost, and the capture appends, with its
other events, one `note` with `source: seldon`, `actor: system`, subject
`state-reset`, detail `state directory missing, unreadable or bound to
another logbook: new baseline for <source> (<files>), … at <baseline>,
recorded <capture time>; changes made in between may not be recorded`
(`<baseline>` is the logbook's `created` or `--since`, as for every
baseline), and `meta.sources` / `meta.files` (comma lists in run order and
in the order logbook, cursors, manifest, owned). A collector whose source
the ledger holds no event of (the first capture of a logbook) takes its
baseline without a note. Known limitation: after `init --no-capture`
there is no `cursors.json`, so when the theme hook or an agent writes an
event of a collector's source before the first capture, that capture
records a state reset that lost nothing. The next capture continues from
the new state, so a loss is recorded once. A crash between the ledger
append and the `cursors.json` save repeats neither the collector events
(the collectors compare with the ledger) nor the `state-reset` note and
the snapper access note (§4) (WP-099): a capture that appends such a note
first saves `cursors.json` as it loaded it (the same binding and
entries, or no logbook and no entries when there was no file) with the
note's time added to `pendingNotes`, and its save after the append
writes the new state without `pendingNotes`. The next capture after a
crash between the two loads that state (same binding, same entries)
and does not write a note the ledger holds at a time in
`pendingNotes` again, known by its identity, not its detail: the
snapper note by its subject, the state reset by the sources its
`meta.sources` names (a source no such note names, e.g. one the crashed
capture did not run, gets a note of its own). It still prints the
warning, which the crash hid, and keeps the earlier times when it marks
a note of its own. A collector such a note names is not waiting in that
capture (below): it is not marked `pendingBaseline` when the capture
does not run it or it degrades, and when it is not run, a mark it had
before the crash is cleared (an entry with only the mark is dropped), as
its gap is recorded; its next successful run takes a silent baseline, as
the collectors the crashed capture ran do. The same save, also when there is no note, adds to
`silentBaselines` under the logbook's canonical path the sources whose
baseline the capture takes or leaves waiting (below) without a note
because the ledger holds no event of them, in run order and after the
names already there; the save after the append writes the new state
without `silentBaselines`. The next capture of that logbook leaves these
sources out of the ledger rule, so the first events the crashed append
wrote of them are no loss, and it records no state reset that lost
nothing (WP-104); marks under another logbook's path count only for
that logbook. A crashed state reset's `recorded` time is the crash time,
while the baselines are saved by the next capture. A collector
that degrades in
a capture in which its state was lost (it would pass the binding gate
and the ledger rule above) takes no baseline there, so that capture's
note does not name it, or there is no note when it alone lost its state;
its entry in `cursors.json` keeps `pendingBaseline` with the kind the gate
gave (`cursors` or `logbook`, the note's `files` word; written only while
set, older files read as not waiting) while it degrades or is not run,
and its first successful run takes the baseline and records it: a
`state-reset` note naming it, files as recorded in the mark, `cursors` or
`logbook` (with `logbook` the warning says that nothing can be restored),
then the mark is cleared (WP-088). The same holds for a collector that
the capture which loses the state does not run (`--source` without it, or
disabled in `config.toml`) and that has no entry in `cursors.json` as
bound for that capture (no file, or `bind` dropped another logbook's): it
passes the same binding gate and ledger rule, and the capture writes an
entry with only the mark (`ok: true`, `events: 0`, no `cursor`, no
`lastRun`); its first successful run, also after it is enabled again,
records the gap the same way. A collector not run that has an entry
(bound to this logbook) keeps it as it is: a cursor that does not read is
recorded by its own next run (WP-091). A corrupt `owned.json` is moved
to `owned.json.bad` by the capture that runs the config collector
successfully (a newer one replaces an older `.bad`). The capture warns:
`warning: state reset recorded: <sources> took a new baseline because
<state dir> was missing, unreadable or bound to another logbook, …` on
stdout (and in `--json` `warnings`), naming the user guide's restore steps
(07 "Back up and restore the state directory"), or, for `logbook`, that
nothing can be restored (07 "Moving or copying the logbook"); `hook
session-stop` prints it on stderr as `seldon: warning: …` (§8). A corrupt
`cursors.json` still fails every capture (exit 2) until it is moved away.

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
theme, config. A run that takes a baseline because its saved state is
missing or unreadable names the state file it missed (`cursors`,
`manifest`, `owned`), so that `capture` can record a state reset (§3,
WP-081). `seldon dossier` (§3) is not a collector and writes no
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
  and the drift routine rule (§5): the intent of a hook `command` event is
  read from its recorded line with the hook's own shell parser (quotes,
  wrappers, `sh -c`, heredocs), so a line the hook records as a package
  command is the line attribution reads one from; the parser's limits are
  listed in §8 (WP-071). Rotation: the tail of `<log>.1` with the old
  inode is read first, then the new file from 0; a rotation to another
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
  `meta.pairOf`. The cursor keeps each snapshot's number, type,
  description and date; a number that disappears becomes a
  `snapshot-delete` at capture time. snapper gives a new snapshot the
  number after the highest one, so a number comes back when the newest
  snapshot is deleted before the next one is made: a known number with
  another date becomes a `snapshot-delete` of the old snapshot (its
  description) and a `snapshot` of the new one, both at the new
  snapshot's date, the deletion first. A cursor entry without a date
  (written before WP-073) gets one without an event. A snapshot has one
  date, an instant: the info files give it in UTC, the list in local
  time, which names the instants that show that time on the wall clock:
  two in the repeated hour when summer time ends (in Central Europe
  02:00:00 to 02:59:59; 03:00:00 is one instant), none in the hour
  skipped when it begins (02:00:00 to 02:59:59), else one. For a list
  time with two the first of these decides: the
  snapshot's info file when it can be read (the canonical date), the
  date the cursor knows for the number when it is one of the two, the
  number order (when the earlier instant lies before the date of the
  snapshot numbered before it, the later), else the earlier instant. A
  known number whose date is the other instant of the same local time is
  the same snapshot and takes the new date without an event, so a switch
  between list and info files never records a `snapshot-delete` plus
  `snapshot` for it, and a cursor entry written before WP-082 (which
  held the later instant) moves to the info file's date the same way.
  The ledger dedupe counts either instant as recorded, so a cursor save
  that failed or a lost state directory before such a switch does not
  record the snapshot twice. A list time with no instant: the snapshot
  is known without a date and without an event until a later read gives
  one (WP-082). For a
  user the snapper config does not list, the command fails with a
  permission error; the collector
  then reads the snapshots from the info files
  (`/.snapshots/<number>/info.xml`, `SELDON_SNAPSHOTS_DIR`; under
  `SELDON_TEST_GUARD` without it `<guard>/.snapshots`) with the same events
  and cursor, so switching between list and info files adds no events (an
  info file that cannot be read is skipped and named in the message; its
  snapshot is neither new nor deleted). When the info files cannot be read
  either, or none is found (an empty directory, e.g. right after booting
  into a snapshot), it reports `ok: false` and the fix command, the read
  grant `sudo setfacl -m u:$USER:rx /.snapshots`, writes no events and
  keeps the cursor, never sudo (ADR-0026); `NO_PERMISSIONS` points at that
  grant. `doctor` and `init` say what it grants: read access to the
  snapshot directory listing and the snapshot info files (files inside a
  snapshot keep their own permissions), no snapshot creation, change or
  deletion. When listing works, `doctor`'s `ok` message adds that this
  user may use the snapper config, which also lets it create, change and
  delete snapshots without a password. When `snapper -c root get-config`
  then succeeds and its `ALLOW_USERS` names the current user (`USER`, else
  `LOGNAME`; the old opt-in of ADR-0011), the row stays `ok`, its message
  says so, and its fix is the revert followed by the read grant:
  `sudo snapper -c root set-config ALLOW_USERS="" SYNC_ACL=no && sudo
  setfacl -m u:$USER:rx /.snapshots` (`SYNC_ACL=no` so that a later
  `set-config` does not sync the grant's ACL away; the revert may drop
  the opt-in's ACL entry, so the grant comes second); `init` prints that fix as a
  recommended next step. snapper is run with `LC_ALL=C` (and without
  `LANGUAGE`), `list` and `get-config` alike; its messages are matched in
  English, whatever the user's locale (`doctor` and `init` use the same
  argv and locale). When the collector's `ok` differs from its last run
  for this logbook (its entry in `cursors.json` as bound for the capture,
  with a `lastRun`), the capture that runs it appends one `note` with
  `source: seldon`, `actor: system`, subject `snapper`, no case, so that
  granting or removing the read access (or a `SYNC_ACL` rewrite that
  removed it) shows in the history (WP-091). Detail: `snapper collector
  degraded: <message>; at its last run it was ok`, or `snapper
  collector ok again (<message>); at its last run it was degraded:
  <earlier message>` (without `(<message>)` when the run has none, e.g.
  through `snapper list`). Any degraded state counts, not only
  `NO_PERMISSIONS`. No note on the first run for a logbook, after a lost
  state directory or another logbook's state (nothing to compare with),
  for an entry with only the `pendingBaseline` mark (it never ran), or
  in a capture that does not run snapper; the next capture compares with
  the saved state, so no change is recorded twice, also after a crash
  between the append and the cursor save (§3 state reset). Known
  limitation: a change that flips back before the next completed
  capture after such a crash leaves the crashed capture's note and none
  for the way back. The note is no drift and no own change (§5).
- **omarchy** — version from `omarchy-version` (prints e.g. `4.0.4-1`;
  `omarchy --version` does not exist and `$OMARCHY_PATH/version` is
  stale); `repoHead` (7-character short hash, like `logbook.git.head`)
  only when `$OMARCHY_PATH` is a git checkout, omitted on package
  installs; change → `update` event with `from`/`to`. Omarchy's programs
  (`omarchy-version` here, `omarchy plugin …` below, the same calls in
  `doctor` and `dossier`) get `OMARCHY_PATH=/usr/share/omarchy` in their
  own environment when the engine's is unset or empty, as under ssh,
  cron or a systemd unit (`omarchy-shell` refuses to run without it); a
  set value is passed on unchanged, like `PATH`. The engine's own
  environment is never changed (WP-089).
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
  theme and config, so the ADR-0017 window can match the agent's command;
  `plugin-remove|enable|disable` get the capture time (the attribution
  window below reaches back to the last check for them). Events the
  ledger already holds since the cursor's last check (same kind, id,
  version, enabled state and `from`/`to`) are dropped, so a capture
  whose cursor save failed after its ledger write repeats nothing
  (WP-073). A non-zero exit, a timeout, non-JSON output or an empty list
  after a non-empty one → degraded, cursor kept.
- **theme** — current theme name (read how Omarchy stores it from
  `~/.local/share/omarchy/bin/omarchy-theme-set`); diff → `theme-set`.
  Optional hook script installed by the wizard into `theme-set.d/` calls
  `seldon event theme theme-set --subject "$1"`.
- **config** — sha256 manifest of files under `watchPaths` (default
  `~/.config/hypr`, `~/.config/omarchy` excluding `plugins/`, `~/.config/
  waybar` if present, `~/.bashrc`, `~/.zshrc`, `~/.local/share/
  applications` excluding `mimeinfo.cache` (WP-089: `update-desktop-database`
  rewrites it on many package transactions; the `.desktop` files it is
  built from are watched), and the persistence paths of ADR-0028 §4d
  `~/.config/systemd/user`, `~/.config/autostart`,
  `~/.config/environment.d`, `~/.config/uwsm`, `~/.profile`,
  `~/.bash_profile` (WP-109; `init` writes all twelve into a new config;
  user list). **Upgrade (ADR-0028 §4d):** a `config.toml` whose
  `watchPaths` equals the default list of an earlier engine, in any order
  (0.1.0–0.1.3: the first five; WP-089: the first six), gains the current
  defaults it lacks at the next `capture`, which saves the file under the
  lock and says so once (`note: config.toml now also watches …`,
  `watchPathsAdded`); the files already there enter the scope without
  events (scope changes below). A list the user wrote is never widened:
  `doctor`'s `watch` row names the default paths it lacks, with the line
  to add. **Evidence marks (ADR-0028 §5):** a new `config-add` or
  `config-change` carries `meta.matches` when, at capture, the file is a
  symlink whose target lies under `/usr/` (`system-link`), its new hash
  equals Omarchy's shipped copy (`omarchy-default`: `$OMARCHY_PATH/config/
  <rel>` for `~/.config/<rel>`; for `~/.local/share/applications/<name>`
  `$OMARCHY_PATH/applications/<name>`, and for `Alacritty.desktop`
  `$OMARCHY_PATH/default/alacritty/Alacritty.desktop`), or it lies in a
  theme directory `~/.config/omarchy/themes/<slug>/` that has a `.git`
  (`theme-repo`: `omarchy theme install` clones there and strips a
  theme's code). Only the fact is recorded, never the link target or the
  content; old events have no mark and classify by path. `OMARCHY_PATH`
  defaults to `/usr/share/omarchy` (under `SELDON_TEST_GUARD` without
  the variable: `<guard>/omarchy`); the files there are only read and
  hashed. Changed/added/
  removed → `config-add|config-change|config-remove` with the path written
  with `~` and both hashes (`detail` `sha256 <8> → <8>`). Binary files (a
  NUL in the first 8000 bytes) and files over 1 MiB are listed as
  `skipped` without a hash, so growing past the limit is not a removal;
  `.git` directories are never walked, symlinked directories are not
  followed, `~/.config/omarchy/plugins/` and
  `~/.local/share/applications/mimeinfo.cache` are excluded wherever the
  watch paths reach them. Known secret-bearing
  files are listed in `config.toml [redaction] skipPaths` and are never
  opened, hashed or named (pattern rules in §7). A file whose `~`-path
  holds a control character or is longer than 512 characters is not
  watched; the collector's message counts them. Event `ts` is the file's
  mtime clamped to [last check, now]; a removal gets the capture time.
  Watch paths resolve as §2 says (a relative one under the home
  directory). A file whose size, mtime, ctime and inode all match the
  manifest's `stats` keeps its stored hash and is not read (WP-069; the
  ctime catches an in-place edit whose mtime was put back, `touch -r`). **Scope
  changes (WP-069):** before the diff, the base generation drops every
  file the current scope does not reach (under no watch path, excluded,
  or matching `skipPaths` itself or through a folder), and, when its own
  `scope` is known, takes every scanned file its scope did not reach as
  it is now: a narrowed scope writes no `config-remove`, a widened one no
  `config-add`, and the message says `watch scope changed: N file(s)
  left it, M entered it; no events for them`. **Replay
  (WP-069, WP-073, WP-103, WP-107):** the config events the ledger holds since
  the cursor's check are first applied to the cursor's generation, each
  to a file whose state is its `hashFrom` (for an addition, no file), so
  the capture repeats nothing and a file that went back to its old
  content before it gets its `B→A`. The ledger holds subjects redacted,
  so files whose names differ only in a masked part share one: an event
  can go to each such file of the cursor's generation and of the
  current one (the manifest's when the cursor is behind it, that is,
  the last capture's cursor save or ledger write failed; else the
  scan). Of several that fit, it goes to the one the current generation
  has in the state the event leaves; one that fits several alike (same
  content) waits until no other event can be placed, then takes the
  first. A subject that no file has names its own path when the
  redaction leaves it as it is; a masked subject does, as it masks to
  itself (a removal the ledger lost is then recorded under it). The
  replay runs on every capture that has a cursor and its generation;
  when the cursor is not behind, the scan stands in for the current
  generation, so a restored older state directory records only what
  changed since (guide 07). It reads every config event stamped after
  the cursor's check and, of those stamped with exactly that time, the
  ones after the cursor's marker: a removal carries the capture time,
  and a change whose file has an older mtime (`cp -p`), or one made in
  the second of the check, is stamped with it, so the cursor's capture
  and later ones both write events at that time. The marker (`atCheck`,
  WP-107) counts the config events stamped with the check that the
  ledger holds, in ledger order, once that capture's own are written. An
  event has no id before the append, and the capture holds the state
  lock from its ledger read to its append, so the count ends at the
  capture's last event at that time (or the last before it). The count
  assumes two things: the events of one instant are read in the order
  they were written, which fails only when two captures stamp it with
  different UTC offsets across a month boundary (separate month files;
  a time zone change within that second plus a failed cursor save can
  then record an event twice); and every line at the check reads the
  same later (a later engine that drops such a line shifts the count by
  one). A cursor without a marker (saved before WP-107, or by a capture
  that could not read the ledger) reads as WP-103 did, for that one
  capture: when behind, from the check on without the removals stamped
  with it; when not, strictly after it.

All events get `actor: system` unless the collector can prove otherwise.
Proof is an agent hook `command` event that (a) named the subject
(package, path, or a full upgrade — `-Syu`/`-Su`/`omarchy update` — for
every member of the resulting transaction) and (b) precedes the collector
event within the same pacman transaction or by at most 10 minutes; then
the collector event inherits that command's `actor` and `case`. Time
proximity alone is never proof (ADR-0014 §1). An event stamped with the
capture time (`plugin-remove|enable|disable`, `config-remove`) happened
somewhere after its collector's last check, so for it the command may
start from 10 minutes before that check up to the capture (WP-073); with
the default 15-minute capture interval a fixed 10 minutes before the
capture time would miss about a third of them. Such a command must also
be later than the newest recorded event of the same source and subject:
a command that came before the last recorded change cannot have caused
a newer one, so an agent's `disable` that proved one disabling does not
claim a person's later disabling of the same plugin. For plugins events the
command's verb must match the kind: `omarchy plugin <verb> <id>` proves
only `plugin-<verb>` of that id, and `add` also `plugin-enable`
(WP-073).

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
4. Otherwise, if the event is drift-eligible (only `pacman`, `omarchy`,
   `plugins`, `theme` and `config` events can be; snapshots, notes, hook
   `command` events and case events never are, ADR-0012 §6), it is
   *linkable* (no case, no resolution) and gets a **class** at index time
   (ADR-0028 §2, normative; computed from the event, the ledger around it
   and `[drift]`, never written to the ledger): *routine* — history in
   the Changelog, not drift, no reason ever asked; *attention* — open
   drift, quiet; *crisis* — open drift and the bar's signal. The rule
   ids (`drift show`): pacman in a plain full upgrade (argv `-S` with
   `-u` naming no package, pacman, yay or paru: `-Syu`, `-Syyuu`, `-Su`,
   bare `yay`, Omarchy's `pacman -Syu --noconfirm --overwrite …`) —
   `upgrade`, `reinstall`, `install` and a removal (`:: Replace`) are
   `sysupgrade`, `alwaysRed` subjects included; a downgrade or removal of
   an `alwaysRed` member is attention `sysupgrade-red`, another downgrade
   attention `downgrade`. Named (explicit: on the command line, or
   `-U <file>` from the cache): `upgrade`/`reinstall` routine `upgrade`
   (`alwaysRed`: attention `upgrade-red`); `install`/`remove`/`downgrade`
   attention `package` (`alwaysRed`: **crisis** `always-red`). A
   transaction naming only `routinePackages` is routine `keyring`. A
   dependency follows the highest class of its transaction's explicit
   members (also when they are resolved); a transaction without a command
   line is attention `other`. Omarchy `update`: routine `omarchy-update`
   when both versions are package-shaped (`N…-N`) and a plain full
   upgrade moved `omarchy` or `omarchy-dev` (install or upgrade) to the
   new version at most 31 days before; else attention `omarchy-other`
   (a bare `dev`, a downgrade, unattributed). Plugins: `plugin-enable`/
   `-disable` routine `plugin-toggle`; `-add`/`-remove`/`-update`
   attention `plugin`. `theme-set` routine `theme`. Config, in this order:
   `meta.matches` `omarchy-default`/`system-link` routine (not for a
   removal); `routinePaths` routine `routine-paths`; any other
   `config-remove` attention `config-remove`; `alwaysRedPaths` **crisis**
   `always-red-paths` (a `*.sample` file under `~/.config/omarchy/hooks/`
   is not: `omarchy-hook` never runs it); in a theme directory
   `~/.config/omarchy/themes/<slug>/`, `meta.matches = theme-repo`
   routine `theme-repo`, a file that is not code (`*.lua`,
   `alacritty.toml`, `foot.ini`, `ghostty.conf`, `kitty.conf`,
   `vscode.json`) routine `theme-assets`; `~/.config/omarchy/backgrounds/
   **` routine `theme-assets`; everything else (Hyprland Lua, waybar,
   `~/.config/omarchy/**`, `themed/*.tpl`, the menu extensions, shell rc,
   desktop entries) attention `config`. Any other event: attention
   `other`. Rules 1–3 come first: a linked event is no drift, and a
   routine event an open case's Plan names (rule 3) is shown as
   attention with its `proposedCase`.
5. **Grouping (ADR-0013).** Linkable `pacman` events that share a `txId`
   form one item keyed by the leader's event id (lowest-id explicit
   member, else lowest-id member); the index row carries `txId` and
   `members`. The item's class is the highest class of its members, its
   rule the leader's (else the lowest id's with that class). Other
   sources are never grouped. `drift[].zone` is the leader's **ledger
   zone** (pacman items are red, ADR-0014; ADR-0028 §7).
6. `crisis: true` iff the item's class is crisis (the harm test,
   ADR-0028 §1); open drift (`index.drift`, `summary.openDrift`) is every
   item that is not routine. `[drift] attention = "all"` restores the
   rules before ADR-0028 (every linkable item open, pacman zone yellow
   iff every member is routine in the ADR-0013 §3 sense, `crisis` iff
   red).
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
   **Built-in templates (ADR-0028 §2, WP-109):** without a record, a new
   `config-add|config-change` is also explained when its new content is
   one of the engine's compiled-in templates: a file named
   `seldon-theme-set.sh` whose hash is the theme hook's (detail
   `installed by seldon init --theme-hook (built-in template)`), and a
   file named `seldon-watch.service` whose content (read again and
   checked against the event's hash) is `engine/systemd/
   seldon-watch.service` with any `ExecStart=<prefix>/seldon watch` line,
   the prefix without whitespace (detail `installed by install.sh --unit
   (built-in template)`); so a lost state directory, `install.sh --unit`
   and its `systemctl --user enable` link are no crisis. A unit that
   differs in any other line is not explained. An unreadable `owned.json`
   is kept and only the templates explain.
8. **Seldon updating itself (WP-086).** Seldon's own components are no
   drift: an event of its plugin `jax.seldon` (`plugin-update`,
   `plugin-enable`, `plugin-disable`) or of its package `jax-seldon`
   (pacman `upgrade`, `reinstall`) without a case gets, from the capture
   that writes it, right after the append and under the same lock, one
   `explained` resolution: `source: seldon`, actor `system`, no case,
   detail `seldon's own plugin` or `seldon's own package` (the ids are
   `attribution::OWN_PLUGIN` and `OWN_PACKAGE`, the plugin manifest's `id`
   and the PKGBUILD's `pkgname`). The event keeps the actor the collector
   or attribution gave it and stays in the Changelog; the index folds the
   resolution (ADR-0021: no case) and a pacman group loses that member
   only. Unlike rule 7 there is no hash, only the id, and nothing checks
   where the code came from; so only changes to a Seldon that is already
   there are explained: `plugin-add` and pacman `install` (somebody
   (re)installing Seldon while a logbook exists), `downgrade` (somebody
   choosing an older one) and the removals (`plugin-remove`, `remove`)
   stay drift. The rule reads the whole ledger, so every capture also
   catches up on own changes an earlier capture left open (the second
   append failed, the engine stopped between the two appends, or a
   version before rule 8 wrote them; WP-088): each such event without a
   case that no resolution line refers to gets the same `explained`
   resolution, and `explainedSelf` counts both. The resolution is dated
   at the capture time or the event's time, whichever is later, so the
   index folds it even when the clock moved back. When the ledger cannot
   be read after the append, only the capture's own events are explained
   and a warning says the earlier ones were not checked. An event that has a
   resolution (dismissed, explained, linked) keeps it, the kinds above
   that stay drift stay drift, and no case is created. `install.sh` writes under its prefix
   (`~/.local`: the binary, man page, completions), outside the default
   `watchPaths`, and, with `--unit`, into `~/.config/systemd/user/`, a
   default watch path since ADR-0028 §4d: rule 7's built-in template
   explains that unit.

   Events explained by rule 7 or 8 count in the weekly drift trend
   (`series.drift`, §6) like any resolved item whose event opened one:
   opened in the week of the event, resolved in the week of the capture,
   mostly the same day.

Resolution events (`kind: resolution`, `refersTo`) are applied when the
index is built; an event with a resolution is not drift and keeps it,
whatever its class. `seldon drift link|explain|dismiss <id>` on a group
member resolves every member that is linkable at that moment (`link`;
`explain|dismiss` refuse a routine item, §3), one resolution line per member in one write with
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
`system/*.md` get the same, the rows kept from the old body included
(WP-075).

Drift classes (ADR-0028 §5; WP-109): the class of every linkable event
(§5 rules 4–6) is computed on each build from the ledger and
`config.toml [drift]`; nothing is written to the ledger, and two builds
from the same ledger and config give byte-identical indexes. Routine
items leave `index.drift`; `drift[].zone` is the leader's ledger zone;
`crisis` is the harm test; `summary.openDrift` counts the items that are
not routine, `summary.crisis` the crises. The timeline's `crisis` rows
follow. `series.drift` counts a group as opened in the week of its
earliest line only when its class is not routine (a proposal does not
count here: it depends on today's open cases), and a resolution write as
resolved only when the event it resolves opened an item, so the curve
cannot go negative (an old dismissal of a theme switch counts on neither
side). `[drift] attention = "all"` reproduces the index of the rules
before ADR-0028 (`fixtures/index.attention-all.json`).

Size budget (CONTRACT.md rule 5, < 1 MB; WP-076): in `index.events` and
`index.drift`, a `detail`, `resolutionDetail` or string value of `meta`
that takes more than 256 bytes in JSON is cut on a character boundary and
ends in `… (N more characters in the ledger)`, N the characters left out
(ADR-0025). The ledger line, the `ledger/*.md` views and the member events
of `drift show` keep the full text; `drift list` and the `item` of `drift
show` come from `index.drift` and are clipped. `subject` (at most 512
characters) is never cut. With the counts of rule 4 this bounds both
sections: 500 events and 200 drift items with 4096-character texts come
to about 520 KB. No field marks the cut (`meta.truncated` stays reserved,
ADR-0020, ADR-0025). Open cases, decisions and
memory topics are not capped: an index of 1 000 000 bytes or more makes
`index` and `status` warn (`warnings`, stderr) and name the largest
section.

Performance budget: 10 000 events, 300 cases, 365 journal files → < 100 ms
warm. `cargo bench --bench index` (`just bench`, CI) asserts the index
build in-process on the fixture logbook scaled ×10 and prints ×150 (10 650
ledger lines, 1 200 cases); `just check-perf` (opt-in, quiet host) asserts
×150 too (`SELDON_BENCH_X150=1`) and `seldon status` at 10 044 ledger
lines, 304 cases and 365 journal files, median wall time of 11 runs,
process start included. A median over budget is measured once more before
a check fails (release, 2026-10-04 on the dev host: ×150 build 80 ms,
`status` 47 ms; WP-076).

## 7. Redaction

Before writing any event, `subject`, `detail` and every string value of
`meta` are passed through redaction. The commands that write free text
into the logbook (`log` with its tags, `plan new` and the `plan` step
reasons, `decide`, `drift explain|dismiss`) pass that text through the
same redaction before
the first write, so the ledger, the journal, case and decision files,
`STATUS.md` and the index hold the same redacted text (WP-062). The
`seldon` notes (§3 state reset, §4 snapper) are events and are redacted
the same way (WP-099). A collector's message (snapper's stderr, for
example) goes through the same redaction once, before the capture saves
it in `cursors.json`, prints it (`capture` and `capture --json`) or
embeds it in the snapper note. A capture also redacts the messages an
older engine saved in `cursors.json` when it loads the file (a message
that already holds `‹redacted›` is left as it is, so a user pattern that
matches across the marker does not grow it on every capture). The index
build and `doctor` (its `collectors` row) redact each message they read
from there again, so `index.json` (`state.collectors`), `STATUS.md` and
`doctor` show none unredacted; `doctor`'s `omarchy` and `snapper` probes,
whose rows `init` prints too, redact what the program printed the same
way. While `config.toml` cannot be parsed or a `[redaction] patterns`
entry is invalid, these show `collectors::MESSAGE_WITHHELD` in place of
the message instead (WP-105).
The rules
(`redact::BUILTIN`, in this order): URLs with userinfo; `--password`
(also wget's `--http-password` and `--ftp-password`);
`--token`, `--with-token`, `--secret`, `--client-secret`, `--passphrase`,
curl's `--pass`, `--proxy-pass` and `--oauth2-bearer`, xh's `--bearer`
and similar options; the `pass:…` value of an option whose name holds
`pass` or `secret` (openssl's `-pass`, `-passin`, `-passout`,
`-password`, `-passcerts`, `-keypass`, `-secret`, …, also with `=` and
two dashes, `--passin=pass:…`, as easyrsa writes it): the whole value,
`pass:` included; a pass phrase source (`env:`, `file:`, `fd:`,
`stdin`) names where the secret is and stays (`openssl-pass`, WP-106;
its trigger is `pass:`); `--api-key`, `--access-key`, `--secret-key`;
`token=`; `…SECRET=`, `…PASSWORD=`, `…PASSWD=`, `…PASSPHRASE=`, `…_PWD=`,
`…_PASS=`, `SSHPASS=` assignments (also `PGPASSWORD=`); `…KEY=`
assignments (also `?api_key=`); the non-empty string value of an inline
JSON key that ends in `password`, `passwd`, `passphrase`, `secret`,
`token`, `api_key` or `apiKey` (`"password": "…"`, `"client_secret":"…"`,
`"openaiApiKey": "…"`, also escaped inside a shell string as
`\"password\":\"…\"`, with white space and newlines around the `:`;
not `"password_hint"` or `"token_type"`); `Authorization:`; headers whose name ends in
a credential word (`X-…-Key:`, `X-…-Token:`, `X-…-Secret:`, `X-Auth:`,
`X-…-Auth:`, `Api-Key:`, `Private-Token:`; not `X-Author:`); a
`Cookie:` or `Set-Cookie:` value on the same line that starts with a
cookie pair `name=` (RFC 6265; not `cookie: banner fixed` or
`Cookie: $COOKIE`);
`(AKIA|ASIA)[0-9A-Z]{16}`; `gh[pousr]_[A-Za-z0-9]{36,}` and
`github_pat_…`; `glpat-…`; `xox[abposr]-…`; `sk-`/`sk_` keys
(`(?-u:\b)sk[-_][A-Za-z0-9_-]{20,}`); anything after `-p ` for
`mysql|psql|smbclient`, up to the end of the command's last continued
line; the value after `curl -u`/`--user`; proxy
credentials: the value after `curl -U` (not `useradd -U`),
`--proxy-user` and wget's `--proxy-password` before a space (its `=`
form is a `…PASSWORD=` assignment), and a `user:pass` without a scheme before the
last `@` of the value after `curl -x`, `--proxy` or a `…proxy=`
assignment (`https_proxy=`, `http.proxy=`; with a scheme it is a URL
with userinfo); the cookies after `curl -b`/`--cookie` when the value
holds a `=` (without one curl reads that file); a client certificate
with its password, the value with a `:` after `curl -E`, `--cert` or
`--proxy-cert` (the long forms also without the command word; the file
name is masked with the password: `cert-password`, WP-097); the value
after `-a` or `--auth` of `http`, `https`, `xh` or `xhs` (HTTPie and xh;
any value, a bearer token included; the command word is followed by a
space, tab, line end or `\` before a line end, so `http://` is none,
and its triggers are those words with that character: `httpie-auth`,
WP-097); after
`sshpass -p`; after `-p` of `docker|podman|buildah|nerdctl|helm registry
login`; e-mail addresses (`email`, WP-093: the local part, the domain
stays: `‹redacted›@example.com`); and user-supplied patterns in
`config.toml [redaction] patterns`.
Replacement: `‹redacted›`; the option, key or header name stays in front
of it (`--proxy-user ‹redacted›`, `"password": ‹redacted›`,
`Cookie: ‹redacted›`). The hook never records stdin/stdout of
commands, only the command line. A name that can only mean a
credential masks any non-empty value: `--password`, `--token`,
`--with-token`, `--secret`, `--client-secret`, `--passphrase`, `--pass`,
`--oauth2-bearer`, not a negation such as `--no-pass`
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
`-pSECRET` and psql's `-p` port are both redacted. The proxy, JSON and
cookie rules (WP-084) check no credential shape, and they take over no
option or key an older rule covers (the import report counts a line once
per rule):
`--proxy http://user:pass@host` stays a URL with userinfo and
`--proxy-password=` a `…PASSWORD=` assignment. URL userinfo that
holds a `:` is cut from `://` up to the last `@` before the next white
space or quote, so a password may contain `/ ? # : @`; userinfo without
a `:` (a bare token) is cut up to the last `@` before the path. Redacting
twice gives the same text for the built-in rules: a match that lies
inside an existing `‹redacted›` is left alone (a user pattern that
matches across the marker's edge is applied as written). Each built-in
rule is compiled once per process, and only when the text holds one of
its literal triggers (`redact::triggers`), checked on the text in lower
case with the Kelvin sign and the long s folded onto `k` and `s`, as
case-insensitive matching folds them (`redact::trigger_text`); a trigger
may join literals with `+` that must all be present, so a `curl` rule
needs `curl` and its option (`curl+-x`; WP-084), or with `>` that must
be present in that order (`://>@`: an `@` after `://`). A trigger with
a capital is checked on the text as written, for a rule whose literals
are case-sensitive (`curl+-U`, `curl>-E>:`: `curl`, then `-E`, then the
`:` of the value; WP-108). The option rules
(`curl -u`/`--user`, `-U`/`--proxy-user`, `-x`/`--proxy`,
`-b`/`--cookie`, `-E`/`--cert`, `http|xh -a`/`--auth`, `sshpass -p`,
`docker … login -p`) look for the option within one command: up to an
unquoted line end or an unquoted `;`, `&` or `|`; a quoted string
(`'a&b'`, `"x;y"`, `\"` inside double quotes, also over several lines),
an ANSI-C string (`$'a;b\''`), a backslash escape (`\;`, and a `\`
before a line end, which continues the command on the next line), a
redirection (`2>&1`, `>&2`, `<&3`, `&>file`, `>|file`) and a quote the
text never closes (an apostrophe in a note; after it the command ends at
the line end) do not end it (WP-097). The hook hands the whole command
line to redaction, its continued lines included (heredoc bodies are cut
before, §8). Quotes pair left to right as written; where the shell reads
them otherwise (quotes inside `"$(…)"`, an escaped space), the plain
reading, up to the first `;`, `&` or `|` (quoted or not) or the line
end, still counts, so the context reaches at least what that plain
reading reaches. White space or a `\` line end may stand between an
option and its value. The value of an option is one shell word: quoted
and bare parts joined together (`admin:'p w'`, `"$U":pw`), `$'…'`, `\"`
inside double quotes and backslash escapes belong to it and are masked
whole. A quoted part ends at a line end that no `\` escapes: a
double-quoted part that never closes as escapes are read is taken up to
the next `"` on its line as written, and a quote its line does not close
(`bob's` in a note, `-u 'admin:pw`) takes the rest of that line only
(WP-097 round 2). The value after `token=`,
`…PASSWORD=` and the other assignments is one quoted or bare part, so a
query `?token=abc` inside a quoted URL stops at the closing quote; a
double-quoted value may hold `\"` (WP-097).
An option given twice in one command is masked
each time (`curl -u a:b … -u c:d`, `-b x … -b y`): the rule scans on
from the end of its previous match, without a second command word,
and compiles the pattern for that only when the text after the match
holds the option as written (`-u`, `-U`, `-x`, `-b`, `-E`, `-a`, `-p`,
or a long form; WP-108);
`sshpass` masks only its first `-p`, as a later one belongs to the
command it runs (`ssh -p 2222`) (WP-087). Not masked (WP-097):
combined short options (`curl -su a:b`, `-sE c.pem:pw`; a trigger is
the option as written, `-u`, and one that every curl line holds would
compile every curl rule for it); abbreviated long options (`--us`); the
last of several `sshpass -p` (sshpass uses the last); a `.netrc` or curl
config file (`curl -n`, `-K`) and its contents shown by another command;
an assignment value that joins quoted and bare
parts (`PASSWORD=a'b'` keeps `'b'`); `--no-pass` and other negations,
which are not `--pass`. Not masked (WP-106): a password given directly
rather than as `pass:…` (`openssl enc -k`, `-srppass`, keytool's
`-storepass`); a `pass:` that quotes or an escape split (`pa'ss':x`,
`\pass:x`), or an option name in quotes; the glued `-passpass:…`,
which openssl rejects. Masked too much, by design: an
option word inside a quoted argument that spans lines (`git commit -m
"…curl…⏎… -U flag"`, as on one line); a stray apostrophe (in the
command's context, up to the line end; an option value with an unclosed
quote ends at the line end); `x264 --pass 1` (`--pass` names a
credential); a certificate's file name next to its password; an option
value at the end of a quoted string (`bash -c "curl -u a:b" && echo
"x"`), which joins the next quoted part as adjacent shell parts would;
the word after `-a` on a line that holds, or follows a line ending in,
`http`, `https`, `xh` or `xhs` as a word followed by white space (`http
redirect … ls -a home`; the gap after the word may be a line end); and a
`\` at the end of a comment, which continues the command for redaction
(bash does not); `pass:…` after any option whose name holds `pass` or
`secret` (`--bypass pass:x`), the empty `pass:`, and the rest of the line
after `$"pass:…"`, which reads as an unclosed quote. A `pass:…` value
after an option another rule also masks (`--pass`, `--password=`,
`--secret-key`, …) is counted under both rules in the import report; the
earlier rule in the order masks it, `openssl-pass` before `key-option`;
the output holds one marker (WP-106). `cert-password` compiles only on
a line holding `curl`, then `-E` as written, then a `:`, or `--cert` or
`--proxy-cert` and a `:` after it, so neither `set -e` nor a later
`sudo -E` without a `:` after it compiles it (WP-108; before, any curl
line holding `-e` did, about 0.3 ms per hook call). `openssl-pass`
compiles only on a line holding `pass:`, about 0.4 ms there; no hook
line does; accepted. An e-mail address (`email`, WP-093) is a local
part, `@`, and a domain of at least two labels whose last holds
letters only (`example.de`, `müller.example`,
`.испытание`). The local part is ASCII letters, digits and `._%+-`,
plus characters beyond ASCII other than the no-break space, general
punctuation (U+2000–U+206F: `—`, `„`, the quotes of `‹redacted›`), CJK
(U+3000–U+9FFF, U+F900–U+FAFF) and full-width forms (U+FF00–U+FFEF),
so text glued to an address (`山田さん（連絡先：me@…`, `Kontakt—me@…`)
stays and only the address is masked. The local part is masked and the
domain stays, so a desktop entry named after an account
(`Mail (‹redacted›@example.com).desktop`) can still be found. Not an
address: `user@host` without a dot; a version (`pkg@1.2.3`,
`react@18.2.0-rc.1`) or an npm scope (`@scope/pkg`); an `@` up to which
`url-userinfo` masks (the rule reads a URL's userinfo as that rule does,
so the import report counts the line once); an address followed by `:`
and then by a character other than white space (also a marker another
rule left there) that starts no further address
(`git@github.com:owner/repo`, `me@host.example:/srv`, a port;
both in `a@b.co:c@d.example` are addresses, as is one before `: text`
or at the end, and the `:` takes no character of what follows); a domain
whose last label is a systemd
unit type (`getty@tty1.service`; also `.socket`, `.target`, `.timer`,
`.mount`, `.automount`, `.path`, `.slice`, `.scope`, `.swap`,
`.device`); and, not recognised: an address encoded in a URL
(`me%40example.org`), a quoted local part (`"me"@example.org`) and a
local part in CJK. An SSH login with a dotted host (`ssh me@host.example`)
cannot be told from an address and is masked, as `ssh://me@host`
already is. The `regex` crate has no look-around, so the pattern
matches these contexts as groups, and the rule leaves a match in which
one takes part as it is (`Rule::unless`; for the `:`,
`Rule::unless_followed` looks at the text after the match). The config collector's
manifest in the state directory keeps the real file names, as it needs
them to compare files; a file in `skipPaths` is named nowhere. Word boundaries in the
rules are ASCII (`(?-u:\b)`): a Unicode `\b` sends a regex to the slow
matcher on any non-ASCII text, the marker of an earlier rule included
(WP-084: a 16 KB curl line took 5.4 ms, 0.13 ms with ASCII boundaries).
The `…=` assignment rules have no boundary, so a name that starts with
`ſ` or `K` still matches. An invalid user pattern is a user error
(exit 1): Seldon writes nothing rather than unredacted text; the index
build and `doctor` still run and withhold every collector message
(above). `subject` is
cut at 512 and `detail` at 4096 characters after redaction. Files written
before a rule existed are not rewritten.

`[redaction] skipPaths` (config collector and the hook, ADR-0014 §4): a
pattern with `/` matches the full path (`~/` = home), as a file or as a
directory with everything below it; a relative pattern matches at any
directory boundary; a pattern without `/` matches a file or directory
name; `*` and `?` stay within one path component, `**` crosses them and
stands for at least one folder (`a/**/b` matches `a/x/b` and `a/x/y/b`,
not `a/b`; the defaults rely on that to leave the files directly in
`~/.config/omarchy/` watched).
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
sub-commands never, ADR-0017 §5; `pacman -Sy` counts; `-V`, `-h`,
`--version`, `--help`, yay's `-P` and `-G` never; yay's `-Yc` does) after
the wrappers `sudo`, `doas`, `pkexec`, `run0`, `env`, `nice`, `timeout`,
`time`, `nohup`, `command`, `exec` with their options and option clusters
(`sudo -Eu root`; `command -v`, `sudo -l` run nothing), `omarchy
(update|pkg add|aur add|drop|remove|install|remove|reinstall|plugin
add|clone|enable|disable|remove|update|theme set|install|remove|update)`
and the `omarchy-*` scripts, `systemctl (enable|disable|start|stop|mask|
unmask)`, `cp|mv|install|ln|tee|sed -i|rm|rmdir|unlink|truncate` and
redirections whose target — or `mv` source — lies in a watched path
(`watchPaths` plus `~/.config/systemd`, always), `git` changing
sub-commands inside the XDG config home or the logbook; the string of
`bash|sh|zsh -c` and `eval` is re-parsed. Limits of this reading: the
commands inside `$(…)`, backticks and `<(…)` are not classified (their
words count only for `skipPaths`); the string of `env -S` is not opened
as a command line; a heredoc fed to a shell (`bash <<EOF`) is stdin like
any other and is not read as commands. A variable the line sets to a
literal (`F=x; … $F`) is read with its value. Relative paths resolve
against the payload's `cwd` and the `cd`, `pushd` and `popd` earlier in
the line (redirections), and for a program's own operands also against
`env -C`, `sudo -D`, `run0 -D` and its `-C DIR` (`git -C`, `make -C`);
`>& file` is a write. Heredoc bodies are the command's stdin and are cut
from the record, also inside `$(…)`, backticks and `<(…)`; the delimiter
line stays when commands follow it, so the record reads as the same
commands; `<<` inside `((…))`, `$((…))` and `$[…]` is a shift, not a
heredoc (WP-071). Redaction runs before the
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
directory the line's commands work in, read as the classifier reads them
(`cd`, `pushd`, `popd`, `-C DIR`, `env -C`). A variable the line sets to a
literal is read with its value; a word with a glob or an unknown part
(`priv*.conf`, `$D/id_rsa`, `"$(pwd)"/x`) counts when a path it can name
matches (`*` and `?` within one path component, an unknown part across
components). A word that starts with an unknown part (`$X/tail`,
`"$(pwd)"/tail`) matches a name pattern by its last component and a path
pattern only by the pattern's literal last components, never by a path
below a pattern, so `$TMPDIR/yay.log` is not taken for
`~/.config/omarchy/**/*.log`. A word in which nothing but `/` is known
(`$1`, `$NAME`, `$D/$F`) names no path; a glob of the word's own (`*`)
does (WP-063, WP-071).
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
or `status` brings the index up to date. Budget (§1, WP-057's
threshold): < 5 ms per call, median wall time of an optimised build
with the state on tmpfs, for a call it does not record and for a
recorded command, just below the threshold (with the rebuild) and at
10 000 lines; `just check-perf` asserts all four (2026-10-04, dev host:
1.3 ms not recorded, 3.5 ms recorded with the rebuild, 1.8 ms recorded
above the threshold; WP-076; 2026-10-05 after WP-092, load 2 to 3:
0.6 ms, 2.8 ms and 1.2 ms, and for a curl line whose URL leaves a
marker 3.9 ms with the rebuild and 2.2 ms above the threshold; with
WP-093's e-mail rule 4.1 ms for that curl line and 3.6 ms for a line
with an address; 2026-10-06 after WP-108, load below 1: 3.9 ms with
the rebuild for a curl line with `-u`, `set -e`, `sudo -E` and `-am`,
4.8 ms before). On a disk the sync of §1 comes on top.
WP-062's redaction is not
slower than before it: measured 2026-10-03 on a loaded dev host (load
average 3 to 8), release builds interleaved with a build of the code
before it, median of 21 runs each, a recorded command took 19 to 36 %
less time with an empty ledger, near 1000 lines with the rebuild, above
1000 lines without it, and for a command with secrets in it. A
non-mutating command compiles no redaction rule.
`seldon hook generic` takes `{"command","actor"?,"cwd","case"?,
"startedAt"?}` with the same rules. Without `"actor"` it takes
`$SELDON_ACTOR` (WP-096); with neither, or a value that is not human or
`agent:<name>`, it records nothing and says so on stderr (exit 0, as
every agent hook). `hook claude-code` and `hook session-stop` do not read
the variable: their actor is the harness's (`agent:claude-code`, or
session-stop's `--actor`).

Attended sessions (ADR-0027 §2d, WP-096): a session is attended by
provenance, not by a probe. `seldon agent start` sets `SELDON_ATTENDED=1`
in the launched agent's environment (with `SELDON_ACTOR`, §3); a session
whose task came from a human message in that session is attended too.
Any other session (a timer, a hook, another agent, another launcher) is
unattended and records and reports only. The variable is the agent's
signal, defined by the logbook's `AGENTS.md` and the skill; nothing in
the engine reads it, and a sudo credential cache or a NOPASSWD rule never
makes a session attended.

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
lock it cannot get within 8 s skips the steps that need it. The capture's
warnings (a state reset, §3) go to stderr as `seldon: warning: …`; stdout
stays empty (WP-081).

## 9. Wizard (`seldon init`)

Interactive via `dialoguer` (no `gum` dependency; gum is optional eye candy
later). The wizard asks: path (the options of ADR-0010: `~/Seldon`,
`~/Documents/Seldon`, a detected project folder, custom) → language →
Obsidian config yes/no → collectors (all on by default) → watched config
paths (defaults shown) → harnesses (`claude-code`, `omarchy-agent`) →
theme hook yes/no (default no) → git → backfill (a note explains the
drift consequence, then a date or empty). Then `init` runs, in order
(WP-024): `config.toml` saved with the choices (first, WP-074: a save that
fails writes nothing into the logbook folder, so the same `init` runs
again once it is fixed; `[git] autocommit = false` when git is declined,
`true` when it is chosen) → layout (the marker `.seldon/logbook.toml`
last, so a layout that stops half-way is no logbook; when the layout
fails, `init` puts `config.toml` back byte for byte, or removes it when
there was none, and removes what it created for the logbook: the highest
folder of the path that was not there, or the contents of the empty
folder that was) + harness files (the Omarchy-Agent kit from
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
for it is explained and opens no drift (WP-038); the step takes the
state lock before it writes the script and holds it through `omarchy
hook install` and the record (WP-074, as `hook install` since WP-049), so
no capture sees the copy before its record, and while another `seldon`
holds the lock it writes nothing and is reported without a fix → next steps (`seldon
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
harnesses from the config (none on a fresh machine), git from the
config's `[git] autocommit` (on without a config; `--git`/`--no-git`
win, and the wizard pre-selects the same, so a re-run keeps a hand-set
`autocommit = false`), first
capture from now (no backfill, no baseline), no Obsidian, no theme hook.
`SELDON_TEST_GUARD=<dir>`: the engine refuses to run (exit 2) when its
resolved home/config/state dirs lie outside that dir — set by every test
harness and manual recipe so a lost environment can never reach the real
home. `--non-interactive` takes flags, then the existing
config, then the defaults. The default language is deterministic:
`--language` > config (when it has the key) > `LC_ALL`/`LC_MESSAGES`/`LANG`
(`de*` → `de`) > `en`; the interactive wizard pre-selects the same. Every flag skips its
step. `init` refuses an existing logbook or a non-empty directory (exit 1)
and never overwrites a file; only `config.toml` is rewritten, with unknown
keys preserved. Empty layout directories get a `.gitkeep`.

The wizard writes templates from `engine/templates/{en,de}/` into the
logbook: `AGENTS.md` (the rules for agents, the short form of
`docs/AGENT-GUIDE.md`, ADR-0027 v2: session start, attended or not,
instructions and data, engine is the only writer, work in cases, when to
ask first, R3, privileged steps and snapshots, zones and risk,
installing software, closing, commands, journal and memory, drift,
hooks, ending a session, never; all inside the block
`<!-- seldon:begin rules v2 -->` … `<!-- seldon:end -->`, which holds no
marker text of its own, then `## Your rules` for the user; no
`CLAUDE.md`, WP-047, WP-100; an existing logbook gets the block with
`seldon rules update`, §3), `PROJECT.md`, `STATUS.md`,
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
