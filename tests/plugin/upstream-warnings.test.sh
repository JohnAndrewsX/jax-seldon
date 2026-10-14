#!/usr/bin/env bash
# Self-test of tests/plugin/upstream-warnings.sh (WP-224), on scratch trees
# (fixture): the harnesses' log check accepts exactly Quickshell's
# "Unable to find hyprland socket" line, and only while the shell's
# Commons/Style.qml is its sole importer of Quickshell.Hyprland; a plugin
# file's warning, any other WARN and a stable tree still fail. Then every
# harness pipes its log check through the helper. Needs no Quickshell.
set -euo pipefail

root=$(cd "$(dirname "$0")/../.." && pwd)
# shellcheck source=tests/plugin/upstream-warnings.sh
source "$root/tests/plugin/upstream-warnings.sh"
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

pass=0
fail=0
ok() { pass=$((pass + 1)); echo "ok   $1"; }
bad() { fail=$((fail + 1)); echo "FAIL $1"; }

hypr_line='WARN: Unable to find hyprland socket. Cannot connect to hyprland.'
# bad_lines SHELL PLUGIN NOT_LOADED LOG: the WARN lines a harness would
# report, by the harnesses' own pipeline (colour codes stripped, then the
# upstream filter); NOT_LOADED is a space-separated list
bad_lines() {
  local re
  # shellcheck disable=SC2086 # the list splits on purpose
  re=$(upstream_allowed "$1" "$2" $3)
  sed 's/\x1b\[[0-9;]*m//g' "$4" | grep -a -E "ERROR|WARN|TypeError" | grep -a -v -E "$re" || true
}
# expect NAME SHELL PLUGIN NOT_LOADED WANT(clean|fails) LINE...
expect() {
  local name=$1 shell=$2 plugin=$3 skip=$4 want=$5 got
  shift 5
  printf '%s\n' "$@" >"$work/log"
  if [[ -z $(bad_lines "$shell" "$plugin" "$skip" "$work/log") ]]; then got=clean; else got=fails; fi
  if [[ $got == "$want" ]]; then ok "$name: $got"; else bad "$name: $got (want $want)"; fi
}

edge=$work/edge stable=$work/stable plugin=$work/plugin hyprplugin=$work/hyprplugin
mkdir -p "$edge/Commons" "$stable/Commons" "$plugin/components" "$hyprplugin"
printf 'pragma Singleton\nimport QtQuick\nimport qs.Commons as Commons\nimport Quickshell\nimport Quickshell.Hyprland\nimport Quickshell.Io\n' >"$edge/Commons/Style.qml"
printf 'pragma Singleton\nimport QtQuick\nimport Quickshell\nimport Quickshell.Io\n// Hyprland owns the gaps; import Quickshell.Hyprland is not needed here\n' >"$stable/Commons/Style.qml"
printf 'import QtQuick\nimport qs.Commons\nItem {}\n' >"$plugin/components/Tone.qml"
printf 'import QtQuick\nimport Quickshell.Hyprland\nItem {}\n' >"$hyprplugin/Pill.qml"

expect "edge: the upstream line alone" "$edge" "$plugin" "" clean "$hypr_line"
expect "edge: the line with colour codes and leading space" "$edge" "$plugin" "" clean $'  \e[33mWARN\e[0m: Unable to find hyprland socket. Cannot connect to hyprland.'
expect "edge: a plugin file's warning with the same words still fails" "$edge" "$plugin" "" fails \
  "$hypr_line" "WARN scene: file://$plugin/components/Tone.qml[13:-1]: Unable to find hyprland socket. Cannot connect to hyprland."
expect "edge: a plugin file's TypeError still fails" "$edge" "$plugin" "" fails \
  "$hypr_line" "WARN scene: file://$plugin/components/Tone.qml[33:-1]: TypeError: Cannot read property 'divider' of undefined"
expect "edge: another WARN still fails" "$edge" "$plugin" "" fails "$hypr_line" "WARN: Unable to find sway socket."
expect "edge: the line with more after it fails" "$edge" "$plugin" "" fails "$hypr_line (retrying)"
expect "edge: half the line fails" "$edge" "$plugin" "" fails "WARN: Unable to find hyprland socket."
expect "edge: an ERROR still fails" "$edge" "$plugin" "" fails "$hypr_line" "ERROR: something broke"
expect "stable (no import in Style.qml, a comment does not count): the line fails" "$stable" "$plugin" "" fails "$hypr_line"
expect "a plugin file imports Quickshell.Hyprland: the line fails" "$edge" "$hyprplugin" "" fails "$hypr_line"
expect "no plugin dir: the line fails" "$edge" "$work/none" "" fails "$hypr_line"
expect "no shell tree: the line fails" "$work/none" "$plugin" "" fails "$hypr_line"
mkdir -p "$hyprplugin/components/desk"
printf 'import QtQuick\nimport Quickshell.Hyprland\nItem {}\n' >"$hyprplugin/components/desk/DeskWindow.qml"
expect "the plugin's importers all named as not loaded: the line passes" "$edge" "$hyprplugin" "Pill.qml components/desk/DeskWindow.qml" clean "$hypr_line"
expect "one importer not named: the line fails" "$edge" "$hyprplugin" "components/desk/DeskWindow.qml" fails "$hypr_line"
expect "a named file's own warning still fails" "$edge" "$hyprplugin" "Pill.qml components/desk/DeskWindow.qml" fails \
  "$hypr_line" "WARN scene: file://$hyprplugin/components/desk/DeskWindow.qml[32:-1]: TypeError: Cannot read property 'name' of null"
expect "a name is matched whole, not as a suffix" "$edge" "$hyprplugin" "Pill.qml DeskWindow.qml" fails "$hypr_line"
expect "a clean log stays clean" "$stable" "$plugin" "" clean "INFO: loaded"

# the real plugin: its only importer of Quickshell.Hyprland is DeskWindow.qml,
# which only Desk.qml creates; the harnesses that name it as not loaded
# never load Desk.qml (desk-view loads Desk.qml with a stub DeskWindow)
importers=$(cd "$root/plugin" && grep -rlE '^[[:space:]]*import[[:space:]]+Quickshell\.Hyprland' --include='*.qml' --include='*.js' . | sed 's#^\./##' | sort | tr '\n' ' ')
if [[ $importers == "components/desk/DeskWindow.qml " ]]; then
  ok "plugin/: DeskWindow.qml is the only importer of Quickshell.Hyprland"
else
  bad "plugin/: importers of Quickshell.Hyprland are '$importers' (update the harnesses' not-loaded lists)"
fi
creators=$(grep -rlE '^[[:space:]]*DeskWindow[[:space:]]*\{' --include='*.qml' "$root/plugin" | sed "s#^$root/##" | tr '\n' ' ')
if [[ $creators == "plugin/Desk.qml " ]]; then ok "only Desk.qml creates a DeskWindow"; else bad "DeskWindow created in '$creators'"; fi
for h in bar-view ipc-restart service-states; do
  f="$root/tests/plugin/$h.sh"
  if grep -q 'upstream_allowed .* components/desk/DeskWindow.qml)' "$f" && ! grep -qE '"Desk\.qml"|/Desk\.qml|load\("Desk\.qml' "$f"; then
    ok "$h.sh names DeskWindow.qml as not loaded and loads no Desk.qml"
  else
    bad "$h.sh: its not-loaded list and what it loads disagree"
  fi
done
if grep -q 'tests/plugin/harness/DeskWindow.qml" "$plugin/components/desk/DeskWindow.qml"' "$root/tests/plugin/desk-view.sh" \
  && ! grep -q 'Quickshell.Hyprland' "$root/tests/plugin/harness/DeskWindow.qml"; then
  ok "desk-view.sh replaces DeskWindow.qml by a stub without Quickshell.Hyprland"
else
  bad "desk-view.sh loads a DeskWindow.qml that may ask Hyprland"
fi

# every harness checks its logs through the helper
# shellcheck disable=SC2016 # the harnesses' own text, not expansions
for h in desk-view bar-view ipc-restart service-states; do
  f="$root/tests/plugin/$h.sh"
  if grep -q 'source "$root/tests/plugin/upstream-warnings.sh"' "$f" \
    && grep -q 'upstream_re=$(upstream_allowed ' "$f" \
    && grep -q 'grep -a -v -E "$upstream_re"' "$f"; then
    ok "$h.sh filters its log through upstream_allowed"
  else
    bad "$h.sh does not filter its log through upstream_allowed"
  fi
done

echo "upstream-warnings.test: $pass passed, $fail failed"
((fail == 0))
