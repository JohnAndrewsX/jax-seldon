# Seldon task runner. `just check` is the gate every WP runs before handover.
#
# Host-only steps (`omarchy plugin validate`, qmllint against the installed
# shell) need an Omarchy install. CI sets SELDON_SKIP_HOST_CHECKS=1 to skip
# them with a notice; everywhere else a missing tool is an error.
# See docs/TESTING.md.

set shell := ["bash", "-euo", "pipefail", "-c"]

omarchy_path := env_var_or_default("OMARCHY_PATH", "/usr/share/omarchy")
skip_host := env_var_or_default("SELDON_SKIP_HOST_CHECKS", "")
musl_target := "x86_64-unknown-linux-musl"

# List recipes.
default:
    @just --list

# Everything a WP must pass: engine, contract, plugin.
check: fmt-check clippy test check-watch check-packaging check-install schema-validate docs-check plugin-validate qmllint plugin-test
    @echo "check: ok"

# rustfmt, no changes allowed.
fmt-check:
    cargo fmt --manifest-path engine/Cargo.toml --all -- --check

# Clippy with warnings as errors.
clippy:
    cargo clippy --manifest-path engine/Cargo.toml --locked --all-targets -- -D warnings

# Engine tests.
test:
    cargo test --manifest-path engine/Cargo.toml --locked

# The optional `watch` feature (WP-034): clippy and tests with it.
check-watch:
    cargo clippy --manifest-path engine/Cargo.toml --locked --all-targets --features watch -- -D warnings
    cargo test --manifest-path engine/Cargo.toml --locked --features watch

# Not part of `check` (it needs an optimised compile); required before the
# handover of a WP that touches engine/src/index/ or commands/watch.rs.
# `seldon watch` RSS < 10 MB on the x10 fixture, bench profile.
check-rss:
    cargo test --manifest-path engine/Cargo.toml --locked --profile bench --features watch --test watch rss_stays_under_10_mb

# The AUR package (WP-040): PKGBUILD and helper syntax, shellcheck when
# installed, .SRCINFO in step with the PKGBUILD. Never runs makepkg.
# The release body from CHANGELOG.md (WP-048): tests/release/.
check-packaging:
    #!/usr/bin/env bash
    set -euo pipefail
    bash -n packaging/PKGBUILD packaging/set-version.sh packaging/check-srcinfo.sh \
      packaging/release-notes.sh tests/release/release-notes.test.sh
    if command -v shellcheck >/dev/null; then
      # PKGBUILD variables are read by makepkg, $srcdir/$pkgdir set by it
      shellcheck -s bash -e SC2034,SC2154,SC2164 packaging/PKGBUILD
      shellcheck packaging/set-version.sh packaging/check-srcinfo.sh \
        packaging/release-notes.sh tests/release/release-notes.test.sh
    else
      echo "check-packaging: shellcheck not installed; bash -n only"
    fi
    bash packaging/check-srcinfo.sh
    bash tests/release/release-notes.test.sh
    echo "check-packaging: ok"

# install.sh (WP-044) against a local mock of the release layout (file://
# URLs, scratch HOME and prefixes, no network); shellcheck when installed.
check-install:
    bash tests/install/install.test.sh

# Validate fixtures against schema/ (script owned by WP-002).
schema-validate:
    #!/usr/bin/env bash
    set -euo pipefail
    if [[ ! -f scripts/validate-fixtures.sh ]]; then
      echo "schema-validate: skipped (scripts/validate-fixtures.sh does not exist yet; WP-002)"
      exit 0
    fi
    bash scripts/validate-fixtures.sh

# The user guide (WP-045): relative links and anchors, the same pages and
# structure in every language under docs/user/, the German source lines,
# every `seldon …` in the guide against the engine's --help, and the CLI
# reference's help blocks equal to `seldon <command> --help`
# (`bash scripts/docs-check.sh --write` regenerates them).
docs-check:
    bash scripts/docs-check.sh

# `omarchy plugin validate plugin/` (host only).
plugin-validate:
    #!/usr/bin/env bash
    set -euo pipefail
    if [[ -n "{{ skip_host }}" ]]; then
      echo "plugin-validate: skipped (SELDON_SKIP_HOST_CHECKS set; needs the omarchy CLI)"
      exit 0
    fi
    command -v omarchy >/dev/null || { echo "plugin-validate: omarchy CLI not found" >&2; exit 1; }
    omarchy plugin validate plugin/
    echo "plugin-validate: ok"

# qmllint every plugin QML file against the installed shell, zero warnings (host only).
qmllint:
    #!/usr/bin/env bash
    set -euo pipefail
    if [[ -n "{{ skip_host }}" ]]; then
      echo "qmllint: skipped (SELDON_SKIP_HOST_CHECKS set; needs the installed Omarchy shell)"
      exit 0
    fi
    shell_dir="{{ omarchy_path }}/shell"
    [[ -d $shell_dir ]] || { echo "qmllint: shell not found at $shell_dir" >&2; exit 1; }
    # qt6-declarative puts qmllint on PATH on some hosts, in Qt's libexec dir on others.
    lint=$(command -v qmllint || true)
    [[ -n $lint ]] || lint=/usr/lib/qt6/bin/qmllint
    [[ -x $lint ]] || { echo "qmllint: not found on PATH or at /usr/lib/qt6/bin" >&2; exit 1; }
    # Quickshell serves the shell root as the `qs` module prefix (qs.Ui,
    # qs.Commons), but the installed tree has no `qs/` directory. Build an
    # import root whose qmldir files point back at the shell's own files by
    # relative path (qmllint joins entries onto the qmldir's directory, so
    # absolute paths do not work); nothing from the shell is copied or linked.
    root=$(mktemp -d)
    trap 'rm -rf "$root"' EXIT
    for qmldir in "$shell_dir"/*/qmldir; do
      dir=$(dirname "$qmldir")
      module_dir="$root/qs/$(basename "$dir")"
      mkdir -p "$module_dir"
      rel=$(realpath --relative-to="$module_dir" "$dir")
      awk -v rel="$rel" '$NF ~ /\.(qml|js)$/ { $NF = rel "/" $NF } { print }' \
        "$qmldir" > "$module_dir/qmldir"
    done
    # Two categories are demoted to info because first-party plugins hit
    # them too and no plugin code can avoid them:
    #   missing-property  nested token objects (Style.font.*, Color.popups.*)
    #                     are typed QObject in the shell's QML
    #   uncreatable-type  Quickshell's qmltypes mark PanelWindow isCreatable: false
    # Everything else must be warning-free.
    shopt -s nullglob
    files=(plugin/*.qml plugin/components/*.qml plugin/components/overlay/*.qml)
    "$lint" --max-warnings 0 --missing-property info --uncreatable-type info \
      -I "$root" -I "$shell_dir" "${files[@]}"
    # The demoted missing-property makes qmllint blind to token typos
    # (Style.font.bodySmal); check every Style/Color/Border/Util reference
    # against the shell's Commons singletons instead.
    python3 tests/plugin/check-tokens.py "$shell_dir" "${files[@]}"
    echo "qmllint: ok (${#files[@]} files)"

# Plugin logic: Model.js under node; Service.qml states and Panel.qml tabs, keys and banners in a private headless Quickshell (host only).
plugin-test:
    #!/usr/bin/env bash
    set -euo pipefail
    if [[ -n "{{ skip_host }}" ]]; then
      echo "plugin-test: skipped (SELDON_SKIP_HOST_CHECKS set; needs node, quickshell, jq and the installed shell)"
      exit 0
    fi
    command -v node >/dev/null || { echo "plugin-test: node not found" >&2; exit 1; }
    node tests/plugin/model.test.js
    node tests/plugin/model.bench.js
    bash tests/plugin/real-home-guard.test.sh
    bash tests/plugin/service-states.sh
    bash tests/plugin/panel-view.sh
    bash tests/plugin/overlay-view.sh
    echo "plugin-test: ok"

# Not part of `check` (it needs a release compile); CI runs it as its own step.
# Index build on the fixture logbook scaled x10, release; fails over 100 ms.
bench:
    cargo bench --manifest-path engine/Cargo.toml --locked --bench index

# Static release binary (needs `rustup target add x86_64-unknown-linux-musl`).
build-release:
    cargo build --manifest-path engine/Cargo.toml --locked --release --target {{ musl_target }}
    @echo "built engine/target/{{ musl_target }}/release/seldon"

# Regenerate fixtures from the engine (stub until the engine can build an index).
fixtures-refresh:
    @echo "fixtures-refresh: not implemented yet (needs \`seldon index\`; see SPEC-ENGINE §10)"

# Engine ↔ plugin end-to-end test (host only; not part of `check`).
# `just e2e` runs on the test host over ssh (SELDON_TEST_HOST, default `test`)
# and restores it; `just e2e --engine-only` runs the engine steps here in
# scratch dirs. See docs/TESTING.md, "Integration".
e2e *args:
    #!/usr/bin/env bash
    set -euo pipefail
    if [[ -n "{{ skip_host }}" ]]; then
      echo "e2e: skipped (SELDON_SKIP_HOST_CHECKS set; needs a real Omarchy host)"
      exit 0
    fi
    bash tests/integration/e2e.sh {{ args }}
