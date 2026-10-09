# VERSIONING.md — Versions, the contract and releases

Seldon follows [Semantic Versioning 2.0.0](https://semver.org/spec/v2.0.0.html).
This page says what the version numbers promise, how `contractVersion`
relates to them, and what has to be true before a tag. The mechanics of
the release workflow are in [packaging/README.md](../packaging/README.md).

## One version for engine and plugin

The engine and the plugin are released **together, from one tag
`vX.Y.Z`, with the same version**:

| Where | Field | Set by |
|---|---|---|
| `engine/Cargo.toml` | `version` (and `engine/Cargo.lock`) | hand; the release workflow fails when the tag differs |
| `plugin/manifest.json` | `version` | hand, same value |
| `plugin/Model.js` | `PLUGIN_VERSION` | hand, same value as the manifest |
| `plugin/manifest.json` | `seldon.engineMin` | hand: the lowest engine the plugin works with |
| `packaging/PKGBUILD` | `pkgver`, `pkgrel` | the release workflow (`bump` job) |

`engineMin` is raised when the plugin starts to use an engine command,
flag or field that older engines lack. A patch release of only one side
still bumps both versions; the CHANGELOG says which side changed.

## What the numbers promise

Seldon's public interface is what users, scripts and the plugin rely
on:

- the CLI: commands, flags, `--json` output, exit codes (0–4);
- the logbook layout and frontmatter (`docs/SPEC-LOGBOOK.md`);
- `config.toml` keys;
- the contract: `schema/*.json`, `index.json`, `contractVersion`;
- the plugin's settings, IPC targets and keyboard map.

| Change | From 1.0.0 | Before 1.0.0 |
|---|---|---|
| Incompatible: a command, flag, field or config key removed or changed in meaning; a logbook change that needs a migration; a contract bump | **major** | **minor** |
| Compatible addition: a command, flag, optional field, collector, plugin surface | minor | minor |
| Fix, performance, docs, packaging that changes the shipped files | patch | patch |
| Packaging-only fix of the AUR package, same sources | `pkgrel` only, no tag | `pkgrel` only, no tag |

Before 1.0.0 the minor number is the breaking slot (SemVer §4): `0.2.0`
may break `0.1.x`, and the CHANGELOG then says so under **Breaking**.

The user guide (`docs/user/`) is in English and German; further
languages are added when there is demand, as a docs change (patch),
following the translation policy in [docs/user/README.md](user/README.md).

Downgrading the engine is not supported: the state files in
`~/.local/state/seldon` may carry fields an older engine refuses; move
`cursors.json` aside after a downgrade (doctor says so). A ledger written
by 0.2.0 holds `case-updated` and `state-loss` lines a 0.1.x engine
reports as invalid (doctor `ledger: degraded`); do not delete them — they
are valid and are read again after the upgrade.

## `contractVersion`

`contractVersion` is one integer kept in four places: the engine
(`CONTRACT_VERSION` in `engine/src/lib.rs`, printed by
`seldon contract-version`), `schema/index.schema.json` (a `const`), every
`index.json`, and `plugin/manifest.json` (`seldon.contractVersion`).
The plugin refuses an index with a different number and shows the
contract-mismatch banner (`docs/CONTRACT.md`, rule 3) — unless the index
is newer and its `contractReadableFrom` (ADR-0051; the engine's
`CONTRACT_READABLE_FROM`) is at most the plugin's number: then the plugin
reads it and asks for its own update in a quiet notice. A contract change
therefore breaks a mixed install — a new engine with an old plugin, or
the other way round — unless its ADR lowers `contractReadableFrom` for
the older plugins (a 0.2.0 plugin or later; a 0.1.x plugin knows no such
field).

- A contract bump needs an ADR, updated fixtures and both sides changed
  together (`docs/CONTRACT.md`, "Changing the contract").
- A release that bumps the contract is **at least a minor** before
  1.0.0 and a **major** from 1.0.0 on.
- The CHANGELOG section of that release names the new number, e.g.
  "Contract v2: …", under **Breaking**.
- A compatible schema clarification that keeps every valid index valid
  and the plugin's reading unchanged is not a contract bump; the ADR
  says why.

## CHANGELOG.md

`CHANGELOG.md` follows [Keep a Changelog 1.1.0](https://keepachangelog.com/en/1.1.0/).

- Every pull request with a user-visible change adds a line under
  `## [Unreleased]`, in the part it touches (`### Engine`, `### Plugin`,
  `### Packaging and docs`, as in `0.1.0`).
- Breaking changes go first, in a `### Breaking` part; security fixes
  name their advisory id.
- Until the AUR package `jax-seldon` exists, every release body starts
  with the standing paragraph for plugin 0.1.0 users, whose panel offers
  `omarchy pkg aur add jax-seldon`: update the plugin first, then restart
  the shell (WP-118). When a release is cut, it stays at the top of
  `## [Unreleased]` for the next one.

**Before a tag**, the CHANGELOG must contain:

1. A heading `## [X.Y.Z] - YYYY-MM-DD` for exactly the tagged version,
   with the `[Unreleased]` lines moved under it and a non-empty body.
   The release workflow publishes this section, without its heading, as
   the GitHub release notes (`packaging/release-notes.sh`). **Without it
   the `build` job fails** — in the dry run and in the tag build — and
   nothing is published: no GitHub release, no AUR push, no plugin push.
   The `release` job checks again before it creates the release.
2. An empty `## [Unreleased]` heading above it.
3. The link references at the end updated:
   `[Unreleased]: …/compare/vX.Y.Z...HEAD` and
   `[X.Y.Z]: …/releases/tag/vX.Y.Z`.

Check it locally before you tag: `bash packaging/release-notes.sh X.Y.Z`
prints exactly the release body, or fails with the reason.
`tests/release/release-notes.test.sh` (part of `just check`) covers the
extraction on the real `CHANGELOG.md` and on edge cases.

## Release acceptance record

Every release from 0.2.0 has one machine-readable record of its live
test, bound to the commit that was tested (WP-192, operator decision
E67): `packaging/acceptance/vX.Y.Z.json`, committed on `main` after the
live test and before the tag. "The tag waits for the live test" is then
a check, not a memory.

```json
{
  "version": "0.2.0",
  "status": "partial",
  "commit": "<the 40 hex characters of the commit deployed to the test host>",
  "date": "2026-10-12",
  "omarchy": { "version": "4.0.4-1", "channel": "rc" },
  "engine": "0.2.0+main.1a2b3c4",
  "plugin": "0.2.0",
  "scenarios": [
    { "id": "a", "title": "clean home to the desk", "where": "test host",
      "result": "passed", "counts": { "humanSteps": 2, "passwordPrompts": 0 },
      "notes": "desk at 100 %, Today shows \"Seldon is recording\"" },
    { "id": "gate", "title": "Omarchy assumptions, --gate", "where": "not run",
      "result": "not run" }
  ],
  "limitations": ["gate: WP-194 has not landed"]
}
```

| Field | What it holds |
|---|---|
| `version` | `X.Y.Z`, the version the record is for |
| `status` | `passed`, `failed` or `partial`; follows the scenarios (rules below) |
| `commit` | the full commit deployed to the test host: `just deploy-test-host` prints it as `commit   <sha>` in its summary |
| `date` | the day of the live test, `YYYY-MM-DD` |
| `omarchy` | `version`: `omarchy version`; `channel`: `omarchy version channel`, both as printed on the test host |
| `engine` | `seldon --version` on the test host: `X.Y.Z`, or `X.Y.Z+main.<short sha>` (or `next`) of the same commit |
| `plugin` | the installed plugin's manifest `version`: `X.Y.Z` |
| `scenarios` | one entry per scenario of the live test, at least one |
| `limitations` | what the test did not cover, one string each; `[]` when nothing |

A scenario has `id` (letters, digits, `.`, `_`, `-`; unique), `title`,
`where` — where it ran, one of AGENTS.md §5's evidence words: `fixture`,
`headless`, `CI`, `test host`, `desktop` or `not run` — `result`
(`passed`, `failed` or `not run`), `counts` and optional short `notes`.
`counts` are ADR-0027 §1's: `humanSteps` and `passwordPrompts` (whole
numbers, required), and when they apply `snapshotCoverage` and
`r3GatesHonoured` (percent 0–100, `null` without an R2/R3 case) and
`agentClosesReopened` (a whole number). A scenario not run has `where`
and `result` both `not run` and no `counts`.

Rules:

- `status` is `failed` when a scenario failed, else `partial` when a
  scenario was not run, else `passed`: `passed` only when every scenario
  passed. "Not run" is never "passed" (AGENTS.md §5).
- Each scenario not run has a `limitations` entry that starts with
  `<id>: ` and says why.
- No host names, user names or private paths (AGENTS.md §8): write
  `~/…`, never `/home/…`, and "the test host", never its name.
- No other fields; a typo is refused, not ignored.

`bash packaging/acceptance-check.sh X.Y.Z [REF]` (bash and jq, no
network; REF defaults to `HEAD`) reads the record as committed in REF and
refuses (exit 1, every reason listed) unless the fields and rules hold,
`status` is not `failed`, `commit` is REF or an ancestor of it, and
between `commit` and REF nothing changed outside `docs/`, `work/`,
`packaging/acceptance/` and `*.md` files that are not under `engine/`
(the engine compiles its skills and templates in). It prints each
offending path; a rename counts as both of its paths. Otherwise it
prints a summary and exits 0: the commit, the later commits, Omarchy,
the versions, each scenario with its counts and each limitation. A
`partial` record passes and says what was not run; the operator decides
on it. The version bump changes `engine/` and `plugin/`, so the live test
runs on the bumped commit; a fix after the live test means a new deploy,
a new live test and a new record. `tests/release/acceptance-check.test.sh`
(in `just check-packaging`) covers the checker. From 0.3 the release
workflow runs it on the tag and refuses a tag without a `passed` record
(WP-188).

## Tag flow

All on `main`, after every work package of the release is merged:

1. Set the version in `engine/Cargo.toml` (cargo updates
   `engine/Cargo.lock`), in `plugin/manifest.json` and in
   `PLUGIN_VERSION` in `plugin/Model.js` (the running code's version, which
   the restart notice compares with the manifest); raise `engineMin` if
   needed. `packaging/plugin-version.sh` fails when the two differ: in
   `just check-packaging`, so also in CI, which skips the host checks, and
   in the release workflow's plugin split, where both must also equal the
   tag's version.
2. Move the `[Unreleased]` lines under `## [X.Y.Z] - YYYY-MM-DD`; update
   the link references; run `bash packaging/release-notes.sh X.Y.Z`.
3. `just check` green; commit (`release: X.Y.Z`); push. Then the live
   restart test on a host with the plugin in the bar on two monitors
   (WP-162; it restarts that host's shell, so on the dev host only with
   the operator's go): install the release's plugin, note
   `ls ~/.cache/quickshell/crashes | wc -l` and `coredumpctl list
   quickshell`, run `omarchy restart shell` three times (wait for the
   bar after each), and compare: no new crash report and no new
   quickshell core.
   Deploy that commit to the test host (`just deploy-test-host`), run the
   release's live scenarios there, write
   `packaging/acceptance/vX.Y.Z.json` ("Release acceptance record"
   above), commit and push it (`release: X.Y.Z acceptance record`), and
   run `bash packaging/acceptance-check.sh X.Y.Z`: the operator's go for
   the tag quotes its output.
4. Run the release workflow's dry run on `main` and read its summary
   (packaging/README.md, "Dry run"). It must be green. One gate, before
   anything is built, is `cargo audit` of `engine/Cargo.lock`: a
   dependency advisory stops the release until the crate is updated or
   the advisory is accepted in `packaging/audit-ignore.txt`
   (CONTRIBUTING.md, "Dependency advisories").
5. Tag and push: `git tag -a vX.Y.Z -m "Seldon X.Y.Z"`,
   `git push origin vX.Y.Z`. The workflow builds, publishes the GitHub
   release with the CHANGELOG section as its notes, then the AUR package
   and the plugin repository.
6. After the workflow is green: verify the release's build provenance
   (download the engine tarball with `gh release download vX.Y.Z -p
   'seldon-*'`, then `gh attestation verify` it with the flags in
   SECURITY.md, "Verifying a release"; `install.sh` runs the same check).
   Then grep `README.md`, `plugin/README.md`, `docs/user/` and `llms.txt`
   for the previous version and for text bound to it ("from the next
   release on", sample `seldon --version` output, the project status
   line) and update it on `main`. Prefer
   wording that does not name a version in the first place. The plugin
   repository's README changes only with the next tag (subtree push), so
   keep version-bound text out of `plugin/README.md`.

The dry run in step 4 already fails without the section. If a tag
build fails on it anyway, no release exists yet: fix the CHANGELOG on
`main`, then move the tag to the fixed commit (delete it locally and on
GitHub, tag again, push). This is the operator's step.
