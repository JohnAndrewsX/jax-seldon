#!/usr/bin/env bash
# The graph's live measurement (WP-125, ADR-0034 §5: "measured once on the
# test host"): the real desk in its real layer-shell window on this
# Hyprland session, read only, at section 8, on the index given — settle,
# replay, a cut, another section — with the graph's tick times and an
# event-loop probe (tests/plugin/harness/graph-live.qml). Not part of
# `just check`: it needs a graphical session and shows the desk for about
# half a minute (it takes the keyboard while shown).
#
# It touches neither the running shell nor ~/.config: like desk-view.sh it
# builds a scratch config root (copies of the shell's Commons/ and Ui/, a
# copy of plugin/) and runs its own Quickshell instance with SELDON_INDEX
# set (dev mode: nothing is written, no engine runs a capture).
#
# Usage: tests/plugin/graph-live.sh <index.json>   (prints GRAPH-LIVE lines)
set -euo pipefail

root=$(cd "$(dirname "$0")/../.." && pwd)
index=${1:?usage: graph-live.sh <index.json>}
[[ -r $index ]] || { echo "graph-live: cannot read $index" >&2; exit 1; }
index=$(realpath "$index")
omarchy="${OMARCHY_PATH:-/usr/share/omarchy}"
shell_dir="$omarchy/shell"
qs_bin=$(command -v quickshell || command -v qs || true)
[[ -n $qs_bin ]] || { echo "graph-live: quickshell not found" >&2; exit 1; }
[[ -n ${WAYLAND_DISPLAY:-} ]] || { echo "graph-live: no WAYLAND_DISPLAY (needs a graphical session)" >&2; exit 1; }

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
mkdir -p "$work/config/Commons" "$work/config/Ui"
cp "$shell_dir"/Commons/* "$work/config/Commons/"
cp "$shell_dir"/Ui/* "$work/config/Ui/"
cp "$root/tests/plugin/harness/graph-live.qml" "$work/config/shell.qml"
cp -r "$root/plugin" "$work/plugin"

HARNESS_PLUGIN_DIR="$work/plugin" SELDON_INDEX="$index" \
  timeout 150 "$qs_bin" -p "$work/config/shell.qml" 2>&1 \
  | sed 's/\x1b\[[0-9;]*m//g' | grep -a "GRAPH-LIVE" | sed 's/.*GRAPH-LIVE /GRAPH-LIVE /' || true
