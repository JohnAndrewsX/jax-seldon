WP-001 HANDOVER

Branch `wp/001-scaffold`, worktree `wt/WP-001`. Not pushed, no PR.

## Done

- **Engine** `engine/`: crate `seldon` 0.1.0, edition 2024, `rust-version`
  1.85, bin `seldon`. Dependencies: `clap` (derive) and `serde_json` only;
  `Cargo.lock` committed. `seldon --version` → `seldon 0.1.0`;
  `seldon contract-version` → `1`. Global `--json` on every command
  (`{"name":"seldon","version":"0.1.0"}`, `{"contractVersion":1}`). Exit
  codes 0–4 defined per AGENTS.md §7 / SPEC-ENGINE §3; clap parse errors
  are mapped to **1** (clap's default would be 2 = engine error); `--help`
  exits 0; no command exits 1. Eight CLI tests in `engine/tests/cli.rs`,
  including one that asserts `contract-version` equals
  `plugin/manifest.json` `seldon.contractVersion`. Release profile: LTO,
  strip, `panic = "abort"`.
- **Plugin** `plugin/`: `manifest.json` per SPEC-PLUGIN §1 (kinds
  `service`, `bar-widget`, `overlay`; no `panel`, see the finding below),
  plus the optional `barWidget.description`. Stubs:
  - `Service.qml`: `Item`, injected `shell`/`manifest`, `status`, `index`,
    `contractVersion`.
  - `BarWidget.qml`: `BarWidget` + `WidgetButton` showing `⟡`, with the
    `open/close/opened` shape.
  - `Overlay.qml`: fullscreen `PanelWindow` with the "Prime Radiant" title;
    `open(payloadJson)`/`close()`/`opened`; Escape or click dismisses via
    `shell.hide`.

  All colours and fonts come from `Color`/`Style`/`Border` tokens. No
  symlinks, no `omarchy.*` ids.
- **justfile**: `check` = `fmt-check clippy test schema-validate
  plugin-validate qmllint`, plus `build-release` (musl), `fixtures-refresh`
  (stub) and `schema-validate` (skips with a notice while
  `scripts/validate-fixtures.sh` is missing). Host-only steps skip only when
  `SELDON_SKIP_HOST_CHECKS` is set; otherwise a missing tool fails. qmllint
  is resolved from `PATH` first, then `/usr/lib/qt6/bin/qmllint`.
- **CI** `.github/workflows/ci.yml`: `archlinux:base-devel` container,
  installs `git rust just jq`, runs `just check` with
  `SELDON_SKIP_HOST_CHECKS=1`. What is skipped and why is documented in the
  workflow header and in `docs/TESTING.md`.
- `docs/TESTING.md`, `LICENSE` (MIT, JohnAndrewsX), `CHANGELOG.md`
  (Unreleased only), `.gitignore` additions (editor noise, `.qmlls.ini`).
- `memory/omarchy-shell.md`: appended the WP-001 findings section.

## Not done

- **The plugin has not been loaded in the live shell.** Enabling it needs
  `omarchy plugin enable`, which writes `~/.config/omarchy/shell.json` (red
  zone). The stubs are verified only by `omarchy plugin validate` and a
  zero-warning qmllint. A smoke load belongs to WP-010, or the operator can
  enable it once.
- **The CI workflow has never run.** Nothing was pushed. It is unverified
  until the first push.
- `fixtures-refresh` is an echo stub, as the WP specifies.
- `test`/`clippy` use `--locked`. The first build needs crates.io to fetch
  the crates; after that, builds work offline.

## Verified by

From a fresh `git clone --branch wp/001-scaffold` in a scratch dir on the dev host:

```
$ just check                     → exit 0
  cargo fmt --check              ok
  cargo clippy -D warnings       ok
  cargo test                     8 passed
  schema-validate: skipped (scripts/validate-fixtures.sh does not exist yet; WP-002)
  plugin-validate: ok
  qmllint: ok (3 files)
  check: ok
$ omarchy plugin validate plugin/   → exit 0
$ find plugin -type l | wc -l       → 0
$ seldon --version                  → seldon 0.1.0
$ seldon contract-version           → 1
$ seldon --version --json           → {"name":"seldon","version":"0.1.0"}
$ seldon contract-version --json    → {"contractVersion":1}
$ seldon bogus                      → exit 1
$ just build-release                → ELF 64-bit, static-pie linked, stripped
$ SELDON_SKIP_HOST_CHECKS=1 just check → exit 0, both host steps print "skipped"
```

I checked that the qmllint gate actually bites: adding a QML file with a
`property-override` warning makes `just qmllint` exit non-zero.

The test host was not exercised. It has no `just` and no musl target, so
`build-release` fails there; `docs/TESTING.md` records this.

## Learned (also in memory/omarchy-shell.md)

- `shell toggle|summon|hide jax.seldon` routes to **Overlay.qml**, not the
  bar widget. Any plugin whose kinds include `panel|overlay|menu` belongs to
  the panel loader. The loader picks one UI kind per id: `panel` before
  `overlay` before `menu`. Adding `panel` to the manifest would therefore
  take `summon` away from the Prime Radiant.
- The overlay root gets `service` injected (the plugin's own Service.qml
  instance). Third-party services are created with no parent.
- `qmllint -I $OMARCHY_PATH/shell` cannot resolve `qs.*` imports, and
  qmllint exits 0 on warnings by default. The justfile builds a temporary
  `qs/` import root from relative-path qmldir files.
- Two qmllint categories cannot be avoided, even by first-party plugins:
  - `missing-property` on nested `Style`/`Color` tokens
  - `uncreatable-type` on `PanelWindow`

  Both are demoted to info.

## Decisions needed

1. **qmllint policy.** Do you accept demoting `missing-property` and
   `uncreatable-type` to info? Strictly, the acceptance line "qmllint clean
   with `-I $OMARCHY_PATH/shell`" is impossible as written. Plain qmllint
   with only that `-I` exits 0 but prints about 40 import warnings. The gate
   I built has zero warnings in every other category.
2. **Engine JSON shapes.** `--version --json`, `contract-version --json` and
   the user-error object `{"error":{"code":1,"message":…}}` are my choice.
   SPEC-ENGINE does not define them. Should SPEC-ENGINE §3 fix them, or
   should WP-003 confirm?
3. **SPEC-PLUGIN §8 vs shell routing.** "open/close/toggle on the bar
   widget" via `shell toggle jax.seldon` is not what the shell does; it hits
   the overlay. WP-010/WP-030 need to choose:
   - add an `IpcHandler` target in the widget, or
   - use `call jax.seldon <method>` for the panel,

   and amend SPEC-PLUGIN §6/§8.
4. **Spec drift to fix (normative docs, not my scope):**
   - SPEC-PLUGIN §1 cites `~/.local/share/omarchy/shell/README.md`; the
     correct path is `$OMARCHY_PATH/shell/README.md`.
   - SPEC-PLUGIN §3 calls the property `state`; the code uses `status`
     because `state` clashes with `Item.state`.
5. **Optional CI improvement.** `omarchy-plugin-validate` is bash + jq. CI
   could fetch it from a pinned Omarchy tag and run it instead of skipping.
   This would add a network fetch to CI; operator's call.
6. **Guard false positives** (`scripts/guard.sh`, operator-owned):
   - It blocks `cp`/`ln` when `/usr/...` is the **source** path (read-only),
     e.g. copying the shell into a temp dir.
   - It blocks any command whose text contains `pacman`, including a heredoc
     that writes a CI file.

   I did not work around either: I used a different, read-only qmllint
   approach and the Write tool for the CI file. You may want to tighten the
   regexes.

## Touched outside WP scope

None beyond what the brief assigned:
- `memory/omarchy-shell.md` (append)
- `docs/TESTING.md`
- `.gitignore`

`schema/`, `fixtures/` and `scripts/` were not touched.
