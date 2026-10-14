# Sourced by the Quickshell harnesses (desk-view, bar-view, ipc-restart,
# service-states): the one upstream warning a clean harness log may hold
# (WP-224). Every other WARN still fails a case.
#
# Omarchy edge's Commons/Style.qml imports Quickshell.Hyprland and connects
# to its Hyprland singleton (animation and gap settings). In an offscreen
# Quickshell with no Hyprland, Quickshell's Hyprland IPC module logs one
# line, without a file:
#   WARN: Unable to find hyprland socket. Cannot connect to hyprland.
# The line names no QML file, so it is attributed by imports: it is
# accepted only when the shell tree the harness loads has a Commons/Style.qml
# that imports Quickshell.Hyprland and every plugin file that imports it is
# one the harness names as never loaded (the plugin's DeskWindow.qml asks
# Hyprland for the focused monitor; desk-view replaces it with a stub, and
# only Desk.qml creates it). Then Style.qml is the only loaded importer. On
# Omarchy stable or rc (no such import) nothing is accepted, so a harness
# that starts loading such a plugin file fails there. The harnesses never
# fake a Hyprland socket or set HYPRLAND_INSTANCE_SIGNATURE. The ledger's
# style-hyprland-warning (tests/omarchy/assumptions.json) records the edge
# fact.

# The exact line, after the colour codes are stripped: the whole line.
UPSTREAM_HYPRLAND_WARNING='^[[:space:]]*WARN: Unable to find hyprland socket\. Cannot connect to hyprland\.$'

# upstream_allowed SHELL_DIR PLUGIN_DIR [NOT_LOADED...]: prints the ERE of
# the upstream lines a clean log may hold, or a pattern that matches no
# WARN line. NOT_LOADED: plugin files (relative to PLUGIN_DIR) the harness
# never loads.
upstream_allowed() {
  local shell=$1 plugin=$2 import='^[[:space:]]*import[[:space:]]+Quickshell\.Hyprland([[:space:];]|$)' f
  shift 2
  if [[ -d $plugin ]] && grep -qE "$import" "$shell/Commons/Style.qml" 2>/dev/null; then
    while IFS= read -r f; do
      f=${f#"$plugin"/}
      if [[ " $* " != *" $f "* ]]; then
        printf '%s\n' '^$'
        return 0
      fi
    done < <(grep -rlE "$import" --include='*.qml' --include='*.js' "$plugin" || true)
    printf '%s\n' "$UPSTREAM_HYPRLAND_WARNING"
  else
    printf '%s\n' '^$'
  fi
}
