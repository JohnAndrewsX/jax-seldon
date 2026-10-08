#!/usr/bin/env bash
# The shell ends cleanly over IPC with two bar widgets (WP-162).
#
# `omarchy restart shell` (and so every `omarchy update`) ends the shell with
# `quickshell kill`. With more than one jax.seldon bar widget (two monitors,
# or a centre placeholder) the widget that owned `jax.seldon.panel` handed
# the target to a sibling from Component.onDestruction; the sibling's
# IpcHandler then registered with an engine generation that was being torn
# down, and Quickshell 0.3.1 crashed (SIGSEGV in
# IpcHandler::updateRegistration).
#
# Each case starts the bar harness (tests/plugin/harness/bar.qml,
# HARNESS_IPC_KILL) in a private, headless Quickshell with two widgets,
# waits until one of them owns the target, ends it with `quickshell kill`
# and checks: no widget becomes the owner once the shell is exiting (the
# hand-over that crashed; an IpcHandler that was not registered looks its
# registry up again when it is enabled, a registered one keeps its own), the
# kill succeeded, the shell exited with status 0, no crash report under the
# scratch HOME's .cache/quickshell/crashes, no quickshell of this config is
# left running, and a clean log.
#
# The harness does not reach the SIGSEGV itself: there Quickshell no longer
# finds the dying engine generation for the sibling's handler and skips the
# registration, where the shell's handler still finds it. The first check
# is the one that fails on the hand-over; the live restart with two monitors
# (docs/RELEASE.md) is the end-to-end proof.
#
# Cases: two drawn widgets, and a hidden centre placeholder next to a drawn
# one (WP-078), each with the owner created first (torn down after its
# sibling) and last (`late-owner`: torn down first, the shell's order).
#
# Every quickshell gets its own short runtime dir (a unix socket path has at
# most 107 bytes), removed afterwards; nothing talks to the running
# omarchy-shell or writes into the real runtime dir. Needs quickshell, jq
# and the installed shell (host check; docs/TESTING.md).
set -euo pipefail

root=$(cd "$(dirname "$0")/../.." && pwd)
plugin="$root/plugin"
fx="$root/fixtures"
omarchy="${OMARCHY_PATH:-/usr/share/omarchy}"
shell_dir="$omarchy/shell"

qs_bin=$(command -v quickshell || command -v qs || true)
[[ -n $qs_bin ]] || { echo "ipc-restart: quickshell not found" >&2; exit 1; }
command -v jq >/dev/null || { echo "ipc-restart: jq not found" >&2; exit 1; }
command -v pgrep >/dev/null || { echo "ipc-restart: pgrep not found" >&2; exit 1; }
setsid_bin=$(command -v setsid) || { echo "ipc-restart: setsid not found" >&2; exit 1; }
[[ -d $shell_dir/Commons && -d $shell_dir/Ui ]] || { echo "ipc-restart: shell not found at $shell_dir" >&2; exit 1; }

work=$(mktemp -d)
chmod 700 "$work"
rt=""
qs_pid=""
cleanup() {
  # Only the quickshell this script started, by its PID.
  if [[ -n $qs_pid ]] && kill -0 "$qs_pid" 2>/dev/null; then kill -9 "$qs_pid" 2>/dev/null || true; fi
  [[ -z $rt ]] || rm -rf "$rt"
  rm -rf "$work"
}
trap cleanup EXIT
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

clean_log() {
  local bad
  bad=$(sed 's/\x1b\[[0-9;]*m//g' "$work/$1.log" | grep -a -E "ERROR|WARN|TypeError|ReferenceError|Binding loop|crash" \
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

# wait_for <seconds> <command…> — poll until the command succeeds.
wait_for() {
  local tries=$(($1 * 10))
  shift
  while ((tries-- > 0)); do
    "$@" && return 0
    sleep 0.1
  done
  return 1
}

ready() { grep -a -q "HARNESS kill-ready " "$1"; }
gone() { ! kill -0 "$1" 2>/dev/null; }

# run <case> [VAR=value …] — start the harness, end it over IPC, check.
run() {
  local name=$1 home="$work/home-$1" log="$work/$1.log" status kill_exit owners left=()
  shift
  mkdir -p "$home/.config/omarchy" "$home/.local/state/omarchy/current/theme"
  printf '[font]\nbase-size = 12\n' >"$home/.config/omarchy/shell.toml"
  cp "$omarchy/themes/tokyo-night/colors.toml" "$home/.local/state/omarchy/current/theme/colors.toml"
  rt=$(mktemp -d /tmp/seldon-rt.XXXXXX)
  local envs=(HOME="$home" XDG_STATE_HOME="$home/.local/state" XDG_CONFIG_HOME="$home/.config"
    XDG_CACHE_HOME="$home/.cache" XDG_RUNTIME_DIR="$rt" PATH="$work/bin" QT_QPA_PLATFORM=offscreen)

  # Its own session (setsid execs, so $! is quickshell's PID and its session
  # id): a crash-report window or a restarted instance stays findable.
  "$setsid_bin" env -i "${envs[@]}" HARNESS_PLUGIN_DIR="$plugin" HARNESS_IPC="$config/shell.qml" \
    SELDON_INDEX="$fx/index.sample.json" "$@" "$qs_bin" -p "$config/shell.qml" >"$log" 2>&1 &
  qs_pid=$!

  if wait_for 30 ready "$log"; then
    owners=$(sed 's/\x1b\[[0-9;]*m//g' "$log" | grep -a "HARNESS kill-ready " | sed 's/.*HARNESS kill-ready //' \
      | jq -r '.owners | map(select(. == true)) | length')
    check "$name one owner before the kill" "$owners" 1
    kill_exit=0
    env -i "${envs[@]}" "$qs_bin" kill --pid "$qs_pid" >"$work/$name.kill.log" 2>&1 || kill_exit=$?
    check "$name quickshell kill exit" "$kill_exit" 0
  else
    check "$name harness ready" "not ready in 30 s" ready
  fi

  if wait_for 20 gone "$qs_pid"; then
    status=0
    wait "$qs_pid" || status=$?
  else
    status="still running after 20 s"
  fi
  check "$name exit status" "$status" 0
  # Once the shell is exiting, a widget may only let go of the target.
  check "$name no new owner while exiting" "$(sed 's/\x1b\[[0-9;]*m//g' "$log" \
    | sed -n '/Exiting due to IPC request/,$p' | grep -a -c -E 'HARNESS owner [0-9]+ true' || true)" 0

  # Quickshell writes a report per crash and may start a fresh instance.
  check "$name crash reports" "$(find "$home/.cache/quickshell/crashes" -mindepth 1 -maxdepth 1 2>/dev/null | wc -l)" 0
  local sid=$qs_pid pid
  qs_pid=""
  mapfile -t left < <({ pgrep -s "$sid" || true; pgrep -f -- "-p $config/shell.qml" || true; } | sort -u)
  check "$name nothing of this shell left" "${left[*]:-none}" none
  for pid in "${left[@]}"; do kill -9 "$pid" 2>/dev/null || true; done
  clean_log "$name"

  rm -rf "$rt"
  rt=""
}

run two HARNESS_IPC_KILL=1
run two-late-owner HARNESS_IPC_KILL=late-owner
run placeholder HARNESS_IPC_KILL=1 HARNESS_IPC_PLACEHOLDER=1
run placeholder-late-owner HARNESS_IPC_KILL=late-owner HARNESS_IPC_PLACEHOLDER=1

real_home_check ipc-restart

echo "ipc-restart: $pass passed, $fail failed"
((fail == 0))
