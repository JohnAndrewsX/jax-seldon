WP-044 HANDOVER

Branch `wp/044-install-script`, worktree `wt/WP-044`, on top of `main` at
`549f40e`. Not pushed, no PR. Commits `main..HEAD`:
`71c2330` install.sh, test, recipe, release asset · `dcaf9bd` banner
text constant · `952f190` READMEs · `338782b` memory · `6d93201`
handover · review round 1: `2d59286` `--force` and SC2001 · `3041d2f`
the one-liner as the banner fix and docs · then `work: WP-044 handover
after review` (this file).

## Review round 1 (APPROVE; fix round per the reviewer, ADR-0024)

1. **SC2001** at `tests/install/install.test.sh:146` (the dry run's only
   finding): `echo "$out" | sed 's/^/     /'` is now
   `printf '     %s\n' "${out//$'\n'/$'\n'     }"`. The other `sed`
   indents read files, not `echo`, so SC2001 does not apply to them.
2. **`INSTALL_ENGINE_COMMAND`** is now
   `curl -fsSL https://github.com/JohnAndrewsX/jax-seldon/releases/latest/download/install.sh | bash`.
   It stays a constant and the argv is unchanged
   (`wl-copy -- <command>`,
   `omarchy-launch-floating-terminal-with-presentation <command>`). The
   pipe works because the launcher runs its argument with `bash -c`. The
   comment above it names ADR-0024 and the flip back to
   `omarchy pkg aur add jax-seldon` once the AUR package is live.
   `ENGINE_MISSING_DETAIL` now describes what the button does: "AUR
   package: coming soon; until then install from GitHub: the command
   below downloads install.sh from the release, which checks the engine
   against SHA256SUMS. Then check again."
   - Tests: `model.test.js` pins the new string. `service-states.sh`
     (`fix-engine`) pinned the AUR string as well and was not on the
     review list; it now expects the one-liner for both `wl-copy` and
     the launcher.
   - plugin/README.md: the States row now names the one-liner. The
     Security list ("one of five constants") has the one-liner first,
     with why it is safe. The **No network** bullet gained one honest
     sentence: the engine-missing click runs `curl … | bash` in a
     visible terminal.
   - SPEC-PLUGIN §3 paragraph and the §5 banner line are rewritten.
     CONTRACT.md is untouched.
3. **`--force`.** A `<prefix>/bin/seldon` that the manifest does not
   record (no manifest, a different hash, or a symlink) is now a refusal
   before any write: exit 1, "… exists and was not installed by
   install.sh (a self-built seldon?); nothing changed. Re-run with
   --force to replace it." I applied the same rule to the `--unit` file
   (a unit copied by hand per engine/systemd/README.md). A file equal to
   the download is never a refusal. `--force` with `--uninstall` is a
   usage error. New tests, 106 checks in total: self-built binary
   refused with nothing changed, then `--force` replaces it and writes
   the manifest; a binary rebuilt over an installed one is refused and
   kept; `seldon` as a symlink is refused and kept; another prefix's
   unit is refused without `--force`. The cross-prefix `--unit` tests
   now pass `--force`.
4. **plugin/README.md, Install and Remove:** both say that
   `releases/latest/download/install.sh` exists from the next release on
   (v0.1.1). Install gives the raw-URL stopgap with `--version v0.1.0`,
   as README.md does; README.md now names v0.1.1 too.
5. **"Download, read, verify, run"** in both READMEs, with a
   `less install.sh  # read what it does` line in the block. README.md's
   options table gained `--force`.
6. **`Banner.qml:9`** comment: the detail wraps (the engine-missing one
   runs to a few lines), and the command wraps anywhere (the one-liner
   is a long URL). Comment only; qmllint is clean.
7. `ci.yml` is not touched.

Verified (round 1, at `3041d2f`):

```
$ bash tests/install/install.test.sh   → install.test: 106 passed, 0 failed
$ node tests/plugin/model.test.js      → model.test.js: 74 passed
$ just check                           → exit 0
  check-packaging: ok · install.test: 106 passed, 0 failed
  validate-fixtures: ok · plugin-validate: ok · tokens: ok (520 references)
  qmllint: ok (28 files) · model.test.js: 74 passed
  service-states: 189 passed · panel-view: 681 passed · overlay-view: 314 passed
  check: ok
```

Real run again, final script, v0.1.0 from GitHub, scratch prefix
`/tmp/seldon-wp044-real` with a fake self-built `seldon` placed first:
plain run → exit 1 with the `--force` message and the fake untouched
(`seldon 0.0.0-dev`). `--force` → exit 0, `seldon 0.1.0`. Re-run → exit
0, 3× unchanged. `--uninstall` → exit 0, 0 files left. My fingerprint of
the real `~/.local/bin/seldon` (+ sha256), `jax-seldon`,
`~/.config/systemd/user` and `~/Seldon` was identical before and after.

Note for the next dry run: shellcheck still cannot run here. The only
new shell constructs are `awk` with a single-quoted program,
`[[ … ]] || refuse` chains and `${out//$'\n'/…}`.

**Until v0.1.1 is published, the banner's one-click fix gets a 404**
from `releases/latest/download/install.sh` (curl `-f` exits 22 in the
terminal and nothing is installed). It starts working with the first
release built from this branch's workflow change.

## Done

- **`install.sh`** (repository root, bash, executable):
  `install.sh [--version vX.Y.Z] [--prefix DIR] [--unit]`,
  `install.sh --uninstall [--prefix DIR]`, `-h/--help`.
  - No `--version`: asks `api.github.com/repos/JohnAndrewsX/jax-seldon/releases/latest`
    for `tag_name`, with `jq` when present, else a `sed` match. The tag
    must be `vX.Y.Z`. `--version 0.1.0` is accepted as `v0.1.0`.
  - Downloads `seldon-X.Y.Z-x86_64-unknown-linux-musl.tar.gz` and
    `SHA256SUMS` into a `mktemp -d` dir, which is removed on exit. It
    takes the tarball's single line from `SHA256SUMS` and runs
    `sha256sum -c --strict`. A mismatch or a missing line stops it with
    exit 2 before anything is written. It then extracts the binary and
    the unit and runs the new `seldon --version`; output other than
    `seldon X.Y.Z` is refused.
  - Installs `<prefix>/bin/seldon` (temp file in the same dir, then
    `mv`, so the replacement is atomic) and `<prefix>/bin/jax-seldon ->
    seldon` (relative symlink). With `--unit` it also installs
    `${XDG_CONFIG_HOME:-~/.config}/systemd/user/seldon-watch.service`,
    never enabled. For `~/.local` the unit stays byte-identical to the
    release's. Under another prefix in HOME, `ExecStart` becomes
    `%h/<rel>/bin/seldon watch`. Elsewhere it is the absolute path. A
    prefix with whitespace, quotes, `%` or `$` is refused for `--unit`.
  - It writes `<prefix>/share/jax-seldon/install-manifest`: sha256 and
    path of each file, plus the symlink's target. `--uninstall` removes
    only manifest entries whose hash still matches. A file changed since
    the install is kept and reported. The symlink goes only if it still
    points to `seldon`. Uninstall is refused (exit 1, nothing removed)
    while `default.target.wants/seldon-watch.service` exists. It ends by
    listing what stays: logbook, `~/.config/seldon/`,
    `~/.local/state/seldon/`, the plugin. A second `--uninstall` says
    "Nothing to remove" and exits 0.
  - **Idempotent.** A file with equal content is never rewritten, so a
    re-run of the same version leaves bytes and mtimes unchanged (tested).
    A newer or older version replaces `seldon` and updates the manifest.
  - Refusals before the first write: a foreign `jax-seldon` (a regular
    file, or a symlink to something else) → exit 1. A non-x86_64 machine,
    a missing tool (`curl sha256sum tar gzip install cmp`), a bad tag, a
    download error → exit 2. Usage errors (relative `--prefix`, unknown
    flag, `--uninstall` combined with `--version`/`--unit`) → exit 1.
  - It never calls `sudo` or `systemctl`. Running as root only prints a
    warning. curl is limited to `--proto '=https,file' --proto-redir
    '=https'`. All code runs from `main "$@"` on the last line, so a
    truncated `curl | bash` does nothing.
  - It prints `seldon --version` of the installed binary, a PATH note
    when `<prefix>/bin` is not on PATH, and next steps: `seldon init`,
    `omarchy plugin add …`, the `systemctl --user enable` line (only
    with `--unit`), and how to update and remove.
- **`tests/install/install.test.sh`** (`just check-install`, added to
  `check`): 92 checks against a mock release tree served as `file://`
  URLs, with scratch HOME and prefixes, no network. The fake binary
  answers `--version`. It covers: latest with jq, without jq (pretty and
  compact JSON), re-run identity (a snapshot of bytes, mtimes and link
  targets), downgrade and update, the default prefix, `--unit` (three
  ExecStart forms, XDG_CONFIG_HOME, not enabled, manifest
  continuity), and refusals with nothing installed: tampered
  `SHA256SUMS`, missing line, wrong binary version, missing release, API
  unreachable, no `tag_name`, bad `--version`, relative prefix, unknown
  flag, foreign `jax-seldon`. It also covers the script piped to
  `bash -s --` and a truncated script, the three-step `sha256sum -c
  --ignore-missing` of `install.sh` itself, and uninstall (enabled-unit
  refusal, edited binary kept, twice, never installed). Recording `sudo`
  and `systemctl` stubs come first on PATH and are never called. The real
  `~/.local/bin/seldon`, `jax-seldon` and `~/.config/systemd/user` are
  fingerprinted before and after. Last step: shellcheck when installed,
  else `bash -n`.
- **`.github/workflows/release.yml`**: one line in the `build` job's
  "Release assets" step, `install -m755 install.sh dist/`, placed before
  `sha256sum`. `gh release create … dist/*` (unchanged) therefore uploads
  it as the fourth asset, and it is listed in `SHA256SUMS`, which the
  three-step form needs. The `release` job (WP-048's notes step) is not
  touched.
- **Banner (round 0; superseded by review round 1, item 2):** `plugin/Model.js` has a new constant `ENGINE_MISSING_DETAIL`:
  "The plugin needs the seldon command. AUR package: coming soon; until
  then install from GitHub (Install in the README at
  github.com/JohnAndrewsX/jax-seldon), then check again." The
  engineMissing banner uses it as `detail`. `INSTALL_ENGINE_COMMAND`,
  `UPDATE_ENGINE_COMMAND`, the actions and the argv are unchanged. A new
  `model.test.js` case pins this.
- **README.md**: a new **Install** section right after the parts table.
  Order: the AUR status sentence (one bold sentence, the only one to
  flip); "Engine from GitHub" with the three-step form first (`curl -O`
  install.sh + SHA256SUMS, `sha256sum -c --ignore-missing SHA256SUMS &&
  bash install.sh`); then the one-liner, noting that the script verifies
  the engine but not itself; an options table; next steps; update;
  remove; the v0.1.0 note; then "Engine from the AUR". The parts table
  row now reads "GitHub release (`install.sh`), AUR package
  `jax-seldon` (see Install)", which needs no flip.
- **plugin/README.md**: Requirements points to Install. Install is now
  1. the engine (the AUR status sentence once, then three-step, then
  one-liner, with a link to the project README for options and the AUR
  commands), 2. `seldon init`, 3. the plugin. The States row says the
  banner text points to the GitHub install. Security says the engine is
  installed separately (install.sh or AUR). Remove offers the
  GitHub-installed engine (`… | bash -s -- --uninstall`) before the AUR
  one.
- **docs/SPEC-PLUGIN.md §3**: one paragraph on the engineMissing banner:
  the AUR command stays the fix, the text names the GitHub install and
  the AUR status, and it is held in `ENGINE_MISSING_DETAIL`.
- **docs/TESTING.md**: a `check-install` row in the `just check` table.

## Not done

- **shellcheck was not run.** It is not installed on the dev host, and
  the docker socket is not accessible. I did not install anything (red
  zone). Both scripts were written for shellcheck: `SC2016` disables
  with reasons on the three single-quoted strings that carry `$1`/`$*`
  for other programs, no `ls` parsing, `--` before globs, and
  `local x; x=$(…)`. `bash -n` passes. **The first real shellcheck run
  will be the release workflow's `build` job** (its container installs
  shellcheck, and `just check` → `check-install` runs it). `ci.yml` does
  not install shellcheck, so CI runs `bash -n` there. See Decisions 1.
- `--unit` was not exercised on this host (as the brief says); it is
  tested only in the mock with a scratch HOME.
- `releases/latest/download/install.sh` 404s today: v0.1.0 does not
  carry the script. It works from the next release on. Both READMEs say
  so, the project README gives the `main` raw URL plus `--version v0.1.0`
  as the interim path, and the handover says it here.
- `docs/user/` (WP-045) was not touched.

## Verified by

```
$ just check                                          → exit 0
  … check-packaging: ok · install.test: 92 passed, 0 failed
  validate-fixtures: ok · plugin-validate: ok · qmllint: ok (28 files)
  model.test.js: 74 passed · service-states: 189 passed
  panel-view: 681 passed, 0 failed · overlay-view: 314 passed, 0 failed
  check: ok
$ bash tests/install/install.test.sh                  → 92 passed, 0 failed
  (re-run on the final script after the last two small edits)
```

Real run on the dev host against the published v0.1.0 (network for
this test only), on the final `install.sh`, with the real HOME and
without `--unit`:

```
$ bash install.sh --version v0.1.0 --prefix /tmp/seldon-wp044-real   → exit 0
  verified   seldon-0.1.0-x86_64-unknown-linux-musl.tar.gz (sha256 db54bf27…a4ef6,
             equal to the release's SHA256SUMS line)
  installed  …/bin/seldon · …/bin/jax-seldon -> seldon · …/share/jax-seldon/install-manifest
  seldon 0.1.0 is installed in /tmp/seldon-wp044-real/bin.
$ /tmp/seldon-wp044-real/bin/jax-seldon --version --json  → {"name":"seldon","version":"0.1.0"}
$ (same command again)                                    → 3× "unchanged"
$ bash install.sh --prefix /tmp/seldon-wp044-real         → latest via the API = v0.1.0, unchanged
$ bash install.sh --uninstall --prefix /tmp/seldon-wp044-real → exit 0, 3 removed, 0 files left
  (rc=141 once, when I piped it into `head -3`: SIGPIPE from head, not the script)
$ fingerprint of ~/.local/bin/seldon (+ sha256), ~/.local/bin/jax-seldon,
  ~/.config/systemd/user, ~/Seldon before vs after           → identical
```

Tampered checksum with the real assets: I downloaded the v0.1.0 tarball
and `SHA256SUMS` into the scratchpad, changed `db54…` to `db55…`, and
served them with `SELDON_INSTALL_DOWNLOAD_URL=file://…` →
`seldon-…tar.gz: FAILED`, "checksum mismatch …; nothing installed",
exit 2, prefix not created. With a correct `SHA256SUMS` and one byte
appended to the tarball → same result, exit 2, prefix not created.

## Learned

Appended to `memory/pitfalls.md` (WP-044): shellcheck exists only in
the release container, so the dry run is the shellcheck gate.
`latest/download` 404s for assets the latest release lacks. curl's
`file://` makes a portless mock release, and `--proto`/`--proto-redir`
keep it while blocking http. A "no jq" PATH symlink farm. `main "$@"`
for `curl | bash` scripts. How to run the real-HOME acceptance safely.

## Decisions needed

Round 1: 1 is settled (the reviewer adds shellcheck to `ci.yml` at
merge), 2 is settled by ADR-0024 (the one-liner is the fix), and 3 is
updated here. Nothing new is open.

1. **shellcheck in CI.** `ci.yml` installs `git rust just jq`. Adding
   `shellcheck` there would make `check-install` and `check-packaging`
   lint on every PR, not only in the release build. That is a one-word
   change in a workflow WP-048 is editing, so I left it to the
   orchestrator. Until then, run the release dry run (`gh workflow run
   release.yml --ref <branch>`) on this branch before merging. It is the
   first run where shellcheck sees `install.sh`.
2. **The banner's "Install in terminal" still runs the AUR command,**
   which fails while the AUR package does not exist. The brief fixed the
   argv ("constant text only, fixed argv unchanged"), so the text now
   says to install from GitHub. If the operator wants the button to work
   now, the option is to swap `INSTALL_ENGINE_COMMAND` to the one-liner
   (`curl -fsSL …/install.sh | bash`) for the AUR pause. That is a
   change to the plugin's command surface: CONTRACT.md "Commands the
   plugin may run" and the README Security list would change with it.
3. **The flip when the AUR goes live** (updated in round 1):
   `INSTALL_ENGINE_COMMAND` → `omarchy pkg aur add jax-seldon` and
   `ENGINE_MISSING_DETAIL` in `plugin/Model.js`, together with their pins
   in `tests/plugin/model.test.js` and `tests/plugin/service-states.sh`
   (`install_engine=`); SPEC-PLUGIN §3 paragraph and §5 line; the
   plugin/README.md States row, Security constants list and No-network
   sentence; the bold
   sentence at the top of README.md "Install" (then move "Engine from
   the AUR" above "Engine from GitHub"); the first sentence of
   plugin/README.md "Install" step 1.

## Touched outside WP scope

- `tests/plugin/model.test.js`: one new test for the banner text
  constant (the test file for the Model.js change in scope).
- `packaging/README.md`: the `release` row of the jobs table said "the
  three assets"; it now says four and names `install.sh`. A one-line
  correction, otherwise wrong after this WP.
- `docs/SPEC-PLUGIN.md`: the paragraph sits in §3 as briefed. §5's
  banner list ("Install the engine: `omarchy pkg aur add jax-seldon`")
  is unchanged and still true for the command.
