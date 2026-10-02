```
WP-043 HANDOVER
Done: `seldon import omarchy-agent <VAULT> [--dry-run|--apply] [--json]` (dry run is the default): cases (ids kept, else renumbered with tag `omarchy-agent/<old id>` and a line under the title; statuses mapped, never active; Auftrag → Intent, Plan → Plan, Ergebnis → Result, the rest under `## History`; references to renumbered ids rewritten in all imported text), one ledger `note` per case at its `created` date (`meta.import: omarchy-agent`), journal sessions split by day (appended under `## Imported from omarchy-agent` when the day exists), knowledge → `memory/<topic>.md` sections (lessons → `memory/lessons.md`), deviation entries → `deviations.table` user rows, inbox/Dashboard/templates/rest of `system/` listed, redaction plus home paths → `~` on every imported line; report `outputs/IMPORT-omarchy-agent.md`; `--apply` = one commit, index rebuilt, marker `.seldon/imports/omarchy-agent.json` (second apply: "Nothing changed"); synthetic vault `fixtures/vaults/omarchy-agent/`, golden report, 6 integration + 13 unit tests in import/ (plus separator tests in index/load.rs and dossier/); review fix round done (below); SPEC-ENGINE §3, SPEC-LOGBOOK §3, TESTING.md, fixtures/README.md
Not done: no `--apply` on the operator's `~/Seldon` (the operator's step; I never read or touched it)
Verified by: `just check` exit 0 (`check: ok`: fmt, clippy -D warnings, all engine tests incl. --features watch, packaging, validate-fixtures 109 instances, plugin-validate, qmllint 28 files, plugin-test); after the fix round: `cargo test --test import` 6 passed, `--lib` 128 passed, clippy -D warnings clean, `just check` exit 0 again; the real-vault dry run below (re-run after the fixes), 0 errors, vault byte-identical before/after (sha256 of every file)
Learned: memory/rust-notes.md + memory/pitfalls.md, sections "WP-043" and "WP-043 review round"
Decisions needed: none (the two open ones were decided in review and are implemented)
Touched outside WP scope: engine/src/dossier/mod.rs and engine/src/index/load.rs (table separator hardening, asked for in review); engine/src/main.rs, commands/mod.rs, lib.rs only register the command and module
```

Branch `wp/043-vault-import`, worktree `wt/WP-043`, from `70696bf`. No PR,
no push. Commits:

- `b992fe6 engine: import omarchy-agent vault, dry run and apply (WP-043)`
- `23fa151 docs: import command in SPEC-ENGINE, SPEC-LOGBOOK and TESTING (WP-043)`
- `9bed78d memory: WP-043 notes and pitfalls`
- `d419a3c work: WP-043 handover`
- `4b10e06 engine: import review fixes: Ergebnis to Result, id rewrites, safe half-apply (WP-043)`
- `e83b336 docs: import review fixes in SPEC-ENGINE, SPEC-LOGBOOK, TESTING; memory notes (WP-043)`
- this handover update

## Real-vault dry run (acceptance)

The vault copy was read-only input. The output went to a scratch copy of
`fixtures/logbook/`, with HOME and all three `XDG_*` in one scratch dir and
`SELDON_TEST_GUARD` set. The recipe is in docs/TESTING.md ("Dry run of
`seldon import omarchy-agent` on a real vault"). Exit 0, `errors: 0`. The
dry run wrote only the report. Counts only, as asked:

| kind | count |
|---|---|
| cases | 36: 26 done → completed, 5 planned → queued, 4 verification → queued, 1 in-progress → queued |
| renumbered | 7 (against the fixture logbook's C-2026-001…008) |
| journal | 41 sessions on 13 days (1 day appended to an existing fixture day) |
| memory | 20 sections in 5 files (3 new) |
| deviation rows | 25 (of 31 entries: 1 resolved, 2 without a path, 3 with a path already listed) |
| not imported | 35 (19 inbox items, 5 other `system/*.md`, 2 templates, Dashboard, STRUCTURE.md, `.obsidian/`, 6 deviation entries) |
| errors | 0 |
| redaction | 1 line (token-assignment); 11 home paths rewritten to `~` |
| id rewrites (fix round) | 27 wikilinks and 83 bare ids in 18 files |
| assumptions (fix round) | 0 (every done case in the vault has `closed`) |

Collisions against the fixture logbook:

| kit id | new id |
|---|---|
| C-2026-001 | C-2026-039 |
| C-2026-002 | C-2026-040 |
| C-2026-004 | C-2026-041 |
| C-2026-005 | C-2026-042 |
| C-2026-006 | C-2026-043 |
| C-2026-007 | C-2026-044 |
| C-2026-008 | C-2026-045 |

On the real `~/Seldon` the list depends on which ids exist there. The
operator's own dry run shows it before any `--apply`.

## SPEC wording (as committed)

**SPEC-ENGINE §3** — a new entry after `update-impact`:
`seldon import omarchy-agent <VAULT> [--dry-run|--apply] [--json]`. It
covers:

- the report and its fence `import-omarchy-agent`;
- the id rule: kept, else the next free id of the year after the highest
  id in the logbook and the kit; the `omarchy-agent` tags;
- the status map and the section map;
- the ledger note (`manual/note`, `ts` = created at local midnight, actor
  `human`, meta `{import, originalId, originalStatus, source}`, its id in
  `events`);
- the journal, memory and deviation rules;
- what is listed, and what counts as an error;
- the dry-run commit `seldon: import omarchy-agent (dry run)`;
- `--apply`: refused while there are errors; write order ledger → cases →
  days → memory → deviations → report → marker; one commit `seldon:
  import omarchy-agent`; index rebuilt;
- the marker shape;
- the no-op rule (marker, or an import note in the ledger when the
  marker is gone);
- exit codes and the `--json` shape.

**SPEC-LOGBOOK §3**, under Case, a paragraph "Imported cases". It covers:

- the status folder (never `active`) and the tags;
- the extra `## History` section after Plan (headings one level deeper);
- the first Log line and `events`;
- imported journal sessions under `## Imported from omarchy-agent` (one
  level deeper, so they are text, not entries);
- imported knowledge as one `## ` section per file with an
  `*Imported from omarchy-agent: …*` line;
- the report and the marker are committed.

## Design notes

- **Plan, then write.** One function builds the plan: every file with its
  final bytes. The dry run renders the report from the plan, and
  `--apply` writes the same plan. So the reviewed report and the applied
  files cannot differ, unless the logbook changes between the two runs.
  In that case the apply simply plans again.
- **Ids.**
  - A kit id collides when the logbook has a case file or a `work/C-…/`
    folder with that id: the same id space as `cases::next_id`.
  - A collision gets the next number after the highest id of that year
    in the logbook and in the whole kit. A renumbered case therefore
    never takes an id that a later kit case keeps.
  - The file keeps the kit's slug (`C-2026-011-foot-paste-binding.md`),
    so kit wikilinks `[[C-…-slug]]` still resolve for cases that were
    not renumbered.
- **Errors block `--apply`.** An error is a case file that cannot be
  mapped (frontmatter, status, zone, risk, priority, dates), a file that
  is not UTF-8, or a logbook day with invalid frontmatter. The apply
  writes the dry-run report and exits 1. It never imports half a vault
  silently.
- **Idempotency.** The marker is the main guard. Deleting `.seldon/`
  must lose nothing (SPEC-LOGBOOK §7), so a ledger note with
  `meta.import: omarchy-agent` also counts. A test covers both.
- **The vault is never written.** Tests compare the copied vault
  byte-for-byte before and after. The real run was checked with sha256
  sums.
- **Redaction.** It uses the built-in rules plus `config.toml [redaction]
  patterns` on every imported line, frontmatter included. Then
  `/home/<user>` at the start of a path becomes `~`. The report lists
  file, line and rule, never the text.

## Review fix round (review of 2026-10-02: APPROVE)

The two open decisions were decided as class (a) and are implemented as
specified. The five fixes are done too.

- **(a) `Ergebnis` → `## Result`.** Match arm `"Ergebnis" | "Result"`;
  the golden report was re-blessed; the `### Ergebnis` example is gone
  from SPEC-LOGBOOK.
- **(b) Id rewrites.**
  - Scope: references to a kit id the logbook already had, in all
    imported text: case bodies (Intent, Plan, History, Result), journal
    sessions, memory sections and deviation reasons.
  - Rule: `[[C-OLD(-slug)?` → `[[C-NEW(-slug)?` and bare `\bC-OLD\b` →
    `C-NEW`, in one regex pass with a map lookup, so a new id is never
    rewritten again.
  - The old id stays in the tag `omarchy-agent/C-OLD`, the title line,
    the Log line and `meta.originalId`.
  - A kit id that two kit files share is not rewritten: it is ambiguous,
    and the logbook did not have it.
  - The journal's `cases:` already used the id map.
  - The report has a new "Id rewrites" section (per file: wikilinks,
    bare ids); the counts are also in `--json`.
- **(1) Table separators.** The new `index::load::has_table_separator`
  recognises `|---|`, `| --- |`, `|:---|---:|`. It is used by the
  importer and by both places in `dossier/mod.rs` (`packages.history`,
  `deviations.table`). Tests: a unit test of the helper, a dossier unit
  test for both fences, and an integration test in which an
  Obsidian-padded `deviations.table` with a user row keeps that row on
  `--apply`.
- **(2) Half-apply safety.** I took the second option: only the marker
  means "imported". Ledger import notes without the marker are a user
  error (exit 1). The message names the note and gives the undo `git
  checkout -- . && git clean -fd` in the logbook. It never says "nothing
  changed", and it never imports a second time, so a deleted `.seldon/`
  still cannot cause duplicates. Every write of `--apply` now sits in one
  `write_plan`. Any error there says "the import stopped half way and
  nothing was committed" plus the undo (exit codes unchanged: 1 user, 2
  engine). Test, with the failure injected by a read-only `system/`:
  1. the apply exits 2 with the hint, makes no commit and writes no
     marker;
  2. the next run is refused;
  3. after `git checkout -- .` and `git clean -fd`, the import runs once,
     with 6 notes and 1 commit.

  I kept the ledger-first order: the case files need the note ids in
  `events:`.
- **(3) `updated`.** An existing `memory/<topic>.md` with valid
  frontmatter gets `updated` = the import day; the rest of the file is
  kept byte for byte. The test pre-dates lessons.md to 2026-09-01.
- **(4) `done` without `closed`.** `closed` is set to `created`. The case
  is listed in a new "Assumptions" report section (and `--json
  assumptions`), and its Log line ends with `closed date assumed:
  <date>`. The same applies to `dropped`. The fixture's C-2026-005 now
  has an empty `closed`.
- **(5) Tests.** One per item:
  - (a): the renumbered case ends with `## Result` + the Ergebnis text,
    and "Ergebnis" no longer appears;
  - (b): a case body (incl. the case's own old id), the journal (and its
    `cases:`), lessons and a deviation reason;
  - (1), (3), (4): as above;
  - `--json` counts `rewrittenLinks 1`, `rewrittenIds 4`, `assumptions
    1`;
  - unit tests for the rewriter and for a renumbered done case without
    `closed`.

**Rewrite totals on the real vault** (against the fixture logbook's 7
collisions): 27 wikilinks and 83 bare ids in 18 files. An independent
count over the vault agrees: 27 links and 87 bare ids, 4 of which sit in
knowledge frontmatter (`source:` lines) that the import does not carry
over. All case-id references in the imported text: 106 links and 260
bare ids. The reviewer's 208/633 counted the whole vault (inbox,
Dashboard, the rest of `system/`), which is not imported. The real
number for `~/Seldon` depends on its collisions; the operator's dry run
shows them.

## Open notes

- Imported open cases take part in drift proposals (ADR-0012 §7): their
  `## Plan` text is matched like any other open case. This is intended,
  but the operator may see proposals for imported plans after the first
  capture.
- The 25 deviation rows have an empty case cell (`—`). A later cased
  config event on the same path fills it (`seldon dossier`, WP-036 rule).
