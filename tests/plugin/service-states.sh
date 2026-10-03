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
for tool in bash sh env cat sed date mkdir mv sleep basename tr grep jq; do
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
expect ok .pill "2 · 4"
expect ok .tone urgent
expect ok .engine present
expect ok .engineVersion 0.1.0-fake
expect ok .crisis "2 changes in the red zone need a reason"
expect ok .snapper ""
clean_log ok

# 2. Same index, no seldon on PATH.
run engine-missing 2500 PATH="$base_path" SELDON_INDEX="$fx/index.sample.json"
expect engine-missing .status engineMissing
expect engine-missing .pill "2 · 4"
expect engine-missing .banner "Seldon engine not installed"
clean_log engine-missing

# 3. Engine reports an uninitialised logbook through the index.
run not-initialised 2500 PATH="$fake_path" SELDON_INDEX="$fx/index-variants/not-initialised.json"
expect not-initialised .status notInitialised
expect not-initialised .banner "Logbook not initialised"
expect not-initialised .pill ""

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
expect index-stale .pill "2 · 4"
run index-fresh 2500 PATH="$fake_path" SELDON_INDEX="$fx/index.sample.json" SELDON_NOW="2026-10-01T19:05:00+02:00"
expect index-fresh .status ok
# 6b. Stale from the data: index-variants/index-stale says indexStale while the
#     dev clock (no SELDON_NOW) is pinned to its own generatedAt.
run variant-stale 2500 PATH="$fake_path" SELDON_INDEX="$fx/index-variants/index-stale.json"
expect variant-stale .status indexStale
expect variant-stale .banner "Index is stale"
expect variant-stale .pill "2 · 4"
clean_log variant-stale

# 7. Contract v2.
run contract-mismatch 2500 PATH="$fake_path" SELDON_INDEX="$fx/invalid/index.contract-v2.json"
expect contract-mismatch .status contractMismatch
expect contract-mismatch .indexContractVersion 2
expect contract-mismatch .banner "Index format mismatch"
expect contract-mismatch .pill ""
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
expect atomic-replace .pill "2 · 4"

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
expect live .pill "2 · 4"
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
expect init-later .pill "2 · 4"

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
# argv_check <case> <expected argv lines, as `printf '%q '` writes them>
argv_check() {
  local got want
  got=$(cat "$work/home-$1/argv.log" 2>/dev/null || true)
  want=$2
  if [[ $got == "$want" ]]; then
    pass=$((pass + 1)); echo "ok   $1: engine argv ($(wc -l <<<"$got") calls)"
  else
    fail=$((fail + 1)); echo "FAIL $1: engine argv differs"
    diff <(echo "$want") <(echo "$got") | sed 's/^/     /' || true
  fi
}
q() { printf '%q ' "$@"; }
run fix-engine 3000 PATH="$work/bin-tools:$base_path" SELDON_INDEX="$fx/index.sample.json" \
  HARNESS_FIX=copy,terminal HARNESS_RECORD="$work/fix-engine.record"
install_engine="curl -fsSL https://github.com/JohnAndrewsX/jax-seldon/releases/latest/download/install.sh | bash"
record_check fix-engine "$(printf '%s\n' wl-copy -- "$install_engine" -- \
  omarchy-launch-floating-terminal-with-presentation "$install_engine" --)"
run fix-contract 3000 PATH="$work/bin-tools:$fake_path" SELDON_INDEX="$fx/invalid/index.contract-v2.json" \
  HARNESS_FIX=copy HARNESS_RECORD="$work/fix-contract.record"
record_check fix-contract "$(printf '%s\n' wl-copy -- "omarchy plugin update jax.seldon" --)"
run fix-init 3000 PATH="$work/bin-tools:$fake_path" SELDON_INDEX="$fx/index-variants/not-initialised.json" \
  HARNESS_FIX=terminal HARNESS_RECORD="$work/fix-init.record"
record_check fix-init "$(printf '%s\n' omarchy-launch-floating-terminal-with-presentation "seldon init" --)"

# 14b. Snapper without permissions (ADR-0011): its banner, with the constant
#      fix behind Copy and Run in terminal, and Check again (WP-054). After
#      Run in terminal the hint shows, and a reload of the unchanged index
#      ("Check again" on the status banner, at 1.5 s) keeps it.
run snapper-degraded 3000 PATH="$work/bin-tools:$fake_path" SELDON_INDEX="$fx/index-variants/snapper-degraded.json" \
  HARNESS_FIX=snapper:copy,snapper:terminal HARNESS_RECHECK_MS=1500 HARNESS_RECORD="$work/snapper-degraded.record"
expect snapper-degraded .status ok
expect snapper-degraded .snapper "Snapshots not readable"
expect snapper-degraded .crisis "2 changes in the red zone need a reason"
snapper_actions="terminal:Run in terminal,copy:Copy,capture:Check again"
snapper_hint="When the command has finished, press Check again"
expect snapper-degraded '.snapperActions | join(",")' "$snapper_actions"
expect snapper-degraded .snapperHint "$snapper_hint"
snapper_fix='sudo snapper -c root set-config ALLOW_USERS=$USER SYNC_ACL=yes'
record_check snapper-degraded "$(printf '%s\n' wl-copy -- "$snapper_fix" -- \
  omarchy-launch-floating-terminal-with-presentation "$snapper_fix" --)"
clean_log snapper-degraded

# 14f. Issue #2, live: the engine reports snapper failing until the user's
#      fix. Run in terminal shows the hint (no engine call); Check again runs
#      the same capture-then-status as Capture now; the fake's index after
#      the second capture has snapper ok, so banner and hint are gone.
mkdir -p "$work/home-snapper-live"
actions='[["snapshot"], ["fix", "terminal", "snapper"], ["snapshot"], ["fix", "capture", "snapper"], ["wait"]]'
run snapper-live 3000 PATH="$work/bin-tools:$fake_path" HOME="$work/home-snapper-live" \
  FAKE_SELDON_FIXTURE="$fx/index-variants/snapper-degraded.json" FAKE_SELDON_FIXTURE_AFTER="$fx/index.sample.json" \
  HARNESS_ACTIONS="$actions" HARNESS_RECORD="$work/snapper-live.record" HARNESS_UNTIL=snapper=
snaps=$(sed 's/\x1b\[[0-9;]*m//g' "$work/snapper-live.log" | grep -a "HARNESS snapshot " | sed 's/.*HARNESS snapshot //' \
  | jq -r -s 'map([.snapper, (.snapperActions | join(",")), .snapperHint] | join(" | ")) | .[]' 2>/dev/null || true)
want_snaps=$(printf '%s\n' "Snapshots not readable | $snapper_actions | " \
  "Snapshots not readable | $snapper_actions | $snapper_hint")
if [[ $snaps == "$want_snaps" ]]; then
  pass=$((pass + 1)); echo "ok   snapper-live: banner, then the hint after Run in terminal"
else
  fail=$((fail + 1)); echo "FAIL snapper-live: snapshots were:"; echo "$snaps" | sed 's/^/     /'
fi
expect snapper-live .status ok
expect snapper-live .snapper ""
expect snapper-live .snapperHint ""
expect snapper-live .lastError ""
argv_check snapper-live "$(printf '%s\n' "$(q --version --json)" "$(q capture --all --json --quiet)" "$(q status --json)" \
  "$(q capture --all --json --quiet)" "$(q status --json)")"
record_check snapper-live "$(printf '%s\n' omarchy-launch-floating-terminal-with-presentation "$snapper_fix" --)"
clean_log snapper-live
#      Check again before the fix took: the capture's new index still has
#      snapper failing (another message, so the index differs even when
#      both writes fall in the same second), so the banner stays, with the
#      new message and without the hint.
mkdir -p "$work/home-snapper-still"
jq '(.state.collectors[] | select(.name == "snapper") | .message) = "Still no permission."' \
  "$fx/index-variants/snapper-degraded.json" >"$work/snapper-still.json"
run snapper-still 3000 PATH="$work/bin-tools:$fake_path" HOME="$work/home-snapper-still" \
  FAKE_SELDON_FIXTURE="$fx/index-variants/snapper-degraded.json" FAKE_SELDON_FIXTURE_AFTER="$work/snapper-still.json" \
  HARNESS_ACTIONS="$actions" HARNESS_RECORD="$work/snapper-still.record" HARNESS_UNTIL=snapperHint=
expect snapper-still .snapper "Snapshots not readable"
expect snapper-still .snapperDetail "Still no permission."
expect snapper-still .snapperHint ""
argv_check snapper-still "$(printf '%s\n' "$(q --version --json)" "$(q capture --all --json --quiet)" "$(q status --json)" \
  "$(q capture --all --json --quiet)" "$(q status --json)")"
clean_log snapper-still

# 14d. A failing non-snapper collector (index-variants/plugins-degraded): no
#      banner of its own today; the service stays ok and the log clean.
run plugins-degraded 2500 PATH="$fake_path" SELDON_INDEX="$fx/index-variants/plugins-degraded.json"
expect plugins-degraded .status ok
expect plugins-degraded .snapper ""
expect plugins-degraded .pill "2 · 4"
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
expect xdg .pill "2 · 4"

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
#     case id, an open target outside journal|ledger|status|logbook|<caseId>|
#     <ADR id>. The engine sees nothing but the start-up calls.
mkdir -p "$work/home-refused"
actions=$(jq -cn '[["log", "", ""], ["log", "x", "C-26-1; rm -rf ~"], ["open", "../../etc/passwd"], ["open", "memory"], ["log", " \n\t ", ""]]')
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

# 22. Work tab actions (WP-020): the exact argv of `plan new` (the title one
#     argument after `--`: an option-like title, quotes; zone, risk, area and
#     priority from the form) and of every step, one plan call at a time.
#     The fake engine moves the cases in the index it writes. Last, `done` on
#     an active case: the engine refuses, its message is the result, nothing
#     else changes and the panel-wide error line stays empty.
mkdir -p "$work/home-plan"
title2='Zed "second" editor'
actions=$(jq -cn --arg t2 "$title2" '[
  ["plan", "new", {title: "--help", zone: "yellow", risk: "R1", area: "", priority: "normal"}], ["wait"],
  ["plan", "new", {title: $t2, zone: "red", risk: "R2", area: "dev-env", priority: "high"}], ["wait"],
  ["plan", "start", "C-2026-005"], ["wait"], ["plan", "verify", "C-2026-005"], ["wait"],
  ["plan", "done", "C-2026-005"], ["wait"], ["plan", "drop", "C-2026-006"], ["wait"],
  ["plan", "done", "C-2026-003"]
]')
run plan 3000 PATH="$fake_path" HOME="$work/home-plan" FAKE_SELDON_FIXTURE="$fx/index.sample.json" HARNESS_ACTIONS="$actions"
argv_check plan "$(printf '%s\n' "$(q --version --json)" "$(q capture --all --json --quiet)" "$(q status --json)" \
  "$(q plan new --zone yellow --risk R1 --json -- --help)" \
  "$(q plan new --zone red --risk R2 --area dev-env --priority high --json -- "$title2")" \
  "$(q plan start C-2026-005 --json)" "$(q plan verify C-2026-005 --json)" "$(q plan done C-2026-005 --json)" \
  "$(q plan drop C-2026-006 --json)" "$(q plan done C-2026-003 --json)")"
refusal='C-2026-003 is active; `seldon plan done` needs a case that is verification; run `seldon plan verify` first'
expect plan .planResult.text "$refusal"
expect plan .planResult.ok false
expect plan .planResult.action done
expect plan .planResult.caseId C-2026-003
expect plan .lastError ""
expect plan .pill "2 · 4"
if [[ $(grep -a -c 'HARNESS action \["plan".* true$' "$work/plan.log") == 7 ]]; then
  pass=$((pass + 1)); echo "ok   plan: all seven calls queued"
else
  fail=$((fail + 1)); echo "FAIL plan: $(grep -a 'HARNESS action' "$work/plan.log")"
fi
columns=$(jq -c '.cases | map_values(map(.id + " " + .status))' "$work/home-plan/.local/state/seldon/index.json" 2>/dev/null || true)
want='{"queued":["C-2026-007 queued","C-2026-009 queued","C-2026-010 queued"],"active":["C-2026-003 active","C-2026-004 active"],"verification":["C-2026-008 verification"],"completed":["C-2026-006 dropped","C-2026-005 completed","C-2026-002 completed","C-2026-001 completed"]}'
if [[ $columns == "$want" ]]; then
  pass=$((pass + 1)); echo "ok   plan: the fake engine's index moved the cases"
else
  fail=$((fail + 1)); echo "FAIL plan: index columns $columns"
fi
clean_log plan

# 23. Refused before the engine is asked: no title, a bad area slug, an id
#     that is not a case id, a step the contract does not list.
mkdir -p "$work/home-plan-refused"
actions=$(jq -cn '[["plan", "new", {title: " \t ", zone: "yellow", risk: "R1"}],
  ["plan", "new", {title: "t", zone: "yellow", risk: "R1", area: "Dev Env"}],
  ["plan", "new", {title: "t", zone: "purple", risk: "R1"}],
  ["plan", "list", ""], ["plan", "start", "C-26-1; rm -rf ~"]]')
run plan-refused 2500 PATH="$fake_path" HOME="$work/home-plan-refused" FAKE_SELDON_FIXTURE="$fx/index.sample.json" HARNESS_ACTIONS="$actions"
argv_check plan-refused "$(printf '%s\n' "$(q --version --json)" "$(q capture --all --json --quiet)" "$(q status --json)")"
expect plan-refused .planResult.text "Not a case id: C-26-1; rm -rf ~"
expect plan-refused .planResult.ok false
if [[ $(grep -a -c 'HARNESS action .* false$' "$work/plan-refused.log") == 5 ]]; then
  pass=$((pass + 1)); echo "ok   plan-refused: all five refused"
else
  fail=$((fail + 1)); echo "FAIL plan-refused: $(grep -a 'HARNESS action' "$work/plan-refused.log")"
fi

# 24. A held lock (exit 4) reaches the result line; dev mode refuses plan.
mkdir -p "$work/home-plan-locked"
actions=$(jq -cn '[["plan", "start", "C-2026-005"]]')
run plan-locked 2500 PATH="$fake_path" HOME="$work/home-plan-locked" FAKE_SELDON_FIXTURE="$fx/index.sample.json" \
  FAKE_SELDON_LOCKED=1 HARNESS_ACTIONS="$actions"
expect plan-locked .planResult.text "the logbook is locked by another seldon (pid 4242)"
expect plan-locked .planResult.ok false
expect plan-locked .lastError ""
mkdir -p "$work/home-plan-dev"
run plan-devmode 2500 PATH="$fake_path" HOME="$work/home-plan-dev" SELDON_INDEX="$fx/index.sample.json" HARNESS_ACTIONS="$actions"
expect plan-devmode .planResult.text "Dev mode is read-only"
argv_check plan-dev "$(q --version --json)"

# 25. The drift sheet's calls (WP-021): the exact argv of link (the proposed
#     case), explain with the text `--help` (zone as the item's, so no
#     --zone), dismiss of a group member with --only and a quoted text,
#     explain of the rest of that group with zone, risk and area, then a
#     re-run (the engine's no-op: "Already resolved"), and a link to a case
#     the engine does not know (refused by the engine, its message the
#     result). `drift show` lists a group's members. The fake engine folds
#     every resolution into the index it writes.
THEME=01M3VTGNY0NZG4AY80814WSKGR UNIT=01M3VNJ9JGZ9169T01XCW16FT0 OLLAMA=01M3VNFTF8EVHWFFZ687N14Q0C
FIREFOX=01M3SXBQVR7AW8PJQC1YXDCQ14 LIBINPUT=01M3SXBRV0WPNQ721VWGG2WXZ1
mkdir -p "$work/home-drift"
quoted='say "hi"; $(reboot)'
actions=$(jq -cn --arg t "$THEME" --arg u "$UNIT" --arg o "$OLLAMA" --arg f "$FIREFOX" --arg l "$LIBINPUT" --arg q "$quoted" '[
  ["driftShow", $f], ["wait"],
  ["drift", "link", {eventId: $t, caseId: "C-2026-005", only: false}], ["wait"],
  ["drift", "explain", {eventId: $u, text: "--help", zone: "red", risk: "R1", area: "", itemZone: "red"}], ["wait"],
  ["drift", "dismiss", {eventId: $l, only: true, text: $q}], ["wait"],
  ["drift", "explain", {eventId: $f, text: "the rest", zone: "red", risk: "R3", area: "browser", itemZone: "yellow"}], ["wait"],
  ["drift", "link", {eventId: $t, caseId: "C-2026-005"}], ["wait"],
  ["drift", "link", {eventId: $o, caseId: "C-2026-999"}]
]')
run drift 3000 PATH="$fake_path" HOME="$work/home-drift" FAKE_SELDON_FIXTURE="$fx/index.sample.json" HARNESS_ACTIONS="$actions"
argv_check drift "$(printf '%s\n' "$(q --version --json)" "$(q capture --all --json --quiet)" "$(q status --json)" \
  "$(q drift show $FIREFOX --json)" \
  "$(q drift link $THEME C-2026-005 --json)" \
  "$(q drift explain $UNIT --json -- --help)" \
  "$(q drift dismiss $LIBINPUT --only --json -- "$quoted")" \
  "$(q drift explain $FIREFOX --zone red --risk R3 --area browser --json -- "the rest")" \
  "$(q drift link $THEME C-2026-005 --json)" \
  "$(q drift link $OLLAMA C-2026-999 --json)")"
expect drift '.driftShown.members | length' 3
expect drift .driftShown.eventId $FIREFOX
expect drift .driftResult.text "unknown case C-2026-999"
expect drift .driftResult.ok false
expect drift .driftResult.action link
expect drift .driftResult.eventId $OLLAMA
expect drift .lastError ""
expect drift .pill "2 · 1"
expect drift .crisis "1 change in the red zone needs a reason"
if grep -a -q 'HARNESS action \["drift","link",{"eventId":"'$THEME'","caseId":"C-2026-005"}\] true' "$work/drift.log"; then
  pass=$((pass + 1)); echo "ok   drift: the re-run was sent"
else
  fail=$((fail + 1)); echo "FAIL drift: $(grep -a 'HARNESS action' "$work/drift.log")"
fi
state="$work/home-drift/.local/state/seldon/index.json"
folded=$(jq -c '[.events[] | select(.id as $i | ["'$THEME'", "'$UNIT'", "'$LIBINPUT'", "'$FIREFOX'", "01M3SXBRV0E702XKBM22HEV1B8"] | index($i))
  | [.subject, .resolution, .resolutionDetail, .case]]' "$state" 2>/dev/null || true)
want='[["tokyo-night","linked",null,"C-2026-005"],["~/.config/systemd/user/ollama.service","explained","--help","C-2026-009"],["libinput","dismissed","say \"hi\"; $(reboot)",null],["noto-fonts","explained","the rest","C-2026-010"],["firefox","explained","the rest","C-2026-010"]]'
if [[ $folded == "$want" ]]; then
  pass=$((pass + 1)); echo "ok   drift: the fake engine folded every resolution"
else
  fail=$((fail + 1)); echo "FAIL drift: folded $folded"
fi
expect_index() { # expect_index <jq filter> <value> — the fake engine's index after scenario 25
  local got
  got=$(jq -c "$1" "$state" 2>/dev/null || true)
  if [[ $got == "$2" ]]; then
    pass=$((pass + 1)); echo "ok   drift index: $1 = $2"
  else
    fail=$((fail + 1)); echo "FAIL drift index: $1 = $got (want $2)"
  fi
}
expect_index '[.drift[].subject]' '["ollama"]'
expect_index '[.summary.openDrift, .summary.crisis]' '[1,1]'
expect_index '[.cases.completed[] | [.id, .zone, .risk, (.area // "")]] | .[0:2]' '[["C-2026-010","red","R3","browser"],["C-2026-009","red","R1",""]]'
expect_index '.cases.queued[0].proposedEvents' 'null'
clean_log drift

# 26. The re-run alone: its no-op answer is the result ("Already resolved").
mkdir -p "$work/home-drift-again"
echo "$THEME linked C-2026-005" >"$work/home-drift-again/resolved"
actions=$(jq -cn --arg t "$THEME" '[["drift", "link", {eventId: $t, caseId: "C-2026-005"}]]')
run drift-again 2500 PATH="$fake_path" HOME="$work/home-drift-again" FAKE_SELDON_FIXTURE="$fx/index.sample.json" HARNESS_ACTIONS="$actions"
expect drift-again .driftResult.text "Already resolved: linked to C-2026-005"
expect drift-again .driftResult.already true
expect drift-again .driftResult.ok true
expect drift-again .driftResult.caseId C-2026-005
expect drift-again .pill "2 · 4"

# 27. Refused before the engine is asked: a malformed event id, Link
#     without a case or with a malformed one, a blank or two-line text, a
#     bad area slug, an action the contract does not list, a malformed id
#     for `drift show`. Then a held lock (exit 4) and dev mode.
mkdir -p "$work/home-drift-refused"
actions=$(jq -cn --arg t "$THEME" '[["drift", "link", {eventId: ($t | ascii_downcase), caseId: "C-2026-005"}],
  ["drift", "link", {eventId: $t, caseId: ""}], ["drift", "link", {eventId: $t, caseId: "C-26-1; rm -rf ~"}],
  ["drift", "dismiss", {eventId: $t, text: " \t "}], ["drift", "explain", {eventId: $t, text: "two\nlines"}],
  ["drift", "explain", {eventId: $t, text: "x", area: "Dev Env"}], ["drift", "purge", {eventId: $t}],
  ["driftShow", "not-an-id"]]')
run drift-refused 2500 PATH="$fake_path" HOME="$work/home-drift-refused" FAKE_SELDON_FIXTURE="$fx/index.sample.json" HARNESS_ACTIONS="$actions"
argv_check drift-refused "$(printf '%s\n' "$(q --version --json)" "$(q capture --all --json --quiet)" "$(q status --json)")"
expect drift-refused .driftResult.text "Not a drift action: purge"
expect drift-refused .driftResult.ok false
if [[ $(grep -a -c 'HARNESS action .* false$' "$work/drift-refused.log") == 8 ]]; then
  pass=$((pass + 1)); echo "ok   drift-refused: all eight refused"
else
  fail=$((fail + 1)); echo "FAIL drift-refused: $(grep -a 'HARNESS action' "$work/drift-refused.log")"
fi
mkdir -p "$work/home-drift-locked"
actions=$(jq -cn --arg u "$UNIT" '[["drift", "dismiss", {eventId: $u, text: "x"}]]')
run drift-locked 2500 PATH="$fake_path" HOME="$work/home-drift-locked" FAKE_SELDON_FIXTURE="$fx/index.sample.json" \
  FAKE_SELDON_LOCKED=1 HARNESS_ACTIONS="$actions"
expect drift-locked .driftResult.text "the logbook is locked by another seldon (pid 4242)"
expect drift-locked .driftResult.ok false
expect drift-locked .lastError ""
mkdir -p "$work/home-drift-dev"
run drift-devmode 2500 PATH="$fake_path" HOME="$work/home-drift-dev" SELDON_INDEX="$fx/index.sample.json" HARNESS_ACTIONS="$actions"
expect drift-devmode .driftResult.text "Dev mode is read-only"
argv_check drift-dev "$(q --version --json)"

# 28. Decisions (WP-023): the exact argv of `decide --no-edit --json --
#     <title>` (an option-like title, one with quotes) and the `open <id>`
#     the service sends with the id from each answer; then `open ADR-0004`
#     and `open logbook` (the Memory tab's target). The fake engine adds
#     ADR-0005 and ADR-0006 to the index it writes; the editor launcher (a
#     recorder) gets each path.
mkdir -p "$work/home-decide"
dtitle='Zed "second" editor'
actions=$(jq -cn --arg t "$dtitle" '[["decide", "--help"], ["wait"], ["decide", $t], ["wait"],
  ["open", "ADR-0004"], ["open", "logbook"]]')
run decide 3000 PATH="$work/bin-tools:$fake_path" HOME="$work/home-decide" FAKE_SELDON_FIXTURE="$fx/index.sample.json" \
  HARNESS_ACTIONS="$actions" HARNESS_RECORD="$work/decide.record"
argv_check decide "$(printf '%s\n' "$(q --version --json)" "$(q capture --all --json --quiet)" "$(q status --json)" \
  "$(q decide --no-edit --json -- --help)" "$(q open ADR-0005 --editor --json)" \
  "$(q decide --no-edit --json -- "$dtitle")" "$(q open ADR-0006 --editor --json)" \
  "$(q open ADR-0004 --editor --json)" "$(q open logbook --editor --json)")"
expect decide .decideResult.text "Created ADR-0006 · $dtitle"
expect decide .decideResult.ok true
expect decide .decideResult.decisionId ADR-0006
expect decide .openResult.text "Opened $work/home-decide/Seldon in omarchy-launch-editor"
expect decide .lastError ""
if [[ $(grep -a -c 'HARNESS action .* true$' "$work/decide.log") == 4 ]]; then
  pass=$((pass + 1)); echo "ok   decide: all four calls queued"
else
  fail=$((fail + 1)); echo "FAIL decide: $(grep -a 'HARNESS action' "$work/decide.log")"
fi
decisions=$(jq -c '[.decisions[] | .id + " " + .status + " " + .path]' "$work/home-decide/.local/state/seldon/index.json" 2>/dev/null || true)
want='["ADR-0006 proposed decisions/ADR-0006-zed-second-editor.md","ADR-0005 proposed decisions/ADR-0005-help.md","ADR-0004 proposed decisions/ADR-0004-ollama-user-service.md","ADR-0003 accepted decisions/ADR-0003-zed.md","ADR-0002 accepted decisions/ADR-0002-snapshots.md","ADR-0001 accepted decisions/ADR-0001-language.md"]'
if [[ $decisions == "$want" ]]; then
  pass=$((pass + 1)); echo "ok   decide: the fake engine's index lists the new decisions"
else
  fail=$((fail + 1)); echo "FAIL decide: index decisions $decisions"
fi
record_check decide "$(printf '%s\n' \
  omarchy-launch-editor "$work/home-decide/Seldon/decisions/ADR-0005-help.md" -- \
  omarchy-launch-editor "$work/home-decide/Seldon/decisions/ADR-0006-zed-second-editor.md" -- \
  omarchy-launch-editor "$work/home-decide/Seldon/decisions/ADR-0004-ollama-user-service.md" -- \
  omarchy-launch-editor "$work/home-decide/Seldon" --)"
clean_log decide

# 29. Refused before the engine is asked: a blank, two-line or non-text
#     title, and open targets that are no decision id (a short or padded id,
#     shell syntax, a path) or not a target at all (memory).
mkdir -p "$work/home-decide-refused"
actions=$(jq -cn '[["decide", ""], ["decide", " \t "], ["decide", "two\nlines"], ["decide", 42],
  ["open", "ADR-4"], ["open", "ADR-0004; reboot"], ["open", "decisions/ADR-0001-language.md"], ["open", "memory"]]')
run decide-refused 2500 PATH="$work/bin-tools:$fake_path" HOME="$work/home-decide-refused" FAKE_SELDON_FIXTURE="$fx/index.sample.json" \
  HARNESS_ACTIONS="$actions" HARNESS_RECORD="$work/decide-refused.record"
argv_check decide-refused "$(printf '%s\n' "$(q --version --json)" "$(q capture --all --json --quiet)" "$(q status --json)")"
expect decide-refused .decideResult.text "Give the decision a title"
expect decide-refused .decideResult.ok false
expect decide-refused .openResult null
if [[ $(grep -a -c 'HARNESS action .* false$' "$work/decide-refused.log") == 8 ]]; then
  pass=$((pass + 1)); echo "ok   decide-refused: all eight refused"
else
  fail=$((fail + 1)); echo "FAIL decide-refused: $(grep -a 'HARNESS action' "$work/decide-refused.log")"
fi

# 30. The engine refuses: a held lock (exit 4) is the decide result and
#     nothing is opened; an unknown decision is the open result (and the
#     panel's error line). Dev mode refuses decide.
mkdir -p "$work/home-decide-locked"
actions=$(jq -cn '[["decide", "keep me"], ["wait"], ["open", "ADR-0009"]]')
run decide-locked 2500 PATH="$work/bin-tools:$fake_path" HOME="$work/home-decide-locked" FAKE_SELDON_FIXTURE="$fx/index.sample.json" \
  FAKE_SELDON_LOCKED=1 HARNESS_ACTIONS="$actions" HARNESS_RECORD="$work/decide-locked.record"
argv_check decide-locked "$(printf '%s\n' "$(q --version --json)" "$(q capture --all --json --quiet)" "$(q status --json)" \
  "$(q decide --no-edit --json -- "keep me")" "$(q open ADR-0009 --editor --json)")"
expect decide-locked .decideResult.text "the logbook is locked by another seldon (pid 4242)"
expect decide-locked .decideResult.ok false
expect decide-locked .decideResult.decisionId ""
expect decide-locked .openResult.text "unknown decision ADR-0009"
expect decide-locked .lastError "seldon open: unknown decision ADR-0009"
mkdir -p "$work/home-decide-dev"
actions=$(jq -cn '[["decide", "x"]]')
run decide-devmode 2500 PATH="$fake_path" HOME="$work/home-decide-dev" SELDON_INDEX="$fx/index.sample.json" HARNESS_ACTIONS="$actions"
expect decide-devmode .decideResult.text "Dev mode is read-only"
argv_check decide-dev "$(q --version --json)"

# 30. Start agent (WP-022): the exact argv `agent start <id> --json` for a
#     validated id only; a malformed id never reaches the engine; one call at
#     a time; the engine's answer (the launcher) and its refusals (a queued
#     case's hint, a missing launcher) land on the plan result line with
#     action "agent"; dev mode refuses.
mkdir -p "$work/home-agent"
actions=$(jq -cn '[["agent", "C-26-1; rm -rf ~"], ["agent", "C-2026-005"], ["agent", "C-2026-004"], ["wait"], ["agent", "C-2026-003"]]')
run agent 2500 PATH="$fake_path" HOME="$work/home-agent" FAKE_SELDON_FIXTURE="$fx/index.sample.json" HARNESS_ACTIONS="$actions"
argv_check agent "$(printf '%s\n' "$(q --version --json)" "$(q capture --all --json --quiet)" "$(q status --json)" \
  "$(q agent start C-2026-005 --json)" "$(q agent start C-2026-003 --json)")"
expect agent .planResult.text "Agent started on C-2026-003 · launcher default (omarchy)"
expect agent .planResult.ok true
expect agent .planResult.action agent
expect agent .planResult.caseId C-2026-003
expect agent .lastError ""
if [[ $(grep -a 'HARNESS action \["agent"' "$work/agent.log" | sed 's/.* //' | tr '\n' ' ') == "false true false true " ]]; then
  pass=$((pass + 1)); echo "ok   agent: malformed id refused, one call at a time"
else
  fail=$((fail + 1)); echo "FAIL agent: $(grep -a 'HARNESS action' "$work/agent.log")"
fi
if [[ $(cat "$work/home-agent/active-case" 2>/dev/null) == C-2026-003 ]]; then
  pass=$((pass + 1)); echo "ok   agent: the fake engine set the active case"
else
  fail=$((fail + 1)); echo "FAIL agent: active-case $(cat "$work/home-agent/active-case" 2>/dev/null)"
fi
clean_log agent
mkdir -p "$work/home-agent-queued"
actions=$(jq -cn '[["agent", "C-2026-005"]]')
run agent-queued 2500 PATH="$fake_path" HOME="$work/home-agent-queued" FAKE_SELDON_FIXTURE="$fx/index.sample.json" HARNESS_ACTIONS="$actions"
expect agent-queued .planResult.text 'C-2026-005 is queued; start it first: `seldon plan start C-2026-005`'
expect agent-queued .planResult.ok false
expect agent-queued .planResult.caseId C-2026-005
expect agent-queued .lastError ""
mkdir -p "$work/home-agent-nolauncher"
actions=$(jq -cn '[["agent", "C-2026-004"]]')
run agent-nolauncher 2500 PATH="$fake_path" HOME="$work/home-agent-nolauncher" FAKE_SELDON_FIXTURE="$fx/index.sample.json" \
  FAKE_SELDON_NO_LAUNCHER=1 HARNESS_ACTIONS="$actions"
expect agent-nolauncher .planResult.text 'launcher `default`: `omarchy` not found; set `[agent] launcher` in config.toml'
expect agent-nolauncher .planResult.ok false
expect agent-nolauncher .lastError ""
mkdir -p "$work/home-agent-dev"
run agent-devmode 2500 PATH="$fake_path" HOME="$work/home-agent-dev" SELDON_INDEX="$fx/index.sample.json" HARNESS_ACTIONS="$actions"
expect agent-devmode .planResult.text "Dev mode is read-only"
argv_check agent-dev "$(q --version --json)"

real_home_check service-states

echo "service-states: $pass passed, $fail failed"
((fail == 0))
