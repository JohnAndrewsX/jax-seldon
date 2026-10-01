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
| HEAD | this handover |

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

- `just check` → `check: ok`, exit 0, 715 tests passed, 0 failed. It
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
7. Run the dry run (`gh workflow run release.yml --ref main`); the summary
   must say `true` for both secrets. Then tag `v0.1.0` when ready.

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

1. **Guard block: makepkg on the test host.**
   - These commands were blocked as "privileged or package command":
     - `ssh <test-host> 'cd /tmp/seldon-wp040 && makepkg --printsrcinfo > srcinfo.test-host && cmp srcinfo.test-host .SRCINFO && echo IDENTICAL'`
     - `ssh <test-host> 'cd /tmp/seldon-wp040 && makepkg -f'`
     - Earlier, a probe containing `…; makepkg --version`. I re-ran it
       with `command -v` only.
   - I did not reword them or run them from a script. The rule matches
     `makepkg` after `;`/`&&` even inside the ssh string.
   - Proposal for `scripts/guard.sh` (the orchestrator's call, with a
     `guard-test.sh` row), like the `omarchy theme set` exception: allow
     the whole command when it is
     `^\s*ssh\s+\S+\s+'cd /tmp/[A-Za-z0-9._-]+ && makepkg (-f|--printsrcinfo)( [^';&|]*)?'$`,
     and keep it blocked whenever `-s`, `-i`, `--syncdeps`, `--install`
     or `-r` appears.
   - Then either a worker reruns the README's test-host block
     (≈ 5 min), or the CI dry run stands in for it.
2. **ADR-0022**: accept as written, or replace it with a note elsewhere.
   Renumber it if another WP took 0022.
3. Optional follow-ups for other owners, not done here because they are
   outside packaging/:
   - engine/systemd/README.md step 1 still says "Until the package ships
     it". It could now say "Installed by `jax-seldon` at
     `/usr/lib/systemd/user/`; just `systemctl --user enable --now
     seldon-watch`". The unit's header comment says "Installed only by
     the user".
   - ci.yml could run a `makepkg --printsrcinfo` diff on every PR.
     `release.yml` does that only on tags and dry runs.

## Touched outside WP scope

- `justfile`: the `check-packaging` recipe and its place in `check`.
- `DECISIONS.md`: one row (ADR-0022, proposed).
- `decisions/ADR-0022-aur-package-glibc-build.md` (new; the WP asked for an
  ADR note).
- engine/, plugin/, schema/, fixtures/ and ci.yml are untouched.
