#!/usr/bin/env bash
# Self-test of tests/plugin/real-home-guard.sh in scratch HOMEs: a change
# by the operator's own engine passes, a test leak still fails; a new
# Quickshell instance dir in a watched runtime dir fails (WP-161). Needs jq.
set -euo pipefail

root=$(cd "$(dirname "$0")/../.." && pwd)
command -v jq >/dev/null || { echo "real-home-guard.test: jq not found" >&2; exit 1; }
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

total_pass=0
total_fail=0

# guard_case <name> <want: ok|live|FAIL> <config logbook, @ = HOME; "-": no config> <mutation> [runtime want: ok|FAIL]
# A fresh HOME with a config.toml (unless "-") and an engine state dir, a
# session runtime dir $HOME/run with one Quickshell instance dir (the live
# shell's), the guard sourced, the mutation run (bash, in HOME), then
# real_home_check: its first line is the home's verdict, the rest the
# runtime dirs' (all ok unless the runtime want says FAIL). The inherited
# XDG_RUNTIME_DIR is $HOME/xdg when the mutation names it, else unset.
guard_case() {
  local name=$1 want=$2 logbook=$3 mutation=$4 rt_want=${5:-ok} home="$work/$1" out got rt_got xdg=()
  logbook=${logbook//@/$home}
  mkdir -p "$home/.config/seldon" "$home/.local/state/seldon/hooks" "$home/run/quickshell/by-id/shell1" "$home/xdg"
  [[ $mutation == *xdg* ]] && xdg=(XDG_RUNTIME_DIR="$home/xdg")
  [[ $logbook == - ]] || printf '# test\nlanguage = "de"\nlogbook = "%s"\n' "$logbook" >"$home/.config/seldon/config.toml"
  echo '{"logbook":{"path":"'"$home"'/Seldon","language":"de","machine":"host-1"},"contractVersion":1}' \
    >"$home/.local/state/seldon/index.json"
  echo '{}' >"$home/.local/state/seldon/cursors.json"
  : >"$home/.local/state/seldon/lock"
  out=$(env -u XDG_RUNTIME_DIR HOME="$home" "${xdg[@]}" bash -c '
    set -euo pipefail
    pass=0; fail=0
    real_runtime_session=$HOME/run
    source "$1/tests/plugin/real-home-guard.sh"
    sleep 0.05
    cd "$HOME"; eval "$2"
    real_home_check guard' _ "$root" "$mutation" || true)
  rt_got=ok
  grep -q '^FAIL guard: new entries no process holds in ' <<<"$out" && rt_got=FAIL
  if [[ $rt_got != "$rt_want" ]]; then
    total_fail=$((total_fail + 1)); echo "FAIL $name runtime: $rt_got (want $rt_want)"; echo "$out" | sed 's/^/     /'
  else
    total_pass=$((total_pass + 1)); echo "ok   $name runtime: $rt_want"
  fi
  out=$(head -n 1 <<<"$out")
  case $out in
    "ok   guard: the real ~/.local/state/seldon and ~/.config/seldon are untouched") got=ok ;;
    "ok   guard: the real ~/.local/state/seldon changed by the operator's live engine (not a leak)") got=live ;;
    "FAIL guard:"*) got=FAIL ;;
    *) got="? $out" ;;
  esac
  if [[ $got == "$want" ]]; then
    total_pass=$((total_pass + 1)); echo "ok   $name: $want"
  else
    total_fail=$((total_fail + 1)); echo "FAIL $name: $got (want $want)"; echo "$out" | sed 's/^/     /'
  fi
}

# The engine's rewrite of its own state: same logbook, same machine.
rewrite='sed "s/\"contractVersion\"/\"x\":2,\"contractVersion\"/" .local/state/seldon/index.json >i && mv i .local/state/seldon/index.json && echo "{\"a\":1}" >.local/state/seldon/cursors.json && touch .local/state/seldon/lock'

guard_case untouched ok "@/Seldon" ":"
guard_case engine-rewrite live "@/Seldon" "$rewrite"
guard_case tilde-config live "~/Seldon" "$rewrite"
guard_case manifest-appears FAIL "@/Seldon" "$rewrite; echo {} >.local/state/seldon/manifest.json"
guard_case other-logbook FAIL "@/Seldon" 'echo "{\"logbook\":{\"path\":\"/tmp/test-logbook\",\"machine\":\"host-1\"}}" >.local/state/seldon/index.json'
guard_case other-machine FAIL "@/Seldon" 'sed -i "s/host-1/host-2/" .local/state/seldon/index.json'
guard_case new-file FAIL "@/Seldon" "$rewrite; echo x >.local/state/seldon/calls.log"
guard_case removed-file FAIL "@/Seldon" "$rewrite; rm .local/state/seldon/cursors.json"
guard_case hooks-changed FAIL "@/Seldon" "$rewrite; touch .local/state/seldon/hooks"
guard_case config-changed FAIL "@/Seldon" "$rewrite; echo '# edited' >>.config/seldon/config.toml"
guard_case no-config FAIL - "$rewrite"

# The session's runtime dir (WP-161): the live shell's instance dir stays,
# a harness Quickshell adds one.
guard_case runtime-untouched ok "@/Seldon" "ls run/quickshell/by-id >/dev/null"
guard_case runtime-new-instance ok "@/Seldon" "mkdir run/quickshell/by-id/test2" FAIL
guard_case runtime-first-instance ok "@/Seldon" "rm -r run/quickshell; mkdir -p run/quickshell/by-id/test2" FAIL
guard_case runtime-instance-gone ok "@/Seldon" "rm -r run/quickshell/by-id/shell1"
guard_case runtime-swapped ok "@/Seldon" "rm -r run/quickshell/by-id/shell1; mkdir run/quickshell/by-id/test2" FAIL
# A live instance started during the run (another Quickshell app, a shell
# restart) holds its files open and is no leftover; one held by nobody
# next to it still fails.
guard_case runtime-live-instance ok "@/Seldon" "mkdir run/quickshell/by-id/live2; exec 9>run/quickshell/by-id/live2/instance.lock"
guard_case runtime-live-and-leftover ok "@/Seldon" "mkdir run/quickshell/by-id/live2 run/quickshell/by-id/test2; exec 9>run/quickshell/by-id/live2/instance.lock" FAIL
# An inherited XDG_RUNTIME_DIR (a private one for the whole check) is
# watched as well.
guard_case runtime-xdg-untouched ok "@/Seldon" ": xdg"
guard_case runtime-xdg-new-instance ok "@/Seldon" "mkdir -p xdg/quickshell/by-id/test2" FAIL

echo "real-home-guard.test: $total_pass passed, $total_fail failed"
((total_fail == 0))
