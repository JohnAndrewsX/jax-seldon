# WP-190 — Handover

Branch `wp/190-ci-plugin-tests` from `next` (9d776f9a). Not pushed (the
orchestrator pushes). Stage 1 only; no Quickshell and no `$OMARCHY_PATH/shell`
checkout (WP-191).

## What was done

- **`packaging/omarchy-pin`**: `repo=omacom/omarchy` (the upstream name was
  checked: `gh api repos/omacom/omarchy`, default branch `quattro`),
  `commit=c668141e9c42b13c80c9ca4ea108e11708c5e8a5` (tag `v4.0.4`, a
  lightweight tag), `validator_sha256=f7507e50…72c8`. The sha256 of
  `bin/omarchy-plugin-validate` at that commit equals the installed
  validator on the test host and on the dev host (both `omarchy version`
  4.0.4-1, 2026-10-09). The file is byte-identical to the installed one.
- **`packaging/omarchy-validate.sh PLUGIN_DIR`**: parses the pin strictly
  (exactly one of each key, nothing else). It fetches the one file from
  `raw.githubusercontent.com` with `curl --proto =https --proto-redir
  =https` and refuses it on a sha256 mismatch without running it. Then it
  runs it with `bash` on the folder. `--print-pin` prints the pin.
  `SELDON_OMARCHY_PIN` and `SELDON_OMARCHY_RAW` exist for tests only; a
  `file://` base is allowed only through the latter.
- **`justfile`**:
  - `plugin-validate`: without the omarchy CLI, it runs the pinned
    validator (in CI and anywhere else). With the CLI, it runs
    `omarchy plugin validate plugin/` as before, plus a notice when the
    installed validator's sha256 is not the pin's.
  - `plugin-test`: `node tests/plugin/model.test.js`, `model.bench.js`,
    `terminal-scripts.sh` and `real-home-guard.test.sh` now run under
    `SELDON_SKIP_HOST_CHECKS` too. Only the Quickshell harnesses are
    skipped there, with the "Quickshell harnesses skipped" notice that
    `deploy-test-host` refuses. `qmllint`, including `check-tokens.py`,
    stays host-only.
  - `check-packaging` runs the new test.
- **`model.bench.js` decision: it runs in CI with a CI budget.** The new
  `SELDON_BENCH_BUDGET_SCALE` (a number ≥ 1) multiplies both time budgets.
  CI sets 3 (30 ms and 24 ms); the dev host keeps 10 ms and 8 ms. The
  graph's path counter is not a timing and is not scaled. Basis: on the dev
  host, pinned to one core shared with two busy loops, the ×10 median was
  about 6 ms (idle about 1.8 ms). A slow runner stays clear of 30 ms, and
  a regression of an order of magnitude still fails. No CI timing has been
  measured yet (see below).
- **`ci.yml`**: `nodejs` is added to the pacman line. The header comment
  says what runs and what does not.
- **`release.yml`**:
  - `nodejs` is installed in the build job, because `just check` there now
    runs the node tests.
  - `Plugin split` gets `id: split` and outputs the split. The build job
    outputs `plugin_split`.
  - A new step `Validate the plugin split` runs after `Plugin split` and
    before `Attest the release assets` and `Summary`. It is unconditional
    and has no `continue-on-error`. It does `git archive "$SPLIT" | tar -x`
    into a `mktemp -d` and runs `packaging/omarchy-validate.sh` on it.
  - The `plugin` job recomputes the split and refuses to push unless it
    equals `needs.build.outputs.plugin_split`. This is an addition beyond
    the WP's text: it ties the pushed split to the validated one instead
    of relying on determinism. The reviewer may drop it.
- **`tests/release/omarchy-pin.test.sh`** (new, offline, `file://` mirror),
  20 cases:
  - The real pin is well-formed.
  - A validator with the pinned sha256 runs; its failure fails the script.
  - A wrong sha256 is refused and the file is never run, both in a test
    pin and through the real pin.
  - Malformed pins are refused: short commit, upper-case commit, missing
    key, key twice, unknown key, trailing space, repo with a path.
  - A missing file, a plain `http://` URL (curl's "Protocol "http" is
    disabled") and a missing folder are refused.
  - Where the installed validator equals the pin, it is served through
    the mirror. It passes `plugin/` and fails a missing entry point file,
    a kind without its entry point, and an `omarchy.*` id.
- **`tests/release/workflow-pins.test.sh`**: asserts the split validation.
  The step exists, has no `if:` or `shell:`, takes the split step's
  output, extracts with `git archive`, ignores no failure, and has the
  validator as its last line. The order is Plugin split, then Validate
  the plugin split, then Summary. The split output and the `plugin_split`
  job output exist, and the plugin job compares before it pushes.
  11 new mutants, each caught (44 cases in all).
- **Docs**:
  - `docs/TESTING.md`: the Packaging, Plugin manifest and Plugin logic
    rows are corrected. The CI section is renamed "What CI runs, and what
    only the dev host runs" (no links pointed at the old anchor) and has
    a "what CI runs" list. `:364`'s "needs the omarchy CLI" is corrected.
    The bench's scale is documented.
  - `packaging/README.md`: file table rows, the build and plugin rows,
    and a section "The Omarchy pin" that says how to refresh it.
  - `CONTRIBUTING.md` and `docs/DEVELOPMENT.md`: the host-only lists are
    corrected.

## What was not done

- No CI run, no push, no release dry run (operator brief: CI runs only on
  PRs and main pushes; a dry run creates public attestations). The four
  CI acceptance items are **not run**; see below for how to run them.
- WP-191 scope (Quickshell, qmllint, harnesses in CI) is untouched.

## How it was verified

All local runs used `TMPDIR`, `CARGO_TARGET_DIR` (`gates/target-wp190`) and
a private 0700 `XDG_RUNTIME_DIR` on disk under `jax-seldon-private/gates/`,
and `CARGO_BUILD_JOBS=4`.

| Check | Where | Result |
|---|---|---|
| `SELDON_FULL_CHECK=1 just check` at `1e47fd4f` (harnesses ran) | fixture, headless (dev host) | exit 0; log `gates/check-wp190.log` |
| `bash tests/release/omarchy-pin.test.sh` | fixture (dev host) | 20 ok, including the real-validator cases |
| `bash tests/release/workflow-pins.test.sh` | fixture (dev host) | 44 ok |
| `bash packaging/omarchy-validate.sh plugin/`, real HTTPS fetch | dev host | ok, sha256 matched |
| `SELDON_SKIP_HOST_CHECKS=1 just plugin-test` (CI mode) | fixture (dev host) | node and bash parts ok, harnesses skipped with the notice |
| Mutant: `Model.js` `pillText` drops the separator → `node tests/plugin/model.test.js` | fixture (dev host) | exit 1 (red) |
| Mutant: manifest `entryPoints.service = "Missing.qml"` → pinned validator, real fetch | dev host | exit 1, "entry point file not found" |
| Mutant: last hex digit of `validator_sha256` changed → `omarchy-validate.sh`, real fetch | dev host | exit 1, "refusing …", validator not run |
| Release split: `git subtree split --prefix=plugin`, then `git archive \| tar -x`, then pinned validator on HEAD | dev host | split `e8eb639a…` valid |
| `just plugin-validate` without the omarchy CLI on PATH | **not run** | the guard blocked the scratch PATH of tool symlinks ("a link to a protected directory at a path the guard cannot know"). Not routed around; CI runs exactly this on the PR |
| `shellcheck` on the new and changed scripts | **not run locally** (not installed on the dev host); CI | `check-packaging` ran `bash -n` only |
| Red CI on a `Model.js` mutant | **not run** (CI) | — |
| Red CI on a manifest with a missing entry point | **not run** (CI) | — |
| Wrong sha256 in `packaging/omarchy-pin` refused | fixture (above); CI **not run** | — |
| Release dry run (`workflow_dispatch`) validates the split | **not run** (CI) | — |
| `model.bench.js` timing on a CI runner | **not run** (CI) | the ×3 budget is an estimate (see above) |
| `just check` on the dev host unchanged and green | fixture, headless (dev host) | green; `plugin-validate` still uses the installed CLI, no notice (sha256 equal) |

**To run the CI acceptance (orchestrator):** open the PR; the green run
covers the pinned validator, the node and bash tests, and shellcheck. On
a throwaway branch, push three commits one at a time, each expected red:
the `pillText` mutant, `entryPoints.service = "Missing.qml"`, and a
changed last digit in `packaging/omarchy-pin`. Then run
`gh workflow run release.yml --ref <branch>` and check the step
`Validate the plugin split`. Link the runs here.

## Open questions

- Does the plugin job's split comparison stay (an addition beyond the WP)?
- CI now also fetches from `raw.githubusercontent.com` for
  `plugin-validate` (before, only the pacman toolchain and the checkout).
  An outage there turns CI red (curl `--retry 3`).
