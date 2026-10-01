#!/usr/bin/env bash
# Drive plugin/Service.qml through every status of its state machine in a
# private, headless Quickshell instance (tests/plugin/harness/shell.qml).
# Nothing here talks to the running omarchy-shell or writes outside a temp
# dir: every run gets its own HOME, XDG_STATE_HOME and XDG_CONFIG_HOME
# there, and the real ones are checked at the end (real-home-guard.sh). Needs quickshell and jq (host check; see docs/TESTING.md).
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
source "$root/tests/plugin/real-home-guard.sh"

# A PATH with the tools the fakes need but never a seldon, even when one is
# installed system-wide: a private dir of symlinks to exactly those tools.
mkdir -p "$work/bin-base"
for tool in bash sh env cat sed date mkdir mv sleep basename tr grep; do
  path=$(command -v "$tool") || { echo "service-states: $tool not found" >&2; exit 1; }
  ln -s "$path" "$work/bin-base/$tool"
done
base_path="$work/bin-base"
timeout_bin=$(command -v timeout) || { echo "service-states: timeout not found" >&2; exit 1; }
mkdir -p "$work/bin-fake" "$work/bin-late"
install -m 755 "$root/tests/plugin/fake-seldon" "$work/bin-fake/seldon"
fake_path="$work/bin-fake:$base_path"

pass=0
fail=0

# run <case> <ms> [VAR=value ...] — one harness run; the final snapshot lands
# in $work/<case>.json, the whole log in $work/<case>.log. HOME is
# $work/home-<case> unless the case names one (HOME=… among the variables);
# XDG_STATE_HOME and XDG_CONFIG_HOME default to that HOME's, so neither the
# fake engine nor anything else can reach the real user's files.
run() {
  local name=$1 ms=$2 home arg
  shift 2
  home="$work/home-$name"
  for arg in "$@"; do [[ $arg == HOME=* ]] && home=${arg#HOME=}; done
  mkdir -p "$home"
  env -u SELDON_INDEX -u SELDON_NOW -u SELDON_CONFIG -u SELDON_LOGBOOK QT_QPA_PLATFORM=offscreen \
    HOME="$home" XDG_STATE_HOME="$home/.local/state" XDG_CONFIG_HOME="$home/.config" \
    HARNESS_PLUGIN_DIR="$plugin" HARNESS_MS="$ms" "$@" \
    "$timeout_bin" 60 "$qs_bin" -p "$harness" >"$work/$name.log" 2>&1 || true
  sed 's/\x1b\[[0-9;]*m//g' "$work/$name.log" | grep -a "HARNESS final " | sed 's/.*HARNESS final //' | tail -n 1 >"$work/$name.json" || true
}

# expect <case> <jq filter> <value>
expect() {
  local got="<no snapshot>"
  [[ -s $work/$1.json ]] && got=$(jq -r "$2" "$work/$1.json" 2>/dev/null || true)
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
expect ok .pill "⟡ 2 · 4"
expect ok .tone urgent
expect ok .engine present
expect ok .engineVersion 0.1.0-fake
expect ok .crisis "2 changes in the red zone need a reason"
expect ok .snapper ""
clean_log ok

# 2. Same index, no seldon on PATH.
run engine-missing 2500 PATH="$base_path" SELDON_INDEX="$fx/index.sample.json"
expect engine-missing .status engineMissing
expect engine-missing .pill "⟡ 2 · 4"
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
expect index-stale .pill "⟡ 2 · 4"
run index-fresh 2500 PATH="$fake_path" SELDON_INDEX="$fx/index.sample.json" SELDON_NOW="2026-10-01T19:05:00+02:00"
expect index-fresh .status ok
# 6b. Stale from the data: index-variants/index-stale says indexStale while the
#     dev clock (no SELDON_NOW) is pinned to its own generatedAt.
run variant-stale 2500 PATH="$fake_path" SELDON_INDEX="$fx/index-variants/index-stale.json"
expect variant-stale .status indexStale
expect variant-stale .banner "Index is stale"
expect variant-stale .pill "⟡ 2 · 4"
clean_log variant-stale

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
run appears-later 8000 HARNESS_UNTIL=status=ok PATH="$fake_path" SELDON_INDEX="$work/late.json"
wait
expect appears-later .status ok

# 10. The index is replaced atomically (temp file + rename, CONTRACT.md rule 2).
cp "$fx/index-variants/not-initialised.json" "$work/swap.json"
(sleep 1; cp "$fx/index.sample.json" "$work/swap.json.tmp"; mv "$work/swap.json.tmp" "$work/swap.json") &
run atomic-replace 4000 HARNESS_UNTIL=status=ok PATH="$fake_path" SELDON_INDEX="$work/swap.json"
wait
expect atomic-replace .status ok
expect atomic-replace .pill "⟡ 2 · 4"

# 11. The engine is installed while the shell runs; "Check again" finds it.
(sleep 1; install -m 755 "$root/tests/plugin/fake-seldon" "$work/bin-late/seldon") &
run engine-appears 4000 HARNESS_UNTIL=status=ok PATH="$work/bin-late:$base_path" SELDON_INDEX="$fx/index.sample.json" HARNESS_RECHECK_MS=2000
wait
expect engine-appears .status ok
expect engine-appears .engine present

# 12. Live loop without the dev override: capture, then status writes the
#     index, which the service picks up. Calls never overlap.
mkdir -p "$work/home-live"
#     An empty XDG_STATE_HOME counts as unset: plugin and engine use HOME's.
run live 9000 HARNESS_UNTIL=status=ok PATH="$fake_path" HOME="$work/home-live" XDG_STATE_HOME= FAKE_SELDON_FIXTURE="$fx/index.sample.json"
expect live .indexPath "$work/home-live/.local/state/seldon/index.json"
expect live .status ok
expect live .devMode false
expect live .pill "⟡ 2 · 4"
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
run live-uninit 4000 HARNESS_UNTIL=status=notInitialised PATH="$fake_path" HOME="$work/home-uninit" FAKE_SELDON_MODE=uninit
expect live-uninit .status notInitialised
expect live-uninit .banner "Logbook not initialised"

# 13b. The user runs `seldon init`; "Check again" clears the banner.
mkdir -p "$work/home-init"
echo uninit >"$work/home-init/mode"
(sleep 1.5; echo ok >"$work/home-init/mode") &
run init-later 7000 HARNESS_UNTIL=status=ok PATH="$fake_path" HOME="$work/home-init" FAKE_SELDON_FIXTURE="$fx/index.sample.json" HARNESS_RECHECK_MS=2500
wait
expect init-later .status ok
expect init-later .pill "⟡ 2 · 4"

# 14. Banner fixes run fixed argument lists with constant commands only.
mkdir -p "$work/bin-tools"
for tool in wl-copy omarchy-launch-floating-terminal-with-presentation; do
  install -m 755 "$root/tests/plugin/fake-recorder" "$work/bin-tools/$tool"
done
# One line per recorded invocation, sorted: detached launches have no order.
invocations() {
  awk 'BEGIN { RS = "--\n" } NF { gsub(/\n/, "\x1f"); print }' | LC_ALL=C sort
}
# The fixes start detached processes that may still be running when the
# harness quits, and a loaded machine slows them down: wait (up to 15 s) until
# the record holds as many invocations as expected.
record_check() { # record_check <case> <expected record>
  local got want deadline=$((SECONDS + 15))
  want=$(invocations <<<"$2"$'\n' | wc -l)
  while :; do
    got=$(cat "$work/$1.record" 2>/dev/null || true)
    (($(invocations <<<"$got"$'\n' | wc -l) >= want || SECONDS >= deadline)) && break
    sleep 0.2
  done
  if [[ $(invocations <<<"$got"$'\n') == "$(invocations <<<"$2"$'\n')" ]]; then
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

# 14b. Snapper without permissions (ADR-0011): its banner, with the constant
#      fix behind Copy and Run in terminal.
run snapper-degraded 3000 PATH="$work/bin-tools:$fake_path" SELDON_INDEX="$fx/index-variants/snapper-degraded.json" \
  HARNESS_FIX=snapper:copy,snapper:terminal HARNESS_RECORD="$work/snapper-degraded.record"
expect snapper-degraded .status ok
expect snapper-degraded .snapper "Snapshots not readable"
expect snapper-degraded .crisis "2 changes in the red zone need a reason"
snapper_fix='sudo snapper -c root set-config ALLOW_USERS=$USER SYNC_ACL=yes'
record_check snapper-degraded "$(printf '%s\n' wl-copy -- "$snapper_fix" -- \
  omarchy-launch-floating-terminal-with-presentation "$snapper_fix" --)"
clean_log snapper-degraded

# 14d. A failing non-snapper collector (index-variants/plugins-degraded): no
#      banner of its own today; the service stays ok and the log clean.
run plugins-degraded 2500 PATH="$fake_path" SELDON_INDEX="$fx/index-variants/plugins-degraded.json"
expect plugins-degraded .status ok
expect plugins-degraded .snapper ""
expect plugins-degraded .pill "⟡ 2 · 4"
clean_log plugins-degraded

# 14e. Omarchy from a git checkout (system.omarchy.repoHead) changes nothing here.
run git-checkout 2500 PATH="$fake_path" SELDON_INDEX="$fx/index-variants/omarchy-git-checkout.json"
expect git-checkout .status ok
clean_log git-checkout

# 14c. No crisis strip and no snapper banner without an index to report them.
run strip-hidden 2500 PATH="$fake_path" SELDON_INDEX="$fx/index-variants/not-initialised.json"
expect strip-hidden .crisis ""
expect strip-hidden .snapper ""

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

# 16. The index path honours XDG_STATE_HOME (CONTRACT.md rule 1); the fake
#     engine writes there too.
mkdir -p "$work/home-xdg" "$work/xdg-state"
run xdg 6000 HARNESS_UNTIL=status=ok PATH="$fake_path" HOME="$work/home-xdg" XDG_STATE_HOME="$work/xdg-state" FAKE_SELDON_FIXTURE="$fx/index.sample.json"
expect xdg .indexPath "$work/xdg-state/seldon/index.json"
expect xdg .status ok
expect xdg .pill "⟡ 2 · 4"

# 17. A relative XDG_STATE_HOME is invalid (XDG spec) and falls back to HOME.
mkdir -p "$work/home-xdg-rel"
run xdg-relative 2500 PATH="$fake_path" HOME="$work/home-xdg-rel" XDG_STATE_HOME="relative/state" FAKE_SELDON_MODE=uninit
expect xdg-relative .indexPath "$work/home-xdg-rel/.local/state/seldon/index.json"

# 18. Panel actions (WP-012): the exact argv of every engine call, in order.
#     The note is one argument after `--`, byte for byte: an option-like
#     text, quotes, a newline. Capture runs before status; a second "Capture
#     now" while one is queued is dropped. Open hands the engine's path to
#     the editor launcher (a recorder here) and the result line reads the
#     engine's `open --json` output.
# argv_check <case> <expected argv lines, as `printf '%q '` writes them>
argv_check() {
  local got want
  got=$(cat "$work/home-$1/argv.log" 2>/dev/null || true)
  want=$2
  if [[ $got == "$want" ]]; then
    pass=$((pass + 1)); echo "ok   $1: engine argv ($(wc -l <<<"$got") calls)"
  else
    fail=$((fail + 1)); echo "FAIL $1: engine argv differs"
    diff <(echo "$want") <(echo "$got") | sed 's/^/     /'
  fi
}
q() { printf '%q ' "$@"; }
install -m 755 "$root/tests/plugin/fake-recorder" "$work/bin-tools/omarchy-launch-editor"
mkdir -p "$work/home-actions"
note2='a "b" c'
note3=$'line one\nline two'
actions=$(jq -cn --arg n2 "$note2" --arg n3 "$note3" '[
  ["log", "--help", ""], ["log", $n2, "C-2026-004"], ["log", $n3, ""],
  ["open", "journal"], ["open", "ledger"], ["open", "status"], ["open", "C-2026-004"],
  ["capture"], ["capture"]
]')
run actions 3000 PATH="$work/bin-tools:$fake_path" HOME="$work/home-actions" FAKE_SELDON_FIXTURE="$fx/index.sample.json" \
  FAKE_SELDON_WRITTEN=3 HARNESS_ACTIONS="$actions" HARNESS_RECORD="$work/actions.record" HARNESS_UNTIL=capturing=false
argv_check actions "$(printf '%s\n' "$(q --version --json)" "$(q capture --all --json --quiet)" "$(q status --json)" \
  "$(q log --json -- --help)" "$(q log --case C-2026-004 --json -- "$note2")" "$(q log --json -- "$note3")" \
  "$(q open journal --editor --json)" "$(q open ledger --editor --json)" "$(q open status --editor --json)" \
  "$(q open C-2026-004 --editor --json)" "$(q capture --all --json --quiet)" "$(q status --json)")"
expect actions .logResult.text "Saved to the journal · 01M3W1FAKE0000000000000NTE"
expect actions .logResult.ok true
expect actions .openResult.text "Opened $work/home-actions/Seldon/work/active/C-2026-004.md in omarchy-launch-editor"
expect actions .captureResult.text "3 new events"
expect actions .lastError ""
if grep -a -q 'HARNESS action \["capture"\] false' "$work/actions.log"; then
  pass=$((pass + 1)); echo "ok   actions: a second Capture now while one is queued is dropped"
else
  fail=$((fail + 1)); echo "FAIL actions: the second capture was not dropped"
fi
if awk 'NR % 2 == 1 { if ($1 != "start") bad = 1; c = $2 } NR % 2 == 0 { if ($1 != "end" || $2 != c) bad = 1 } END { exit bad }' \
    "$work/home-actions/calls.log"; then
  pass=$((pass + 1)); echo "ok   actions: engine calls never overlap"
else
  fail=$((fail + 1)); echo "FAIL actions: overlapping engine calls: $(tr '\n' ' ' <"$work/home-actions/calls.log")"
fi
record_check actions "$(printf '%s\n' \
  omarchy-launch-editor "$work/home-actions/Seldon/journal/2026/2026-10-01.md" -- \
  omarchy-launch-editor "$work/home-actions/Seldon/ledger/2026-10.jsonl" -- \
  omarchy-launch-editor "$work/home-actions/Seldon/STATUS.md" -- \
  omarchy-launch-editor "$work/home-actions/Seldon/work/active/C-2026-004.md" --)"
clean_log actions

# 19. Refused before the engine is asked: blank notes, an id that is not a
#     case id, an open target outside journal|ledger|status|<caseId>. The
#     engine sees nothing but the start-up calls.
mkdir -p "$work/home-refused"
actions=$(jq -cn '[["log", "", ""], ["log", "x", "C-26-1; rm -rf ~"], ["open", "../../etc/passwd"], ["open", "logbook"], ["log", " \n\t ", ""]]')
run refused 2500 PATH="$work/bin-tools:$fake_path" HOME="$work/home-refused" FAKE_SELDON_FIXTURE="$fx/index.sample.json" \
  HARNESS_ACTIONS="$actions" HARNESS_RECORD="$work/refused.record"
argv_check refused "$(printf '%s\n' "$(q --version --json)" "$(q capture --all --json --quiet)" "$(q status --json)")"
expect refused .logResult.text "Write something first"
expect refused .logResult.ok false
expect refused .openResult null
if [[ $(grep -a -c 'HARNESS action .* false$' "$work/refused.log") == 5 ]]; then
  pass=$((pass + 1)); echo "ok   refused: all five refused"
else
  fail=$((fail + 1)); echo "FAIL refused: $(grep -a 'HARNESS action' "$work/refused.log")"
fi

# 20. The engine's errors reach the result lines: an unknown case for log
#     (shown by the QuickEntry only) and for open (also the panel's error line).
mkdir -p "$work/home-errors"
actions=$(jq -cn '[["log", "x", "C-2026-999"], ["open", "C-2026-999"]]')
run errors 2500 PATH="$work/bin-tools:$fake_path" HOME="$work/home-errors" FAKE_SELDON_FIXTURE="$fx/index.sample.json" \
  HARNESS_ACTIONS="$actions" HARNESS_RECORD="$work/errors.record"
expect errors .logResult.text "unknown case C-2026-999"
expect errors .logResult.ok false
expect errors .openResult.text "unknown case C-2026-999"
expect errors .lastError "seldon open: unknown case C-2026-999"
clean_log errors

# 21. Dev mode and a missing engine: actions are refused with a reason.
mkdir -p "$work/home-act-dev"
actions=$(jq -cn '[["log", "hello", ""], ["open", "journal"]]')
run actions-devmode 2500 PATH="$fake_path" HOME="$work/home-act-dev" SELDON_INDEX="$fx/index.sample.json" HARNESS_ACTIONS="$actions"
expect actions-devmode .canWrite false
expect actions-devmode .logResult.text "Dev mode is read-only"
expect actions-devmode .openResult.text "dev mode (SELDON_INDEX): engine calls are disabled"
argv_check act-dev "$(q --version --json)"
run actions-noengine 2500 PATH="$base_path" HOME="$work/home-act-dev" FAKE_SELDON_FIXTURE="$fx/index.sample.json" HARNESS_ACTIONS="$actions"
expect actions-noengine .logResult.text "Needs the Seldon engine"

real_home_check service-states

echo "service-states: $pass passed, $fail failed"
((fail == 0))
