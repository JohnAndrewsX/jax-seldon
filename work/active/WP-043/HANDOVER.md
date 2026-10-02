```
WP-043 HANDOVER
Done: `seldon import omarchy-agent <VAULT> [--dry-run|--apply] [--json]` (dry run is the default): cases (ids kept, else renumbered with tag `omarchy-agent/<old id>` and a line under the title; statuses mapped, never active; Auftrag → Intent, Plan → Plan, the rest under `## History`), one ledger `note` per case at its `created` date (`meta.import: omarchy-agent`), journal sessions split by day (appended under `## Imported from omarchy-agent` when the day exists), knowledge → `memory/<topic>.md` sections (lessons → `memory/lessons.md`), deviation entries → `deviations.table` user rows, inbox/Dashboard/templates/rest of `system/` listed, redaction plus home paths → `~` on every imported line; report `outputs/IMPORT-omarchy-agent.md`; `--apply` = one commit, index rebuilt, marker `.seldon/imports/omarchy-agent.json` (second apply: "Nothing changed"); synthetic vault `fixtures/vaults/omarchy-agent/`, golden report, 4 integration + 11 unit tests; SPEC-ENGINE §3, SPEC-LOGBOOK §3, TESTING.md, fixtures/README.md
Not done: no `--apply` on the operator's `~/Seldon` (the operator's step; I never read or touched it); wikilinks to renumbered cases are not rewritten (see Decisions needed)
Verified by: `just check` exit 0 (`check: ok`: fmt, clippy -D warnings, all engine tests incl. --features watch, packaging, validate-fixtures 109 instances, plugin-validate, qmllint 28 files, plugin-test); `cargo test --test import` 4 passed; the real-vault dry run below, 0 errors, vault byte-identical before/after (sha256 of every file)
Learned: memory/rust-notes.md + memory/pitfalls.md, sections "WP-043"
Decisions needed: two small ones, neither blocks the merge (below)
Touched outside WP scope: none (engine/src/main.rs, commands/mod.rs, lib.rs only register the command and module)
```

Branch `wp/043-vault-import`, worktree `wt/WP-043`, from `70696bf`. No PR,
no push. Commits:

- `b992fe6 engine: import omarchy-agent vault, dry run and apply (WP-043)`
- `23fa151 docs: import command in SPEC-ENGINE, SPEC-LOGBOOK and TESTING (WP-043)`
- `9bed78d memory: WP-043 notes and pitfalls`
- this handover

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

## Decisions needed (none blocks the merge)

1. **`Ergebnis` → `## Result`?** The WP maps only Auftrag → Intent and
   Plan → Plan, with "the rest verbatim under History". So the kit's
   `Ergebnis` lands as `### Ergebnis` under History, and `## Result`
   stays empty, also for completed cases. Mapping Ergebnis → Result
   would be a one-line change in `case_body` plus the golden report.
   Operator's call, before the real `--apply`.
2. **Wikilinks to renumbered cases.** Kit text links cases by file stem
   (`[[C-2026-001-slug]]`). A renumbered case's file is
   `C-2026-0NN-slug.md`, so its old links dangle. They do not point to
   the wrong case: Seldon file names carry a slug, so `[[C-2026-001-x]]`
   matches nothing else. The report's collision table is the map.
   Rewriting links in imported text is possible, but it changes the
   kit's prose. I left it out.

## Open notes

- Imported open cases take part in drift proposals (ADR-0012 §7): their
  `## Plan` text is matched like any other open case. This is intended,
  but the operator may see proposals for imported plans after the first
  capture.
- The 25 deviation rows have an empty case cell (`—`). A later cased
  config event on the same path fills it (`seldon dossier`, WP-036 rule).
