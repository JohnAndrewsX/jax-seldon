#!/usr/bin/env bash
# The seldon package's static parts (WP-230), without makepkg: the recipe's
# names and dependencies, the desktop entry, the icon, the install script,
# the omarchy-pkgs recipe draft, the app-era gate and the workflows' use of
# it. Runs in `just check-packaging`; the real build is the release
# workflow's and `bin/install`'s.
#
# Usage: bash tests/release/seldon-package.test.sh
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
check "provides jax-seldon" grep -qx 'provides=jax-seldon' <<<"$r"
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
if command -v xmllint >/dev/null; then
  check "icon is well-formed XML" xmllint --noout "$pkg/seldon.svg"
fi

# --- the install script: cache refreshes only -----------------------------
i=$pkg/seldon.install
check "install: bash syntax" bash -n "$i"
absent "install: does not run seldon" grep -Eq '(^|[;&|[:space:]])(/usr/bin/)?(seldon|jax-seldon)[[:space:]]' "$i"
absent "install: no sudo, systemctl or pacman" grep -Eq '(sudo|systemctl|pacman|rm |chmod|chown)' "$i"
check "install: refreshes the desktop database" grep -q 'update-desktop-database' "$i"
check "install: refreshes the icon cache" grep -q 'gtk-update-icon-cache' "$i"

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
check "release.yml: jax-seldon build is not gated" grep -q 'Build the package$' "$rel"
check "release.yml: app-era output" grep -q 'app_era' "$rel"
# every step that builds or checks the seldon package is gated on the output
gated=$(grep -c "if: steps.era.outputs.app_era == 'true'" "$rel")
check "release.yml: gated steps present" test "$gated" -ge 1
for f in "$rel" "$ci"; do
  for q in qt6-base qt6-declarative qt6-svg qt6-wayland; do
    check "$(basename "$f") installs $q" grep -Eq "pacman -Syu.* $q( |\$)" "$f"
  done
done

exit "$fail"
