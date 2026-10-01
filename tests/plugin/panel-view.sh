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
for tool in bash env cat sed date mkdir mv sleep basename; do
  ln -s "$(command -v "$tool")" "$work/bin/$tool"
done
install -m 755 "$root/tests/plugin/fake-seldon" "$work/bin/seldon"
# The editor launcher the engine calls without a terminal records its argv.
install -m 755 "$root/tests/plugin/fake-recorder" "$work/bin/omarchy-launch-editor"
# The shell's Style.qml asks Hyprland and fontconfig for gaps, rounding and
# the font; outside Hyprland it keeps its defaults when they fail.
printf '#!/bin/sh\nexit 1\n' >"$work/bin/hyprctl"
printf '#!/bin/sh\necho monospace\n' >"$work/bin/fc-match"
chmod 755 "$work/bin/hyprctl" "$work/bin/fc-match"
ln -s "$(command -v sh)" "$work/bin/sh"

pass=0
fail=0

# run <case> <index file> <steps> [VAR=value ...] — one harness run; step reports land in
# $work/<case>.steps (one JSON object per line), the whole log in <case>.log.
run() {
  local name=$1 index=$2 steps=$3
  shift 3
  env -i HOME="$work/home" PATH="$work/bin" QT_QPA_PLATFORM=offscreen \
    XDG_RUNTIME_DIR="${XDG_RUNTIME_DIR:-$work}" \
    HARNESS_PLUGIN_DIR="$plugin" HARNESS_STEPS="$steps" SELDON_INDEX="$index" "$@" \
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
expect sample 2 '.view.changelog.driftTones | join(",")' \
  "tokyo-night accent,~/.config/systemd/user/ollama.service urgent,ollama urgent,libinput accent,noto-fonts accent,firefox accent"
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

# 2. Keyboard (SPEC-PLUGIN §5): Tab / Shift-Tab only hand over to the
#    neighbouring bar panel (a stand-in bar records the direction); ←/→ and
#    h/l switch tabs; digits are fixed per tab id (Today 1, Changelog 2,
#    System 5) and the digit of an absent tab (3 = Work) does nothing.
run keys "$fx/index.sample.json" \
  "key:Tab;key:Backtab;key:Right;key:Right;key:Right;key:Left;text:l;text:h;text:5;text:3;text:1;text:2;key:Down;key:Down*2;text:k;text:j;key:Return;key:Escape" \
  HARNESS_BAR=1
expect keys 1 .view.tab today
expect keys 1 '.switches | join(",")' 1
expect keys 2 .view.tab today
expect keys 2 '.switches | join(",")' "1,-1"
expect keys 3 .view.tab changelog
expect keys 4 .view.tab system
expect keys 5 .view.tab today
expect keys 6 .view.tab system
expect keys 7 .view.tab today
expect keys 8 .view.tab system
expect keys 9 .view.tab system
expect keys 10 .view.tab system
expect keys 11 .view.tab today
expect keys 12 .view.tab changelog
expect keys 12 '.switches | length' 2
expect keys 13 .view.cursorActive true
expect keys 14 .view.cursor 2
expect keys 15 .view.cursor 1
expect keys 16 .view.cursor 2
expect keys 18 .view.opened false
clean_log keys

# 3. The yesterday row opens with Enter and stays in view (the list scrolls
#    to it once the new rows are laid out, one step later).
run yesterday "$fx/index.sample.json" "key:Down;key:Down*10;key:Return;view;key:Down"
expect yesterday 2 .view.cursor 4
expect yesterday 3 .view.today.rows 6
shows yesterday 4 "▾ Yesterday · 1 entry"
expect yesterday 5 .view.cursor 5
shows yesterday 5 "Snapshots aufgeräumt, 108 und 109 gelöscht."
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

# 7. Live (no dev mode): the QuickEntry, Open in editor on every tab and
#    Capture now, against the fake engine. Typing in the field never
#    reaches the panel's keys ("--help" has h and l, which switch tabs
#    otherwise). The case picker is driven by keys (Tab, Down, Return). After
#    "Capture now" the fake engine's status writes an index with one more
#    event; the Changelog shows it through the FileView, without a restart.
jq '.events = [{id: "01M3W2NEWEVENT000000000000", ts: "2026-10-01T18:30:00+02:00", source: "manual", kind: "note",
  subject: "journal", detail: "Written by the harness after Capture now", zone: "green", actor: "human", case: null}] + .events' \
  "$fx/index.sample.json" >"$work/after.json"
mkdir -p "$work/home-live"
run live "" \
  "view;text:n;type:--help;key:Return;settle;type:   ;key:Return;key:Backspace*3;key:Tab;key:Down;key:Down;key:Down;key:Return;key:Backtab;type:for the case;key:Return;settle;key:Escape;text:e;tab:changelog;text:e;text:c;view;wait:changelog.rows=59;tab:system;text:e;settle" \
  HOME="$work/home-live" FAKE_SELDON_FIXTURE="$fx/index.sample.json" FAKE_SELDON_FIXTURE_AFTER="$work/after.json" \
  FAKE_SELDON_WRITTEN=1 HARNESS_RECORD="$work/live.record"
expect live 1 .view.status ok
expect live 1 .view.today.quickEntry.enabled true
expect live 1 .view.today.quickEntry.cases 6
expect live 1 .view.today.quickEntry.editing false
shows live 1 "Note for today's journal, Enter saves"
shows live 1 "No case"
expect live 2 .view.today.quickEntry.editing true
expect live 3 .view.today.quickEntry.text "--help"
expect live 3 .view.tab today
expect live 5 .view.today.quickEntry.result "Saved to the journal · 01M3W1FAKE0000000000000NTE"
expect live 5 .view.today.quickEntry.text ""
shows live 5 "Saved to the journal · 01M3W1FAKE0000000000000NTE"
expect live 7 .view.today.quickEntry.result "Write something first"
expect live 7 .view.today.quickEntry.text "   "
expect live 8 .view.today.quickEntry.text ""
expect live 13 .view.today.quickEntry.caseId C-2026-004
shows live 13 "C-2026-004 · Zed als zweiten Editor installieren"
shows live 13 "Open case"
expect live 17 .view.today.quickEntry.result "Saved to C-2026-004 · 01M3W1FAKE0000000000000NTE"
expect live 18 .view.today.quickEntry.editing false
expect live 18 .view.opened true
expect live 19 .view.tab today
expect live 20 .view.tab changelog
expect live 22 .view.capturing true
shows live 22 "Capturing"
expect live 24 .view.changelog.rows 59
shows live 24 "Written by the harness after Capture now · human"
expect live 24 .view.captureResult "1 new event"
shows live 24 "Last capture: 1 new event"
expect live 25 .view.tab system
expect live 27 .view.openResult "Opened $work/home-live/Seldon/STATUS.md in omarchy-launch-editor"
expect live 27 .view.lastError ""
q() { printf '%q ' "$@"; }
want=$(printf '%s\n' "$(q --version --json)" "$(q capture --all --json --quiet)" "$(q status --json)" \
  "$(q log --json -- --help)" "$(q log --case C-2026-004 --json -- "for the case")" \
  "$(q open journal --editor --json)" "$(q open ledger --editor --json)" \
  "$(q capture --all --json --quiet)" "$(q status --json)" "$(q open status --editor --json)")
got=$(cat "$work/home-live/argv.log" 2>/dev/null || true)
if [[ $got == "$want" ]]; then
  pass=$((pass + 1)); echo "ok   live: engine argv"
else
  fail=$((fail + 1)); echo "FAIL live: engine argv differs"; diff <(echo "$want") <(echo "$got") | sed 's/^/     /'
fi
want=$(printf '%s\n' omarchy-launch-editor "$work/home-live/Seldon/journal/2026/2026-10-01.md" -- \
  omarchy-launch-editor "$work/home-live/Seldon/ledger/2026-10.jsonl" -- \
  omarchy-launch-editor "$work/home-live/Seldon/STATUS.md" --)
got=$(cat "$work/live.record" 2>/dev/null || true)
if [[ $got == "$want" ]]; then
  pass=$((pass + 1)); echo "ok   live: editor paths"
else
  fail=$((fail + 1)); echo "FAIL live: editor launches differ"; diff <(echo "$want") <(echo "$got") | sed 's/^/     /'
fi
clean_log live

echo "panel-view: $pass passed, $fail failed"
((fail == 0))
