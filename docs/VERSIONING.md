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

## `contractVersion`

`contractVersion` is one integer kept in four places: the engine
(`CONTRACT_VERSION` in `engine/src/lib.rs`, printed by
`seldon contract-version`), `schema/index.schema.json` (a `const`), every
`index.json`, and `plugin/manifest.json` (`seldon.contractVersion`).
The plugin refuses an index with a different number and shows the
contract-mismatch banner (`docs/CONTRACT.md`, rule 3). A contract change
therefore always breaks a mixed install — a new engine with an old
plugin, or the other way round.

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

## Tag flow

All on `main`, after every work package of the release is merged:

1. Set the version in `engine/Cargo.toml` (cargo updates
   `engine/Cargo.lock`) and in `plugin/manifest.json`; raise `engineMin`
   if needed.
2. Move the `[Unreleased]` lines under `## [X.Y.Z] - YYYY-MM-DD`; update
   the link references; run `bash packaging/release-notes.sh X.Y.Z`.
3. `just check` green; commit (`release: X.Y.Z`); push.
4. Run the release workflow's dry run on `main` and read its summary
   (packaging/README.md, "Dry run"). It must be green.
5. Tag and push: `git tag -a vX.Y.Z -m "Seldon X.Y.Z"`,
   `git push origin vX.Y.Z`. The workflow builds, publishes the GitHub
   release with the CHANGELOG section as its notes, then the AUR package
   and the plugin repository.

The dry run in step 4 already fails without the section. If a tag
build fails on it anyway, no release exists yet: fix the CHANGELOG on
`main`, then move the tag to the fixed commit (delete it locally and on
GitHub, tag again, push). This is the operator's step.
