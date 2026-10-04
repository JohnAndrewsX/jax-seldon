# WP-080 HANDOVER

Branch `wp/080-review` (worktree `wt/WP-080`), base `2c1d7ec`.

## Done

- **release.yml** (`b3246f1`): step *Attest the release assets* in the
  `build` job, `actions/attest-build-provenance@4d101475d8b20a2381f78447822ac1eab6504dd8 # v4.2.2`
  (latest release; lightweight tag → commit, resolved with
  `gh api repos/actions/attest-build-provenance/git/ref/tags/v4.2.2`; the
  action is a composite wrapper that pins `actions/attest` v4.2.1 by SHA
  itself). Subjects: `seldon-X.Y.Z-x86_64-unknown-linux-musl.tar.gz`,
  `jax-seldon-X.Y.Z.tar.gz`, `SHA256SUMS`, `install.sh`. It runs after
  every check (last step before the summary), so a failing build attests
  nothing. The `build` job got `permissions: contents: read, id-token:
  write, attestations: write`; the workflow-level `contents: read` is
  unchanged, no other job changed (PKGBUILD and the AUR job untouched).
  The summary prints the attestation URL and the ref.
- **Dry-run decision: the step runs in dry runs too.** Downside named and
  neutralised: a dry run creates real, public attestations (Sigstore
  Rekor log, repo attestations API) for branch builds, and `gh
  attestation verify --repo` alone would accept them. install.sh
  therefore pins `--signer-workflow
  JohnAndrewsX/jax-seldon/.github/workflows/release.yml` and
  `--source-ref refs/tags/<tag>`; a dry-run attestation names
  `refs/heads/<branch>` and never passes. Remaining cost: dry-run entries
  in the public transparency log (repo is public anyway). Documented in
  the release.yml header, packaging/README "Dry run", SECURITY.md.
- **tests/release/workflow-pins.test.sh**: checks workflow permissions
  are `contents: read` alone, the build job's are exactly the three, the
  attest step exists, uses attest-build-provenance, has no `if:`, names
  all four subjects; 7 new self-mutants (`expect_problem`).
- **install.sh** (`1ca5898`, comment reflow `053ceb4`): after the
  SHA256SUMS check, `verify_provenance`:
  - release ≤ v0.1.1 → note "released before attestations" (gh not asked);
  - no `gh` → note; `gh` without `attestation verify --source-ref` → note
    "update gh"; `gh` exit 4 (not logged in) → note;
  - otherwise `gh attestation verify <tarball> --repo "$REPO"
    --signer-workflow … --source-ref refs/tags/<tag>`: 0 → `attested`
    line; anything else → gh's lines on stderr, exit 2, "nothing
    installed" (before the first write, like every other refusal).
  - `--require-verified`: every "note" case refuses with exit 1 instead.
    Not allowed with `--uninstall` (usage error). `--help`, usage line,
    header comment and exit-code comment describe it.
- **tests/install/install.test.sh**: the host's `gh` is never on PATH (left
  out of the linked tool dirs; stub first where a test uses the host
  `$PATH`). Stubs: `gh-ok` (verifies the file's sha256 against
  `$work/attested`, honours `--repo/--signer-workflow/--source-ref`),
  `gh-noauth` (exit 4), `gh-old` (no `attestation`), `gh-none`. Mock
  releases: v9.9.3 tarball+SHA256SUMS replaced (tampered), v9.9.2
  attested only for a branch ref, v0.1.1 without attestation. Matrix
  gh ok/failing/absent/noauth/old × `--require-verified` on/off, plus
  pre-attestation release, the exact gh argument line, `--uninstall
  --require-verified`, `--help`. All earlier checks unchanged and green
  (186 total).
- **Docs** (`b7cfb82`, de `f57b615`): README install section (sentence +
  `--require-verified` row), plugin/README, user guides 01 and 11 (en;
  de re-stamped to `b7cfb82` in a separate docs(de) commit), SECURITY.md
  new "Verifying a release" (the exact manual command), packaging/README
  (build row, dry run), docs/VERSIONING.md tag flow step 6, CHANGELOG
  Unreleased → Packaging and docs.
- memory/pitfalls.md: WP-080 section.

## Not done

- Nothing from the outputs list. Not done on purpose: SHA256SUMS'
  attestation is not checked by install.sh (the tarball's own attestation
  is what matters); the man page/completions come from the verified
  binary.
- install.sh cannot be exercised end to end with the *real* gh: the only
  attested assets are dry-run ones (branch ref), which install.sh rejects
  by design. The real gh behaviour was checked directly instead (below).
  The first real end-to-end check is `install.sh` after the next tag.

## Verified by

- `just check` → `check: ok` (local, shellcheck 0.11.0 from a scratch
  venv on PATH, so `check-install` and `check-packaging` ran shellcheck,
  not `bash -n`): install.test 186 passed 0 failed, workflow-pins ok,
  docs-check ok, plugin harness all green on the first run (no
  transient). The header-comment reflow `053ceb4` was committed while that
  run was in progress (comment only).
- `actionlint` (scratch venv) clean on all workflows; `shellcheck` clean
  on install.sh, install.test.sh, workflow-pins.test.sh.
- **Release dry run 1:** run `37226384458` on `1ca5898` (code only):
  success; step *Attest the release assets*: success.
- **Release dry run 2 (final code+docs head):** run `37227250296` on
  `053ceb4` (`gh run watch 37227250296 --exit-status` → 0): build
  success, step *Attest the release assets* ran and succeeded; release,
  bump, aur, plugin skipped as in every dry run. The handover commit after
  it touches only this file and memory/pitfalls.md.
- **Real gh against dry run 1's `dist` artifact** (`gh run download
  37226384458 -n dist`), gh 2.102.0:
  - all four assets, `--repo JohnAndrewsX/jax-seldon --signer-workflow
    JohnAndrewsX/jax-seldon/.github/workflows/release.yml --source-ref
    refs/heads/wp/080-review` → rc 0 each;
  - the tarball with `--source-ref refs/tags/v0.1.1` → rc 1, "expected
    SourceRepositoryRef to be refs/tags/v0.1.1, got
    refs/heads/wp/080-review" (dry-run attestation rejected by install.sh's flags);
  - `--signer-workflow …/ci.yml` → rc 1; a modified `install.sh` → rc 1.
  - logged-out gh (`GH_CONFIG_DIR=<empty scratch>`, no token) → rc 4, which
    is why install.sh treats 4 as "not usable".
- **Mutants** (install.sh mutated in place, install.test.sh run, file
  restored with `git checkout`; each killed):

  | Mutant | Failing checks |
  |---|---|
  | `verify_provenance` call removed (tampered tarball + failing gh installs) | 36 |
  | gh exit 1 accepted as success | 12 (tampered refused, exit 2) |
  | `--require-verified` ignored (`--require-verified` without gh exits 0) | 16 |
  | `--source-ref` dropped | 4 (branch-attested release refused) |
  | gh exit 4 treated as failure | 5 |
  | every release treated as attested | 6 |
  | gh feature check removed | 5 |
  | missing gh refuses without the flag | 7 |

  The two acceptance mutants: "tampered tarball with a stubbed failing gh
  is refused" = rows 1–2; "`--require-verified` without gh exits non-zero"
  = row 3. workflow-pins.test.sh carries 7 self-mutants for the
  permissions and the attest step (all found).

## Open questions / decisions

- **The WP says "keep install.sh POSIX-sh compatible as it is today";**
  it is bash today (`#!/usr/bin/env bash`, arrays, `[[ ]]`, run as `|
  bash`). I kept its dialect; nothing new needs more than the existing
  bash use.
- **Cut-off "after v0.1.1" is hard-coded** (`attested_release`). Releases
  v0.1.0/v0.1.1 have no attestations; without the cut-off every user with
  a logged-in gh could no longer install them (gh cannot tell "never
  attested" from "tampered"). Consequence: someone able to replace
  v0.1.1's assets gets a checksum-only install unless the user passes
  `--require-verified` (which refuses v0.1.1). Attesting v0.1.1 after
  the fact would claim a build that did not happen, so I did not propose
  it; the gap closes with the next release becoming "latest".
- `gh` exit codes other than 0/4 (including a network error during the
  check) refuse the install: fail closed once gh is usable. If that is
  too strict for flaky networks, the alternative is a retry, not a skip.
- Exit codes: verification failure 2 (as checksum), `--require-verified`
  without a usable gh 1 (fixable refusal). Reviewer may prefer otherwise.

## Touched outside WP scope

- `tests/release/workflow-pins.test.sh` (not an input; the natural home
  for the "pinned and least-privilege" claim and its mutants).
- `memory/pitfalls.md` (learned section).
