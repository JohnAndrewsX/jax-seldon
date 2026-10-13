# ADR-0055 — A pinned Omarchy shell tree in CI

**Status:** proposed
**Date:** 2026-10-13

> Stage 2 of the plugin's checks in CI (operator decision E66; WP-191).
> Stage 1 (WP-190, no ADR) brought the node tests and Omarchy's pinned
> validator to CI. This ADR lets CI check out a second thing from the
> same pin, Omarchy's shell tree, and run the plugin's QML checks
> against it. It does not change the contract, the engine or the plugin.
> It stays *proposed* until the operator accepts it; the CI change does
> not merge before that.

## Context

1. **CI runs no QML.** `just qmllint` (with `tests/plugin/check-tokens.py`)
   and the four offscreen Quickshell harnesses (`service-states.sh`,
   `desk-view.sh`, `bar-view.sh`, `ipc-restart.sh`) read
   `$OMARCHY_PATH/shell` and need `quickshell`, so `SELDON_SKIP_HOST_CHECKS`
   skips them in CI. A pull request that breaks a binding is green there
   and caught only by the dev host's `just check`. WP-183b's readability
   report is specified "in CI" and cannot run there.
2. **"Code is truth" (AGENTS.md §1).** The reference for Omarchy is the
   tree installed on the machine (`/usr/share/omarchy`). A copy fetched by
   CI is a second tree; if it ever became the reference, the plugin could
   pass against a shell nobody runs.
3. **The pin already exists.** `packaging/omarchy-pin` (WP-190) names the
   `omacom/omarchy` commit whose validator equals the one installed on
   the test host and the dev host.
4. **Measured 2026-10-13** (WP-191):
   - At the pinned commit (`c668141e`, tag `v4.0.4`) the shell is the
     top-level `shell/` directory. `shell/` and the theme colours
     (`themes/*/colors.toml`), 205 files, are byte-identical to the
     installed tree on the dev host and on the test host (both
     `omarchy version` 4.0.4-1).
   - The harnesses copy `shell/Commons/` and `shell/Ui/` into a scratch
     config and read `themes/<theme>/colors.toml`. Those two modules
     import only Qt modules, `Quickshell*`, `qs.Commons`, `qs.Ui` and
     `../Commons/BorderGeometry.js`: the tree is self-contained for what
     the checks read.
   - Omarchy's own QML text-format scan is at the pin, at
     `test/shell.d/qml-text-format-scan.py`. It scans `ROOT/shell`, prints
     one line per `Text` with a non-literal binding and no `textFormat`,
     and exits 0 with findings (1 only when it can read nothing).
   - `quickshell` is in Arch's official repositories (`extra`, 0.3.2-1;
     `qt6-declarative` 6.12.0-1). Both hosts run Quickshell 0.3.1 and
     Qt 6.11.2 on Omarchy's `rc` channel. Omarchy's own package mirrors
     (`rc-mirror.omarchy.org`, `stable-mirror.omarchy.org`; the
     `default/pacman/mirrorlist-*` files of the tree) serve exactly those:
     `quickshell` 0.3.1-1, `qt6-base` 6.11.2-3, `qt6-declarative`
     6.11.2-1. Arch's own mirrors are already ahead of the hosts.
   - Quickshell under `QT_QPA_PLATFORM=offscreen` loads Qt Quick's
     software scene-graph backend (`qt.scenegraph.general: Loading
     backend software`), so the checks need no GPU.

## Decision

### 1. The pin is a mirror of the test host's tree, never a second reference

`packaging/omarchy-pin` names the commit that matches the test host's
installed Omarchy. CI checks out from that commit only what the checks
read: `shell/`, `themes/*/colors.toml`, `default/pacman/mirrorlist-*` and
`test/shell.d/qml-text-format-scan.py` (`packaging/omarchy-tree.sh`, a
sparse, blob-filtered fetch over HTTPS of that one commit). Before any
check runs, the script proves that HEAD is the pinned commit, that every
file the `Commons` and `Ui` `qmldir` files name exists, that their
imports stay inside the tree (Qt, `Quickshell*`, `qs.Commons`, `qs.Ui`,
relative files under `shell/Commons` or `shell/Ui`), that the harnesses'
themes and the scan are there, and prints a digest of `shell/` and the
theme colours. `omarchy-tree.sh --digest DIR` prints the same digest for
an installed tree, so the test host and the pin are compared with one
formula. The pin file keeps its three keys (no new key).

### 2. CI repeats what the dev host already runs, nothing more

A separate job, `plugin-qml` in `ci.yml`, so stage 1's `check` job stays
as fast as it is. It runs, with `OMARCHY_PATH` set to the checkout:

- `just qmllint`, the token check included;
- `just plugin-test` with `SELDON_FULL_CHECK=1`: the node and bash parts
  and the four harnesses. The merge-base skip never applies there, and
  a log that says the harnesses were skipped fails the job;
- Omarchy's text-format scan on a copy of `plugin/`; any finding fails
  the job.

These are the same recipes the dev host's `just check` runs against the
installed tree; CI adds no check of its own and never sets
`SELDON_SKIP_HOST_CHECKS` in this job. Every step that runs a recipe gets
its own `HOME` and its own 0700 `XDG_RUNTIME_DIR` (and `JUST_TEMPDIR`),
made in the runner's temp dir and removed afterwards (AGENTS.md §6); each
Quickshell inside a harness gets its own as on the dev host (WP-161).
The dev host's gate is unchanged: `just check` there still reads
`/usr/share/omarchy` and still runs everything CI runs.

### 3. Quickshell and Qt come from Omarchy's mirror of the test host's channel

No AUR package and no build in CI: `quickshell` is an official Arch
package. The job installs it, `qt6-declarative` and the tools the
recipes need from Omarchy's package mirror for the channel the test host
uses (`rc` today): the job writes `Server =
https://rc-mirror.omarchy.org/$repo/os/$arch` as the container's
mirrorlist and runs `pacman -Syuu`, so the whole container matches that
snapshot (downgrades included). The packages are Arch's, signed by Arch's
packagers, and pacman checks them against the image's
`archlinux-keyring` as on any Arch mirror. A step compares that line with
the pin's `default/pacman/mirrorlist-rc` and fails on a difference, so the
mirror changes only with the pin.

What this pins: the snapshot Omarchy ships to that channel, not a
package version. The snapshot moves when Omarchy moves the channel, the
same event that moves the pin. The job prints `quickshell --version`,
`pacman -Q` of `quickshell`, `qt6-base`, `qt6-declarative` and the font,
`qmllint --version`, the mirror line and the pinned commit to its log
and its run summary. The font is `ttf-jetbrains-mono-nerd`, the family
`fc-match monospace` gives on an Omarchy install; the harnesses' `fc-match`
stub answers `monospace`.

If the test host moves to another channel, the job's mirror line and the
compared file move with it, in the commit that records the move.

### 4. Where CI and the test host disagree, the test host wins

A check that passes in CI and fails on the dev host or the test host
fails. A CI failure that the dev host does not reproduce against the
installed tree is first a question about the pin or the mirror (the
versions in the log), not about the plugin. The evidence word for these
results is *CI*, never *test host* or *headless* (AGENTS.md §5).

### 5. Refreshing the pin after an `omarchy update`

On the event that moves the test host (the operator's `omarchy update`),
or when WP-194's `just check-upstream` prints a mismatch or its pin
notice ("CI validates with another validator"):

1. Read `omarchy version` and `omarchy version channel` on the test host.
2. Take the release tag of that version, as for the validator
   (packaging/README.md, "The Omarchy pin"), and the validator's sha256.
3. `bash packaging/omarchy-tree.sh DIR` at the new commit and
   `bash packaging/omarchy-tree.sh --digest /usr/share/omarchy` on the
   test host (from the deployed clone): the two digests must be equal.
   If they differ, the test host runs a build between tags: pick the
   commit whose tree matches, or leave the pin and record the difference
   as a known gap. CI never runs against a tree the test host does not
   have.
4. Change the pin, `tests/omarchy/assumptions.json`'s validator entry
   (WP-194) and, on a channel change, the job's mirror line in one
   commit. CI on that pull request runs the new tree.

### 6. What CI never claims

CI renders offscreen in a container without a compositor, so it never
shows, and a CI result never stands for:

- **layer-shell geometry**: where the bar and the desk sit on a real
  output, exclusive zones, the `PanelWindow` and `WlrLayershell` placement
  (the harness replaces `DeskWindow.qml` with a plain item);
- **live IPC with the real shell**: `omarchy-shell`, the facade's
  injected API and `qs ipc` against the running shell (`ipc-restart.sh`
  talks only to its own Quickshells);
- **monitors**: real outputs, their scales and hot-plugging (`out125` is
  `QT_SCALE_FACTOR`, not an output), and Hyprland (`hyprctl` is a stub);
- the real engine (the harnesses use `fake-seldon`), the user's fonts and
  theme hooks, and the runtime smoke test (docs/TESTING.md, "Runtime smoke
  test in the shell").

Those stay with the test host and the operator's desktop.

## Consequences

- A pull request that breaks a QML binding, a token or a `Text`'s format
  turns CI red before review, against the same Omarchy and Quickshell the
  hosts run. WP-183b can run its report in CI.
- CI fetches from two more places: `github.com/omacom/omarchy` (git over
  HTTPS, one commit) and Omarchy's package mirror. An outage there turns
  this job red; stage 1's `check` job does not depend on them.
- The job takes minutes, not seconds (a container sync and four
  harnesses); it runs in parallel with `check`.
- Between an Omarchy channel move and the operator's update of the test
  host, CI can run newer Quickshell or Qt than the test host. The log
  shows it; §4 decides.
- A refreshed pin is one more comparison (§5.3), done by whoever
  refreshes the validator pin today.
- Omarchy's text-format scan is Omarchy's code at the pin; CI runs it
  with `python3 -I` on a copy of `plugin/`. If a later pin drops it,
  `omarchy-tree.sh` refuses the tree and the refresh has to decide.
