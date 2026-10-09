# WP-176 — Handover

Branch `wp/176-readable-from` from `next` (f7f85aae, WP-176 queued).
Not pushed (the orchestrator pushes).

## What was done

**ADR-0051** (`decisions/ADR-0051-contract-forward-compatibility.md`,
status **proposed**, DECISIONS.md row): the optional index field
`contractReadableFrom` (integer, 1 ≤ value ≤ `contractVersion`, "the
oldest plugin contract that can read this index without misreading a
field it keys on"); the plugin's check (`V = P`, or `V > P` and
`R ≤ P`; absent or out of range = `V`, today's strict rule); the quiet
notice; §3 the obligation on every later bump (per added or changed
field, why an older reader does not misread — only new keys, open-set
values the reader already falls back on, nothing removed or changed in
meaning, `summary` counts and crises keep their meaning, the commands an
`ok` reader runs keep working); §4 what it does not do (older index,
0.1.x plugins, no feature parity). The preview stays strict.

**Contract and engine** (commit c3abbe53):

- `schema/index.schema.json`: `contractReadableFrom` `{type: integer,
  minimum: 1, maximum: 2}` with the description.
- `engine/src/lib.rs`: `CONTRACT_READABLE_FROM = 2`; `index::model::Index`
  gains `contract_readable_from` right after `contract_version`; both
  index builders (`build`, `not_initialised`) write it, so every index
  starts `{"contractVersion":2,"contractReadableFrom":2,…`.
- The built-in schema validator (`index::check`) learns `maximum`
  (it reports unknown keywords as errors) with a unit test.
- Tests: `engine/tests/index.rs`
  `the_index_says_which_plugin_contract_reads_it` (both builders write 2
  as the second key; absent and 1 validate; 0, 3, 1.5, "2", null fail
  the engine checker and `jsonschema`; the forward fixture fails both);
  `contract_v2.rs` end to end through `seldon index`. The golden test
  holds the engine to the sample.

**Fixtures and reference derive** (`scripts/validate-fixtures.py`):

- `CONTRACT_READABLE_FROM = 2`; `as_sample` writes it after
  `contractVersion`, and the check fails when the sample's value differs.
  `--write-index` regenerated the sample, `index.attention-all.json` and
  all 11 variants (one added line each, nothing else changed).
- New `fixtures/forward/index.contract-v3-readable.json` = sample +
  `FORWARD_OPS`: `contractVersion: 3`, `contractReadableFrom: 2`,
  top-level `crashes`/`reports`, `summary.crashes`, an event
  `journal/crash`, an event and a non-crisis drift row
  `journal/boot-error`, a timeline kind `crash`. Written by
  `--write-index`, checked as sample + overlay, and must **fail** the v2
  schema. Documented in `fixtures/README.md`.
- New `fixtures/invalid/index.readable-from-above-version.json`
  (`contractVersion 2`, `contractReadableFrom 3`).

**Plugin** (commit 59a4bcda):

- `Model.js`: `readableFrom(data, version)` and `readsContract(version,
  from)`; `parseIndex` returns `readableFrom` and `newer` and accepts a
  newer readable index; a non-integer or non-finite version is never
  newer. `contractNewerNotice(parsed)`: neutral, title "The engine is
  newer than the plugin", detail "The engine writes index v3; this
  plugin reads v2 — update the plugin.", `command` /
  `script` = `UPDATE_PLUGIN_COMMAND` / `UPDATE_PLUGIN_SCRIPT` (Omarchy's
  `omarchy plugin update jax.seldon`, checked against `omarchy commands
  --json`: `omarchy plugin update [id] [--yes]`; no `--yes`, Omarchy
  shows the diff and asks), actions *Update* and *Copy*.
- `Service.qml`: `contractNotice`; `fix(…, "contract")` uses it as the
  source (terminal via `Model.terminalArgv`, i.e. only a constant
  script; copy via `wl-copy --`); snapshot fields `indexReadableFrom`,
  `contractNotice`, `contractActions`.
- `components/desk/Notices.qml`: the notice after the status banner.
- `manifest.json` unchanged (`seldon.contractVersion` stays 2). The
  preview check (`previewResult`) unchanged, strict.
- `tests/plugin/model.test.js`: the forward fixture → ok, `newer`,
  status ok, counts equal to the sample's, `pillTone` urgent, pill text,
  tooltip and crisis text equal, `sourceGlyph("journal")` is "•", the
  period table's timeline rows equal the sample's (unknown kind
  filtered), `deskChangelog`/`deskToday`/`deskWork`/`deskKpis`/
  `graphBuild` run on it; the notice's text, command, argv and actions.
  Second test: `contractReadableFrom: 3` → `contractMismatch`, no index,
  no notice, banner names the plugin; absent, null, "2", 2.5, 0, -1, 4,
  true, [2] → strict mismatch; 1 → ok; a version 2.5 or 1e400 →
  mismatch; an older index with `readableFrom 1` stays a mismatch; the
  own version with an out-of-range value reads as before.
- `tests/plugin/service-states.sh` 7a: the forward fixture in a private
  Quickshell → status ok, no banner, pill "2 · 2", tone urgent, the
  notice and its actions, log clean; with `contractReadableFrom: 3` →
  the mismatch banner, no notice, no pill.

**Docs:** CONTRACT.md rule 3 reworded, rule 9 extended, "Changing the
contract" names the obligation; SPEC-PLUGIN §3 and §5.6 (the notice,
its order); VERSIONING.md; SPEC-ENGINE's `contract-version` line (it
still said 1); plugin README state table; user guide en/de
(troubleshooting row, update page), German source lines bumped;
CHANGELOG Engine and Plugin.

## What was not done

- `seldon contract-version --json` still prints only `contractVersion`
  (the plugin does not run it; the WP did not ask).
- The proposal file's strict `onlyKeys` reading stays: ADR-0051 §3 makes
  it the later bump's job to keep the file a v2 reader can read or to
  say that v2 plugins lose the proposal (never misread it).
- No bar mark for "plugin behind": the notice is in the desk only, as
  the WP says ("quiet").
- The live check on the test host (the bar coloured with the forward
  fixture) is the orchestrator's: `SELDON_INDEX=<repo>/fixtures/forward/index.contract-v3-readable.json`.

## How it was verified

`SELDON_FULL_CHECK=1 just check` on the dev host, exit 0 ("check: ok"),
with `CARGO_TARGET_DIR`, `TMPDIR` and a private 0700 `XDG_RUNTIME_DIR`
under `jax-seldon-private/gates/` (on disk), `CARGO_BUILD_JOBS=4`
(the recipe's `cargo test` has no `-j`; the env var is its equivalent),
at the commit before this handover:

- fmt, clippy `-D warnings` (also `--features watch`): clean. The first
  run failed on `clippy::assertions_on_constants` in the new test; the
  bound is now a compile-time `const _: () = assert!(…)` in `lib.rs`
  (commit "engine: check the readable-from bound at compile time").
- engine tests: 108 test binaries `ok`, none failed (golden index =
  sample, the new `index.rs` and `contract_v2.rs` assertions, the
  validator's `minimum_and_maximum`).
- validate-fixtures: 151 instances (21 expected failures, including the
  forward and the new invalid fixture), 11 variants, 56 self-checks.
- docs-check ok; plugin-validate ok; qmllint ok (49 files).
- model.test.js 197 passed; terminal-scripts 65; real-home-guard 40;
  service-states 358 passed, 0 failed (including the new
  `contract-newer` and `contract-unreadable` cases); desk-view 1915;
  bar-view 196; ipc-restart 44; runtime-dir 45; install 229; deploy 324;
  guard mutants 85.

Not done here: the live check on the test host (the orchestrator's,
per the WP).

## Open questions

1. ADR-0051 is **proposed**; the operator accepts the text (E55 accepted
   the direction).
2. The forward fixture lives in a new folder `fixtures/forward/` (it is
   neither a v2 variant nor a plain invalid file: the plugin must read
   it). If the reviewer prefers `fixtures/invalid/`, it is a one-line
   mapping change in the validator plus the paths in two tests.
3. The Quickshell harnesses create their tiny runtime dir with
   `mktemp -d /tmp/seldon-rt.XXXXXX` (WP-161, the 107-byte socket path
   limit); that is the repo's harness design, not changed here. My own
   TMPDIR, cargo target and XDG_RUNTIME_DIR were on disk.
