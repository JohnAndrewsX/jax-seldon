#!/usr/bin/env bash
# Drive plugin/Service.qml through every status of its state machine in a
# private, headless Quickshell instance (tests/plugin/harness/shell.qml).
# Nothing here talks to the running omarchy-shell or writes outside a temp
# dir. Needs quickshell and jq (host check; see docs/TESTING.md).
set -euo pipefail

root=$(cd "$(dirname "$0")/../.." && pwd)
plugin="$root/plugin"
harness="$root/tests/plugin/harness/shell.qml"
fx="$root/fixtures"

qs_bin=$(command -v quickshell || command -v qs || true)
[[ -n $qs_bin ]] || { echo "service-states: quickshell not found" >&2; exit 1; }
command -v jq >/dev/null || { echo "service-states: jq not found" >&2; exit 1; }

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

# A PATH with the system tools but no seldon.
base_path="/usr/bin:/bin"
if PATH=$base_path command -v seldon >/dev/null 2>&1; then
  echo "service-states: seldon is installed in $base_path; cannot build a PATH without it" >&2
  exit 1
fi
mkdir -p "$work/bin-fake" "$work/bin-late"
install -m 755 "$root/tests/plugin/fake-seldon" "$work/bin-fake/seldon"
fake_path="$work/bin-fake:$base_path"

pass=0
fail=0

# run <case> <ms> [VAR=value ...] — one harness run; the final snapshot lands
# in $work/<case>.json, the whole log in $work/<case>.log.
run() {
  local name=$1 ms=$2
  shift 2
  env -u SELDON_INDEX -u SELDON_NOW QT_QPA_PLATFORM=offscreen \
    HARNESS_PLUGIN_DIR="$plugin" HARNESS_MS="$ms" "$@" \
    timeout 60 "$qs_bin" -p "$harness" >"$work/$name.log" 2>&1 || true
  sed 's/\x1b\[[0-9;]*m//g' "$work/$name.log" | grep -a "HARNESS final " | sed 's/.*HARNESS final //' | tail -n 1 >"$work/$name.json" || true
}

# expect <case> <jq filter> <value>
expect() {
  local got
  got=$(jq -r "$2" "$work/$1.json" 2>/dev/null || true)
  [[ -n $got ]] || got="<no snapshot>"
  if [[ $got == "$3" ]]; then
    pass=$((pass + 1))
    echo "ok   $1: $2 = $3"
  else
    fail=$((fail + 1))
    echo "FAIL $1: $2 = $got (want $3)"
    sed 's/\x1b\[[0-9;]*m//g' "$work/$1.log" | tail -n 8 | sed 's/^/     /'
  fi
}

# QML errors and warnings, except the one a missing engine must cause.
clean_log() {
  local bad
  bad=$(sed 's/\x1b\[[0-9;]*m//g' "$work/$1.log" | grep -a -E "ERROR|WARN" \
    | grep -a -v -E "Process failed to start|WAYLAND_DISPLAY is present|QT_QPA_PLATFORM|--- WARNING ---|most functionality will be broken" || true)
  if [[ -z $bad ]]; then
    pass=$((pass + 1))
    echo "ok   $1: log clean"
  else
    fail=$((fail + 1))
    echo "FAIL $1: log has errors"
    echo "$bad" | sed 's/^/     /'
  fi
}

# 1. Fixture index, engine on PATH: the pill shows the fixture's counts.
run ok 2500 PATH="$fake_path" SELDON_INDEX="$fx/index.sample.json"
expect ok .status ok
expect ok .pill "⟡ 2 · 3"
expect ok .tone urgent
expect ok .engine present
expect ok .engineVersion 0.1.0-fake
clean_log ok

# 2. Same index, no seldon on PATH.
run engine-missing 2500 PATH="$base_path" SELDON_INDEX="$fx/index.sample.json"
expect engine-missing .status engineMissing
expect engine-missing .pill "⟡ 2 · 3"
expect engine-missing .banner "Seldon engine not installed"
clean_log engine-missing

# 3. Engine reports an uninitialised logbook through the index.
run not-initialised 2500 PATH="$fake_path" SELDON_INDEX="$fx/index-variants/not-initialised.json"
expect not-initialised .status notInitialised
expect not-initialised .banner "Logbook not initialised"
expect not-initialised .pill "⟡"

# 4. No index file.
run index-missing 2500 PATH="$fake_path" SELDON_INDEX="$work/does-not-exist.json"
expect index-missing .status indexMissing
expect index-missing .banner "No index yet"
clean_log index-missing

# 5. Unparseable index.
printf '{"contractVersion": 1, "generatedAt": ' >"$work/broken.json"
run index-unreadable 2500 PATH="$fake_path" SELDON_INDEX="$work/broken.json"
expect index-unreadable .status indexMissing
expect index-unreadable .banner "Index unreadable"

# 6. Stale: the dev clock three hours after the fixture's generatedAt.
run index-stale 2500 PATH="$fake_path" SELDON_INDEX="$fx/index.sample.json" SELDON_NOW="2026-10-01T20:05:12+02:00"
expect index-stale .status indexStale
expect index-stale .banner "Index is stale"
expect index-stale .pill "⟡ 2 · 3"
run index-fresh 2500 PATH="$fake_path" SELDON_INDEX="$fx/index.sample.json" SELDON_NOW="2026-10-01T19:05:00+02:00"
expect index-fresh .status ok

# 7. Contract v2.
run contract-mismatch 2500 PATH="$fake_path" SELDON_INDEX="$fx/invalid/index.contract-v2.json"
expect contract-mismatch .status contractMismatch
expect contract-mismatch .indexContractVersion 2
expect contract-mismatch .banner "Index format mismatch"
expect contract-mismatch .pill "⟡"
clean_log contract-mismatch

# 8. A relative SELDON_INDEX resolves against the shell's working directory.
(cd "$root" && run relative 2500 PATH="$fake_path" SELDON_INDEX="fixtures/index.sample.json")
expect relative .status ok

# 9. The index appears after start.
(sleep 1; cp "$fx/index.sample.json" "$work/late.json") &
run appears-later 8000 PATH="$fake_path" SELDON_INDEX="$work/late.json"
wait
expect appears-later .status ok

# 10. The index is replaced atomically (temp file + rename, CONTRACT.md rule 2).
cp "$fx/index-variants/not-initialised.json" "$work/swap.json"
(sleep 1; cp "$fx/index.sample.json" "$work/swap.json.tmp"; mv "$work/swap.json.tmp" "$work/swap.json") &
run atomic-replace 4000 PATH="$fake_path" SELDON_INDEX="$work/swap.json"
wait
expect atomic-replace .status ok
expect atomic-replace .pill "⟡ 2 · 3"

# 11. The engine is installed while the shell runs; "Check again" finds it.
(sleep 1; install -m 755 "$root/tests/plugin/fake-seldon" "$work/bin-late/seldon") &
run engine-appears 4000 PATH="$work/bin-late:$base_path" SELDON_INDEX="$fx/index.sample.json" HARNESS_RECHECK_MS=2000
wait
expect engine-appears .status ok
expect engine-appears .engine present

# 12. Live loop without the dev override: capture, then status writes the
#     index, which the service picks up. Calls never overlap.
mkdir -p "$work/home-live"
run live 9000 PATH="$fake_path" HOME="$work/home-live" FAKE_SELDON_FIXTURE="$fx/index.sample.json"
expect live .status ok
expect live .devMode false
expect live .pill "⟡ 2 · 3"
calls=$(tr '\n' ' ' <"$work/home-live/calls.log" | sed 's/ $//')
want="start --version end --version start capture end capture start status end status"
if [[ $calls == "$want" ]]; then
  pass=$((pass + 1)); echo "ok   live: engine calls serialised ($calls)"
else
  fail=$((fail + 1)); echo "FAIL live: engine calls = $calls (want $want)"
fi
clean_log live

# 13. Engine exit 3 (logbook not initialised) without any index.
mkdir -p "$work/home-uninit"
run live-uninit 4000 PATH="$fake_path" HOME="$work/home-uninit" FAKE_SELDON_MODE=uninit
expect live-uninit .status notInitialised
expect live-uninit .banner "Logbook not initialised"

# 13b. The user runs `seldon init`; "Check again" clears the banner.
mkdir -p "$work/home-init"
echo uninit >"$work/home-init/mode"
(sleep 1.5; echo ok >"$work/home-init/mode") &
run init-later 7000 PATH="$fake_path" HOME="$work/home-init" FAKE_SELDON_FIXTURE="$fx/index.sample.json" HARNESS_RECHECK_MS=2500
wait
expect init-later .status ok
expect init-later .pill "⟡ 2 · 3"

# 14. Banner fixes run fixed argument lists with constant commands only.
mkdir -p "$work/bin-tools"
for tool in wl-copy omarchy-launch-floating-terminal-with-presentation; do
  install -m 755 "$root/tests/plugin/fake-recorder" "$work/bin-tools/$tool"
done
record_check() { # record_check <case> <expected record>
  local got
  got=$(cat "$work/$1.record" 2>/dev/null || true)
  if [[ $got == "$2" ]]; then
    pass=$((pass + 1)); echo "ok   $1: fix commands"
  else
    fail=$((fail + 1)); echo "FAIL $1: fix commands were:"; echo "$got" | sed 's/^/     /'
  fi
}
run fix-engine 3000 PATH="$work/bin-tools:$base_path" SELDON_INDEX="$fx/index.sample.json" \
  HARNESS_FIX=copy,terminal HARNESS_RECORD="$work/fix-engine.record"
record_check fix-engine "$(printf '%s\n' wl-copy -- "omarchy pkg aur add jax-seldon" -- \
  omarchy-launch-floating-terminal-with-presentation "omarchy pkg aur add jax-seldon" --)"
run fix-contract 3000 PATH="$work/bin-tools:$fake_path" SELDON_INDEX="$fx/invalid/index.contract-v2.json" \
  HARNESS_FIX=copy HARNESS_RECORD="$work/fix-contract.record"
record_check fix-contract "$(printf '%s\n' wl-copy -- "omarchy plugin update jax.seldon" --)"
run fix-init 3000 PATH="$work/bin-tools:$fake_path" SELDON_INDEX="$fx/index-variants/not-initialised.json" \
  HARNESS_FIX=terminal HARNESS_RECORD="$work/fix-init.record"
record_check fix-init "$(printf '%s\n' omarchy-launch-floating-terminal-with-presentation "seldon init" --)"

# 15. Dev mode never runs the engine, not even on an explicit fix.
mkdir -p "$work/home-dev"
run fix-devmode 3000 PATH="$fake_path" SELDON_INDEX="$fx/index.sample.json" SELDON_NOW="2026-10-01T20:05:12+02:00" \
  HARNESS_FIX=capture HOME="$work/home-dev"
expect fix-devmode .status indexStale
expect fix-devmode .lastError "dev mode (SELDON_INDEX): engine calls are disabled"
if grep -q -v -x -E "(start|end) --version" "$work/home-dev/calls.log"; then
  fail=$((fail + 1)); echo "FAIL fix-devmode: engine was called: $(tr '\n' ' ' <"$work/home-dev/calls.log")"
else
  pass=$((pass + 1)); echo "ok   fix-devmode: engine only probed"
fi

echo "service-states: $pass passed, $fail failed"
((fail == 0))
