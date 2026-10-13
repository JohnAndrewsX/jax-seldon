# ADR-0055 — A pinned Omarchy shell tree in CI

**Status:** proposed
**Date:** 2026-10-13

> Stage 2 of the plugin's checks in CI (operator decision E66; WP-191).
> Stage 1 (WP-190, no ADR) brought the node tests and Omarchy's pinned
> validator to CI. This ADR lets CI check out a second thing from the
> same pin, Omarchy's shell tree, and run the plugin's QML checks
> against it. The tree is the one Omarchy **stable** ships (E93: a release
> must work on stable; the test host's rc is the rehearsal, the dev host's
> edge a preview). It does not change the contract, the engine or the
> plugin. It stays *proposed* until the operator accepts it; the CI change
> does not merge before that.

## Context

1. **CI runs no QML.** `just qmllint` (with `tests/plugin/check-tokens.py`)
   and the four offscreen Quickshell harnesses (`service-states.sh`,
   `desk-view.sh`, `bar-view.sh`, `ipc-restart.sh`) read
   `$OMARCHY_PATH/shell` and need `quickshell`, so `SELDON_SKIP_HOST_CHECKS`
   skips them in CI. A pull request that breaks a binding is green there
   and caught only by the dev host's `just check`. WP-183b's readability
   report is specified "in CI" and cannot run there.
2. **"Code is truth" (AGENTS.md §1).** The reference for Omarchy is the
   tree an Omarchy install has (`/usr/share/omarchy`). A copy fetched by
   CI is a second tree; if it ever became the reference, the plugin could
   pass against a shell nobody runs.
3. **The pin already exists.** `packaging/omarchy-pin` (WP-190) names the
   `omacom/omarchy` commit whose validator equals the installed one.
4. **E93 (2026-10-13), channels.** A release must work on Omarchy
   *stable*, the channel of a fresh install. The test host runs *rc*, the
   rehearsal before a release. The dev host runs *edge* as a preview, and
   Seldon stays forward-compatible with it. No host runs stable.
5. **Measured 2026-10-13** (WP-191; both hosts on rc then, the dev host
   moves to edge under E93):
   - At the pinned commit (`c668141e`, tag `v4.0.4`) the shell is the
     top-level `shell/` directory. `shell/` and the theme colours
     (`themes/*/colors.toml`), 205 files, are byte-identical to the
     installed tree on the dev host and on the test host (both
     `omarchy version` 4.0.4-1).
   - Stable and rc ship the same package. `pkgs.omarchy.org/stable` and
     `pkgs.omarchy.org/rc` list `omarchy-4.0.4-1` with sha256 `78fa778d…`.
     The `usr/share/omarchy` of the stable package, downloaded and checked
     against that sha256, has the pinned tree's digest (`637b7166…`,
     `omarchy-tree.sh --digest`).
   - The harnesses copy `shell/Commons/` and `shell/Ui/` into a scratch
     config and read `themes/<theme>/colors.toml`. Those two modules
     import only Qt modules, `Quickshell*`, `qs.Commons`, `qs.Ui` and
     `../Commons/BorderGeometry.js`: the tree is self-contained for what
     the checks read.
   - Omarchy's own QML text-format scan is at the pin, at
     `test/shell.d/qml-text-format-scan.py`. It scans `ROOT/shell`, prints
     one line per `Text` with a non-literal binding and no `textFormat`,
     and exits 0 with findings (1 only when it can read nothing). The
     `omarchy` package does not install `test/`: `/usr/share/omarchy` has
     no `test/` on either host, and no recipe runs the scan.
   - `quickshell` is in Arch's official repositories (`extra`, 0.3.2-1;
     `qt6-declarative` 6.12.0-1). Both hosts run Quickshell 0.3.1 and
     Qt 6.11.2. Omarchy's package mirrors (`stable-mirror.omarchy.org`,
     `rc-mirror.omarchy.org`; the `default/pacman/mirrorlist-*` files of
     the tree) serve the same snapshot: `quickshell` 0.3.1-1, `qt6-base`
     6.11.2-3, `qt6-declarative` 6.11.2-1. Arch's own mirrors are already
     ahead.
   - Quickshell under `QT_QPA_PLATFORM=offscreen` loads Qt Quick's
     software scene-graph backend (`qt.scenegraph.general: Loading
     backend software`), so the checks need no GPU.

## Decision

### 1. The pin is Omarchy stable's tree, never a second reference

`packaging/omarchy-pin` names the commit of the release that the stable
channel ships (E93: a release must work on stable). Its reference is that
channel's `omarchy` package. At each refresh, the pinned tree's digest
equals the digest of `usr/share/omarchy` in the package that
`pkgs.omarchy.org/stable` lists, checked against the database's sha256,
or the digest of a stable install.

The test host's rc tree is the rehearsal (E93). While rc ships the same
package (2026-10-13: `omarchy-4.0.4-1` on both), the pin is also the test
host's tree. The dev host runs edge, a forward-compatibility preview, and
is never compared with the pin.

CI checks out from that commit only what the checks read: `shell/`,
`themes/*/colors.toml`, `default/pacman/mirrorlist-*` and
`test/shell.d/qml-text-format-scan.py`. `packaging/omarchy-tree.sh` does
this with a sparse, blob-filtered fetch over HTTPS of that one commit.
Before any check runs, the script proves:

- that HEAD is the pinned commit;
- that every file the `Commons` and `Ui` `qmldir` files name exists;
- that their imports stay inside the tree (Qt, `Quickshell*`,
  `qs.Commons`, `qs.Ui`, relative files under `shell/Commons` or
  `shell/Ui`);
- that the harnesses' themes and the scan are there.

It then prints a digest of `shell/` and the theme colours.
`omarchy-tree.sh --digest DIR` prints the same digest for an installed or
extracted tree, so the package and the pin are compared with one formula.
The pin file keeps its three keys (no new key).

No stable+rc matrix now. If rc ever needs a different pin than stable
(rc ships an `omarchy` whose tree differs from stable's while a release is
prepared), the step to take is a `channel: [stable, rc]` matrix: a second
pin file for rc, the job run once per channel against its own pin and
mirror, and a matrix-aware rule in `tests/release/workflow-pins.test.sh`.
That needs its own decision.

### 2. CI repeats what the hosts already run, with one exception

A separate job, `plugin-qml` in `ci.yml`, so stage 1's `check` job stays
as fast as it is. It runs, with `OMARCHY_PATH` set to the checkout:

- `just qmllint`, the token check included;
- `just plugin-test` with `SELDON_FULL_CHECK=1`: the node and bash parts
  and the four harnesses. The merge-base skip never applies there, and
  a log that says the harnesses were skipped fails the job;
- Omarchy's text-format scan on a copy of `plugin/`; any finding fails
  the job.

The first two are the recipes the hosts' `just check` runs against their
installed trees. The dev host's runs against edge (E93) and the test
host's against rc. CI never sets `SELDON_SKIP_HOST_CHECKS` in this job.

One exception: Omarchy's QML text-format scan is not installed with
Omarchy (`/usr/share/omarchy` has no `test/`), so only CI runs it, on a
copy of `plugin/`. The dev host does not run it. It is a second,
independent parser for a rule the dev host already enforces more
strictly: `plaintext.test.js` (E72) requires `textFormat: Text.PlainText`
on every `Text`, with an allow-list. A finding there that
`plaintext.test.js` does not report is a bug in one of the two parsers,
not a plugin change.

Every step that runs a recipe gets its own `HOME` and its own 0700
`XDG_RUNTIME_DIR` (and `JUST_TEMPDIR`), made in the runner's temp dir and
removed afterwards (AGENTS.md §6). Each Quickshell inside a harness gets
its own as on the dev host (WP-161). Every check runs under `shell: bash`
(`-eo pipefail`), so a red recipe piped to `tee` or a crashed scan fails
its step. The checkout keeps no token (`persist-credentials: false`):
Omarchy's scan and the plugin run as root in the container. The hosts'
gates are unchanged.

### 3. Quickshell and Qt come from Omarchy's stable mirror

No AUR package and no build in CI: `quickshell` is an official Arch
package. The job installs it, `qt6-declarative` and the tools the recipes
need from Omarchy's stable package mirror:

- It writes `Server = https://stable-mirror.omarchy.org/$repo/os/$arch` as
  the container's mirrorlist and runs `pacman -Syuu`, so the whole
  container matches that snapshot, downgrades included.
- The packages are Arch's, signed by Arch's packagers. The image is an
  unmodified Arch image, so pacman's default
  `SigLevel = Required DatabaseOptional` holds: every package is checked
  against the image's `archlinux-keyring`; the databases are not signed.
  A mirror therefore decides *which* signed packages CI sees, never their
  content; the *Versions* step is the record.
- A step compares that line with the pin's
  `default/pacman/mirrorlist-stable` and fails on a difference, so the
  mirror changes only with the pin.

The channel is stable by decision (E93), not by a host's channel.
`tests/release/workflow-pins.test.sh` refuses any other.

This pins the snapshot Omarchy ships to stable, not a package version.
The snapshot is expected to move with a stable release, the event that
moves the pin; CI does not check this (assumption, 2026-10-13), the
*Versions* step shows it.

The job prints to its log and its run summary:

- `quickshell --version`;
- `pacman -Q` of `quickshell`, `qt6-base`, `qt6-declarative` and the
  font;
- `qmllint --version`;
- the mirror line and the pinned commit.

The font is `ttf-jetbrains-mono-nerd`, the family `fc-match monospace`
gives on an Omarchy install. The harnesses' `fc-match` stub answers
`monospace`.

### 4. Where CI and the hosts disagree

The evidence word for these results is *CI*, never *test host* or
*headless* (AGENTS.md §5). Three cases (E93):

- **Red in CI** blocks a release. No host runs stable, so CI is the only
  automated check against the channel a release must work on. A CI
  failure that the test host does not reproduce is first a question about
  the pin or the mirror (the versions in the log), then about the plugin.
- **Green in CI, red on the test host (rc)** is a failed rc rehearsal and
  blocks a release, as today.
- **Green in CI, red on the dev host (edge)** is a forward-compatibility
  finding: a WP, not a release blocker.

### 5. Refreshing the pin

Refresh when Omarchy releases to stable (a new `omarchy` in
`pkgs.omarchy.org/stable`). WP-194's `just check-upstream` also prints its
pin notice ("CI validates with another validator") on a host whose
channel is stable or rc. On an edge host, the same difference is
information ("edge preview"), never a request to refresh: the pin follows
stable.

`check-upstream` reads the channel through Omarchy's own
`omarchy-version-channel`, which greps `/etc/pacman.d/mirrorlist` and
`/etc/pacman.conf`: a dev tool reading two files, no pacman, not a
collector (AGENTS.md §6).

1. Read the `omarchy` entry of
   `https://pkgs.omarchy.org/stable/x86_64/omarchy.db`: `%VERSION%`,
   `%FILENAME%` and `%SHA256SUM%`.
2. Take the release tag of that version and the validator's sha256, as
   for the validator (packaging/README.md, "The Omarchy pin").
3. Download that package into a scratch directory. Check its sha256
   against the database, then extract `usr/share/omarchy/shell` and
   `usr/share/omarchy/themes`. Two digests must be equal:
   - `bash packaging/omarchy-tree.sh --digest <scratch>/usr/share/omarchy`;
   - the digest `bash packaging/omarchy-tree.sh DIR` prints at the new
     commit.

   If they differ, stable ships a build between tags. Pick the commit
   whose tree matches, or leave the pin and record the difference as a
   known gap. CI never runs against a tree stable does not ship.
4. Change the pin and `tests/omarchy/assumptions.json`'s validator entry
   (WP-194) in one commit. CI on that pull request runs the new tree.

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
  turns CI red before review, against the Omarchy, Quickshell and Qt that
  stable ships. WP-183b can run its report in CI.
- CI fetches from two more places: `github.com/omacom/omarchy` (git over
  HTTPS, one commit) and Omarchy's stable package mirror. An outage there
  turns this job red; stage 1's `check` job does not depend on them.
- The job takes minutes, not seconds (a container sync and four
  harnesses); it runs in parallel with `check`.
- Between a stable release and the pin refresh, CI runs the previous
  stable; the log shows the versions. The test host's rc may be ahead of
  the pin: that is the rehearsal (§4).
- A refreshed pin is one more comparison (§5.3), done by whoever
  refreshes the validator pin today.
- Omarchy's text-format scan is Omarchy's code at the pin. CI runs it
  with `python3 -I` on a copy of `plugin/`, and only CI runs it (§2). If a
  later pin drops it, `omarchy-tree.sh` refuses the tree and the refresh
  has to decide.
