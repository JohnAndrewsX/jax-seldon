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
# Its Quickshell gets a private runtime dir (WP-161), so its by-id/<id>
# logs never land in the session's; the dir holds links to exactly the
# session's Wayland socket and Hyprland dir, which the real window needs.
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

session_rt=${XDG_RUNTIME_DIR:?graph-live: no XDG_RUNTIME_DIR (needs a graphical session)} # live runtime dir: only its Wayland and Hyprland sockets, linked below

work=$(mktemp -d)
rt=$(mktemp -d /tmp/seldon-rt.XXXXXX)
chmod 700 "$rt"
# rm -rf removes the links in $rt, never what they point to.
trap 'rm -rf "$work" "$rt"' EXIT
# An absolute WAYLAND_DISPLAY needs no link.
[[ $WAYLAND_DISPLAY == /* ]] || ln -s "$session_rt/$WAYLAND_DISPLAY" "$rt/$WAYLAND_DISPLAY"
[[ ! -d $session_rt/hypr ]] || ln -s "$session_rt/hypr" "$rt/hypr"
mkdir -p "$work/config/Commons" "$work/config/Ui"
cp "$shell_dir"/Commons/* "$work/config/Commons/"
cp "$shell_dir"/Ui/* "$work/config/Ui/"
cp "$root/tests/plugin/harness/graph-live.qml" "$work/config/shell.qml"
cp -r "$root/plugin" "$work/plugin"

XDG_RUNTIME_DIR="$rt" HARNESS_PLUGIN_DIR="$work/plugin" SELDON_INDEX="$index" \
  timeout 150 "$qs_bin" -p "$work/config/shell.qml" 2>&1 \
  | sed 's/\x1b\[[0-9;]*m//g' | grep -a "GRAPH-LIVE" | sed 's/.*GRAPH-LIVE /GRAPH-LIVE /' || true
