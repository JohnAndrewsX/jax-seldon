# WP-002 HANDOVER

Branch `wp/002-contract-fixtures`, worktree `wt/WP-002`. Not pushed, no PR.
Commits: `01a41af` schema · `af8c58c` logbook · `9a2f267` logs+hooks ·
`aa63286` validate-fixtures · `727dd9f` sample index · `04fd092` memory · this handover.
(Only the branch head is consistent: the old draft index does not validate
against the new logbook until `727dd9f`.)

## Done
- **Schema reviewed** (`schema/*.json`), contractVersion stays **1**. The one hard
  contradiction: `case.schema.json` rejected `type: case`, which SPEC-LOGBOOK §3
  puts in every case frontmatter. Also tightened: shared ULID/actor/case-id defs,
  closed index objects, enums/patterns/formats, `linked` needs `case`,
  `correction` needs `refersTo`, documented `meta` keys. Semantics written down in
  **`decisions/ADR-0012-contract-v1-clarifications.md` (status: proposed)**.
- **`schema/external/`**: schemas for the third-party inputs (snapper list,
  `omarchy plugin list --json`, `omarchy plugin catalog`, Claude Code hook
  payload). Not contract, not versioned (README there).
- **`fixtures/logbook/`**: PROJECT.md, AGENTS.md, STATUS.md, DECISIONS.md,
  `.seldon/logbook.toml`, `.gitignore`; 8 cases (queued 3, active 2 + 1 in
  verification under `work/active/`, completed 2); 10 journal days; `ledger/2026-09.jsonl` +
  `2026-10.jsonl` (64 events, 9 resolutions, every kind consistent with
  source/subject) and their generated `.md` views; 4 decisions (1 proposed); 6 areas
  (2 with AGENTS.md); dossier with fenced sections (hardware, packages incl.
  history, omarchy, services, plugins, deviations); memory (lessons + 2 topics); inbox.
  The three drift rows of the old sample are kept (ollama install, ollama user
  unit = 2 crises; tokyo-night theme proposed for C-2026-005); all old cases kept.
- **`fixtures/logs/`**: `pacman.log` (archinstall `-r /mnt` lines at +0000, local
  +0200 afterwards, transaction blocks, remove/downgrade/reinstall/epoch, yay's
  `-- extra/zed` + `-D --asexplicit`, `--ask 4`, 7 malformed lines, unterminated
  last line); `pacman-rotation/{pacman.log.1,pacman.log}` (inode change + one
  duplicated transaction); `snapper-before.json`, `snapper.json`,
  `snapper-no-permissions.stderr`; `plugin-list-{before,after}.json` (real field
  set, no `version`); `plugin-catalog.json`. Expected collector results (init byte
  offset 6129, diffs) are in `fixtures/README.md`.
- **`fixtures/hooks/claude-code-{mutating,non-mutating,secret}.json`**
  (PostToolUse/Bash), with expected outcomes in `fixtures/README.md`.
- **`fixtures/index.sample.json` rebuilt**: every event is a ledger line
  (resolutions folded), every derived section (summary, today, drift, cases,
  decisions, system, memory, series) is recomputed from the logbook. Renders all
  surfaces: 4 today + 1 yesterday entries, 55 events incl. snapshot/drift rows,
  all case columns, decisions, system, memory, heatmap/packages/drift/risk/timeline,
  crisis banner.
- **Extra fixtures**: `index-variants/{snapper-degraded,not-initialised}.json`
  (banner states); `invalid/*.json` (7 must-fail cases; `index.contract-v2.json`
  doubles as the plugin's contract-mismatch case).
- **`scripts/validate-fixtures.sh`** (+ `validate-fixtures.py`): validates every
  JSON fixture (fails on unmapped files), every ledger line, every case
  frontmatter, asserts the `invalid/` files fail, and diffs `index.sample.json`
  against the logbook derivation. Backends: python `jsonschema` → `check-jsonschema`
  → builtin Draft 2020-12 subset (fails closed on unknown keywords).
  `--write-index` regenerates the derived parts after a logbook edit.
  **Call signature for WP-001 is unchanged:** `bash scripts/validate-fixtures.sh`,
  no arguments, exit 0/1, only needs `python3`, ~0.1 s, works from any cwd.
- `fixtures/README.md` documents the story, derivation inputs and the engine
  assumptions the fixtures encode.

## Not done
- The **jsonschema and check-jsonschema backends are untested**: neither is
  installed on this host and installing is red zone. Only the builtin backend
  ran. CI (WP-001) should run once with `--validator jsonschema` after
  `pip install jsonschema`; the code registers all schemas locally (no network
  `$ref`s) and the check-jsonschema path rewrites `$id`s to `file://` copies.
- **snapper JSON shape not verified on the host** (needs ALLOW_USERS, ADR-0011);
  taken from snapper upstream. Re-check once the operator opts in.
- **Spec text not edited** (docs/ is not in my scope): SPEC-ENGINE §5 rules 3–4
  and SPEC-LOGBOOK §3 case folders need the one-line changes listed in ADR-0012;
  SPEC-ENGINE §4 "plugins" needs the version source (see decisions 4).
- No plugin manifest fixtures for reading `version` from `manifestPath` (WP-005 can add them).
- "Renders every plugin surface" is confirmed by WP-010, not here. Note for the
  plugin dev: `generatedAt` is fixed at 2026-10-01 17:05, so a live clock always
  sees the sample as stale (> 2 h).
- Moved nothing in `work/` (as instructed).

## Verified by
```
$ bash scripts/validate-fixtures.sh
validate-fixtures: ok — 90 instances (90 incl. 7 expected failures), 64 ledger events traced to index.sample.json; backend builtin
$ echo $?
0
$ bash scripts/validate-fixtures.sh --validator jsonschema    # → FAIL python module jsonschema not available, exit 1
```
Mutation checks (reverted afterwards): changing one ledger `detail` → 2 diffs
reported in `/events` and `/drift`; dropping an id from a case's `events:` →
reported; an unmapped `fixtures/logs/stray.json` → reported. Each `invalid/` file
fails for the intended reason (missing `refersTo`, missing `case` on `linked`,
bad actor, naive timestamp, unknown key, bad status, contractVersion 2).
Real formats read on this host (read-only): `/var/log/pacman.log`,
`omarchy plugin list --json`, `omarchy plugin catalog`, `theme.name`,
`snapper --jsonout list` (permission error), and the scripts
`omarchy-plugin-catalog`, `omarchy-plugin-list`, `omarchy-plugin-clone`,
`omarchy-snapshot`, `omarchy-theme-set`, `omarchy-update`.

## Learned
In `memory/host.md` ("Verified during WP-002") and new `memory/pitfalls.md`. Key points:
- `omarchy plugin catalog` has **no `version`** (host.md item 3 was wrong about it).
- `omarchy plugin list --json` is shell IPC → fails if the shell is not running.
- pacman.log mixes +0000 (install) and +0200; unprivileged snapper exits 1 with
  `No permissions.` on stderr; `omarchy update` snapshots are `single`/`number`
  with the old version as description.
- The guard blocks the word `pacman` and `ln`/`rm`/… substrings even inside
  heredocs of prose; use the Write tool.

## Decisions needed
1. **Accept ADR-0012** (proposed) — particularly: (6) only pacman/omarchy/
   plugins/theme/config events can be drift; (7) proposals also match *queued*
   cases (the old sample already proposed queued C-2026-005, contradicting
   SPEC-ENGINE §5 "active case"); (8) resolution events are folded, not listed;
   (9) verification cases live in `work/active/`, dropped in `work/completed/`.
2. **Hook → collector attribution.** The fixtures (and the old sample) give
   collector events the actor/case of a preceding hook `command` event that
   caused them (e.g. pacman `install zed` → `agent:claude-code`, C-2026-004).
   SPEC-ENGINE §4 only says "only hooks set agent actors". Confirm this reading
   for WP-004/WP-006, or the fixtures change.
3. **Routine updates flood drift.** An `omarchy update` without a case yields one
   red (crisis) drift item per upgraded package plus the omarchy `update` event
   (the fixture keeps it to one package). Options: transaction-level resolution,
   or `-Syu`/`omarchy update` treated as one item. Architect call before WP-005.
4. **`plugin-update` source.** Catalog and list have no version; read
   `version` from the manifest at `manifestPath` (or the plugin dir's git HEAD).
   SPEC-ENGINE §4 needs that line.
5. **Event zones** are not specified anywhere. Fixtures use: pacman, omarchy,
   config under `~/.config/systemd` → red; other config, theme, plugins →
   yellow; snapper/notes/case events → none. Needs a home in SPEC-ENGINE.
6. Should the Claude Code hook also record Edit/Write tool calls
   (`tool_input.file_path`)? Without it, agent file edits arrive as drift with a
   proposal (modelled on 2026-09-12). WP-006.
7. ADR number: this branch adds **ADR-0012**; if WP-001 also adds one, renumber at merge.

Changes to the old sample worth a reviewer's glance (no decision needed):
plugin ids now `io.github.example.*` (no real-looking handles); Omarchy versions
as `omarchy-version` prints them (`4.0.7-1`); `repoHead` omitted (package
install); C-2026-001 and ADR dates moved into September (the ledger starts
2026-09); created dates of C-2026-003…006 reordered so ids are monotonic;
C-2026-003 `snapshotBefore` 111 (112 is created by `omarchy update` after the
case started); new C-2026-008 (verification) and ADR-0004 (proposed).

## Touched outside WP scope
None beyond what the brief allows: `decisions/ADR-0012-…md`, `memory/host.md`
(appended), `memory/pitfalls.md` (new), this file. No justfile, engine/, plugin/,
CI, docs/ or work/queued changes.
