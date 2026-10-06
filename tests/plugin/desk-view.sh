#!/usr/bin/env bash
# Drive plugin/Desk.qml, the desk (ADR-0034), in a private headless
# Quickshell (tests/plugin/harness/desk.qml): open and close it the way the
# shell's overlay loader does (hide drops the item, summon makes a new one),
# through the pill's clicks and the `jax.seldon.panel` shim; check the width
# against the setting on four screen widths, the sidebar and stacked
# thresholds, the keyboard map with real key events, the Settings section's
# one write per release, and the notices under the header (today's banners
# with their fixes). The old panel's and overlay's scenarios and where each
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
  bad=$(sed 's/\x1b\[[0-9;]*m//g' "$work/$1.log" | grep -a -E "ERROR|WARN|TypeError|ReferenceError|Binding loop|HARNESS error|nothing to (click|drag|hover)|wait timed out" \
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
#    the same preset again writes nothing (the value is stored). The
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
expect ipc 14 '[.call, .view.section] | join(",")' "ok,changelog"
expect ipc 15 .call "unknown source"
expect ipc 16 .view.opened false
expect ipc 17 '[.view.opened, .view.section] | map(tostring) | join(",")' "true,radiant"
expect ipc 18 '[.call, .view.section] | join(",")' "ok,decisions"
expect ipc 19 '[.call, .view.section] | join(",")' "unknown section,decisions"
expect ipc 20 .call "not found"
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
# Offscreen renders in three themes (only with DESK_SHOTS; not live
# screenshots): Today at 100 % and 50 %, Settings, and a not-initialised
# logbook with its notice.
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
    run "shot-uninit-$theme" "$fx/index-variants/not-initialised.json" 1920x1080 "summon;shot:desk-$theme-uninit" \
      HOME="$home" HARNESS_SHOTS="$DESK_SHOTS"
    clean_log "shot-uninit-$theme"
  done
fi

real_home_check desk-view

echo "desk-view: $pass passed, $fail failed"
((fail == 0))
