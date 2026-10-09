# Packaging and releases

The engine ships as the AUR package **`jax-seldon`** (ADR-0001, ADR-0016);
users install it with `omarchy pkg aur add jax-seldon` and upgrade with
`yay -S jax-seldon`. The plugin ships from the separate repository
**`jax-seldon-plugin`**, generated from `plugin/` (ADR-0009); users
install it with `omarchy plugin add
https://github.com/JohnAndrewsX/jax-seldon-plugin.git`. A tag `vX.Y.Z`
publishes both through `.github/workflows/release.yml`.

| File | What |
|---|---|
| `PKGBUILD` | the AUR package; source of truth for the AUR copy |
| `.SRCINFO` | `makepkg --printsrcinfo` of the PKGBUILD; the AUR needs it in every commit |
| `set-version.sh VERSION SHA256` | sets `pkgver`, `pkgrel=1` and the source checksum (the workflow runs it) |
| `check-srcinfo.sh` | checks `.SRCINFO` against the PKGBUILD without makepkg (`just check-packaging`) |
| `release-notes.sh X.Y.Z [CHANGELOG]` | prints the version's `CHANGELOG.md` section, the release body; exit 1 without it (`tests/release/`) |
| `expected-files.txt` | the exact file list of the built package (`tar tf`, dot files left out) |
| `audit-ignore.txt` | RustSec advisories accepted for `engine/Cargo.lock`, each with an expiry and a reason (CONTRIBUTING.md, "Dependency advisories") |
| `audit-ignore.sh [FILE [TODAY]]` | checks that list and prints its ids for `cargo audit --ignore`; exit 1 on an expired or malformed entry (`tests/release/`) |
| `omarchy-pin` | the `omacom/omarchy` commit and the sha256 of its `bin/omarchy-plugin-validate` (WP-190; "The Omarchy pin" below) |
| `omarchy-validate.sh PLUGIN_DIR` | fetches that validator over HTTPS, refuses it unless the sha256 matches, runs it on `PLUGIN_DIR`: `just plugin-validate` without the omarchy CLI (CI), the release workflow on the plugin split (`tests/release/`) |

## What the package contains

- `/usr/bin/seldon`, built with `cargo build --frozen --release --features
  watch` (the `watch` feature is in, WP-034), and the symlink
  `/usr/bin/jax-seldon` → `seldon` (ADR-0001).
- `/usr/lib/systemd/user/seldon-watch.service`: the optional watcher unit
  from `engine/systemd/`, with `ExecStart=/usr/bin/seldon watch` instead
  of the self-built `~/.local/bin/seldon`. It is **installed, not
  enabled** (ADR-0005). A user who wants it runs
  `systemctl --user enable --now seldon-watch`; see
  `engine/systemd/README.md` for what it does (skip its build and install
  steps — the package has done them).
- `/usr/share/licenses/jax-seldon/LICENSE`, `/usr/share/doc/jax-seldon/README.md`.

Nothing under `/etc`, no pacman hooks, no `.install` script, nothing in
any home directory. `seldon init` (run by the user) creates the logbook
and config; the package never does.

`check()` runs the engine's unit tests (`--lib --bins`, with the feature)
in a scratch `HOME` under `$srcdir`; `CARGO_HOME` and `RUSTUP_HOME` keep
pointing at the builder's own (the registry `prepare()` fetched, the
installed toolchain). The integration tests stay in CI. Cargo strips the
binary, so `options=('!debug')`.

**glibc, not musl** (ADR-0022): the package builds for the host target
with `makedepends=(cargo)`, which `rust` and `rustup` both satisfy. The
static musl binary required by AGENTS.md §7 is the release asset.

## Cutting a release

The first tag, `v0.1.0`, is the operator's call.

1. On `main`: set `version` in `engine/Cargo.toml` (and let `cargo`
   update `engine/Cargo.lock`), move the `[Unreleased]` notes in
   `CHANGELOG.md` under the new version, commit, push. `just check` is
   green.
2. Dry run (below) on `main`. Read its summary. **It must be green
   before the tag**: otherwise the AUR copy ships a PKGBUILD whose
   `check()` (or build) fails for every user.
3. Tag and push:

   ```
   git tag -a v0.1.0 -m "Seldon 0.1.0"
   git push origin v0.1.0
   ```

The workflow then runs, in order:

| Job | Runs on | Does |
|---|---|---|
| `build` | tag and dry run | fails unless the tag equals `v` + the `engine/Cargo.toml` version; fails without a `## [X.Y.Z]` section in `CHANGELOG.md` (`release-notes.sh`, docs/VERSIONING.md); **`cargo audit` of `engine/Cargo.lock`, the release gate**: an advisory, an unmaintained or a yanked crate fails the build unless `audit-ignore.txt` accepts its id (an expired entry fails it too); `just check`; static musl binary with `--features watch` (checked: static, `--version --json`, `watch --help`); assets `jax-seldon-X.Y.Z.tar.gz` (`git archive` of the tag — the PKGBUILD's source), `seldon-X.Y.Z-x86_64-unknown-linux-musl.tar.gz` (binary, LICENSE, README, unit, unit README) `install.sh` and `SHA256SUMS`; `set-version.sh` + `makepkg --printsrcinfo`; a real `makepkg -f` of the PKGBUILD from that tarball as an unprivileged user, its file list against `expected-files.txt`, the packaged binary run; `git subtree split --prefix=plugin` and a check of the split's `manifest.json` (its id; its `version` and `Model.js` `PLUGIN_VERSION` equal to the tag's, `plugin-version.sh`); **the split's own tree (`git archive` of the split, extracted) against Omarchy's pinned validator** (`omarchy-validate.sh`, WP-190), so the plugin the store installs is validated before anything is published; **a build-provenance attestation** (`actions/attest-build-provenance`) of the binary tarball, the source tarball, `SHA256SUMS` and `install.sh` — the job alone has `id-token: write` and `attestations: write`; `install.sh` checks it with `gh attestation verify` (SECURITY.md, "Verifying a release") |
| `release` | tag | GitHub release `vX.Y.Z` with the four assets (the three above and `install.sh`, also listed in `SHA256SUMS`; README.md "Install"); the release notes are that `CHANGELOG.md` section (`packaging/release-notes.sh`) |
| `bump` | tag | commits the updated `PKGBUILD` and `.SRCINFO` to `main` (`packaging: jax-seldon X.Y.Z`). Skipped with a warning if `main`'s `packaging/` changed after the tag; then bump by hand (below) |
| `aur` | tag | clones `ssh://aur@aur.archlinux.org/jax-seldon.git`, commits `PKGBUILD` + `.SRCINFO` (`Update to X.Y.Z`), pushes `master`. The host key is pinned (Ed25519 `SHA256:RFzBCUItH9LZS0cKB5UE6ceAYhBD5C8GeOBip8Z11+4`, as published on aur.archlinux.org). **Skipped with a notice** without `AUR_SSH_PRIVATE_KEY` |
| `plugin` | tag | `git subtree split --prefix=plugin`; refuses unless it is the split `build` validated (its `plugin_split` output); pushes it to `jax-seldon-plugin` as `main` and as the tag. **Skipped with a notice** without `PLUGIN_REPO_TOKEN` |

`aur` and `plugin` wait for `release`, so the AUR source URL exists before
the AUR knows the version. `bump` failing (for example a protected `main`)
does not stop them.

After the run: the AUR page shows the version; on a test host
`omarchy pkg aur add jax-seldon` (fresh) or `yay -S jax-seldon` (upgrade)
installs it, `seldon --version` prints it; `omarchy plugin update
jax.seldon` pulls the plugin. Installing is the operator's step.

### Dry run

Actions → **release** → *Run workflow* → pick the branch, or:

```
gh workflow run release.yml --ref main
gh run watch
```

Only `build` runs. The version comes from `engine/Cargo.toml`, the source
tarball is `git archive` of the branch head (no tag needed). Nothing is
pushed, released or committed. The attestation step runs too, so it is
exercised before a tag depends on it: the dry run's assets get real,
public attestations, but with the branch as their source ref
(`refs/heads/<branch>`), which `install.sh` (`--source-ref
refs/tags/vX.Y.Z`) never accepts. Check one with `gh run download <id>
-n dist -D dist` and `gh attestation verify dist/SHA256SUMS --repo
JohnAndrewsX/jax-seldon --source-ref refs/heads/<branch>`. The run
summary shows the checksums, the attestation link, the `packaging/` diff
a tag would commit, and whether the two secrets are set (true/false,
never the value). The artifacts `dist` (release assets), `packaging`
(PKGBUILD, .SRCINFO) and `package` (the built `.pkg.tar.zst`) can be
downloaded and inspected (`gh run download`).

### Bumping by hand

When `bump` was skipped, or for a packaging-only change (`pkgrel`):

1. Edit `packaging/PKGBUILD` (for a new version: `bash
   packaging/set-version.sh X.Y.Z <sha256 of jax-seldon-X.Y.Z.tar.gz from
   SHA256SUMS>`; for a packaging fix: raise `pkgrel`).
2. Regenerate `.SRCINFO` with `makepkg --printsrcinfo > .SRCINFO` on a
   host where makepkg may run (the dev host's guard blocks it; use the
   test host, as below). `just check-packaging` fails while the two
   disagree.
3. Commit to `main`. For a `pkgrel` change, push `PKGBUILD` and `.SRCINFO`
   to the AUR repository by hand; the workflow publishes on tags only.

## Pinned actions and image

Every `uses:` in `.github/workflows/` names a full commit SHA, with the
release it belongs to as a comment, and every job container names an
image digest, with the dated tag as a comment:

```
- uses: actions/checkout@11d5960a326750d5838078e36cf38b85af677262 # v4.4.0
container: archlinux:base-devel@sha256:51dd…cc3 # base-devel-20260927.0.600689
```

A run therefore uses exactly the reviewed action code and image until a
commit changes the pin. `tests/release/workflow-pins.test.sh` (part of
`just check-packaging`) fails on a tag or branch reference, a short SHA,
a missing comment, an image without a digest, or one action pinned to
two commits; it also checks the release gate in `release.yml` (see the
`build` row above). It cannot tell whether a SHA belongs to the version
in its comment (that needs the network); the refresh steps below
resolve both together. The toolchain inside the image still comes from
the Arch repositories at run time.

**Refreshing the pins** is a manual step, done in one commit for all
three workflows (each action and the image have one pin everywhere):

- *When:* about once a month, when an action publishes a release, or
  when the toolchain install step fails because the image is too old
  (for example, its keyring no longer knows a packager's key).
- *An action:* resolve the release tag to its commit. An annotated tag
  points at a tag object first; resolve that once more:

  ```
  gh api repos/actions/checkout/git/ref/tags/v4.4.0 --jq '.object.type + " " + .object.sha'
  # "tag <sha>" → gh api repos/actions/checkout/git/tags/<sha> --jq .object.sha
  ```

  Read the action's release notes between the old and the new version
  before you change the pin.
- *The image:* the digest of `base-devel` and the dated tag that has the
  same digest:

  ```
  curl -fsS 'https://hub.docker.com/v2/repositories/library/archlinux/tags?page_size=5&name=base-devel-' \
    | jq -r '.results[] | "\(.name) \(.digest)"'
  ```

  Take the newest line; the digest goes after `@`, the name into the
  comment.
- *Then:* `just check-packaging`, and the release dry run on the branch
  ("Dry run" above) must be green before the change is merged.

## The Omarchy pin

`omarchy-pin` names one `omacom/omarchy` commit and the sha256 of
`bin/omarchy-plugin-validate` at that commit, with the Omarchy version as
a comment. Omarchy's validator is bash and jq; `omarchy-validate.sh`
fetches that one file from `raw.githubusercontent.com` (HTTPS only,
redirects too), refuses it on a sha256 mismatch and never runs it then,
and runs it on a plugin folder. It runs where the omarchy CLI is absent:
`just plugin-validate` in CI, and the release workflow on the extracted
plugin split. On the dev host `just plugin-validate` keeps the installed
`omarchy plugin validate` and prints a notice when the installed
validator is not the pinned one. The pin mirrors the validator installed
on the test host; it never replaces the installed tree as the reference
(AGENTS.md §1).

**Refreshing the pin**, like the action pins, is a manual step in one
commit:

- *When:* after the test host moves to a new Omarchy release, or when
  `just plugin-validate` on the dev host prints the notice.
- *The commit:* the release tag of the version `omarchy version` prints
  on the test host (lightweight tags point at the commit; an annotated
  one at a tag object, resolve it once more as for the actions):

  ```
  gh api repos/omacom/omarchy/git/ref/tags/v4.0.4 --jq '.object.type + " " + .object.sha'
  ```

- *The sha256:* of the file at that commit, which must equal the test
  host's installed validator:

  ```
  curl -fsSL --proto '=https' https://raw.githubusercontent.com/omacom/omarchy/<commit>/bin/omarchy-plugin-validate | sha256sum
  ssh <test-host> 'omarchy version; sha256sum /usr/share/omarchy/bin/omarchy-plugin-validate'
  ```

  If they differ, the test host runs a build between tags: pick the
  commit whose file matches, and say so in the comment. Read the
  validator's diff between the old and the new commit before you change
  the pin.
- *Then:* `just check-packaging` (`tests/release/omarchy-pin.test.sh`
  runs the installed validator through the pin where they match), and
  CI on the pull request runs the fetched one.

## One-time setup (operator)

1. **Make `JohnAndrewsX/jax-seldon` public** before the first tag. The
   AUR package downloads the source tarball from the GitHub release;
   release assets of a private repository are not downloadable without a
   token, so `makepkg` would fail for every user.
2. **AUR account and key.** Register at https://aur.archlinux.org. Make a
   key used only for the release workflow:

   ```
   ssh-keygen -t ed25519 -N '' -C 'jax-seldon release workflow' -f aur-release
   ```

   Add `aur-release.pub` under *My Account → SSH Public Key* (one key per
   line; keep your personal key there too). The AUR has no per-package
   deploy keys: this key can push every package of the account.
3. **Secret `AUR_SSH_PRIVATE_KEY`** in `JohnAndrewsX/jax-seldon`:

   ```
   gh secret set AUR_SSH_PRIVATE_KEY --repo JohnAndrewsX/jax-seldon < aur-release
   shred -u aur-release
   ```

   The package name `jax-seldon` is claimed by the first push; nothing to
   create on the AUR beforehand.
4. **Repository `JohnAndrewsX/jax-seldon-plugin`**, public and **empty**
   (no README, licence or .gitignore — the first push of the split must
   not need a merge; the split carries `plugin/README.md`, which points
   to this repository):

   ```
   gh repo create JohnAndrewsX/jax-seldon-plugin --public \
     --description 'Seldon plugin for Omarchy (generated from JohnAndrewsX/jax-seldon; do not edit)'
   ```

5. **Secret `PLUGIN_REPO_TOKEN`**: a fine-grained personal access token
   (GitHub → Settings → Developer settings → Fine-grained tokens), resource
   owner `JohnAndrewsX`, *Only select repositories* →
   `jax-seldon-plugin`, permission *Contents: Read and write*, an expiry
   you will notice. Then
   `gh secret set PLUGIN_REPO_TOKEN --repo JohnAndrewsX/jax-seldon`.
6. **Actions may push to `main`.** `bump` pushes with the workflow's own
   token. If `main` is protected, allow GitHub Actions to bypass the rule
   or bump by hand after each release.
7. **Maintainer address.** `packaging/PKGBUILD` starts with
   `# Maintainer: JohnAndrewsX <EMAIL>`. The AUR convention is
   `Name <email>`; replace `EMAIL` with the address you want public on
   the AUR (often obfuscated, `name at example dot org`) and commit it
   before the first tag. Agents never fill it in.
8. Run the dry run; the summary must say `true` for both secrets.

Without steps 2–5 a tag still builds, tests and publishes the GitHub
release; `aur` and `plugin` end with a notice.

## Testing the PKGBUILD on a disposable host

`makepkg` builds as the current user and needs no root, but on the dev
host the red-zone guard blocks it (AGENTS.md §6). Use a disposable host
(the test host, a VM or a container) with `base-devel`, `git` and `cargo`
(`rust` or `rustup` with a stable toolchain). Never pass `-s`, `-i`,
`--syncdeps` or `--install`; installing is the operator's step.

Only these two makepkg forms, each as its own ssh command (the forms
the guard's planned ssh exception allows; until then the operator runs
them, or the CI dry run stands in):

```
ssh <test-host> 'cd /tmp/<dir> && makepkg -f'
ssh <test-host> 'cd /tmp/<dir> && makepkg --printsrcinfo > SRCINFO.new'
```

The whole procedure, from the repository root on the dev host:

```
git archive --format=tar.gz --prefix=jax-seldon-0.1.0/ -o /tmp/jax-seldon-0.1.0.tar.gz HEAD
ssh <test-host> 'mkdir /tmp/seldon-pkg'
scp packaging/PKGBUILD /tmp/jax-seldon-0.1.0.tar.gz <test-host>:/tmp/seldon-pkg/

# .SRCINFO: generate there, compare here
ssh <test-host> 'cd /tmp/seldon-pkg && makepkg --printsrcinfo > SRCINFO.new'
scp <test-host>:/tmp/seldon-pkg/SRCINFO.new /tmp/SRCINFO.new
cmp /tmp/SRCINFO.new packaging/.SRCINFO && echo ".SRCINFO identical"

# build, then list and run what it packaged
ssh <test-host> 'cd /tmp/seldon-pkg && makepkg -f'
ssh <test-host> 'tar tf /tmp/seldon-pkg/jax-seldon-0.1.0-1-x86_64.pkg.tar.zst' \
  | grep -v '^\.' | LC_ALL=C sort | diff packaging/expected-files.txt - && echo "file list ok"
ssh <test-host> 'mkdir /tmp/seldon-pkg/root && tar xf /tmp/seldon-pkg/jax-seldon-0.1.0-1-x86_64.pkg.tar.zst -C /tmp/seldon-pkg/root'
ssh <test-host> '/tmp/seldon-pkg/root/usr/bin/seldon --version'
ssh <test-host> '/tmp/seldon-pkg/root/usr/bin/jax-seldon --version --json'
ssh <test-host> '/tmp/seldon-pkg/root/usr/bin/seldon watch --help'

ssh <test-host> 'rm -rf /tmp/seldon-pkg'
rm /tmp/jax-seldon-0.1.0.tar.gz /tmp/SRCINFO.new
```

makepkg uses the tarball in the build directory instead of downloading
it (same file name as the `source` entry). With the committed
`sha256sums=('SKIP')` this works for any tree; after a release the sum is
real and the tarball must be the release asset itself.
