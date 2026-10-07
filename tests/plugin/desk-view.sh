#!/usr/bin/env bash
# Drive plugin/Desk.qml, the desk (ADR-0034), in a private headless
# Quickshell (tests/plugin/harness/desk.qml): open and close it the way the
# shell's overlay loader does (hide drops the item, summon makes a new one),
# through the pill's clicks and the `jax.seldon.panel` shim; check the width
# against the setting on four screen widths, the sidebar and stacked
# thresholds, the keyboard map with real key events, the Settings section's
# one write per release, the notices under the header (today's banners
# with their fixes), and the sections built so far: Decisions, System,
# Memory and the Prime Radiant (WP-123; its charts' first-frame and paint
# counters, hover read-outs and grid at every desk width). The old panel's and overlay's scenarios and where each
# went are listed in tests/plugin/COVERAGE.md.
#
# Like the old panel and overlay harnesses, it builds a temp config root with
# copies of the installed shell's Commons/ and Ui/, so `import qs.*`
# resolves to the real shell code. The desk's layer-shell window cannot
# exist offscreen: the script runs a copy of plugin/ whose
# components/desk/DeskWindow.qml is replaced by
# tests/plugin/harness/DeskWindow.qml (a plain Item filling the harness
# window). Nothing talks to the running omarchy-shell.
#
# DESK_SHOTS=<dir> also renders the desk in three themes at 100 % and 50 %
# into <dir> (offscreen renders, not live screenshots).
# Needs quickshell, jq and the installed shell (host check; docs/TESTING.md).
set -euo pipefail

root=$(cd "$(dirname "$0")/../.." && pwd)
fx="$root/fixtures"
omarchy="${OMARCHY_PATH:-/usr/share/omarchy}"
shell_dir="$omarchy/shell"

qs_bin=$(command -v quickshell || command -v qs || true)
[[ -n $qs_bin ]] || { echo "desk-view: quickshell not found" >&2; exit 1; }
command -v jq >/dev/null || { echo "desk-view: jq not found" >&2; exit 1; }
[[ -d $shell_dir/Commons && -d $shell_dir/Ui ]] || { echo "desk-view: shell not found at $shell_dir" >&2; exit 1; }
timeout_bin=$(command -v timeout) || { echo "desk-view: timeout not found" >&2; exit 1; }

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
source "$root/tests/plugin/real-home-guard.sh"

config="$work/config"
plugin="$work/plugin"
mkdir -p "$config/Commons" "$config/Ui" "$work/home" "$work/bin"
cp "$shell_dir"/Commons/* "$config/Commons/"
cp "$shell_dir"/Ui/* "$config/Ui/"
cp "$root/tests/plugin/harness/desk.qml" "$config/shell.qml"
cp -r "$root/plugin" "$plugin"
cp "$root/tests/plugin/harness/DeskWindow.qml" "$plugin/components/desk/DeskWindow.qml"

# Tools for the fake engine, and the fake engine; never a real seldon.
for tool in bash env cat sed date mkdir mv sleep basename grep jq; do
  ln -s "$(command -v "$tool")" "$work/bin/$tool"
done
install -m 755 "$root/tests/plugin/fake-seldon" "$work/bin/seldon"
# The launchers behind a notice's buttons record their argv.
install -m 755 "$root/tests/plugin/fake-recorder" "$work/bin/omarchy-launch-editor"
install -m 755 "$root/tests/plugin/fake-recorder" "$work/bin/omarchy-launch-floating-terminal-with-presentation"
install -m 755 "$root/tests/plugin/fake-recorder" "$work/bin/omarchy-restart-shell"
# The shell's Style.qml asks Hyprland and fontconfig for gaps, rounding and
# the font; outside Hyprland it keeps its defaults when they fail.
printf '#!/bin/sh\nexit 1\n' >"$work/bin/hyprctl"
printf '#!/bin/sh\necho monospace\n' >"$work/bin/fc-match"
chmod 755 "$work/bin/hyprctl" "$work/bin/fc-match"
ln -s "$(command -v sh)" "$work/bin/sh"

pass=0
fail=0

# run <case> <index file> <W>x<H> <steps> [VAR=value ...] — one harness run;
# step reports land in $work/<case>.steps (one JSON object per line), the
# whole log in <case>.log. HOME is $work/home unless the case names one;
# XDG_STATE_HOME and XDG_CONFIG_HOME follow it. An empty index file is a
# live run: the service reads the state index the fake engine writes.
run() {
  local name=$1 index=$2 size=$3 steps=$4 home="$work/home" arg
  shift 4
  for arg in "$@"; do [[ $arg == HOME=* ]] && home=${arg#HOME=}; done
  mkdir -p "$home"
  env -i HOME="$home" XDG_STATE_HOME="$home/.local/state" XDG_CONFIG_HOME="$home/.config" \
    PATH="$work/bin" QT_QPA_PLATFORM=offscreen \
    XDG_RUNTIME_DIR="${XDG_RUNTIME_DIR:-$work}" \
    HARNESS_PLUGIN_DIR="$plugin" HARNESS_STEPS="$steps" HARNESS_W="${size%x*}" HARNESS_H="${size#*x}" \
    SELDON_INDEX="$index" "$@" \
    "$timeout_bin" 90 "$qs_bin" -p "$config/shell.qml" >"$work/$name.log" 2>&1 || true
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
# The text goes into a jq string literal: write a `"` in it as `\"`.
shows() {
  expect "$1" "$2" "[.texts[] | select(. == \"$3\")] | length > 0" true
}

# check <name> <got> <want> — a plain comparison.
check() {
  if [[ $2 == "$3" ]]; then
    pass=$((pass + 1))
    echo "ok   $1"
  else
    fail=$((fail + 1))
    echo "FAIL $1: $2 (want $3)"
  fi
}

# The warnings of engine calls a case makes the fake engine refuse on purpose.
expected_warnings='jax\.seldon: seldon (rules exit 1: AGENTS\.md is not UTF-8 text)$'

# clean_log <case> [regex] — no error, warning or binding loop in the log
# (also allow the warnings the regex matches).
clean_log() {
  local bad
  bad=$(sed 's/\x1b\[[0-9;]*m//g' "$work/$1.log" | grep -a -E "ERROR|WARN|TypeError|ReferenceError|Binding loop|HARNESS error|nothing to (click|drag|hover|wheel)|wait timed out" \
    | grep -a -v -E "WAYLAND_DISPLAY is present|QT_QPA_PLATFORM|--- WARNING ---|most functionality will be broken" \
    | grep -a -v -E "$expected_warnings" | grep -a -v -E "${2:-^$}" || true)
  if [[ -z $bad ]]; then
    pass=$((pass + 1))
    echo "ok   $1: log clean"
  else
    fail=$((fail + 1))
    echo "FAIL $1: log has errors"
    echo "$bad" | sed 's/^/     /'
  fi
}

sample="$fx/index.sample.json"
gap=5 # Style.gapsOut outside Hyprland (hyprctl fails)

# deskw <window width> <pct> — the width ADR-0034 §1 gives: avail = W − 2·gap,
# clamp(round(avail × pct / 100), min(960, avail), avail).
deskw() {
  local avail=$(($1 - 2 * gap)) want min
  want=$(((avail * $2 * 2 + 100) / 200))
  min=$((avail < 960 ? avail : 960))
  ((want < min)) && want=$min
  ((want > avail)) && want=$avail
  echo "$want"
}

# ---------------------------------------------------------------------------
# 1. Width (ADR-0034 §1): 50 / 67 / 75 / 100 % on 1366, 1920, 2560 and 3840 px
#    windows, set as Omarchy's bar settings set them (the pill pushes the
#    entry to the service): the expected width, centred within a pixel, the
#    top and bottom at the gaps; 1366 at 50 % is the 960 px floor. The
#    sidebar is open at every width here (the desk is never under 960).
for W in 1366 1920 2560 3840; do
  run "width-$W" "$sample" "${W}x1080" "summon;width:50;width:67;width:75;width:100"
  i=1
  for pct in 100 50 67 75 100; do
    want=$(deskw "$W" "$pct")
    expect "width-$W" $i .view.widthPct "$pct"
    expect "width-$W" $i .view.desk.w "$want"
    expect "width-$W" $i '(.view.desk.x - '"$gap"') - ('"$W"' - '"$gap"' - .view.desk.x - .view.desk.w) | fabs <= 1' true
    expect "width-$W" $i .view.desk.y "$gap"
    expect "width-$W" $i .view.desk.h $((1080 - 2 * gap))
    expect "width-$W" $i .view.layout.sidebar open
    expect "width-$W" $i '.overflow | join(" | ")' ""
    i=$((i + 1))
  done
  expect "width-$W" 1 '.writes | length' 0
  clean_log "width-$W"
done

# ---------------------------------------------------------------------------
# 2. Thresholds (ADR-0034 §1, WP-121 Decisions 2): a desk under 960 px (only
#    on a window that narrow) shows the sidebar's icons whatever the
#    setting; under 760 px list and detail stack; the setting `deskSidebar`
#    collapsed gives icons on a wide desk; solo sections (7, 8) have no list
#    column and never stack. Nothing leaves the window or the desk.
run thresholds "$sample" 1920x1080 \
  "summon;resize:950x1000;resize:750x1000;resize:1920x1080;sidebar:collapsed;sidebar:open;text:7;resize:700x1000;text:8"
expect thresholds 1 '[.view.layout.sidebar, .view.layout.forced, .view.layout.stacked] | map(tostring) | join(",")' "open,false,false"
expect thresholds 2 .view.desk.w 940
expect thresholds 2 '[.view.layout.sidebar, .view.layout.forced, .view.layout.stacked] | map(tostring) | join(",")' "icons,true,false"
expect thresholds 2 '[.texts[] | select(. == "Collapse" or . == "SECTIONS")] | length' 0
expect thresholds 3 .view.desk.w 740
expect thresholds 3 '[.view.layout.sidebar, .view.layout.forced, .view.layout.stacked] | map(tostring) | join(",")' "icons,true,true"
expect thresholds 4 '[.view.layout.sidebar, .view.layout.stacked] | map(tostring) | join(",")' "open,false"
expect thresholds 5 '[.view.layout.sidebar, .view.layout.forced, .view.layout.stacked] | map(tostring) | join(",")' "icons,false,false"
expect thresholds 5 .view.settings.sidebar collapsed
expect thresholds 6 .view.layout.sidebar open
expect thresholds 7 .view.section radiant
expect thresholds 7 '[.view.layout.solo, .view.layout.listW] | map(tostring) | join(",")' "true,0"
expect thresholds 8 '[.view.layout.solo, .view.layout.stacked, .view.layout.sidebar] | map(tostring) | join(",")' "true,false,icons"
expect thresholds 9 .view.section graph
for i in 1 2 3 4 5 6 7 8 9; do expect thresholds $i '.overflow | join(" | ")' ""; done
expect thresholds 9 '.writes | length' 0
clean_log thresholds

# ---------------------------------------------------------------------------
# 3. Keys (ADR-0034 §2), real key events: 1–8 and `,` reach the nine
#    targets; Alt+↓ / Alt+↑ walk them and wrap both ways; Tab does nothing;
#    `/` gives the search the keys (a digit then types, it switches
#    nothing), Esc in it clears and leaves; Enter leaves and keeps the
#    filter, and Esc then clears it before it closes the desk; the next Esc
#    closes through the shell's hide.
run keys "$sample" 1920x1080 \
  "summon;text:2;text:3;text:4;text:5;text:6;text:7;text:8;text:,;text:1;key:Alt+Down;key:Alt+Down*8;key:Alt+Up;key:Alt+Down;key:Tab;key:Backtab;text:/;text:3;type:ab;key:Escape;text:/;type:mesa;key:Return;key:Escape;key:Escape;summon;text:/;type:x;click:Work;text:4"
i=1
for want in today changelog work decisions system memory radiant graph settings today changelog today settings today today today; do
  expect keys $i .view.section "$want"
  i=$((i + 1))
done
expect keys 1 .view.keys true
expect keys 1 .bare true
expect keys 1 '.view.visited | join(",")' today
expect keys 10 '.view.visited | length' 9
expect keys 17 '[.view.search.focused, .view.keys, .view.editing] | map(tostring) | join(",")' "true,false,true"
expect keys 18 .view.search.text 3
expect keys 18 .view.section today
expect keys 19 .view.search.text 3ab
expect keys 20 '[.view.search.focused, .view.search.text, .view.keys, .view.opened] | map(tostring) | join(",")' "false,,true,true"
expect keys 22 .view.search.text mesa
expect keys 23 '[.view.search.focused, .view.search.text, .view.keys] | map(tostring) | join(",")' "false,mesa,true"
expect keys 24 '[.view.search.text, .view.opened] | map(tostring) | join(",")' ",true"
expect keys 25 .view.opened false
expect keys 25 '.calls | map(select(startswith("hide"))) | length' 1
expect keys 25 '.writes | length' 0
# A click on a sidebar row while the search has the keys: the section
# changes, its filter goes and the keys come back to the desk.
expect keys 28 '[.view.search.focused, .view.search.text] | map(tostring) | join(",")' "true,x"
expect keys 29 '[.view.section, .view.search.focused, .view.search.text, .view.keys] | map(tostring) | join(",")' "work,false,,true"
expect keys 30 .view.section decisions
clean_log keys

# ---------------------------------------------------------------------------
# 4. Settings › Appearance (ADR-0034 §1): `,` opens it; dragging the slider
#    from Full to 50 % previews the width on the desk and writes nothing; the
#    release writes once, through the facade's updateEntryInline, with every
#    key of the entry (an unknown one too) and the new width; the shell's
#    reload brings it back as the stored value. A preset click writes once;
#    a second click on the selected preset sends nothing (ButtonGroup
#    emits only a change; the stored-value guard is case settings-stored). The
#    sidebar switch and the sidebar's fold button write deskSidebar, the
#    width kept. Omarchy's bar settings changing the key move the desk too.
entry='{"captureIntervalMin":30,"wipLimit":4,"driftInBar":"all","futureKey":"kept"}'
run settings "$sample" 1920x1080 \
  "summon;text:,;drag:deskWidthSlider:0.98,0.02;release;wait:settings.pending=false;click:67 %;click:67 %;click:Collapsed;clickName:deskFold;width:75;key:Down;key:Down;key:Down;key:Up" \
  HARNESS_SETTINGS="$entry"
expect settings 2 .view.section settings
expect settings 2 .view.sectionView.group appearance
shows settings 2 "Desk width"
shows settings 2 "1910 px on this screen"
shows settings 2 "100 %"
expect settings 3 '.writes | length' 0
expect settings 3 .view.settings.preview 50
expect settings 3 .view.widthPct 50
expect settings 3 .view.desk.w 960
shows settings 3 "960 px on this screen"
expect settings 4 '.writes | length' 1
expect settings 4 '.writes[0].id' jax.seldon
expect settings 4 '.writes[0].settings | [to_entries[] | "\(.key)=\(.value)"] | sort | join(",")' 'captureIntervalMin=30,deskWidth=50,driftInBar=all,futureKey=kept,wipLimit=4'
expect settings 4 .view.settings.preview -1
expect settings 4 .view.desk.w 960
expect settings 5 '[.view.settings.pending, .view.settings.stored, .entry.deskWidth, .view.widthPct] | map(tostring) | join(",")' "false,50,50,50"
expect settings 5 '.writes | length' 1
expect settings 6 '.writes | length' 2
expect settings 6 '.writes[1].settings | [to_entries[] | "\(.key)=\(.value)"] | sort | join(",")' 'captureIntervalMin=30,deskWidth=67,driftInBar=all,futureKey=kept,wipLimit=4'
expect settings 6 .view.desk.w 1280
expect settings 7 '.writes | length' 2
expect settings 8 '.writes | length' 3
expect settings 8 '.writes[2].settings | [to_entries[] | "\(.key)=\(.value)"] | sort | join(",")' 'captureIntervalMin=30,deskSidebar=collapsed,deskWidth=67,driftInBar=all,futureKey=kept,wipLimit=4'
expect settings 8 .view.layout.sidebar icons
expect settings 9 '.writes | length' 4
expect settings 9 '.writes[3].settings.deskSidebar + " " + (.writes[3].settings.deskWidth | tostring)' "open 67"
expect settings 9 .view.layout.sidebar open
expect settings 10 '.writes | length' 4
expect settings 10 .view.widthPct 75
shows settings 10 "75 %"
expect settings 10 .view.settings.refused false
expect settings 13 .view.sectionView.group quiet
expect settings 14 .view.sectionView.group agents
shows settings 14 "4, set in Omarchy's bar settings (Seldon widget)"
expect settings 14 '.writes | length' 4
expect settings 14 '.overflow | join(" | ")' ""
clean_log settings

# A shell that refuses the write: the stored value stays, the page says
# where to set it, and the desk carries on.
run settings-refused "$sample" 1920x1080 "summon;text:,;click:75 %;click:Collapsed" HARNESS_REFUSE=1
expect settings-refused 3 '.writes | length' 1
expect settings-refused 3 '[.view.settings.refused, .view.widthPct, .view.settings.pending] | map(tostring) | join(",")' "true,100,false"
shows settings-refused 3 "The shell did not take the change. Change it in Omarchy's bar settings (Seldon widget)."
expect settings-refused 4 '.writes | length' 2
expect settings-refused 4 .view.layout.sidebar open
clean_log settings-refused

# The rule "never write a stored value" (Desk.writeSetting; its twin is
# Model.deskSettingsWrite, unit-tested): a click on the slider at the
# value in force writes nothing — the default 100 with no deskWidth in the
# entry (a write would add the default to shell.json), and 50 once the key
# is there (the shell would answer "nothing changed" with false, which the
# desk must not show as a refusal; the stand-in answers so, as shell.qml).
run settings-stored "$sample" 1920x1080 \
  "summon;text:,;drag:deskWidthSlider:0.98,0.98;release;width:50;drag:deskWidthSlider:0.02,0.02;release;view" \
  HARNESS_SETTINGS='{"captureIntervalMin":30}'
expect settings-stored 3 .view.settings.preview 100
expect settings-stored 4 '[(.writes | length), .view.settings.refused, .view.widthPct] | map(tostring) | join(",")' "0,false,100"
expect settings-stored 5 .view.widthPct 50
expect settings-stored 7 '[(.writes | length), .view.settings.refused, .view.widthPct] | map(tostring) | join(",")' "0,false,50"
expect settings-stored 8 '.entry | has("deskWidth")' true
shows settings-stored 8 "50 %"
clean_log settings-stored

# The mouse wheel (and a touchpad) over the slider previews and writes
# once after a 600 ms pause, never per notch: three notches down from Full
# give one write of 70; a notch then a preset click before the pause give
# the preset's write alone, and nothing follows; a notch then Esc writes
# the notch's value as the desk closes.
run settings-wheel "$sample" 1920x1080 \
  "summon;text:,;wheel:deskWidthSlider:-120*3;wait:settings.writes=1;wheel:deskWidthSlider:120;click:67 %;pause:900;wheel:deskWidthSlider:-120;key:Escape"
expect settings-wheel 3 '[(.writes | length), .view.settings.preview, .view.widthPct, .view.settings.wheelPending] | map(tostring) | join(",")' "0,70,70,true"
expect settings-wheel 3 .view.desk.w "$(deskw 1920 70)"
expect settings-wheel 4 '[(.writes | length), .view.settings.preview, .view.settings.wheelPending] | map(tostring) | join(",")' "1,-1,false"
expect settings-wheel 4 '.writes[0].settings.deskWidth' 70
expect settings-wheel 5 '[(.writes | length), .view.settings.preview] | map(tostring) | join(",")' "1,80"
expect settings-wheel 6 '[(.writes | length), .view.settings.wheelPending, .view.widthPct] | map(tostring) | join(",")' "2,false,67"
expect settings-wheel 6 '.writes[1].settings.deskWidth' 67
expect settings-wheel 7 '.writes | length' 2
expect settings-wheel 8 '[(.writes | length), .view.settings.preview] | map(tostring) | join(",")' "2,60"
expect settings-wheel 9 '[(.writes | length), .view.opened] | map(tostring) | join(",")' "3,false"
expect settings-wheel 9 '.writes[2].settings.deskWidth' 60
clean_log settings-wheel

# The plugin enabled but not in the bar (no pill, so no entry pushed):
# the desk writes nothing, keeps a change for this shell (across a hide)
# and says how to keep it — no refusal.
run settings-no-pill "$sample" 1920x1080 "summon;text:,;click:75 %;clickName:deskFold;hide;summon;view" HARNESS_NO_PILL=1
expect settings-no-pill 3 '[(.writes | length), .view.widthPct, .view.settings.noEntry, .view.settings.refused] | map(tostring) | join(",")' "0,75,true,false"
shows settings-no-pill 3 "Add Seldon to the bar to keep this setting; until then it holds until the shell restarts."
expect settings-no-pill 3 '[.texts[] | select(startswith("The shell did not take"))] | length' 0
expect settings-no-pill 4 '[(.writes | length), .view.layout.sidebar] | map(tostring) | join(",")' "0,icons"
expect settings-no-pill 7 '[(.writes | length), .view.widthPct, .view.settings.sidebar, .view.section] | map(tostring) | join(",")' "0,75,collapsed,settings"
clean_log settings-no-pill

# ---------------------------------------------------------------------------
# 5. Open and close the way the shell and the pill do (ADR-0034 §4, §7):
#    toggle twice closes; the pill's left click toggles, its middle click
#    opens the Prime Radiant; the `jax.seldon.panel` shim forwards to the
#    desk (`tab work` lands on section 3, an unknown tab changes nothing,
#    `view` reads the desk or {"opened":false}, `resolve crisis` and
#    `filter` open the Changelog); the desk remembers its section across a
#    hide (the shell drops the item); the old overlay payload
#    {"period":"30"} lands on the Prime Radiant; `call section|select|view`;
#    Esc and a click beside the desk close it.
run ipc "$sample" 1920x1080 \
  'view;toggle;toggle;pill:left;pill:left;pill:middle;shim:tab:work;shim:tab:bogus;shim:view;shim:toggle;shim:view;shim:open;shim:resolve:crisis;shim:filter:pacman;shim:filter:nope;shim:close;summon:{"period":"30"};call:section:decisions;call:section:nope;call:select:ADR-0004;call:view:;hide;summon;key:Escape;summon;clickAt:2,2;shim:pill'
expect ipc 1 .view.opened false
expect ipc 2 '[.view.opened, .view.section, .bare] | map(tostring) | join(",")' "true,today,true"
expect ipc 3 .view.opened false
expect ipc 4 .view.opened true
expect ipc 4 .deskCalls 1
expect ipc 5 .view.opened false
expect ipc 6 '[.view.opened, .view.section] | map(tostring) | join(",")' "true,radiant"
expect ipc 6 '.calls[-1]' 'summon jax.seldon {"section":"radiant"}'
expect ipc 7 '[.call, .view.section] | join(",")' "ok,work"
expect ipc 8 '[.call, .view.section] | join(",")' "unknown tab,work"
expect ipc 9 '.call | fromjson | [.opened, .section] | map(tostring) | join(",")' "true,work"
expect ipc 10 .view.opened false
expect ipc 11 .call '{"opened":false}'
expect ipc 12 '[.view.opened, .view.section] | map(tostring) | join(",")' "true,work"
expect ipc 13 '[.call, .view.section] | join(",")' "ok,changelog"
# A summon of the open desk re-targets it to the focused monitor again
# (SPEC-PLUGIN §5.1); a fresh desk starts at its first.
expect ipc 12 .view.screen harness-1
expect ipc 13 .view.screen harness-2
expect ipc 14 '[.call, .view.section] | join(",")' "ok,changelog"
expect ipc 15 .call "unknown source"
expect ipc 16 .view.opened false
expect ipc 17 '[.view.opened, .view.section] | map(tostring) | join(",")' "true,radiant"
expect ipc 18 '[.call, .view.section] | join(",")' "ok,decisions"
expect ipc 19 '[.call, .view.section] | join(",")' "unknown section,decisions"
expect ipc 20 '[.call, .view.selected] | join(",")' "ok,ADR-0004"
expect ipc 21 '.call | fromjson | .section' decisions
expect ipc 22 .view.opened false
expect ipc 23 '[.view.opened, .view.section] | map(tostring) | join(",")' "true,decisions"
expect ipc 24 .view.opened false
expect ipc 25 .view.opened true
expect ipc 26 .view.opened false
expect ipc 27 '.call | fromjson | [.opened, .text] | map(tostring) | join(",")' "false,2 · 2"
expect ipc 27 '.writes | length' 0
clean_log ipc

# ---------------------------------------------------------------------------
# 6. The stacked layout (a window under 770 px): the list, Enter shows the
#    detail with its back row, Esc goes back to the list, the next Esc
#    closes; the back row's click goes back too.
run stacked "$sample" 700x900 "summon;text:,;key:Down;key:Return;key:Escape;key:Return;clickName:deskBack;key:Escape"
expect stacked 2 '[.view.layout.stacked, .view.detailShown] | map(tostring) | join(",")' "true,false"
shows stacked 2 "SETTINGS"
expect stacked 2 '[.texts[] | select(. == "Desk width")] | length' 0
expect stacked 3 .view.sectionView.group capture
expect stacked 4 .view.detailShown true
shows stacked 4 "‹ Back to the list"
shows stacked 4 "Capture interval"
expect stacked 4 '[.texts[] | select(. == "SETTINGS")] | length' 0
expect stacked 5 '[.view.detailShown, .view.opened] | map(tostring) | join(",")' "false,true"
expect stacked 6 .view.detailShown true
expect stacked 7 '[.view.detailShown, .view.keys] | map(tostring) | join(",")' "false,true"
expect stacked 8 .view.opened false
for i in 1 2 3 4 5 6 7; do expect stacked $i '.overflow | join(" | ")' ""; done
clean_log stacked

# ---------------------------------------------------------------------------
# 7. The notices under the header (today's banners, with their fixes) and
#    the header's chip.
# 7a. Snapper not readable (ADR-0026, WP-054): the notice with Run in
#     terminal and Check again; after the click, the hint under the buttons.
#     On a narrow desk the chip's title does not fit beside the KPI strip:
#     it says "1 notice".
run snapper "$fx/index-variants/snapper-degraded.json" 1920x1080 "summon;click:Run in terminal;resize:1000x900" \
  HARNESS_RECORD="$work/snapper.record"
expect snapper 1 '.view.notices | join(",")' "Snapshots not readable"
expect snapper 1 .view.chip "Snapshots not readable"
shows snapper 1 'sudo setfacl -m u:$USER:rx /.snapshots'
shows snapper 1 "Check again"
expect snapper 1 '[.texts[] | select(. == "When the command has finished, press Check again")] | length' 0
shows snapper 2 "When the command has finished, press Check again"
expect snapper 1 .view.chipShown "▾ Snapshots not readable"
expect snapper 3 .view.chipShown "▾ 1 notice"
expect snapper 3 '.overflow | join(" | ")' ""
clean_log snapper

# 7b. Not initialised: the status notice with its pictogram's fix, no KPI
#     figures, no counts; the chip folds and unfolds the notices.
run uninit "$fx/index-variants/not-initialised.json" 1920x1080 "summon;clickName:deskChip;clickName:deskChip"
expect uninit 1 .view.status notInitialised
expect uninit 1 '.view.notices | join(",")' "Logbook not initialised"
expect uninit 1 '.view.kpis | length' 0
expect uninit 1 '[.view.counts[] | .text] | join("")' ""
shows uninit 1 "Create your logbook once with seldon init."
expect uninit 2 .view.noticesFolded true
expect uninit 2 '[.texts[] | select(. == "Create your logbook once with seldon init.")] | length' 0
shows uninit 2 "▸ Logbook not initialised"
expect uninit 3 .view.noticesFolded false
clean_log uninit

# 7c. A plugin updated under a running shell (WP-090): the restart notice
#     first, the status notice after it (the chip says one more); Restart
#     shell runs omarchy-restart-shell once for a double click. With the
#     repository's manifest, no notice.
manifest=$(jq -c . "$root/plugin/manifest.json")
run restart-same "$sample" 1920x1080 "summon" HARNESS_MANIFEST="$manifest"
expect restart-same 1 '.view.notices | length' 0
expect restart-same 1 .view.chip ""
clean_log restart-same
run restart-updated "$fx/index-variants/not-initialised.json" 1920x1080 "summon;click:Restart shell;click:Restart shell" \
  HARNESS_MANIFEST="$(jq -c '.version = "99.0.0"' <<<"$manifest")" HARNESS_RECORD="$work/restart-updated.record"
expect restart-updated 1 '.view.notices | join(",")' "Restart the shell to finish the update,Logbook not initialised"
expect restart-updated 1 .view.chip "Restart the shell to finish the update +1"
shows restart-updated 1 "Seldon 99.0.0 is installed, but the shell still runs $(jq -r .version <<<"$manifest"). The shell loads new plugin code only when it restarts."
deadline=$((SECONDS + 15))
until [[ -s $work/restart-updated.record ]] || ((SECONDS >= deadline)); do sleep 0.2; done
sleep 1
check "restart-updated: Restart shell runs omarchy-restart-shell once, no arguments" \
  "$(cat "$work/restart-updated.record" 2>/dev/null || true)" "$(printf '%s\n' omarchy-restart-shell --)"
clean_log restart-updated

# 7d. The rules notice (WP-101, WP-111), live against the fake engine:
#     doctor when the desk opens (beside the queue), Update rules runs
#     `rules update --json` once, doctor again, and the notice turns into
#     the one-line result; opened again, the result is gone. A refusal
#     says what failed under the notice, which stays.
q() { printf '%q ' "$@"; }
mkdir -p "$work/home-rules"
run rules-outdated "" 1920x1080 \
  "summon;wait:chip=The logbook's agent rules are outdated (v1);click:Update rules;settle;wait:chip^=Agent rules updated to v3;hide;summon;view" \
  HOME="$work/home-rules" FAKE_SELDON_FIXTURE="$sample" FAKE_SELDON_RULES=outdated
expect rules-outdated 2 '.view.notices | join(",")' "The logbook's agent rules are outdated (v1)"
shows rules-outdated 2 "Agents read AGENTS.md. Update rewrites only Seldon's block; your own rules stay, an edited block is archived first."
expect rules-outdated 5 '.view.notices | join(",")' "Agent rules updated to v3; your old copy is in archive/AGENTS-2026-10-06.md"
expect rules-outdated 8 '.view.notices | length' 0
if grep -qx "$(q rules update --json)" "$work/home-rules/argv.log" && [[ $(grep -c . "$work/home-rules/doctor.log") == 2 ]] \
  && ! grep -q '^doctor' "$work/home-rules/argv.log"; then
  check "rules-outdated: rules update once, doctor before and after, beside the queue" ok ok
else
  check "rules-outdated: rules update once, doctor before and after, beside the queue" "$(cat "$work/home-rules/argv.log" "$work/home-rules/doctor.log" 2>/dev/null | tr '\n' '|')" ok
fi
clean_log rules-outdated
mkdir -p "$work/home-rules-fail"
echo "AGENTS.md is not UTF-8 text" >"$work/home-rules-fail/rules-fail"
run rules-fail "" 1920x1080 "summon;wait:chip=The logbook's agent rules are outdated (v1);click:Update rules;settle;view" \
  HOME="$work/home-rules-fail" FAKE_SELDON_FIXTURE="$sample" FAKE_SELDON_RULES=outdated
shows rules-fail 5 "Updating the agent rules failed: AGENTS.md is not UTF-8 text"
expect rules-fail 5 '.view.notices | join(",")' "The logbook's agent rules are outdated (v1)"
clean_log rules-fail

# 7e. Capture warnings (WP-085), live: the start-up capture warns, the
#     neutral notice shows the first line of each; `c` captures again
#     without warnings and the notice goes. The header's subline says when.
reset_warning="state reset recorded: pacman took a new baseline because ~/.local/state/seldon was missing, unreadable or bound to another logbook, so changes made in between may be missing. If you have a backup of it, restore it and run \`seldon capture\` again (user guide: Back up and restore the state directory)"
move_warning=$'cannot move ~/.local/state/seldon/owned.json aside: permission denied\ncaused by: EACCES'
mkdir -p "$work/home-capture-warned"
run capture-warned "" 1920x1080 "summon;settle;text:c;settle;view" \
  HOME="$work/home-capture-warned" FAKE_SELDON_FIXTURE="$sample" FAKE_SELDON_CAPTURE_WARNED=1 \
  FAKE_SELDON_CAPTURE_WARNINGS="$(jq -cn --arg a "$reset_warning" --arg b "$move_warning" '[$a, $b]')"
expect capture-warned 2 '.view.notices | join(",")' "Capture warned"
expect capture-warned 2 '[.texts[] | select(startswith("state reset recorded: pacman took a new baseline"))] | length > 0' true
expect capture-warned 5 '.view.notices | length' 0
expect capture-warned 5 '.view.subline | test("^workstation-7f3a · Omarchy 4.0.7-1 · captured ")' true
check "capture-warned: captures" "$(cat "$work/home-capture-warned/captures" 2>/dev/null || echo 0)" 2
clean_log capture-warned

# ---------------------------------------------------------------------------
# 8. The Prime Radiant, section 7 (ADR-0034 §4, SPEC-PLUGIN §6; WP-123): the
#    0.1 overlay's scenarios (overlay-view.sh, COVERAGE.md) on the desk.
#    `.view.sectionView` is the section's read-out: period, window, grid
#    mode, the grid area, aggregation passes and the six slots (window
#    coordinates) with their charts.

# rcounts <case> <step> <period> <heatmap,series,driftBars,riskDonut,timeline,plan rows>
rcounts() {
  expect "$1" "$2" .view.sectionView.period "$3"
  expect "$1" "$2" '[.view.sectionView.slots[] | .rows | tostring] | join(",")' "$4"
}

# rfits <case> <step> — six slots with a size inside the grid area's width
# (and its height unless the grid scrolls); no text leaves its slot, and
# unless the grid scrolls (rows scrolled off reach past the desk and the
# window, clipped) none leaves the desk or the window.
rfits() {
  local a='.view.sectionView as $v | [$v.slots[] | select(.w <= 0 or .h <= 0 or .x < $v.area.x or .x + .w > $v.area.x + $v.area.w + 1'
  expect "$1" "$2" "$a"' or (($v.scrolls | not) and (.y < $v.area.y or .y + .h > $v.area.y + $v.area.h + 1)))] | length' 0
  expect "$1" "$2" 'if .view.sectionView.scrolls then [.overflow[] | select(test(" @slot:"))] else .overflow end | join(" | ")' ""
}

# rsummaries <case> <step> <text> — every chart's summary, " | "-joined.
rsummaries() {
  expect "$1" "$2" '[.view.sectionView.slots[] | .chart.summary] | join(" | ")' "$3"
}

# rhovered <case> <step> <slot> <text> — that chart's hover read-out, and
# it is the only one.
rhovered() {
  expect "$1" "$2" "[.view.sectionView.slots[] | select(.chart.hover != \"\") | .id + \"=\" + .chart.hover] | join(\" | \")" "$3=$4"
}

# rpaints <case> <step> <heatmap,series,driftBars,riskDonut,timeline,plan paints>
rpaints() {
  expect "$1" "$2" '[.view.sectionView.slots[] | .chart.paints | tostring] | join(",")' "$3"
}

plan_s="2 active cases · 6 of 9 steps done"
risk_s="8 cases · R0 1 · R1 3 · R2 3 · R3 1 · all time"
drift_s="12 opened · 7 resolved in 5 weeks · peak 2026-W40"
s30="68 events on 14 of 30 days · busiest 2026-10-01 (30) | explicit 324 → 327 · total 2005 → 2009 · 2 samples | $drift_s | $risk_s | 7 cases (6 open) · 2 releases · 6 snapshots · 2 crises | $plan_s"
s90="73 events on 15 of 90 days · busiest 2026-10-01 (30) | explicit 323 → 327 · total 2004 → 2009 · 3 samples | $drift_s | $risk_s | 8 cases (6 open) · 2 releases · 6 snapshots · 2 crises | $plan_s"
s365=${s90/of 90 days/of 365 days}
sall=${s90/of 90 days/of 366 days}
radiant='{"section":"radiant"}'

# 8a. Periods and IPC at 1920×1080 on the sample (overlay scenario 1). The
#     section opens on 90 d; ←/→ and h/l walk the periods and wrap (the
#     digits are the desk's sections now); a chip click picks one and the
#     keys stay with the desk; leaving the section and coming back starts on
#     90 d again; a summon of the open desk with {"period":"all"} sets it;
#     `call setPeriod` picks a period (an unknown id changes nothing) and
#     shows section 7 from another section; `call hover` answers only while
#     section 7 is shown; Esc closes. Nothing aggregates on the way.
run radiant-ipc "$sample" 1920x1080 \
  "summon:$radiant;key:Left;key:Left;key:Right;text:l;text:h;click:365 d;text:4;text:7;summon:{\"period\":\"all\"};call:setPeriod:30;call:setPeriod:7;section:decisions;call:setPeriod:365;call:hover:;section:system;call:hover:series 0.5,0.5;call:view:;text:7;key:Escape"
expect radiant-ipc 1 '[.view.opened, .view.section, .view.status] | map(tostring) | join(",")' "true,radiant,ok"
rcounts radiant-ipc 1 90 "90,3,5,4,18,2"
expect radiant-ipc 1 '.view.sectionView.window.from + " " + .view.sectionView.window.to' "2026-07-04 2026-10-01"
expect radiant-ipc 1 .view.sectionView.caption "90 d · 2026-07-04 – 2026-10-01"
for text in "Prime Radiant" "90 d · 2026-07-04 – 2026-10-01" "30 d" "90 d" "365 d" "All" \
  Heatmap Series DriftBars RiskDonut Timeline "The Plan" releases snapshots cases crises \
  "73 events on 15 of 90 days · busiest 2026-10-01 (30)" "$risk_s" "$plan_s" \
  "C-2026-003 · R3" "Omarchy auf 4.0.7 aktualisieren" "4/5 steps · agent: claude-code" "2/4 steps · agent: claude-code"; do
  shows radiant-ipc 1 "$text"
done
expect radiant-ipc 1 '[.texts[] | select(. == "Releases, snapshots, cases, crises")] | length' 0
rsummaries radiant-ipc 1 "$s90"
expect radiant-ipc 1 '[.view.sectionView.slots[] | .chart.empty] | any' false
expect radiant-ipc 1 .view.sectionView.mode wide
expect radiant-ipc 1 .view.layout.solo true
rfits radiant-ipc 1
rcounts radiant-ipc 2 30 "30,2,5,4,17,2"
rsummaries radiant-ipc 2 "$s30"
expect radiant-ipc 2 '.view.sectionView.window.from + " " + .view.sectionView.window.to' "2026-09-02 2026-10-01"
shows radiant-ipc 2 "30 d · 2026-09-02 – 2026-10-01"
rcounts radiant-ipc 3 all "366,3,5,4,18,2"
rsummaries radiant-ipc 3 "$sall"
expect radiant-ipc 3 '.view.sectionView.window.from + "|" + .view.sectionView.window.to' "|"
shows radiant-ipc 3 "All · everything in the index"
rcounts radiant-ipc 4 30 "30,2,5,4,17,2"
rcounts radiant-ipc 5 90 "90,3,5,4,18,2"
rcounts radiant-ipc 6 30 "30,2,5,4,17,2"
rcounts radiant-ipc 7 365 "365,3,5,4,18,2"
rsummaries radiant-ipc 7 "$s365"
expect radiant-ipc 7 .view.sectionView.window.from "2025-10-02"
expect radiant-ipc 7 .view.keys true
expect radiant-ipc 8 .view.section decisions
expect radiant-ipc 9 .view.section radiant
rcounts radiant-ipc 9 90 "90,3,5,4,18,2"
rcounts radiant-ipc 10 all "366,3,5,4,18,2"
expect radiant-ipc 11 '[.call, .view.sectionView.period] | join(",")' "30,30"
expect radiant-ipc 12 '[.call, .view.sectionView.period] | join(",")' "30,30"
expect radiant-ipc 13 .view.section decisions
expect radiant-ipc 14 '[.call, .view.section, .view.sectionView.period] | join(",")' "365,radiant,365"
expect radiant-ipc 15 '.call | fromjson | .slot + "=" + .hover' "="
expect radiant-ipc 17 '.call | fromjson | .error' "the Prime Radiant is not shown"
expect radiant-ipc 18 '.call | fromjson | .section' system
expect radiant-ipc 19 '[.view.section, .view.sectionView.period] | join(",")' "radiant,90"
expect radiant-ipc 19 '.view.sectionView.slots | length' 6
expect radiant-ipc 20 .view.opened false
# The service's passes are those of loading the index; the section's none.
expect radiant-ipc 19 .view.sectionView.aggregations.section 0
expect radiant-ipc 19 .view.sectionView.aggregations.service "$(sed -n 1p "$work/radiant-ipc.steps" | jq .view.sectionView.aggregations.service)"
clean_log radiant-ipc

# 8b. Entering the section from closed, as the shell's loader does it (a
#     new Desk.qml, created bare, `service` injected afterwards; overlay
#     scenario 2): the first frame runs no aggregation — the service's count
#     is the one from before the desk existed, the section's is 0 — and has
#     painted nothing; every chart has painted once by frame 2. A period
#     switch aggregates nothing and repaints only the charts whose data
#     changed (RiskDonut and The Plan have no period); hovering repaints
#     nothing; a resize repaints every chart once. A second fresh open
#     behaves the same. (firstFrame is recorded on the frame after the
#     step's report, so it shows in the next step's.)
run radiant-fresh "$sample" 1920x1080 \
  "fresh:$radiant;view;key:Left;view;hoverItem:heatmap:-1;hoverItem:plan:0;leave;view;resize:2560x1080;view;fresh:{\"period\":\"all\"};view"
expect radiant-fresh 1 .firstFrame null
expect radiant-fresh 2 '.firstFrame.serviceBefore == .firstFrame.service' true
expect radiant-fresh 2 '.firstFrame.service > 0' true
expect radiant-fresh 2 '[.firstFrame.bare, .firstFrame.opened, .firstFrame.section] | map(tostring) | join(",")' "true,true,0"
expect radiant-fresh 2 '.firstFrame.paints | join(",")' "0,0,0,0,0,0"
expect radiant-fresh 2 .firstFrame.paintedBy 2
rpaints radiant-fresh 2 "1,1,1,1,1,1"
expect radiant-fresh 2 '.view.sectionView.aggregations.service == .firstFrame.service' true
expect radiant-fresh 2 .view.sectionView.aggregations.section 0
rcounts radiant-fresh 3 30 "30,2,5,4,17,2"
rpaints radiant-fresh 4 "2,2,2,1,2,1"
expect radiant-fresh 4 '.view.sectionView.aggregations.service == .firstFrame.service' true
expect radiant-fresh 4 .view.sectionView.aggregations.section 0
rhovered radiant-fresh 5 heatmap "Thu 2026-10-01 · 30 events · pacman 7 · agent 6 · seldon 6 · snapper 4 · config 2 · manual 2 · omarchy 1 · plugins 1 · theme 1"
shows radiant-fresh 5 "Thu 2026-10-01 · 30 events · pacman 7 · agent 6 · seldon 6 · snapper 4 · config 2 · manual 2 · omarchy 1 · plugins 1 · theme 1"
rhovered radiant-fresh 6 plan "C-2026-003 · Omarchy auf 4.0.7 aktualisieren · 4/5 steps · agent: claude-code · red R3"
expect radiant-fresh 7 '[.view.sectionView.slots[] | .chart.hover] | join("")' ""
rpaints radiant-fresh 8 "2,2,2,1,2,1"
expect radiant-fresh 9 .view.window.w 2560
rpaints radiant-fresh 10 "3,3,3,2,3,2"
rfits radiant-fresh 10
expect radiant-fresh 12 '.firstFrame.serviceBefore == .firstFrame.service' true
expect radiant-fresh 12 '[.firstFrame.bare, .firstFrame.section] | map(tostring) | join(",")' "true,0"
expect radiant-fresh 12 '.firstFrame.paints | join(",")' "0,0,0,0,0,0"
expect radiant-fresh 12 .firstFrame.paintedBy 2
expect radiant-fresh 12 '[.view.section, .view.sectionView.period] | join(",")' "radiant,all"
rpaints radiant-fresh 12 "1,1,1,1,1,1"
clean_log radiant-fresh

# 8c. Hover read-outs from real mouse moves onto each chart's items
#     (chart.locate) and from `call hover <slot> <fx>,<fy>` (fractions of
#     the plot; overlay scenario 3). A malformed argument (no such slot, not
#     two numbers, a point outside [0, 1]) returns { error } and leaves the
#     hover as it was.
run radiant-hover "$sample" 1920x1080 \
  "fresh:$radiant;view;hoverItem:heatmap:0;hoverItem:series:1;hoverItem:driftBars:-1;hoverItem:riskDonut:0;hoverItem:riskDonut:2;hoverItem:timeline:2;hoverItem:timeline:11;hoverItem:plan:1;key:Left;hoverItem:heatmap:-2;hoverItem:timeline:12;call:hover:riskDonut 0.99,0.01;call:hover:driftBars 0.97,0.5;call:hover:;call:hover:nope 0.5,0.5;call:hover:driftBars 0.97,0.5;call:hover:series .,.;call:hover:series 1.2.3,0.5;call:hover:series 1.5,0.5;call:hover:series 0.5;call:hover:series .5,1"
rhovered radiant-hover 3 heatmap "Sat 2026-07-04 · 0 events"
rhovered radiant-hover 4 series "2026-09-03 · explicit 324 · total 2005"
rhovered radiant-hover 5 driftBars "2026-W40 · 28 Sep – 4 Oct · opened 6 · resolved 2"
rhovered radiant-hover 6 riskDonut "R0 · 1 case · 13% · all time"
rhovered radiant-hover 7 riskDonut "R2 · 3 cases · 38% · all time"
rhovered radiant-hover 8 timeline "crisis · config config-add ~/.config/omarchy/hooks/post-update.d/backup-dotfiles.sh · 2026-09-29 20:45"
rhovered radiant-hover 9 timeline "case · C-2026-002 Hyprland-Monitorlayout für Dual-WQHD · 2026-09-12 – 2026-09-13"
rhovered radiant-hover 10 plan "C-2026-004 · Zed als zweiten Editor installieren · 2/4 steps · agent: claude-code · red R2"
# A period switch clears the hover of the charts whose data changed; The
# Plan has no period, so its hover (the pointer is still on it) stays.
rhovered radiant-hover 11 plan "C-2026-004 · Zed als zweiten Editor installieren · 2/4 steps · agent: claude-code · red R2"
rhovered radiant-hover 12 heatmap "Wed 2026-09-30 · 8 events · pacman 3 · snapper 3 · manual 1 · seldon 1"
rhovered radiant-hover 13 timeline "case · C-2026-004 Zed als zweiten Editor installieren · 2026-09-28 – open"
expect radiant-hover 14 '.call | fromjson | .slot + "=" + .hover' "riskDonut="
expect radiant-hover 15 '.call | fromjson | .slot + "=" + .hover' "driftBars=2026-W40 · 28 Sep – 4 Oct · opened 6 · resolved 2"
expect radiant-hover 16 '[.view.sectionView.slots[] | .chart.hover] | join("")' ""
expect radiant-hover 17 '.call | fromjson | .error' "no chart nope"
rhovered radiant-hover 18 driftBars "2026-W40 · 28 Sep – 4 Oct · opened 6 · resolved 2"
for step in 19 20 21 22; do
  expect radiant-hover $step '.call | fromjson | .error' 'expected "<slot> <x>,<y>" with x and y in [0, 1], or ""'
  rhovered radiant-hover $step driftBars "2026-W40 · 28 Sep – 4 Oct · opened 6 · resolved 2"
done
expect radiant-hover 23 '.call | fromjson | .slot + "=" + .hover' "series=2026-09-03 · explicit 324 · total 2005"
clean_log radiant-hover

# 8d. Layout (overlay scenarios 4 and 5): the desk at 100 % on 2560×1440
#     and on the 1.25-scaled outputs (logical 1536×864 and 2048×1152; the
#     test host runs 1.25), for every period; every chart has a plot area of
#     its own. Then a 1920×1080 output rendered at QT_SCALE_FACTOR=1.25.
for size in 2560x1440 1536x864 2048x1152; do
  run "radiant-size-$size" "$sample" "$size" "summon:$radiant;key:Left;key:Right;key:Right;key:Right"
  for step in 1 2 3 4 5; do rfits "radiant-size-$size" $step; done
  expect "radiant-size-$size" 1 .view.sectionView.mode wide
  expect "radiant-size-$size" 5 '[.view.sectionView.slots[] | select(.chart.w < 100 or .chart.h < 40)] | length' 0
  rcounts "radiant-size-$size" 2 30 "30,2,5,4,17,2"
  rsummaries "radiant-size-$size" 2 "$s30"
  clean_log "radiant-size-$size"
done
run radiant-scaled "$sample" 1536x864 "summon:$radiant;key:Right" QT_SCALE_FACTOR=1.25
rfits radiant-scaled 1
rfits radiant-scaled 2
rcounts radiant-scaled 2 365 "365,3,5,4,18,2"
clean_log radiant-scaled

# 8e. Narrow desks reflow the grid (overlay scenario 6; WP-123 acceptance):
#     at 50 % on 1920 (the 960 px floor) and on 1366, two columns; on a
#     window under 960 px the desk shows the sidebar's icons and the grid
#     one slot per row and scrolls. No chart leaves its slot at any of them.
#     A change from wide to medium to narrow repaints each chart once per
#     size change (the harness changes one dimension at a time: each is a
#     geometry change of its own) and aggregates nothing.
run radiant-half "$sample" 1920x1080 "summon:$radiant;width:50;resize:1366x768;resize:560x900;key:Down;width:100;resize:1920x1080"
expect radiant-half 1 .view.sectionView.mode wide
expect radiant-half 2 '[.view.desk.w, .view.sectionView.mode] | map(tostring) | join(",")' "960,medium"
expect radiant-half 2 '[.view.sectionView.slots | group_by(.y)[] | map(.id) | join(" ")] | join(" | ")' "heatmap | series driftBars | riskDonut timeline | plan"
expect radiant-half 3 '[.view.desk.w, .view.sectionView.mode] | map(tostring) | join(",")' "960,medium"
expect radiant-half 4 '[.view.desk.w, .view.layout.sidebar, .view.sectionView.mode, .view.sectionView.scrolls] | map(tostring) | join(",")' "550,icons,narrow,true"
expect radiant-half 4 '[.view.sectionView.slots | group_by(.y)[] | map(.id) | join(" ")] | join(" | ")' "heatmap | series | driftBars | riskDonut | timeline | plan"
for step in 1 2 3 4 5 6 7; do rfits radiant-half $step; done
# ↓ scrolls a grid that does not fit.
expect radiant-half 5 '(.view.sectionView.slots[0].y < (.view.sectionView.area.y))' true
expect radiant-half 7 '[.view.sectionView.slots[] | select(.chart.w + 36 > .w)] | length' 0
expect radiant-half 7 .view.sectionView.aggregations.section 0
clean_log radiant-half
run radiant-reflow "$sample" 1920x1080 "fresh:$radiant;view;width:50;view;resize:560x1080;view"
rpaints radiant-reflow 2 "1,1,1,1,1,1"
expect radiant-reflow 3 .view.sectionView.mode medium
rpaints radiant-reflow 4 "2,2,2,2,2,2"
expect radiant-reflow 5 .view.sectionView.mode narrow
rpaints radiant-reflow 6 "3,3,3,3,3,3"
expect radiant-reflow 6 .view.sectionView.aggregations.section 0
clean_log radiant-reflow

# 8f. A logbook that is not initialised (overlay scenario 7): the desk's
#     notice (7b); every chart in its empty state, nothing painted, no hover.
run radiant-uninit "$fx/index-variants/not-initialised.json" 1920x1080 "summon:$radiant;call:hover:heatmap 0.5,0.5;call:hover:timeline 0.5,0.5"
expect radiant-uninit 1 '.view.notices | join(",")' "Logbook not initialised"
rfits radiant-uninit 1
rcounts radiant-uninit 1 90 "0,0,0,0,0,0"
expect radiant-uninit 1 '[.view.sectionView.slots[] | .chart.empty] | all' true
expect radiant-uninit 1 '[.texts[] | select(. == "no data in this period")] | length' 4
shows radiant-uninit 1 "no cases yet · all time"
shows radiant-uninit 1 "no active cases"
rpaints radiant-uninit 1 "0,0,0,0,0,0"
expect radiant-uninit 2 '.call | fromjson | .slot + "=" + .hover' "heatmap="
expect radiant-uninit 3 '.call | fromjson | .slot + "=" + .hover' "timeline="
clean_log radiant-uninit

# 8g. Every other index variant renders every chart (no empty state), one
#     paint each (overlay scenario 8). Where a notice settles under the
#     header in the first frame (index-stale, snapper-degraded) the section
#     gets its final height a frame late and each chart paints once more.
for variant in "$fx"/index-variants/*.json; do
  name=$(basename "$variant" .json)
  [[ $name == not-initialised ]] && continue
  run "radiant-variant-$name" "$variant" 1920x1080 "fresh:$radiant;view"
  expect "radiant-variant-$name" 2 '[.view.sectionView.slots[] | .chart.empty] | any' false
  if [[ $name == index-stale || $name == snapper-degraded ]]; then
    expect "radiant-variant-$name" 2 '[.view.sectionView.slots[] | .chart.paints] | min >= 1 and max <= 2' true
  else
    rpaints "radiant-variant-$name" 2 "1,1,1,1,1,1"
  fi
  rfits "radiant-variant-$name" 2
  clean_log "radiant-variant-$name"
done

# ---------------------------------------------------------------------------
# 9. Decisions, System, Memory (sections 4–6; ADR-0034 §2, WP-123): the 0.1
#    panel's scenarios for these tabs (panel-view.sh 5–7, 20–23; COVERAGE.md)
#    on the desk. `.view.sectionView`: rows (ids), cursor (the row the
#    detail shows), the enabled actions of the sticky bar and the section's
#    own keys.
# 9a. The sample, dev mode (read-only): `4` lists the four decisions newest
#     first, the proposed one striped and selected, its detail with Accept
#     and Open in editor and what Accept means; ↓/↑ move the selection and
#     the detail follows (an accepted one has no Accept); a click on a row
#     selects it; `e` and Accept are refused with dev mode's reason; `d`
#     opens no form without an engine to write; `/` narrows the list, Esc
#     clears it; `call select` picks a decision ("not found" for none). The index's contract 1 has no decision cases: no CASES
#     block.
run decisions "$sample" 1920x1080 \
  "summon;text:4;key:Down;key:Down*5;key:Up;click:Zed statt VS Code als Zweiteditor;text:e;key:Up;click:Accept;text:d;text:/;type:snap;key:Return;key:Escape;call:select:ADR-0001;call:select:ADR-0999"
expect decisions 2 .view.section decisions
expect decisions 2 '.view.sectionView.rows | join(",")' "ADR-0004,ADR-0003,ADR-0002,ADR-0001"
expect decisions 2 '[.view.sectionView.cursor, .view.selected] | join(",")' "ADR-0004,ADR-0004"
expect decisions 2 '.view.sectionView.actions | join(",")' "Accept,Open in editor"
expect decisions 2 .view.sectionView.cases null
for text in "DECISIONS" "4 decisions · 1 proposed" "New decision" "Ollama nur als User-Service mit Case" "ADR-0004 · proposed" \
  "2026-10-01" "Logbuch-Sprache Deutsch, Struktur Englisch" "ADR-0001 · accepted" "2026-09-01" \
  "ADR-0004 · PROPOSED · 2026-10-01" "Accept" "Open in editor" "decisions/ADR-0004-ollama-user-service.md" \
  "Proposed: it waits for your decision. Accept opens it in the editor; set status: accepted in its frontmatter, and the index follows on the next capture." \
  "The text is in the file; Open in editor shows it."; do
  shows decisions 2 "$text"
done
expect decisions 2 '[.texts[] | select(startswith("CASES"))] | length' 0
expect decisions 3 '[.view.sectionView.cursor, .view.sectionView.actions[0]] | join(",")' "ADR-0003,Open in editor"
expect decisions 3 '[.texts[] | select(. == "Accept")] | length' 0
expect decisions 4 .view.sectionView.cursor ADR-0001
expect decisions 5 .view.sectionView.cursor ADR-0002
expect decisions 6 '[.view.sectionView.cursor, .view.keys] | map(tostring) | join(",")' "ADR-0003,true"
expect decisions 7 .view.sectionView.openResult "dev mode (SELDON_INDEX): engine calls are disabled"
expect decisions 9 .view.sectionView.cursor ADR-0004
expect decisions 9 .view.sectionView.openResult "dev mode (SELDON_INDEX): engine calls are disabled"
expect decisions 10 '[.view.sectionView.form.open, .view.editing] | map(tostring) | join(",")' "false,false"
expect decisions 13 '[.view.search.text, (.view.sectionView.rows | join(",")), .view.sectionView.cursor, .view.sectionView.filtered] | map(tostring) | join("|")' "snap|ADR-0002|ADR-0002|true"
expect decisions 14 '[.view.search.text, (.view.sectionView.rows | length), .view.opened] | map(tostring) | join(",")' ",4,true"
expect decisions 15 '[.call, .view.selected] | join(",")' "ok,ADR-0001"
expect decisions 16 '[.call, .view.selected] | join(",")' "not found,ADR-0001"
for i in 2 3 9 13; do expect decisions $i '.overflow | join(" | ")' ""; done
clean_log decisions

# 9b. A decision that names cases (contract v2, `decisions[].cases`; the
#     plugin reads the field wherever it is): the CASES block lists them
#     with their titles from the index, a case the index no longer lists by
#     its id; a click goes to the case in Work. A decision with an empty
#     list says so.
jq '.decisions[0].cases = ["C-2026-003", "C-2026-099"] | .decisions[1].cases = []' "$sample" >"$work/decision-cases.json"
run decision-cases "$work/decision-cases.json" 1920x1080 "summon;text:4;key:Down;key:Up;click:Omarchy auf 4.0.7 aktualisieren"
expect decision-cases 2 '.view.sectionView.cases | join(",")' "C-2026-003,C-2026-099"
for text in "CASES · 2" "Omarchy auf 4.0.7 aktualisieren" "C-2026-003 · active" "C-2026-099" "not in the index"; do
  shows decision-cases 2 "$text"
done
expect decision-cases 3 '.view.sectionView.cases | length' 0
shows decision-cases 3 "This decision names no case."
expect decision-cases 5 .view.section work
clean_log decision-cases

# 9c. Live, against the fake engine, with real keys (panel scenario 21): `d`
#     opens the form in the detail pane and gives it the keys; the title
#     `--help "q"` is typed into it (no section switch, no digit); Enter
#     arms ("Press Enter again: …"), a change to the title disarms, Enter
#     twice creates it: `decide --no-edit --json -- <title>`, then `open
#     ADR-0005 --editor --json` from the answer; the form closes, the keys
#     come back and the selection sits on ADR-0005 once the index lists it.
#     Then Accept on the proposed ADR-0004, `e` and Open in editor on
#     ADR-0003; on Memory `e` and Open in editor open the logbook, on
#     System STATUS.md. The exact argv and editor paths.
mkdir -p "$work/home-decisions-live"
run decisions-live "" 1920x1080 \
  "summon;text:4;text:d;type:--help \"q\";key:Return;key:Backspace;type:\";key:Return;key:Return;settle;wait:sectionView.cursor=ADR-0005;key:Down;click:Accept;settle;key:Down;text:e;settle;click:Open in editor;settle;text:6;key:Down;text:e;settle;click:Open in editor;settle;text:5;text:e;settle" \
  HOME="$work/home-decisions-live" FAKE_SELDON_FIXTURE="$sample" HARNESS_RECORD="$work/decisions-live.record"
expect decisions-live 3 '[.view.sectionView.form.open, .view.sectionView.form.editing, .view.editing, .view.keys] | map(tostring) | join(",")' "true,true,true,false"
shows decisions-live 3 "New decision"
shows decisions-live 3 "NEW DECISION"
shows decisions-live 3 "Title, Enter twice creates the decision"
expect decisions-live 3 '[.texts[] | select(. == "Accept")] | length' 0
expect decisions-live 4 '[.view.sectionView.form.title, .view.section] | join(",")' '--help "q",decisions'
expect decisions-live 5 .view.sectionView.form.armed true
expect decisions-live 5 .view.sectionView.form.hint 'Press Enter again: create the decision “--help "q"”'
shows decisions-live 5 'Press Enter again: create the decision “--help \"q\"”'
expect decisions-live 6 '[.view.sectionView.form.armed, .view.sectionView.form.title] | map(tostring) | join(",")' 'false,--help "q'
expect decisions-live 7 .view.sectionView.form.armed false
expect decisions-live 8 .view.sectionView.form.armed true
expect decisions-live 11 '[.view.sectionView.form.open, .view.sectionView.form.editing, .view.sectionView.form.title, .view.keys] | map(tostring) | join(",")' "false,false,,true"
expect decisions-live 11 .view.sectionView.result 'Created ADR-0005 · --help "q"'
expect decisions-live 11 '.view.sectionView.rows | join(",")' "ADR-0005,ADR-0004,ADR-0003,ADR-0002,ADR-0001"
shows decisions-live 11 'Created ADR-0005 · --help \"q\"'
shows decisions-live 11 "5 decisions · 2 proposed"
expect decisions-live 12 .view.sectionView.cursor ADR-0004
expect decisions-live 14 .view.sectionView.openResult "Opened $work/home-decisions-live/Seldon/decisions/ADR-0004-ollama-user-service.md in omarchy-launch-editor"
expect decisions-live 15 .view.sectionView.cursor ADR-0003
expect decisions-live 17 .view.sectionView.openResult "Opened $work/home-decisions-live/Seldon/decisions/ADR-0003-zed.md in omarchy-launch-editor"
expect decisions-live 19 .view.sectionView.openResult "Opened $work/home-decisions-live/Seldon/decisions/ADR-0003-zed.md in omarchy-launch-editor"
expect decisions-live 21 .view.sectionView.cursor "lesson:Theme-Overrides nie im Omarchy-Repo"
expect decisions-live 23 .view.sectionView.openResult "Opened $work/home-decisions-live/Seldon in omarchy-launch-editor"
expect decisions-live 28 .view.sectionView.openResult "Opened $work/home-decisions-live/Seldon/STATUS.md in omarchy-launch-editor"
expect decisions-live 28 .view.lastError ""
want=$(printf '%s\n' "$(q --version --json)" "$(q capture --all --json --quiet)" "$(q status --json)" \
  "$(q decide --no-edit --json -- '--help "q"')" "$(q open ADR-0005 --editor --json)" \
  "$(q open ADR-0004 --editor --json)" "$(q open ADR-0003 --editor --json)" "$(q open ADR-0003 --editor --json)" \
  "$(q open logbook --editor --json)" "$(q open logbook --editor --json)" "$(q open status --editor --json)")
got=$(grep -v '^doctor' "$work/home-decisions-live/argv.log" 2>/dev/null || true)
check "decisions-live: engine argv" "$got" "$want"
d="$work/home-decisions-live/Seldon"
want=$(printf '%s\n' omarchy-launch-editor "$d/decisions/ADR-0005-help-q.md" -- \
  omarchy-launch-editor "$d/decisions/ADR-0004-ollama-user-service.md" -- \
  omarchy-launch-editor "$d/decisions/ADR-0003-zed.md" -- omarchy-launch-editor "$d/decisions/ADR-0003-zed.md" -- \
  omarchy-launch-editor "$d" -- omarchy-launch-editor "$d" -- omarchy-launch-editor "$d/STATUS.md" --)
check "decisions-live: editor paths" "$(cat "$work/decisions-live.record" 2>/dev/null || true)" "$want"
clean_log decisions-live

# 9d. Refusals keep the title (panel scenario 22): Enter on a blank title is
#     refused in the plugin; the engine refuses the decision (lock held, exit
#     4): the form shows its message and keeps the title, nothing is opened;
#     Esc gives the keys back and the list head shows the refusal; `d`
#     brings the form back with the title; a click on a decision shows it
#     instead and gives the keys back, so the next Esc closes the desk.
mkdir -p "$work/home-decisions-locked"
run decisions-locked "" 1920x1080 "summon;text:4;text:d;key:Return;type:keep me;key:Return;key:Return;settle;key:Escape;view;text:d;click:Zed statt VS Code als Zweiteditor;key:Escape" \
  HOME="$work/home-decisions-locked" FAKE_SELDON_FIXTURE="$sample" FAKE_SELDON_LOCKED=1
expect decisions-locked 4 .view.sectionView.form.result "Give the decision a title"
expect decisions-locked 4 .view.sectionView.form.armed false
expect decisions-locked 5 .view.sectionView.form.result ""
expect decisions-locked 8 .view.sectionView.form.result "the logbook is locked by another seldon (pid 4242)"
expect decisions-locked 8 '[.view.sectionView.form.title, .view.sectionView.form.editing] | map(tostring) | join(",")' "keep me,true"
shows decisions-locked 8 "the logbook is locked by another seldon (pid 4242)"
expect decisions-locked 9 '[.view.sectionView.form.open, .view.sectionView.form.editing, .view.keys] | map(tostring) | join(",")' "false,false,true"
expect decisions-locked 10 .view.sectionView.result "the logbook is locked by another seldon (pid 4242)"
shows decisions-locked 10 "the logbook is locked by another seldon (pid 4242)"
expect decisions-locked 10 .view.lastError ""
expect decisions-locked 11 '[.view.sectionView.form.open, .view.sectionView.form.title, .view.sectionView.form.editing] | map(tostring) | join(",")' "true,keep me,true"
expect decisions-locked 12 '[.view.sectionView.form.open, .view.sectionView.form.editing, .view.keys, .view.sectionView.cursor] | map(tostring) | join(",")' "false,false,true,ADR-0003"
expect decisions-locked 13 .view.opened false
want=$(printf '%s\n' "$(q --version --json)" "$(q capture --all --json --quiet)" "$(q status --json)" \
  "$(q decide --no-edit --json -- "keep me")")
check "decisions-locked: engine argv" "$(grep -v '^doctor' "$work/home-decisions-locked/argv.log" 2>/dev/null || true)" "$want"
clean_log decisions-locked "seldon decide exit 4: the logbook is locked by another seldon"

# 9e. System on the sample (panel scenarios 1 and 6): `5` lists the five
#     tiles with their big values; the detail shows the big value, the lead,
#     the rows and where they come from; ↓ walks the tiles; `e` and Open in
#     editor ask for STATUS.md (refused in dev mode). Every system field is
#     optional: an empty `system` and a sparse one give "—" tiles that say
#     so, and the Collectors tile keeps machine, engine and index time; a
#     failing collector stripes its tile.
run system "$sample" 1920x1080 "summon;text:5;key:Down;key:Down;key:Down;key:Down;text:e;click:Open in editor"
expect system 2 .view.section system
expect system 2 '.view.sectionView.tiles | join(",")' "omarchy 4.0.7-1,packages 2009 installed,snapshots 115 newest,deviations 5 files,collectors 6/6 ok"
expect system 2 '[.view.sectionView.cursor, .view.sectionView.big, .view.sectionView.actionMeta] | join(",")' "omarchy,4.0.7-1,STATUS.md"
for text in "SYSTEM" "Omarchy" "Packages" "Snapshots" "Deviations" "Collectors" "2009 installed" "6/6 ok" \
  "OMARCHY" "4.0.7-1" "theme tokyo-night · updated 7 h ago" "Theme" "tokyo-night" "Plugins" "33 of 40 enabled" \
  "Open in editor" "STATUS.md" "From the dossier; rebuilt on every capture."; do
  shows system 2 "$text"
done
expect system 3 '[.view.sectionView.big, (.view.sectionView.detailRows | join(","))] | join("|")' "2009|Explicit,Installed,AUR"
shows system 3 "installed"
shows system 3 "327 explicit · 41 from the AUR"
expect system 4 '[.view.sectionView.big, (.view.sectionView.detailRows | length)] | map(tostring) | join("|")' "115|6"
shows system 4 "2026-10-01 16:30 · tailscale: MagicDNS · post"
expect system 5 '[.view.sectionView.big, (.view.sectionView.detailRows | length)] | map(tostring) | join("|")' "5|0"
shows system 5 "Config files that differ from Omarchy's defaults; the list is in STATUS.md"
expect system 6 '.view.sectionView.detailRows | join(",")' "pacman,snapper,omarchy,plugins,theme,config,Machine,Engine,Index written,Area dev-env,Area hyprland,Area packages,Area plugins,Area shell,Area themes"
shows system 6 "1 case · AGENTS.md"
expect system 7 .view.sectionView.openResult "dev mode (SELDON_INDEX): engine calls are disabled"
expect system 8 .view.sectionView.openResult "dev mode (SELDON_INDEX): engine calls are disabled"
for i in 2 3 4 5 6; do expect system $i '.overflow | join(" | ")' ""; done
clean_log system
jq '.system = {} | del(.state.collectors)' "$sample" >"$work/system-empty.json"
run system-empty "$work/system-empty.json" 1920x1080 "summon;text:5;key:Down*4"
expect system-empty 2 '.view.sectionView.tiles | join(",")' "omarchy —,packages —,snapshots —,deviations —,collectors —"
shows system-empty 2 "Not in the index"
expect system-empty 3 '.view.sectionView.detailRows | join(",")' "Machine,Engine,Index written"
clean_log system-empty
jq '.system = {packages: {aur: 3}, deviations: 1} | del(.state.collectors)' "$sample" >"$work/system-sparse.json"
run system-sparse "$work/system-sparse.json" 1920x1080 "summon;text:5;key:Down"
expect system-sparse 2 '.view.sectionView.tiles | join(",")' "omarchy —,packages —,snapshots —,deviations 1 file,collectors —"
expect system-sparse 3 '.view.sectionView.detailRows | join(",")' "AUR"
shows system-sparse 3 "3 from the AUR"
clean_log system-sparse
run system-degraded "$fx/index-variants/snapper-degraded.json" 1920x1080 "summon;text:5;key:Down*4"
expect system-degraded 3 .view.sectionView.big "5/6"
shows system-degraded 3 "5/6 ok"
expect system-degraded 3 '[.texts[] | select(startswith("1 collector failing"))] | length' 1
clean_log system-degraded

# 9f. Memory on the sample (panel scenario 20): `6` lists three lessons and
#     two topics; the detail names the file and, for a topic, its date; `e`
#     and Open in editor open the logbook (refused in dev mode).
run memory "$sample" 1920x1080 "summon;text:6;key:Down*3;key:Down;text:e"
expect memory 2 .view.section memory
expect memory 2 '.view.sectionView.rows | map(split(":")[0]) | join(",")' "lesson,lesson,lesson,topic,topic"
expect memory 2 .view.sectionView.summary "3 lessons · 2 topics"
for text in "MEMORY" "3 lessons · 2 topics" "Theme-Overrides nie im Omarchy-Repo" "lesson" "omarchy" \
  "memory/omarchy.md · updated 2026-10-01" "hyprland" "memory/hyprland.md · updated 2026-09-13" "LESSON" "memory/lessons.md" \
  "Every agent reads this at the start of a session. The index carries the headings; the text is in the logbook's memory/ folder." \
  "Open in editor" "memory/"; do
  shows memory 2 "$text"
done
expect memory 3 .view.sectionView.cursor "topic:omarchy"
shows memory 3 "TOPIC"
shows memory 3 "Updated"
shows memory 3 "2026-10-01"
expect memory 4 .view.sectionView.cursor "topic:hyprland"
expect memory 5 .view.sectionView.openResult "dev mode (SELDON_INDEX): engine calls are disabled"
for i in 2 3 4; do expect memory $i '.overflow | join(" | ")' ""; done
clean_log memory

# 9g. Not initialised (panel scenario 5): sections 4–6 say there is no
#     index, `d` opens no form, `e` runs nothing; the Prime Radiant is 8f.
mkdir -p "$work/home-uninit-sections"
run uninit-sections "" 1920x1080 "summon;text:4;text:d;text:e;text:5;text:e;text:6;text:e;view" \
  HOME="$work/home-uninit-sections" FAKE_SELDON_MODE=uninit
expect uninit-sections 2 '[.view.status, (.view.sectionView.rows | length), .view.sectionView.form.open] | map(tostring) | join(",")' "notInitialised,0,false"
shows uninit-sections 2 "No index to show"
expect uninit-sections 2 '[.texts[] | select(. == "Accept" or . == "Open in editor")] | length' 0
expect uninit-sections 3 .view.sectionView.form.open false
expect uninit-sections 5 '.view.sectionView.rows | length' 0
shows uninit-sections 5 "No index to show"
expect uninit-sections 7 '.view.sectionView.rows | length' 0
shows uninit-sections 7 "No index to show"
check "uninit-sections: no open, no decide" "$(grep -c -E '^(open|decide) ' "$work/home-uninit-sections/argv.log" 2>/dev/null || true)" 0
clean_log uninit-sections "seldon capture exit 3: logbook not initialised"

# 9h. The stacked layout (a window under 770 px; WP-121's case 6 for these
#     sections): the list; Enter shows the detail with its back row and the
#     sticky bar; Esc goes back to the list; `d` shows the form as the
#     detail (dev mode: no form); the back row returns; a row click shows
#     its detail. Nothing leaves the window or the desk.
run stacked-sections "$sample" 700x900 "summon;text:4;key:Down;key:Return;key:Escape;text:5;key:Return;clickName:deskBack;text:6;click:hyprland"
expect stacked-sections 2 '[.view.layout.stacked, .view.detailShown] | map(tostring) | join(",")' "true,false"
expect stacked-sections 2 '[.texts[] | select(. == "Open in editor")] | length' 0
expect stacked-sections 4 '[.view.detailShown, .view.sectionView.cursor] | map(tostring) | join(",")' "true,ADR-0003"
shows stacked-sections 4 "‹ Back to the list"
shows stacked-sections 4 "Open in editor"
expect stacked-sections 5 '[.view.detailShown, .view.opened] | map(tostring) | join(",")' "false,true"
expect stacked-sections 7 '[.view.section, .view.detailShown] | map(tostring) | join(",")' "system,true"
shows stacked-sections 7 "4.0.7-1"
expect stacked-sections 8 .view.detailShown false
expect stacked-sections 10 '[.view.detailShown, .view.sectionView.cursor] | map(tostring) | join(",")' "true,topic:hyprland"
for i in 2 3 4 5 6 7 8 9 10; do expect stacked-sections $i '.overflow | join(" | ")' ""; done
clean_log stacked-sections

# 9i. Label fit (panel scenario 23, the desk's part): sections 4–7 on a
#     1366 px screen at 50 % (the 960 px floor) and at 100 %, and on 3840 px:
#     no text leaves its box, slot, the desk or the window.
for W in 1366 3840; do
  run "fit-$W" "$sample" "${W}x1080" "summon;width:50;text:4;text:5;key:Down*4;text:6;text:7;width:100;text:4;text:5;text:6;text:7"
  for i in 3 4 5 6 7 9 10 11 12; do expect "fit-$W" $i '.overflow | join(" | ")' ""; done
  clean_log "fit-$W"
done

# ---------------------------------------------------------------------------
# Offscreen renders in three themes (only with DESK_SHOTS; not live
# screenshots): Today at 100 % and 50 %, Settings, Decisions, System,
# Memory, the Prime Radiant at 100 % and 50 % (and a hover), and a
# not-initialised logbook with its notice.
if [[ -n ${DESK_SHOTS:-} ]]; then
  mkdir -p "$DESK_SHOTS"
  for theme in tokyo-night kanagawa catppuccin-latte; do
    home="$work/home-shot-$theme"
    mkdir -p "$home/.local/state/omarchy/current/theme"
    cp "$omarchy/themes/$theme/colors.toml" "$home/.local/state/omarchy/current/theme/colors.toml"
    run "shot-$theme" "$sample" 1920x1080 \
      "summon;shot:desk-$theme-100;width:50;shot:desk-$theme-50;width:100;text:,;shot:desk-$theme-settings;text:2;view" \
      HOME="$home" HARNESS_SHOTS="$DESK_SHOTS"
    expect "shot-$theme" 6 .view.section settings
    clean_log "shot-$theme"
    run "shot-sections-$theme" "$sample" 1920x1080 \
      "summon;text:4;shot:desk-$theme-decisions;text:5;shot:desk-$theme-system;text:6;shot:desk-$theme-memory;text:7;shot:desk-$theme-radiant;key:Right;hoverItem:heatmap:-1;shot:desk-$theme-radiant-365d-hover;width:50;shot:desk-$theme-radiant-50;view" \
      HOME="$home" HARNESS_SHOTS="$DESK_SHOTS"
    rfits "shot-sections-$theme" 15
    clean_log "shot-sections-$theme"
    run "shot-uninit-$theme" "$fx/index-variants/not-initialised.json" 1920x1080 "summon;shot:desk-$theme-uninit" \
      HOME="$home" HARNESS_SHOTS="$DESK_SHOTS"
    clean_log "shot-uninit-$theme"
  done
fi

real_home_check desk-view

echo "desk-view: $pass passed, $fail failed"
((fail == 0))
