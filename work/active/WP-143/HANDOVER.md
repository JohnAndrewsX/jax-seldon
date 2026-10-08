# WP-143 — Handover

Branch `wp/143-tightenings`, from `next` (aaf7a0a), merges into `next`.
Plan and decisions: `PLAN.md` (D1–D7).

## Done

1. **Stop if** — the case template's Plan ends with `- Stop if:` (empty).
   The rules block (en, de), the skill (`SKILL.md`, `case.md`) and
   AGENT-GUIDE tell agents to fill it before the first change and to stop
   when it holds: say so in the *Log* and to the user, go on only after
   the user's go.
2. **Verify the effect, not the setting** — rules *Closing*, skill
   `case.md` (*Close It Yourself*, with the key-binding example on
   `~/.config/hypr/bindings.lua`), `SKILL.md` *Act, Then Account*,
   AGENT-GUIDE §8.
3. **Closing commits** — `plan done` commits `seldon: <ID> completed —
   <title>: <first line of Result>`; `plan drop` `seldon: <ID> dropped —
   <title>: <reason>`; without a line or reason the title alone. The text
   after the dash is one line, redacted, then clipped to 100 characters
   (`closing_summary`, `engine/src/commands/plan.rs`). Other steps keep
   `<ID> <status>`.
4. **doctor `workpieces` row** — `ok` always, no fix: orphaned
   `work/<case-id>…/` folders and those a completed/dropped case left
   over 10 MiB; count, size, the oldest by case id; control characters in
   the shown name become `?`; symbolic links not followed; an open
   case's folder is not measured.
5. **J3** — `- Persists: <!-- survives reboot and update | reboot only |
   lost at reboot -->` in the Plan after *Affected paths*; the rules,
   skill and AGENT-GUIDE ask for `measured` / `documented` / `inferred`
   on each claim in *Result* and `memory/`. No schema change.
6. **Existing logbooks** get the new template lines: a
   `.seldon/templates/case.md` that is byte for byte WP-006's (en or de,
   the only one ever shipped) counts as the built-in template; an edited
   copy is used as it is; nothing is written (D2).
7. **Rules v4 stays v4** (D1): the 0.1.4 block is stored under
   `engine/templates/rules-v4/AGENTS-wp116-{en,de}.md` and its two hashes
   are in `RELEASED_BLOCKS`, so the next capture upgrades an unedited
   0.1.4 block silently (`rules update (unedited, v4 → v4)`); an edited
   one shows doctor's `outdated (v4, its text differs …)`.
8. Docs: SPEC-LOGBOOK §2/§3, SPEC-ENGINE §3 (doctor list and
   `workpieces`, the autocommit), AGENT-GUIDE §3/§8, user guide 06, 07,
   10 (en; de follows with its source line), CHANGELOG [Unreleased].

## Not done

- Nothing of the WP left out. The engine does not read `Persists:` or
  `Stop if:` (the WP asks for habits, not checks).

## Verification

- New tests: `commands::plan::tests` (summary: title, Result line, list
  marker, control characters, clip, redaction before the clip, drop);
  `tests/plan.rs` `closing_commits_carry_the_title_and_the_result_line`
  (real git: Result with a comment and a heading before the line, a
  secret clipped and redacted, drop reason, other steps unchanged) and
  `new_writes_persists_and_stop_if_into_the_plan` (built-in, shipped copy
  upgraded without a write, edited copy kept); `tests/doctor.rs`
  `doctor_reports_leftover_workpiece_folders` (none, small/open not
  counted, orphaned, oversized, 4-digit id, not-a-case-id names, symlink,
  oldest by id — 999 before 1000 —, control characters) and the unit
  test `a_workpiece_folder_is_named_by_its_case`; rules
  `the_released_blocks_are_exactly_the_shipped_blocks` and
  `a_shipped_block_is_upgraded_silently_an_edited_one_is_not` now cover v4.
- Manual mutants: `work/active/WP-143/mutants.py` (copy of `engine/`, own
  target, `python3 work/active/WP-143/mutants.py <copy> <target>`):
  17/17 caught (summary redaction, clip, control characters, Result
  line, list marker, drop reason; shipped/edited template; orphans, open
  cases, threshold, oldest order by id, symlinks, shown name, id suffix,
  short numbers). The first run let two name-parsing mutants and the
  name-order mutant through; the unit test and the 999/1000 step were
  added for them.
- `flock /tmp/seldon-check.lock just check` on `cf0b7b9`: `check: ok`
  (fmt, clippy, tests, watch feature, packaging, install, deploy, guard,
  schema, docs-check, plugin validate, qmllint, plugin tests). Run on
  this host, not as root and not in UTC; nothing here depends on either
  (the tests' times carry offsets).
- The diff against `next`, grepped for absolute home paths: none.

## Open questions

- **Rule 9 reads the whole Plan.** A path or package named under
  `Stop if:` (e.g. "`linux` would be upgraded") counts as planned for
  §5 rule 9, as anything in *Rollback* already does. I left rule 9 alone;
  if the orchestrator wants `Stop if:` excluded, that is a small follow-up
  in `reconcile.rs`.
- **Rules v4 vs v5** (D1): text changed within v4 because the engine
  already handles "this version, other wording, shipped"; if the reviewer
  prefers a bump for the clearer `v4 → v5` commit and doctor wording, it
  is mechanical (marker, `VERSION`, tests, docs).
- **10 MiB** threshold and the 100-character clip are my picks; both are
  one constant each.

## Round 2

Brief: the orchestrator's round-2 brief (stage-1 review). `next` merged
first (d4a7a48: WP-124, WP-140).

### Fixed

- **B1 — format and bidi characters.** `closing_summary` now drops
  `import::is_direction_or_format` characters before redaction (a ZWSP
  can no longer split a token from its pattern) and turns control
  characters and `is_line_breaking` (U+2028, U+2029) into spaces. The
  doctor `workpieces` name shows `import::bad_path_char` and
  line-breaking characters as `?`. Tests: a `ghp_` token split by U+200B
  is redacted; U+202E, U+2028, U+2029 and U+200B are absent from the
  subject (unit test) and from the shown name (integration test).
- **B2 — "Stop if" and the stop rules.** *When to ask first* (rules en
  and de, SKILL, AGENT-GUIDE §3) now lists four cases, the fourth
  "**your own *Stop if***: the condition you wrote in the *Plan* holds";
  the intro says "only in these four cases". The *Plan* lines point
  there ("stop and ask"). The heading "R3: the one stop" is now "R3:
  always the user's go" (rules en/de, AGENT-GUIDE, `tests/init.rs`, the
  init golden), and AGENT-GUIDE's "R3 is the one step that keeps the
  user's explicit go" says "R3 always keeps the user's explicit go,
  whatever the *Plan* says". The example is `the binding is taken by
  another app` everywhere (no `linux`).

### Also in this round

- **N1:** rule 9 reads the Plan without its `Stop if:` item and the lines
  indented below it (`cases::without_stop_if`, sharing the item matcher
  with the `Verification:` check, now `item_text` + `continuation`).
  Tests: `cases::without_stop_if_drops_the_item_and_its_lines` and
  `reconcile::a_stop_condition_is_not_a_plan` (a package named only under
  *Stop if* is not linked; one in *Steps* is). SPEC-ENGINE §5 rule 9 (b),
  SPEC-LOGBOOK and AGENT-GUIDE say so.
- **N2:** tests now kill: A inner symlinks followed (a link to a dir and
  to a 20 MiB file inside an orphan folder), B the de hash
  (`the_shipped_case_templates_are_known`, both languages), C a case
  file that does not parse counts as present (`work/queued/C-2026-050-…`
  without frontmatter owns `work/C-2026-050/`), H the 10 MiB boundary (a
  closed case's folder of exactly 10 MiB is not oversized).
- **N3:** `folder_size(dir, stop_above, max_entries)`: same filesystem
  (`MetadataExt::dev`; a mount point below is neither entered nor
  counted), at most `WALK_MAX` = 100 000 entries per folder, a closed
  case's folder measured only until it passes 10 MiB; when entries were
  left the row says `≥ <size> in all`. Unit test
  `a_workpiece_walk_is_bounded`; the integration test shows `≥`. The
  same-filesystem check has no test (it needs a mount).
- **N4:** WP-116 round 1's v4 block (7e7a459) is in `RELEASED_BLOCKS`
  (en `03d1fb35…`, de `c1bd885f…`), rendered under
  `templates/rules-v4/AGENTS-wp116r1-{en,de}.md`; the rules tests cover
  it.
- **N5:** `SHIPPED_TEMPLATES` is keyed by template name
  (`shipped_template(name, text)`); a case template copied as
  `decision.md` is not taken for the built-in one.
- **Wording:** 0.1.4 "ships in", not "released in" (rules.rs, CHANGELOG).
  My round-1 reply called 0.1.4 released; it is prepared, not tagged.

### Verification

- `cargo test --no-fail-fast`: all green; clippy `-D warnings` and fmt
  clean.
- Mutants (`work/active/WP-143/mutants.py`, a copy with its own target):
  **31/31 caught**, the 17 of round 1 (patterns updated) and 14 new ones
  for B1, N1, N2 A/B/C/H, N3 and N5.
- `flock /tmp/seldon-check.lock just check` on c2d57b5:
  - The plain run stopped at `check-packaging`: `just` could not write its
    shebang script — "No space left on device" — because
    `/run/user/1000` (tmpfs, `XDG_RUNTIME_DIR`) is 100 % full of old
    quickshell instance folders. Nothing there was touched (orchestrator
    note).
  - Re-run as `just --tempdir <my scratch dir> check` (only where `just`
    puts its own temp scripts; no recipe changed): fmt, clippy, test,
    check-watch, check-packaging, check-install (229/0),
    check-deploy (190/0), check-guard, schema-validate, docs-check,
    plugin-validate, qmllint (47 files), and in plugin-test model,
    terminal-scripts (65/0), real-home-guard (11/0): **ok**.
  - **Environment failure:** `service-states` 312 passed, **30 failed**,
    every one "log has errors" and every error line quickshell's
    `Failed to copy (detailed) log from memfd … error code 28 "No space
    left on device"` (30 + 30 lines). This branch changes nothing under
    `plugin/`. The rest of plugin-test, run by hand under the same lock:
    `desk-view` 1438 passed, **118 failed** (every one "log has errors",
    the only error lines the same quickshell ENOSPC pair, 118 + 118);
    `bar-view` 194 passed, 0 failed.
- The diff against `next`, grepped for absolute home paths: none.
