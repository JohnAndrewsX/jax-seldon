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
# counters, hover read-outs and grid at every desk width), the graph
# (WP-125; its layout ticks and their time, replay, hover, drag). The old
# panel's and overlay's scenarios and where each went are listed in
# tests/plugin/COVERAGE.md.
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
# Needs quickshell, jq, node and the installed shell (host check; docs/TESTING.md).
set -euo pipefail

root=$(cd "$(dirname "$0")/../.." && pwd)
fx="$root/fixtures"
omarchy="${OMARCHY_PATH:-/usr/share/omarchy}"
shell_dir="$omarchy/shell"

qs_bin=$(command -v quickshell || command -v qs || true)
[[ -n $qs_bin ]] || { echo "desk-view: quickshell not found" >&2; exit 1; }
command -v jq >/dev/null || { echo "desk-view: jq not found" >&2; exit 1; }
command -v node >/dev/null || { echo "desk-view: node not found" >&2; exit 1; }
[[ -d $shell_dir/Commons && -d $shell_dir/Ui ]] || { echo "desk-view: shell not found at $shell_dir" >&2; exit 1; }
timeout_bin=$(command -v timeout) || { echo "desk-view: timeout not found" >&2; exit 1; }

work=$(mktemp -d)
# Quickshell's runtime dir (its by-id/<id> logs, the IPC socket): private,
# never the session's, and short, since a unix socket path has at most 107
# bytes and $work follows TMPDIR (WP-161).
rt=$(mktemp -d /tmp/seldon-rt.XXXXXX)
chmod 700 "$rt"
trap 'rm -rf "$work" "$rt"' EXIT
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
for tool in bash env cat sed date mkdir mv sleep basename grep jq rm touch; do
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
    XDG_RUNTIME_DIR="$rt" \
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
# 7a. Snapper not readable (ADR-0026, WP-054): the notice with Grant, Copy
#     and Check again (WP-117): one sentence, the plain command; the
#     engine's message and what the grant gives on hover. Grant opens the
#     terminal script and adds no hint. On a narrow desk the chip's title
#     does not fit beside the KPI strip: it says "1 notice". Grant opens a
#     window, so the desk steps aside at once (WP-156); the case opens it
#     again.
run snapper "$fx/index-variants/snapper-degraded.json" 1920x1080 \
  "summon;click:Grant;summon;hover:Read snapshots (optional);wait:snapperTip.shown=true;view;resize:1000x900" \
  HARNESS_RECORD="$work/snapper.record"
expect snapper 1 '.view.notices | join(",")' "Read snapshots (optional)"
expect snapper 1 .view.chip "Read snapshots (optional)"
shows snapper 1 'sudo setfacl -m u:$USER:rx /.snapshots'
shows snapper 1 "A one-time read grant on /.snapshots; it asks for your password once, and Seldon works without it."
shows snapper 1 "Grant"
shows snapper 1 "Copy"
shows snapper 1 "Check again"
expect snapper 1 '[.texts[] | select(. == "Run in terminal")] | length' 0
expect snapper 1 '[.texts[] | select(contains("snapshot directory listing"))] | length' 0
expect snapper 2 '[.view.opened, .service.stepAsides] | map(tostring) | join(",")' "false,1"
expect snapper 2 '.calls | map(select(startswith("hide"))) | length' 1
expect snapper 3 '[.texts[] | select(startswith("When the command has finished"))] | length' 0
snapper_grants="The command below grants your user read access to the snapshot directory listing and the snapshot info files (files inside a snapshot keep their own permissions), nothing else: no snapshot creation, change or deletion."
snapper_message=$(jq -r '.state.collectors[] | select(.name == "snapper") | .message' "$fx/index-variants/snapper-degraded.json")
expect snapper 1 .view.snapperTip.shown false
expect snapper 6 .view.snapperTip.text "$snapper_message"$'\n'"$snapper_grants"
expect snapper 6 .view.snapperTip.shown true
expect snapper 6 .view.snapperTip.fits true
# the launcher's argv is the grant script, verbatim (model.test.js pins its text)
script=$(node -e '
  const fs = require("fs"), vm = require("vm"), M = {}
  vm.createContext(M)
  vm.runInContext(fs.readFileSync(process.argv[1], "utf8"), M)
  process.stdout.write(M.SNAPPER_FIX_SCRIPT)' "$root/plugin/Model.js")
deadline=$((SECONDS + 15))
until [[ -s $work/snapper.record ]] || ((SECONDS >= deadline)); do sleep 0.2; done
check "snapper: Grant opened the terminal with the grant script" \
  "$(cat "$work/snapper.record" 2>/dev/null || true)" \
  "$(printf '%s\n' omarchy-launch-floating-terminal-with-presentation "$script" --)"
expect snapper 1 .view.chipShown "▾ Read snapshots (optional)"
expect snapper 7 .view.chipShown "▾ 1 notice"
expect snapper 7 '.overflow | join(" | ")' ""
clean_log snapper

# 7b. Not initialised: the status notice with its pictogram's fix, no KPI
#     figures, no counts; the chip folds and unfolds the notices.
run uninit "$fx/index-variants/not-initialised.json" 1920x1080 "summon;clickName:deskChip;clickName:deskChip"
expect uninit 1 .view.status notInitialised
expect uninit 1 '.view.notices | join(",")' "Create your logbook"
expect uninit 1 '.view.kpis | length' 0
expect uninit 1 '[.view.counts[] | .text] | join("")' ""
shows uninit 1 "Sets up your logbook and starts recording; the terminal asks a few questions, no password."
shows uninit 1 "seldon init"
shows uninit 1 "Create"
expect uninit 2 .view.noticesFolded true
expect uninit 2 '[.texts[] | select(. == "Sets up your logbook and starts recording; the terminal asks a few questions, no password.")] | length' 0
shows uninit 2 "▸ Create your logbook"
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
expect restart-updated 1 '.view.notices | join(",")' "Restart the shell to finish the update,Create your logbook"
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
# 8. Sections 1–3: Today, Changelog, Work (WP-122; ADR-0034 §2). The 0.1
#    panel's scenarios for these tabs, one to one (tests/plugin/COVERAGE.md
#    names each successor), on the sample in dev mode (read-only) and live
#    against the fake engine, with real keys and clicks. `sectionView` is
#    the current section's view(); the engine's argv is compared argument
#    by argument with the CONTRACT.md forms.
expected_warnings='jax\.seldon: seldon (rules exit 1: AGENTS\.md is not UTF-8 text$|log exit 1: unknown case C-2026-004$|plan exit 1: C-2026-008 is active; |agent exit 1: C-2026-004 is queued; |agent exit 1: no default agent|(plan|drift|decide) exit 4: the logbook is locked by another seldon \(pid 4242\)$|capture exit 4: another seldon process holds the lock )'
q() { printf '%q ' "$@"; }
# argv_check <case> <home> <want lines>
argv_check() {
  local got
  got=$(cat "$2/argv.log" 2>/dev/null || true)
  if [[ $got == "$3" ]]; then
    pass=$((pass + 1)); echo "ok   $1: engine argv"
  else
    fail=$((fail + 1)); echo "FAIL $1: engine argv differs"; diff <(echo "$3") <(echo "$got") | sed 's/^/     /' || true
  fi
}
startup=$(printf '%s\n' "$(q --version --json)" "$(q capture --all --json --quiet)" "$(q status --json)")
THEME=01M3VTGNY0NZG4AY80814WSKGR UNIT=01M3VNJ9JGZ9169T01XCW16FT0 OLLAMA=01M3VNFTF8EVHWFFZ687N14Q0C
HOOK_EVENT=01M3Q7R0Z08ZD5R76DQA3PHQ1G
MESA=01M3H6M720FC6BAG7ETNQTXW9K LIB32=01M3H6M8184NVTFDTEGPD71P5H VULKAN=01M3H6M818EPKV6HMJ0GN4PGFG
tv='.view.sectionView'
td="$tv.detail"
tf="$td.form"
tc="$tv.case"
ts="$tv.sheet"
cl='{"section":"changelog"}'
wk='{"section":"work"}'
sel() { printf '{"section":"changelog","select":"%s"}' "$1"; }

# 8a. Today on the sample (panel 1, 3, 13b): the date and the day's state,
#     the tiles, NEEDS YOU (the crises: the 0.1 red strip's successor),
#     the journal, the overview with the active cases and New case; a
#     crisis row shows the event with "why loud"; the yesterday row opens
#     in place; a case tile opens the case in Work.
run today "$sample" 1920x1080 "summon;key:Down;key:Down;key:Down*5;key:Return;key:Down;click:Zed als zweiten Editor installieren"
expect today 1 .view.section today
expect today 1 "$tv.state" crisis
expect today 1 "$tv.tiles | join(\",\")" "events today 33,7 days 54"
expect today 1 "$tv.needs | join(\",\")" "$UNIT,$HOOK_EVENT"
expect today 1 "[$tv.entries, $tv.yesterday, $tv.rows] | map(tostring) | join(\",\")" "4,1,7"
expect today 1 "[$tv.selected, $tv.shown] | join(\",\")" ",overview"
expect today 1 "$tv.cases | join(\",\")" "C-2026-003,C-2026-004"
for text in "Thursday, 1 Oct 2026" "Seldon is recording. 2 changes need you." "NEEDS YOU" "JOURNAL" "ollama.service" \
  "09:25 · claude-code · C-2026-003" "▸ Yesterday · 1 entry" "ACTIVE CASES" "C-2026-003 · R3" "4/5 steps · claude-code" \
  "NEW CASE" "Dev mode is read-only" "events today" "33"; do
  shows today 1 "$text"
done
expect today 2 "[$tv.selected, $tv.shown, $tv.detail.cls] | join(\",\")" "$UNIT,event,crisis"
expect today 2 "$tv.detail.actions | join(\",\")" "Link to case…,Explain…,Dismiss…"
shows today 2 "Why loud?"
# the rule is the index's (ADR-0038 §1): known in dev mode too, no engine call
shows today 2 "The path matches your crisis list ([drift] alwaysRedPaths in ~/.config/seldon/config.toml). No open case plans it, and no case is linked."
shows today 2 "~/.config/systemd/user/ollama.service"
expect today 3 "$tv.selected" "$HOOK_EVENT"
expect today 4 "[$tv.cursor, $tv.selected, $tv.shown] | map(tostring) | join(\",\")" "6,toggle,overview"
expect today 5 "$tv.rows" 8
shows today 5 "▾ Yesterday · 1 entry"
expect today 6 "$tv.cursor" 7
shows today 6 "Snapshots aufgeräumt, 108 und 109 gelöscht."
expect today 7 "[.view.section, .view.selected] | join(\",\")" "work,C-2026-004"
for i in 1 2 5 7; do expect today $i '.overflow | join(" | ")' ""; done
clean_log today

# 8a'. One event today: the tile's singular (WP-117, panel 4b).
jq '.summary.eventsToday = 1' "$sample" >"$work/one-event.json"
run one-event "$work/one-event.json" 1920x1080 "summon"
expect one-event 1 "$tv.tiles | join(\",\")" "event today 1,7 days 54"
shows one-event 1 "event today"
expect one-event 1 '[.texts[] | select(. == "events today")] | length' 0
clean_log one-event

# A crisis resolved from Today stays shown with the engine's answer after
# it leaves NEEDS YOU (live).
mkdir -p "$work/home-today-resolve"
run today-resolve "" 1920x1080 "summon;key:Down;key:Return;type:hook test;key:Return;key:Return;wait:sectionView.detail.form.isOpen=false;settle" \
  HOME="$work/home-today-resolve" FAKE_SELDON_FIXTURE="$sample"
expect today-resolve 3 "[$tv.detail.form.shown, $tv.detail.form.action, .view.keys] | map(tostring) | join(\",\")" "true,explain,false"
expect today-resolve 8 "[$tv.selected, $tv.shown, ($tv.needs | join(\"+\")), $tv.headline] | join(\",\")" "$UNIT,event,$HOOK_EVENT,Seldon is recording. 1 change needs you."
expect today-resolve 8 "[$tv.detail.form.result, ($tv.detail.actions | join(\"+\")), .view.keys] | map(tostring) | join(\",\")" "Explained 1 event · created C-2026-009,Open case,true"
argv_check today-resolve "$work/home-today-resolve" "$(printf '%s\n' "$startup" "$(q drift explain $UNIT --json -- "hook test")")"
clean_log today-resolve

# The sidebar search filters Today's crises and entries (yesterday's too).
run today-search "$sample" 1920x1080 "summon;text:/;type:ollama;key:Return;key:Down;key:Escape"
expect today-search 4 "[$tv.rows, .view.search.text, .view.keys] | map(tostring) | join(\",\")" "2,ollama,true"
expect today-search 5 "[$tv.selected, $tv.shown] | join(\",\")" "$UNIT,event"
expect today-search 6 "[.view.search.text, $tv.rows] | map(tostring) | join(\",\")" ",7"
clean_log today-search

jq '.events = [{id: "01M3W2NEWEVENT000000000000", ts: "2026-10-01T18:30:00+02:00", source: "manual", kind: "note",
  subject: "journal", detail: "Written by the harness after Capture now", zone: "green", actor: "human", case: null}] + .events' \
  "$sample" >"$work/after.json"
mkdir -p "$work/home-today-live"
run today-live "" 1920x1080 \
  "summon;text:n;type:  --help 2 ;key:Return;settle;type:   ;key:Return;key:Backspace*3;key:Tab;key:Down;key:Down;key:Down;key:Return;key:Backtab;type:for the case;key:Return;settle;key:Escape;text:e;settle;summon;text:2;text:e;settle;summon;text:c;wait:sectionView.chips.5=all 83;settle" \
  HOME="$work/home-today-live" FAKE_SELDON_FIXTURE="$sample" FAKE_SELDON_FIXTURE_AFTER="$work/after.json" \
  FAKE_SELDON_WRITTEN=1 HARNESS_RECORD="$work/today-live.record"
tj="$tv.journal"
expect today-live 1 "[$tj.enabled, $tj.cases, $tj.editing] | map(tostring) | join(\",\")" "true,7,false"
shows today-live 1 "Note for today's journal, Enter saves"
shows today-live 1 "No case"
expect today-live 2 "[$tj.editing, .view.keys] | map(tostring) | join(\",\")" "true,false"
expect today-live 3 "[$tj.text, .view.section] | join(\",\")" "  --help 2 ,today"
expect today-live 5 "$tj.result" "Saved to the journal · 01M3W1FAKE0000000000000NTE"
expect today-live 5 "$tj.text" ""
shows today-live 5 "Saved to the journal · 01M3W1FAKE0000000000000NTE"
expect today-live 7 "$tj.result" "Write something first"
expect today-live 7 "$tj.text" "   "
expect today-live 8 "$tj.text" ""
expect today-live 13 "$tj.caseId" C-2026-004
shows today-live 13 "Open case"
expect today-live 17 "$tj.result" "Saved to C-2026-004 · 01M3W1FAKE0000000000000NTE"
expect today-live 18 "[$tj.editing, .view.keys, .view.opened] | map(tostring) | join(\",\")" "false,true,true"
# the editor opened: the desk steps aside (WP-156); summoned, it is where it was
expect today-live 20 "[.view.opened, .service.stepAsides] | map(tostring) | join(\",\")" "false,1"
expect today-live 21 .view.section today
expect today-live 22 .view.section changelog
expect today-live 24 "[.view.opened, .service.stepAsides] | map(tostring) | join(\",\")" "false,2"
expect today-live 25 .view.section changelog
expect today-live 26 "$tv.capturing" true
shows today-live 26 "Capturing"
expect today-live 27 "$tv.chips | join(\",\")" "open 6,crisis 2,attention 4,routine 37,case 38,all 83"
expect today-live 28 "$tv.captureResult" "1 new event"
shows today-live 28 "Last capture: 1 new event"
argv_check today-live "$work/home-today-live" "$(printf '%s\n' "$startup" \
  "$(q log --json -- "  --help 2 ")" "$(q log --case C-2026-004 --json -- "for the case")" \
  "$(q open journal --editor --json)" "$(q open ledger --editor --json)" \
  "$(q capture --all --json --quiet)" "$(q status --json)")"
check "today-live: editor paths" "$(cat "$work/today-live.record" 2>/dev/null | tr '\n' '|')" \
  "$(printf '%s\n' omarchy-launch-editor "$work/home-today-live/Seldon/journal/2026/2026-10-01.md" -- \
    omarchy-launch-editor "$work/home-today-live/Seldon/ledger/2026-10.jsonl" -- | tr '\n' '|')"
clean_log today-live

mkdir -p "$work/home-today-refuse"
run today-refuse "" 1920x1080 "summon;text:n;type:keep this;key:Tab;key:Down;key:Down;key:Down;key:Return;key:Backtab;key:Return;settle" \
  HOME="$work/home-today-refuse" FAKE_SELDON_FIXTURE="$sample" FAKE_SELDON_UNKNOWN_CASE=C-2026-004
expect today-refuse 8 "$tv.journal.caseId" C-2026-004
expect today-refuse 11 "$tv.journal.result" "unknown case C-2026-004"
expect today-refuse 11 "[$tv.journal.text, $tv.journal.editing] | map(tostring) | join(\",\")" "keep this,true"
shows today-refuse 11 "unknown case C-2026-004"
expect today-refuse 11 .view.lastError ""
clean_log today-refuse

intent='Install tool X. It needs --help $(id) and one package '
mkdir -p "$work/home-today-new"
run today-new "" 1920x1080 "summon;text:i;type:$intent;key:Return;settle;summon" \
  HOME="$work/home-today-new" FAKE_SELDON_FIXTURE="$sample"
expect today-new 2 "[$tv.intentEditing, .view.keys] | map(tostring) | join(\",\")" "true,false"
expect today-new 3 "$tv.intent" "$intent"
# the agent started: the desk steps aside (WP-156)
expect today-new 5 "[.view.opened, .service.stepAsides, .service.planOk] | map(tostring) | join(\",\")" "false,1,true"
expect today-new 5 ".service.plan" "Created C-2026-009 · Install tool X · agent started · launcher default (omarchy)"
expect today-new 6 "[$tv.intent, $tv.intentEditing, .view.keys, .view.opened] | map(tostring) | join(\",\")" ",false,true,true"
expect today-new 6 "$tv.cases | join(\",\")" "C-2026-003,C-2026-004,C-2026-009"
argv_check today-new "$work/home-today-new" "$(printf '%s\n' "$startup" "$(q agent start --new --json -- "$intent")")"
clean_log today-new

# 8b. The Changelog on the sample (panel 1, 12; dev mode): the chips and
#     their counts, rows by class (crisis urgent, attention accent), the
#     quiet attention line, the event detail with its bar and key/values,
#     `f`/`F`, a group and its members, Enter opens the default form and
#     Esc hides it (the desk stays open), the shim's `filter <source>` (all
#     plus the sidebar search) and `resolve`, Hide (this session only) and
#     Show. ADR-0020: "+N more … not listed here" (panel 13).
run changelog "$sample" 1920x1080 \
  "summon:$cl;text:f;text:F;text:F;select:$MESA;key:Return;key:Escape;shim:filter:pacman;key:Escape;shim:resolve:$LIB32;key:Return;key:Escape;shim:resolve:crisis;select:$THEME;click:Hide;text:f;click:Show"
expect changelog 1 "[.view.section, $tv.chip] | join(\",\")" "changelog,open"
expect changelog 1 "$tv.chips | join(\",\")" "open 6,crisis 2,attention 4,routine 37,case 38,all 83"
expect changelog 1 "[$tv.rows, $tv.cursor] | map(tostring) | join(\",\")" "6,0"
# One count everywhere (B2): the open chip = the sidebar's Changelog count,
# crisis = the header's crises, attention = the header's attention = the
# quiet line; a group counts once.
expect changelog 1 "[.view.counts.changelog.text, (.view.kpis | map(select(startswith(\"crises \") or startswith(\"attention \"))) | join(\"+\")), $tv.attention] | join(\",\")" \
  "6,crises 2+attention 4,4 changes without a case"
expect changelog 1 "$tv.selected" "$THEME"
expect changelog 1 "$tv.stripes | join(\",\")" \
  "tokyo-night attention,~/.config/systemd/user/ollama.service crisis,ollama attention,~/.config/omarchy/hooks/post-update.d/backup-dotfiles.sh crisis,~/.config/hypr/monitors.conf attention,mesa attention"
expect changelog 1 "$tv.badges | join(\",\")" "mesa +2"
expect changelog 1 "[$tv.attention, $tv.attentionDim, $tv.triageSlot] | map(tostring) | join(\",\")" "4 changes without a case,true,true"
expect changelog 1 "$td.actions | join(\",\")" "Link to C-2026-005…,Explain…,Dismiss…,Hide"
expect changelog 1 "[$td.heading, $td.cls, $td.whyLoud] | join(\",\")" "theme · theme-set,attention,"
expect changelog 1 "$td.kv | join(\" | \")" \
  "When: 2026-10-01 15:30 | Who: human | What: kanagawa → tokyo-night | Case: proposed: C-2026-005 | Rule: attention · planned by C-2026-005, not linked; quiet until you say something | Source: Omarchy's current theme | Zone: yellow | Event: $THEME"
expect changelog 1 "[$tf.shown, $tf.action, $tf.caseId, $tf.hint] | map(tostring) | join(\",\")" "false,link,C-2026-005,Dev mode is read-only"
expect changelog 1 "$tf.cases | join(\",\")" "C-2026-005,C-2026-003,C-2026-004,C-2026-008,C-2026-006,C-2026-007"
for text in "6 changes · newest first" "4 changes without a case" "proposed for C-2026-005" "TODAY" "TUE 29 SEP" \
  "tokyo-night" "mesa +2" "27 Sep 12:30" "open 6" "in case 38" "Ledger" "Capture now"; do
  shows changelog 1 "$text"
done
expect changelog 2 "[$tv.chip, $tv.rows, $tv.selected] | map(tostring) | join(\",\")" "crisis,2,$UNIT"
expect changelog 3 "$tv.chip" open
expect changelog 4 "[$tv.chip, $tv.rows] | map(tostring) | join(\",\")" "all,83"
expect changelog 5 "[.call, $tv.selected, $tf.subject, $tf.badge] | join(\",\")" "ok,$MESA,mesa,+2"
expect changelog 5 "$tf.members | join(\" | \")" \
  "· downgrade mesa  1:26.2.0-2 → 1:26.1.0-1 | · downgrade lib32-mesa  1:26.2.0-2 → 1:26.1.0-1 | · downgrade vulkan-radeon  1:26.2.0-2 → 1:26.1.0-1"
# WP-137: the transaction above lists every open member, so the form's lines hide
expect changelog 5 "[$tf.membersShown, $td.transaction.shown] | map(tostring) | join(\",\")" "false,true"
shows changelog 5 "3 packages: 3 downgraded in this transaction"
expect changelog 6 "[$tf.shown, $tf.editing, $tf.action, .view.keys] | map(tostring) | join(\",\")" "true,true,explain,false"
shows changelog 6 "All 3"
shows changelog 6 "Only mesa"
shows changelog 6 "EXPLAIN"
expect changelog 7 "[$tf.shown, $tf.editing, .view.keys, .view.opened] | map(tostring) | join(\",\")" "false,false,true,true"
expect changelog 8 "[.call, $tv.chip, .view.search.text, $tv.rows] | map(tostring) | join(\",\")" "ok,all,pacman,23"
expect changelog 9 "[.view.search.text, $tv.rows, .view.opened] | map(tostring) | join(\",\")" ",83,true"
expect changelog 10 "[$tv.selected, $tf.eventId, $tf.subject] | join(\",\")" "$LIB32,$LIB32,mesa"
shows changelog 11 "Only lib32-mesa"
expect changelog 13 "[$tv.chip, $tv.rows] | map(tostring) | join(\",\")" "crisis,2"
expect changelog 14 "[.call, $tv.chip, $tv.selected] | join(\",\")" "ok,all,$THEME"
expect changelog 15 "[$td.hidden, $tv.hidden, $tv.rows] | map(tostring) | join(\",\")" "true,1,83"
expect changelog 15 "$td.actions | join(\",\")" "Link to C-2026-005…,Explain…,Dismiss…,Show"
shows changelog 15 "attention · hidden this session"
expect changelog 16 "[$tv.chip, $tv.rows, $tv.selected] | map(tostring) | join(\",\")" "open,5,$UNIT"
expect changelog 16 "$tv.chips[0]" "open 5"
shows changelog 16 "1 change hidden this session"
expect changelog 16 "[$tv.hidden, ($tv.chips[0:3] | join(\"+\")), .view.counts.changelog.text] | map(tostring) | join(\",\")" "1,open 5+crisis 2+attention 3,6"
expect changelog 17 "[$tv.hidden, $tv.rows] | map(tostring) | join(\",\")" "0,6"
for i in 1 5 6 10 16; do expect changelog $i '.overflow | join(" | ")' ""; done
clean_log changelog

# 8b'. A pacman transaction's packages (WP-137, ADR-0043) on the sample: the
#      09-19 -Syu interrupted after two upgrades is marked in the urgent
#      style in its rows and in the detail (callout); the 09-18 mixed -Syu
#      lists − + ↑ ↑ with old → new and its command; the 09-27 downgrade
#      group lists ↓ ×3 and its form's member lines give way to the list;
#      the btop install (one package) has the rows and no list; nothing
#      overflows at 100 % and 50 %.
PIPEWIRE=01M2TRP54G6K1N4WZF19XZFHZW GTK4=01M2W5S4XG9MX3PXAMR7WEEBVZ BTOP=01M1MB2M1GWZYF485HTGVZ1KS3
run transactions "$sample" 1920x1080 \
  "summon:{\"section\":\"changelog\",\"filter\":\"all\"};select:$GTK4;select:$PIPEWIRE;select:$MESA;select:$BTOP;select:$GTK4;width:50;view"
txv="$td.transaction"
expect transactions 1 "$tv.alerts | join(\",\")" "libadwaita interrupted,gtk4 interrupted"
shows transactions 2 "interrupted"
expect transactions 2 "[$txv.status, $txv.statusShown, $txv.title, $txv.summary] | map(tostring) | join(\",\")" \
  "interrupted,true,Transaction interrupted,2 packages: 2 upgraded in this transaction"
expect transactions 2 "$txv.lines | join(\" | \")" "↑ gtk4  1:4.18.6-1 → 1:4.18.7-1 | ↑ libadwaita  1:1.7.6-1 → 1:1.7.7-1"
expect transactions 2 "$txv.selected | join(\",\")" "gtk4"
expect transactions 2 "[$td.cls, ($td.kv | map(select(startswith(\"Command\") or startswith(\"Transaction\"))) | join(\" | \"))] | join(\",\")" \
  "routine,Command: pacman -Syu | Transaction: 2 packages: 2 upgraded · interrupted"
for text in "Transaction interrupted" "gtk4  1:4.18.6-1 → 1:4.18.7-1" "↑" "pacman -Syu"; do shows transactions 2 "$text"; done
expect transactions 3 "[$txv.status, $txv.statusShown, $txv.summary] | map(tostring) | join(\",\")" \
  ",false,4 packages: 1 removed, 1 installed, 2 upgraded in this transaction"
expect transactions 3 "$txv.lines | join(\" | \")" \
  "− pulseaudio  17.0-3 | + pipewire-pulse  1:1.4.8-1 | ↑ pipewire  1:1.4.7-1 → 1:1.4.8-1 | ↑ wireplumber  0.5.10-1 → 0.5.11-1"
expect transactions 3 "$txv.selected | join(\",\")" "pipewire"
for text in "−" "+" "pulseaudio  17.0-3" "pipewire-pulse  1:1.4.8-1"; do shows transactions 3 "$text"; done
expect transactions 3 '[.texts[] | select(. == "Transaction interrupted")] | length' 0
expect transactions 4 "$txv.lines | join(\" | \")" \
  "↓ lib32-mesa  1:26.2.0-2 → 1:26.1.0-1 | ↓ mesa  1:26.2.0-2 → 1:26.1.0-1 | ↓ vulkan-radeon  1:26.2.0-2 → 1:26.1.0-1"
expect transactions 4 "[$tf.membersShown, $txv.statusShown] | map(tostring) | join(\",\")" "false,false"
expect transactions 4 '[.texts[] | select(. == "3 packages in one transaction:")] | length' 0
expect transactions 5 "[$txv.shown, ($td.kv | map(select(startswith(\"Transaction\"))) | join(\"\"))] | map(tostring) | join(\",\")" \
  "false,Transaction: 1 package: 1 installed"
expect transactions 6 "$txv.statusShown" true
for i in 2 3 4 6 7; do expect transactions $i '.overflow | join(" | ")' ""; done
clean_log transactions

jq '.summary.openDrift = 250' "$sample" >"$work/capped.json"
run changelog-capped "$work/capped.json" 1920x1080 "summon:$cl" HARNESS_SETTINGS='{"driftInBar":"all"}'
expect changelog-capped 1 "$tv.more" "+244 more changes without a case not listed here"
shows changelog-capped 1 "+244 more changes without a case not listed here"
expect changelog-capped 1 .pill.text "2 · 250"
expect changelog-capped 1 "$tv.attention" "248 changes without a case"
clean_log changelog-capped

# 8c. Quiet surfaces (panel 13b, 13c; ADR-0028 §4b), then the inline forms
#     live against the fake engine (panel 14–19): link with the proposed
#     case (Enter opens, Enter arms, Enter runs; the event stays shown when
#     it leaves the open chip; Open case goes to Work), explain with risk
#     and area, dismiss a group as one; `--only` with the picker driven by
#     keys and a refusal in the plugin; an already resolved re-run; a lock
#     refusal that keeps the text and the per-event drafts; `drift show`
#     for members the index no longer lists, from the leader and a member.
HOOK="~/.config/omarchy/hooks/post-update.d/10-sync"
jq --arg u "$UNIT" --arg o "$OLLAMA" --arg h "$HOOK" --arg k "$HOOK_EVENT" '
  .summary.crisis = 1
  | .drift |= map(if .eventId == $u then .zone = "yellow" | .subject = $h
                  elif .eventId == $o or .eventId == $k then .crisis = false else . end)
  | .events |= map(if .id == $u then .zone = "yellow" | .subject = $h else . end)' \
  "$sample" >"$work/quiet-hook.json"
run quiet-crisis "$work/quiet-hook.json" 1920x1080 "summon;text:2;select:$UNIT;select:$OLLAMA"
expect quiet-crisis 1 "[$tv.state, $tv.headline, .pill.text] | join(\",\")" "crisis,Seldon is recording. 1 change needs you.,2 · 1"
expect quiet-crisis 1 "$tv.needs | join(\",\")" "$UNIT"
expect quiet-crisis 2 "[$tv.attention, $tv.attentionDim] | map(tostring) | join(\",\")" "5 changes without a case,true"
expect quiet-crisis 2 "$tv.stripes | join(\",\")" \
  "tokyo-night attention,$HOOK crisis,ollama attention,~/.config/omarchy/hooks/post-update.d/backup-dotfiles.sh attention,~/.config/hypr/monitors.conf attention,mesa attention"
expect quiet-crisis 3 "[$td.cls, $tf.crisis, $tf.zone, $tf.explainZone] | map(tostring) | join(\",\")" "crisis,true,yellow,yellow"
expect quiet-crisis 3 "$td.whyLoud" "The path matches your crisis list ([drift] alwaysRedPaths in ~/.config/seldon/config.toml). No open case plans it, and no case is linked."
expect quiet-crisis 3 "$td.actions | join(\",\")" "Link to case…,Explain…,Dismiss…"
expect quiet-crisis 4 "[$td.cls, $tf.crisis, $tf.zone, $td.whyLoud] | map(tostring) | join(\",\")" "attention,false,red,"
expect quiet-crisis 4 '[.texts[] | select(. == "Why loud?")] | length' 0
clean_log quiet-crisis

jq '.summary.crisis = 0 | .summary.activeCases = 0 | .drift |= map(.crisis = false)' "$sample" >"$work/quiet-only.json"
run quiet-attention "$work/quiet-only.json" 1920x1080 "summon;text:2"
expect quiet-attention 1 "[$tv.state, $tv.headline, .pill.text] | join(\",\")" "all-clear,Seldon is recording. Nothing needs you.,"
expect quiet-attention 1 "$tv.needs | length" 0
expect quiet-attention 1 '[.texts[] | select(. == "NEEDS YOU")] | length' 0
expect quiet-attention 2 "$tv.attention" "6 changes without a case"
expect quiet-attention 2 "[$tv.stripes[] | select(endswith(\" crisis\"))] | length" 0
expect quiet-attention 2 "$tv.chips[1]" "crisis 0"
for n in 1 2; do expect quiet-attention $n '[.texts[] | select(test("Why loud|boot, login or the shell, and"))] | length' 0; done
clean_log quiet-attention
jq '.summary.activeCases = 2' "$work/quiet-only.json" >"$work/quiet-active.json"
run quiet-active "$work/quiet-active.json" 1920x1080 "summon"
expect quiet-active 1 "[$tv.state, .pill.text] | join(\",\")" "case-active,2"
clean_log quiet-active

mkdir -p "$work/home-drift"
run drift-live "" 1920x1080 \
  "summon:$cl;key:Return;key:Return;key:Return;wait:sectionView.detail.form.isOpen=false;settle;click:Open case;text:2;key:Down;click:Explain…;type: --help ;key:Return;key:Tab;key:Tab;key:Right;key:Return;key:Tab;type:dev-env;key:Return;key:Return;wait:sectionView.detail.form.isOpen=false;settle;select:$MESA;click:Dismiss…;type:routine update  ;key:Return;key:Return;wait:sectionView.detail.form.isOpen=false;settle;text:3" \
  HOME="$work/home-drift" FAKE_SELDON_FIXTURE="$sample"
expect drift-live 1 "$tv.selected" "$THEME"
expect drift-live 2 "[$tf.shown, $tf.editing, $tf.action, $tf.caseId] | map(tostring) | join(\",\")" "true,true,link,C-2026-005"
shows drift-live 2 "LINK TO A CASE"
expect drift-live 3 "[$tf.armed, $tf.hint] | map(tostring) | join(\",\")" "true,Press Enter again: Link tokyo-night to C-2026-005"
shows drift-live 3 "Press Enter again: Link tokyo-night to C-2026-005"
expect drift-live 6 "[$tf.isOpen, $tf.result, $tf.resolution] | map(tostring) | join(\",\")" "false,Linked 1 event to C-2026-005,linked to C-2026-005"
expect drift-live 6 "[$tv.selected, $tv.cursor, $tv.rows, $tf.shown, .view.keys] | map(tostring) | join(\",\")" "$THEME,-1,5,false,true"
expect drift-live 6 "$td.actions | join(\",\")" "Open case"
expect drift-live 6 .pill.text "2 · 2"
shows drift-live 6 "Resolved: linked to C-2026-005"
expect drift-live 7 "[.view.section, .view.selected] | join(\",\")" "work,C-2026-005"
expect drift-live 8 "[.view.section, $tv.selected] | join(\",\")" "changelog,$THEME"
expect drift-live 9 "$tv.selected" "$UNIT"
expect drift-live 10 "[$tf.shown, $tf.action, $tf.explainZone, .view.keys] | map(tostring) | join(\",\")" "true,explain,red,false"
expect drift-live 11 "$tf.intent" " --help "
expect drift-live 11 .view.section changelog
expect drift-live 12 "$tf.armed" true
expect drift-live 14 "$tf.armed" true
expect drift-live 16 "[$tf.risk, $tf.armed] | map(tostring) | join(\",\")" "R2,false"
expect drift-live 18 "$tf.area" dev-env
expect drift-live 19 "$tf.hint" "Press Enter again: Explain ~/.config/systemd/user/ollama.service as a new completed case"
expect drift-live 22 "[$tf.result, $tf.resolution] | join(\",\")" "Explained 1 event · created C-2026-009 · new area dev-env,explained · C-2026-009: --help"
expect drift-live 22 "[.pill.text, ($td.actions | join(\"+\"))] | join(\",\")" "2 · 1,Open case"
expect drift-live 23 "$tv.selected" "$MESA"
expect drift-live 24 "[$tf.shown, $tf.action] | map(tostring) | join(\",\")" "true,dismiss"
expect drift-live 25 "$tf.reason" "routine update  "
expect drift-live 26 "$tf.hint" "Press Enter again: Dismiss mesa and 2 more"
expect drift-live 29 "$tf.result" "Dismissed 3 events"
expect drift-live 29 "$tv.stripes | join(\",\")" \
  "ollama attention,~/.config/omarchy/hooks/post-update.d/backup-dotfiles.sh crisis,~/.config/hypr/monitors.conf attention"
expect drift-live 29 "[($tv.badges | length), ($td.actions | length), .pill.text, .view.lastError] | map(tostring) | join(\",\")" "0,0,2 · 1,"
expect drift-live 30 "$tv.groups | join(\",\")" "active 2,verification 1,queued 3,completed 3"
expect drift-live 30 "$tv.ids | index(\"C-2026-009\") >= 6" true
argv_check drift-live "$work/home-drift" "$(printf '%s\n' "$startup" \
  "$(q drift link $THEME C-2026-005 --json)" \
  "$(q drift explain $UNIT --risk R2 --area dev-env --json -- " --help ")" \
  "$(q drift dismiss $MESA --json -- "routine update  ")")"
clean_log drift-live

mkdir -p "$work/home-drift-only"
run drift-only "" 1920x1080 \
  "summon:$(sel $MESA);click:Link to case…;key:Return;key:Backtab;key:Backtab;key:Return;key:Down;key:Down;key:Return;key:Tab;key:Right;key:Return;key:Tab;key:Return;key:Return;wait:sectionView.detail.form.isOpen=false;settle;select:$LIB32" \
  HOME="$work/home-drift-only" FAKE_SELDON_FIXTURE="$sample"
expect drift-only 1 "$tv.selected" "$MESA"
expect drift-only 2 "[$tf.shown, $tf.action, $tf.caseId] | map(tostring) | join(\",\")" "true,link,"
expect drift-only 3 "[$tf.result, $tf.armed] | map(tostring) | join(\",\")" "Pick a case first,false"
shows drift-only 3 "Pick a case first"
expect drift-only 9 "[$tf.caseId, $tf.result] | join(\",\")" "C-2026-004,"
expect drift-only 12 "$tf.only" true
shows drift-only 12 "Only mesa"
expect drift-only 14 "$tf.hint" "Press Enter again: Link mesa only to C-2026-004"
expect drift-only 17 "[$tf.result, $tf.resolution] | join(\",\")" "Linked 1 event to C-2026-004,linked to C-2026-004"
expect drift-only 17 "[($tv.badges | join(\"+\")), .pill.text] | join(\",\")" "lib32-mesa +1,2 · 2"
expect drift-only 18 "[$tf.eventId, $tf.badge, ($tf.members | length)] | map(tostring) | join(\",\")" "$LIB32,+1,2"
shows drift-only 18 "2 packages in one transaction:"
argv_check drift-only "$work/home-drift-only" "$(printf '%s\n' "$startup" "$(q drift link $MESA C-2026-004 --only --json)")"
clean_log drift-only

mkdir -p "$work/home-drift-already"
echo "$THEME linked C-2026-005" >"$work/home-drift-already/resolved"
run drift-already "" 1920x1080 "summon:$(sel $THEME);key:Return;key:Return;key:Return;settle" \
  HOME="$work/home-drift-already" FAKE_SELDON_FIXTURE="$sample"
expect drift-already 5 "[$tf.result, $tf.already, $tf.resultOk, $tf.isOpen, .pill.text] | map(tostring) | join(\",\")" \
  "Already resolved: linked to C-2026-005,true,true,true,2 · 2"
shows drift-already 5 "Already resolved: linked to C-2026-005"
clean_log drift-already

mkdir -p "$work/home-drift-locked"
run drift-locked "" 1920x1080 \
  "summon:$(sel $UNIT);key:Return;type:keep this;key:Return;key:Return;settle;key:Escape;select:$THEME;key:Return;key:Escape;select:$UNIT;key:Return" \
  HOME="$work/home-drift-locked" FAKE_SELDON_FIXTURE="$sample" FAKE_SELDON_LOCKED=1
expect drift-locked 6 "[$tf.result, $tf.resultOk, $tf.intent, $tf.isOpen, .view.lastError] | map(tostring) | join(\",\")" \
  "the logbook is locked by another seldon (pid 4242),false,keep this,true,"
shows drift-locked 6 "the logbook is locked by another seldon (pid 4242)"
expect drift-locked 7 "[$tf.shown, $tf.editing, .view.keys, .view.opened] | map(tostring) | join(\",\")" "false,false,true,true"
expect drift-locked 8 "[$tf.intent, $tf.action] | join(\",\")" ",link"
expect drift-locked 11 "[$tf.intent, $tf.action] | join(\",\")" "keep this,explain"
expect drift-locked 12 "[$tf.shown, $tf.intent] | map(tostring) | join(\",\")" "true,keep this"
clean_log drift-locked

jq '.events |= map(select(.id != "'$LIB32'"))' "$sample" >"$work/members-capped.json"
mkdir -p "$work/home-drift-show"
jq '[.events[] | select(.id == "'$LIB32'")]' "$sample" >"$work/home-drift-show/extra-events.json"
run drift-show "" 1920x1080 "summon:$(sel $MESA);wait:sectionView.detail.form.members.1=· downgrade lib32-mesa  1:26.2.0-2 → 1:26.1.0-1" \
  HOME="$work/home-drift-show" FAKE_SELDON_FIXTURE="$work/members-capped.json"
expect drift-show 2 "$tf.members | join(\" | \")" \
  "· downgrade mesa  1:26.2.0-2 → 1:26.1.0-1 | · downgrade lib32-mesa  1:26.2.0-2 → 1:26.1.0-1 | · downgrade vulkan-radeon  1:26.2.0-2 → 1:26.1.0-1"
argv_check drift-show "$work/home-drift-show" "$(printf '%s\n' "$startup" "$(q drift show $MESA --json)")"
clean_log drift-show
mkdir -p "$work/home-drift-show-member"
cp "$work/home-drift-show/extra-events.json" "$work/home-drift-show-member/"
run drift-show-member "" 1920x1080 \
  "summon:$(sel $VULKAN);wait:sectionView.detail.form.members.1=· downgrade lib32-mesa  1:26.2.0-2 → 1:26.1.0-1;key:Return" \
  HOME="$work/home-drift-show-member" FAKE_SELDON_FIXTURE="$work/members-capped.json"
expect drift-show-member 2 "[$tf.eventId, $tf.subject, ($tf.members | length)] | map(tostring) | join(\",\")" "$VULKAN,mesa,3"
shows drift-show-member 3 "Only vulkan-radeon"
argv_check drift-show-member "$work/home-drift-show-member" "$(printf '%s\n' "$startup" "$(q drift show $MESA --json)")"
clean_log drift-show-member

# 8d. Work (panel 9–11, 30–32): groups, the WIP line, the case detail from
#     the index (key/values, plan, log, linked changes), the bar by status
#     and dev mode's refusal; By agent; a reopen's case; live: the new-case
#     sheet with keys, an invalid area refused in the plugin, start → to
#     verification → complete (each armed, then run), Open in editor, the
#     engine's refusal, `x x` drops; hand to agent (`a` twice, a click and
#     Confirm) and its refusal; a locked new case keeps its title; Run with
#     the sentence as one argument, and its refusal; Reopen once and `r`.
run work "$sample" 1920x1080 \
  "summon:$wk;text:a;click:Hand to agent;key:Down;key:Down;key:Down;key:Down*3;click:By agent;click:By agent;key:Down"
expect work 1 "$tv.groups | join(\",\")" "active 2,verification 1,queued 3,completed 2"
expect work 1 "$tv.ids | join(\",\")" "C-2026-003,C-2026-004,C-2026-008,C-2026-005,C-2026-006,C-2026-007,C-2026-002,C-2026-001"
expect work 1 "[$tv.wip, $tv.selected, $tc.heading] | join(\",\")" "2 / 3 active,C-2026-003,C-2026-003 · active"
expect work 1 "$tc.actions | join(\",\")" "Hand to agent,To verification,Drop,Open in editor"
expect work 1 "$tc.kv | join(\" | \")" \
  "Status: active | Risk: R3 · every step that can break boot needs your go | Zone: red | Area: shell | Priority: high | Agent: agent:claude-code | Rollback: snapshot 111 | Dates: created 2026-09-26 · started 2026-10-01 | File: work/active/C-2026-003-omarchy-407.md"
expect work 1 "[$tc.plan, $tc.log, $tc.linked, $tc.hint] | map(tostring) | join(\",\")" "4 of 5 steps done,3,6,Dev mode is read-only"
for text in "ACTIVE · 2" "VERIFICATION · 1" "QUEUED · 3" "COMPLETED · 2" "2 / 3 active" "C-2026-005 · R1 · themes · 1 proposed" \
  "4/5" "Run" "New case" "By agent" "Dev mode is read-only" "PLAN" "LOG" "LINKED CHANGES · 6" "C-2026-003 · R3" \
  "4 of 5 steps done. The steps and the full Intent and Result are in the case file." "case-started · human · R3" "case-updated · human · R3" \
  "INTENT" "Omarchy 4.0.7 einspielen, ohne die eigenen Hyprland-Bindings zu verlieren." \
  "Omarchy auf 4.0.7 aktualisieren" "Hand to agent" "To verification"; do
  shows work 1 "$text"
done
expect work 2 "[$tc.armed, $tv.result] | join(\",\")" ","
expect work 3 "$tc.armed" ""
expect work 4 "$tv.selected" C-2026-004
expect work 5 "[$tv.selected, $tc.status, ($tc.actions | join(\"+\"))] | join(\",\")" "C-2026-008,verification,Complete+Drop+Open in editor"
expect work 6 "[$tv.selected, ($tc.actions | join(\"+\"))] | join(\",\")" "C-2026-005,Start+Drop+Open in editor"
expect work 6 "[$tc.kv[] | select(startswith(\"Proposed\"))] | join(\",\")" "Proposed: 1 open change the engine thinks belong here"
expect work 7 "[$tv.selected, $tc.heading, ($tc.actions | join(\"+\"))] | join(\",\")" "C-2026-002,C-2026-002 · completed by agent,Reopen+Open in editor"
shows work 7 "completed by agent"
shows work 7 "C-2026-002 · R1 · hyprland · closed by agent"
expect work 8 "[$tv.filter, ($tv.groups | join(\"+\")), $tv.selected] | join(\",\")" "agent,active 2+verification 1+queued 3+completed 1,C-2026-002"
shows work 8 "COMPLETED · 1 / 2"
expect work 9 "[$tv.filter, ($tv.groups | join(\"+\"))] | join(\",\")" ",active 2+verification 1+queued 3+completed 2"
expect work 10 "[$tv.selected, ($tc.kv[] | select(startswith(\"Dates\")))] | join(\",\")" "C-2026-001,Dates: created 2026-09-01 · started 2026-09-01 · closed 2026-09-01"
for i in 1 7 8; do expect work $i '.overflow | join(" | ")' ""; done
clean_log work

run work-reopened "$fx/index-variants/case-reopened.json" 1920x1080 "summon:$wk;select:C-2026-009"
expect work-reopened 2 "[$tv.selected, ($tc.kv[] | select(startswith(\"Reopens\")))] | join(\",\")" "C-2026-009,Reopens: C-2026-002"
shows work-reopened 2 "reopens C-2026-002"
shows work-reopened 2 "3 / 3 active · at the limit"
clean_log work-reopened

mkdir -p "$work/home-work"
echo "C-2026-008 active" >"$work/home-work/cases"
run work-live "" 1920x1080 \
  "summon:$wk;text:+;type: --help;key:Tab;key:Right;key:Return;key:Tab;key:Right;key:Return;key:Tab;key:Left;key:Return;key:Tab;type:Dev;key:Return;key:Backspace*3;type:dev-env;key:Return;settle;wait:sectionView.selected=C-2026-009;select:C-2026-005;key:Return;key:Return;settle;wait:sectionView.case.status=active;key:Return;key:Return;settle;wait:sectionView.case.status=verification;key:Return;key:Return;settle;wait:sectionView.case.status=completed;text:e;settle;summon:$wk;select:C-2026-008;key:Return;key:Left;key:Return;key:Return;settle;select:C-2026-004;text:x;text:x;settle;wait:sectionView.case.status=dropped;text:e;settle" \
  HOME="$work/home-work" FAKE_SELDON_FIXTURE="$sample" HARNESS_RECORD="$work/work-live.record"
expect work-live 1 "$tc.hint" ""
expect work-live 2 "[$ts.open, $ts.editing, $ts.zone, $ts.risk, $ts.priority, .view.keys] | map(tostring) | join(\",\")" "true,true,yellow,R1,normal,false"
shows work-live 2 "Title, Enter creates the case"
shows work-live 2 "NEW CASE"
expect work-live 3 "[$ts.title, .view.section] | join(\",\")" " --help,work"
expect work-live 6 "$ts.zone" red
expect work-live 9 "$ts.risk" R2
expect work-live 12 "$ts.priority" high
expect work-live 14 "$ts.area" Dev
shows work-live 14 "Area: lowercase letters, digits and -, starting with a letter or digit"
expect work-live 15 "[$ts.result, $ts.open, $ts.title] | map(tostring) | join(\",\")" "Area must be a lowercase slug: letters, digits and -,true, --help"
expect work-live 17 "$ts.area" dev-env
expect work-live 20 "[$ts.open, $ts.editing, $ts.title, $ts.zone, .view.keys] | map(tostring) | join(\",\")" "false,false,,yellow,true"
expect work-live 20 "[$tv.result, ($tv.groups | join(\"+\")), $tc.id] | join(\",\")" "Created C-2026-009 ·  --help · new area dev-env,active 2+verification 1+queued 4+completed 2,C-2026-009"
expect work-live 20 "[$tc.kv[] | select(startswith(\"Zone\") or startswith(\"Risk\") or startswith(\"Area\") or startswith(\"Priority\"))] | join(\",\")" "Risk: R2,Zone: red,Area: dev-env,Priority: high"
expect work-live 22 "[$tc.armed, $tc.hint] | join(\",\")" "start,Start C-2026-005? Press Enter again or click Confirm."
shows work-live 22 "Confirm start"
expect work-live 25 "[$tv.result, $tv.selected, ($tv.groups | join(\"+\")), $tv.wip] | join(\",\")" "C-2026-005: queued → active,C-2026-005,active 3+verification 1+queued 3+completed 2,3 / 3 active"
shows work-live 25 "3 / 3 active · at the limit"
expect work-live 25 "$tc.actions | join(\",\")" "Hand to agent,To verification,Drop,Open in editor,Ask agent"
expect work-live 26 "[$tc.armed, ($tc.actions | join(\"+\"))] | join(\",\")" "verify,Hand to agent+Confirm to verification+Drop+Open in editor+Ask agent"
expect work-live 29 "$tv.result" "C-2026-005: active → verification"
expect work-live 33 "[$tv.result, ($tc.actions | join(\"+\"))] | join(\",\")" "C-2026-005: verification → completed · journal journal/2026/2026-10-01.md,Reopen+Open in editor+Ask agent"
expect work-live 33 "$tv.groups | join(\",\")" "active 2,verification 1,queued 3,completed 3"
# the editor opened: the desk steps aside (WP-156); opened again, the case is still selected
expect work-live 35 "[.view.opened, .service.stepAsides] | map(tostring) | join(\",\")" "false,1"
expect work-live 36 "[.view.opened, .view.section, $tv.selected] | map(tostring) | join(\",\")" "true,work,C-2026-005"
expect work-live 38 "$tc.armed" done
expect work-live 39 "$tc.armed" ""
expect work-live 40 "$tc.armed" done
refusal='C-2026-008 is active; `seldon plan done` needs a case that is verification; run `seldon plan verify` first'
expect work-live 42 "[$tv.result, $tv.resultOk, $tv.selected, .view.lastError] | map(tostring) | join(\",\")" "$refusal,false,C-2026-008,"
shows work-live 42 "$refusal"
expect work-live 44 "[$tc.armed, $tc.hint] | join(\",\")" "drop,Drop C-2026-004? Press x again or click Confirm. This is final."
shows work-live 44 "Confirm drop"
expect work-live 47 "[$tv.result, $tv.wip, ($tc.actions | join(\"+\"))] | join(\",\")" "C-2026-004: active → dropped,1 / 3 active,Open in editor+Ask agent"
expect work-live 47 "$tv.groups | join(\",\")" "active 1,verification 1,queued 3,completed 4"
argv_check work-live "$work/home-work" "$(printf '%s\n' "$startup" \
  "$(q plan new --zone red --risk R2 --area dev-env --priority high --json -- " --help")" \
  "$(q plan start C-2026-005 --json)" "$(q plan verify C-2026-005 --json)" "$(q plan done C-2026-005 --json)" \
  "$(q open C-2026-005 --editor --json)" "$(q plan done C-2026-008 --json)" "$(q plan drop C-2026-004 --json)" \
  "$(q open C-2026-004 --editor --json)")"
check "work-live: editor paths" "$(cat "$work/work-live.record" 2>/dev/null | tr '\n' '|')" \
  "$(printf '%s\n' omarchy-launch-editor "$work/home-work/Seldon/work/active/C-2026-005.md" -- \
    omarchy-launch-editor "$work/home-work/Seldon/work/active/C-2026-004.md" -- | tr '\n' '|')"
clean_log work-live

mkdir -p "$work/home-agent"
echo "C-2026-004 queued" >"$work/home-agent/cases"
run work-agent "" 1920x1080 \
  "summon:$wk;select:C-2026-005;text:a;select:C-2026-003;text:a;key:Return;key:Down;key:Up;text:a;text:a;settle;summon:$wk;select:C-2026-004;click:Hand to agent;click:Confirm hand to agent;settle" \
  HOME="$work/home-agent" FAKE_SELDON_FIXTURE="$sample"
expect work-agent 3 "$tc.armed" ""
expect work-agent 5 "[$tc.armed, $tc.hint] | join(\",\")" "agent,Hand to agent C-2026-003? Press a again or click Confirm."
shows work-agent 5 "Confirm hand to agent"
# Enter never launches: after `a` it arms To verification instead (0.1's habit)
expect work-agent 6 "[$tc.armed, $tc.hint] | join(\",\")" "verify,To verification C-2026-003? Press Enter again or click Confirm."
expect work-agent 7 "[$tv.selected, $tc.armed] | join(\",\")" "C-2026-004,"
# launched: the desk steps aside (WP-156); opened again, the answer is there
expect work-agent 11 "[.service.plan, .service.planOk, .view.opened] | map(tostring) | join(\",\")" "Agent started on C-2026-003 · launcher default (omarchy),true,false"
expect work-agent 12 "[$tv.result, $tv.resultOk, $tc.armed] | map(tostring) | join(\",\")" "Agent started on C-2026-003 · launcher default (omarchy),true,"
shows work-agent 12 "Agent started on C-2026-003 · launcher default (omarchy)"
expect work-agent 14 "$tc.armed" agent
refusal='C-2026-004 is queued; start it first: `seldon plan start C-2026-004`'
# a refusal keeps the desk open with the engine's text
expect work-agent 16 "[$tv.result, $tv.resultOk, .view.lastError, .view.opened] | map(tostring) | join(\",\")" "$refusal,false,,true"
argv_check work-agent "$work/home-agent" "$(printf '%s\n' "$startup" "$(q agent start C-2026-003 --json)" "$(q agent start C-2026-004 --json)")"
clean_log work-agent

# WP-102b, Import tasks…: the form in the detail, a dry run first (the list
# of what would be created and what is skipped), then one click imports;
# the cases arrive with the index, the first is selected, its detail asks
# the engine for the whole Intent (`plan show`, read-only) and shows it as
# plain text, and only then enables Start, which arms by click; Enter
# neither starts nor drops it. A second dry run reports the tasks as
# already imported. The path is one argument after `--` (argv compared).
mkdir -p "$work/home-import/projects"
printf '%s\n' "# Desk" "- [ ] Fix the bar flicker" "  Only after resume." "  ## Result" "- [x] Install zed" \
  "- [ ] Try a lighter theme" >"$work/home-import/projects/TODO.md"
run import-live "" 1920x1080 \
  "summon:$wk;view;click:Import tasks…;type:~/projects/TODO.md;key:Return;settle;click:Import 2 cases;settle;wait:sectionView.case.reviewed=true;key:Return;key:Return;click:Start;click:Confirm start;settle;wait:sectionView.case.status=active;click:Import tasks…;type:~/projects/TODO.md;key:Return;settle" \
  HOME="$work/home-import" FAKE_SELDON_FIXTURE="$sample"
ti="$tv.import"
shows import-live 2 "Import tasks…"
shows import-live 2 "C-2026-007 · imported · R1 · dev-env"
expect import-live 3 "[$ti.open, $ti.editing, .view.sectionView.case.id] | map(tostring) | join(\",\")" "true,true,C-2026-003"
shows import-live 3 "IMPORT TASKS"
expect import-live 3 "$tc.actions | join(\",\")" ""
expect import-live 6 "[$ti.result, $ti.canImport, ($ti.rows | join(\"+\")), ($ti.skipped | join(\"+\"))] | map(tostring) | join(\",\")" \
  "Would create 2 cases · 1 task skipped,true,|Fix the bar flicker|~/projects/TODO.md#2+|Try a lighter theme|~/projects/TODO.md#6,~/projects/TODO.md#5|done (- [x])|"
for text in "Import 2 cases" "Would create 2 cases · 1 task skipped" "Fix the bar flicker" "queued · ~/projects/TODO.md#2" \
  "Skipped ~/projects/TODO.md#5: done (- [x])"; do
  shows import-live 6 "$text"
done
expect import-live 9 "[$ti.open, $tv.selected, $tv.importLine, $tc.imported, $tc.reviewed, $tc.startEnabled] | map(tostring) | join(\",\")" \
  "false,C-2026-009,Imported 2 cases: C-2026-009, C-2026-010 · 1 task skipped,true,true,true"
expect import-live 9 "[$tc.review.lines, $tc.review.intent] | map(tostring) | join(\"|\")" \
  "5|Imported from ~/projects/TODO.md#2 — read before you start this case.

Fix the bar flicker
Only after resume.
## Result"
expect import-live 9 "[$tc.kv[] | select(startswith(\"Imported from\"))] | join(\",\")" "Imported from: ~/projects/TODO.md#2"
for text in "C-2026-009 · imported · R1" "IMPORTED TASK · 5 lines" \
  "From ~/projects/TODO.md#2. Read the whole Intent before you start the case: once started, an agent acts on it without asking. Only you start it."; do
  shows import-live 9 "$text"
done
# shown whole: no first-paragraph INTENT block beside it
expect import-live 9 '[.texts[] | select(. == "INTENT")] | length' 0
# Enter neither starts nor drops an imported case; Start arms by click
expect import-live 11 "[$tc.armed, $tc.status] | join(\",\")" ",queued"
expect import-live 12 "[$tc.armed, $tc.hint] | join(\",\")" "start,Start C-2026-009? Click Confirm."
expect import-live 15 "[$tc.status, $tv.result] | join(\",\")" "active,C-2026-009: queued → active"
expect import-live 19 "[$ti.open, $ti.result, $ti.canImport, ($ti.skipped | join(\"+\"))] | map(tostring) | join(\",\")" \
  "true,Nothing new to import · 3 tasks skipped,false,~/projects/TODO.md#2|already imported|C-2026-009+~/projects/TODO.md#5|done (- [x])|+~/projects/TODO.md#6|already imported|C-2026-010"
shows import-live 19 "Skipped ~/projects/TODO.md#6: already imported (C-2026-010)"
for i in 6 9 19; do expect import-live $i '.overflow | join(" | ")' ""; done
# the path one argument after `--`; `plan show` (read-only) only for the
# imported case, at least once before its Start (more when the index moves)
check "import-live: engine argv without plan show" "$(grep -v '^plan show ' "$work/home-import/argv.log" | tr '\n' '|')" \
  "$(printf '%s\n' "$startup" "$(q import task --json --dry-run -- '~/projects/TODO.md')" \
    "$(q import task --json -- '~/projects/TODO.md')" "$(q plan start C-2026-009 --json)" \
    "$(q import task --json --dry-run -- '~/projects/TODO.md')" | tr '\n' '|')"
check "import-live: plan show names only the imported case" \
  "$(grep '^plan show ' "$work/home-import/argv.log" | sort -u | tr '\n' '|')" "$(q plan show C-2026-009 --json)|"
check "import-live: plan show before Start" "$(grep -m 1 -e '^plan show ' -e '^plan start ' "$work/home-import/argv.log")" \
  "$(q plan show C-2026-009 --json)"
clean_log import-live

# The form's own check, the engine's refusal (shown in the form, fields
# kept after Esc), and a whole Intent the engine withholds: Start stays off.
mkdir -p "$work/home-import-refused"
run import-refused "" 1920x1080 \
  "summon:{\"section\":\"work\",\"select\":\"C-2026-007\"};settle;click:Import tasks…;type:notes.txt;key:Backspace*9;type:~/missing.md;key:Return;settle;key:Escape;click:Import tasks…;key:Escape;click:Start;click:Start;trigger:workDetail:start;trigger:workDetail:start" \
  HOME="$work/home-import-refused" FAKE_SELDON_FIXTURE="$sample" FAKE_SELDON_SHOW_WITHHELD=1
expect import-refused 2 "[$tc.imported, $tc.startEnabled, $tc.hint, $tc.review.text] | map(tostring) | join(\",\")" \
  "true,false,Start waits until the whole Intent below is shown; only you start an imported case,The engine withholds the Intent while the redaction patterns do not compile"
shows import-refused 2 "The engine withholds the Intent while the redaction patterns do not compile"
# withheld: the index's first paragraph stays
shows import-refused 2 "INTENT"
expect import-refused 4 "[$ti.pathError, $ti.canImport] | map(tostring) | join(\",\")" "Give the path from your home (~/…) or from / (absolute),false"
shows import-refused 4 "Give the path from your home (~/…) or from / (absolute)"
expect import-refused 8 "$ti.result" "~/missing.md: cannot read the task file: No such file or directory (os error 2)"
shows import-refused 8 "~/missing.md: cannot read the task file: No such file or directory (os error 2)"
expect import-refused 9 "[$ti.open, .view.keys] | map(tostring) | join(\",\")" "false,true"
expect import-refused 10 "[$ti.open, $ti.path] | map(tostring) | join(\",\")" "true,~/missing.md"
# the disabled Start: two clicks arm and run nothing (the argv below); two
# stray triggers behind the bar neither (press() holds on its own)
expect import-refused 13 "[$ti.open, $tc.armed, $tc.status, $tc.startEnabled] | map(tostring) | join(\",\")" "false,,queued,false"
expect import-refused 15 "[$tc.armed, $tc.status] | join(\",\")" ",queued"
argv_check import-refused "$work/home-import-refused" "$(printf '%s\n' "$startup" "$(q plan show C-2026-007 --json)" \
  "$(q import task --json --dry-run -- '~/missing.md')")"
clean_log import-refused 'import exit 1: ~/missing\.md: cannot read the task file'

# Round 2 (WP-102b): the dry run is for a path *and* an area — a new area
# turns Import off until its own dry run (P4), and the import carries it.
mkdir -p "$work/home-import-area/projects"
printf '%s\n' "- [ ] One" "- [ ] Two" >"$work/home-import-area/projects/TODO.md"
run import-area "" 1920x1080 \
  "summon:$wk;click:Import tasks…;type:~/projects/TODO.md;key:Return;settle;key:Tab;type:dev;key:Return;settle;click:Import 2 cases;settle" \
  HOME="$work/home-import-area" FAKE_SELDON_FIXTURE="$sample"
expect import-area 5 "[$ti.area, $ti.canImport] | map(tostring) | join(\",\")" ",true"
expect import-area 7 "[$ti.area, $ti.canImport] | map(tostring) | join(\",\")" "dev,false"
expect import-area 9 "[$ti.area, $ti.canImport] | map(tostring) | join(\",\")" "dev,true"
# `plan show` once or twice (the import's index may come while it runs)
check "import-area: engine argv without plan show" "$(grep -v '^plan show ' "$work/home-import-area/argv.log" | tr '\n' '|')" \
  "$(printf '%s\n' "$startup" "$(q import task --json --dry-run -- '~/projects/TODO.md')" \
    "$(q import task --json --dry-run --area dev -- '~/projects/TODO.md')" \
    "$(q import task --json --area dev -- '~/projects/TODO.md')" | tr '\n' '|')"
check "import-area: plan show names only the new case" \
  "$(grep '^plan show ' "$work/home-import-area/argv.log" | sort -u | tr '\n' '|')" "$(q plan show C-2026-009 --json)|"
clean_log import-area

# A new index asks the engine again (P2); meanwhile the last text stays on
# screen and Start is off (P5, N2); the answer brings Start back. Stage 2:
# the first `plan show` sees the index rewritten while it runs
# (FAKE_SELDON_SHOW_TOUCH): its answer enables nothing and is asked again
# once.
mkdir -p "$work/home-import-reask"
run import-reask "" 1920x1080 \
  "summon:{\"section\":\"work\",\"select\":\"C-2026-007\"};settle;wait:sectionView.case.reviewed=true;text:c;wait:sectionView.case.review.pending=true;wait:sectionView.case.reviewed=true" \
  HOME="$work/home-import-reask" FAKE_SELDON_FIXTURE="$sample" FAKE_SELDON_SHOW_TOUCH=1
first=$(sed -n 3p "$work/import-reask.steps" | jq -r "$tc.review.intent")
check "import-reask: the review's text" "$first" \
  "Imported from ~/Notizen/aufgaben.md#4 — read before you start this case.

Herdr-Orchestrator als Default-Agent registrieren — Agenten sollen über Herdr starten, damit Sitzungen sichtbar bleiben."
expect import-reask 3 "[$tc.reviewed, $tc.startEnabled] | map(tostring) | join(\",\")" "true,true"
expect import-reask 5 "[$tc.review.pending, $tc.review.ok, $tc.reviewed, $tc.startEnabled] | map(tostring) | join(\",\")" "true,true,false,false"
expect import-reask 5 "$tc.review.intent == $(jq -Rs . <<<"$first" | sed 's/\\n"$/"/')" true
shows import-reask 5 "IMPORTED TASK · 3 lines"
expect import-reask 6 "[$tc.reviewed, $tc.startEnabled] | map(tostring) | join(\",\")" "true,true"
# once on select, once more for the index that came while it ran, once for
# the capture's index
check "import-reask: the index rewritten during the first plan show" "$([[ -f $work/home-import-reask/show-touched ]] && echo yes)" yes
check "import-reask: plan show again on each new index" "$(grep -c '^plan show C-2026-007 ' "$work/home-import-reask/argv.log")" 3
clean_log import-reask

# Hidden characters marked and an Intent longer than the desk shows: Start
# stays off, the hint says why and where to read it (B1, B2).
mkdir -p "$work/home-import-hidden"
run import-hidden "" 1920x1080 \
  "summon:{\"section\":\"work\",\"select\":\"C-2026-007\"};settle;wait:sectionView.case.review.pending=false;click:Start;click:Start" \
  HOME="$work/home-import-hidden" FAKE_SELDON_FIXTURE="$sample" FAKE_SELDON_SHOW_HIDDEN=21 FAKE_SELDON_SHOW_TRUNCATED=1
expect import-hidden 3 "[$tc.review.ok, $tc.review.hidden, $tc.review.truncated, $tc.reviewed, $tc.startEnabled] | map(tostring) | join(\",\")" \
  "true,21,true,false,false"
expect import-hidden 3 "$tc.hint" \
  "21 hidden characters are marked and the Intent is longer than the desk shows: read the whole Intent in the editor; start this case from the terminal."
shows import-hidden 3 "21 hidden characters are marked ‹U+…› above: text you cannot see in the file. Read the case in the editor; start it from the terminal."
shows import-hidden 3 "The first 64 KiB are shown; the rest is in the case file. Read the whole Intent in the editor; start this case from the terminal."
expect import-hidden 5 "[$tc.armed, $tc.status] | join(\",\")" ",queued"
argv_check import-hidden "$work/home-import-hidden" "$(printf '%s\n' "$startup" "$(q plan show C-2026-007 --json)")"
clean_log import-hidden

# Dev mode (read-only): an imported case shows the index's first paragraph,
# says the whole Intent needs the engine, never enables Start; Enter does
# nothing; Import tasks… stays shut.
run import-dev "$sample" 1920x1080 "summon:{\"section\":\"work\",\"select\":\"C-2026-007\"};key:Return;click:Import tasks…;width:50"
expect import-dev 1 "[$tc.imported, $tc.startEnabled, $tc.hint, $tc.review.text] | map(tostring) | join(\",\")" \
  "true,false,Dev mode is read-only,Dev mode is read-only"
for text in "C-2026-007 · imported · R1 · dev-env" "IMPORTED TASK" "INTENT" "Dev mode is read-only" \
  "Herdr-Orchestrator als Default-Agent registrieren — Agenten sollen über Herdr starten, damit Sitzungen sichtbar bleiben."; do
  shows import-dev 1 "$text"
done
expect import-dev 2 "$tc.armed" ""
expect import-dev 3 "$ti.open" false
for i in 1 4; do expect import-dev $i '.overflow | join(" | ")' ""; done
clean_log import-dev

mkdir -p "$work/home-work-locked"
run work-locked "" 1920x1080 "summon:$wk;text:+;type:keep me;key:Return;settle;key:Escape;view;text:+" \
  HOME="$work/home-work-locked" FAKE_SELDON_FIXTURE="$sample" FAKE_SELDON_LOCKED=1
expect work-locked 2 "$ts.editing" true
expect work-locked 5 "[$ts.result, $ts.title, $ts.open, .view.lastError] | map(tostring) | join(\",\")" "the logbook is locked by another seldon (pid 4242),keep me,true,"
shows work-locked 5 "the logbook is locked by another seldon (pid 4242)"
expect work-locked 6 "[$ts.open, .view.opened, .view.keys] | map(tostring) | join(\",\")" "false,true,true"
expect work-locked 7 "$tv.groups | join(\",\")" "active 2,verification 1,queued 3,completed 2"
expect work-locked 8 "[$ts.open, $ts.title] | map(tostring) | join(\",\")" "true,keep me"
clean_log work-locked

intent=' Install tool X. It needs --help $(id) and one package'
mkdir -p "$work/home-run"
run work-run "" 1920x1080 "summon:$wk;text:i;type:$intent;key:Return;settle;summon:$wk;wait:sectionView.selected=C-2026-009;view" \
  HOME="$work/home-run" FAKE_SELDON_FIXTURE="$sample"
expect work-run 2 "[$tv.intentEditing, .view.keys] | map(tostring) | join(\",\")" "true,false"
expect work-run 3 "$tv.intent" "$intent"
# the agent started: the desk steps aside (WP-156) and remembers the new case
expect work-run 5 "[.view.opened, .service.stepAsides] | map(tostring) | join(\",\")" "false,1"
expect work-run 7 "[$tv.result, $tv.resultOk, $tv.intent, $tc.status] | map(tostring) | join(\",\")" \
  "Created C-2026-009 ·  Install tool X · agent started · launcher default (omarchy),true,,active"
expect work-run 7 "$tv.groups | join(\",\")" "active 3,verification 1,queued 3,completed 2"
expect work-run 8 "[.view.keys, .view.opened] | map(tostring) | join(\",\")" "true,true"
check "work-run: agent start --new, the sentence one argument after --" "$(grep '^agent' "$work/home-run/argv.log" 2>/dev/null)" \
  "$(q agent start --new --json -- "$intent")"
clean_log work-run
mkdir -p "$work/home-run-refused"
run work-run-refused "" 1920x1080 "summon:$wk;text:i;type:Install zed;key:Return;settle" \
  HOME="$work/home-run-refused" FAKE_SELDON_FIXTURE="$sample" FAKE_SELDON_NO_DEFAULT_AGENT=1
expect work-run-refused 5 "[$tv.resultOk, $tv.intent, ($tv.groups | join(\"+\"))] | map(tostring) | join(\",\")" "false,Install zed,active 2+verification 1+queued 3+completed 2"
expect work-run-refused 5 "$tv.result" "no default agent: Omarchy has none set, so \`omarchy agent prompt\` cannot start one; nothing was created. Fix: \`omarchy default agent <name>\` (e.g. claude), or set \`[agent] launcher\` in ~/.config/seldon/config.toml"
clean_log work-run-refused

mkdir -p "$work/home-reopen"
run work-reopen "" 1920x1080 \
  "summon:$wk;select:C-2026-002;click:Reopen;settle;wait:sectionView.selected=C-2026-009;select:C-2026-002;text:r;settle;wait:sectionView.selected=C-2026-010;select:C-2026-003;text:r;settle" \
  HOME="$work/home-reopen" FAKE_SELDON_FIXTURE="$sample"
expect work-reopen 5 "[$tv.result, ($tv.groups | join(\"+\")), ($tc.kv[] | select(startswith(\"Reopens\")))] | join(\",\")" \
  "Reopened C-2026-002 as C-2026-009 (active),active 3+verification 1+queued 3+completed 2,Reopens: C-2026-002"
expect work-reopen 9 "[$tv.result, $tc.id, $tc.status] | join(\",\")" \
  "Reopened C-2026-002 as C-2026-010 (active) · reopened before as C-2026-009 · the active case stays C-2026-009,C-2026-010,active"
check "work-reopen: plan reopen C-2026-002 --json, twice, nothing for r on an open case" "$(grep '^plan' "$work/home-reopen/argv.log" 2>/dev/null | tr '\n' '|')" \
  "$(printf '%s\n' "$(q plan reopen C-2026-002 --json)" "$(q plan reopen C-2026-002 --json)" | tr '\n' '|')"
clean_log work-reopen

# 8e. A section change gives the keys back (panel 25): a field of a hidden
#     section never keeps them, its draft stays, nothing is sent; the
#     Changelog's selection follows its event across an index update and a
#     new chip starts at the top (panel 26, the acceptance's cursor
#     stability); Capture now replaces a pending lock retry (panel 27); the
#     sticky bar while the detail scrolls; the stacked layout.
mkdir -p "$work/home-tab-focus"
run tab-focus "" 1920x1080 \
  "summon;text:n;type:abc;click:Work;text:j;key:Return;settle;text:1;text:n;key:Escape;text:+;type:xyz;section:changelog;key:Down;settle;text:3;text:1;text:n;key:Tab;key:Down;section:work" \
  HOME="$work/home-tab-focus" FAKE_SELDON_FIXTURE="$sample"
expect tab-focus 2 "[$tv.journal.editing, .view.keys] | map(tostring) | join(\",\")" "true,false"
expect tab-focus 4 "[.view.section, .view.keys, .view.editing] | map(tostring) | join(\",\")" "work,true,false"
expect tab-focus 5 "$tv.selected" C-2026-004
expect tab-focus 6 "$tv.case.armed" verify
expect tab-focus 8 "[.view.section, $tv.journal.text, $tv.journal.result] | join(\",\")" "today,abc,"
expect tab-focus 9 "[$tv.journal.editing, $tv.journal.text] | map(tostring) | join(\",\")" "true,abc"
expect tab-focus 11 "[.view.section, $tv.sheet.editing] | map(tostring) | join(\",\")" "work,true"
expect tab-focus 12 "$tv.sheet.title" xyz
expect tab-focus 13 "[.view.section, .view.keys, .view.editing] | map(tostring) | join(\",\")" "changelog,true,false"
expect tab-focus 14 "$tv.cursor" 1
expect tab-focus 16 "[.view.section, $tv.sheet.open, $tv.sheet.title, $tv.result] | map(tostring) | join(\",\")" "work,true,xyz,"
expect tab-focus 18 "$tv.journal.editing" true
expect tab-focus 21 "[.view.section, .view.keys, .view.editing] | map(tostring) | join(\",\")" "work,true,false"
argv_check tab-focus "$work/home-tab-focus" "$startup"
clean_log tab-focus

jq '.events = [
    {id: "01M3W2NEWEVENT000000000001", ts: "2026-10-01T18:31:00+02:00", source: "manual", kind: "note",
     subject: "journal", detail: "Second event above the cursor", zone: "green", actor: "human", case: null},
    {id: "01M3W2NEWEVENT000000000000", ts: "2026-10-01T18:30:00+02:00", source: "manual", kind: "note",
     subject: "journal", detail: "First event above the cursor", zone: "green", actor: "human", case: null}
  ] + .events' "$sample" >"$work/after-two.json"
mkdir -p "$work/home-cursor-follow"
run cursor-follow "" 1920x1080 \
  'summon:{"section":"changelog","filter":"all"};key:Down*4;text:c;wait:sectionView.rows=85;key:Return;key:Escape;text:F' \
  HOME="$work/home-cursor-follow" FAKE_SELDON_FIXTURE="$sample" FAKE_SELDON_FIXTURE_AFTER="$work/after-two.json"
expect cursor-follow 1 "[$tv.chip, $tv.cursor] | map(tostring) | join(\",\")" "all,0"
expect cursor-follow 2 "[$tv.cursor, $tv.selected] | map(tostring) | join(\",\")" "4,$THEME"
expect cursor-follow 4 "[$tv.rows, $tv.cursor, $tv.selected] | map(tostring) | join(\",\")" "85,6,$THEME"
expect cursor-follow 5 "[$tv.detail.form.shown, $tv.detail.form.eventId] | map(tostring) | join(\",\")" "true,$THEME"
expect cursor-follow 7 "[$tv.chip, $tv.cursor, $tv.selected != \"$THEME\"] | map(tostring) | join(\",\")" "case,0,true"
clean_log cursor-follow

mkdir -p "$work/home-capture-click"
run capture-click "" 1920x1080 "summon:$cl;view;click:Capturing;settle;view" \
  HOME="$work/home-capture-click" FAKE_SELDON_FIXTURE="$sample" FAKE_SELDON_CAPTURE_LOCKED=1 SELDON_LOCK_RETRY_MS=50000
expect capture-click 2 "[$tv.capturing, $tv.captureResult] | map(tostring) | join(\",\")" "true,waiting for another seldon process; trying again shortly"
shows capture-click 2 "Capturing"
expect capture-click 5 "[$tv.capturing, $tv.captureResult, .view.lastError] | map(tostring) | join(\",\")" "false,nothing new,"
shows capture-click 5 "Capture now"
check "capture-click: locked captures" "$(cat "$work/home-capture-click/locked-captures" 2>/dev/null || echo 0)" 1
check "capture-click: captures" "$(cat "$work/home-capture-click/captures" 2>/dev/null || echo 0)" 1
clean_log capture-click

# The sticky bar: a detail taller than its pane scrolls under the bar,
# which keeps its place (ADR-0034 §2); the same in the Changelog.
run sticky "$sample" 1920x640 \
  "summon:$wk;view;wheel:deskDetailScroll:-480;wheel:deskDetailScroll:-480;shot:sticky-work;resize:1920x420;text:2;select:$MESA;view;wheel:deskDetailScroll:-480;wheel:deskDetailScroll:-480;pause:300"
expect sticky 2 "[$tv.scroll.y, ($tv.scroll.h > $tv.scroll.view)] | map(tostring) | join(\",\")" "0,true"
expect sticky 4 "$tv.scroll.y > 0" true
bar_y=$(sed -n 2p "$work/sticky.steps" | jq -r "$tv.bar.sceneY")
for i in 2 3 4; do
  expect sticky $i "[$tv.bar.y, $tv.bar.visible, $tv.bar.sceneY] | map(tostring) | join(\",\")" "0,true,$bar_y"
  shows sticky $i "Hand to agent"
  expect sticky $i '.overflow | join(" | ")' ""
done
expect sticky 9 "[$tv.detail.scroll.y, ($tv.detail.scroll.h > $tv.detail.scroll.view)] | map(tostring) | join(\",\")" "0,true"
bar_y=$(sed -n 9p "$work/sticky.steps" | jq -r "$tv.detail.bar.sceneY")
expect sticky 12 "[$tv.detail.scroll.y > 0, $tv.detail.bar.y, $tv.detail.bar.visible, $tv.detail.bar.sceneY] | map(tostring) | join(\",\")" "true,0,true,$bar_y"
shows sticky 12 "Explain…"
clean_log sticky

# Stacked (a desk under 760 px): the list, Enter shows the detail, Esc
# goes back; in Work Enter on the detail arms the primary action.
run stacked-sections "$sample" 700x900 \
  "summon:$cl;key:Return;key:Escape;text:3;key:Return;key:Return;key:Escape;text:1;key:Down;key:Return;clickName:deskBack"
expect stacked-sections 1 "[.view.layout.stacked, .view.detailShown] | map(tostring) | join(\",\")" "true,false"
shows stacked-sections 1 "tokyo-night"
expect stacked-sections 2 .view.detailShown true
shows stacked-sections 2 "‹ Back to the list"
shows stacked-sections 2 "Link to C-2026-005…"
expect stacked-sections 3 "[.view.detailShown, .view.opened] | map(tostring) | join(\",\")" "false,true"
expect stacked-sections 5 "[.view.section, .view.detailShown] | map(tostring) | join(\",\")" "work,true"
expect stacked-sections 6 "$tv.case.armed" ""
expect stacked-sections 7 .view.detailShown false
expect stacked-sections 9 "$tv.shown" event
expect stacked-sections 10 .view.detailShown true
expect stacked-sections 11 .view.detailShown false
for i in 1 2 5 10; do expect stacked-sections $i '.overflow | join(" | ")' ""; done
clean_log stacked-sections

# Not initialised (panel 5): the three sections are empty and say so;
# `+` opens no sheet, Enter does nothing.
run sections-uninit "$fx/index-variants/not-initialised.json" 1920x1080 "summon;text:2;text:3;text:+;key:Return"
expect sections-uninit 1 "[$tv.rows, $tv.headline, $tv.state] | map(tostring) | join(\",\")" "1,No index to show,"
shows sections-uninit 1 "No index to show"
expect sections-uninit 2 "[$tv.rows, ($tv.chips | join(\"+\")), $tv.detail.found] | map(tostring) | join(\",\")" "0,open 0+crisis 0+attention 0+routine 0+case 0+all 0,false"
shows sections-uninit 2 "No index to show"
expect sections-uninit 3 "[($tv.ids | length), $tv.case, $tv.wip] | map(tostring) | join(\",\")" "0,null,0 / 3 active"
shows sections-uninit 3 "No index to show"
expect sections-uninit 4 "$tv.sheet.open" false
expect sections-uninit 5 "[.view.opened, .view.keys] | map(tostring) | join(\",\")" "true,true"
for i in 1 2 3; do expect sections-uninit $i '.overflow | join(" | ")' ""; done
clean_log sections-uninit

# 8f. Review round 2 (WP-122): the "why loud" callout from the engine's
#     rule (`seldon drift show`, live) — a path, a planned crisis whose
#     callout agrees with the Case and Rule rows, `attention = "all"`, a
#     pacman group from a member; Open case for a case the index no longer
#     lists; key/values wrap at word boundaries at 50 %.
expected_warnings="$expected_warnings|jax\\.seldon: seldon open exit 1: unknown case C-2026-001\$"

# B1: the callout from the engine's rule, live: the index's own (ADR-0038
# §1), so no click starts a process.
mkdir -p "$work/home-why"
run why-loud "" 1920x1080 \
  "summon:$(sel $UNIT);wait:sectionView.detail.rule=known always-red-paths;select:$HOOK_EVENT;wait:sectionView.detail.rule=known always-red-paths;text:1;key:Down" \
  HOME="$work/home-why" FAKE_SELDON_FIXTURE="$sample"
expect why-loud 2 "$td.whyLoud" "The path matches your crisis list ([drift] alwaysRedPaths in ~/.config/seldon/config.toml). No open case plans it, and no case is linked."
expect why-loud 2 "[$td.kv[] | select(startswith(\"Case\") or startswith(\"Rule\"))] | join(\" | \")" "Case: — | Rule: crisis · rule always-red-paths · no case"
shows why-loud 2 "Why loud?"
expect why-loud 4 "[$td.id, $td.rule] | join(\",\")" "$HOOK_EVENT,known always-red-paths"
expect why-loud 6 "[$tv.shown, $tv.detail.rule] | join(\",\")" "event,known always-red-paths"
argv_check why-loud "$work/home-why" "$startup"
clean_log why-loud

# … an index without `rule` (an earlier contract-2 engine): `drift show`
# names it, once per selected crisis — the fallback, unchanged.
jq 'del(.drift[].rule)' "$sample" >"$work/no-rule.json"
mkdir -p "$work/home-why-bare"
run why-loud-bare "" 1920x1080 \
  "summon:$(sel $UNIT);wait:sectionView.detail.rule=known always-red-paths;select:$HOOK_EVENT;wait:sectionView.detail.rule=known always-red-paths;text:1;key:Down" \
  HOME="$work/home-why-bare" FAKE_SELDON_FIXTURE="$work/no-rule.json"
expect why-loud-bare 2 "$td.whyLoud" "The path matches your crisis list ([drift] alwaysRedPaths in ~/.config/seldon/config.toml). No open case plans it, and no case is linked."
expect why-loud-bare 4 "[$td.id, $td.rule] | join(\",\")" "$HOOK_EVENT,known always-red-paths"
argv_check why-loud-bare "$work/home-why-bare" "$(printf '%s\n' "$startup" "$(q drift show $UNIT --json)" "$(q drift show $HOOK_EVENT --json)")"
clean_log why-loud-bare
run why-loud-bare-dev "$work/no-rule.json" 1920x1080 "summon:$(sel $UNIT)"
expect why-loud-bare-dev 1 "$td.whyLoud" "The engine classed this config change as a crisis; \`seldon drift show $UNIT\` names the rule. No open case plans it, and no case is linked."
clean_log why-loud-bare-dev

# … when an open case's plan names the crisis: the callout, the Case and
# the Rule rows say the same.
jq --arg u "$UNIT" '.drift |= map(if .eventId == $u then .proposedCase = "C-2026-003" else . end)' "$sample" >"$work/planned-crisis.json"
mkdir -p "$work/home-why-planned"
run why-loud-planned "" 1920x1080 "summon:$(sel $UNIT);wait:sectionView.detail.rule=known always-red-paths" \
  HOME="$work/home-why-planned" FAKE_SELDON_FIXTURE="$work/planned-crisis.json"
expect why-loud-planned 2 "$td.whyLoud" "The path matches your crisis list ([drift] alwaysRedPaths in ~/.config/seldon/config.toml). C-2026-003 plans it (its plan names this change); nothing has linked it yet."
expect why-loud-planned 2 "[$td.kv[] | select(startswith(\"Case\") or startswith(\"Rule\"))] | join(\" | \")" "Case: proposed: C-2026-003 | Rule: crisis · rule always-red-paths · planned by C-2026-003, not linked"
expect why-loud-planned 2 "$td.actions[0:2] | join(\"+\")" "Ask agent+Link to C-2026-003…"
clean_log why-loud-planned

# … under `[drift] attention = "all"`: a crisis is a red-zone change (the
# fallback's answer).
mkdir -p "$work/home-why-all"
run why-loud-all "" 1920x1080 "summon:$(sel $UNIT);wait:sectionView.detail.rule=known attention-all" \
  HOME="$work/home-why-all" FAKE_SELDON_FIXTURE="$work/no-rule.json" FAKE_SELDON_ATTENTION_ALL=1
expect why-loud-all 2 "$td.whyLoud" "[drift] attention = \"all\" is set: every change without a case is open drift, and a crisis is a change in the red zone. No open case plans it, and no case is linked."
clean_log why-loud-all

# … a pacman group in crisis, from a member: the leader's rule.
jq --arg m "$MESA" '.summary.crisis = 3 | .drift |= map(if .eventId == $m then .crisis = true else . end)' "$work/no-rule.json" >"$work/group-crisis.json"
mkdir -p "$work/home-why-group"
run why-loud-group "" 1920x1080 "summon:$(sel $LIB32);wait:sectionView.detail.rule=known always-red" \
  HOME="$work/home-why-group" FAKE_SELDON_FIXTURE="$work/group-crisis.json"
expect why-loud-group 2 "[$td.cls, ($td.whyLoud | startswith(\"A package on your crisis list ([drift] alwaysRed in ~/.config/seldon/config.toml)\"))] | map(tostring) | join(\",\")" "crisis,true"
argv_check why-loud-group "$work/home-why-group" "$(printf '%s\n' "$startup" "$(q drift show $MESA --json)")"
clean_log why-loud-group

# B2 after a Hide: the mesa group (three events) is one change hidden.
run hide-group "$sample" 1920x1080 "summon:$(sel $MESA);click:Hide"
expect hide-group 2 "[$tv.hidden, $tv.rows, ($tv.chips[0:3] | join(\"+\")), .view.counts.changelog.text] | map(tostring) | join(\",\")" \
  "1,5,open 5+crisis 2+attention 3,6"
shows hide-group 2 "1 change hidden this session"
clean_log hide-group

# N5: Open case for a case the index no longer lists: the line, and Open
# in editor asks the engine, whose answer replaces it.
C1_NOTE=01M1F161B0EJ8KHDPZAK9M0GTD
jq '.cases.completed |= map(select(.id != "C-2026-001"))' "$sample" >"$work/case-gone.json"
mkdir -p "$work/home-case-gone"
run case-gone "" 1920x1080 \
  "summon:{\"section\":\"changelog\",\"filter\":\"case\",\"select\":\"$C1_NOTE\"};click:Open case;click:Open in editor;settle" \
  HOME="$work/home-case-gone" FAKE_SELDON_FIXTURE="$work/case-gone.json" FAKE_SELDON_UNKNOWN_CASE=C-2026-001
expect case-gone 2 "[.view.section, $td.caseMissing] | join(\",\")" "changelog,C-2026-001 is not in the index any more (it keeps the last 50 completed cases)."
shows case-gone 2 "Open in editor"
expect case-gone 4 "$td.caseMissing" "unknown case C-2026-001"
argv_check case-gone "$work/home-case-gone" "$(printf '%s\n' "$startup" "$(q open C-2026-001 --editor --json)")"
clean_log case-gone "jax\\.seldon: seldon open exit 1: "

# Contract 2 (ADR-0035 §3): a detail the index clipped says so in the
# What row (the sample's note with meta.truncated; a drift item's
# truncated, on the crisis, also through its `drift show` rule, live).
run clipped "$sample" 1920x1080 "summon:{\"section\":\"changelog\",\"select\":\"01M2ACN5Q043TW9W44NJEZ4K9S\"}"
expect clipped 1 "[$td.kv[] | select(startswith(\"What: \"))] | .[0] | endswith(\" (clipped in the index; the ledger has it in full)\")" true
clean_log clipped
jq --arg u "$UNIT" '.drift |= map(if .eventId == $u then .truncated = true else . end)' "$sample" >"$work/drift-truncated.json"
mkdir -p "$work/home-why-truncated"
run why-loud-truncated "" 1920x1080 "summon:$(sel $UNIT);wait:sectionView.detail.rule=known always-red-paths" \
  HOME="$work/home-why-truncated" FAKE_SELDON_FIXTURE="$work/drift-truncated.json"
expect why-loud-truncated 2 "[($td.kv[] | select(startswith(\"What: \")) | endswith(\"(clipped in the index; the ledger has it in full)\")), ($td.whyLoud | startswith(\"The path matches your crisis list\"))] | map(tostring) | join(\",\")" "true,true"
clean_log why-loud-truncated

# N4: key/values wrap at word boundaries; at 50 % nothing leaves the desk.
run kv-wrap "$sample" 1920x1080 "summon:$wk;width:50;shot:kv-wrap-50"
expect kv-wrap 2 '.overflow | join(" | ")' ""
expect kv-wrap 2 "[.texts[] | select(startswith(\"R3 · every step\"))] | length" 1
clean_log kv-wrap

# ADR-0038: the details show what the index carries — an imported case's
# intent (after its provenance line) and source, a completed case's
# result, a decision's lead, each as plain text with Open in editor kept;
# an index without the four fields renders as before.
run details "$sample" 1920x1080 \
  "summon:{\"section\":\"work\",\"select\":\"C-2026-007\"};call:select:C-2026-001;text:4;call:select:ADR-0003"
expect details 1 "[$tc.id, $tc.intent, $tc.result] | join(\"|\")" \
  "C-2026-007|Herdr-Orchestrator als Default-Agent registrieren — Agenten sollen über Herdr starten, damit Sitzungen sichtbar bleiben.|"
expect details 1 "[$tc.kv[] | select(startswith(\"Imported from\"))] | join(\",\")" "Imported from: ~/Notizen/aufgaben.md#4"
for text in "INTENT" "Herdr-Orchestrator als Default-Agent registrieren — Agenten sollen über Herdr starten, damit Sitzungen sichtbar bleiben." \
  "~/Notizen/aufgaben.md#4" "Open in editor"; do
  shows details 1 "$text"
done
expect details 1 '[.texts[] | select(. == "RESULT")] | length' 0
expect details 2 "[$tc.id, $tc.result] | join(\"|\")" "C-2026-001|Logbuch läuft, Baseline erfasst, \`seldon doctor\` ohne Befund."
shows details 2 "RESULT"
expect details 4 "[.view.selected, $tv.text] | join(\"|\")" "ADR-0003|Zed wird Zweiteditor, Neovim bleibt Standard."
shows details 4 "Zed wird Zweiteditor, Neovim bleibt Standard."
for i in 1 2 4; do expect details $i '.overflow | join(" | ")' ""; done
clean_log details
jq 'del(.drift[].rule) | .cases[][] |= del(.intent, .result, .source) | .decisions[] |= del(.lead)' "$sample" >"$work/bare.json"
run details-bare "$work/bare.json" 1920x1080 \
  "summon:{\"section\":\"work\",\"select\":\"C-2026-007\"};text:4;call:select:ADR-0003"
expect details-bare 1 "[$tc.id, $tc.intent, $tc.result, ([$tc.kv[] | select(startswith(\"Imported from\"))] | length)] | map(tostring) | join(\"|\")" "C-2026-007|||0"
shows details-bare 1 "0 of 5 steps done. The steps, the Intent and the Result are in the case file."
expect details-bare 1 '[.texts[] | select(. == "INTENT" or . == "RESULT")] | length' 0
expect details-bare 3 "[.view.selected, $tv.text] | join(\"|\")" "ADR-0003|"
shows details-bare 3 "The text is in the file; Open in editor shows it."
clean_log details-bare

# ---------------------------------------------------------------------------
# 8g. The desk steps aside for what it opens; one agent per case (WP-156).
#     A launch the engine answered closes the desk through the facade (as
#     Esc does), a refusal keeps it open with the engine's text; an active
#     case an agent works on shows Focus in place of Hand to agent; a
#     button whose call is in flight is busy; a double press sends one
#     call; the same open is not sent again within 2 s.
w3='{"section":"work","select":"C-2026-003"}'
w4='{"section":"work","select":"C-2026-004"}'
sv='.service'
hides='[.calls[] | select(startswith("hide"))] | length'
mkdir -p "$work/home-aside"
run aside "" 1920x1080 \
  "summon:$w3;text:a;text:a;settle;pause:800;summon:$w3;text:a;settle;summon:$w4;click:Hand to agent;click:Confirm hand to agent;settle;pause:800;summon:$w4" \
  HOME="$work/home-aside" FAKE_SELDON_FIXTURE="$sample"
expect aside 1 "$tc.actions | join(\",\")" "Hand to agent,To verification,Drop,Open in editor,Ask agent"
expect aside 1 "$sv.sessions | length" 0
expect aside 3 "[($tc.actions | join(\",\")), $tc.enabled[0], $sv.planPending] | map(tostring) | join(\"|\")" \
  "Starting…,To verification,Drop,Open in editor,Ask agent|false|true"
shows aside 3 "Starting…"
# launched: the desk is gone, through the facade's hide
expect aside 4 "[.view.opened, $sv.stepAsides, $sv.planOk, ($hides)] | map(tostring) | join(\",\")" "false,1,true,1"
expect aside 4 "$sv.plan" "Agent started on C-2026-003 · launcher default (omarchy)"
expect aside 5 "$sv.sessions | join(\",\")" "C-2026-003"
# opened again: Focus in place of a second Hand to agent
expect aside 6 "[$tv.selected, ($tc.actions | join(\",\")), $tc.meta, $tc.working] | map(tostring) | join(\"|\")" \
  "C-2026-003|Focus,To verification,Drop,Open in editor,Ask agent|agent working · C-2026-003 · R3|true"
expect aside 6 "$tc.kv[] | select(startswith(\"Agent:\"))" "Agent: working now · agent:default · workspace 2"
expect aside 6 "$tv.working | join(\",\")" "C-2026-003"
# the row's aside and the bar's meta
expect aside 6 '[.texts[] | select(startswith("agent working · "))] | length' 2
shows aside 6 "agent working · C-2026-003 · R3"
shows aside 6 "Focus"
# `a` focuses at once, no arm; busy, then aside
expect aside 7 "[$tc.armed, $tc.actions[0], $tc.enabled[0]] | map(tostring) | join(\",\")" ",Focusing…,false"
expect aside 8 "[.view.opened, $sv.stepAsides, $sv.plan] | map(tostring) | join(\",\")" \
  "false,2,The agent on C-2026-003 is in front · workspace 2"
check "aside: focus reached the engine once" "$(cat "$work/home-aside/focus.log" 2>/dev/null)" "C-2026-003"
# a click hands another case to an agent; it steps aside too
expect aside 9 "$tc.actions | join(\",\")" "Hand to agent,To verification,Drop,Open in editor,Ask agent"
expect aside 11 "$tc.actions[0]" "Starting…"
expect aside 12 "[.view.opened, $sv.stepAsides] | map(tostring) | join(\",\")" "false,3"
expect aside 13 "$sv.sessions | join(\",\")" "C-2026-003,C-2026-004"
expect aside 14 "$tc.actions | join(\",\")" "Focus,To verification,Drop,Open in editor,Ask agent"
argv_check aside "$work/home-aside" "$(printf '%s\n' "$startup" "$(q agent start C-2026-003 --json)" \
  "$(q agent focus C-2026-003 --json)" "$(q agent start C-2026-004 --json)")"
check "aside: the sessions asked beside the queue" \
  "$(sort -u "$work/home-aside/sessions.log" 2>/dev/null)" "$(q agent sessions --json)"
clean_log aside

# The desk had not seen the session: the engine refuses, the desk stays
# open with its text, asks again and shows Focus.
mkdir -p "$work/home-aside-stale"
echo C-2026-003 >"$work/home-aside-stale/sessions"
run aside-stale "" 1920x1080 "summon:$w3;pause:600;text:a;text:a;settle;pause:800" \
  HOME="$work/home-aside-stale" FAKE_SELDON_FIXTURE="$sample" FAKE_SELDON_SESSIONS_LATE=1
expect aside-stale 2 "$tc.actions[0]" "Hand to agent"
already='an agent is already working on C-2026-003 (window 0xf0c5 on workspace 2); focus it with `seldon agent focus C-2026-003`, or start another with `seldon agent start C-2026-003 --again`; nothing was launched'
expect aside-stale 5 "[.view.opened, $sv.stepAsides, $tv.resultOk] | map(tostring) | join(\",\")" "true,0,false"
expect aside-stale 5 "$tv.result" "$already"
shows aside-stale 5 "$already"
expect aside-stale 6 "$tc.actions | join(\",\")" "Focus,To verification,Drop,Open in editor,Ask agent"
clean_log aside-stale 'agent exit 1: an agent is already working on C-2026-003'

# The session ended since the desk asked: Focus is refused, the desk stays
# open and shows Hand to agent again.
mkdir -p "$work/home-aside-gone"
echo C-2026-003 >"$work/home-aside-gone/sessions"
run aside-gone "" 1920x1080 "summon:$w3;pause:600;text:a;settle;pause:800" \
  HOME="$work/home-aside-gone" FAKE_SELDON_FIXTURE="$sample" FAKE_SELDON_FOCUS_GONE=1
expect aside-gone 2 "$tc.actions[0]" "Focus"
gone='no agent is working on C-2026-003: no window of an agent `seldon agent start` launched on it is open; start one with `seldon agent start C-2026-003`'
expect aside-gone 4 "[.view.opened, $sv.stepAsides, $tv.result] | map(tostring) | join(\",\")" "true,0,$gone"
expect aside-gone 5 "$tc.actions | join(\",\")" "Hand to agent,To verification,Drop,Open in editor,Ask agent"
clean_log aside-gone 'agent exit 1: no agent is working on C-2026-003'

# Double presses: one agent start, one open; the same open again within
# 2 s sends nothing (the desk stays), after 2 s it is sent. The fake holds
# each open until `touch:release-open` (FAKE_SELDON_HOLD_OPEN), so "in
# flight" is a state of the case, not a race (round 2, N4).
mkdir -p "$work/home-aside-double"
run aside-double "" 1920x1080 \
  "summon:$w3;text:a*4;settle;summon:$w4;text:e*3;touch:release-open;settle;summon:$w4;text:e;settle;pause:2100;text:e;touch:release-open;settle;summon:{\"section\":\"today\"};text:e*3;touch:release-open;settle" \
  HOME="$work/home-aside-double" FAKE_SELDON_FIXTURE="$sample" FAKE_SELDON_NO_SESSION=1 FAKE_SELDON_HOLD_OPEN=1 \
  HARNESS_RECORD="$work/aside-double.record"
expect aside-double 2 "[$sv.planPending, $tc.actions[0]] | map(tostring) | join(\",\")" "true,Starting…"
expect aside-double 3 "[.view.opened, $sv.stepAsides] | map(tostring) | join(\",\")" "false,1"
expect aside-double 5 "[$sv.openPending, $tc.actions[3], $tc.enabled[3]] | map(tostring) | join(\",\")" "true,Opening…,false"
shows aside-double 5 "Opening…"
expect aside-double 7 "[.view.opened, $sv.stepAsides, $sv.open] | map(tostring) | join(\",\")" \
  "false,2,Opened $work/home-aside-double/Seldon/work/active/C-2026-004.md in omarchy-launch-editor"
expect aside-double 9 "[.view.opened, $sv.openPending] | map(tostring) | join(\",\")" "true,false"
expect aside-double 10 "[.view.opened, $sv.stepAsides] | map(tostring) | join(\",\")" "true,2"
expect aside-double 12 "[.view.opened, $sv.openPending] | map(tostring) | join(\",\")" "true,true"
expect aside-double 14 "[.view.opened, $sv.stepAsides] | map(tostring) | join(\",\")" "false,3"
# Today's `e` has no guard of its own: the service's one open at a time
expect aside-double 16 "[$sv.openPending, $sv.busyRefusals] | map(tostring) | join(\",\")" "true,2"
expect aside-double 18 "[.view.opened, $sv.stepAsides] | map(tostring) | join(\",\")" "false,4"
argv_check aside-double "$work/home-aside-double" "$(printf '%s\n' "$startup" "$(q agent start C-2026-003 --json)" \
  "$(q open C-2026-004 --editor --json)" "$(q open C-2026-004 --editor --json)" "$(q open journal --editor --json)")"
clean_log aside-double

# No Hyprland to ask (FAKE_SELDON_NO_TRACKING): the engine tracks nothing,
# so the desk keeps Hand to agent and a second hand-off launches again;
# the busy state and the 2 s floor stay (round 2).
mkdir -p "$work/home-aside-nowindow"
run aside-nowindow "" 1920x1080 "summon:$w3;text:a;text:a;settle;pause:800;summon:$w3;text:a;text:a;settle" \
  HOME="$work/home-aside-nowindow" FAKE_SELDON_FIXTURE="$sample" FAKE_SELDON_NO_TRACKING=1
expect aside-nowindow 4 "[.view.opened, $sv.stepAsides] | map(tostring) | join(\",\")" "false,1"
expect aside-nowindow 5 "$sv.sessions | length" 0
expect aside-nowindow 6 "[$tc.actions[0], $tc.working] | map(tostring) | join(\",\")" "Hand to agent,false"
expect aside-nowindow 9 "[.view.opened, $sv.stepAsides] | map(tostring) | join(\",\")" "false,2"
argv_check aside-nowindow "$work/home-aside-nowindow" "$(printf '%s\n' "$startup" "$(q agent start C-2026-003 --json)" \
  "$(q agent start C-2026-003 --json)")"
clean_log aside-nowindow

# An index change while the desk is open asks for the sessions again: the
# first answer has none, a capture rewrites the index, the second shows the
# agent's window (round 2, N3/P1).
mkdir -p "$work/home-aside-index"
echo C-2026-003 >"$work/home-aside-index/sessions"
run aside-index "" 1920x1080 "summon:$w3;pause:600;text:c;settle;pause:800" \
  HOME="$work/home-aside-index" FAKE_SELDON_FIXTURE="$sample" FAKE_SELDON_SESSIONS_LATE=1
expect aside-index 2 "$tc.actions[0]" "Hand to agent"
expect aside-index 5 "$tc.actions | join(\",\")" "Focus,To verification,Drop,Open in editor,Ask agent"
clean_log aside-index

# A failed open is no open: the same target again is sent at once (round 2,
# N3/P5: the 2 s rule starts only from a successful open).
mkdir -p "$work/home-aside-openfail"
run aside-openfail "" 1920x1080 "summon:$w4;text:e;settle;text:e;settle" \
  HOME="$work/home-aside-openfail" FAKE_SELDON_FIXTURE="$sample" FAKE_SELDON_UNKNOWN_CASE=C-2026-004
expect aside-openfail 3 "[.view.opened, $sv.stepAsides, $sv.open] | map(tostring) | join(\",\")" "true,0,unknown case C-2026-004"
argv_check aside-openfail "$work/home-aside-openfail" "$(printf '%s\n' "$startup" "$(q open C-2026-004 --editor --json)" \
  "$(q open C-2026-004 --editor --json)")"
clean_log aside-openfail 'open exit 1: unknown case C-2026-004'

# An editor the engine opened on the file before: focused, nothing
# launched, the desk steps aside.
mkdir -p "$work/home-aside-focused"
run aside-focused "" 1920x1080 "summon:$w4;text:e;settle" \
  HOME="$work/home-aside-focused" FAKE_SELDON_FIXTURE="$sample" FAKE_SELDON_OPEN_FOCUSED=1 HARNESS_RECORD="$work/aside-focused.record"
expect aside-focused 3 "[.view.opened, $sv.stepAsides, $sv.open] | map(tostring) | join(\",\")" \
  "false,1,$work/home-aside-focused/Seldon/work/active/C-2026-004.md is already open; its window is in front"
check "aside-focused: no editor launched" "$(cat "$work/aside-focused.record" 2>/dev/null)" ""
clean_log aside-focused

# 9. The Prime Radiant, section 7 (ADR-0034 §4, SPEC-PLUGIN §6; WP-123): the
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
# (and its height unless the grid scrolls); no text leaves its slot, the
# desk or the window (the harness counts only what a clip shows, so rows
# scrolled off the grid do not count).
rfits() {
  local a='.view.sectionView as $v | [$v.slots[] | select(.w <= 0 or .h <= 0 or .x < $v.area.x or .x + .w > $v.area.x + $v.area.w + 1'
  expect "$1" "$2" "$a"' or (($v.scrolls | not) and (.y < $v.area.y or .y + .h > $v.area.y + $v.area.h + 1)))] | length' 0
  expect "$1" "$2" '.overflow | join(" | ")' ""
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
drift_s="13 opened · 8 resolved in 5 weeks · peak 2026-W40"
s30="78 events on 17 of 30 days · busiest 2026-10-01 (33) | explicit 324 → 327 · total 2005 → 2009 · 2 samples | $drift_s | $risk_s | 7 cases (6 open) · 2 releases · 6 snapshots · 2 crises | $plan_s"
s90="83 events on 18 of 90 days · busiest 2026-10-01 (33) | explicit 323 → 327 · total 2004 → 2009 · 3 samples | $drift_s | $risk_s | 8 cases (6 open) · 2 releases · 6 snapshots · 2 crises | $plan_s"
s365=${s90/of 90 days/of 365 days}
sall=${s90/of 90 days/of 366 days}
radiant='{"section":"radiant"}'

# 9a. Periods and IPC at 1920×1080 on the sample (overlay scenario 1). The
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
  "83 events on 18 of 90 days · busiest 2026-10-01 (33)" "$risk_s" "$plan_s" \
  "C-2026-003 · R3" "Omarchy auf 4.0.7 aktualisieren" "4/5 steps · agent: claude-code" "2/4 steps · agent: claude-code"; do
  shows radiant-ipc 1 "$text"
done
expect radiant-ipc 1 '[.texts[] | select(. == "Releases, snapshots, cases, crises")] | length' 0
# The period keys beside the chips (round 2, N4).
expect radiant-ipc 1 .view.sectionView.keyHint "←/→ period"
shows radiant-ipc 1 "←/→ period"
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

# 9a'. `setPeriod` with an unknown id changes nothing (SPEC-PLUGIN §8;
#     WP-123 round 2, N3): not the section, not the period. Before the
#     first visit of section 7 it answers ""; afterwards its period.
run radiant-setperiod "$sample" 1920x1080 \
  "summon;call:setPeriod:7;text:4;call:setPeriod:bogus;call:setPeriod:all;text:4;call:setPeriod:x;call:setPeriod:"
expect radiant-setperiod 2 '[.call, .view.section, (.view.visited | index("radiant") != null)] | map(tostring) | join(",")' ",today,false"
expect radiant-setperiod 4 '[.call, .view.section] | join(",")' ",decisions"
expect radiant-setperiod 5 '[.call, .view.section, .view.sectionView.period] | join(",")' "all,radiant,all"
expect radiant-setperiod 7 '[.call, .view.section] | join(",")' "all,decisions"
expect radiant-setperiod 8 '[.call, .view.section] | join(",")' "all,decisions"
clean_log radiant-setperiod

# 9b. Entering the section from closed, as the shell's loader does it (a
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
rhovered radiant-fresh 5 heatmap "Thu 2026-10-01 · 33 events · pacman 8 · seldon 8 · agent 6 · snapper 4 · config 2 · manual 2 · omarchy 1 · plugins 1 · theme 1"
shows radiant-fresh 5 "Thu 2026-10-01 · 33 events · pacman 8 · seldon 8 · agent 6 · snapper 4 · config 2 · manual 2 · omarchy 1 · plugins 1 · theme 1"
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

# 9c. Hover read-outs from real mouse moves onto each chart's items
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

# 9d. Layout (overlay scenarios 4 and 5): the desk at 100 % on 2560×1440
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

# 9e. Narrow desks reflow the grid (overlay scenario 6; WP-123 acceptance):
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
# The RiskDonut's "all time" under its count only where the hole holds it
# (round 2, N5): at 100 % yes, in the 50 % medium grid no (the caption
# still says "all time").
donut='[.view.sectionView.slots[] | select(.id == "riskDonut") | .chart.readout.centreLabel] | first | tostring'
expect radiant-half 1 "$donut" true
expect radiant-half 2 "$donut" false
expect radiant-half 2 '[.view.sectionView.slots[] | select(.id == "riskDonut") | .chart.summary] | first' "$risk_s"
clean_log radiant-half
run radiant-reflow "$sample" 1920x1080 "fresh:$radiant;view;width:50;view;resize:560x1080;view"
rpaints radiant-reflow 2 "1,1,1,1,1,1"
expect radiant-reflow 3 .view.sectionView.mode medium
rpaints radiant-reflow 4 "2,2,2,2,2,2"
expect radiant-reflow 5 .view.sectionView.mode narrow
rpaints radiant-reflow 6 "3,3,3,3,3,3"
expect radiant-reflow 6 .view.sectionView.aggregations.section 0
clean_log radiant-reflow

# 9f. A logbook that is not initialised (overlay scenario 7): the desk's
#     notice (7b); every chart in its empty state, nothing painted, no hover.
run radiant-uninit "$fx/index-variants/not-initialised.json" 1920x1080 "summon:$radiant;call:hover:heatmap 0.5,0.5;call:hover:timeline 0.5,0.5"
expect radiant-uninit 1 '.view.notices | join(",")' "Create your logbook"
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

# 9g. Every other index variant renders every chart (no empty state), one
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
# 10. Decisions, System, Memory (sections 4–6; ADR-0034 §2, WP-123): the 0.1
#     panel's scenarios for these tabs (panel-view.sh 5–7, 20–23; COVERAGE.md)
#     on the desk. `.view.sectionView`: rows (ids), cursor (the row the
#     detail shows), the enabled actions of the sticky bar and the section's
#     own keys.
# 10a. The sample, dev mode (read-only): `4` lists the four decisions newest
#      first, the proposed one striped and selected, its detail with Accept
#      and Open in editor and what Accept means; ↓/↑ move the selection and
#      the detail follows (an accepted one has no Accept); a click on a row
#      selects it; `e` is refused with dev mode's reason, Accept is shown
#      but disabled (dev mode writes nothing: a click neither arms nor
#      runs); `d`
#      opens no form without an engine to write; `/` narrows the list, Esc
#      clears it; `call select` picks a decision ("not found" for none).
#      The sample is contract 2: every decision has its CASES block
#      (ADR-0004 names none, ADR-0003 two).
run decisions "$sample" 1920x1080 \
  "summon;text:4;key:Down;key:Down*5;key:Up;click:Zed statt VS Code als Zweiteditor;text:e;key:Up;click:Accept;text:d;text:/;type:snap;key:Return;key:Escape;call:select:ADR-0001;call:select:ADR-0999"
expect decisions 2 .view.section decisions
expect decisions 2 '.view.sectionView.rows | join(",")' "ADR-0004,ADR-0003,ADR-0002,ADR-0001"
expect decisions 2 '[.view.sectionView.cursor, .view.selected] | join(",")' "ADR-0004,ADR-0004"
expect decisions 2 '.view.sectionView.actions | join(",")' "Open in editor"
expect decisions 2 '.view.sectionView.cases | length' 0
for text in "DECISIONS" "4 decisions · 1 proposed" "New decision" "Ollama nur als User-Service mit Case" "ADR-0004 · proposed" \
  "2026-10-01" "Logbuch-Sprache Deutsch, Struktur Englisch" "ADR-0001 · accepted" "2026-09-01" \
  "ADR-0004 · PROPOSED · 2026-10-01" "Accept" "Open in editor" "decisions/ADR-0004-ollama-user-service.md" \
  "Proposed: it waits for your decision. Accept marks it accepted with today's date and notes it in the ledger; Open in editor shows the whole text." \
  "Lokale Modelle nur über einen Case; ollama läuft, wenn überhaupt, als User-Service ohne Autostart." \
  "The whole text is in the file; Open in editor shows it."; do
  shows decisions 2 "$text"
done
shows decisions 2 "CASES · 0"
shows decisions 2 "This decision names no case."
expect decisions 3 '[.view.sectionView.cursor, .view.sectionView.actions[0]] | join(",")' "ADR-0003,Open in editor"
expect decisions 3 '[.texts[] | select(. == "Accept")] | length' 0
expect decisions 3 '.view.sectionView.cases | join(",")' "C-2026-004,C-2026-005"
for text in "CASES · 2" "Zed als zweiten Editor installieren" "C-2026-004 · active" "C-2026-005 · queued"; do
  shows decisions 3 "$text"
done
expect decisions 4 .view.sectionView.cursor ADR-0001
expect decisions 5 .view.sectionView.cursor ADR-0002
expect decisions 6 '[.view.sectionView.cursor, .view.keys] | map(tostring) | join(",")' "ADR-0003,true"
expect decisions 7 .view.sectionView.openResult "dev mode (SELDON_INDEX): engine calls are disabled"
expect decisions 9 .view.sectionView.cursor ADR-0004
expect decisions 9 .view.sectionView.openResult "dev mode (SELDON_INDEX): engine calls are disabled"
expect decisions 9 '[.view.sectionView.accept.armed, .view.sectionView.accept.result, .view.arm.armed] | map(tostring) | join(",")' "false,,"
expect decisions 10 '[.view.sectionView.form.open, .view.editing] | map(tostring) | join(",")' "false,false"
expect decisions 13 '[.view.search.text, (.view.sectionView.rows | join(",")), .view.sectionView.cursor, .view.sectionView.filtered] | map(tostring) | join("|")' "snap|ADR-0002|ADR-0002|true"
expect decisions 14 '[.view.search.text, (.view.sectionView.rows | length), .view.opened] | map(tostring) | join(",")' ",4,true"
expect decisions 15 '[.call, .view.selected] | join(",")' "ok,ADR-0001"
expect decisions 16 '[.call, .view.selected] | join(",")' "not found,ADR-0001"
for i in 2 3 9 13; do expect decisions $i '.overflow | join(" | ")' ""; done
clean_log decisions

# 10b. A decision that names cases (contract v2, `decisions[].cases`; the
#      plugin reads the field wherever it is): the CASES block lists them
#      with their titles from the index, a case the index no longer lists by
#      its id; a click goes to the case in Work, selected there. A decision
#      with an empty list says so; an index without the field hides the
#      block.
jq '.decisions[0].cases = ["C-2026-003", "C-2026-099"] | .decisions[1].cases = []' "$sample" >"$work/decision-cases.json"
run decision-cases "$work/decision-cases.json" 1920x1080 "summon;text:4;key:Down;key:Up;click:Omarchy auf 4.0.7 aktualisieren"
expect decision-cases 2 '.view.sectionView.cases | join(",")' "C-2026-003,C-2026-099"
for text in "CASES · 2" "Omarchy auf 4.0.7 aktualisieren" "C-2026-003 · active" "C-2026-099" "not in the index"; do
  shows decision-cases 2 "$text"
done
expect decision-cases 3 '.view.sectionView.cases | length' 0
shows decision-cases 3 "This decision names no case."
expect decision-cases 5 '[.view.section, .view.selected] | join(",")' "work,C-2026-003"
clean_log decision-cases
jq 'del(.decisions[].cases)' "$sample" >"$work/decision-nocases.json"
run decision-nocases "$work/decision-nocases.json" 1920x1080 "summon;text:4;key:Down"
expect decision-nocases 3 .view.sectionView.cases null
expect decision-nocases 3 '[.texts[] | select(startswith("CASES") or . == "This decision names no case.")] | length' 0
clean_log decision-nocases

# 10c. Live, against the fake engine, with real keys (panel scenario 21): `d`
#      opens the form in the detail pane and gives it the keys; the title
#      `--help "q"` is typed into it (no section switch, no digit); Enter
#      arms ("Press Enter again: …"), a change to the title disarms, Enter
#      twice creates it: `decide --no-edit --json -- <title>`, then `open
#      ADR-0005 --editor --json` from the answer; the form closes, the keys
#      come back and the selection sits on ADR-0005 once the index lists it.
#      Then Open in editor on the proposed ADR-0004, `e` and Open in editor on
#      ADR-0003; on Memory `e` and Open in editor open the logbook, on
#      System STATUS.md. The exact argv and editor paths.
mkdir -p "$work/home-decisions-live"
run decisions-live "" 1920x1080 \
  "summon;text:4;text:d;type:--help \"q\";key:Return;key:Backspace;type:\";key:Return;key:Return;settle;summon;wait:sectionView.cursor=ADR-0005;key:Down;click:Open in editor;settle;summon;key:Down;text:e;settle;summon;pause:2100;click:Open in editor;settle;summon;text:6;key:Down;text:e;settle;summon;pause:2100;click:Open in editor;settle;summon;text:5;text:e;settle" \
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
# Every open steps the desk aside (WP-156): the new decision's editor too;
# the case summons the desk again after each, and waits 2 s before the same
# target again (Model.OPEN_REPEAT_MS).
expect decisions-live 10 '[.view.opened, .service.stepAsides] | map(tostring) | join(",")' "false,1"
expect decisions-live 12 '[.view.sectionView.form.open, .view.sectionView.form.editing, .view.sectionView.form.title, .view.keys] | map(tostring) | join(",")' "false,false,,true"
expect decisions-live 12 .view.sectionView.result 'Created ADR-0005 · --help "q"'
expect decisions-live 12 '.view.sectionView.rows | join(",")' "ADR-0005,ADR-0004,ADR-0003,ADR-0002,ADR-0001"
shows decisions-live 12 'Created ADR-0005 · --help \"q\"'
shows decisions-live 12 "5 decisions · 2 proposed"
expect decisions-live 13 .view.sectionView.cursor ADR-0004
expect decisions-live 15 .service.open "Opened $work/home-decisions-live/Seldon/decisions/ADR-0004-ollama-user-service.md in omarchy-launch-editor"
expect decisions-live 17 .view.sectionView.cursor ADR-0003
expect decisions-live 19 .service.open "Opened $work/home-decisions-live/Seldon/decisions/ADR-0003-zed.md in omarchy-launch-editor"
expect decisions-live 21 .view.sectionView.openResult "Opened $work/home-decisions-live/Seldon/decisions/ADR-0003-zed.md in omarchy-launch-editor"
expect decisions-live 23 .service.open "Opened $work/home-decisions-live/Seldon/decisions/ADR-0003-zed.md in omarchy-launch-editor"
expect decisions-live 26 .view.sectionView.cursor "lesson:Theme-Overrides nie im Omarchy-Repo"
expect decisions-live 28 .service.open "Opened $work/home-decisions-live/Seldon in omarchy-launch-editor"
expect decisions-live 36 .service.open "Opened $work/home-decisions-live/Seldon/STATUS.md in omarchy-launch-editor"
expect decisions-live 36 '[.view.opened, .service.stepAsides] | map(tostring) | join(",")' "false,7"
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

# 10c'. Accept (WP-135, ADR-0040), live: Accept on the proposed ADR-0004
#      arms ("Confirm accept", the hint in the sticky bar) and runs nothing;
#      another key disarms, and so does a click on another decision (no
#      key); armed again, the
#      second click runs `decide accept ADR-0004 --json` — once, and no
#      `open` — and the decision arrives accepted with the index: no
#      Accept, the engine's answer in the detail (that decision's only),
#      nothing proposed. The
#      engine's refusal shows in place and the decision stays proposed; a
#      held lock too.
mkdir -p "$work/home-decisions-accept"
run decisions-accept "" 1920x1080 \
  "summon;text:4;click:Accept;text:z;click:Accept;click:Zed statt VS Code als Zweiteditor;key:Up;click:Accept;click:Confirm accept;settle;key:Down" \
  HOME="$work/home-decisions-accept" FAKE_SELDON_FIXTURE="$sample"
ta="$tv.accept"
expect decisions-accept 2 '[.view.sectionView.cursor, (.view.sectionView.actions | join(","))] | join("|")' "ADR-0004|Accept,Open in editor"
expect decisions-accept 3 "[$ta.armed, $ta.hint, .view.arm.armed] | map(tostring) | join(\"|\")" \
  "true|Accept ADR-0004? Click Confirm: it becomes accepted with today's date.|decision:ADR-0004:accept"
expect decisions-accept 3 '.view.sectionView.actions | join(",")' "Confirm accept,Open in editor"
shows decisions-accept 3 "Confirm accept"
shows decisions-accept 3 "Accept ADR-0004? Click Confirm: it becomes accepted with today's date."
expect decisions-accept 4 "[$ta.armed, $ta.hint, .view.arm.armed] | map(tostring) | join(\"|\")" "false||"
expect decisions-accept 4 '.view.sectionView.actions | join(",")' "Accept,Open in editor"
expect decisions-accept 5 "$ta.armed" true
expect decisions-accept 6 "[.view.sectionView.cursor, $ta.armed, .view.arm.armed] | map(tostring) | join(\"|\")" "ADR-0003|false|"
expect decisions-accept 7 "[.view.sectionView.cursor, $ta.armed] | map(tostring) | join(\"|\")" "ADR-0004|false"
expect decisions-accept 8 "$ta.armed" true
expect decisions-accept 9 "$ta.armed" false
expect decisions-accept 10 "[.view.sectionView.cursor, $ta.result, $ta.pending] | map(tostring) | join(\"|\")" \
  "ADR-0004|Accepted ADR-0004 · Ollama nur als User-Service mit Case|false"
expect decisions-accept 10 '.view.sectionView.actions | join(",")' "Open in editor"
expect decisions-accept 10 '[.texts[] | select(. == "Accept" or . == "Confirm accept")] | length' 0
for text in "Accepted ADR-0004 · Ollama nur als User-Service mit Case" "ADR-0004 · ACCEPTED · 2026-10-07" "4 decisions"; do
  shows decisions-accept 10 "$text"
done
expect decisions-accept 10 '.view.lastError' ""
expect decisions-accept 11 "[.view.sectionView.cursor, $ta.result] | join(\"|\")" "ADR-0003|"
for i in 3 10; do expect decisions-accept $i '.overflow | join(" | ")' ""; done
argv_check decisions-accept "$work/home-decisions-accept" "$(printf '%s\n' "$startup" "$(q decide accept ADR-0004 --json)")"
clean_log decisions-accept

# the engine's own text for an agent session (decide.rs user_actor)
refusal='ADR-0004 is not accepted: agent:claude-code may propose a decision (`seldon decide`), only the user accepts one (ADR-0040); ask them to accept it in the desk or in their own terminal'
mkdir -p "$work/home-decisions-accept-refused"
run decisions-accept-refused "" 1920x1080 "summon;text:4;click:Accept;click:Confirm accept;settle" \
  HOME="$work/home-decisions-accept-refused" FAKE_SELDON_FIXTURE="$sample" FAKE_SELDON_ACCEPT_REFUSE="$refusal"
expect decisions-accept-refused 5 "[.view.sectionView.cursor, $ta.result, $ta.armed] | map(tostring) | join(\"|\")" "ADR-0004|$refusal|false"
expect decisions-accept-refused 5 '.view.sectionView.actions | join(",")' "Accept,Open in editor"
shows decisions-accept-refused 5 "$refusal"
shows decisions-accept-refused 5 "4 decisions · 1 proposed"
expect decisions-accept-refused 5 '.view.lastError' ""
argv_check decisions-accept-refused "$work/home-decisions-accept-refused" "$(printf '%s\n' "$startup" "$(q decide accept ADR-0004 --json)")"
clean_log decisions-accept-refused 'jax\.seldon: seldon decide exit 1: ADR-0004 is not accepted: '

# A new index disarms Accept (Decisions.qml onAllRowsChanged; round 2,
# N4): armed, then a capture from the pill's right click — no key, no click
# in the desk — makes the fake engine write an index with one more
# decision (ADR-0005, proposed); Accept is no longer armed, nothing ran.
jq '.decisions = [{id: "ADR-0005", title: "Neuer Vorschlag", status: "proposed", date: "2026-10-07", cases: [],
    path: "decisions/ADR-0005-neuer-vorschlag.md"}] + .decisions' "$sample" >"$work/decisions-after.json"
mkdir -p "$work/home-decisions-accept-index"
run decisions-accept-index "" 1920x1080 \
  "summon;text:4;click:Accept;pill:right;wait:sectionView.summary=5 decisions · 2 proposed" \
  HOME="$work/home-decisions-accept-index" FAKE_SELDON_FIXTURE="$sample" FAKE_SELDON_FIXTURE_AFTER="$work/decisions-after.json"
expect decisions-accept-index 3 "[.view.sectionView.cursor, $ta.armed] | map(tostring) | join(\"|\")" "ADR-0004|true"
expect decisions-accept-index 5 "[.view.section, .view.sectionView.cursor, $ta.armed, .view.arm.armed] | map(tostring) | join(\"|\")" \
  "decisions|ADR-0004|false|"
expect decisions-accept-index 5 '.view.sectionView.actions | join(",")' "Accept,Open in editor"
argv_check decisions-accept-index "$work/home-decisions-accept-index" \
  "$(printf '%s\n' "$startup" "$(q capture --all --json --quiet)" "$(q status --json)")"
clean_log decisions-accept-index

mkdir -p "$work/home-decisions-accept-locked"
run decisions-accept-locked "" 1920x1080 "summon;text:4;click:Accept;click:Confirm accept;settle" \
  HOME="$work/home-decisions-accept-locked" FAKE_SELDON_FIXTURE="$sample" FAKE_SELDON_LOCKED=1
expect decisions-accept-locked 5 "[$ta.result, (.view.sectionView.actions | join(\",\"))] | join(\"|\")" \
  "the logbook is locked by another seldon (pid 4242)|Accept,Open in editor"
clean_log decisions-accept-locked

# 10d. Refusals keep the title (panel scenario 22): Enter on a blank title is
#      refused in the plugin; the engine refuses the decision (lock held, exit
#      4): the form shows its message and keeps the title, nothing is opened;
#      Esc gives the keys back and the list head shows the refusal; `d`
#      brings the form back with the title; a click on a decision shows it
#      instead and gives the keys back, so the next Esc closes the desk.
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

# 10e. System on the sample (panel scenarios 1 and 6): `5` lists the six
#      tiles with their big values; the detail shows the big value, the lead,
#      the rows and where they come from; ↓ walks the tiles; `e` and Open in
#      editor ask for STATUS.md (refused in dev mode). Every system field is
#      optional: an empty `system` and a sparse one give "—" tiles that say
#      so, and the Collectors tile keeps machine, engine and index time; a
#      failing collector stripes its tile. The sixth, Recently edited
#      (WP-139), lists the files with their age, "not watched" and Watch;
#      Watch is refused in dev mode.
run system "$sample" 1920x1080 "summon;text:5;key:Down;key:Down;key:Down;key:Down;text:e;click:Open in editor;key:Down;click:Watch"
expect system 2 .view.section system
expect system 2 '.view.sectionView.tiles | join(",")' "omarchy 4.0.7-1,packages 2009 installed,snapshots 115 newest,deviations 5 files,collectors 6/6 ok,recent 4 files"
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
expect system 9 '[.view.sectionView.cursor, .view.sectionView.big, (.view.sectionView.detailRows | join(","))] | join("|")' "recent|4|Scanned"
expect system 9 '.view.sectionView.files | join(",")' \
  "~/.config/zed/settings.json 6 h ago,~/.config/git/config 19 h ago,~/.config/starship.toml 2 days ago,~/.config/alacritty/alacritty.toml 3 days ago"
for text in "Recently edited" "4 files" "~/.config/zed/settings.json" "6 h ago · not watched" "~/.config/alacritty/alacritty.toml" \
  "Under ~/.config in the last 7 days, outside the watched paths: no record of what changed" "Watch" \
  "From the last capture's scan of ~/.config: paths and times only, never content. Seldon keeps no record of these edits until a path is watched."; do
  shows system 9 "$text"
done
expect system 9 '[.texts[] | select(. == "Watch")] | length' 4
expect system 10 .view.sectionView.watchResult ""
for i in 2 3 4 5 6 9; do expect system $i '.overflow | join(" | ")' ""; done
clean_log system

# 10e'. Watch, live (WP-139, ADR-0045): each click runs `config watch --json
#       -- <path>` with the row's path as one argument; the row goes with the
#       index the engine rebuilds and the answer shows above the list. A held
#       lock is the engine's message in place, no retry, no lastError.
mkdir -p "$work/home-system-watch"
run system-watch "" 1920x1080 "summon;text:5;key:Down*5;click:Watch;settle;click:Watch;settle" \
  HOME="$work/home-system-watch" FAKE_SELDON_FIXTURE="$sample"
expect system-watch 3 '.view.sectionView.files | length' 4
expect system-watch 5 '.view.sectionView.files | map(split(" ")[0]) | join(",")' \
  "~/.config/git/config,~/.config/starship.toml,~/.config/alacritty/alacritty.toml"
expect system-watch 5 .view.sectionView.watchResult \
  "Watching ~/.config/zed/settings.json from the next capture on; it is taken as it is, without an event"
shows system-watch 5 "Watching ~/.config/zed/settings.json from the next capture on; it is taken as it is, without an event"
expect system-watch 7 '[.view.sectionView.big, (.view.sectionView.files | map(split(" ")[0]) | join(","))] | join("|")' \
  "2|~/.config/starship.toml,~/.config/alacritty/alacritty.toml"
expect system-watch 7 .view.lastError ""
argv_check system-watch "$work/home-system-watch" "$(printf '%s\n' "$startup" \
  "$(q config watch --json -- "~/.config/zed/settings.json")" "$(q config watch --json -- "~/.config/git/config")")"
clean_log system-watch
mkdir -p "$work/home-system-watch-locked"
run system-watch-locked "" 1920x1080 "summon;text:5;key:Down*5;click:Watch;settle" \
  HOME="$work/home-system-watch-locked" FAKE_SELDON_FIXTURE="$sample" FAKE_SELDON_LOCKED=1
expect system-watch-locked 5 '[.view.sectionView.watchResult, (.view.sectionView.files | length), .view.lastError] | map(tostring) | join("|")' \
  "the logbook is locked by another seldon (pid 4242)|4|"
argv_check system-watch-locked "$work/home-system-watch-locked" "$(printf '%s\n' "$startup" \
  "$(q config watch --json -- "~/.config/zed/settings.json")")"
clean_log system-watch-locked "seldon config exit 4: the logbook is locked by another seldon"
jq '.system = {} | del(.state.collectors)' "$sample" >"$work/system-empty.json"
run system-empty "$work/system-empty.json" 1920x1080 "summon;text:5;key:Down*4"
expect system-empty 2 '.view.sectionView.tiles | join(",")' "omarchy —,packages —,snapshots —,deviations —,collectors —,recent —"
shows system-empty 2 "Not in the index"
expect system-empty 3 '.view.sectionView.detailRows | join(",")' "Machine,Engine,Index written"
clean_log system-empty
jq '.system = {packages: {aur: 3}, deviations: 1} | del(.state.collectors)' "$sample" >"$work/system-sparse.json"
run system-sparse "$work/system-sparse.json" 1920x1080 "summon;text:5;key:Down"
expect system-sparse 2 '.view.sectionView.tiles | join(",")' "omarchy —,packages —,snapshots —,deviations 1 file,collectors —,recent —"
expect system-sparse 3 '.view.sectionView.detailRows | join(",")' "AUR"
shows system-sparse 3 "3 from the AUR"
clean_log system-sparse
run system-degraded "$fx/index-variants/snapper-degraded.json" 1920x1080 "summon;text:5;key:Down*4"
expect system-degraded 3 .view.sectionView.big "5/6"
shows system-degraded 3 "5/6 ok"
expect system-degraded 3 '[.texts[] | select(startswith("1 collector failing"))] | length' 1
clean_log system-degraded

# 10f. Memory on the sample (panel scenario 20): `6` lists three lessons and
#      two topics; the detail names the file and, for a topic, its date; `e`
#      and Open in editor open the logbook (refused in dev mode).
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

# 10f'. The search in System and Memory (WP-123 round 2, N1, N2): `/aur`
#       finds Packages by its lead, `/installed` by its value, `/memory/hyp`
#       the hyprland topic by its path; Esc clears. With the selection
#       filtered out the detail shows the first row, and the first `j` moves
#       on from it (it used to select that row again); a search that only
#       hid the selection gives it back when it is cleared.
run search-sections "$sample" 1920x1080 \
  "summon;text:5;text:/;type:aur;key:Return;key:Escape;text:/;type:installed;key:Return;key:Escape;text:6;text:/;type:hyprland;key:Return;text:j;text:k;key:Escape;text:/;type:memory/hyp;key:Return"
expect search-sections 5 '[(.view.sectionView.rows | join(",")), .view.sectionView.cursor, .view.sectionView.filtered] | map(tostring) | join("|")' "packages|packages|true"
expect search-sections 6 '[(.view.sectionView.rows | length), .view.sectionView.cursor, .view.sectionView.filtered] | map(tostring) | join("|")' "5|omarchy|false"
expect search-sections 9 '.view.sectionView.rows | join(",")' "packages"
expect search-sections 14 '[(.view.sectionView.rows | join(",")), .view.sectionView.cursor] | join("|")' "lesson:Hyprland reload nach bindings.conf,topic:hyprland|lesson:Hyprland reload nach bindings.conf"
expect search-sections 14 .view.selected 'lesson:`omarchy pkg add` statt yay direkt'
expect search-sections 15 .view.sectionView.cursor "topic:hyprland"
expect search-sections 16 .view.sectionView.cursor "lesson:Hyprland reload nach bindings.conf"
expect search-sections 17 '[(.view.sectionView.rows | length), .view.sectionView.cursor] | map(tostring) | join("|")' "5|lesson:Hyprland reload nach bindings.conf"
expect search-sections 20 '.view.sectionView.rows | join(",")' "topic:hyprland"
for i in 5 14 20; do expect search-sections $i '.overflow | join(" | ")' ""; done
clean_log search-sections

# 10g. Not initialised (panel scenario 5): sections 4–6 say there is no
#      index, `d` opens no form, `e` runs nothing; the Prime Radiant is 9f.
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

# 10h. The stacked layout (a window under 770 px; WP-121's case 6 for these
#      sections): the list; Enter shows the detail with its back row and the
#      sticky bar; Esc goes back to the list; `d` shows the form as the
#      detail (dev mode: no form); the back row returns; a row click shows
#      its detail. Nothing leaves the window or the desk.
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

# 10i. Label fit (panel scenario 23, the desk's part): sections 4–7 on a
#      1366 px screen at 50 % (the 960 px floor) and at 100 %, and on 3840 px:
#      no text leaves its box, slot, the desk or the window.
for W in 1366 3840; do
  run "fit-$W" "$sample" "${W}x1080" "summon;width:50;text:4;text:5;key:Down*4;text:6;text:7;width:100;text:4;text:5;text:6;text:7"
  for i in 3 4 5 6 7 9 10 11 12; do expect "fit-$W" $i '.overflow | join(" | ")' ""; done
  clean_log "fit-$W"
done

# ---------------------------------------------------------------------------
# 11. The graph, section 8 (ADR-0034 §5, SPEC-PLUGIN §5.4; WP-125): the
#     machine's memory as a network from the index alone, laid out by
#     Model.graphStep on a Timer. On the sample: it settles and sleeps
#     within the budget (each reported tickMs ≤ 8, step plus drawing calls
#     on the shell thread; graph_tick_ok below), nothing ticks while another
#     section is shown or the desk is closed, a reopened desk keeps the
#     settled layout, the service builds the graph only for a shown section
#     8 and again only after the index changed; the
#     replay adds nodes monotonically; hover, the card and Open case; drag
#     wakes the layout, pan and zoom only repaint; `select` keeps a card.
#     A busy index (tests/plugin/graph-index.js, 400 nodes after folding)
#     draws within the budget too, but on a shared build host its ticks
#     are reported, not gated one by one (see 11f). No index, an empty
#     index; a narrow desk (labels flip at the edge); a still picture above
#     400 fixed nodes.
# graph_tick_ok <case> <step>: the tick the view reports is within the
# budget (tickMs ≤ 8: Model.graphStep plus the drawing calls, both on the
# shell thread), and so are all ticks so far but at most two of them. The
# dev host builds other work packages at the same time: a compile that
# takes the core preempts a tick now and then (seen: 9–25 ms on the
# 67-node sample, whose ticks take 1–3 ms, in bursts under a load of 7).
# So a case that misses runs once more (graph_run keeps its arguments)
# and must pass then; a slower graph misses twice. slowTicks names the
# ticks in the log either way.
declare -A graph_args=() graph_retried=()
graph_run() {
  graph_args[$1]=$(printf '%q ' "$@")
  run "$@"
}
# graph_time_ok <case> <jq condition> <step>… — a timing gate: the
# condition (true/false) holds at each step. If it misses at one, the case
# runs once more (graph_run kept its arguments) and must hold then.
graph_time_ok() {
  local name=$1 cond=$2 i miss=""
  shift 2
  for i in "$@"; do
    [[ $(sed -n "${i}p" "$work/$name.steps" | jq -r "$cond" 2>/dev/null) == true ]] || miss="$miss #$i"
  done
  if [[ -n $miss && -n ${graph_args[$name]:-} && -z ${graph_retried[$name]:-} ]]; then
    graph_retried[$name]=1
    echo "     $name:$miss over the time budget, runs once more (a loaded host?)"
    eval "run ${graph_args[$name]}"
  fi
  if [[ -n $miss ]] && graph_timing_soft; then
    miss=""
    for i in "$@"; do
      [[ $(sed -n "${i}p" "$work/$name.steps" | jq -r "$cond" 2>/dev/null) == true ]] || miss="$miss #$i"
    done
    if [[ -n $miss ]]; then
      echo "     WARN $name:$miss over the time budget twice on a loaded host ($(graph_load)); counted only with SELDON_PERF_STRICT=1"
      return 0
    fi
  fi
  for i in "$@"; do expect "$name" "$i" "$cond" true; done
}
# graph_timing_soft — true when a timing miss may be a warning: not under
# SELDON_PERF_STRICT and the host's 1-minute load is at least half its
# cores (other checks and builds running). The test host's live run
# (graph-live.sh) and SELDON_PERF_STRICT=1 keep every budget strict.
graph_load() { cut -d' ' -f1 /proc/loadavg; }
graph_timing_soft() {
  [[ -z ${SELDON_PERF_STRICT:-} ]] || return 1
  awk -v l="$(graph_load)" -v n="$(nproc)" 'BEGIN { exit !(l >= n / 2) }'
}
# graph_time_min <case> <path> <max> <step>… — a timing gate on the fastest
# of several reports of the same work (a still picture's paints): load
# makes some of them slower, never the fastest one faster than the work.
# Once more on a miss, as graph_time_ok.
graph_time_min() {
  local name=$1 path=$2 max=$3 lines
  shift 3
  lines=$(printf '%sp;' "$@")
  min_of() { sed -n "$lines" "$work/$name.steps" | jq -s "[.[] | $path] | min"; }
  if ! jq -en "$(min_of) <= $max" >/dev/null && [[ -n ${graph_args[$name]:-} && -z ${graph_retried[$name]:-} ]]; then
    graph_retried[$name]=1
    echo "     $name: the fastest $path $(min_of) over $max, runs once more (a loaded host?)"
    eval "run ${graph_args[$name]}"
  fi
  if ! jq -en "$(min_of) <= $max" >/dev/null && graph_timing_soft; then
    echo "     WARN $name: the fastest $path $(min_of) over $max twice on a loaded host ($(graph_load)); counted only with SELDON_PERF_STRICT=1"
    return 0
  fi
  check "$name: the fastest $path of steps $* ≤ $max ($(min_of))" "$(jq -n "$(min_of) <= $max")" true
}
graph_tick_ok() {
  local slow
  slow=$(sed -n "${2}p" "$work/$1.steps" | jq -c '.view.graph.slowTicks // []')
  [[ $slow == "[]" ]] || echo "     $1 #$2: ticks over the budget: $slow"
  graph_time_ok "$1" '.view.graph.tickMs <= 8 and .view.graph.ticksOver <= 2' "$2"
  slow=$(sed -n "${2}p" "$work/$1.steps" | jq -c '.view.graph.slowTicks // []')
  [[ -z ${graph_retried[$1]:-} || $slow == "[]" ]] || echo "     $1 #$2 (again): ticks over the budget: $slow"
}

# 11a. Settle and sleep: 200 ticks at most, then the Timer stops; no tick
#      and no paint after that; the legend, the date, the footer.
graph_run graph-settle "$sample" 1920x1080 "summon;text:8;wait:graph.sleeping=true;pause:300;pause:1000"
expect graph-settle 2 .view.section graph
expect graph-settle 2 '[.view.graph.nodes, .view.graph.edges, .view.graph.visible, .view.graph.folded] | map(tostring) | join(",")' "75,27,75,0"
expect graph-settle 2 '[.view.graph.sleeping, .view.graph.timer] | map(tostring) | join(",")' "false,true"
expect graph-settle 3 '[.view.graph.sleeping, .view.graph.timer, .view.graph.ticks, .view.graph.run] | map(tostring) | join(",")' "true,false,200,200"
expect graph-settle 3 '.view.graph.tickSamples > 150' true
graph_tick_ok graph-settle 3
expect graph-settle 5 '[.view.graph.ticks, .view.graph.timer] | map(tostring) | join(",")' "200,false"
# (the last tick's paint may still be pending at step 3: compare 4 and 5)
check "graph-settle: no paint while asleep" "$(sed -n 5p "$work/graph-settle.steps" | jq .view.graph.paints)" "$(sed -n 4p "$work/graph-settle.steps" | jq .view.graph.paints)"
for t in "Graph" "Play growth" "2026-10-01 · 75 nodes" "Case" "Area" "Decision" "Change" "Crisis" \
  "Newest 83 events · 2 completed cases in the index" "←/→ day · Space play · drag, scroll · 0 fit"; do
  shows graph-settle 3 "$t"
done
expect graph-settle 3 '.view.sectionView.legend | join(",")' "Case,Area,Decision,Change,Crisis"
expect graph-settle 3 '.overflow | join(" | ")' ""
clean_log graph-settle

# 11b. Nothing while hidden: another section stops the Timer at once (the
#      tick count stands still), coming back resumes to sleep; a closed and
#      reopened desk (the loader makes a new one) shows the settled layout
#      from the service without a tick.
graph_run graph-hidden "$sample" 1920x1080 "summon;text:8;pause:300;text:1;pause:1500;text:8;wait:graph.sleeping=true;hide;summon;pause:800"
# The service builds the graph only for a shown section 8.
expect graph-hidden 1 '[.graphBuilds, .graphNodes, .view.graph] | map(tostring) | join(",")' "0,0,null"
expect graph-hidden 2 '[.graphBuilds, .graphNodes] | map(tostring) | join(",")' "1,75"
t4=$(sed -n 4p "$work/graph-hidden.steps" | jq .view.graph.ticks)
expect graph-hidden 3 '[.view.section, .view.graph.timer] | map(tostring) | join(",")' "graph,true"
expect graph-hidden 4 '[.view.section, .view.graph.timer] | map(tostring) | join(",")' "today,false"
expect graph-hidden 5 .view.graph.ticks "$t4"
check "graph-hidden: ticks before the switch" "$( ((t4 > 0 && t4 < 200)) && echo yes)" yes
expect graph-hidden 7 '[.view.graph.sleeping, .view.graph.ticks] | map(tostring) | join(",")' "true,200"
graph_tick_ok graph-hidden 7
expect graph-hidden 8 .view.opened false
expect graph-hidden 8 .view.graph null
for i in 9 10; do
  expect graph-hidden $i '[.view.section, .view.graph.sleeping, .view.graph.timer, .view.graph.ticks] | map(tostring) | join(",")' "graph,true,false,200"
done
expect graph-hidden 10 '.view.graph.paints <= 2' true
clean_log graph-hidden

# 11b'. The build waits for the section (live, the fake engine rewrites
#      the index on each capture): two captures while the Prime Radiant is
#      shown leave the graph dirty and unbuilt; showing section 8 builds
#      once, and the same nodes keep their settled layout (no tick).
run graph-dirty "" 1920x1080 "summon;settle;text:8;wait:graph.sleeping=true;text:7;text:c;settle;pause:500;text:c;settle;pause:500;text:8;pause:300" \
  HOME="$work/home-graph-dirty" FAKE_SELDON_FIXTURE="$sample"
expect graph-dirty 2 '[.graphBuilds, .graphNodes] | map(tostring) | join(",")' "0,0"
expect graph-dirty 4 '[.graphBuilds, .graphDirty, .view.graph.sleeping, .view.graph.ticks] | map(tostring) | join(",")' "1,false,true,200"
expect graph-dirty 11 '[.view.section, .graphBuilds, .graphDirty] | map(tostring) | join(",")' "radiant,1,true"
expect graph-dirty 12 '[.view.section, .graphBuilds, .graphDirty, .graphNodes] | map(tostring) | join(",")' "graph,2,false,75"
expect graph-dirty 12 '[.view.graph.sleeping, .view.graph.ticks, .view.graph.timer] | map(tostring) | join(",")' "true,200,false"
clean_log graph-dirty

# 11c. Replay: Play from day 0 to the last day adds nodes monotonically and
#      ends with all of them; the slider's day (graphCut) and ←/→; Space.
graph_run graph-replay "$sample" 1920x1080 "summon;text:8;wait:graph.sleeping=true;graphPlay;wait:graph.playing=false;graphCut:0;key:Right;key:Space;pause:300;key:Escape;wait:graph.sleeping=true"
expect graph-replay 4 '[.view.graph.playing, .view.graph.cut] | map(tostring) | join(",")' "true,0"
expect graph-replay 5 '.view.graph.replay | (. == sort) and (length > 10) and (.[0] < .[-1]) and (.[-1] == 75)' true
expect graph-replay 5 '[.view.graph.playing, .view.graph.cut, .view.graph.visible] | map(tostring) | join(",")' "false,30,75"
expect graph-replay 6 '[.view.graph.cut, .view.graph.date, .view.sectionView.date] | map(tostring) | join(",")' "0,2026-09-01,2026-09-01 · 4 nodes of 75"
expect graph-replay 7 '[.view.graph.cut, .view.graph.sleeping] | map(tostring) | join(",")' "1,false"
expect graph-replay 8 .view.graph.playing true
expect graph-replay 9 '.view.graph.cut > 1' true
expect graph-replay 10 '[.view.graph.playing, .view.opened] | map(tostring) | join(",")' "false,true"
expect graph-replay 11 .view.graph.sleeping true
graph_tick_ok graph-replay 11
clean_log graph-replay

# 11d. Hover: the pointer on a case lights it and shows its card; the card
#      stays while the pointer travels to Open case, which shows the case
#      in Work. `select` keeps a card (IPC); Esc lets it go; unknown ids.
run graph-hover "$sample" 1920x1080 "summon;text:8;wait:graph.sleeping=true;graphHover:C-2026-003;leave;click:Open case;text:8;select:ADR-0003;select:C-2026-999;key:Escape;key:Escape"
expect graph-hover 4 '[.view.graph.hovered, .view.graph.card.title, .view.graph.card.caseId] | join(",")' "C-2026-003,C-2026-003 Omarchy auf 4.0.7 aktualisieren,C-2026-003"
expect graph-hover 4 '.view.graph.card.line | test("^Case · since 2026-09-26 · day 25 · [0-9]+ links$")' true
shows graph-hover 4 "Open case"
shows graph-hover 4 "active · R3 · shell"
expect graph-hover 4 '[.view.graph.ticks, .view.graph.timer] | map(tostring) | join(",")' "200,false"
expect graph-hover 5 '[.view.graph.hovered, .view.graph.cardNode] | join(",")' ",C-2026-003"
expect graph-hover 6 '[.view.section, .view.selected] | join(",")' "work,C-2026-003"
expect graph-hover 8 '[.call, .view.selected, .view.graph.pinned, .view.graph.card.title] | join(",")' "ok,ADR-0003,ADR-0003,ADR-0003 Zed statt VS Code als Zweiteditor"
expect graph-hover 8 '[.texts[] | select(. == "Open case")] | length' 0
expect graph-hover 9 '[.call, .view.graph.pinned] | join(",")' "not found,ADR-0003"
expect graph-hover 10 '[.view.opened, .view.graph.pinned, .view.graph.card] | map(tostring) | join(",")' "true,,null"
expect graph-hover 11 .view.opened false
clean_log graph-hover

# 11e. Drag a node: it follows the pointer, the layout wakes (and sleeps
#      again), the view stops fitting; a drag beside the nodes pans, the
#      wheel zooms, neither ticks; 0 fits again.
graph_run graph-drag "$sample" 1920x1080 "summon;text:8;wait:graph.sleeping=true;graphDrag:C-2026-004:160,90;wait:graph.sleeping=true;graphDrag:empty:-100,40;wheel:graphCanvas:120;text:0"
expect graph-drag 4 '(.call | fromjson | (.to.x - .from.x - 160 | fabs) <= 8 and (.to.y - .from.y - 90 | fabs) <= 8)' true
expect graph-drag 4 '[.view.graph.sleeping, .view.graph.wakes > 0, .view.graph.view.fit] | map(tostring) | join(",")' "false,true,false"
expect graph-drag 5 '[.view.graph.sleeping, .view.graph.run <= 200] | map(tostring) | join(",")' "true,true"
graph_tick_ok graph-drag 5
t5=$(sed -n 5p "$work/graph-drag.steps" | jq .view.graph.ticks)
v5=$(sed -n 5p "$work/graph-drag.steps" | jq -c '[.view.graph.view.x, .view.graph.view.y]')
expect graph-drag 6 '[.view.graph.ticks, .view.graph.sleeping] | map(tostring) | join(",")' "$t5,true"
expect graph-drag 6 "[.view.graph.view.x, .view.graph.view.y] == ($v5 | .[0] -= 100 | .[1] += 40)" true
expect graph-drag 7 '[.view.graph.ticks, .view.graph.view.k > 0] | map(tostring) | join(",")' "$t5,true"
k6=$(sed -n 6p "$work/graph-drag.steps" | jq .view.graph.view.k)
expect graph-drag 7 "(.view.graph.view.k / $k6 * 100 | round)" 115
expect graph-drag 8 .view.graph.view.fit true
clean_log graph-drag

# 11f. A busy index: 500 events, 50 completed cases, 66 cases, 20
#      decisions → 400 nodes, 295 changes folded into 99 groups; the legend
#      gains "Folded"; a folded group's card lists its changes. Its ticks
#      run Barnes–Hut at 400 nodes (about 2–4 ms of step in QV4 on the dev
#      host); a build host shared with compiles preempts single ticks, so
#      the gate here is "at most 5 of 200 ticks over 8 ms" (once more on a
#      miss, as graph_tick_ok) — the strict
#      every-tick gate is the sample (11a) and the test host's measurement.
node "$root/tests/plugin/graph-index.js" >"$work/graph-big.json"
cluster=$(node -e '
  const fs = require("fs"), vm = require("vm"), M = {}; vm.createContext(M)
  vm.runInContext(fs.readFileSync(process.argv[1] + "/plugin/Model.js", "utf8"), M)
  const b = M.graphBuild(M.parseIndex(fs.readFileSync(process.argv[2], "utf8")).index, 400)
  process.stdout.write(b.nodes.filter((n) => n.kind === "cluster").sort((x, y) => y.count - x.count)[0].id)' "$root" "$work/graph-big.json")
graph_run graph-big "$work/graph-big.json" 1920x1080 "summon;text:8;wait:graph.sleeping=true;graphHover:$cluster"
expect graph-big 3 '[.view.graph.nodes, .view.graph.folded, .view.graph.clusters, .view.graph.ticks] | map(tostring) | join(",")' "400,295,99,200"
graph_time_ok graph-big '.view.graph.ticksOver <= 5' 3
expect graph-big 3 '.view.sectionView.legend | join(",")' "Case,Area,Decision,Change,Crisis,Folded"
shows graph-big 3 "Newest 500 events · 50 completed cases in the index · older ones are only in the logbook · 295 changes folded into 99"
expect graph-big 4 '[.view.graph.card.members > 0, (.view.graph.card.title | test("^[0-9]+ changes · 20[0-9-]+ · [a-z]+$"))] | map(tostring) | join(",")' "true,true"
expect graph-big 4 '.view.graph.card.line | startswith("Folded changes · since ")' true
expect graph-big 4 '.overflow | join(" | ")' ""
echo "     graph-big: tickMsMax $(sed -n 3p "$work/graph-big.steps" | jq -c '[.view.graph.tickMsMax, .view.graph.stepMsMax, .view.graph.ticksOver, .view.graph.slowTicks]')"
clean_log graph-big

# 11g. No index (not initialised) and an index with nothing to draw.
run graph-uninit "$fx/index-variants/not-initialised.json" 1920x1080 "summon;text:8"
expect graph-uninit 2 '[.view.graph.nodes, .view.graph.timer, .view.sectionView.empty] | map(tostring) | join(",")' "0,false,No index to show"
clean_log graph-uninit
jq '.events = [] | .drift = [] | .decisions = [] | .system.areas = [] | .cases = {queued: [], active: [], verification: [], completed: []}' "$sample" >"$work/graph-nothing.json"
run graph-nothing "$work/graph-nothing.json" 1920x1080 "summon;text:8;pause:500"
expect graph-nothing 3 '[.view.graph.nodes, .view.graph.ticks, .view.sectionView.empty] | map(tostring) | join(",")' "0,0,Nothing to draw yet: no areas, cases, decisions or changes in the index"
clean_log graph-nothing

# 11h. Narrow desks: 50 % on 1366 (the 960 px floor) and the stacked
#      window; nothing leaves its box, the desk or the window.
graph_run graph-narrow "$sample" 1366x900 "summon;width:50;text:8;wait:graph.sleeping=true;resize:700x900;pause:300"
expect graph-narrow 4 '.overflow | join(" | ")' ""
expect graph-narrow 6 '.overflow | join(" | ")' ""
graph_tick_ok graph-narrow 4
clean_log graph-narrow
# A label at the right edge goes to the left of its node, not past the
# canvas. The settled layout of the sample decides which node lies there
# (before WP-113's fixture event the backup-dotfiles.sh crisis did at
# 50 %), so the case drags a case node past the right edge itself.
graph_run graph-flip "$sample" 1366x900 "summon;width:50;text:8;wait:graph.sleeping=true;graphDrag:C-2026-004:600,0;wait:graph.sleeping=true"
expect graph-flip 4 '.view.graph.flipped' 0
expect graph-flip 6 '.view.graph.flipped >= 1' true
expect graph-flip 6 '.overflow | join(" | ")' ""
clean_log graph-flip

# 11i. More fixed nodes than the cap (2000 more areas: 2022 nodes): a still
#      picture in node order — no tick, ever (no Timer; a cut and a drag do
#      not wake it), the caption says why; hover and drag still work (the
#      dragged node moves at once), drawing stays in the budget.
jq '.system.areas += [range(2000) | {name: ("area-" + tostring), hasAgentsMd: false, cases: 0}]' "$sample" >"$work/graph-many.json"
graph_run graph-many "$work/graph-many.json" 1920x1080 "summon;text:8;pause:1500;graphHover:area:area-5;graphDrag:area:area-7:80,40;graphCut:3;pause:500"
expect graph-many 3 '[.view.sectionView.still, .view.graph.nodes, .view.graph.ticks, .view.graph.timer, .view.graph.sleeping] | map(tostring) | join(",")' "true,2022,0,false,true"
expect graph-many 3 .view.sectionView.caption "A still picture: 2020 areas, cases, decisions and crises are more than the 400 nodes the layout moves"
expect graph-many 4 .view.graph.hovered area:area-5
expect graph-many 5 '(.call | fromjson | (.to.x - .from.x - 80 | fabs) <= 2 and (.to.y - .from.y - 40 | fabs) <= 2)' true
# Strict: never a tick, never the Timer. Timing (drawing the 2022 nodes):
# the fastest of the three paints ≤ 8 ms, once more on a miss — gate-125
# missed "every paint ≤ 8" on a host loaded by other checks; the picture
# is the same in each, so the fastest is its cost (about 4 ms idle).
for i in 5 6 7; do
  expect graph-many $i '[.view.graph.ticks, .view.graph.timer] | map(tostring) | join(",")' "0,false"
done
graph_time_min graph-many .view.graph.drawMs 8 5 6 7
expect graph-many 6 .view.graph.cut 3
clean_log graph-many

# ---------------------------------------------------------------------------
# Bulk triage and Ask agent (WP-124b; ADR-0034 §6, ADR-0036): the button
# (only with open changes and an engine that can write), the proposal's row
# and detail (the bar's line, every evidence text author first, a crisis
# by its own button, the marks), Apply bound to the id the user saw, the
# outcome per item (applied ≠ done; a second Apply skips), the refusal of
# a launch, Ask agent on an event and a case, Discard. Exact argv each.
PROPOSAL=01M3VZS4J0NDXZFC2F7RBBD3FJ MONITORS=01M3KVWFR06078ZQTPRZCFYHK0
fx_work="$work/fx-triage"
mkdir -p "$fx_work"
tt="$tv.triage"
ttd="$tt.detail"
head='3 items proposed by agent:claude-code at 2026-10-01 17:02, 1 crisis held back — apply each below'
mkdir -p "$work/home-triage"
run triage "" 1920x1080 \
  "summon:$cl;view;clickName:triageAsk;settle;summon;hover:6 changes · newest first;clickName:proposalRow;pause:300;click:Apply proposals (2);wait:sectionView.triage.detail.result=Applied 2 · skipped 1 · refused 0;click:Apply this crisis;wait:sectionView.triage.detail.result=Applied 1 · skipped 0 · refused 0;click:Apply proposals;wait:sectionView.triage.detail.result=Applied 0 · skipped 3 · refused 0;view" \
  HOME="$work/home-triage" FAKE_SELDON_FIXTURE="$sample"
expect triage 2 "[$tt.button, $tt.row, $tt.shown] | map(tostring) | join(\",\")" "Agent sorts 6 open changes,Proposal · $head,false"
# Ask agent opened the agent's window: the desk steps aside (WP-156)
expect triage 4 "[.view.opened, .service.stepAsides] | map(tostring) | join(\",\")" "false,1"
expect triage 5 "[$tt.ask, $tt.askOk] | map(tostring) | join(\",\")" \
  "Agent started to sort 6 open changes; its proposal shows here · launcher default (omarchy),true"
expect triage 8 "[$tt.shown, $ttd.head, $ttd.state, ($ttd.actions | join(\"+\"))] | map(tostring) | join(\",\")" \
  "true,$head,agent:claude-code · proposal, nothing written yet,Apply proposals (2)+Discard"
expect triage 8 "[$ttd.crises[].id] | join(\",\")" "$UNIT"
expect triage 8 "[$ttd.regular[].id] | join(\",\")" "$THEME,$MONITORS"
expect triage 8 "$ttd.regular[0].evidence | join(\" | \")" \
  'Plan of C-2026-005: by human · - [ ] `omarchy theme set tokyo-night` | Journal 2026-10-01 17:00: by human · Zed fühlt sich gut an. Theme-Sync fehlt noch, siehe Inbox.'
expect triage 8 "[$ttd.regular[].flagged, $ttd.crises[].flagged] | map(tostring) | join(\",\")" "false,false,false"
shows triage 8 "$head"
expect triage 8 "$ttd.hint" "$head"
expect triage 8 '.overflow | join(" | ")' ""
shows triage 8 'by human · - [ ] `omarchy theme set tokyo-night`'
shows triage 8 "by system · config-change ~/.config/hypr/monitors.conf: sha256 40ab1178 → 6d81c412"
shows triage 8 "Apply this crisis"
shows triage 8 "CRISES — EACH ON ITS OWN"
expect triage 10 "[$ttd.regular[].outcome] | join(\",\")" "done,done"
expect triage 10 "$ttd.crises[0].outcome" "skipped: crisis: applied only one by one (\`--item\`), never with the rest"
expect triage 12 "$ttd.crises[0].outcome" "done"
expect triage 14 "[$ttd.result, $ttd.resultOk, ([$ttd.regular[].outcome] | join(\"+\"))] | map(tostring) | join(\",\")" \
  "Applied 0 · skipped 3 · refused 0,true,skipped: no longer open drift: $THEME is already resolved+skipped: no longer open drift: $MONITORS is already resolved"
expect triage 15 "$ttd.state | startswith(\"Applied \")" true
shows triage 15 "Skipped: no longer open drift: $THEME is already resolved"
argv_check triage "$work/home-triage" "$(printf '%s\n' "$startup" "$(q agent ask triage --json)" \
  "$(q drift apply $PROPOSAL --json)" "$(q drift apply $PROPOSAL --item $UNIT --json)" "$(q drift apply $PROPOSAL --json)")"
clean_log triage

# (The hover moves the pointer off the button: the harness clicks without
# moving it, so the button's tooltip would open and take the next click.)

# The engine refuses the launch (no default agent) and one item (its
# evidence is gone): both shown, in the urgent colour, nothing else run.
mkdir -p "$work/home-triage-refused"
run triage-refused "" 1920x1080 \
  "summon:$cl;clickName:triageAsk;settle;view;hover:6 changes · newest first;clickName:proposalRow;pause:300;click:Apply proposals (2);wait:sectionView.triage.detail.result=Applied 1 · skipped 1 · refused 1;view" \
  HOME="$work/home-triage-refused" FAKE_SELDON_FIXTURE="$sample" FAKE_SELDON_NO_DEFAULT_AGENT=1 FAKE_SELDON_APPLY_REFUSED="$THEME"
refusal='no default agent: Omarchy has none set, so `omarchy agent prompt` cannot start one; nothing was launched. Fix: `omarchy default agent <name>` (e.g. claude), or set `[agent] launcher` in ~/.config/seldon/config.toml'
expect triage-refused 4 "[$tt.ask, $tt.askOk, $tt.button] | map(tostring) | join(\",\")" "$refusal,false,Agent sorts 6 open changes"
shows triage-refused 4 "$refusal"
expect triage-refused 10 "$ttd.regular[0].outcome" \
  'refused: evidence journal `2026-10-01 14:40` no longer resolves (no journal entry at 2026-10-01 14:40)'
shows triage-refused 10 'Refused: evidence journal `2026-10-01 14:40` no longer resolves (no journal entry at 2026-10-01 14:40)'
argv_check triage-refused "$work/home-triage-refused" "$(printf '%s\n' "$startup" "$(q agent ask triage --json)" "$(q drift apply $PROPOSAL --json)")"
clean_log triage-refused "jax\\.seldon: seldon agent exit 1: no default agent"

# Ask agent on an open change and on a case; then Discard: the row and the
# detail go, the logbook is untouched (only the three calls).
mkdir -p "$work/home-triage-ask"
run triage-ask "" 1920x1080 \
  "summon:$(sel $UNIT);click:Ask agent;settle;summon;text:3;select:C-2026-004;click:Ask agent;settle;summon;text:2;clickName:proposalRow;click:Discard;click:Confirm discard;settle;wait:sectionView.triage.row=;view" \
  HOME="$work/home-triage-ask" FAKE_SELDON_FIXTURE="$sample"
# each ask opens the agent's window: the desk steps aside (WP-156), summoned again
expect triage-ask 3 "[.view.opened, .service.stepAsides] | map(tostring) | join(\",\")" "false,1"
expect triage-ask 8 "[.view.opened, .service.stepAsides] | map(tostring) | join(\",\")" "false,2"
expect triage-ask 4 "[$td.ask, ($td.actions | join(\"+\"))] | map(tostring) | join(\",\")" \
  "Agent asked about $UNIT; it answers in its window · launcher default (omarchy),Ask agent+Link to case…+Explain…+Dismiss…"
expect triage-ask 9 "[$tc.ask, ($tc.actions | join(\"+\"))] | map(tostring) | join(\",\")" \
  "Agent asked about C-2026-004; it answers in its window · launcher default (omarchy),Hand to agent+To verification+Drop+Open in editor+Ask agent"
expect triage-ask 12 "[($ttd.actions | join(\"+\")), $ttd.hint] | join(\",\")" \
  "Apply proposals (2)+Confirm discard,Discard proposal $PROPOSAL? Click Confirm discard. The logbook does not change."
expect triage-ask 16 "[$tt.row, $tt.shown, $tt.button, $ttd.seen, $ttd.result] | map(tostring) | join(\",\")" \
  ",true,Agent sorts 6 open changes,gone,Proposal discarded; nothing in the logbook changed"
shows triage-ask 16 "Proposal $PROPOSAL is not there any more: applied and replaced, or discarded."
argv_check triage-ask "$work/home-triage-ask" "$(printf '%s\n' "$startup" "$(q agent ask drift $UNIT --json)" \
  "$(q agent ask case C-2026-004 --json)" "$(q drift discard $PROPOSAL --json)")"
clean_log triage-ask

# Nothing open: no button (the proposal row stays while the index names one).
jq '.summary.openDrift = 0 | .summary.crisis = 0 | .drift = []' "$sample" >"$fx_work/index.none.json"
mkdir -p "$work/home-triage-none"
run triage-none "" 1920x1080 "summon:$cl;view" HOME="$work/home-triage-none" FAKE_SELDON_FIXTURE="$fx_work/index.none.json"
expect triage-none 2 "[$tt.button, ($tt.row != \"\")] | map(tostring) | join(\",\")" ",true"
clean_log triage-none

# Dev mode (read-only): no button, the detail shows, its actions are off;
# evidence an agent wrote or nobody signed is marked, its text in full.
mkdir -p "$fx_work/flag/proposals"
cp "$sample" "$fx_work/flag/index.json"
jq '.items[0].evidence[1].text = "by agent:codex · the theme switch was mine, a test of the new palette, part of the Zed setup in C-2026-004 and nothing else" |
    .items[1].evidence[0].text = "by unknown · monitors.conf removed"' \
  "$fx/proposals/$PROPOSAL.json" >"$fx_work/flag/proposals/$PROPOSAL.json"
run triage-dev "$fx_work/flag/index.json" 1920x1080 "summon:$cl;clickName:proposalRow;view"
expect triage-dev 3 "[$tt.button, $tt.shown, ($ttd.actions | join(\"+\"))] | map(tostring) | join(\",\")" \
  ",true,Apply proposals (2) (off)+Discard (off)"
expect triage-dev 3 "[$ttd.regular[].flagged, $ttd.crises[].flagged] | map(tostring) | join(\",\")" "true,true,false"
shows triage-dev 3 "Read twice: some evidence names an agent or an unknown author."
shows triage-dev 3 "by agent:codex · the theme switch was mine, a test of the new palette, part of the Zed setup in C-2026-004 and nothing else"
clean_log triage-dev

# B1 (WP-124b round 2): the proposal the user opened is the one Apply
# names. A capture brings a newer proposal (agent:codex) while the detail
# shows the first: the pane says so, Apply, the crises and Discard are off,
# and the service refuses the first id when asked directly; only Review
# opens the new one. Nothing reaches the engine.
SWAP=01M3W10000000000000000000S
mkdir -p "$fx_work/swap/proposals"
jq --arg id "$SWAP" '.triage.id = $id | .triage.path = "proposals/\($id).json" | .triage.actor = "agent:codex"
    | .triage.at = "2026-10-01T17:30:00+02:00"' "$sample" >"$fx_work/swap/index.json"
jq --arg id "$SWAP" '.id = $id | .actor = "agent:codex" | .at = "2026-10-01T17:30:00+02:00"' \
  "$fx/proposals/$PROPOSAL.json" >"$fx_work/swap/proposals/$SWAP.json"
mkdir -p "$work/home-triage-swap"
run triage-swap "" 1920x1080 \
  "summon:$cl;clickName:proposalRow;view;text:c;wait:sectionView.triage.detail.seen=replaced;click:Apply proposals;click:Discard;service:applyProposal:$PROPOSAL;click:Review the new proposal;view" \
  HOME="$work/home-triage-swap" FAKE_SELDON_FIXTURE="$sample" FAKE_SELDON_FIXTURE_AFTER="$fx_work/swap/index.json"
expect triage-swap 3 "[$ttd.id, $ttd.seen] | join(\",\")" "$PROPOSAL,current"
replaced='Replaced by a newer proposal by agent:codex at 2026-10-01 17:30 — review it'
expect triage-swap 5 "[$ttd.id, $ttd.seen, $ttd.hint, ($ttd.actions | join(\"+\")), ($ttd.regular | length)] | map(tostring) | join(\",\")" \
  "$PROPOSAL,replaced,$replaced,Review the new proposal+Apply proposals (off)+Discard (off),0"
shows triage-swap 5 "$replaced"
expect triage-swap 8 "[.call, $ttd.result, $ttd.resultOk] | map(tostring) | join(\",\")" \
  "false,This proposal is not the current one any more; review what the Changelog shows now,false"
expect triage-swap 10 "[$ttd.id, $ttd.seen, ($ttd.actions | join(\"+\")), $ttd.result] | map(tostring) | join(\",\")" \
  "$SWAP,current,Apply proposals (2)+Discard,"
expect triage-swap 10 "$ttd.hint" "3 items proposed by agent:codex at 2026-10-01 17:30, 1 crisis held back — apply each below"
argv_check triage-swap "$work/home-triage-swap" "$(printf '%s\n' "$startup" "$(q capture --all --json --quiet)" "$(q status --json)")"
clean_log triage-swap

# R2, R3: a proposal of 200 items × 10 refs of 256 characters (dev mode,
# read-only). The items are built only when the detail shows; the desk
# opens and the detail opens in time; a 256-character text wraps on a
# 960 px desk and the stacked 700 px one with nothing outside its box.
mkdir -p "$fx_work/big/proposals"
cp "$sample" "$fx_work/big/index.json"
long="by human · $(printf 'ollama.service/%.0s' $(seq 1 20))"
long=${long:0:256}
node -e '
  const [id, text] = process.argv.slice(1)
  const ev = Array.from({ length: 10 }, (_, j) => ({ kind: "journal", ref: "2026-10-01 14:" + String(10 + j), text }))
  const ids = Array.from({ length: 200 }, (_, i) => "01M3W2" + String(i).padStart(20, "0"))
  const items = ids.map((e, i) => i === 0
    ? { eventId: "01M3VNJ9JGZ9169T01XCW16FT0", action: "explain", title: "t", intent: "i", crisis: true, evidence: ev }
    : { eventId: e, action: "link", caseId: "C-2026-004", crisis: false, evidence: ev })
  console.log(JSON.stringify({ id, at: "2026-10-01T17:02:00+02:00", actor: "agent:claude-code", logbook: "/home/user/Seldon",
    applied: null, items }))' "$PROPOSAL" "$long" >"$fx_work/big/proposals/$PROPOSAL.json"
check "triage-big: the text is 256 characters" "${#long}" 256
run triage-big "$fx_work/big/index.json" 1920x1080 \
  "fresh:$cl;view;timedClickName:proposalRow;wait:sectionView.triage.detail.built=true;resize:960x900;pause:300;resize:700x900;pause:300;resize:1920x1080;select:$THEME;view"
expect triage-big 2 "[$ttd.built, $tt.shown, (.firstFrame.createMs < 1500)] | map(tostring) | join(\",\")" "false,false,true"
# the click returns before the items are built (incubated in slices)
expect triage-big 3 "(.call | tonumber) < 200" true
expect triage-big 4 "[$ttd.built, ($ttd.regular | length), ($ttd.crises | length)] | map(tostring) | join(\",\")" \
  "true,199,1"
expect triage-big 4 "$ttd.regular[0].evidence[0]" "Journal 2026-10-01 14:10: $long"
for i in 4 6 8; do expect triage-big $i '.overflow | join(" | ")' ""; done
# an event selected: the proposal's items are dropped again
expect triage-big 11 "[$tt.shown, $ttd.built] | map(tostring) | join(\",\")" "false,false"
clean_log triage-big
echo "     triage-big: desk created in $(sed -n 2p "$work/triage-big.steps" | jq -r '.firstFrame.createMs') ms," \
  "the click took $(sed -n 3p "$work/triage-big.steps" | jq -r '.call') ms"

# ---------------------------------------------------------------------------
# Offscreen renders in three themes (only with DESK_SHOTS; not live
# screenshots): Today at 100 % and 50 %, Settings, the Changelog, Work,
# Decisions, System, Memory, the Prime Radiant at 100 % and 50 % (and a
# hover), the graph (settled, a hover, 50 %, the replay at day 12), and a
# not-initialised logbook with its notice.
if [[ -n ${DESK_SHOTS:-} ]]; then
  mkdir -p "$DESK_SHOTS"
  for theme in tokyo-night kanagawa catppuccin-latte; do
    home="$work/home-shot-$theme"
    mkdir -p "$home/.local/state/omarchy/current/theme"
    cp "$omarchy/themes/$theme/colors.toml" "$home/.local/state/omarchy/current/theme/colors.toml"
    run "shot-$theme" "$sample" 1920x1080 \
      "summon;shot:desk-$theme-100;width:50;shot:desk-$theme-50;width:100;text:,;shot:desk-$theme-settings;text:2;shot:desk-$theme-changelog;text:3;shot:desk-$theme-work;key:Down;view" \
      HOME="$home" HARNESS_SHOTS="$DESK_SHOTS"
    expect "shot-$theme" 6 .view.section settings
    clean_log "shot-$theme"
    run "shot-sections-$theme" "$sample" 1920x1080 \
      "summon;text:4;shot:desk-$theme-decisions;text:5;shot:desk-$theme-system;text:6;shot:desk-$theme-memory;text:7;shot:desk-$theme-radiant;key:Right;hoverItem:heatmap:-1;shot:desk-$theme-radiant-365d-hover;width:50;shot:desk-$theme-radiant-50;view" \
      HOME="$home" HARNESS_SHOTS="$DESK_SHOTS"
    rfits "shot-sections-$theme" 15
    clean_log "shot-sections-$theme"
    run "shot-graph-$theme" "$sample" 1920x1080 \
      "summon;text:8;wait:graph.sleeping=true;shot:desk-$theme-graph;graphHover:C-2026-004;shot:desk-$theme-graph-hover;leave;width:50;pause:400;shot:desk-$theme-graph-50;width:100;graphCut:12;wait:graph.sleeping=true;shot:desk-$theme-graph-replay" \
      HOME="$home" HARNESS_SHOTS="$DESK_SHOTS"
    expect "shot-graph-$theme" 5 .view.graph.hovered C-2026-004
    clean_log "shot-graph-$theme"
    # WP-137: the interrupted transaction's detail and the mixed one's list
    run "shot-tx-$theme" "$sample" 1920x1080 \
      "summon:{\"section\":\"changelog\",\"filter\":\"all\"};select:$GTK4;shot:desk-$theme-tx-interrupted;select:$PIPEWIRE;shot:desk-$theme-tx-mixed;select:$GTK4;width:50;shot:desk-$theme-tx-interrupted-50;key:Return;shot:desk-$theme-tx-interrupted-50-detail" \
      HOME="$home" HARNESS_SHOTS="$DESK_SHOTS"
    for i in 3 5 8 9; do expect "shot-tx-$theme" $i '.overflow | join(" | ")' ""; done
    clean_log "shot-tx-$theme"
    run "shot-uninit-$theme" "$fx/index-variants/not-initialised.json" 1920x1080 "summon;shot:desk-$theme-uninit" \
      HOME="$home" HARNESS_SHOTS="$DESK_SHOTS"
    clean_log "shot-uninit-$theme"
  done
fi

real_home_check desk-view

echo "desk-view: $pass passed, $fail failed"
((fail == 0))
