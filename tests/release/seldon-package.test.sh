#!/usr/bin/env bash
# The seldon package's static parts (WP-230), without makepkg: the recipe's
# names and dependencies, the desktop entry, the icon, the install script,
# the omarchy-pkgs recipe draft, the app-era gate and the workflows' use of
# it. Runs in `just check-packaging`; the real build is the release
# workflow's and `bin/install`'s.
#
# Usage: bash tests/release/seldon-package.test.sh
# Single quotes are meant in this file: literal $ in patterns and the
# scripts handed to bash -c.
# shellcheck disable=SC2016
set -euo pipefail

root=$(cd "$(dirname "$0")/../.." && pwd)
pkg=$root/packaging/seldon
fail=0
check() { # NAME COMMAND...
  local name=$1; shift
  if "$@" >/dev/null 2>&1; then echo "ok   $name"; else echo "FAIL $name"; fail=1; fi
}
absent() { # NAME COMMAND...: the command must fail
  local name=$1; shift
  if "$@" >/dev/null 2>&1; then echo "FAIL $name"; fail=1; else echo "ok   $name"; fi
}

# --- the recipe ---------------------------------------------------------
# The names come from the sourced PKGBUILD.
# shellcheck disable=SC2154
recipe() ( # prints "key=value" lines of the arrays and names
  # shellcheck disable=SC1091
  source "$pkg/PKGBUILD"
  echo "pkgname=$pkgname"
  echo "install=$install"
  printf 'provides=%s\n' "${provides[*]}"
  printf 'conflicts=%s\n' "${conflicts[*]}"
  printf 'replaces=%s\n' "${replaces[*]}"
  printf 'depends=%s\n' "${depends[*]}"
  printf 'makedepends=%s\n' "${makedepends[*]}"
  printf 'options=%s\n' "${options[*]}"
)
r=$(recipe)
check "pkgname is seldon" grep -qx 'pkgname=seldon' <<<"$r"
check "install script is seldon.install" grep -qx 'install=seldon.install' <<<"$r"
check "provides jax-seldon, versioned" grep -Eqx 'provides=jax-seldon=[0-9]+\.[0-9]+\.[0-9]+' <<<"$r"
check "source is the attested release asset" grep -Fq 'releases/download/v$pkgver/jax-seldon-$pkgver.tar.gz")' "$pkg/PKGBUILD"
absent "source is not GitHub's generated archive" grep -q 'archive/refs' "$pkg/PKGBUILD"
check "conflicts jax-seldon" grep -qx 'conflicts=jax-seldon' <<<"$r"
check "replaces jax-seldon" grep -qx 'replaces=jax-seldon' <<<"$r"
for dep in qt6-base qt6-declarative qt6-svg qt6-wayland hicolor-icon-theme; do
  check "depends on $dep" grep -Eq "^depends=.*(^| |=)$dep(>=[0-9.]+)?( |\$)" <<<"$r"
done
check "Qt 6.9 floor on qt6-base" grep -q 'qt6-base>=6.9' <<<"$r"
check "Qt 6.9 floor on qt6-declarative" grep -q 'qt6-declarative>=6.9' <<<"$r"
check "makedepends cargo" grep -Eq '^makedepends=.*cargo' <<<"$r"
check "no debug package" grep -q '^options=.*!debug' <<<"$r"
check "desk binary in /usr/lib/seldon" grep -q '"\$pkgdir/usr/lib/seldon/seldon-desk"' "$pkg/PKGBUILD"
check "plugin copy in /usr/share/seldon/plugin" grep -q '"\$pkgdir/usr/share/seldon/plugin/' "$pkg/PKGBUILD"
check "app LICENSE (templates' MIT notice) installed" grep -Fq 'install -Dm644 app/LICENSE "$pkgdir/usr/share/licenses/$pkgname/LICENSE.app"' "$pkg/PKGBUILD"
check "LICENSE installed" grep -q 'usr/share/licenses/\$pkgname/LICENSE' "$pkg/PKGBUILD"
absent "no sudo or pacman in the recipe" grep -Eq '(^|[^a-z-])(sudo|pacman) ' "$pkg/PKGBUILD"

# --- desktop entry and icon ----------------------------------------------
d=$pkg/seldon.desktop
check "desktop: Exec=seldon open" grep -qx 'Exec=seldon open' "$d"
check "desktop: StartupWMClass=seldon" grep -qx 'StartupWMClass=seldon' "$d"
check "desktop: Icon=seldon" grep -qx 'Icon=seldon' "$d"
check "desktop: Name[de]" grep -q '^Name\[de\]=' "$d"
check "desktop: Comment[de]" grep -q '^Comment\[de\]=' "$d"
check "desktop: Terminal=false" grep -qx 'Terminal=false' "$d"
if command -v desktop-file-validate >/dev/null; then
  check "desktop-file-validate" desktop-file-validate "$d"
fi
check "icon is an SVG" grep -q '<svg ' "$pkg/seldon.svg"
absent "icon has no script" grep -qi '<script' "$pkg/seldon.svg"
absent "icon has no external reference" grep -Eqi 'href=|xlink:|url\(http' "$pkg/seldon.svg"
absent "icon has no event handler" grep -Eqi '[[:space:]]on[a-z]+[[:space:]]*=' "$pkg/seldon.svg"
if command -v xmllint >/dev/null; then
  check "icon is well-formed XML" xmllint --noout "$pkg/seldon.svg"
fi

# --- the install script: cache refreshes only -----------------------------
i=$pkg/seldon.install
check "install: bash syntax" bash -n "$i"
# the whole file, not a denylist: omacom's cache refreshes, ending in `:` so a
# missing tool never fails the transaction
expected_install='post_install() {
  command -v update-desktop-database >/dev/null 2>&1 && update-desktop-database -q
  command -v gtk-update-icon-cache >/dev/null 2>&1 && gtk-update-icon-cache -q -t -f usr/share/icons/hicolor
  # a missing tool never fails the transaction
  :
}

post_upgrade() {
  post_install
}

post_remove() {
  post_install
}'
check "install: is exactly the fixed text" test "$(cat "$i")" = "$expected_install"
# with only one tool on PATH the scriptlet still succeeds
check "install: post_install succeeds without its tools" bash -c 'source "$1"; PATH=/nonexistent post_install' _ "$i"

# --- hex colours live in the icon file only -------------------------------
absent "no hex colour outside the icon" grep -Eq '#[0-9a-fA-F]{6}\b' \
  "$pkg/PKGBUILD" "$d" "$i" "$pkg/expected-files.txt" "$root/bin/build" "$root/bin/test" "$root/bin/install"

# --- the omarchy-pkgs recipe draft ---------------------------------------
j=$root/packaging/opr/.omarchy/package.json
check "opr: source local" jq -e '.source == "local"' "$j"
check "opr: auto_merge" jq -e '.auto_merge == true' "$j"
check "opr: upstream.watch github" jq -e '.upstream.watch.github == "JohnAndrewsX/jax-seldon"' "$j"
check "opr: upstream.watch pattern has a version group" jq -e '.upstream.watch.pattern | test("\\(\\?P<version>")' "$j"

# --- the expected file list ------------------------------------------------
e=$pkg/expected-files.txt
check "expected-files: sorted" bash -c 'LC_ALL=C sort -c "$1"' _ "$e"
for f in usr/bin/seldon usr/lib/seldon/seldon-desk usr/share/applications/seldon.desktop \
         usr/share/icons/hicolor/scalable/apps/seldon.svg usr/share/licenses/seldon/LICENSE \
         usr/share/seldon/plugin/manifest.json; do
  check "expected-files lists $f" grep -qx "$f" "$e"
done
check "expected-files: plugin copy equals plugin/" diff \
  <(grep '^usr/share/seldon/plugin/.' "$e" | grep -v '/$' | sed 's|^usr/share/seldon/plugin/||' | LC_ALL=C sort) \
  <(cd "$root/plugin" && find . -type f -printf '%P\n' | LC_ALL=C sort)

# --- the app-era gate -------------------------------------------------------
gate=$root/packaging/app-era.sh
for v in 0.4.0 0.4.1 0.4.99; do absent "gate: $v is not app-era" bash "$gate" "$v"; done
for v in 0.5.0 0.5.1 0.10.0 1.0.0; do check "gate: $v is app-era" bash "$gate" "$v"; done
absent "gate: refuses a bad version" bash "$gate" 0.5
absent "gate: refuses a tag" bash "$gate" v0.5.0

# --- the workflows ---------------------------------------------------------
rel=$root/.github/workflows/release.yml
ci=$root/.github/workflows/ci.yml
step() { # FILE NAME: the step's block (from its "- name:" line to the next step)
  awk -v n="      - name: $2" '
    $0 == n { on = 1; print; next }
    on && /^      - / { exit }
    on && /^  [a-z]/ { exit }
    on { print }' "$1"
}
for name in "Install toolchain" "Build the package"; do
  b=$(step "$rel" "$name")
  check "release.yml: '$name' exists" test -n "$b"
  absent "release.yml: '$name' has no if:" grep -q '^        if:' <<<"$b"
done
absent "release.yml: the shared toolchain has no Qt or hicolor" grep -Eq 'qt6|hicolor' <<<"$(step "$rel" "Install toolchain")"
check "release.yml: App era skips 0.4.x" grep -Fq "if: \${{ !startsWith(steps.version.outputs.version, '0.4.') }}" <<<"$(step "$rel" "App era")"
for name in "Install the seldon package toolchain" "Build the seldon package"; do
  check "release.yml: '$name' is gated on the app era" grep -Fqx "        if: steps.era.outputs.app_era == 'true'" <<<"$(step "$rel" "$name")"
done
check "release.yml: the seldon toolchain has the Qt packages" grep -Eq 'qt6-base.*qt6-declarative.*qt6-svg.*qt6-wayland' <<<"$(step "$rel" "Install the seldon package toolchain")"
# nothing after the first app-era step runs unconditionally from the seldon recipe
absent "release.yml: the jax-seldon build step does not touch packaging/seldon" \
  bash -c 'grep -v "^[[:space:]]*#" | grep -q "packaging/seldon"' <<<"$(step "$rel" "Build the package")"
check "ci.yml: seldon-package job runs makepkg and diffs the list" grep -Fq 'diff -u packaging/seldon/expected-files.txt' "$ci"
check "ci.yml: seldon-package job installs the Qt packages" grep -Eq 'pacman -Syu.*qt6-base.*qt6-declarative.*qt6-svg.*qt6-wayland' "$ci"
absent "ci.yml: the check job's toolchain has no Qt" grep -Eq 'pacman -Syu.* nodejs qt6' "$ci"

exit "$fail"
