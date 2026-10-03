```
WP-072 HANDOVER
```

"ci: pin actions and images; audit gates the release". Branch
`wp/072-review`. Commits: `bd82bee` (accepted-advisory list and its
check), `0d9c633` (workflows, pins test, justfile), `ef76075` (docs,
CHANGELOG), `e0979e9` (this handover, `memory/pitfalls.md`). Round 2:
`4fe35d7` (pins test), `c4989c0` (docs), plus this update.

**Round 2 (review: APPROVE with a fix round)**

- N1: `workflow-pins.test.sh` now reads the `cargo audit` step itself
  (`step`, `run_block`) and requires: no `if:`; no `shell:` of its own
  (`shell: bash {0}` would drop `-e`); the exact line
  `ids=$(bash packaging/audit-ignore.sh)`; no `||` and no `set +e` /
  `set +o` in the run block; `cargo audit --file engine/Cargo.lock
  --deny warnings "${args[@]}"` as the block's last line. New built-in
  changes, all caught: `if: false`; `shell: bash {0}`; `set +e` plus
  `echo done` after the audit line ("not the last line"); `set +e`
  alone; `|| true` on the ids line; `|| true` on the audit line.
- N4: `bump`, `aur` and `plugin` must each have
  `needs: [build, release]`; the release job's check is tightened to
  exactly `build` (or `[build]`). New built-in changes, all caught:
  `aur` needs only `build`, `plugin` needs only `release`, `bump`
  without `needs`.
- N3: CONTRIBUTING.md says an expired entry also fails `just check`, and
  so CI on every push and pull request, and that this is intended.
  Shown: a scratch copy with `RUSTSEC-2022-0078 2026-10-01 …` appended
  to the real list → `audit-ignore.test.sh` exit 1, "expired on
  2026-10-01".
- N2: packaging/README.md names the test's limit (a SHA is not checked
  against its version comment; the refresh steps resolve both). The
  test's header says the same.
- Verified: `bash tests/release/audit-ignore.test.sh` → ok (21);
  `bash tests/release/workflow-pins.test.sh` → ok (24: the real
  workflows, the count, 22 built-in changes), looped 10 times clean;
  `just check-packaging` with shellcheck on PATH (scratch venv) → ok;
  shellcheck on `tests/release/*.sh` and `packaging/audit-ignore.sh`
  clean (one `SC2016` disabled on the literal workflow line, with a
  comment); actionlint on the workflows → ok. The full `just check` was
  not re-run (round 2 touches only the test and two docs; round 1's run
  is below).

## Done

- **Actions pinned to commits.** All 12 `uses:` lines in
  `.github/workflows/{ci,release,audit}.yml` name a full 40-hex commit
  with the release as a comment. The pins are the commits the floating
  `v4` tags pointed at on 2026-10-03, so the code that runs is unchanged:
  - `actions/checkout@11d5960a326750d5838078e36cf38b85af677262 # v4.4.0`
  - `actions/upload-artifact@ea165f8d65b6e75b540449e92b4886f43607fa02 # v4.6.2`
  - `actions/download-artifact@d3f86a106a0bac45b974a628896c90dbdf5c8093 # v4.3.0`
- **Container pinned to a digest.** The three job containers are
  `archlinux:base-devel@sha256:51dd3d24f7fba779e7c471caeee7804c50e8c134ad948e19685a1c83a42facc3`
  with the comment `# base-devel-20260927.0.600689` (the dated Docker Hub
  tag that has the same digest as `base-devel` today).
- **`cargo audit` gates the release.** `release.yml`'s `build` job
  installs `cargo-audit` and runs `cargo audit --file engine/Cargo.lock
  --deny warnings` with the accepted ids, after the version and
  CHANGELOG checks and before `just check`. There is no
  `continue-on-error` in `release.yml`. `release`, `bump`, `aur` and
  `plugin` all need `build`, so a finding stops every publishing job, in
  the dry run and for a tag.
- **Accepted advisories with reason and expiry.**
  `packaging/audit-ignore.txt` (empty list, header explains the format:
  `RUSTSEC-YYYY-NNNN  YYYY-MM-DD  reason`). `packaging/audit-ignore.sh
  [FILE [TODAY]]` checks it and prints the ids; it fails (nothing on
  stdout, every problem on stderr) on a malformed id or date, a missing
  reason, a repeated id, an expiry on or before today, or an expiry more
  than 365 days ahead. The release step and the weekly audit both derive
  their `--ignore` arguments from it.
- **Weekly audit stays advisory.** `audit.yml` keeps `continue-on-error`,
  uses the same list (step shell set to `bash`, so an invalid list fails
  the step and shows the warning), also runs when the list or its script
  changes, and its warning now says the next release build fails the
  same way.
- **Tests in `just check-packaging`:**
  `tests/release/audit-ignore.test.sh` (21 cases: the real list, the
  default path, accepted forms, every rejection, arguments) and
  `tests/release/workflow-pins.test.sh` (the real workflows, plus 13
  built-in changes the check must catch: tag, short SHA, missing comment,
  image by tag, image without comment, service image, one action on two
  commits, `continue-on-error`, no audit step, audit without the list,
  cargo-audit not installed, audit after `just check`, release job not
  needing build).
- **Docs.** `CONTRIBUTING.md` "Dependency advisories" (gate in the
  release build, weekly early warning, how to accept an advisory, yanked
  crates must be updated); `docs/VERSIONING.md` tag flow step 4;
  `packaging/README.md` (two file rows, the gate in the `build` row, new
  section "Pinned actions and image" with the manual refresh: when, the
  `gh api` and Docker Hub commands, and that one action has one pin
  everywhere); `CHANGELOG.md` `[Unreleased]` → Packaging and docs.

## Not done

- **Release signing / attestation and a check in `install.sh`:** not in
  this WP (operator decision pending). `install.sh` is unchanged. What
  signing would need:
  1. A method: GitHub artifact attestations (`actions/attest-build-provenance`,
     pinned like the rest; the job needs `id-token: write` and
     `attestations: write`; users verify with `gh attestation verify`), or
     a key-based signature of `SHA256SUMS` (minisign, or cosign with a key).
  2. Key custody (for a key): where the private key lives (repository
     secret vs. the operator signing offline after the build), rotation
     and revocation, and publishing the public key on a second channel
     (README, the AUR PKGBUILD, the plugin repository).
  3. `release.yml`: a signing step or job after `build`, before
     `release`, with only the permissions it needs; the signature (or
     bundle) as a release asset.
  4. `install.sh`: verify the signature of `SHA256SUMS` with a public
     key it carries, before the checksum check; decide what happens when
     the verifier tool is missing (the attestation route needs `gh`,
     minisign needs `minisign`); tests in `tests/install/` with a test key.
  5. The AUR package: `validpgpkeys` and a `.sig` source if the source
     tarball is signed with OpenPGP.
  6. The plugin repository: signed tags, and whether users are told to
     follow tags rather than `main` (depends on what `omarchy plugin
     update` supports).
  7. `SECURITY.md` / README: how a user verifies a download by hand.
- **No `.github/dependabot.yml`** (needs reviewer approval). Proposal: a
  `github-actions` ecosystem entry, weekly, which keeps SHA pins and
  their version comments up to date as pull requests. Dependabot does not
  update a workflow's `container:` digest, so the image stays a manual
  refresh either way.
- **The toolchain inside the image** is still installed from the Arch
  repositories at run time (`-Syu` in the install steps); the digest pins
  the starting image only. Not changed (out of scope; the build needs a
  current toolchain).
- **The workflows were not run** (no runner here); see "For the
  orchestrator".

## Verified by

- Acceptance greps:
  - `grep -n 'uses:' .github/workflows/*.yml` → 12 lines, every one
    `@<40 hex> # vX.Y.Z`;
  - `grep -n container: .github/workflows/*.yml` → 3 lines, every one
    `@sha256:<64 hex> # base-devel-…`;
  - `grep -n -E 'uses:|container:' .github/workflows/*.yml | grep -v -E
    '@[0-9a-f]{40} # v|@sha256:[0-9a-f]{64} # '` → nothing.
- `actionlint 1.7.12` (from `actionlint-py` in a scratch venv, with
  `shellcheck` on PATH, so the `run:` scripts were linted too) on all
  three workflows → exit 0. YAML loads with PyYAML. `shellcheck` on the
  new scripts → clean.
- `just check-packaging` → `audit-ignore.test: ok`, `workflow-pins.test:
  ok`, `check-packaging: ok`. `workflow-pins.test.sh` looped 20 times →
  no failure.
- The gate with a real `cargo-audit 0.22.2` (built into the scratchpad,
  advisory database in the scratchpad):
  - current `engine/Cargo.lock`, empty list → exit 0 (164 crates), so the
    dry run should pass this step;
  - a scratch copy of `engine/` with `cargo add --dev bumpalo@=3.11.0`
    (`cargo metadata --locked` still passes) → exit 1, `RUSTSEC-2022-0078`,
    "1 denied warning found";
  - same copy, list entry `RUSTSEC-2022-0078 2027-01-31 …` → exit 0 with
    `--ignore RUSTSEC-2022-0078`;
  - same copy, entry expired `2026-10-01` → `audit-ignore.sh` exit 1,
    "expired on 2026-10-01"; cargo audit not reached.
- The release step's `run:` body extracted from `release.yml` and run
  with `bash --noprofile --norc -eo pipefail` (GitHub's `shell: bash`)
  and a stub `cargo`: valid entry → `cargo audit … --ignore
  RUSTSEC-2022-0078`, exit 0; expired entry → exit 1 before cargo; empty
  list → `cargo audit … --deny warnings`, exit 0.
- Mutants, each in a scratch copy, each failing its test:
  - M1 `audit-ignore.sh` without the expiry check → "expiry today",
    "expiry in the past" FAIL;
  - M2 without the reason check → "no reason" FAIL;
  - M3 horizon 3650 days → "expiry more than 365 days ahead" FAIL;
  - M4 without the duplicate check → "id listed twice" FAIL;
  - M5 one `download-artifact` back to `@v4` in `release.yml` →
    `workflows` FAIL (names file, line and the tag);
  - M6 `ci.yml` image back to `archlinux:base-devel` → `workflows` FAIL;
  - M7 `continue-on-error: true` on the release audit step → `workflows`
    FAIL;
  - M8 the release `cargo audit` line replaced by `true` → `workflows`
    FAIL ("does not run cargo audit with the ignore list's arguments");
  - plus the 13 built-in changes in `workflow-pins.test.sh`, all caught.
- `just check`: see the last section.

## For the orchestrator (before the merge)

1. Dry run on the branch: `gh workflow run release.yml --ref
   wp/072-review`, then `gh run watch`. Expected green. Check in the log:
   "Initialize containers" pulls
   `archlinux:base-devel@sha256:51dd3d24…`; "Set up job" downloads
   `actions/checkout@11d5960a…` and `actions/upload-artifact@ea165f8d…`;
   the `cargo audit` step prints `accepted advisories: none` and scans
   `engine/Cargo.lock`.
2. Advisory dry run on a throw-away branch (never merged): from the WP
   branch, `cargo add --manifest-path engine/Cargo.toml --dev
   bumpalo@=3.11.0`, commit `engine/Cargo.toml` and `engine/Cargo.lock`,
   push, `gh workflow run release.yml --ref <that branch>`. Expected:
   `build` fails at the `cargo audit` step with RUSTSEC-2022-0078 (the
   Version and CHANGELOG steps pass, every later step is skipped). Then
   delete the branch on the remote.
3. Optional: `gh workflow run audit.yml --ref wp/072-review` → green,
   no warning. `ci.yml` has no manual trigger; its pinned run shows on
   the first push to `main` after the merge.
4. If the image digest has disappeared from Docker Hub by the time of the
   run (it should not; old digests of official images stay pullable),
   refresh as in packaging/README.md "Pinned actions and image".

## Learned

Added to `memory/pitfalls.md`: cargo-audit / actionlint in the
scratchpad; how to add a known advisory without breaking `--locked`; how
to resolve action tags and image digests; `function | grep -q` under
`pipefail` flakes in bash tests.

## Decisions needed

- Release signing: method and key custody (list above).
- Whether to add `.github/dependabot.yml` for the action pins.

## Touched outside WP scope

Approved by the orchestrator before the change (option 1):
`packaging/audit-ignore.sh`, `tests/release/audit-ignore.test.sh`,
`tests/release/workflow-pins.test.sh` (new), `justfile`
(`check-packaging`: the two tests, `bash -n` and shellcheck lists),
`packaging/README.md`. Also `memory/pitfalls.md` (append).

## just check

`just check` on `ef76075` (no other process in the worktree) → exit 0,
`check: ok`: 1063 cargo tests passed (plain and `--features watch`),
clippy and fmt clean, `check-packaging: ok` (with `audit-ignore.test: ok`
and `workflow-pins.test: ok`; shellcheck not on the dev host, so `bash -n`
there; the new scripts were shellchecked separately, see above),
docs-check ok (388 links), `plugin-validate: ok`, `qmllint: ok (29
files)`, plugin harnesses ok (bar-view 131/131). Scratch builds and venv
under the session scratchpad are deleted after the handover.
