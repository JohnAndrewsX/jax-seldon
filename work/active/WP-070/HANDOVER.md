# WP-070 HANDOVER

Branch `wp/070-review`, worktree `wt/WP-070`. Commits on top of `5042509`:

- `2b5cfc9` engine: duplicate case ids are their own index check; corrupt cursors.json shown in the index
- `766a0be` engine: doctor checks patterns, state files, ledger lines, fences and case ids
- `d40c11e` docs: doctor checks, index --check exit 1 for a duplicate case id, corrupt cursors in the index
- `5b01c4c` engine: doctor reports an unreadable STATUS.md or DECISIONS.md as an error
- plus the commit that adds this handover and the memory note

## Done

- **F-133 (index).** `index::collector_state` no longer swallows a
  `cursors.json` load error. It now returns `(State, Option<warning>)`.
  Every *enabled* collector row is `ok: false`, `lastRun: null`, and has
  the message `cursors.json is corrupt or unreadable, so every capture
  fails (<cause>); `seldon doctor` has the fix`. A disabled row stays
  `ok: true` with no message. `derive_at` adds the load warning
  `~/.local/state/seldon/cursors.json: corrupt or unreadable (<cause>); every
  enabled collector is shown as failing`. This stays within the current
  schema: no contract change and no new fixture variant.
- **F-540.** If `config.toml` does not parse and the logbook path would
  come from the default, the `logbook` row is `error` with "not checked:
  config.toml is invalid, so the logbook path is not known" and no fix.
  The header reads `seldon doctor · logbook not known (config.toml is
  invalid)`, and `--json` gives `"logbook": null`. The exit code is 1,
  never 3. A path given with `--path`, `--logbook` or `SELDON_LOGBOOK` is
  still checked, and the exit code stays 1.
- **F-541.** doctor makes these checks:
  - The `config` row compiles `[redaction] patterns` (`Redactor::for_config`).
    An invalid pattern is an `error` on one line, with the fix "fix or
    remove that pattern under [redaction] patterns in <config>".
  - A new `state` row loads `cursors.json`, `manifest.json` and
    `owned.json` with strict serde. A missing file is fine.
  - Each corrupt or unreadable file gets its own `error` row. The row says
    what the file breaks, and its fix is `mv <file> <file>.bad` (or
    `chmod u+rw` when the file cannot be read).
  - The `.git/index.lock` part was already done by WP-061.
- **F-132 (doctor).** A new `ledger` row uses `Ledger::bad_lines` and the
  index warning text (`load::bad_lines_warning`). It is `degraded` and
  names, per month, the count and the first line numbers, with a fix
  line.
- **(a) Duplicate case rule.** The rule moved out of
  `index::check::Validator` into `commands::index::duplicate_cases`:
  - `index --check` refuses with **exit 1** and the message `case <id>
    exists more than once (<a>, <b>); keep one file; <index> not written`.
    Schema errors are checked first and still exit 2.
  - Plain `index` and `status` write the index and put the same line in
    `warnings` (stdout `warning: …`).
  - doctor has a new `cases` row (`error`, with a fix) built from the
    case files.
- **(b) Fences.** A new `fences` row checks `STATUS.md` and
  `DECISIONS.md`:
  - A damaged fence is `degraded` and the fix names the marker lines to
    restore. doctor asks the same question the writer asks:
    `views::merge_status(…).err()` for STATUS.md, `views::fence_damaged`
    for DECISIONS.md.
  - A file that cannot be read as text is `error`, because `status`
    stops on it with exit 2. The test proves the exit 2.
- **(c) Stray end markers.** An end marker that closes no fence is
  `degraded`. The row names the line and says: "If the <name> fence's own
  end marker was removed, `seldon status` takes the text up to the next
  end marker as the fence body and replaces it, your text included;
  doctor cannot tell a removed end marker from an intact fence".
  SPEC-ENGINE §3 states the limit: with only one end marker left, nothing
  in the file shows the problem.
- **(d) SELDON_SNAPPER.** `check_snapper` runs
  `Sources::from_env().snapper`, the same program the collector runs.
  `init` uses `check_snapper` too, so it follows without any edit to
  `init.rs`.
- **Read-only.** doctor still takes no lock and writes nothing. A new
  test makes every new check fail, then compares the whole home (state,
  config) and the logbook (including `.git`) byte for byte. It also
  checks that no `index.json` and no lock file appear. The existing
  `doctor_leaves_dot_git_untouched` still passes.
- **Docs.**
  - SPEC-ENGINE §3: the doctor command line, the doctor JSON shape
    (`"logbook":"<path>"|null`), a new doctor-checks paragraph, and the
    `index --check` / broken-files paragraph (exit 1 for duplicates, a
    corrupt cursors.json in `state.collectors`).
  - CHANGELOG `[Unreleased]` → Engine: three bullets.
  - TESTING.md: the `engine/tests/doctor.rs` row.

## Not done

- The `doctor` help line (`engine/src/main.rs`, "Check engine, config,
  logbook, omarchy, snapper and git") is unchanged. So is the CLI
  reference (`docs/user/{en,de}/05-cli-reference.md`), which docs-check
  pins verbatim. Both are outside my file list. A one-line follow-up can
  name the new rows.
- `index::rebuild_if_initialised`, the rebuild after writing commands,
  does not warn about duplicate case ids. Only `index` and `status` do,
  as the brief asks.

## Verified by

- `just check` → exit 0, `check: ok`: fmt, clippy `-D warnings`, cargo
  tests (1127 passed, 0 failed, including the watch-feature run),
  packaging, install, schema/fixture validation, docs-check, plugin
  validate, qmllint, plugin-test.
- `just check-rss` → exit 0 (`rss_stays_under_10_mb_on_the_x10_fixture
  ok`).
- **Pre-fix run.** I restored `engine/src` from `5042509` and kept the new
  tests, then ran `cargo test --no-fail-fast --test doctor --test index
  --test plan`. All 11 new or changed tests fail:
  - doctor: `an_invalid_config_is_not_reported_as_a_missing_logbook`,
    `an_invalid_redaction_pattern_is_a_config_error`,
    `a_corrupt_state_file_is_an_error_with_its_fix`,
    `bad_ledger_lines_are_counted`, `a_case_id_twice_is_an_error`,
    `damaged_fences_and_stray_end_markers_are_reported`,
    `the_snapper_probe_honours_seldon_snapper`,
    `doctor_writes_nothing_while_reporting_problems` (8)
  - index: `a_corrupt_cursors_file_marks_the_collectors_failing`,
    `a_case_id_twice_warns_in_index_and_status` (2)
  - plan: `index_check_reports_a_case_in_two_folders` (exit 2, not 1)
- **Mutants on the fixed code.** One per item. Every one is killed by an
  assertion, not by a build error:

  | Mutant | Test | Failure |
  |---|---|---|
  | M1 `stray_end_markers` finds nothing | `damaged_fences_and_stray_end_markers_are_reported` | fences `ok` where `degraded` expected |
  | M2 invalid config: default logbook checked again | `an_invalid_config_is_not_reported_as_a_missing_logbook` | `logbook` not null |
  | M3 invalid config: exit 3 wins again | same | "the config error wins over exit 3" |
  | M4 patterns not compiled | `an_invalid_redaction_pattern_is_a_config_error` | config `ok` |
  | M5 manifest.json parsed leniently | `a_corrupt_state_file_is_an_error_with_its_fix` | `manifest.json:` exit 0 |
  | M6 bad ledger lines ignored | `bad_ledger_lines_are_counted` | ledger `ok` |
  | M7 cursors error swallowed in `collector_state` | `a_corrupt_cursors_file_marks_the_collectors_failing` | no warning |
  | M8 duplicate under `--check` not refused | `plan::index_check_reports_a_case_in_two_folders` | exit 0 |
  | M9 duplicate not warned | `a_case_id_twice_warns_in_index_and_status` | `warnings: []` |
  | M10 probe runs bare `snapper` | `the_snapper_probe_honours_seldon_snapper` | "snapper not installed" |
  | M11 doctor calls `rebuild_if_initialised` | `doctor_writes_nothing_while_reporting_problems` | "doctor wrote into the state directory, the config or the logbook" |
  | M12 doctor `cases` always ok | `a_case_id_twice_is_an_error` | cases `ok` |
  | M13 damaged fence not detected | `damaged_fences_and_stray_end_markers_are_reported` | fences `ok` |
  | M14 unreadable fence file `degraded` | same | status `degraded`, not `error` |

  M7 failed to build on the first try (`None` without a type). I reran it
  with `None::<String>` and it was killed.
- **Manual run** in a scratch home, as in TESTING.md "Manual runs":
  `SELDON_TEST_GUARD`, `HOME` and all three `XDG_*` in one scratch dir,
  PATH = stubs + git. The human output was as intended for these cases:
  - green after `init`
  - corrupt `cursors.json` + a bad ledger line + edited STATUS.md → exit 1
  - `language = "fr"` → exit 1, header "logbook not known"

  The scratch dir is deleted. The output is not pasted here because the
  machine id carries the host name.

## Learned

Appended to `memory/pitfalls.md` (WP-070 section):

- `git checkout -- file` to undo a mutant also undoes an uncommitted
  fix. I lost one small change that way and re-applied it before the
  final check.
- Pre-fix runs via `git checkout <base> -- engine/src`, not the shared
  stash.
- The `init` STATUS.md has no fence.
- Where `index`/`status` warnings go (stdout) versus rebuild/hook
  warnings (stderr).
- `plan new --json` → `case.id`.
- `"logbook": null` in `doctor --json`.

## Decisions needed

- **Severities.** I chose these; please confirm:
  - `error`: a corrupt state file, an invalid pattern, a duplicate case
    id, an unreadable STATUS.md/DECISIONS.md. Each one makes a command
    fail or refuse.
  - `degraded`: bad ledger lines, a damaged fence, a stray end marker.
    The engine goes on with less.
  - The WP says "each failure is an error" for the state files.
    manifest.json and owned.json are read leniently by the collector, so
    they would fit `degraded` as well; I followed the WP.
- **`"logbook": null`** in `doctor --json` is a change to the doctor
  shape. It is documented in SPEC-ENGINE §3 and not part of `schema/`.
  The plugin does not parse doctor (checked: `plugin/` has no doctor
  reader).
- The "not checked" `logbook` row is `error`, so `ok` is false twice
  (config + logbook). If you prefer `degraded` for that row, it is a
  one-word change.

## Touched outside WP scope

- `engine/src/index/check.rs` (the duplicate rule removed from the
  validator, its unit test moved) and `engine/src/commands/index.rs` (the
  rule as its own check). Both are named in the brief's fold-in (a).
- `engine/src/index/mod.rs`: besides `collector_state`, two lines in
  `derive_at` take its new return value and push the load warning.
- `engine/tests/plan.rs`: the existing `index_check_reports_a_case_in_two_folders`
  now expects exit 1 and the plain message. It pinned the old exit 2.
- `docs/TESTING.md`: the `engine/tests/doctor.rs` row (named in the
  brief's read-first list). No WP-069/071/072 file is touched.
- `engine/tests/common/mod.rs`: not touched.
