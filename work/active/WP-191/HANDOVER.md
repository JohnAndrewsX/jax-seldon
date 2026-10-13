# WP-191 — Handover

Branch `wp/191-ci-quickshell` (from `next` at `f0bcf5b2`), worktree
`wt/WP-191`. CI only: no engine, plugin, schema or contract change.

**ADR-0055 is proposed.** The operator accepts it before the CI change
(`ci.yml`, `workflow-pins.test.sh`) merges. Nothing here has run in CI:
every CI acceptance below is *not run*.

## What was measured (2026-10-13)

- The pin (`packaging/omarchy-pin`, `c668141e`) is tag `v4.0.4`; both
  hosts report `omarchy version` 4.0.4-1, channel `rc`. At the pin the
  shell is the top-level `shell/` (183 files). `shell/` plus
  `themes/*/colors.toml` (205 files) are byte-identical at the pin, on the
  dev host and on the test host: `omarchy-tree.sh` digest
  `637b7166a05ce69e290cce88a1a7c22e8f04afbc0b41fcbe53969a220466ff6e` on all
  three (the checkout, and `--digest /usr/share/omarchy` on each host).
- `shell/Commons` and `shell/Ui` (what the harnesses copy) import only Qt,
  `Quickshell*`, `qs.Commons`, `qs.Ui` and `../Commons/BorderGeometry.js`:
  self-contained for the checks.
- Omarchy's scan is at the pin, `test/shell.d/qml-text-format-scan.py`. It
  reads `ROOT/shell` and exits 0 with findings, so the job fails on any
  output line.
- `quickshell` is an official Arch package (`extra`: 0.3.2-1, Qt 6.12.0).
  The hosts run Quickshell 0.3.1 and Qt 6.11.2. Omarchy's channel mirrors
  (`rc-mirror.omarchy.org`, `stable-mirror.omarchy.org`) serve exactly
  those: `quickshell` 0.3.1-1, `qt6-base` 6.11.2-3, `qt6-declarative`
  6.11.2-1. ADR-0055 §3 takes them from the `rc` mirror, so no AUR and no
  build.
- Offscreen Quickshell loads Qt Quick's software backend; no GPU needed.

## What was done

- **`decisions/ADR-0055-pinned-omarchy-shell-tree-in-ci.md`** (proposed)
  and its `DECISIONS.md` row. It covers:
  - the pin as a mirror of the test host's tree, never a second reference;
  - CI repeats the dev host's recipes and adds no check of its own;
  - Quickshell and Qt from Omarchy's mirror of the test host's channel;
  - the test host wins;
  - the refresh after an `omarchy update` (with WP-194's notice and a
    digest comparison);
  - what CI never claims: layer-shell geometry, live IPC with the real
    shell, monitors and Hyprland, the real engine, fonts, the runtime
    smoke test.
- **`packaging/omarchy-tree.sh`**: a sparse, blob-filtered fetch of the
  pinned commit over HTTPS only. It checks out `shell/`,
  `themes/*/colors.toml`, `default/pacman/mirrorlist-*` and the scan.
  - It proves HEAD is the pinned commit, every `Commons`/`Ui` `qmldir`
    entry exists, their imports stay inside the tree, and the harness
    themes and the scan are present.
  - It prints the digest. `--digest DIR` gives the same digest for an
    installed tree.
  - Its test is `tests/release/omarchy-tree.test.sh`: 19 cases against
    offline `file://` origins, in `just check-packaging`.
- **`.github/workflows/ci.yml`**: a new job, `plugin-qml`, in parallel to
  `check`, using the same container image and the same `needs`/`if`.
  - It installs `quickshell`, `qt6-declarative`, the font and the recipes'
    tools from the `rc` mirror (`pacman -Syuu`).
  - It fetches the pinned tree, then `cmp`s the mirror line against the
    pin's `mirrorlist-rc`.
  - It prints the versions to the log and the run summary: Quickshell,
    `pacman -Q`, qmllint, the mirror and the commit.
  - Then three steps run against `OMARCHY_PATH=<tree>`:
    - `just qmllint`, which includes `check-tokens.py`;
    - `just plugin-test`, with `SELDON_FULL_CHECK=1` and the budgets
      tripled. The four harnesses always run, and a "skipped" line fails
      the step;
    - Omarchy's scan on a copy of `plugin/`.
  - Each of the three has `if: !cancelled() && steps.tree.outcome ==
    'success'`, so a red run shows all three.
  - Each recipe step has its own `HOME`, 0700 `XDG_RUNTIME_DIR` and
    `JUST_TEMPDIR`, made in `$RUNNER_TEMP` and removed by an EXIT trap.
  - The steps use `shell: bash`, so `pipefail` holds on the `| tee`.
  - Inside, the harnesses keep their WP-161 setup:
    - a `HOME` per case;
    - a private runtime dir per harness script (per Quickshell in
      `ipc-restart.sh`);
    - never the session's runtime dir.
- **`tests/release/workflow-pins.test.sh`**: rules for the job, with 19
  new mutants, each caught. The rules:
  - the job exists;
  - no `SELDON_SKIP_HOST_CHECKS`, no `continue-on-error`, no `|| true` or
    `set +e`;
  - the mirror's channel equals the compared `mirrorlist-<channel>`;
  - quickshell and qt6-declarative are installed;
  - the tree step;
  - exactly three `if:` lines, each the tree-gated one;
  - the private-dir lines;
  - the exact recipe lines;
  - the skip and scan refusals.
- **`tests/release/mirror-image.test.sh`**: reads the one mirror digest
  from `ci.yml` with `sort -u`, because two jobs now name it. A second
  digest or tag there fails the test.
- **`justfile`**: `check-packaging` runs and lints the new script and
  test. Comments now name the CI job; no recipe changed behaviour.
- **Docs**:
  - `docs/TESTING.md`:
    - what each CI job runs;
    - the full `plugin-qml` description;
    - the evidence word *CI*;
    - what CI never shows;
    - the dev host stays the gate.
  - `packaging/README.md`: the script, and a refresh step for the shell
    tree using the digest.
  - `CONTRIBUTING.md` and `docs/DEVELOPMENT.md`: the job split.

## How it was verified

All local runs were on the dev host, in a scratch directory outside the
repo, never under `/tmp` (`TMPDIR` pointed there too). The CI job's steps
were replayed from a clone at `24873061`, with `RUNNER_TEMP` in that
scratch dir and `OMARCHY_PATH` on the sparse checkout of the pin made by
`omarchy-tree.sh`.

| Check | Where | Result |
|---|---|---|
| `omarchy-tree.test.sh` (19 cases), `workflow-pins.test.sh` (95, 19 new mutants) | fixture | ok |
| `omarchy-tree.sh` against GitHub at the pin: HEAD, self-contained, themes, scan, digest | fixture (real fetch, dev host) | ok, digest `637b7166…` |
| The same digest of `/usr/share/omarchy` | test host, dev host | equal |
| "Mirror against the pin": the job's `echo` line written to a scratch file, `cmp` against the pin's `mirrorlist-rc` | fixture | equal |
| "Versions" step (without `pacman -Q`, no pacman here) | dev host's own tools | Quickshell 0.3.1, qmllint 6.11.2: the `rc` mirror's versions |
| `just qmllint` step, private `HOME`/rt | fixture | `tokens: ok (1033 references)`, `qmllint: ok (53 files)` |
| `just plugin-test` step, private `HOME`/rt, under the shared lock | headless | ok: service-states 367, desk-view 2723, bar-view 196, ipc-restart 44 passed, 0 failed; no skip line; by-id leftovers 0 (session dir unchanged); private dirs removed |
| Scan step | fixture | `text-format scan: ok (53 QML files)` |
| Mutant: hard-coded colour (`BarWidget.qml` glyph `color: pill.ink` → `"#ff0000"`), all three steps | headless | **red** in `plugin-test`: bar-view 17 failed (`tint = #ff0000 (want …)` per theme and scale). qmllint and the scan stay green. |
| Mutant: broken `Text` binding (`countsText` `text: button.text` → `button.txet`) | headless | **red** in `plugin-test`: desk-view 222 failed (`Unable to assign [undefined] to QString`, "log has errors"). qmllint and the scan stay green. |
| Mutant: `textFormat: Text.PlainText` dropped on `countsText` | headless, fixture | **red** twice: `plugin-test` (`plaintext.test.js`) and Omarchy's scan (`plugin/BarWidget.qml:291: text binding without textFormat`) |
| Full gate `flock /tmp/seldon-check.lock env SELDON_FULL_CHECK=1 just check` | fixture, headless | `check: ok` at `95033588` (service-states 367, desk-view 2722, bar-view 195, ipc-restart 43 passed, 0 failed; no skip line; the real state and config dirs untouched). The first two runs were red, both from this WP, both fixed: `mirror-image.test.sh` read `ci.yml`'s image line, now there twice (`sort -u`, as the script does); a `workflow-pins` mutant named `XDG_RUNTIME_DIR` after `)`, which `check-runtime-dir` reads as the session's dir (rewritten as an assignment after a blank) |
| The job green in CI with the versions in its log | CI | **not run** |
| The colour and binding mutants red in CI | CI | **not run** |
| Install step (`pacman -Syuu` from the `rc` mirror in the container), `pacman -Q`, `actions/checkout`, writing `/etc/pacman.d/mirrorlist`, running as root in the container, the container's fonts, the GitHub runner | CI | **not run** |

Notes on the local replay:

- The replays ran in the tool's shell, where `set -e` in a subshell had no
  effect (checked: `( set -eo pipefail; false; echo after )` prints
  `after`). Pass and fail were read from the logs, not from exit codes:
  - "plugin-test: ok" / `error: recipe \`plugin-test\` failed`;
  - the `FAIL` lines;
  - the scan's output.

  In CI, `shell: bash` runs each step with `-eo pipefail`.
- The replay counts one more `ok` per harness than the gate (2723
  against 2722, …). With a private `XDG_RUNTIME_DIR` inherited, each
  harness also reports the by-id dir under it (0 before, 0 after).
- `check-runtime-space` ran in the dev host's gate. CI skips it, as
  before.

## What only CI can show

- That the `rc` mirror installs into the pinned `base-devel` image.
  `-Syuu` may downgrade, e.g. glibc and `archlinux-keyring`: the image is
  newer than the mirror's snapshot. A keyring older than a packager's key
  would fail the install.
- That Quickshell runs as root in the container.
- That the harnesses' timings hold on a runner (budgets ×3, as in
  `check`).
- The container's fonts: `ttf-jetbrains-mono-nerd` only. The harnesses'
  `fc-match` stub answers `monospace`, so pixel widths may differ from the
  hosts.

A first run on a pushed branch shows these. A throwaway branch per
mutant shows the red runs. Both are the operator's or the reviewer's
call, since I cannot push.

## Not done / open

1. **ADR-0055 acceptance** by the operator, before merge.
2. **The token check does not catch a hard-coded colour.**
   `check-tokens.py` has no rule against a hex literal. The colour mutant
   was caught only because bar-view checks that glyph's tint. A hex colour
   on an element no harness measures would pass every check, locally and
   in CI. Proposal: a house rule for `color:`/`*Color:` literals in
   `plugin/` (a small follow-up WP; not in this WP's scope).
3. **qmllint does not catch the misspelt property** (`button.txet`). Only
   the harness log check did. Same follow-up as item 2 if wanted.
4. **`docs/TESTING.md` says the test host has `/usr/bin/qmllint`**
   (lines ~592 and ~690). Measured 2026-10-13: the test host has no
   `/usr/bin/qmllint`, only `/usr/lib/qt6/bin/qmllint`, as on the dev
   host. The recipe finds it either way (`PATH`, then the Qt dir). Left
   unchanged here; it is a one-line fix.
5. **Mirror dependency.** An outage of `rc-mirror.omarchy.org` or GitHub
   turns `plugin-qml` red; `check` does not depend on either
   (ADR-0055, Consequences).
6. **Channel move.** If the test host leaves `rc`, the `echo` line and
   the compared file change in one commit. `workflow-pins` enforces that
   they agree.
