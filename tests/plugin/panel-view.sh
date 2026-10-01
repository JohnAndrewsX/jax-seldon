#!/usr/bin/env bash
# Drive plugin/Panel.qml through its tabs, keys and banners in a private,
# headless Quickshell (tests/plugin/harness/panel.qml).
#
# The panel imports qs.Commons and qs.Ui, which only the shell provides. This
# builds a temp config root with copies of the installed shell's Commons/ and
# Ui/ (only Ui/KeyboardPanel.qml, a layer-shell window, is replaced by
# tests/plugin/harness/KeyboardPanel.qml), so the panel runs against the real
# shell components. Keys are real key events (QtTest keyClick) through the
# shell's own PanelKeyCatcher. Nothing talks to the running omarchy-shell.
# Needs quickshell, jq and the installed shell (host check; docs/TESTING.md).
set -euo pipefail

root=$(cd "$(dirname "$0")/../.." && pwd)
plugin="$root/plugin"
fx="$root/fixtures"
shell_dir="${OMARCHY_PATH:-/usr/share/omarchy}/shell"

qs_bin=$(command -v quickshell || command -v qs || true)
[[ -n $qs_bin ]] || { echo "panel-view: quickshell not found" >&2; exit 1; }
command -v jq >/dev/null || { echo "panel-view: jq not found" >&2; exit 1; }
[[ -d $shell_dir/Commons && -d $shell_dir/Ui ]] || { echo "panel-view: shell not found at $shell_dir" >&2; exit 1; }
timeout_bin=$(command -v timeout) || { echo "panel-view: timeout not found" >&2; exit 1; }

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

config="$work/config"
mkdir -p "$config/Commons" "$config/Ui" "$work/home" "$work/bin"
cp "$shell_dir"/Commons/* "$config/Commons/"
cp "$shell_dir"/Ui/* "$config/Ui/"
cp "$root/tests/plugin/harness/KeyboardPanel.qml" "$config/Ui/KeyboardPanel.qml"
cp "$root/tests/plugin/harness/panel.qml" "$config/shell.qml"

# Tools for the fake engine, and the fake engine; never a real seldon.
for tool in bash env cat sed date mkdir mv sleep; do
  ln -s "$(command -v "$tool")" "$work/bin/$tool"
done
install -m 755 "$root/tests/plugin/fake-seldon" "$work/bin/seldon"
# The shell's Style.qml asks Hyprland and fontconfig for gaps, rounding and
# the font; outside Hyprland it keeps its defaults when they fail.
printf '#!/bin/sh\nexit 1\n' >"$work/bin/hyprctl"
printf '#!/bin/sh\necho monospace\n' >"$work/bin/fc-match"
chmod 755 "$work/bin/hyprctl" "$work/bin/fc-match"
ln -s "$(command -v sh)" "$work/bin/sh"

pass=0
fail=0

# run <case> <index file> <steps> — one harness run; step reports land in
# $work/<case>.steps (one JSON object per line), the whole log in <case>.log.
run() {
  local name=$1 index=$2 steps=$3
  env -i HOME="$work/home" PATH="$work/bin" QT_QPA_PLATFORM=offscreen \
    XDG_RUNTIME_DIR="${XDG_RUNTIME_DIR:-$work}" \
    HARNESS_PLUGIN_DIR="$plugin" HARNESS_STEPS="$steps" SELDON_INDEX="$index" \
    "$timeout_bin" 60 "$qs_bin" -p "$config/shell.qml" >"$work/$name.log" 2>&1 || true
  sed 's/\x1b\[[0-9;]*m//g' "$work/$name.log" | grep -a "HARNESS step " | sed 's/.*HARNESS step [^ ]* //' >"$work/$name.steps" || true
}

# expect <case> <step number, 1-based> <jq filter> <value>
expect() {
  local got="<no report>"
  if [[ $(wc -l <"$work/$1.steps") -ge $2 ]]; then
    got=$(sed -n "${2}p" "$work/$1.steps" | jq -r "$3" 2>/dev/null || true)
  fi
  if [[ $got == "$4" ]]; then
    pass=$((pass + 1))
    echo "ok   $1 #$2: $3 = $4"
  else
    fail=$((fail + 1))
    echo "FAIL $1 #$2: $3 = $got (want $4)"
  fi
}

# shows <case> <step> <text> — the text is visible on screen at that step.
shows() {
  expect "$1" "$2" "[.texts[] | select(. == \"$3\")] | length > 0" true
}

clean_log() {
  local bad
  bad=$(sed 's/\x1b\[[0-9;]*m//g' "$work/$1.log" | grep -a -E "ERROR|WARN|TypeError|ReferenceError|Binding loop" \
    | grep -a -v -E "WAYLAND_DISPLAY is present|QT_QPA_PLATFORM|--- WARNING ---|most functionality will be broken" || true)
  if [[ -z $bad ]]; then
    pass=$((pass + 1))
    echo "ok   $1: log clean"
  else
    fail=$((fail + 1))
    echo "FAIL $1: log has errors"
    echo "$bad" | sed 's/^/     /'
  fi
}

# 1. The sample: every tab renders its data; the strip is on every tab.
run sample "$fx/index.sample.json" \
  "view;tab:changelog;filter:pacman;text:f;filter:all;key:Down;key:Down*29;key:Return;tab:system;key:Down*40"
expect sample 1 .view.status ok
expect sample 1 .view.tab today
expect sample 1 .view.banner ""
expect sample 1 .view.crisis "2 changes in the red zone need a reason"
shows sample 1 "2 changes in the red zone need a reason"
expect sample 1 .view.today.entries 4
expect sample 1 .view.today.yesterday 1
shows sample 1 "Thursday, 1 Oct 2026"
shows sample 1 "09:25 · claude-code · C-2026-003"
shows sample 1 "▸ Yesterday · 1 entry"
expect sample 2 .view.tab changelog
expect sample 2 .view.changelog.rows 58
expect sample 2 '.view.changelog.badges | join(",")' "firefox +3"
expect sample 2 .view.changelog.folded 7
expect sample 2 .view.changelog.snapshots 6
shows sample 2 "2 changes in the red zone need a reason"
shows sample 2 "58 events · newest first"
shows sample 2 "explained: Zeiterfassung nur zum Testen, noch nicht in der Bar."
shows sample 2 "Unexplained · proposed for C-2026-005"
expect sample 3 .view.changelog.filter pacman
expect sample 3 .view.changelog.rows 12
shows sample 3 "12 events from pacman · newest first"
expect sample 4 .view.changelog.filter snapper
expect sample 4 .view.changelog.rows 8
expect sample 5 .view.changelog.rows 58
expect sample 6 .view.cursorActive true
expect sample 7 .view.cursor 29
shows sample 7 "firefox"
shows sample 7 "+3"
shows sample 7 "Unexplained"
expect sample 8 .view.changelog.expanded 01M3SXBQVR7AW8PJQC1YXDCQ14
shows sample 8 "· upgrade libinput  1.29.1-1 → 1.29.2-1"
expect sample 9 .view.tab system
expect sample 9 '.view.system | join(",")' "OMARCHY,PACKAGES,PLUGINS,SNAPSHOTS,AREAS,COLLECTORS,SELDON"
shows sample 9 "33 of 40 enabled"
shows sample 9 "2 changes in the red zone need a reason"
expect sample 10 .view.cursor 26
clean_log sample

# 2. Keyboard: Tab / Shift-Tab, arrows, digits, Esc.
run keys "$fx/index.sample.json" \
  "key:Tab;key:Tab;key:Backtab;key:Right;key:Left;text:3;text:1;key:Backtab;key:Tab;key:Down;key:Down*2;key:Up;key:Return;key:Escape"
expect keys 1 .view.tab changelog
expect keys 2 .view.tab system
expect keys 3 .view.tab changelog
expect keys 4 .view.tab system
expect keys 5 .view.tab changelog
expect keys 6 .view.tab system
expect keys 7 .view.tab today
# Without another bar panel to hand over to, Tab wraps.
expect keys 8 .view.tab system
expect keys 9 .view.tab today
expect keys 10 .view.cursorActive true
expect keys 11 .view.cursor 2
expect keys 12 .view.cursor 1
expect keys 14 .view.opened false
clean_log keys

# 3. The yesterday row opens with Enter.
run yesterday "$fx/index.sample.json" "key:Down;key:Down*10;key:Return;key:Down"
expect yesterday 2 .view.cursor 4
expect yesterday 3 .view.today.rows 6
shows yesterday 3 "▾ Yesterday · 1 entry"
expect yesterday 4 .view.cursor 5
shows yesterday 4 "Snapshots aufgeräumt, 108 und 109 gelöscht."
clean_log yesterday

# 4. Snapper without permissions (ADR-0011): its banner on every tab.
run snapper "$fx/index-variants/snapper-degraded.json" "view;tab:changelog;tab:system"
for step in 1 2 3; do
  expect snapper $step .view.snapper "Snapshots not readable"
done
shows snapper 1 'sudo snapper -c root set-config ALLOW_USERS=$USER SYNC_ACL=yes'
shows snapper 1 "Run in terminal"
shows snapper 3 "failing · snapper: No permissions. The snapper config does not list this user in ALLOW_USERS; see \`seldon doctor\`."
clean_log snapper

# 5. Not initialised: the banner, no strip, empty tabs.
run uninit "$fx/index-variants/not-initialised.json" "view;tab:changelog;tab:system"
expect uninit 1 .view.banner "Logbook not initialised"
expect uninit 1 .view.crisis ""
shows uninit 1 "No index to show"
expect uninit 2 .view.changelog.rows 0
shows uninit 3 "No index to show"
clean_log uninit

# 6. Every system field is optional: an empty section and none at all.
jq '.system = {}' "$fx/index.sample.json" >"$work/system-empty.json"
run system-empty "$work/system-empty.json" "tab:system"
expect system-empty 1 '.view.system | join(",")' "COLLECTORS,SELDON"
clean_log system-empty
jq '.system = {packages: {aur: 3}} | del(.state.collectors) | del(.today.yesterday) | .drift = [] | .summary.crisis = 0' \
  "$fx/index.sample.json" >"$work/sparse.json"
run sparse "$work/sparse.json" "view;tab:changelog;tab:system"
expect sparse 1 .view.crisis ""
expect sparse 1 .view.today.yesterday 0
expect sparse 2 '.view.changelog.badges | length' 0
expect sparse 3 '.view.system | join(",")' "PACKAGES,SELDON"
clean_log sparse

echo "panel-view: $pass passed, $fail failed"
((fail == 0))
