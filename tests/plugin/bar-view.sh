#!/usr/bin/env bash
# Render plugin/BarWidget.qml (the pill) in a private, headless Quickshell
# (tests/plugin/harness/bar.qml) and check the bar glyph (A4): the file for
# the box, the theme's tint, and brief check 4 — the glyph's vertical centre
# on the digits' centre within 1 px, at scale 1.0 and 1.25 — measured in
# the pixels of the render (tests/plugin/png-ink.py). The widget's own
# centres are in the report for the record only: the glyph is placed by
# the same formula, so comparing them could not fail.
#
# Scales: `100` font base size 12 (bar 26, box 16), `125` base size 15 (bar
# 33, box 20; the shell scales the bar with the font), `out125` base size 12
# on a 1.25 output (QT_SCALE_FACTOR; box 16 logical = 20 device px). Themes:
# Tokyo Night, Catppuccin Latte, Osaka Jade (their colors.toml in the
# scratch HOME). BAR_SHOTS=<dir> keeps the renders (bar-<theme>-<scale>.png);
# BAR_WORK=<dir> runs in that dir and keeps it (logs, reports) for debugging.
# Needs quickshell, jq, python3 and the installed shell (host check;
# docs/TESTING.md). Nothing talks to the running omarchy-shell.
set -euo pipefail

root=$(cd "$(dirname "$0")/../.." && pwd)
plugin="$root/plugin"
fx="$root/fixtures"
omarchy="${OMARCHY_PATH:-/usr/share/omarchy}"
shell_dir="$omarchy/shell"

qs_bin=$(command -v quickshell || command -v qs || true)
[[ -n $qs_bin ]] || { echo "bar-view: quickshell not found" >&2; exit 1; }
command -v jq >/dev/null || { echo "bar-view: jq not found" >&2; exit 1; }
command -v python3 >/dev/null || { echo "bar-view: python3 not found" >&2; exit 1; }
[[ -d $shell_dir/Commons && -d $shell_dir/Ui ]] || { echo "bar-view: shell not found at $shell_dir" >&2; exit 1; }
timeout_bin=$(command -v timeout) || { echo "bar-view: timeout not found" >&2; exit 1; }

work=${BAR_WORK:-$(mktemp -d)}
trap '[[ -n ${BAR_WORK:-} ]] || rm -rf "$work"' EXIT
source "$root/tests/plugin/real-home-guard.sh"

config="$work/config"
mkdir -p "$config/Commons" "$config/Ui" "$work/bin"
cp "$shell_dir"/Commons/* "$config/Commons/"
cp "$shell_dir"/Ui/* "$config/Ui/"
# BarWidget.qml loads Panel.qml, whose KeyboardPanel is a layer-shell window.
cp "$root/tests/plugin/harness/KeyboardPanel.qml" "$config/Ui/KeyboardPanel.qml"
cp "$root/tests/plugin/harness/bar.qml" "$config/shell.qml"

# The fake engine, so the dev-mode service reports ok; never a real seldon.
for tool in bash env cat sed date mkdir mv sleep basename grep jq; do
  ln -s "$(command -v "$tool")" "$work/bin/$tool"
done
install -m 755 "$root/tests/plugin/fake-seldon" "$work/bin/seldon"
printf '#!/bin/sh\nexit 1\n' >"$work/bin/hyprctl"
printf '#!/bin/sh\necho monospace\n' >"$work/bin/fc-match"
chmod 755 "$work/bin/hyprctl" "$work/bin/fc-match"
ln -s "$(command -v sh)" "$work/bin/sh"

pass=0
fail=0

check() { # check <label> <got> <want>
  if [[ $2 == "$3" ]]; then
    pass=$((pass + 1))
    echo "ok   $1 = $3"
  else
    fail=$((fail + 1))
    echo "FAIL $1 = $2 (want $3)"
  fi
}

within() { # within <label> <a> <b> <tolerance>
  if python3 -c "import sys; sys.exit(0 if abs($2 - $3) <= $4 else 1)"; then
    pass=$((pass + 1))
    echo "ok   $1: |$2 - $3| <= $4"
  else
    fail=$((fail + 1))
    echo "FAIL $1: |$2 - $3| > $4"
  fi
}

clean_log() {
  local bad
  bad=$(sed 's/\x1b\[[0-9;]*m//g' "$work/$1.log" | grep -a -E "ERROR|WARN|TypeError|ReferenceError|Binding loop" \
    | grep -a -v -E "WAYLAND_DISPLAY is present|QT_QPA_PLATFORM|--- WARNING ---|most functionality will be broken|Failed to start IPC server" || true)
  if [[ -z $bad ]]; then
    pass=$((pass + 1))
    echo "ok   $1: log clean"
  else
    fail=$((fail + 1))
    echo "FAIL $1: log has errors"
    echo "$bad" | sed 's/^/     /'
  fi
}

# run <case> <theme> <base size> <index> [VAR=value …] — one render; the
# report lands in $work/<case>.json, the PNG in $work/<case>.png.
run() {
  local name=$1 theme=$2 base=$3 index=$4 home="$work/home-$1"
  shift 4
  mkdir -p "$home/.config/omarchy" "$home/.local/state/omarchy/current/theme"
  printf '[font]\nbase-size = %s\n' "$base" >"$home/.config/omarchy/shell.toml"
  cp "$omarchy/themes/$theme/colors.toml" "$home/.local/state/omarchy/current/theme/colors.toml"
  env -i HOME="$home" XDG_STATE_HOME="$home/.local/state" XDG_CONFIG_HOME="$home/.config" \
    PATH="$work/bin" QT_QPA_PLATFORM=offscreen XDG_RUNTIME_DIR="$work" \
    HARNESS_PLUGIN_DIR="$plugin" HARNESS_SHOT="$work/$name.png" SELDON_INDEX="$index" "$@" \
    "$timeout_bin" 60 "$qs_bin" -p "$config/shell.qml" >"$work/$name.log" 2>&1 || true
  sed 's/\x1b\[[0-9;]*m//g' "$work/$name.log" | grep -a "HARNESS bar " | sed 's/.*HARNESS bar //' >"$work/$name.json" || true
  if [[ -n ${BAR_SHOTS:-} && -f $work/$name.png ]]; then
    mkdir -p "$BAR_SHOTS"
    cp "$work/$name.png" "$BAR_SHOTS/bar-$name.png"
  fi
}

field() { jq -r "$2" "$work/$1.json" 2>/dev/null || echo "<no report>"; }

# ink <case> <rect field> — png-ink.py over that rect, in device pixels.
ink() {
  local dpr x y w h
  dpr=$(field "$1" .dpr)
  read -r x y w h < <(jq -r --argjson d "$dpr" \
    ".$2 | [(.x*\$d|floor), (.y*\$d|floor), (.w*\$d|ceil), (.h*\$d|ceil)] | @tsv" "$work/$1.json")
  python3 "$root/tests/plugin/png-ink.py" "$work/$1.png" "$x" "$y" "$w" "$h"
}

# 1. The sample (2 active, 4 open drift, crises: the urgent tone) in three
# themes at the three scales.
for theme in tokyo-night catppuccin-latte osaka-jade; do
  for scale in 100 125 out125; do
    name="$theme-$scale"
    case $scale in
      100) run "$name" "$theme" 12 "$fx/index.sample.json"; want_file=a4-bar-glyph-16.svg; want_box=16 ;;
      125) run "$name" "$theme" 15 "$fx/index.sample.json"; want_file=a4-bar-glyph-20.svg; want_box=20 ;;
      out125) run "$name" "$theme" 12 "$fx/index.sample.json" QT_SCALE_FACTOR=1.25; want_file=a4-bar-glyph-20.svg; want_box=16 ;;
    esac
    check "$name file" "$(field "$name" .file)" "$want_file"
    check "$name box" "$(field "$name" .box)" "$want_box"
    check "$name ready" "$(field "$name" .ready)" true
    check "$name text" "$(field "$name" .pill.text)" "2 · 4"
    check "$name glyph" "$(field "$name" .pill.glyph)" "$want_file"
    check "$name tone" "$(field "$name" .pill.tone)" urgent
    # check 4 from the pixels (device px), and the tint: the hinted glyph's
    # pixels are exactly the pill's ink colour (the theme's urgent colour)
    glyph=$(ink "$name" glyphRect)
    digits=$(ink "$name" countsRect)
    gc=$(jq -r .centre <<<"$glyph")
    dc=$(jq -r .centre <<<"$digits")
    within "$name centre (pixels)" "$gc" "$dc" 1
    check "$name tint" "$(jq -r .colour <<<"$glyph")" "$(field "$name" .ink)"
    echo "     measure $name: glyph ink rows $(jq -r '"\(.top)–\(.bottom)"' <<<"$glyph"), centre $gc; digit rows $(jq -r '"\(.top)–\(.bottom)"' <<<"$digits"), centre $dc (device px, bar $(field "$name" .barSize), text $(field "$name" .fontSize) px, dpr $(field "$name" .dpr))"
    clean_log "$name"
  done
done

# 2. The other two tones, derived from the sample in the scratch dir: no
# crisis → accent (2 active cases), no crisis and no active case → the bar
# foreground (`· 4`). The glyph takes the tone in every theme.
jq '.summary.crisis = 0' "$fx/index.sample.json" >"$work/index-accent.json"
jq '.summary.crisis = 0 | .summary.activeCases = 0' "$fx/index.sample.json" >"$work/index-default.json"
for theme in tokyo-night catppuccin-latte osaka-jade; do
  for tone in accent default; do
    name="$theme-$tone"
    run "$name" "$theme" 12 "$work/index-$tone.json"
    check "$name tone" "$(field "$name" .pill.tone)" "$tone"
    check "$name text" "$(field "$name" .pill.text)" "$([[ $tone == accent ]] && echo "2 · 4" || echo "· 4")"
    glyph=$(ink "$name" glyphRect)
    digits=$(ink "$name" countsRect)
    within "$name centre (pixels)" "$(jq -r .centre <<<"$glyph")" "$(jq -r .centre <<<"$digits")" 1
    check "$name tint" "$(jq -r .colour <<<"$glyph")" "$(field "$name" .ink)"
    clean_log "$name"
  done
done
# The three tones differ in every theme (the tint follows the tone).
for theme in tokyo-night catppuccin-latte osaka-jade; do
  inks=$(for c in "$theme-100" "$theme-accent" "$theme-default"; do field "$c" .ink; done | sort -u | wc -l)
  check "$theme three tones" "$inks" 3
done

# 3. Not initialised: the glyph alone, no counts, dimmed.
run uninit tokyo-night 12 "$fx/index-variants/not-initialised.json"
check "uninit text" "$(field uninit .pill.text)" ""
check "uninit glyph" "$(field uninit .pill.glyph)" a4-bar-glyph-16.svg
check "uninit dimmed" "$(field uninit .pill.dimmed)" true
check "uninit ready" "$(field uninit .ready)" true
clean_log uninit

# 4. Two monitors, two widgets (WP-067): the bar builds the widget once per
# monitor, and `jax.seldon.panel` takes one handler. Exactly one widget
# registers it (no "another handler is registered" warning), an IPC `open`
# opens that widget's panel, and once its monitor is gone the other widget
# takes the target over. The IPC socket needs a short runtime dir (a unix
# socket path has at most 107 bytes; a long TMPDIR fails "Failed to start
# IPC server").
ipc_rt=$(mktemp -d /tmp/seldon-ipc.XXXXXX)
ln -s "$qs_bin" "$work/bin/quickshell"
run ipc tokyo-night 12 "$fx/index.sample.json" HARNESS_IPC="$config/shell.qml" XDG_RUNTIME_DIR="$ipc_rt"
rm -rf "$ipc_rt"
sed 's/\x1b\[[0-9;]*m//g' "$work/ipc.log" | grep -a "HARNESS ipc " | sed 's/.*HARNESS ipc //' >"$work/ipc.json" || true
check "ipc owners" "$(field ipc '.owners | map(tostring) | join(",")')" "true,false"
check "ipc open exit" "$(field ipc .open.exit)" 0
check "ipc open output" "$(field ipc .open.out)" ""
check "ipc open reaches the owner" "$(field ipc '.opened | map(tostring) | join(",")')" "true,false"
check "ipc close" "$(field ipc '.closed | map(tostring) | join(",")')" "false,false"
check "ipc owners after" "$(field ipc '.ownersAfter | map(tostring) | join(",")')" "null,true"
check "ipc open after exit" "$(field ipc .openAfter.exit)" 0
# "Target not found." (still exit 0) when nobody took the target over
check "ipc open after output" "$(field ipc .openAfter.out)" ""
check "ipc open after reaches the new owner" "$(field ipc '.openedAfter | map(tostring) | join(",")')" "null,true"
check "ipc one handler" "$(grep -a -c 'another handler is registered' "$work/ipc.log" || true)" 0
clean_log ipc

# 5. A widget in the bar's centre section (WP-078; once a centre anchor is
# set, the default): the bar lists a zero-size, hidden
# placeholder first and the drawn widget second. The drawn one owns the
# target (as the shell's pickDrawnSlot routes a hotkey) and IPC `open`
# reaches it; a live reconfiguration that draws the placeholder and hides
# the other moves the target with one handler at a time; when the owner
# goes, the hidden one is the only instance left and takes it.
ipc_rt=$(mktemp -d /tmp/seldon-ipc.XXXXXX)
run ipc-placeholder tokyo-night 12 "$fx/index.sample.json" HARNESS_IPC="$config/shell.qml" HARNESS_IPC_PLACEHOLDER=1 \
  XDG_RUNTIME_DIR="$ipc_rt"
rm -rf "$ipc_rt"
sed 's/\x1b\[[0-9;]*m//g' "$work/ipc-placeholder.log" | grep -a "HARNESS ipc " | sed 's/.*HARNESS ipc //' >"$work/ipc-placeholder.json" || true
check "ipc-placeholder owners" "$(field ipc-placeholder '.owners | map(tostring) | join(",")')" "false,true"
check "ipc-placeholder open output" "$(field ipc-placeholder .open.out)" ""
check "ipc-placeholder open reaches the drawn one" "$(field ipc-placeholder '.opened | map(tostring) | join(",")')" "false,true"
check "ipc-placeholder close" "$(field ipc-placeholder '.closed | map(tostring) | join(",")')" "false,false"
check "ipc-placeholder owners swapped" "$(field ipc-placeholder '.ownersSwapped | map(tostring) | join(",")')" "true,false"
check "ipc-placeholder open swapped output" "$(field ipc-placeholder .openSwapped.out)" ""
check "ipc-placeholder open swapped reaches the drawn one" "$(field ipc-placeholder '.openedSwapped | map(tostring) | join(",")')" "true,false"
check "ipc-placeholder owners after" "$(field ipc-placeholder '.ownersAfter | map(tostring) | join(",")')" "null,true"
check "ipc-placeholder open after output" "$(field ipc-placeholder .openAfter.out)" ""
check "ipc-placeholder open after reaches the last one" "$(field ipc-placeholder '.openedAfter | map(tostring) | join(",")')" "null,true"
check "ipc-placeholder one handler" "$(grep -a -c 'another handler is registered' "$work/ipc-placeholder.log" || true)" 0
clean_log ipc-placeholder

real_home_check bar-view

echo "bar-view: $pass passed, $fail failed"
((fail == 0))
