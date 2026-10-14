#!/usr/bin/env bash
# packaging/omarchy-tree.sh (WP-191, ADR-0055): the sparse checkout of the
# pinned commit has what CI's QML checks read and nothing else, it is the
# pinned commit, and a tree that is not self-contained, misses a harness
# theme or the text-format scan, a commit the origin does not have, a
# DEST that exists, a non-HTTPS URL and a malformed pin are refused; the
# digest of a checkout equals the digest of the same files installed.
# Offline: scratch origins served as file:// URLs. Runs in
# `just check-packaging`.
#
# Usage: bash tests/release/omarchy-tree.test.sh
# shellcheck disable=SC2317  # the tree mutations below are called by name
set -euo pipefail

root=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
script=$root/packaging/omarchy-tree.sh
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
fails=0

pass() { echo "ok   $1"; }
fail() { echo "FAIL $1" >&2; fails=$((fails + 1)); }

# git without the user's or the system's configuration
export GIT_CONFIG_NOSYSTEM=1 GIT_CONFIG_GLOBAL=/dev/null
export GIT_AUTHOR_NAME=test GIT_AUTHOR_EMAIL=test@example.invalid
export GIT_COMMITTER_NAME=test GIT_COMMITTER_EMAIL=test@example.invalid

# origin NAME [MUTATION]: a scratch omacom/omarchy at $tmp/git/NAME/omacom/omarchy.git
# with a small tree like the real one, MUTATION applied (a function name
# or empty); prints the commit
origin() {
  local name=$1 mutation=${2:-} work=$tmp/src-$1 bare=$tmp/git/$1/omacom/omarchy.git
  mkdir -p "$work/shell/Commons" "$work/shell/Ui" "$work/shell/plugins/clock" \
    "$work/default/pacman" "$work/test/shell.d" "$work/bin" "$(dirname "$bare")"
  printf 'module qs.Commons\nsingleton Style 1.0 Style.qml\nGeometry 1.0 Geometry.js\n' > "$work/shell/Commons/qmldir"
  printf 'pragma Singleton\nimport QtQuick\nimport Quickshell\nQtObject {}\n' > "$work/shell/Commons/Style.qml"
  printf '.pragma library\nfunction f() { return 1 }\n' > "$work/shell/Commons/Geometry.js"
  printf 'module qs.Ui\nButton 1.0 Button.qml\n' > "$work/shell/Ui/qmldir"
  printf 'import QtQuick\nimport QtQuick.Controls as QQC\nimport Quickshell.Io\nimport qs.Commons\nimport "../Commons/Geometry.js" as G\nItem {}\n' > "$work/shell/Ui/Button.qml"
  printf 'import QtQuick\nimport qs.Services\nItem {}\n' > "$work/shell/plugins/clock/Main.qml"
  printf 'import QtQuick\nItem {}\n' > "$work/shell/shell.qml"
  for theme in tokyo-night catppuccin-latte osaka-jade rose-pine; do
    mkdir -p "$work/themes/$theme/backgrounds"
    printf 'accent = "#%s"\n' "$theme" > "$work/themes/$theme/colors.toml"
    printf 'not a picture\n' > "$work/themes/$theme/backgrounds/1.png"
  done
  # shellcheck disable=SC2016  # pacman's variables, literally
  printf 'Server = https://rc-mirror.example.invalid/$repo/os/$arch\n' > "$work/default/pacman/mirrorlist-rc"
  printf '[options]\n' > "$work/default/pacman/pacman-rc.conf"
  printf 'print("scan")\n' > "$work/test/shell.d/qml-text-format-scan.py"
  printf 'echo other\n' > "$work/test/shell.d/other-test.sh"
  printf 'echo validator\n' > "$work/bin/omarchy-plugin-validate"
  [[ -z $mutation ]] || (cd "$work" && "$mutation")
  git -C "$work" init -q
  git -C "$work" add -A
  git -C "$work" commit -q -m tree
  git clone -q --bare "$work" "$bare"
  git -C "$bare" config uploadpack.allowFilter true
  git -C "$bare" config uploadpack.allowAnySHA1InWant true
  git -C "$work" rev-parse HEAD
}

# pin FILE COMMIT: a pin file naming COMMIT (the validator hash is not read here)
pin() {
  printf '# test pin\nrepo=omacom/omarchy\ncommit=%s\nvalidator_sha256=%064d\n' "$2" 0 > "$1"
}

# run NAME COMMIT DEST [ENV...]: the script against origin NAME; sets rc,
# leaves stdout and stderr in $tmp/out and $tmp/err
run() {
  local name=$1 commit=$2 dest=$3
  shift 3
  pin "$tmp/pin-$name" "$commit"
  rc=0
  env SELDON_OMARCHY_PIN="$tmp/pin-$name" SELDON_OMARCHY_GIT="file://$tmp/git/$name" "$@" \
    bash "$script" "$dest" > "$tmp/out" 2> "$tmp/err" || rc=$?
}

# refused NAME MUTATION PATTERN: a tree with MUTATION is refused, and the
# message matches PATTERN
refused() {
  local name=$1 mutation=$2 pattern=$3 commit
  commit=$(origin "$name" "$mutation")
  run "$name" "$commit" "$tmp/dest-$name"
  if ((rc == 1)) && grep -q -- "$pattern" "$tmp/err"; then
    pass "$name: refused ($pattern)"
  else
    fail "$name: exit $rc, wanted 1 with '$pattern'"
    cat "$tmp/out" "$tmp/err" >&2
  fi
}

# --- the real pin is readable ---
if bash "$root/packaging/omarchy-validate.sh" --print-pin > /dev/null; then
  pass "packaging/omarchy-pin is readable"
else
  fail "packaging/omarchy-pin is not readable"
fi

# --- a good tree ---
good=$(origin good)
dest=$tmp/dest-good
run good "$good" "$dest"
if ((rc == 0)) && grep -q "omarchy-tree: omacom/omarchy@$good at $dest" "$tmp/out"; then
  pass "good: exit 0, names the commit"
else
  fail "good: exit $rc"
  cat "$tmp/out" "$tmp/err" >&2
fi
if [[ $(git -C "$dest" rev-parse HEAD) == "$good" ]]; then pass "good: HEAD is the pinned commit"; else fail "good: HEAD is not the pinned commit"; fi
missing=()
for f in shell/Commons/qmldir shell/Ui/Button.qml shell/plugins/clock/Main.qml shell/shell.qml \
  themes/tokyo-night/colors.toml themes/rose-pine/colors.toml default/pacman/mirrorlist-rc \
  test/shell.d/qml-text-format-scan.py; do
  [[ -f $dest/$f ]] || missing+=("$f")
done
if ((${#missing[@]} == 0)); then
  pass "good: shell/, the colours, the mirrorlists and the scan are there"
else
  fail "good: missing ${missing[*]}"
fi
extra=()
for f in themes/tokyo-night/backgrounds/1.png default/pacman/pacman-rc.conf test/shell.d/other-test.sh bin/omarchy-plugin-validate; do
  [[ ! -e $dest/$f ]] || extra+=("$f")
done
if ((${#extra[@]} == 0)); then
  pass "good: nothing else is checked out (backgrounds, other tests, bin/)"
else
  fail "good: also checked out ${extra[*]}"
fi
# the same files installed elsewhere give the same digest
installed=$tmp/installed
mkdir -p "$installed/themes"
cp -r "$dest/shell" "$installed/"
for theme in "$dest"/themes/*; do
  mkdir -p "$installed/themes/$(basename "$theme")/backgrounds"
  cp "$theme/colors.toml" "$installed/themes/$(basename "$theme")/"
  echo picture > "$installed/themes/$(basename "$theme")/backgrounds/2.png"
done
want=$(sed -n 's/^omarchy-tree: digest //p' "$tmp/out")
got=$(bash "$script" --digest "$installed" | cut -d' ' -f1)
if [[ $want =~ ^[0-9a-f]{64}$ && $got == "$want" ]]; then
  pass "--digest of the installed copy equals the checkout's digest"
else
  fail "--digest: checkout '$want', installed '$got'"
fi
echo 'accent = "#000000"' > "$installed/themes/rose-pine/colors.toml"
got=$(bash "$script" --digest "$installed" | cut -d' ' -f1)
if [[ $got =~ ^[0-9a-f]{64}$ && $got != "$want" ]]; then pass "--digest changes with a theme's colors.toml"; else fail "--digest ignores a changed colors.toml"; fi
cp "$dest/themes/rose-pine/colors.toml" "$installed/themes/rose-pine/colors.toml"
echo changed >> "$installed/shell/Ui/Button.qml"
got=$(bash "$script" --digest "$installed" | cut -d' ' -f1)
if [[ $got != "$want" ]]; then pass "--digest changes with a shell file"; else fail "--digest ignores a changed shell file"; fi

# --- refusals ---
run good "$good" "$dest"
if ((rc == 1)) && grep -q "exists" "$tmp/err"; then pass "an existing DEST is refused"; else fail "an existing DEST: exit $rc"; fi

run good 0123456789abcdef0123456789abcdef01234567 "$tmp/dest-unknown"
if ((rc == 1)) && grep -q "cannot fetch" "$tmp/err"; then pass "a commit the origin does not have is refused"; else fail "an unknown commit: exit $rc"; fi

pin "$tmp/pin-http" "$good"
rc=0
SELDON_OMARCHY_PIN="$tmp/pin-http" SELDON_OMARCHY_GIT="http://127.0.0.1:9" bash "$script" "$tmp/dest-http" > "$tmp/out" 2> "$tmp/err" || rc=$?
if ((rc == 1)) && grep -q "cannot fetch" "$tmp/err" && grep -q "transport 'http' not allowed" "$tmp/err"; then
  pass "a plain http:// URL is refused by git's protocol list"
else
  fail "http://: exit $rc"
  cat "$tmp/err" >&2
fi

printf 'repo=omacom/omarchy\ncommit=%s\n' "$good" > "$tmp/pin-bad"
rc=0
SELDON_OMARCHY_PIN="$tmp/pin-bad" SELDON_OMARCHY_GIT="file://$tmp/git/good" bash "$script" "$tmp/dest-bad" > "$tmp/out" 2> "$tmp/err" || rc=$?
if ((rc == 1)) && grep -q "cannot read the pin" "$tmp/err" && [[ ! -e $tmp/dest-bad ]]; then
  pass "a malformed pin is refused before anything is fetched"
else
  fail "a malformed pin: exit $rc"
fi

services() { printf 'import QtQuick\nimport qs.Services\nItem {}\n' > shell/Ui/Panel.qml; }
outside() { printf 'import QtQuick\nimport "../plugins/clock/Main.qml" as M\nItem {}\n' > shell/Ui/Panel.qml; }
gone() { printf 'import QtQuick\nimport "../Commons/Gone.js" as G\nItem {}\n' > shell/Ui/Panel.qml; }
js_import() { printf '.import "../../bin/omarchy-plugin-validate" as V\n' > shell/Commons/Helper.js; }
qmldir_missing() { printf 'Toggle 1.0 Toggle.qml\n' >> shell/Ui/qmldir; }
no_qmldir() { rm shell/Commons/qmldir; }
no_theme() { rm themes/osaka-jade/colors.toml; }
no_scan() { rm test/shell.d/qml-text-format-scan.py; }
refused services services "shell/Ui/Panel.qml:2 imports outside the tree: import qs.Services"
refused outside outside "imports ../plugins/clock/Main.qml, outside shell/Commons and shell/Ui"
refused gone gone "imports ../Commons/Gone.js, which is missing"
refused js-import js_import "shell/Commons/Helper.js:1 imports ../../bin/omarchy-plugin-validate"
refused qmldir-missing qmldir_missing "shell/Ui/qmldir names Toggle.qml, which is missing"
refused no-qmldir no_qmldir "shell/Commons/qmldir is missing"
refused no-theme no_theme "themes/osaka-jade/colors.toml is missing"
refused no-scan no_scan "qml-text-format-scan.py is missing"

if ((fails > 0)); then
  echo "omarchy-tree.test: $fails failure(s)" >&2
  exit 1
fi
echo "omarchy-tree.test: ok"
