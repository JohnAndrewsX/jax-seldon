#!/usr/bin/env bash
# Drive plugin/Overlay.qml, the Prime Radiant, in a private headless
# Quickshell (tests/plugin/harness/overlay.qml): open and close it the way
# the shell's IPC does, pick periods with real keys and clicks, and check
# the slot counts per period and the layout at 1920×1080, 2560×1440 and
# both at a 1.25 output scale. The charts (WP-031): each chart's summary and
# numbers per period, hover read-outs from real mouse moves, the first frame
# after a fresh open (no aggregation; one paint per chart, by frame 2),
# repaints only on data or size change, the index variants and the empty
# states.
#
# Like panel-view.sh, it builds a temp config root with copies of the
# installed shell's Commons/ and Ui/, so `import qs.*` resolves to the real
# shell code. The overlay's layer-shell window cannot exist offscreen: the
# script runs a copy of plugin/ whose components/overlay/OverlayWindow.qml is
# replaced by tests/plugin/harness/OverlayWindow.qml (a plain Item filling
# the harness window). Nothing talks to the running omarchy-shell.
#
# OVERLAY_SHOTS=<dir> also renders the overlay at both sizes in three themes
# into <dir> (offscreen renders, not live screenshots).
# Needs quickshell, jq and the installed shell (host check; docs/TESTING.md).
set -euo pipefail

root=$(cd "$(dirname "$0")/../.." && pwd)
fx="$root/fixtures"
omarchy="${OMARCHY_PATH:-/usr/share/omarchy}"
shell_dir="$omarchy/shell"

qs_bin=$(command -v quickshell || command -v qs || true)
[[ -n $qs_bin ]] || { echo "overlay-view: quickshell not found" >&2; exit 1; }
command -v jq >/dev/null || { echo "overlay-view: jq not found" >&2; exit 1; }
[[ -d $shell_dir/Commons && -d $shell_dir/Ui ]] || { echo "overlay-view: shell not found at $shell_dir" >&2; exit 1; }
timeout_bin=$(command -v timeout) || { echo "overlay-view: timeout not found" >&2; exit 1; }

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
source "$root/tests/plugin/real-home-guard.sh"

config="$work/config"
plugin="$work/plugin"
mkdir -p "$config/Commons" "$config/Ui" "$work/home" "$work/bin"
cp "$shell_dir"/Commons/* "$config/Commons/"
cp "$shell_dir"/Ui/* "$config/Ui/"
cp "$root/tests/plugin/harness/overlay.qml" "$config/shell.qml"
cp -r "$root/plugin" "$plugin"
cp "$root/tests/plugin/harness/OverlayWindow.qml" "$plugin/components/overlay/OverlayWindow.qml"

# The fake engine answers the service's version probe; never a real seldon.
for tool in bash env cat sed date mkdir mv sleep basename grep jq; do
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

# run <case> <index file> <W>x<H> <steps> [VAR=value ...] — one harness run;
# step reports land in $work/<case>.steps (one JSON object per line), the
# whole log in <case>.log. HOME is $work/home unless the case names one.
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

# counts <case> <step> <period> <heatmap,series,driftBars,riskDonut,timeline,plan rows>
counts() {
  expect "$1" "$2" .view.period "$3"
  expect "$1" "$2" '[.view.slots[] | .rows | tostring] | join(",")' "$4"
}

# fits <case> <step> <W> <H> — six slots, each with a size, inside the
# window; no text leaves its slot or the window.
fits() {
  expect "$1" "$2" '.view.slots | length' 6
  expect "$1" "$2" "[.view.slots[] | select(.w <= 0 or .h <= 0 or .x < 0 or .y < 0 or .x + .w > $3 or .y + .h > $4)] | length" 0
  expect "$1" "$2" '.overflow | join(" | ")' ""
  expect "$1" "$2" .view.scrolls false
}

# summaries <case> <step> <text> — every chart's summary, " | "-joined.
summaries() {
  expect "$1" "$2" '[.view.slots[] | .chart.summary] | join(" | ")' "$3"
}

# hovered <case> <step> <slot> <text> — that chart's hover read-out, and it
# is the only one.
hovered() {
  expect "$1" "$2" "[.view.slots[] | select(.chart.hover != \"\") | .id + \"=\" + .chart.hover] | join(\" | \")" "$3=$4"
}

# paints <case> <step> <heatmap,series,driftBars,riskDonut,timeline,plan paints>
paints() {
  expect "$1" "$2" '[.view.slots[] | .chart.paints | tostring] | join(",")' "$3"
}

clean_log() {
  local bad
  bad=$(sed 's/\x1b\[[0-9;]*m//g' "$work/$1.log" | grep -a -E "ERROR|WARN|TypeError|ReferenceError|Binding loop|HARNESS error|nothing to click" \
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

sample="$fx/index.sample.json"
plan_s="2 active cases · 6 of 9 steps done"
risk_s="8 cases · R0 1 · R1 3 · R2 4 · R3 0 · all time"
drift_s="13 opened · 9 resolved in 5 weeks · peak 2026-W40"
s30="57 events on 12 of 30 days · busiest 2026-10-01 (30) | explicit 324 → 327 · total 2005 → 2009 · 2 samples | $drift_s | $risk_s | 7 cases (6 open) · 2 releases · 6 snapshots · 2 crises | $plan_s"
s90="62 events on 13 of 90 days · busiest 2026-10-01 (30) | explicit 323 → 327 · total 2004 → 2009 · 3 samples | $drift_s | $risk_s | 8 cases (6 open) · 2 releases · 6 snapshots · 2 crises | $plan_s"
s365=${s90/of 90 days/of 365 days}
sall=${s90/of 90 days/of 366 days}

# 1. IPC and keys at 1920×1080 on the sample. Closed until toggled; toggle
#    opens on the default period (90 d); 1–4 and ←/→, h/l pick periods (wrap);
#    Esc closes through shell.hide; toggle opens again, toggle closes; a
#    click on the scrim closes; summon with a payload opens on its period; a
#    click on a period chip picks one and keeps the keys with the overlay;
#    `call setPeriod` picks periods, an unknown id changes nothing; *Close*
#    closes.
run ipc "$sample" 1920x1080 \
  'view;toggle;text:1;text:2;text:3;text:4;key:Left;key:Right;key:Right;text:h;text:l;key:Escape;toggle;toggle;toggle;clickAt:4,4;summon:{"period":"365"};click:30 d;text:3;call:setPeriod:all;call:setPeriod:7;call:view:;click:Close'
expect ipc 1 .view.opened false
expect ipc 2 .view.opened true
expect ipc 2 .view.status ok
expect ipc 2 .view.banner ""
counts ipc 2 90 "90,3,5,3,18,2"
expect ipc 2 '.view.window.from + " " + .view.window.to' "2026-07-04 2026-10-01"
expect ipc 2 .view.meta "workstation-7f3a · Omarchy 4.0.7-1 · generated 2026-10-01 17:05"
shows ipc 2 "Prime Radiant"
shows ipc 2 "workstation-7f3a · Omarchy 4.0.7-1 · generated 2026-10-01 17:05"
shows ipc 2 "90 d · 2026-07-04 – 2026-10-01"
shows ipc 2 "1–4 period · ←/→ previous / next · Esc close"
for label in Heatmap Series DriftBars RiskDonut Timeline "The Plan"; do shows ipc 2 "$label"; done
# Each chart's summary is its caption, in the slot's title row.
summaries ipc 2 "$s90"
shows ipc 2 "62 events on 13 of 90 days · busiest 2026-10-01 (30)"
shows ipc 2 "$risk_s"
shows ipc 2 "$plan_s"
# The Plan's cards: id and risk, title, steps and agent.
shows ipc 2 "C-2026-003 · R2"
shows ipc 2 "Omarchy auf 4.0.7 aktualisieren"
shows ipc 2 "4/5 steps · agent: claude-code"
shows ipc 2 "2/4 steps · agent: claude-code"
expect ipc 2 '[.view.slots[] | .chart.empty] | any' false
expect ipc 2 .view.mode wide
fits ipc 2 1920 1080
counts ipc 3 30 "30,2,5,3,17,2"
summaries ipc 3 "$s30"
expect ipc 3 '.view.window.from + " " + .view.window.to' "2026-09-02 2026-10-01"
shows ipc 3 "57 events on 12 of 30 days · busiest 2026-10-01 (30)"
shows ipc 3 "30 d · 2026-09-02 – 2026-10-01"
counts ipc 4 90 "90,3,5,3,18,2"
counts ipc 5 365 "365,3,5,3,18,2"
summaries ipc 5 "$s365"
expect ipc 5 '.view.window.from' "2025-10-02"
counts ipc 6 all "366,3,5,3,18,2"
summaries ipc 6 "$sall"
expect ipc 6 '.view.window.from + "|" + .view.window.to' "|"
shows ipc 6 "All · everything in the index"
counts ipc 7 365 "365,3,5,3,18,2"
counts ipc 8 all "366,3,5,3,18,2"
counts ipc 9 30 "30,2,5,3,17,2"
counts ipc 10 all "366,3,5,3,18,2"
counts ipc 11 30 "30,2,5,3,17,2"
expect ipc 12 .view.opened false
expect ipc 12 '.hides | join(",")' jax.seldon
expect ipc 12 '.texts | length' 0
expect ipc 13 .view.opened true
expect ipc 14 .view.opened false
expect ipc 14 '.hides | length' 2
expect ipc 15 .view.opened true
expect ipc 16 .view.opened false
expect ipc 16 '.hides | length' 3
expect ipc 17 .view.opened true
counts ipc 17 365 "365,3,5,3,18,2"
counts ipc 18 30 "30,2,5,3,17,2"
expect ipc 18 .view.opened true
# The chip click left the keys with the overlay.
counts ipc 19 365 "365,3,5,3,18,2"
counts ipc 20 all "366,3,5,3,18,2"
expect ipc 20 .call all
expect ipc 21 .call all
expect ipc 21 .view.period all
expect ipc 22 '.call | fromjson | .slots | length' 6
expect ipc 22 '.call | fromjson | [.slots[] | .chart.summary] | join(" | ")' "$sall"
expect ipc 23 .view.opened false
expect ipc 23 '.hides | length' 4
# Nothing the overlay did aggregated anything: the service's passes are
# those of loading the index, the overlay's are none.
expect ipc 23 '.view.aggregations.overlay' 0
expect ipc 23 '.view.aggregations.service' "$(sed -n 1p "$work/ipc.steps" | jq .view.aggregations.service)"
clean_log ipc

# 2. A fresh open, as the shell's Loader does it (a new Overlay.qml on every
#    summon): the first frame runs no aggregation — the service's count is
#    the one from before the overlay existed, the overlay's own is 0 — and
#    has painted nothing yet (Canvas gets its context on the first frame);
#    every chart has painted once by frame 2. Switching periods aggregates
#    nothing and repaints only the charts whose data changed (RiskDonut and
#    The Plan have no period); hovering repaints nothing; a resize repaints
#    every chart once (the harness changes one dimension at a time: each is
#    a geometry change of its own). A second fresh open behaves the same.
#    (firstFrame is recorded on the frame after the step's report, so it
#    shows in the next step's.)
run fresh "$sample" 1920x1080 \
  'fresh;view;text:1;view;hoverItem:heatmap:-1;hoverItem:plan:0;leave;view;resize:2560x1080;view;fresh:{"period":"all"};view'
expect fresh 1 .firstFrame null
expect fresh 2 '.firstFrame.serviceBefore == .firstFrame.service' true
expect fresh 2 '.firstFrame.service > 0' true
expect fresh 2 .firstFrame.overlay 0
expect fresh 2 .firstFrame.opened true
expect fresh 2 '.firstFrame.paints | join(",")' "0,0,0,0,0,0"
expect fresh 2 .firstFrame.paintedBy 2
paints fresh 2 "1,1,1,1,1,1"
expect fresh 2 '.view.aggregations.service == .firstFrame.service' true
expect fresh 2 .view.aggregations.overlay 0
counts fresh 3 30 "30,2,5,3,17,2"
paints fresh 4 "2,2,2,1,2,1"
expect fresh 4 '.view.aggregations.service == .firstFrame.service' true
expect fresh 4 .view.aggregations.overlay 0
hovered fresh 5 heatmap "Thu 2026-10-01 · 30 events · pacman 7 · agent 6 · seldon 6 · snapper 4 · config 2 · manual 2 · omarchy 1 · plugins 1 · theme 1"
shows fresh 5 "Thu 2026-10-01 · 30 events · pacman 7 · agent 6 · seldon 6 · snapper 4 · config 2 · manual 2 · omarchy 1 · plugins 1 · theme 1"
hovered fresh 6 plan "C-2026-003 · Omarchy auf 4.0.7 aktualisieren · 4/5 steps · agent: claude-code · red R2"
expect fresh 7 '[.view.slots[] | .chart.hover] | join("")' ""
paints fresh 8 "2,2,2,1,2,1"
expect fresh 9 .view.size.w 2560
paints fresh 10 "3,3,3,2,3,2"
fits fresh 10 2560 1080
expect fresh 12 '.firstFrame.serviceBefore == .firstFrame.service' true
expect fresh 12 .firstFrame.overlay 0
expect fresh 12 '.firstFrame.paints | join(",")' "0,0,0,0,0,0"
expect fresh 12 .firstFrame.paintedBy 2
expect fresh 12 .view.period all
paints fresh 12 "1,1,1,1,1,1"
clean_log fresh

# 3. Hover read-outs, from real mouse moves onto each chart's items
#    (chart.locate), and from `call hover <slot> <fx>,<fy>` (fractions of
#    the plot, what `shell call jax.seldon hover …` does on a live shell).
#    A malformed argument (no such slot, not two numbers, a point outside
#    [0, 1]) returns { error } and leaves the hover as it was.
run hover "$sample" 1920x1080 \
  'fresh;view;hoverItem:heatmap:0;hoverItem:series:1;hoverItem:driftBars:-1;hoverItem:riskDonut:0;hoverItem:riskDonut:2;hoverItem:timeline:2;hoverItem:timeline:11;hoverItem:plan:1;text:1;hoverItem:heatmap:-2;hoverItem:timeline:12;call:hover:riskDonut 0.99,0.01;call:hover:driftBars 0.97,0.5;call:hover:;call:hover:nope 0.5,0.5;call:hover:driftBars 0.97,0.5;call:hover:series .,.;call:hover:series 1.2.3,0.5;call:hover:series 1.5,0.5;call:hover:series 0.5;call:hover:series .5,1'
hovered hover 3 heatmap "Sat 2026-07-04 · 0 events"
hovered hover 4 series "2026-09-03 · explicit 324 · total 2005"
hovered hover 5 driftBars "2026-W40 · 28 Sep – 4 Oct · opened 6 · resolved 2"
hovered hover 6 riskDonut "R0 · 1 case · 13% · all time"
hovered hover 7 riskDonut "R2 · 4 cases · 50% · all time"
hovered hover 8 timeline "snapshot · 111 vor Snapshot-Aufräumen · 2026-09-30 19:00"
hovered hover 9 timeline "case · C-2026-002 Hyprland-Monitorlayout für Dual-WQHD · 2026-09-12 – 2026-09-13"
hovered hover 10 plan "C-2026-004 · Zed als zweiten Editor installieren · 2/4 steps · agent: claude-code · red R2"
# A period switch clears the hover of the charts whose data changed; The
# Plan has no period, so its hover (the pointer is still on it) stays.
hovered hover 11 plan "C-2026-004 · Zed als zweiten Editor installieren · 2/4 steps · agent: claude-code · red R2"
hovered hover 12 heatmap "Wed 2026-09-30 · 8 events · pacman 3 · snapper 3 · manual 1 · seldon 1"
hovered hover 13 timeline "case · C-2026-004 Zed als zweiten Editor installieren · 2026-09-28 – open"
expect hover 14 '.call | fromjson | .slot + "=" + .hover' "riskDonut="
expect hover 15 '.call | fromjson | .slot + "=" + .hover' "driftBars=2026-W40 · 28 Sep – 4 Oct · opened 6 · resolved 2"
expect hover 16 '[.view.slots[] | .chart.hover] | join("")' ""
expect hover 17 '.call | fromjson | .error' "no chart nope"
# Every malformed argument: an error, and the DriftBars hover set at step 18
# is still there.
hovered hover 18 driftBars "2026-W40 · 28 Sep – 4 Oct · opened 6 · resolved 2"
for step in 19 20 21 22; do
  expect hover $step '.call | fromjson | .error' 'expected "<slot> <x>,<y>" with x and y in [0, 1], or ""'
  hovered hover $step driftBars "2026-W40 · 28 Sep – 4 Oct · opened 6 · resolved 2"
done
# ".5" and "1" are fractions too (mid-September on 30 d: the 3 Sep sample
# holds there).
expect hover 23 '.call | fromjson | .slot + "=" + .hover' "series=2026-09-03 · explicit 324 · total 2005"
clean_log hover

# 4. Layout: 2560×1440, and both sizes at a 1.25 output scale (logical
#    1536×864 and 2048×1152; the test host runs 1.25), for every period.
#    Every chart has a plot area of its own.
for size in 2560x1440 1536x864 2048x1152; do
  run "size-$size" "$sample" "$size" "toggle;text:1;text:3;text:4"
  for step in 1 2 3 4; do fits "size-$size" $step "${size%x*}" "${size#*x}"; done
  expect "size-$size" 1 .view.mode wide
  expect "size-$size" 4 '[.view.slots[] | select(.chart.w < 100 or .chart.h < 40)] | length' 0
  counts "size-$size" 2 30 "30,2,5,3,17,2"
  summaries "size-$size" 2 "$s30"
  clean_log "size-$size"
done

# 5. The same 1920×1080 output rendered at QT_SCALE_FACTOR=1.25.
run scaled "$sample" 1536x864 "toggle;text:2" QT_SCALE_FACTOR=1.25
fits scaled 1 1536 864
fits scaled 2 1536 864
counts scaled 2 90 "90,3,5,3,18,2"
clean_log scaled

# 6. Narrow windows reflow the grid: two columns, then one slot per row and
#    a scrolling grid; slots never leave the window's width. The charts
#    follow their slots (a resize from wide to medium repaints them).
run medium "$sample" 760x1000 "toggle"
expect medium 1 .view.mode medium
expect medium 1 '[.view.slots[] | select(.x < 0 or .x + .w > 760)] | length' 0
# Rows: Heatmap | Series DriftBars | RiskDonut Timeline | The Plan.
expect medium 1 '[.view.slots | group_by(.y)[] | map(.id) | join(" ")] | join(" | ")' "heatmap | series driftBars | riskDonut timeline | plan"
expect medium 1 '.overflow | join(" | ")' ""
clean_log medium
run narrow "$sample" 560x700 "toggle"
expect narrow 1 .view.mode narrow
expect narrow 1 .view.scrolls true
# Only the scrolled-off rows reach past the window; no text leaves its slot.
expect narrow 1 '[.overflow[] | select(endswith("@window") | not)] | length' 0
expect narrow 1 '[.view.slots[] | select(.x < 0 or .x + .w > 560)] | length' 0
expect narrow 1 '[.view.slots | group_by(.y)[] | map(.id) | join(" ")] | join(" | ")' "heatmap | series | driftBars | riskDonut | timeline | plan"
clean_log narrow
run reflow "$sample" 1920x1080 "fresh;view;resize:760x1080;view;resize:560x1080;view"
expect reflow 3 .view.mode medium
paints reflow 4 "2,2,2,2,2,2"
expect reflow 4 '[.view.slots[] | select(.chart.w + 2 * 18 > .w)] | length' 0
expect reflow 5 .view.mode narrow
paints reflow 6 "3,3,3,3,3,3"
expect reflow 6 .view.aggregations.overlay 0
clean_log reflow

# 7. A logbook that is not initialised: the banner with its one fix that
#    runs no engine command (Copy) and the hint; every chart in its empty
#    state, nothing painted, no hover.
run uninit "$fx/index-variants/not-initialised.json" 1920x1080 "toggle;hoverItem:heatmap:0;call:hover:timeline 0.5,0.5"
expect uninit 1 .view.status notInitialised
expect uninit 1 .view.banner "Logbook not initialised"
shows uninit 1 "Logbook not initialised"
shows uninit 1 "Copy"
expect uninit 1 '[.texts[] | select(. == "Run in terminal" or . == "Check again")] | length' 0
shows uninit 1 "Fix it from the Seldon panel (click ⟡ in the bar)."
fits uninit 1 1920 1080
counts uninit 1 90 "0,0,0,0,0,0"
expect uninit 1 '[.view.slots[] | .chart.empty] | all' true
expect uninit 1 '[.texts[] | select(. == "no data in this period")] | length' 4
shows uninit 1 "no cases yet · all time"
shows uninit 1 "no active cases"
paints uninit 1 "0,0,0,0,0,0"
expect uninit 2 '[.view.slots[] | .chart.hover] | join("")' ""
expect uninit 3 '.call | fromjson | .slot + "=" + .hover' "timeline="
clean_log uninit

# 8. Every other index variant renders every chart (no empty state), with a
#    clean log. One paint per chart; two where a status banner (index-stale)
#    settles in the first frame: Qt's Canvas then paints once more at the
#    same size (WP-031 handover).
for variant in "$fx"/index-variants/*.json; do
  name=$(basename "$variant" .json)
  [[ $name == not-initialised ]] && continue
  run "variant-$name" "$variant" 1920x1080 "fresh;view"
  expect "variant-$name" 2 '[.view.slots[] | .chart.empty] | any' false
  if [[ $name == index-stale ]]; then
    expect "variant-$name" 2 '[.view.slots[] | .chart.paints] | min >= 1 and max <= 2' true
  else
    paints "variant-$name" 2 "1,1,1,1,1,1"
  fi
  fits "variant-$name" 2 1920 1080
  clean_log "variant-$name"
done

# 9. Offscreen renders in three themes at both sizes, plus one with a hover
#    (only with OVERLAY_SHOTS).
if [[ -n ${OVERLAY_SHOTS:-} ]]; then
  mkdir -p "$OVERLAY_SHOTS"
  for theme in osaka-jade tokyo-night catppuccin-latte; do
    home="$work/home-$theme"
    mkdir -p "$home/.local/state/omarchy/current/theme"
    cp "$omarchy/themes/$theme/colors.toml" "$home/.local/state/omarchy/current/theme/colors.toml"
    for size in 1920x1080 2560x1440; do
      run "shot-$theme-$size" "$sample" "$size" "fresh;view;shot:offscreen-$theme-$size;text:3;hoverItem:heatmap:-1;shot:offscreen-$theme-$size-365d-hover;view" \
        HOME="$home" HARNESS_SHOTS="$OVERLAY_SHOTS"
      fits "shot-$theme-$size" 2 "${size%x*}" "${size#*x}"
      clean_log "shot-$theme-$size"
    done
  done
fi

real_home_check overlay-view

echo "overlay-view: $pass passed, $fail failed"
((fail == 0))
