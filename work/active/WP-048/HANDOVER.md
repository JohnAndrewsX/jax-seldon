WP-048 HANDOVER

Branch `wp/048-repo-hygiene`, worktree `wt/WP-048`, based on `main` at
`549f40e`. Not pushed, no PR.

| Commit | What |
|---|---|
| `d4b0844` | `packaging/release-notes.sh`, `tests/release/release-notes.test.sh` (in `just check-packaging`), release job notes step, `ci.yml` `audit` job |
| `9b95460` | CONTRIBUTING.md, SECURITY.md, CODE_OF_CONDUCT.md, `.github/ISSUE_TEMPLATE/{bug_report,feature_request,config}.yml`, `.github/PULL_REQUEST_TEMPLATE.md` |
| `9fc5af8` | docs/VERSIONING.md, CHANGELOG `[Unreleased]` entry |
| `c8b12c7` | memory/pitfalls.md |
| HEAD | this handover |

## Done

- **CONTRIBUTING.md**: prerequisites and `just check` (with
  `SELDON_SKIP_HOST_CHECKS=1` for contributors without Omarchy), the
  work-package/worktree flow in two paragraphs (plus how outside
  contributors fit in), commit style and the English rule, specs /
  contract / ADR rules, review expectations, and when `cargo audit`
  becomes blocking (four consecutive green weeks on `main`, at the
  latest before 1.0.0; ignores only via `.cargo/audit.toml` with a
  reason).
- **SECURITY.md**: supported versions (latest 0.x only), reporting via
  GitHub private vulnerability reporting, with the PKGBUILD maintainer
  address as the alternative, response times, scope (engine as the user,
  no network, redaction; plugin unsandboxed in the shell; release chain)
  and what is out of scope. Also covers `jax-seldon-plugin`.
- **Private vulnerability reporting enabled** on `JohnAndrewsX/jax-seldon`
  with `gh api -X PUT repos/JohnAndrewsX/jax-seldon/private-vulnerability-reporting`
  (the Outputs allowed it); read back `{"enabled":true}`.
- **CODE_OF_CONDUCT.md**: Contributor Covenant 2.1, the official text
  (EthicalSource `release` branch) unchanged except the Hugo front matter
  removed and `[INSERT CONTACT METHOD]` → the maintainer address. A
  `diff` against the source shows only that line (plus the front matter).
- **Issue forms**: `bug_report.yml` (part, what happened, expected,
  steps, **`seldon --version --json`** and **`seldon doctor`**, both
  required, plugin version, Omarchy version, install method, and a
  required checkbox confirming that host names, user names and secrets
  were removed, because doctor output names the machine);
  `feature_request.yml` (part, problem, proposal, alternatives, optional
  version/doctor). `config.yml` adds a "Report a security vulnerability"
  contact link (blank issues stay enabled).
- **PR template**: what/why, how verified, checklist (`just check`,
  tests/idempotency, specs + TESTING, contract rule, CHANGELOG line,
  English/no secrets).
- **Repository metadata** (`gh repo edit`, the only other mutating
  command):
  - `jax-seldon`: topics `omarchy arch-linux hyprland quickshell rust
    obsidian flight-recorder`, homepage
    `https://github.com/JohnAndrewsX/jax-seldon/tree/main/docs`;
    description unchanged.
  - `jax-seldon-plugin`: the same homepage and topics, except **`qml`
    instead of `rust`**, because the plugin repository contains no Rust.
    Description unchanged.
- **Release notes**: the `release` job now checks out the tag, runs
  `bash packaging/release-notes.sh "$VERSION" CHANGELOG.md`, and creates
  the release with `--notes-file` (instead of `--generate-notes`). The
  script prints the `## [X.Y.Z]` section body (to the next `## ` or the
  link references, outer blank lines trimmed) and exits 1 when the
  version is malformed or the section is missing or empty. A failing
  `release` job also stops `bump`, `aur` and `plugin`.
- **`cargo audit`**: new `audit` job in `ci.yml`, `archlinux:base-devel`
  container, Arch's `cargo-audit` package (extra, 0.22.2, depends on
  cargo), `cargo audit --file engine/Cargo.lock --deny warnings`,
  `continue-on-error: true` (non-blocking).
- **docs/VERSIONING.md**: one version and tag for engine and plugin,
  `engineMin`, what major/minor/patch mean for Seldon's public interface
  (pre-1.0 and post-1.0), `contractVersion` (four places; a bump is at
  least a minor, a major from 1.0), what CHANGELOG.md must contain
  before a tag, the tag flow, and what to do when the notes step fails.
- **CHANGELOG** `[Unreleased]` → `### Packaging and docs`, three lines.

## Not done

- **Community profile not yet green.** GitHub reads it from the default
  branch only; this branch is not pushed (as instructed). Today it reads
  `health_percentage: 42`, with description, README and license present
  and code_of_conduct, contributing, issue_template and
  pull_request_template `null`. All files are at paths GitHub recognises
  (root and `.github/`). **Re-run after the merge:**
  `gh api repos/JohnAndrewsX/jax-seldon/community/profile`.
- **No release dry run.** `workflow_dispatch` has no inputs and runs only
  `build`; the `release` job (`if: github.event_name == 'push'`) never
  runs in a dry run, so a dry run cannot show the notes step. Starting a
  workflow is also outside "`gh repo edit` is the only mutating command".
  Proven locally instead (below).
- `cargo audit` itself was not run: the dev host has no `cargo-audit`,
  and I did not install it. Stand-in below.
- No header-comment or `packaging/README.md` updates for the new notes
  step (the job table there still says "GitHub release vX.Y.Z with the
  three assets"); I kept release.yml to the release job and left
  packaging/README.md to WP-044, which edits it. VERSIONING.md documents
  the rule. A one-line follow-up after both merges.
- docs/TESTING.md has no row for `tests/release/` (WP-044 and WP-047
  both edit that file; avoided a third conflict). Same follow-up.

## Verified by

- `just check` → exit 0, `check: ok` (dev host, all host steps ran:
  `plugin-validate: ok`, `qmllint: ok (28 files)`, plugin-test, and the
  new `release-notes: ok` inside `check-packaging`).
- `bash tests/release/release-notes.test.sh` → 13 ok: the real
  CHANGELOG 0.1.0 (whole section, first line "First release.", last line
  "…preview image.", all three `###` parts, no `## ` heading, no link
  references), default path, middle section with inner blank lines,
  heading without a date, last section before link refs; must-fail:
  missing version, empty section, `0.1.0` vs `0.10.0`, malformed
  `0x10x0`, `Unreleased`, `v1.2.0`, missing file, no arguments.
- The release job's notes step body, taken from the YAML with PyYAML and
  run with `VERSION=0.1.0 RUNNER_TEMP=<scratch>` → exit 0, 49 lines,
  starting "First release. Engine `seldon` …"; with `VERSION=0.2.0` →
  exit 1, `release-notes: no '## [0.2.0]' section with content in
  CHANGELOG.md`.
- YAML: `python3 yaml.safe_load` on `ci.yml`, `release.yml` and the three
  issue-template files: all parse (jobs `check, audit`; release steps
  `checkout, download-artifact, Release notes from CHANGELOG.md, GitHub
  release`). `actionlint` (actionlint-py with shellcheck on PATH, in a
  scratchpad venv) → 0 errors in both workflows. Issue forms and
  `config.yml` validate against SchemaStore's `github-issue-forms.json`
  and `github-issue-config.json`.
- `shellcheck` 0.11.0 (shellcheck-py, scratchpad venv) clean on
  `release-notes.sh`, the test, `set-version.sh`, `check-srcinfo.sh`;
  `just check-packaging` with it on PATH → ok (CI's release build runs
  shellcheck too).
- `cargo audit` stand-in: cloned `rustsec/advisory-db` (HEAD `117edb3`,
  2026-10-02) into the scratchpad and matched every package of
  `engine/Cargo.lock` against the advisories' patched/unaffected ranges
  with a small script → **0 findings in 161 packages**. Positive
  control: a fake lock with `time 0.1.45` and `serde_yaml 0.8.0` →
  RUSTSEC-2020-0071 and RUSTSEC-2018-0005. The script does not check
  yanked crates; the real job will.
- `gh repo view … --json homepageUrl,repositoryTopics` for both repos →
  the values above; `gh api …/private-vulnerability-reporting` →
  `{"enabled":true}`.

## Learned (memory/pitfalls.md)

- The community profile reads the default branch only.
- A release dry run never reaches the `release` job.
- No shellcheck/actionlint on the dev host; use a scratchpad venv.

## Decisions needed

1. **Plugin repo topics:** `qml` instead of `rust` (the WP said "the
   same"). Change with `gh repo edit JohnAndrewsX/jax-seldon-plugin
   --remove-topic qml --add-topic rust` if the identical set is wanted.
2. **Notes check before the tag?** Today a missing section fails only
   after the tag is pushed (`release` job; the tag then has to be moved,
   VERSIONING.md says how). Running `packaging/release-notes.sh` in the
   `build` job too would make the dry run and the tag build fail early.
   That is a `build` job change, outside this WP's allowed hunk;
   suggested as a follow-up.
3. **Scheduled audit?** The audit job runs on push/PR only. A weekly
   `schedule:` would catch new advisories against an unchanged lock;
   that changes `ci.yml`'s `on:` (and would also run `check` weekly) —
   separate workflow or follow-up, operator's call.
4. The public contact in SECURITY.md and CODE_OF_CONDUCT.md is the
   PKGBUILD maintainer address `e.andres+aur@ik.me`, as briefed. Swap if
   a non-AUR alias is preferred.

## Touched outside WP scope

- `justfile`: `check-packaging` also `bash -n`/shellchecks the two new
  scripts and runs `tests/release/release-notes.test.sh` (inside the
  recipe, not the `check:` line WP-044 edits).
- `.github/ISSUE_TEMPLATE/config.yml` (security contact link) is beyond
  the two named forms.
- GitHub setting: private vulnerability reporting enabled (Outputs
  allowed it).
- Expected merge conflicts: `release.yml` (`gh release create` line, if
  WP-044 adds `install.sh` there) and `CHANGELOG.md` `[Unreleased]`;
  both trivial.
