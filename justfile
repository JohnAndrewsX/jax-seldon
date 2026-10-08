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
check: check-runtime-space fmt-check clippy test check-watch check-packaging check-install check-deploy check-guard check-runtime-dir schema-validate docs-check plugin-validate qmllint plugin-test
    @echo "check: ok"

# The session's runtime dir (WP-161): a full /run/user/<uid> takes the
# desktop down. Warns above 50 %, refuses above 80 %; skipped where the dir
# does not exist (CI).
check-runtime-space:
    #!/usr/bin/env bash
    set -euo pipefail
    dir=/run/user/$(id -u)
    if [[ ! -d $dir ]]; then
      echo "check-runtime-space: skipped ($dir does not exist)"
      exit 0
    fi
    df -h "$dir"
    use=$(df --output=pcent "$dir" | tail -n 1 | tr -dc '0-9')
    entries=0
    [[ -d $dir/quickshell/by-id ]] && entries=$(find "$dir/quickshell/by-id" -mindepth 1 -maxdepth 1 | wc -l)
    echo "check-runtime-space: $dir is $use % full, $entries entries in quickshell/by-id"
    if ((use > 80)); then
      echo "check-runtime-space: refusing to run above 80 %; find what fills $dir first (docs/TESTING.md, \"The session's runtime dir\")" >&2
      exit 1
    fi
    if ((use > 50)); then
      echo "check-runtime-space: WARNING: $dir is over 50 % full; find what fills it before a long run" >&2
    fi

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
# `seldon watch` RSS < 11 MB on the x10 fixture, bench profile (10 MB until 2026-10-07).
check-rss:
    cargo test --manifest-path engine/Cargo.toml --locked --profile bench --features watch --test watch rss_stays_under_11_mb

# Not part of `check` (optimised compile, timing on a quiet host); required
# before the handover of a WP that touches the index build, `status` or the
# hooks. SPEC-ENGINE §1 budgets at the stated scale, bench profile (WP-076):
# the index build bench with SELDON_BENCH_X150=1 (x10 and x150 < 100 ms),
# `status` at 10 788 ledger lines / 304 cases / 365 journal files < 100 ms,
# `hook claude-code` at 10 000 lines and just below the 1000-line rebuild
# threshold < 5 ms (not recorded and recorded; the temp dir on tmpfs);
# redaction of long lines (WP-084, WP-087): 16 KB < 1 ms, 64 KB < 2 ms
# without a masked value, 128 KB with many masked values < 20 ms (two curl
# option kinds) and < 10 ms (`--password`/`token=`);
# capture cost of the config and plugins collectors (WP-113): config cold
# < 100 ms and warm < 20 ms, plugins warm < 60 ms and with cold trees
# < 150 ms on a synthetic home (the medians are the numbers to report).
# Every check, the bench included, measures a median over budget once more
# before it fails.
check-perf:
    SELDON_BENCH_X150=1 cargo bench --manifest-path engine/Cargo.toml --locked --bench index
    cargo test --manifest-path engine/Cargo.toml --locked --profile bench --test index --test hooks --test redaction --test capture_cost -- --ignored --test-threads=1 --nocapture

# The AUR package (WP-040): PKGBUILD and helper syntax, shellcheck when
# installed, .SRCINFO in step with the PKGBUILD. Never runs makepkg.
# The release body from CHANGELOG.md (WP-048): tests/release/.
# Pinned workflow actions and images, the cargo audit release gate and
# its list of accepted advisories (WP-072): tests/release/.
# The plugin's manifest version equals Model.js PLUGIN_VERSION (WP-090).
check-packaging:
    #!/usr/bin/env bash
    set -euo pipefail
    bash -n packaging/PKGBUILD packaging/set-version.sh packaging/check-srcinfo.sh \
      packaging/release-notes.sh tests/release/release-notes.test.sh \
      packaging/audit-ignore.sh tests/release/audit-ignore.test.sh \
      tests/release/workflow-pins.test.sh \
      packaging/plugin-version.sh tests/release/plugin-version.test.sh
    if command -v shellcheck >/dev/null; then
      # PKGBUILD variables are read by makepkg, $srcdir/$pkgdir set by it
      shellcheck -s bash -e SC2034,SC2154,SC2164 packaging/PKGBUILD
      shellcheck packaging/set-version.sh packaging/check-srcinfo.sh \
        packaging/release-notes.sh tests/release/release-notes.test.sh \
        packaging/audit-ignore.sh tests/release/audit-ignore.test.sh \
        tests/release/workflow-pins.test.sh \
        packaging/plugin-version.sh tests/release/plugin-version.test.sh
    else
      echo "check-packaging: shellcheck not installed; bash -n only"
    fi
    bash packaging/check-srcinfo.sh
    # WP-111: the capture upgrades the rules and the skill as the user; no
    # package hook runs seldon as root
    if grep -Eq '^[[:space:]]*install=' packaging/PKGBUILD || compgen -G 'packaging/*.install' >/dev/null; then
      echo "check-packaging: PKGBUILD must not have an install script (upgrades run as the user)"; exit 1
    fi
    bash tests/release/release-notes.test.sh
    bash tests/release/audit-ignore.test.sh
    bash tests/release/workflow-pins.test.sh
    bash packaging/plugin-version.sh plugin/manifest.json plugin/Model.js
    bash tests/release/plugin-version.test.sh
    echo "check-packaging: ok"

# install.sh (WP-044) against a local mock of the release layout (file://
# URLs, scratch HOME and prefixes, no network); shellcheck when installed.
check-install:
    bash tests/install/install.test.sh

# An ssh stub runs the remote side here with a scratch HOME and stubbed
# omarchy commands, a cargo stub builds a fake engine; no network, the real
# session and home untouched; shellcheck when installed.
# scripts/deploy-test-host.sh (WP-098) against a fake test host.
check-deploy:
    bash tests/deploy/deploy-test-host.test.sh

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
# (`bash scripts/docs-check.sh --write` regenerates them). The front pages
# (WP-046: both READMEs, docs/DEVELOPMENT.md, llms.txt): links, anchors,
# links into the public repositories, plugin/ links that survive the
# subtree split, and their `seldon …` lines.
docs-check:
    bash scripts/docs-check.sh

# The dev-host guard hook (WP-130): the expectation table, every mutant of
# the mutant list caught by it, shellcheck when installed.
check-guard:
    #!/usr/bin/env bash
    set -euo pipefail
    bash -n scripts/guard.sh scripts/guard-test.sh
    python3 -c 'import ast, sys; [ast.parse(open(f).read(), f) for f in sys.argv[1:]]' scripts/guard.py scripts/guard-mutants.py scripts/guard-table.py
    if command -v shellcheck >/dev/null; then shellcheck scripts/guard.sh scripts/guard-test.sh; fi
    GUARD_TEST_QUIET=1 bash scripts/guard-test.sh
    python3 scripts/guard-mutants.py

# No test or script reaches the session's runtime dir (WP-161): every
# Quickshell a test starts sets its own XDG_RUNTIME_DIR; the guard's
# mutants prove it catches the old pattern; shellcheck when installed.
check-runtime-dir:
    #!/usr/bin/env bash
    set -euo pipefail
    bash -n tests/plugin/runtime-dir.test.sh
    if command -v shellcheck >/dev/null; then shellcheck tests/plugin/runtime-dir.test.sh; fi
    bash tests/plugin/runtime-dir.test.sh

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
    files=(plugin/*.qml plugin/components/*.qml plugin/components/overlay/*.qml plugin/components/desk/*.qml plugin/components/graph/*.qml plugin/sections/*.qml)
    "$lint" --max-warnings 0 --missing-property info --uncreatable-type info \
      -I "$root" -I "$shell_dir" "${files[@]}"
    # The demoted missing-property makes qmllint blind to token typos
    # (Style.font.bodySmal); check every Style/Color/Border/Util reference
    # against the shell's Commons singletons instead.
    python3 tests/plugin/check-tokens.py "$shell_dir" "${files[@]}"
    echo "qmllint: ok (${#files[@]} files)"

# Plugin logic: Model.js under node; the banners' terminal scripts under bash with stubs; Service.qml states, the desk (Desk.qml: width, layout, keys, settings writes, notices, IPC), the pill (BarWidget.qml) and an IPC exit with two pills in a private headless Quickshell (host only).
# The Quickshell harnesses run only when plugin/, tests/plugin/, schema/,
# fixtures/ or this justfile changed against the merge base with main, and
# always on main itself (HEAD is the merge base) or with SELDON_FULL_CHECK=1
# (gates set it); see docs/TESTING.md.
plugin-test: check-runtime-space
    #!/usr/bin/env bash
    set -euo pipefail
    if [[ -n "{{ skip_host }}" ]]; then
      echo "plugin-test: skipped (SELDON_SKIP_HOST_CHECKS set; needs node, quickshell, jq, python3 and the installed shell)"
      exit 0
    fi
    command -v node >/dev/null || { echo "plugin-test: node not found" >&2; exit 1; }
    node tests/plugin/model.test.js
    node tests/plugin/model.bench.js
    bash tests/plugin/terminal-scripts.sh
    bash tests/plugin/real-home-guard.test.sh
    # Operator decision E29 (WP-161): less load on the dev host. Without git
    # or a merge base the harnesses run; at the merge base itself (main, a
    # detached main, a fresh branch with no commit yet) they run too, so
    # the main check never skips them.
    paths=(plugin tests/plugin schema fixtures justfile)
    if [[ ${SELDON_FULL_CHECK:-} != 1 ]] \
      && base=$(git merge-base HEAD main 2>/dev/null || git merge-base HEAD origin/main 2>/dev/null) \
      && [[ $base != "$(git rev-parse HEAD)" ]] \
      && changed=$(git diff --name-only "$base" -- "${paths[@]}") \
      && untracked=$(git ls-files --others --exclude-standard -- "${paths[@]}") \
      && [[ -z $changed$untracked ]]; then
      echo "plugin-test: Quickshell harnesses skipped (nothing under ${paths[*]} changed against ${base:0:12}, the merge base with main; SELDON_FULL_CHECK=1 runs them; deploy-test-host refuses this log)"
    else
      bash tests/plugin/service-states.sh
      bash tests/plugin/desk-view.sh
      bash tests/plugin/bar-view.sh
      bash tests/plugin/ipc-restart.sh
    fi
    echo "plugin-test: ok"

# Not part of `check` (it needs a release compile); CI runs it as its own step.
# Index build on the fixture logbook scaled x10, release; fails over 100 ms
# (x150 is printed; `check-perf` asserts it).
bench:
    cargo bench --manifest-path engine/Cargo.toml --locked --bench index

# Static release binary (needs `rustup target add x86_64-unknown-linux-musl`).
build-release:
    cargo build --manifest-path engine/Cargo.toml --locked --release --target {{ musl_target }}
    @echo "built engine/target/{{ musl_target }}/release/seldon"

# Regenerate fixtures from the engine (stub until the engine can build an index).
fixtures-refresh:
    @echo "fixtures-refresh: not implemented yet (needs \`seldon index\`; see SPEC-ENGINE §10)"

# Run by the orchestrator after a green main check and the push. Host from
# SELDON_TEST_HOST, listed in scripts/guard-hosts.local. See docs/TESTING.md,
# "Test host follows main".
#   just deploy-test-host <main check log>       deploy main
#   just deploy-test-host --dry-run <log>        show what it would do
#   just deploy-test-host --release vX.Y.Z       back to a release
# Put the main build of engine and plugin on the test host (WP-098).
deploy-test-host *args:
    bash scripts/deploy-test-host.sh {{ args }}

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
