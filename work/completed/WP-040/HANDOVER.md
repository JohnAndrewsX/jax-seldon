WP-040 HANDOVER

Branch `wp/040-packaging`, worktree `wt/WP-040`, based on `main` at
`1614f2e`. Not pushed, no PR.

| Commit | What |
|---|---|
| `3a76c70` | `packaging/PKGBUILD`, `.SRCINFO`, `set-version.sh`, `check-srcinfo.sh`; `just check-packaging` (part of `check`) |
| `9b1aa26` | `packaging/README.md`, `packaging/expected-files.txt` |
| `85b0629` | `.github/workflows/release.yml` |
| `4041447` | `decisions/ADR-0022` (**proposed**), DECISIONS.md row |
| `ce3e281` | memory/host.md, memory/pitfalls.md |
| `98ed95f` | first handover |
| `b364092` | review, blocking: `check()` keeps `CARGO_HOME`/`RUSTUP_HOME`; `options=('!debug')`; maintainer placeholder |
| `55aafb4` | review, non-blocking: release.yml, check-srcinfo.sh, README, ADR-0022 |
| HEAD | this handover, updated after the review |

## Review follow-ups (round 2)

**Blocking, fixed (`b364092`).** `check()` moved `HOME` to the scratch
dir without pinning cargo's and rustup's dirs:
- `cargo test --frozen` then looked for the registry that `prepare()`
  had fetched into the builder's real `~/.cargo`.
- It looked in the empty scratch home instead, so the build failed.
- I reproduced it on the dev host with plain cargo:
  `RUSTUP_TOOLCHAIN=stable HOME=<scratch> cargo metadata --frozen` →
  exit 101, `no matching package named 'anyhow'`.
- It was worse than the review said: **rustup first auto-installed a
  1.5 GB stable toolchain** into the scratch home ("the missing active
  toolchain … has been auto-installed"). I deleted that scratch dir.
- Fix: `check()` now exports `CARGO_HOME="${CARGO_HOME:-$HOME/.cargo}"`
  and `RUSTUP_HOME="${RUSTUP_HOME:-$HOME/.rustup}"` before `HOME`
  moves. It already set `RUSTUP_TOOLCHAIN=stable` and
  `CARGO_TARGET_DIR=target`, as `build()` does.
- Stand-in re-verified (plain cargo, no makepkg) on a fresh `git archive`
  tarball:
  - `cargo fetch --locked`, then `cargo build --frozen --release
    --features watch` succeeds;
  - then the exact `check()` body, with its exports and the scratch
    `HOME`/`XDG_*`/`SELDON_TEST_GUARD`:
    `cargo test --frozen --features watch --lib --bins` → 111 + 2
    passed, exit 0;
  - the scratch home is still empty afterwards (0 bytes).

**Non-blocking, done:**
1. `options=('!debug')`, and the matching `options = !debug` line in
   `.SRCINFO` (srcinfo.sh order: after `optdepends`, before `source`;
   `check-srcinfo.sh` agrees).
2. release.yml:
   - `cargo metadata --locked`;
   - `shellcheck` added to the container's pacman list, so `just
     check-packaging` runs it there;
   - a comment that the `bump` commit, pushed with `GITHUB_TOKEN`, does
     not trigger ci.yml.

   check-srcinfo.sh gains a file-wide `# shellcheck disable=SC1090,SC2154`,
   for the `source`d PKGBUILD and its variables.
3. packaging/README.md, test-host section: only
   `ssh <host> 'cd /tmp/<dir> && makepkg -f'` and
   `ssh <host> 'cd /tmp/<dir> && makepkg --printsrcinfo > SRCINFO.new'`.
   Then `scp` the file back and `cmp` it locally. The `| cmp - .SRCINFO`
   pipe is gone, and every other step is its own plain ssh command.
4. `# Maintainer: JohnAndrewsX <EMAIL>` (a visible placeholder). It is
   in the operator setup below and in README § One-time setup, step 7.
5. ADR-0022: status stays proposed, date 2026-10-02, and the §7 sentence
   is replaced word for word as given.
6. README § Cutting a release, step 2: the dry run must be green before
   the tag, or the AUR copy ships a PKGBUILD whose `check()` fails for
   every user.

**Optional, not done:**
- `rustc -vV | sed …` instead of `--print host-tuple`. `host-tuple`
  needs rustc ≥ 1.84; the crate's `rust-version` is 1.89 and the current
  Arch Rust guidelines use it.
- SHA-pinned actions: left as major tags, consistent with ci.yml.

**Not verifiable here: shellcheck has never run on these files.** It is
not on the dev host and I installed nothing. Its first run is the CI
dry run (`just check` in `build`). A finding there fails the dry run,
not ci.yml, because ci.yml does not install shellcheck. I reasoned
through the likely codes:
- PKGBUILD: SC2034, SC2154 and SC2164 are excluded by the recipe.
- check-srcinfo.sh: SC1090 and SC2154 are disabled.
- set-version.sh: none expected.

## Done

- **`packaging/PKGBUILD`** for `jax-seldon` 0.1.0:
  - `source`: the release asset
    `…/releases/download/v$pkgver/jax-seldon-$pkgver.tar.gz` (a `git archive`
    of the tag that the workflow builds and uploads, so its checksum is
    known when the workflow writes it). `sha256sums=('SKIP')` until the
    first release; the workflow writes the real sum.
  - `depends=(gcc-libs git glibc)` (the binary links `libgcc_s` and `libc`
    only; the engine runs `git`), `makedepends=(cargo)`,
    `optdepends=(snapper: …)`.
  - `prepare()`: `cargo fetch --locked --target "$(rustc --print host-tuple)"`.
    `build()`: `cargo build --frozen --release --features watch`
    (`--frozen` = `--locked` + offline, per the Arch Rust guidelines;
    `RUSTUP_TOOLCHAIN=stable`, `CARGO_TARGET_DIR=target`).
    `check()`: `cargo test --frozen --features watch --lib --bins` (the
    113 unit tests) in a scratch `HOME`/`XDG_*` under `$srcdir` with
    `SELDON_TEST_GUARD`, so the builder's own Seldon state is never touched.
  - `package()`: `usr/bin/seldon`, symlink `usr/bin/jax-seldon → seldon`,
    `usr/lib/systemd/user/seldon-watch.service` (installed, not enabled),
    `usr/share/licenses/jax-seldon/LICENSE`, `usr/share/doc/jax-seldon/README.md`.
    Nothing under `/etc`, no hooks, no `.install` script.
  - The unit: `engine/systemd/seldon-watch.service` runs
    `%h/.local/bin/seldon`. `package()` rewrites that path to
    `/usr/bin/seldon` with `sed` and fails unless
    `ExecStart=/usr/bin/seldon watch` is then present. engine/ is untouched.
  - **glibc, not musl** (one line, in the PKGBUILD and ADR-0022): the AUR
    convention, and the musl target would need `rust-musl`, which pulls in
    `rust` and conflicts with rustup (Omarchy's `dev-env rust`). The
    static musl binary is the release asset.
- **`packaging/.SRCINFO`** for that PKGBUILD. `packaging/check-srcinfo.sh`
  checks it against the PKGBUILD in pure bash (srcinfo.sh field order),
  never with makepkg, so `just check` never runs makepkg on any host.
  `just check-packaging`: `bash -n` of the PKGBUILD and both scripts,
  shellcheck when installed (not on the dev host: notice), and
  check-srcinfo.
- **`packaging/set-version.sh VERSION SHA256`**: sets `pkgver`, `pkgrel=1`
  and the checksum; checks its arguments and that each line matched once.
- **`packaging/expected-files.txt`**: the exact `tar tf` list without dot
  files (15 entries).
- **`.github/workflows/release.yml`**:
  - `build` (tag **and** dry run, `archlinux:base-devel` with
    `rust rust-musl just jq`):
    - the tag must equal `v` + the engine version;
    - `just check`;
    - static musl build `--features watch`, checked: static-linked,
      `--version --json`, `watch --help`;
    - assets: source tarball, musl tarball (binary, LICENSE, README, unit,
      unit README), `SHA256SUMS`;
    - `set-version.sh`, then `makepkg --printsrcinfo` as an unprivileged
      user, then `check-srcinfo.sh`;
    - a real `makepkg -f` from the local source tarball, then
      `tar tf` against `expected-files.txt`, then the packaged
      `seldon`/`jax-seldon` and the unit's `ExecStart` checked;
    - `git subtree split --prefix=plugin` with a check of the split's
      `manifest.json` id;
    - a step summary (checksums, `packaging/` diff, whether each secret
      is set as true/false) and the artifacts `dist`, `packaging`,
      `package`.
  - `release` (tag): `gh release create --verify-tag` with the assets.
  - `bump` (tag): commits the new PKGBUILD/.SRCINFO to `main`. If
    `main`'s `packaging/` changed after the tag, it is skipped with a
    warning and nothing is overwritten. A failure here (protected `main`)
    does not stop `aur` or `plugin`.
  - `aur` (tag, after `release`): skipped with a notice without
    `AUR_SSH_PRIVATE_KEY`. Pins the AUR Ed25519 host key fingerprint,
    which I checked against a live `ssh-keyscan`. Clones, commits, pushes
    `master`, and does nothing if there are no changes.
  - `plugin` (tag, after `release`): skipped with a notice without
    `PLUGIN_REPO_TOKEN`. Checks out without persisted credentials, then
    pushes the split as `main` and as the tag. The token goes in through
    an `http.extraheader`, never in the URL.
  - `workflow_dispatch` = dry run: only `build` runs; nothing is pushed,
    released or committed.
  - `defaults.run.shell: bash`, so every step has `pipefail`.
- **`packaging/README.md`**:
  - what the package contains;
  - how a release is cut, with a table of the jobs;
  - the dry run;
  - bumping by hand;
  - the operator's one-time setup (below);
  - testing the PKGBUILD on a disposable host, with the exact commands.
- **ADR-0022 (proposed)**: AUR package against glibc, musl as the release
  asset. The WP asked for "a one-line ADR note"; the orchestrator
  accepts it or folds it elsewhere. **Number collision risk**: WP-036/041
  run in parallel; renumber at merge if one of them also took 0022.

## Not done

- **The PKGBUILD has not been built on the test host.** The guard
  blocked it; see *Decisions needed*. So these acceptance items are
  **open**:
  - `.SRCINFO` byte-identical to `makepkg --printsrcinfo`;
  - `makepkg -f` succeeds;
  - `tar tf` lists exactly the expected files;
  - `seldon --version` and `seldon watch --help` from the extracted
    package.

  What I checked instead (none of it with makepkg):
  - `check-srcinfo.sh` passes. A mutation (dropping `git` from
    `depends`) is caught.
  - A plain `cargo fetch --locked` + `cargo build --frozen --release
    --features watch` on the dev host, from the `git archive` tarball
    named like the release asset, succeeds:
    - `seldon --version` → `seldon 0.1.0`;
    - `--version --json` → `{"name":"seldon","version":"0.1.0"}`;
    - `watch --help` works;
    - `ldd` → `libgcc_s`, `libc`.
  - The `sed` of the unit changes exactly the `ExecStart` line and one
    comment line. `systemd-analyze --user verify` of a scratch copy
    (`ExecStart=/usr/bin/true`) prints nothing, exit 0.

  The `build` job of `release.yml` runs the same makepkg checks in CI,
  so the dry run would also close these items, except the test-host one.
- The workflow has never run (it needs network, a GitHub runner and the
  operator's go; there is no `act`). The dry run is the operator's step
  (packaging/README.md § Dry run).
- No `yamllint` or `actionlint` on the dev host. I checked instead:
  - PyYAML parses the file;
  - every step has exactly one of `run`/`uses`;
  - `bash -n` passes on every `run:` block (with `${{ }}` stubbed);
  - no tabs, no trailing spaces, no lines over 120;
  - a careful manual review.
- `rust-musl` as the musl target in the `archlinux` container is from my
  knowledge of the Arch repos; it is not verified. If the package name is
  wrong, the dry run fails at "Install toolchain". The fallback is
  `rustup` + `rustup target add x86_64-unknown-linux-musl` there.
- `engine/Cargo.toml` needed no metadata (`repository`, `license`,
  `description` are there). The version stays `0.1.0`. No tag was made.

## Verified by

- After the review round: `bash -n` on the PKGBUILD and both scripts,
  and on every `run:` block of release.yml, is ok. PyYAML parses it. No
  tabs or trailing spaces. `just check` → `check: ok`, exit 0, 715
  passed, 0 failed, `check-packaging: ok`.
- First round: `just check` → `check: ok`, exit 0, 715 tests passed, 0 failed. It
  includes `check-packaging: ok` ("shellcheck not installed; bash -n
  only", `check-srcinfo: ok`), `plugin-validate: ok` and
  `qmllint: ok (28 files)`.
- `bash packaging/check-srcinfo.sh` passes. On a copy of the PKGBUILD
  with `git` removed from `depends`, it prints a diff and exits 1.
- `set-version.sh 0.2.0 <sha>` on a copy writes the three lines.
  `set-version.sh 1.0 abc` exits 1.
- Engine unit tests in a scratch home: `cargo test --locked --features
  watch --lib --bins` → 111 + 2 passed; nothing outside the scratch dir.
- The release build from the source tarball: see *Not done*.
- `ssh-keyscan -t ed25519 aur.archlinux.org | ssh-keygen -lf -` →
  `SHA256:RFzBCUItH9LZS0cKB5UE6ceAYhBD5C8GeOBip8Z11+4`, the value pinned
  in the workflow.
- The test-host scratch dir `/tmp/seldon-wp040` (PKGBUILD, .SRCINFO,
  tarball) was created, then removed. Nothing ran there except
  `command -v`/version probes, `mkdir`, `scp` and `rm`. The shell and the
  lock were not touched, and nothing was installed anywhere.
- `git diff main..HEAD` has no hostnames, IPs or private paths.

## Test host: what it has and lacks

- **Has:**
  - `cargo` and `rustc` 1.98.1 (pacman `rust`);
  - `makepkg`, `git`, `bsdtar`, `curl`;
  - `/tmp` tmpfs 3.9 GB, 6 cores, 7 GB RAM.
- **For the glibc PKGBUILD build:** nothing is missing. `cargo`
  satisfies `makedepends`; `git`, `gcc-libs`, `glibc` and `base-devel`
  are there.
- **Lacks** (none of these is needed for this WP's build):
  - rustup, and the musl target (`rust-musl`): needed only to build the
    static release binary locally; CI builds it;
  - `just`, `shellcheck`, `yamllint`, `namcap`: `namcap` would be nice
    for a lint of the built package.

## Operator's one-time setup (packaging/README.md § One-time setup)

1. Make `JohnAndrewsX/jax-seldon` **public** before the first tag; release
   assets of a private repo cannot be downloaded by `makepkg`.
2. AUR account; a dedicated key (`ssh-keygen -t ed25519 -f aur-release`)
   added under *My Account → SSH Public Key* (account-wide, the AUR has no
   per-package deploy keys).
3. Secret `AUR_SSH_PRIVATE_KEY` = that private key; then shred the local copy.
4. Create `JohnAndrewsX/jax-seldon-plugin`, public and **empty** (no
   README/licence, so the first split push is a fast-forward).
5. Secret `PLUGIN_REPO_TOKEN` = a fine-grained PAT, only
   `jax-seldon-plugin`, *Contents: Read and write*.
6. If `main` is protected, let GitHub Actions push to it (`bump`) or bump
   by hand after each release.
7. Replace `EMAIL` in `# Maintainer: JohnAndrewsX <EMAIL>`
   (packaging/PKGBUILD, line 1) with the address to show on the AUR, and
   commit before the first tag. No agent fills it in.
8. Run the dry run (`gh workflow run release.yml --ref main`); it must be
   green and the summary must say `true` for both secrets. Then tag
   `v0.1.0` when ready.

## Learned (in memory/)

- host.md: the test host's toolchain (above), its makepkg.conf; the dev
  host has no shellcheck/yamllint/actionlint.
- pitfalls.md:
  - the guard blocks makepkg inside ssh strings;
  - `rust-musl` conflicts with rustup;
  - the engine has a lib and a bin target (`--lib --bins`);
  - rustup needs `RUSTUP_HOME` when `HOME` is redirected;
  - GitHub Actions: pipefail, persisted checkout credentials,
    `upload-artifact` and dot files, makepkg refuses root;
  - makepkg takes a local file for a URL source.

## Decisions needed

Settled by the review:
- ADR-0022 is accepted at merge, with the review's wording.
- The guard exception for makepkg over ssh is the operator's (the guard
  is operator-owned). Until it exists, the CI dry run stands in for the
  test-host build. README § Testing uses exactly the two forms the
  exception is to allow.
- The `bump` job stays.
- The first tag is the operator's.

So the test-host acceptance items stay open until either the guard
exception exists or the dry run is green. The test host was not touched
in this round.

Still open, outside packaging/ (owners other than this WP):
- engine/systemd/README.md step 1 still says "Until the package ships
  it". The unit's header says "Installed only by the user".
- ci.yml could install shellcheck and diff `makepkg --printsrcinfo` on
  PRs. Today only release.yml does.

The first round's request (for the record): these commands were
blocked by the guard:
- `ssh <test-host> 'cd /tmp/seldon-wp040 && makepkg --printsrcinfo > … && cmp …'`
- `ssh <test-host> 'cd /tmp/seldon-wp040 && makepkg -f'`
- a probe with `…; makepkg --version`.

## Touched outside WP scope

- `justfile`: the `check-packaging` recipe and its place in `check`.
- `DECISIONS.md`: one row (ADR-0022, proposed).
- `decisions/ADR-0022-aur-package-glibc-build.md` (new; the WP asked for an
  ADR note).
- engine/, plugin/, schema/, fixtures/ and ci.yml are untouched.
