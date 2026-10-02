# Contributing to Seldon

Thanks for helping. Seldon is a small project with one maintainer and a
strict contract between its two halves, so a few rules keep it working.
Everyone taking part agrees to the [Code of Conduct](CODE_OF_CONDUCT.md).
Vulnerabilities go through [SECURITY.md](SECURITY.md), never a public
issue.

## Before you start

- **Bug or idea?** Open an issue with the matching form first. For
  anything larger than a typo, wait for a reply before you write code:
  the change may collide with planned work or need a decision (ADR).
- **Read** `PROJECT.md` (scope), `AGENTS.md` (the rules every
  contributor follows, human or agent) and the spec of the part you
  touch: `docs/SPEC-ENGINE.md`, `docs/SPEC-PLUGIN.md`,
  `docs/SPEC-LOGBOOK.md`, `docs/CONTRACT.md`.

## Build and test

One command gates every change, run from the repository root:

```
just check
```

It must exit 0 before a pull request is reviewed. What it runs and why
is in [docs/TESTING.md](docs/TESTING.md). You need:

- Rust ≥ 1.89 with `cargo`, `clippy` and `rustfmt` (Arch: `rust`), `just`,
  `git`, `jq`, `python3`; `shellcheck` if you touch `packaging/` or a shell
  script (without it, `check-packaging` falls back to `bash -n`).
- For the host-only steps (`omarchy plugin validate`, `qmllint` against
  the installed shell, the plugin harnesses): an Omarchy 4 install,
  `qt6-declarative`, Quickshell and `node`.

Without an Omarchy host, run `SELDON_SKIP_HOST_CHECKS=1 just check` —
the same set CI runs — and say in the pull request that the host steps
did not run; the maintainer runs them before merging a plugin change.

Tests never touch your real home, config, state or logbook: the engine
tests use temporary homes and a test guard. A manual run of the engine
must do the same (scratch directories, see "Manual runs" in
docs/TESTING.md); `seldon init` without them writes to `~/.config` and
`~/Seldon`.

## How work flows

Every unit of work is a **work package** `WP-NNN` under `work/`
(`queued/`, `active/`, `completed/`), with a goal, inputs, outputs and
acceptance tests. Each package is done on its own git worktree
`wt/WP-NNN` and branch `wp/NNN-short-slug`, never on `main`, and ends
with a handover (a pull request, or `work/active/WP-NNN/HANDOVER.md`)
that says what was done, what was not, how it was verified and which
questions are open. `docs/ORCHESTRATION.md` describes the full loop,
which is also how the project's AI agents work.

As an outside contributor you do not need a work package: fork, branch
from `main`, and open a pull request against `main`. The maintainer
links it to a package (or makes one) when it touches planned work. Keep
one topic per pull request; a reviewer approves or sends it back, and
only the maintainer merges.

## Commits and language

- **English** for everything in the repository: code, comments, commit
  messages, docs, identifiers, log and error messages, UI strings, pull
  requests (`AGENTS.md` §2). Logbook content a user writes stays in the
  user's language.
- Small commits, subject `area: what changed`, plus the work package
  when there is one:
  `engine: add the theme collector (WP-012)`,
  `plugin: drift sheet keyboard focus`, `docs: SPEC-ENGINE §7 redaction`.
  Areas in use: `engine`, `plugin`, `schema`, `fixtures`, `tests`,
  `docs`, `decisions`, `packaging`, `ci`, `repo`.
- Add a line under `## [Unreleased]` in `CHANGELOG.md` for every change a
  user or packager would notice (see [docs/VERSIONING.md](docs/VERSIONING.md)).
- No secrets, tokens, real host names or private paths — not in code,
  fixtures, test output or commit messages (`AGENTS.md` §8). Redact
  `seldon init`/`doctor` output (it names the machine) before you paste
  it anywhere.

## Specs, the contract and decisions

- **Specs are normative.** If code and spec disagree, fix one of them in
  the same pull request and say which.
- **The contract** (`schema/*.json`) is the only agreement between engine
  and plugin. Changing it needs an ADR, a `contractVersion` bump, updated
  fixtures (`bash scripts/validate-fixtures.sh --write-index`), and both
  sides updated together (`docs/CONTRACT.md`, "Changing the contract").
- **Decisions** are ADRs in `decisions/ADR-NNNN-*.md`, listed in
  `DECISIONS.md`. Propose one with status `proposed`; an accepted ADR is
  never edited — a new ADR supersedes it.
- Engine dependencies are limited to the crates listed in `AGENTS.md` §7;
  any other crate needs a one-line justification in the pull request.

## Review

A reviewer reads the diff against the work package (or the issue), the
specs and `AGENTS.md`, and expects:

- `just check` green, and the host-only steps run or explicitly named as
  not run;
- tests for the change, including the idempotency of anything a
  collector writes (running it twice produces no new events);
- specs, `docs/TESTING.md` and the CHANGELOG updated in the same pull
  request;
- the plugin following the active Omarchy theme (`Style` tokens only) and
  never running a shell string built from logbook content.

Review comments are about the change, not the person. Expect a first
answer within a week.

## Dependency advisories

`.github/workflows/audit.yml` runs `cargo audit` on `engine/Cargo.lock`
every Monday, on demand (*Actions → audit → Run workflow*), and on every
push or pull request that changes the lock file. It is **advisory for
now**: the `cargo audit` step may fail (`continue-on-error`), and a
RustSec finding, an unmaintained or a yanked crate then shows as a
**warning annotation** on the run and in its summary while the job stays
green. Act on it anyway — update the crate, or document why it does not
apply.

It becomes **blocking** (`continue-on-error` removed from the step) once
four consecutive weekly runs have had no warning, and at the latest
before `1.0.0`. From then on an advisory that does not affect Seldon is
ignored only by an entry in `.cargo/audit.toml` at the repository root,
with the advisory id and a one-line reason, reviewed like code.
