# TESTING.md — How Seldon is checked

One command gates every work package: `just check`, run from the repository
root. It must exit 0 before a handover (AGENTS.md §5).

## `just check`

| Step | Recipe | What it runs | CI |
|---|---|---|---|
| Format | `fmt-check` | `cargo fmt --check` on `engine/` | yes |
| Lint | `clippy` | `cargo clippy --all-targets -- -D warnings` | yes |
| Tests | `test` | `cargo test` (unit + CLI tests in `engine/tests/`) | yes |
| Contract | `schema-validate` | `bash scripts/validate-fixtures.sh` (WP-002); skipped with a notice while the script does not exist | yes |
| Plugin manifest | `plugin-validate` | `omarchy plugin validate plugin/` | **no** (dev host) |
| QML lint | `qmllint` | `qmllint` on `plugin/*.qml` and `plugin/components/*.qml` against `$OMARCHY_PATH/shell` | **no** (dev host) |

Other recipes: `just build-release` (static musl binary,
`x86_64-unknown-linux-musl`), `just fixtures-refresh` (stub until the engine
builds an index).

## What CI cannot run, and where it runs instead

CI (`.github/workflows/ci.yml`) runs `just check` in an `archlinux:base-devel`
container with `SELDON_SKIP_HOST_CHECKS=1`. That variable makes the two
host-only steps print a skip notice and exit 0:

- **`omarchy plugin validate`** needs the `omarchy` CLI, which only exists on
  an Omarchy install.
- **qmllint against the shell** needs the installed shell tree
  (`$OMARCHY_PATH/shell`, default `/usr/share/omarchy/shell`) and Quickshell's
  QML modules (`/usr/lib/qt6/qml/Quickshell`).

Both run on the **dev host**: `just check` there runs them, and a missing
tool is an error, not a skip. Never set `SELDON_SKIP_HOST_CHECKS` on the dev
host.

## qmllint details

- **Binary.** `qmllint` from `qt6-declarative`. On the dev host it is at
  `/usr/lib/qt6/bin/qmllint` (not on `PATH`); on the test host it is
  `/usr/bin/qmllint`. The recipe uses `PATH` first, then the Qt libexec dir.
- **`qs.*` imports.** Quickshell serves the shell root as the module prefix
  `qs` (`import qs.Ui`, `import qs.Commons`), but the installed tree has no
  `qs/` directory, so `-I $OMARCHY_PATH/shell` alone does not resolve them.
  The recipe builds a temporary import root with `qs/<Module>/qmldir` files
  whose entries point back at the shell's files by relative path. Nothing
  from the shell is copied or linked.
- **Zero warnings** (`--max-warnings 0`), except two categories demoted to
  info because first-party plugins hit them as well and no plugin code can
  avoid them:
  - `missing-property` — nested token objects such as `Style.font.body` or
    `Color.popups.text` are declared as `QtObject` properties, so qmllint
    sees them as plain `QObject`.
  - `uncreatable-type` — Quickshell's qmltypes register `PanelWindow` with
    `isCreatable: false`; it is created fine at runtime.

## Dev host vs test host

| | Dev host | Test host |
|---|---|---|
| Rust | rustup, musl target | pacman `rust`, no musl target → `build-release` fails |
| `just` | via mise | not installed |
| qmllint | `/usr/lib/qt6/bin/qmllint` | `/usr/bin/qmllint` |

The first build fetches the engine's crates from crates.io; after that the
build itself needs no network (`cargo build --offline` works).
