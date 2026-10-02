#!/usr/bin/env bash
# Self-test of tests/plugin/real-home-guard.sh in scratch HOMEs: a change
# by the operator's own engine passes, a test leak still fails. Needs jq.
set -euo pipefail

root=$(cd "$(dirname "$0")/../.." && pwd)
command -v jq >/dev/null || { echo "real-home-guard.test: jq not found" >&2; exit 1; }
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

total_pass=0
total_fail=0

# guard_case <name> <want: ok|live|FAIL> <config logbook, @ = HOME; "-": no config> <mutation>
# A fresh HOME with a config.toml (unless "-") and an engine state dir, the
# guard sourced, the mutation run (bash, in HOME), then real_home_check.
guard_case() {
  local name=$1 want=$2 logbook=$3 mutation=$4 home="$work/$1" out got
  logbook=${logbook//@/$home}
  mkdir -p "$home/.config/seldon" "$home/.local/state/seldon/hooks"
  [[ $logbook == - ]] || printf '# test\nlanguage = "de"\nlogbook = "%s"\n' "$logbook" >"$home/.config/seldon/config.toml"
  echo '{"logbook":{"path":"'"$home"'/Seldon","language":"de","machine":"host-1"},"contractVersion":1}' \
    >"$home/.local/state/seldon/index.json"
  echo '{}' >"$home/.local/state/seldon/cursors.json"
  : >"$home/.local/state/seldon/lock"
  out=$(HOME=$home bash -c '
    set -euo pipefail
    pass=0; fail=0
    source "$1/tests/plugin/real-home-guard.sh"
    sleep 0.05
    cd "$HOME"; eval "$2"
    real_home_check guard' _ "$root" "$mutation" || true)
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

echo "real-home-guard.test: $total_pass passed, $total_fail failed"
((total_fail == 0))
