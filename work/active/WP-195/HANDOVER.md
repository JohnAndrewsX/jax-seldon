# WP-195 — Handover (part 1, the Docker Hub limit, check-rss)

Rounds 2 (review 1: APPROVE, fold-ins) and 3 (Fable stage 2: APPROVE, fold-ins) are at the end. **check-rss: the limit is 11 MB (operator decision E8); proposed 12 MB, pending the operator** (round 3).

Branch `wp/195-supply-chain` (from `next` at `fdaca081`), worktree
`wt/WP-195`. Nothing pushed, no workflow triggered, no secret created.
Part 2 (the AUR package in a container) is deferred by the WP and not
started.

## What was done

- **`.github/dependabot.yml`**: `github-actions` (directory `/`, weekly,
  one group `actions` with pattern `*`, so every action bump arrives in
  one pull request and each action keeps one pin across the workflows;
  Dependabot updates the same-line `# vX.Y.Z` comment with the SHA) and
  `cargo` (directory `/engine`, weekly, group `cargo-minor-patch` with
  `update-types: [minor, patch]`; a major update gets its own pull
  request). Commit prefixes `ci` and `engine`, as in this repository's
  log. Target branch: the default branch (see Q1).
- **Does Dependabot read the `container:` digests? No.** GitHub's docs
  (supported ecosystems, read 2026-10-10): Dependabot "only supports
  updates to GitHub Actions using the GitHub repository syntax";
  `docker://` references and registry URLs are not supported, and the
  `docker` ecosystem covers Dockerfiles, Kubernetes manifests and
  Compose, not workflow containers. So the manual image refresh in
  `packaging/README.md` stays, and the README and `dependabot.yml` say
  so.
- **`scripts/check-no-network.sh`** in `just check-packaging`: `cargo tree
  --manifest-path engine/Cargo.toml --locked --offline -e normal,build
  --all-features --prefix none --format '{p}'`, fails on `reqwest`,
  `hyper`, `h2`, `tokio`, `async-std`, `ureq`, `curl`, `isahc`, `rustls`,
  `native-tls`, `openssl`, `openssl-sys`, `hickory-*`, `trust-dns-*`
  (exact crate names, globs only for the two families; `mio` allowed with
  the one comment), and on `std::net`, `std::{…, net}`, `TcpStream`,
  `UdpSocket` or `TcpListener` in a `.rs` file under `engine/src` (and
  `engine/build.rs`, should one appear). `--tree FILE` and `--src DIR`
  are the test mode. Exit 2 when the graph cannot be read (no offline
  registry) or is empty. On the real tree: 72 crates, none denied.
- **`tests/release/no-network.test.sh`**: 25 cases. Each of the 14 denied
  crates appended to a fake clean graph turns it red and is named;
  `reqwest … (*)` too; two denied crates are both reported; six scratch
  sources (`use std::net::TcpStream`, a `std::net::` path, `use std::{io,
  net}`, `TcpListener`, `UdpSocket`, `TcpStream` alone) turn it red; the
  clean graph with `mio`, `curly` and `tokio-free-parser` and a source
  with "networked"/"std::network_name" in prose stays green, as does a
  network name in a non-`.rs` file; an empty graph is exit 2.
- **Docker Hub's pull limit → a GHCR mirror of the pinned digest.**
  - `packaging/mirror-image.sh`: reads the one
    `image: ghcr.io/johnandrewsx/jax-seldon/archlinux@sha256:<d> # <tag>`
    the workflows pin (refuses none or two), logs in to ghcr.io with the
    job's token (stdin, an auth file in `RUNNER_TEMP`, removed on exit),
    and only when GHCR does not serve `<d>` (the sha256 of the raw
    manifest's bytes must equal `<d>`) runs `skopeo copy --all
    --preserve-digests --retry-times 3
    docker://docker.io/library/archlinux@sha256:<d>
    docker://ghcr.io/…/archlinux:<tag>`, then checks again that GHCR
    serves `<d>`. So Docker Hub is asked once per digest. That is the
    WP's "runs only when the digest changes", and it also heals a deleted
    package. Same digest means the same bytes; only the registry changes.
  - `ci.yml`, `audit.yml`, `release.yml`: a new first job `image`
    (`ubuntu-24.04`, whose image ships skopeo 1.13.3; permissions exactly
    `contents: read`, `packages: write`; checkout without persisted
    credentials; one step running the script). The image-using jobs
    (`check`, `audit`, `build`) `needs: image`, have `packages: read`, and
    use `container: image: ghcr.io/…@sha256:<same digest>` with
    `credentials` (`github.actor`, `secrets.GITHUB_TOKEN`). `packages:
    write` appears in the three `image` jobs only. The release `build`
    job's permissions are now `contents: read`, `packages: read`,
    `id-token: write`, `attestations: write`.
  - `tests/release/mirror-image.test.sh` (11 cases, a fake `skopeo`): on
    GHCR → no copy; not on GHCR → one copy with the exact arguments and a
    second check; the auth file removed; GHCR serving other bytes → copied
    again; a failed copy → exit 2 with the hint; a copy GHCR then does not
    serve → exit 2; two digests, no mirror pin, no token → refused before
    any registry call; the real workflows → one digest. Mutation check by
    hand (four `sed` mutants of the script: no early exit, no check after
    the copy, no `--preserve-digests`, a digest compare that always
    passes): all four killed.
  - `tests/release/workflow-pins.test.sh`: new rules (every image from
    `ghcr.io/johnandrewsx/jax-seldon/`; a workflow with a container job
    has an `image` job; that job has exactly contents: read and packages:
    write, no `if:`, no container, and runs the script; no other job
    writes packages; every container job needs `image`, has `packages:
    read` and logs in with the job's token) with 12 new mutants; the two
    old image mutants rewritten for the new form; the build job's expected
    permissions include `packages: read` (new mutant: without it, red).
    70 checks, all green.
- **check-rss re-measured, limit 12 MB.** Dev host, `next` `fdaca081`,
  bench profile, 8 runs: peak 11 348 to 11 632 kB (median 11 460), heap
  3 184 to 3 340 kB; 5 more runs on the branch: 11 392 to 11 572 kB (same
  heap). The limit is the highest peak plus a margin of at least twice the
  run-to-run spread (284 kB), rounded up to a whole MB: 11 632 + 656 =
  12 288 kB. The rule, the history (10 → 11 → 12 MB) and a table with a
  row per place are in docs/TESTING.md, "Memory bound". The test is
  renamed `rss_peak_stays_under_the_limit_on_the_x10_fixture` (no MB in
  the name any more), and `just check-rss` runs it with `--nocapture`, so
  every run prints its measurement line. `ci.yml` runs `just check-rss`
  five times after `just bench`, writes the five lines to the run summary,
  and only warns (an annotation) when a run is over the limit, so CI
  measures before it gates. SPEC-ENGINE's budget line says 12 MB.
- **Docs**: `packaging/README.md` (the `mirror-image.sh` row, the `image`
  job row, the GHCR paragraph under "Pinned actions and image", the image
  refresh steps now naming the GHCR lines and the branch-not-fork rule, a
  "Dependabot" section with the operator's settings), docs/TESTING.md
  (the `check-packaging` row, "What CI runs", "Dependabot pull
  requests": how one is reviewed, an allowed crate only, AGENTS.md §7;
  the memory bound), CHANGELOG `[Unreleased]` → Docs.

## What was not done

- **Test host RSS: not run.** The WP asks for it, but this brief allowed
  no network beyond public docs, so I did not ssh. Command for whoever
  measures it: `just check-rss` five times on the test host's checkout
  (the line `watch on ×10 (optimised): … peak … kB` per run), then fill
  the table row in docs/TESTING.md and apply the rule.
- **CI RSS: not run** (CI runs only after the merge). The first CI run
  of `ci.yml` puts the five peaks into its run summary.
- **12 MB comes from one host.** If the test host and CI stay well
  under it, the rule lowers it; if either is higher, it rises with a
  recorded reason.
- **Part 2** (namcap, `pacman -U`/`-R` in the release container): deferred
  by the WP until the AUR package exists.
- **`dependabot.yml` accepted by GitHub, the GHCR mirror's first copy,
  CI green with the new jobs: not run** (needs the merge and GitHub).
- Not in this WP, as the WP says: `actionlint`, an Arch Linux Archive
  snapshot, `cargo deny`, double builds.

## How it was verified

| Check | Where | Result |
|---|---|---|
| `SELDON_FULL_CHECK=1 just check` (all recipes, Quickshell harnesses included) | desktop (dev host; target, TMPDIR and a private 0700 XDG_RUNTIME_DIR under `jax-seldon-private/gates/`, `*-dev195`) | `check: ok`, exit 0, at `4788bea9` (log: `jax-seldon-private/gates/check-dev195.log`, first line `head 4788bea9…`); the commit after it only reflows the image jobs' comments in the three workflows, and `workflow-pins.test.sh` and `mirror-image.test.sh` ran green on it again |
| `bash scripts/check-no-network.sh` on the real graph and `engine/src` | fixture (the dev host's offline registry) | ok, 72 crates |
| `bash tests/release/no-network.test.sh`, both mutant kinds | fixture | 25/25 |
| `bash tests/release/mirror-image.test.sh` + 4 hand mutants of the script | fixture | 11/11, 4/4 killed |
| `bash tests/release/workflow-pins.test.sh` | fixture | 70/70 |
| `dependabot.yml` and the three workflows parse as YAML (python `yaml.safe_load`) | fixture | ok |
| `just check-rss` (13 runs, see above) | desktop | all under 12 288 kB |
| shellcheck on the new and changed scripts | not run, CI (shellcheck is not installed here; written to pass it, `bash -n` ran) | — |
| `dependabot.yml` accepted (the Dependabot tab shows no config error) | not run, CI | — |
| GHCR mirror's first copy, `check`/`audit`/`build` pulling from GHCR | not run, CI | — |
| check-rss on the runner | not run, CI | — |
| check-rss on the test host | not run | — |

The real `~/Seldon`, `~/.local/state/seldon` and `~/.config` were not
touched; no guard block occurred; nothing was written under `/tmp`.

## What the operator clicks on GitHub

1. **Dependabot alerts and security updates** in `JohnAndrewsX/jax-seldon`:
   Settings → (Security) **Advanced Security** → *Dependency graph*
   Enable (already on for a public repository) → *Dependabot alerts*
   **Enable** → *Dependabot security updates* **Enable** → *Grouped
   security updates* **Enable**. Do **not** press *Configure* under
   *Dependabot version updates*: the file comes with this merge. (The same
   as `jax-seldon-private/study-tcballard/BRANCH-PROTECTION.md`, Part C.)
2. **GHCR: nothing beforehand; make the package public after the first
   push to `next`.** The `mirror` job creates the package
   `jax-seldon/archlinux` with the workflow's token on that push. Expect it
   **private** at first (GitHub's default for a first publish; untested
   here); the repository's own CI pulls it anyway. Then: the repository's
   *Packages* → `archlinux` → Package settings → *Change visibility* →
   **Public**, so forks can pull it too. If the first `mirror` run fails
   with `denied: permission_denied`: Package settings → *Manage Actions
   access* → add `jax-seldon` with role **Write**, then re-run the job. No
   secret, no token.
3. After the merge, look once at Insights → **Dependency graph** →
   **Dependabot** tab: `.github/dependabot.yml` listed, no error.

## Open questions

1. **Dependabot's target branch** (answered in round 2: `next` until 0.2.0). Its pull requests go to `main` (the
   default branch); WPs merge into `next`. Keep it (security fixes land on
   `main` at once, and `next` takes them with the next merge from `main`),
   or set `target-branch: next` for version updates? (Security updates
   always go to the default branch.)
2. **The first GHCR push** needs GitHub to let the workflow token create
   a package under a user account. GitHub's docs say a package published
   from a workflow with `GITHUB_TOKEN` is linked to the repository. I could
   not try it (no push). The fallback is click 2 above.
3. **A fork's CI** pulls `ghcr.io/johnandrewsx/…`: fine while the
   repository, and so the package, is public; a fork cannot mirror a new
   digest itself (read-only token, the job says so).
4. **Gating check-rss in CI** once the CI numbers are in, or keep it a
   warning (shared runners are noisy)?
5. Stage 2 (Fable) per the WP is for part 2; this part changes
   `release.yml` (the `image` job and the `build` job's permissions), so
   the reviewer may want Fable on that diff too.

## Round 2 (review 1: APPROVE, fold-ins)

Commits `2750bcb7` (workflows, pin test, script), `e7ab6063` (Dependabot),
`f3078115` (docs), then this section.

- **Q4: the copy runs on a push to `main` or `next` only.** The job is now
  `mirror` (was `image`), in `ci.yml` and `audit.yml` only, with `if:
  github.event_name == 'push'`; both workflows push-trigger on `[main,
  next]` (`ci.yml` gained `next`, `audit.yml` too). The jobs that use the
  image `needs: mirror` with `if: ${{ !cancelled() &&
  (needs.mirror.result == 'success' || needs.mirror.result == 'skipped')
  }}`: on a pull request, a schedule or a dispatch the mirror is skipped
  and they pull by digest; after a failed mirror they do not run.
  `release.yml` has no mirror job any more: tags and dry runs pull by
  digest only (the tag's commit was pushed to `main` first), and `build`
  no longer needs anything. The `packages: write` token therefore never
  reaches a pull request's code (the reviewer's checkout-token remark is
  moot). Consequence, documented in packaging/README.md: a digest refresh
  must be pushed to `next` before its pull request can pull the image.
- **workflow-pins.test.sh**, rules rewritten: `packages: write` only in a
  job named `mirror`; that job has exactly `contents: read` and `packages:
  write`, `if: github.event_name == 'push'` exactly, no container, and runs
  the script; a workflow with a mirror job push-triggers on `branches:
  [main, next]` only, with no `tags`, `tags-ignore` or `branches-ignore`;
  `ci.yml` must have one; in a workflow with one, every container job
  needs it and carries the exact `if:` above; every container job has
  `packages: read` and logs in with the job's token. Mutants (16 for the
  mirror rules): no needs, needs another job, no `if:` on check, `always()`
  instead of the result check, no `packages: read`, no token, no mirror
  job in `ci.yml`, the job without the script, with `id-token`, without
  its `if:` (so also on pull requests), with `!= 'pull_request_target'`,
  in a container, tags added to the push trigger, `branches: ['**']`,
  and check and build writing packages. 76 checks, all green.
- **F1:** the CI check-rss step collects the measurement lines with `||
  true`; with none it writes `(no measurement line: …)` inside a closed
  fence, prints `::error title=check-rss::…` and exits 1. Checked with a
  stub `just` and the step body extracted from `ci.yml` under `bash
  --noprofile --norc -eo pipefail`: a measurement → rc 0, the summary;
  over the limit → rc 0, the summary, the warning; a compile error →
  rc 1, the error, the fence closed.
- **F2:** the copy's hint names Docker Hub's limit, GHCR being
  unavailable, and the token (needs `packages: write`); the script's header
  and packaging/README.md say there is no fallback to Docker Hub when GHCR
  is down. `mirror-image.test.sh` checks the three causes in the hint.
- **F3:** packaging/README.md states the package visibility as expected
  and untested until the first push, with the check; operator click 2
  above says the same.
- **Q3:** `.github/dependabot.yml` has `target-branch: next` for both
  ecosystems; packaging/README.md ("Dependabot"), the file's comment and
  docs/TESTING.md say it moves to `main` after 0.2.0, and that security
  updates always go to `main`. This replaces round 1's open question 1.
- **Q1/F4: the 12 MB limit stays on the branch, and it replaces operator
  decision E8 (11 MB) pending the operator's yes.** CHANGELOG and
  docs/TESTING.md ("Memory bound", the history line) say so. The
  orchestrator measures on the test host after the merge; TESTING.md's
  test-host row says that.

**Verified (round 2):**

| Check | Where | Result |
|---|---|---|
| `just check-packaging` (all its tests, `check-no-network` on the real graph) | fixture (dev host, offline registry) | ok |
| `workflow-pins.test.sh` | fixture | 76/76 |
| `mirror-image.test.sh` | fixture | 11/11 |
| the check-rss step with a stub `just`, three cases | fixture | as above |
| `dependabot.yml` and the workflows parse as YAML | fixture | ok |
| `SELDON_FULL_CHECK=1 just check` | not run in round 2 (no engine, plugin or schema change since `4788bea9`, where it was green on the desktop) | — |
| shellcheck | not run, CI | — |
| the first `mirror` run on a push to `next`, Dependabot against `next`, pull requests pulling from GHCR | not run, CI | — |

## Round 3 (Fable stage 2: APPROVE, fold-ins)

- **The check-rss limit is back at 11 MB (operator decision E8).
  Proposed: 12 MB, pending the operator.** Only the value is reverted
  (`LIMIT_KB = 11 * 1024`, SPEC-ENGINE's budget line back to 11 MB). Kept:
  the measurements and the rule in docs/TESTING.md (now "how a limit is
  proposed; the operator decides it"), the test's new name, `--nocapture`,
  and the CI measurement step. With 11 MB, `just check-rss` fails on the
  dev host on `next` itself (as before WP-195), and the CI step will warn
  until the operator decides. packaging/README.md ("The memory limit of
  `seldon watch`"), the justfile comment, the test's comment and the
  CHANGELOG say "proposed 12 MB, pending the operator".
- **Visibility:** packaging/README.md and operator click 2 above now
  expect the package **private** at first publish, then Packages →
  `archlinux` → *Change visibility* → Public.
- **3a, the first copy:** it is one anonymous Docker Hub pull from a
  shared runner, so it can hit the same limit that stopped PR #7; then
  `mirror` is red and `check`/`audit` are skipped, and the fix is
  *Re-run failed jobs* later, not a change.
- **3b, a pull request that changes the digest** (this WP's own, if it
  lands through one) has red `check` and `audit` by design until the push
  to `next` creates the mirror; the orchestrator's local merge and push to
  `next` avoids it. (Fable: if `next` or `main` ever require `check`, add
  a `workflow_dispatch` path to the mirror first.)
- **skopeo on `ubuntu-24.04`: confirmed.** actions/runner-images,
  `images/ubuntu/Ubuntu2404-Readme.md` (Image Version 20261004.327.1, read
  2026-10-10), lists "Skopeo 1.13.3" under Tools. No install step added.

**Verified (round 3):**

| Check | Where | Result |
|---|---|---|
| `just check-packaging` | fixture | ok |
| `cargo clippy --all-targets --features watch -D warnings`, `cargo fmt --check` | fixture | ok |
| `just check-rss` with the 11 MB limit | desktop | **fails, as expected and as on `next`:** 3 runs, peak 11 368 to 11 480 kB ≥ 11 264 kB (heap 3 184 to 3 336 kB); passes once the proposed 12 MB is accepted |
| `SELDON_FULL_CHECK=1 just check` | not run in round 3 (only a test constant and docs changed; green at `4788bea9` on the desktop) | — |
| the first `mirror` run, GHCR visibility | not run, CI | — |
