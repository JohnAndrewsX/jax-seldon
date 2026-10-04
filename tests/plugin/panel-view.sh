#!/usr/bin/env bash
# Drive plugin/Panel.qml through its tabs, keys and banners in a private,
# headless Quickshell (tests/plugin/harness/panel.qml).
#
# The panel imports qs.Commons and qs.Ui, which only the shell provides. This
# builds a temp config root with copies of the installed shell's Commons/ and
# Ui/ (only Ui/KeyboardPanel.qml, a layer-shell window, is replaced by
# tests/plugin/harness/KeyboardPanel.qml), so the panel runs against the real
# shell components. Keys are real key events (QtTest keyClick) through the
# shell's own PanelKeyCatcher. Nothing talks to the running omarchy-shell.
# Needs quickshell, jq and the installed shell (host check; docs/TESTING.md).
set -euo pipefail

root=$(cd "$(dirname "$0")/../.." && pwd)
plugin="$root/plugin"
fx="$root/fixtures"
shell_dir="${OMARCHY_PATH:-/usr/share/omarchy}/shell"

qs_bin=$(command -v quickshell || command -v qs || true)
[[ -n $qs_bin ]] || { echo "panel-view: quickshell not found" >&2; exit 1; }
command -v jq >/dev/null || { echo "panel-view: jq not found" >&2; exit 1; }
[[ -d $shell_dir/Commons && -d $shell_dir/Ui ]] || { echo "panel-view: shell not found at $shell_dir" >&2; exit 1; }
timeout_bin=$(command -v timeout) || { echo "panel-view: timeout not found" >&2; exit 1; }

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
source "$root/tests/plugin/real-home-guard.sh"

config="$work/config"
mkdir -p "$config/Commons" "$config/Ui" "$work/home" "$work/bin"
cp "$shell_dir"/Commons/* "$config/Commons/"
cp "$shell_dir"/Ui/* "$config/Ui/"
cp "$root/tests/plugin/harness/KeyboardPanel.qml" "$config/Ui/KeyboardPanel.qml"
cp "$root/tests/plugin/harness/panel.qml" "$config/shell.qml"

# Tools for the fake engine, and the fake engine; never a real seldon.
for tool in bash env cat sed date mkdir mv sleep basename grep jq; do
  ln -s "$(command -v "$tool")" "$work/bin/$tool"
done
install -m 755 "$root/tests/plugin/fake-seldon" "$work/bin/seldon"
# The editor launcher the engine calls without a terminal records its argv.
install -m 755 "$root/tests/plugin/fake-recorder" "$work/bin/omarchy-launch-editor"
# So does the floating-terminal launcher behind a banner's Run in terminal.
install -m 755 "$root/tests/plugin/fake-recorder" "$work/bin/omarchy-launch-floating-terminal-with-presentation"
# The shell's Style.qml asks Hyprland and fontconfig for gaps, rounding and
# the font; outside Hyprland it keeps its defaults when they fail.
printf '#!/bin/sh\nexit 1\n' >"$work/bin/hyprctl"
printf '#!/bin/sh\necho monospace\n' >"$work/bin/fc-match"
chmod 755 "$work/bin/hyprctl" "$work/bin/fc-match"
ln -s "$(command -v sh)" "$work/bin/sh"

pass=0
fail=0

# run <case> <index file> <steps> [VAR=value ...] — one harness run; step reports land in
# $work/<case>.steps (one JSON object per line), the whole log in <case>.log.
# HOME is $work/home unless the case names one; XDG_STATE_HOME and
# XDG_CONFIG_HOME follow it, so nothing reaches the real user's files.
run() {
  local name=$1 index=$2 steps=$3 home="$work/home" arg
  shift 3
  for arg in "$@"; do [[ $arg == HOME=* ]] && home=${arg#HOME=}; done
  mkdir -p "$home"
  env -i HOME="$home" XDG_STATE_HOME="$home/.local/state" XDG_CONFIG_HOME="$home/.config" \
    PATH="$work/bin" QT_QPA_PLATFORM=offscreen \
    XDG_RUNTIME_DIR="${XDG_RUNTIME_DIR:-$work}" \
    HARNESS_PLUGIN_DIR="$plugin" HARNESS_STEPS="$steps" SELDON_INDEX="$index" "$@" \
    "$timeout_bin" 60 "$qs_bin" -p "$config/shell.qml" >"$work/$name.log" 2>&1 || true
  sed 's/\x1b\[[0-9;]*m//g' "$work/$name.log" | grep -a "HARNESS step " | sed 's/.*HARNESS step [^ ]* //' >"$work/$name.steps" || true
}

# expect <case> <step number, 1-based> <jq filter> <value>
expect() {
  local got="<no report>"
  if [[ $(wc -l <"$work/$1.steps") -ge $2 ]]; then
    got=$(sed -n "${2}p" "$work/$1.steps" | jq -r "$3" 2>/dev/null || true)
  fi
  if [[ $got == "$4" ]]; then
    pass=$((pass + 1))
    echo "ok   $1 #$2: $3 = $4"
  else
    fail=$((fail + 1))
    echo "FAIL $1 #$2: $3 = $got (want $4)"
  fi
}

# shows <case> <step> <text> — the text is visible on screen at that step.
# The text goes into a jq string literal: write a `"` in it as `\"`.
shows() {
  expect "$1" "$2" "[.texts[] | select(. == \"$3\")] | length > 0" true
}

# The one-line warnings of the engine calls some cases make the fake engine
# refuse on purpose (WP-068); any other warning still fails the case.
expected_warnings='jax\.seldon: seldon (log exit 1: unknown case C-2026-004$|plan exit 1: C-2026-008 is active; |agent exit 1: C-2026-004 is queued; |(plan|drift|decide) exit 4: the logbook is locked by another seldon \(pid 4242\)$)'

# clean_log <case> [regex] — also allow the warnings the regex matches.
clean_log() {
  local bad
  bad=$(sed 's/\x1b\[[0-9;]*m//g' "$work/$1.log" | grep -a -E "ERROR|WARN|TypeError|ReferenceError|Binding loop" \
    | grep -a -v -E "WAYLAND_DISPLAY is present|QT_QPA_PLATFORM|--- WARNING ---|most functionality will be broken" \
    | grep -a -v -E "$expected_warnings" | grep -a -v -E "${2:-^$}" || true)
  if [[ -z $bad ]]; then
    pass=$((pass + 1))
    echo "ok   $1: log clean"
  else
    fail=$((fail + 1))
    echo "FAIL $1: log has errors"
    echo "$bad" | sed 's/^/     /'
  fi
}

# 1. The sample: every tab renders its data; the strip is on every tab.
run sample "$fx/index.sample.json" \
  "view;tab:changelog;filter:pacman;text:f;filter:all;key:Down;key:Down*32;key:Return;tab:system;key:Down*40"
expect sample 1 .view.status ok
expect sample 1 .view.tab today
expect sample 1 .view.banner ""
# WP-051: the header mark (A5 at the default font) and the day's state (A11)
expect sample 1 .view.mark.file a5-panel-mark-24.svg
expect sample 1 .view.mark.box 24
expect sample 1 .view.mark.ready true
expect sample 1 .view.bannerPictogram ""
expect sample 1 .view.today.state crisis
expect sample 1 .view.crisis "2 changes in the red zone need a reason"
shows sample 1 "2 changes in the red zone need a reason"
expect sample 1 .view.today.entries 4
expect sample 1 .view.today.yesterday 1
shows sample 1 "Thursday, 1 Oct 2026"
shows sample 1 "09:25 · claude-code · C-2026-003"
shows sample 1 "▸ Yesterday · 1 entry"
expect sample 2 .view.tab changelog
expect sample 2 .view.changelog.rows 62
expect sample 2 '.view.changelog.badges | join(",")' "firefox +2"
expect sample 2 .view.changelog.folded 7
expect sample 2 .view.changelog.snapshots 8
expect sample 2 '.view.changelog.driftTones | join(",")' \
  "tokyo-night accent,~/.config/systemd/user/ollama.service urgent,ollama urgent,libinput accent,noto-fonts accent,firefox accent"
shows sample 2 "2 changes in the red zone need a reason"
shows sample 2 "62 events · newest first"
shows sample 2 "explained: Zeiterfassung nur zum Testen, noch nicht in der Bar."
shows sample 2 "Unexplained · proposed for C-2026-005"
expect sample 3 .view.changelog.filter pacman
expect sample 3 .view.changelog.rows 12
shows sample 3 "12 events from pacman · newest first"
expect sample 4 .view.changelog.filter snapper
expect sample 4 .view.changelog.rows 10
expect sample 5 .view.changelog.rows 62
expect sample 6 .view.cursorActive true
expect sample 7 .view.cursor 32
shows sample 7 "firefox"
shows sample 7 "+2"
shows sample 7 "Unexplained"
expect sample 8 .view.drift.open true
expect sample 8 .view.drift.subject firefox
expect sample 8 '.view.drift.members | length' 3
shows sample 8 "3 packages in one transaction:"
shows sample 8 "· upgrade libinput  1.29.1-1 → 1.29.2-1"
expect sample 9 .view.tab system
expect sample 9 '.view.system | join(",")' "OMARCHY,PACKAGES,PLUGINS,SNAPSHOTS,AREAS,COLLECTORS,SELDON"
shows sample 9 "33 of 40 enabled"
shows sample 9 "2026-10-01 16:30 · tailscale: MagicDNS · pre"
shows sample 9 "2 changes in the red zone need a reason"
expect sample 10 .view.cursor 28
clean_log sample

# 2. Keyboard (SPEC-PLUGIN §5): Tab / Shift-Tab only hand over to the
#    neighbouring bar panel (a stand-in bar records the direction); ←/→ and
#    h/l switch tabs and wrap; digits are fixed per tab id (Today 1,
#    Changelog 2, Work 3, Decisions 4, System 5, Memory 6).
run keys "$fx/index.sample.json" \
  "key:Tab;key:Backtab;key:Right;key:Right;key:Right;key:Right;key:Right;key:Right;key:Left;text:l;text:h;text:5;text:4;text:6;text:3;text:1;text:2;key:Down;key:Down*2;text:k;text:j;key:Return;key:Escape" \
  HARNESS_BAR=1
expect keys 1 .view.tab today
expect keys 1 '.switches | join(",")' 1
expect keys 2 .view.tab today
expect keys 2 '.switches | join(",")' "1,-1"
expect keys 3 .view.tab changelog
expect keys 4 .view.tab work
expect keys 5 .view.tab decisions
expect keys 6 .view.tab system
expect keys 7 .view.tab memory
expect keys 8 .view.tab today
expect keys 9 .view.tab memory
expect keys 10 .view.tab today
expect keys 11 .view.tab memory
expect keys 12 .view.tab system
expect keys 13 .view.tab decisions
expect keys 14 .view.tab memory
expect keys 15 .view.tab work
expect keys 16 .view.tab today
expect keys 17 .view.tab changelog
expect keys 17 '.switches | length' 2
expect keys 18 .view.cursorActive true
expect keys 19 .view.cursor 2
expect keys 20 .view.cursor 1
expect keys 21 .view.cursor 2
expect keys 23 .view.opened false
clean_log keys

# 3. The yesterday row opens with Enter and stays in view (the list scrolls
#    to it once the new rows are laid out, one step later).
run yesterday "$fx/index.sample.json" "key:Down;key:Down*10;key:Return;view;key:Down"
expect yesterday 2 .view.cursor 4
expect yesterday 3 .view.today.rows 6
shows yesterday 4 "▾ Yesterday · 1 entry"
expect yesterday 5 .view.cursor 5
shows yesterday 5 "Snapshots aufgeräumt, 108 und 109 gelöscht."
clean_log yesterday

# 4. Snapper without permissions (ADR-0011): its banner on every tab, with
#    Check again (WP-054); after Run in terminal, the hint under the buttons.
run snapper "$fx/index-variants/snapper-degraded.json" "view;tab:changelog;tab:system;click:Run in terminal" \
  HARNESS_RECORD="$work/snapper.record"
for step in 1 2 3 4; do
  expect snapper $step .view.snapper "Snapshots not readable"
done
shows snapper 1 'sudo snapper -c root set-config ALLOW_USERS=$USER SYNC_ACL=yes'
shows snapper 1 "Run in terminal"
shows snapper 1 "Check again"
# the detail: the engine's message, then what the fix grants
snapper_grants="The command below adds your user to ALLOW_USERS of the root snapper config, which also lets your user create, change and delete root snapshots without a password."
expect snapper 1 "[.texts[] | select(endswith(\"\\n$snapper_grants\"))] | length > 0" true
expect snapper 3 '[.texts[] | select(. == "When the command has finished, press Check again")] | length' 0
shows snapper 4 "When the command has finished, press Check again"
shows snapper 3 "failing · snapper: No permissions. The snapper config does not list this user in ALLOW_USERS; see \`seldon doctor\`."
clean_log snapper

# 5. Not initialised: the banner, no strip, empty tabs.
run uninit "$fx/index-variants/not-initialised.json" "view;tab:changelog;tab:system;tab:work;text:+;tab:decisions;text:d;tab:memory"
expect uninit 1 .view.banner "Logbook not initialised"
expect uninit 1 .view.bannerPictogram logbook-not-initialised
expect uninit 1 .view.crisis ""
shows uninit 1 "No index to show"
expect uninit 2 .view.changelog.rows 0
shows uninit 3 "No index to show"
expect uninit 4 '.view.work.columns | join(",")' "queued 0,active 0,completed 0"
expect uninit 4 .view.work.card null
shows uninit 4 "No index to show"
expect uninit 5 .view.work.sheet.open false
expect uninit 6 '.view.decisions.rows | length' 0
shows uninit 6 "No index to show"
expect uninit 7 .view.decisions.sheet.open false
expect uninit 8 '.view.memory.rows | length' 0
shows uninit 8 "No index to show"
clean_log uninit

# 6. Every system field is optional: an empty section and none at all.
jq '.system = {}' "$fx/index.sample.json" >"$work/system-empty.json"
run system-empty "$work/system-empty.json" "tab:system"
expect system-empty 1 '.view.system | join(",")' "COLLECTORS,SELDON"
clean_log system-empty
jq '.system = {packages: {aur: 3}} | del(.state.collectors) | del(.today.yesterday) | .drift = [] | .summary.crisis = 0' \
  "$fx/index.sample.json" >"$work/sparse.json"
run sparse "$work/sparse.json" "view;tab:changelog;tab:system"
expect sparse 1 .view.crisis ""
expect sparse 1 .view.today.yesterday 0
expect sparse 2 '.view.changelog.badges | length' 0
expect sparse 3 '.view.system | join(",")' "PACKAGES,SELDON"
clean_log sparse

# 7. Live (no dev mode): the QuickEntry, Open in editor on every tab and
#    Capture now, against the fake engine. Typing in the field never
#    reaches the panel's keys ("--help" has h and l, which switch tabs
#    otherwise). The case picker is driven by keys (Tab, Down, Return). After
#    "Capture now" the fake engine's status writes an index with one more
#    event; the Changelog shows it through the FileView, without a restart.
jq '.events = [{id: "01M3W2NEWEVENT000000000000", ts: "2026-10-01T18:30:00+02:00", source: "manual", kind: "note",
  subject: "journal", detail: "Written by the harness after Capture now", zone: "green", actor: "human", case: null}] + .events' \
  "$fx/index.sample.json" >"$work/after.json"
mkdir -p "$work/home-live"
run live "" \
  "view;text:n;type:--help;key:Return;settle;type:   ;key:Return;key:Backspace*3;key:Tab;key:Down;key:Down;key:Down;key:Return;key:Backtab;type:for the case;key:Return;settle;key:Escape;text:e;tab:changelog;text:e;text:c;view;wait:changelog.rows=63;tab:system;text:e;settle" \
  HOME="$work/home-live" FAKE_SELDON_FIXTURE="$fx/index.sample.json" FAKE_SELDON_FIXTURE_AFTER="$work/after.json" \
  FAKE_SELDON_WRITTEN=1 HARNESS_RECORD="$work/live.record"
expect live 1 .view.status ok
expect live 1 .view.today.quickEntry.enabled true
expect live 1 .view.today.quickEntry.cases 6
expect live 1 .view.today.quickEntry.editing false
shows live 1 "Note for today's journal, Enter saves"
shows live 1 "No case"
expect live 2 .view.today.quickEntry.editing true
expect live 3 .view.today.quickEntry.text "--help"
expect live 3 .view.tab today
expect live 5 .view.today.quickEntry.result "Saved to the journal · 01M3W1FAKE0000000000000NTE"
expect live 5 .view.today.quickEntry.text ""
shows live 5 "Saved to the journal · 01M3W1FAKE0000000000000NTE"
expect live 7 .view.today.quickEntry.result "Write something first"
expect live 7 .view.today.quickEntry.text "   "
expect live 8 .view.today.quickEntry.text ""
expect live 13 .view.today.quickEntry.caseId C-2026-004
shows live 13 "C-2026-004 · Zed als zweiten Editor installieren"
shows live 13 "Open case"
expect live 17 .view.today.quickEntry.result "Saved to C-2026-004 · 01M3W1FAKE0000000000000NTE"
expect live 18 .view.today.quickEntry.editing false
expect live 18 .view.opened true
expect live 19 .view.tab today
expect live 20 .view.tab changelog
expect live 22 .view.capturing true
shows live 22 "Capturing"
expect live 24 .view.changelog.rows 63
shows live 24 "Written by the harness after Capture now · human"
expect live 24 .view.captureResult "1 new event"
shows live 24 "Last capture: 1 new event"
expect live 25 .view.tab system
expect live 27 .view.openResult "Opened $work/home-live/Seldon/STATUS.md in omarchy-launch-editor"
expect live 27 .view.lastError ""
q() { printf '%q ' "$@"; }
want=$(printf '%s\n' "$(q --version --json)" "$(q capture --all --json --quiet)" "$(q status --json)" \
  "$(q log --json -- --help)" "$(q log --case C-2026-004 --json -- "for the case")" \
  "$(q open journal --editor --json)" "$(q open ledger --editor --json)" \
  "$(q capture --all --json --quiet)" "$(q status --json)" "$(q open status --editor --json)")
got=$(cat "$work/home-live/argv.log" 2>/dev/null || true)
if [[ $got == "$want" ]]; then
  pass=$((pass + 1)); echo "ok   live: engine argv"
else
  fail=$((fail + 1)); echo "FAIL live: engine argv differs"; diff <(echo "$want") <(echo "$got") | sed 's/^/     /'
fi
want=$(printf '%s\n' omarchy-launch-editor "$work/home-live/Seldon/journal/2026/2026-10-01.md" -- \
  omarchy-launch-editor "$work/home-live/Seldon/ledger/2026-10.jsonl" -- \
  omarchy-launch-editor "$work/home-live/Seldon/STATUS.md" --)
got=$(cat "$work/live.record" 2>/dev/null || true)
if [[ $got == "$want" ]]; then
  pass=$((pass + 1)); echo "ok   live: editor paths"
else
  fail=$((fail + 1)); echo "FAIL live: editor launches differ"; diff <(echo "$want") <(echo "$got") | sed 's/^/     /'
fi
clean_log live

# 8. The engine refuses a note (the fake does not know C-2026-004 here): the
#    result line shows its message and the text stays in the field.
mkdir -p "$work/home-refuse"
run refuse "" "text:n;type:keep this;key:Tab;key:Down;key:Down;key:Down;key:Return;key:Backtab;key:Return;settle" \
  HOME="$work/home-refuse" FAKE_SELDON_FIXTURE="$fx/index.sample.json" FAKE_SELDON_UNKNOWN_CASE=C-2026-004
expect refuse 7 .view.today.quickEntry.caseId C-2026-004
expect refuse 10 .view.today.quickEntry.result "unknown case C-2026-004"
expect refuse 10 .view.today.quickEntry.text "keep this"
expect refuse 10 .view.today.quickEntry.editing true
shows refuse 10 "unknown case C-2026-004"
expect refuse 10 .view.lastError ""
clean_log refuse

# 9. Work tab on the sample (dev mode, read-only): three columns, the WIP
#    text, the proposedEvents badge on C-2026-005, the card of the case under
#    the cursor with its actions by status. The cursor walks the columns in
#    order; nothing can be armed or run without an engine to write.
run work "$fx/index.sample.json" "text:3;view;key:Down;key:Down*3;key:Down*2;key:Return;text:x;key:Down*5;key:Up*9;key:Down*3;text:a;click:Start agent"
expect work 1 .view.tab work
expect work 1 '.view.work.columns | join(",")' "queued 3,active 3,completed 2"
expect work 1 '.view.work.ids | join(" | ")' \
  "C-2026-005,C-2026-006,C-2026-007 | C-2026-003,C-2026-004,C-2026-008 | C-2026-002,C-2026-001"
expect work 1 .view.work.wip "2 / 3 active"
expect work 1 .view.work.cursor C-2026-005
expect work 1 '.view.work.card.actions | join(",")' "Start,Open"
expect work 1 .view.work.card.proposed 1
for text in "QUEUED 3" "ACTIVE 3" "COMPLETED 2" "2 / 3 active" "1 proposed" "1 proposed event" \
  "Theme-Wechsel auf Tokyo Night durchziehen (Zed, Neovim)" "C-2026-005 · queued" "yellow · R1" \
  "themes · priority normal · 0/4 steps" "created 2026-09-29" "Dev mode is read-only" "New case" "verification" \
  "Tailscale für Fernzugriff einrichten" "Seldon-Logbuch anlegen" "4/5" "Start" "Open"; do
  shows work 1 "$text"
done
expect work 1 '[.texts[] | select(. == "1 proposed")] | length' 1
expect work 3 .view.cursorActive true
expect work 3 .view.work.cursor C-2026-005
expect work 4 .view.work.cursor C-2026-003
expect work 4 '.view.work.card.actions | join(",")' "Verify,Start agent,Drop,Open"
shows work 4 "agent: claude-code"
shows work 4 "red · R2"
shows work 4 "shell · priority high · 4/5 steps"
shows work 4 "created 2026-09-26 · started 2026-10-01"
expect work 5 .view.work.cursor C-2026-008
expect work 5 .view.work.card.status verification
expect work 5 '.view.work.card.actions | join(",")' "Done,Drop,Open"
expect work 6 .view.work.card.armed ""
expect work 6 .view.work.result ""
expect work 7 .view.work.card.armed ""
expect work 8 .view.work.cursor C-2026-001
expect work 8 '.view.work.card.actions | join(",")' "Open"
shows work 8 "created 2026-09-01 · started 2026-09-01 · closed 2026-09-01"
expect work 9 .view.work.cursor C-2026-005
# Start agent (WP-022) is gated by canWrite: neither key a nor a click arms it
expect work 10 .view.work.cursor C-2026-003
expect work 11 .view.work.card.armed ""
expect work 11 .view.work.card.hint "Dev mode is read-only"
expect work 12 .view.work.card.armed ""
expect work 12 .view.work.result ""
clean_log work

# 10. Work tab live, against the fake engine: a new case through the sheet
#     with real keys (Tab walks the pickers, an invalid area is refused in
#     the plugin), then start → verify → done on C-2026-005 with Enter twice
#     each, the case moving columns through the FileView and the cursor
#     following it; Open on the completed case; Done on C-2026-008, which
#     the index shows in verification but the fake engine's logbook has
#     active: the engine refuses and its message is the result line; x twice
#     drops C-2026-004; `e` opens it. Each engine step settles (its
#     result line is the engine's answer, not "Completing …") before the
#     wait for the index to move the card (WP-076: the result line came
#     after the card under load).
mkdir -p "$work/home-work"
echo "C-2026-008 active" >"$work/home-work/cases"
run work-live "" \
  "text:3;text:+;type:--help;key:Tab;key:Right;key:Return;key:Tab;key:Right;key:Return;key:Tab;key:Left;key:Return;key:Tab;type:Dev;key:Return;key:Backspace*3;type:dev-env;key:Return;settle;wait:work.cursor=C-2026-009;key:Up;key:Up*3;key:Return;key:Return;settle;wait:work.card.status=active;key:Return;key:Return;settle;wait:work.card.status=verification;key:Return;key:Return;settle;wait:work.card.status=completed;key:Return;settle;key:Up;key:Return;key:Down;key:Up;key:Return;key:Return;settle;key:Up;text:x;text:x;settle;wait:work.card.status=dropped;text:e;settle" \
  HOME="$work/home-work" FAKE_SELDON_FIXTURE="$fx/index.sample.json" HARNESS_RECORD="$work/work-live.record"
expect work-live 1 .view.tab work
expect work-live 1 .view.work.card.hint ""
expect work-live 2 .view.work.sheet.open true
expect work-live 2 .view.work.sheet.editing true
expect work-live 2 .view.work.sheet.zone yellow
expect work-live 2 .view.work.sheet.risk R1
expect work-live 2 .view.work.sheet.priority normal
shows work-live 2 "NEW CASE"
shows work-live 2 "Title, Enter creates the case"
expect work-live 3 .view.work.sheet.title "--help"
expect work-live 3 .view.tab work
expect work-live 6 .view.work.sheet.zone red
expect work-live 9 .view.work.sheet.risk R2
expect work-live 12 .view.work.sheet.priority high
expect work-live 14 .view.work.sheet.area Dev
shows work-live 14 "Area: lowercase letters, digits and -, starting with a letter or digit"
expect work-live 15 .view.work.sheet.result "Area must be a lowercase slug: letters, digits and -"
expect work-live 15 .view.work.sheet.open true
expect work-live 15 .view.work.sheet.title "--help"
expect work-live 17 .view.work.sheet.area dev-env
expect work-live 20 .view.work.sheet.open false
expect work-live 20 .view.work.sheet.editing false
expect work-live 20 .view.work.result "Created C-2026-009 · --help · new area dev-env"
expect work-live 20 '.view.work.columns | join(",")' "queued 4,active 3,completed 2"
expect work-live 20 .view.work.card.id C-2026-009
expect work-live 20 .view.work.sheet.title ""
expect work-live 20 .view.work.sheet.zone yellow
shows work-live 20 "red · R2"
shows work-live 20 "dev-env · priority high · 0/0 steps"
expect work-live 22 .view.work.cursor C-2026-005
expect work-live 23 .view.work.card.armed start
expect work-live 23 .view.work.card.hint "Press Enter again: Start C-2026-005"
shows work-live 23 "Press Enter again: Start C-2026-005"
expect work-live 26 .view.work.cursor C-2026-005
expect work-live 26 '.view.work.columns | join(",")' "queued 3,active 4,completed 2"
expect work-live 26 .view.work.result "C-2026-005: queued → active"
expect work-live 26 .view.work.wip "3 / 3 active"
shows work-live 26 "3 / 3 active · at the limit"
expect work-live 26 '.view.work.card.actions | join(",")' "Verify,Start agent,Drop,Open"
expect work-live 30 .view.work.result "C-2026-005: active → verification"
expect work-live 34 '.view.work.columns | join(",")' "queued 3,active 3,completed 3"
expect work-live 34 .view.work.result "C-2026-005: verification → completed · journal journal/2026/2026-10-01.md"
expect work-live 34 '.view.work.card.actions | join(",")' "Open"
expect work-live 36 .view.openResult "Opened $work/home-work/Seldon/work/active/C-2026-005.md in omarchy-launch-editor"
expect work-live 37 .view.work.cursor C-2026-008
expect work-live 38 .view.work.card.armed done
expect work-live 39 .view.work.card.armed ""
expect work-live 40 .view.work.cursor C-2026-008
expect work-live 40 .view.work.card.armed ""
refusal='C-2026-008 is active; `seldon plan done` needs a case that is verification; run `seldon plan verify` first'
expect work-live 43 .view.work.result "$refusal"
expect work-live 43 .view.work.resultOk false
shows work-live 43 "$refusal"
expect work-live 43 '.view.work.columns | join(",")' "queued 3,active 3,completed 3"
expect work-live 43 .view.work.cursor C-2026-008
expect work-live 43 .view.lastError ""
expect work-live 44 .view.work.cursor C-2026-004
expect work-live 45 .view.work.card.armed drop
shows work-live 45 "Confirm drop"
shows work-live 45 "Drop C-2026-004? Press x again or click Confirm drop. This is final."
expect work-live 48 .view.work.result "C-2026-004: active → dropped"
expect work-live 48 '.view.work.columns | join(",")' "queued 3,active 2,completed 4"
expect work-live 48 .view.work.wip "1 / 3 active"
expect work-live 48 '.view.work.card.actions | join(",")' "Open"
shows work-live 48 "dropped"
expect work-live 50 .view.openResult "Opened $work/home-work/Seldon/work/active/C-2026-004.md in omarchy-launch-editor"
q() { printf '%q ' "$@"; }
want=$(printf '%s\n' "$(q --version --json)" "$(q capture --all --json --quiet)" "$(q status --json)" \
  "$(q plan new --zone red --risk R2 --area dev-env --priority high --json -- --help)" \
  "$(q plan start C-2026-005 --json)" "$(q plan verify C-2026-005 --json)" "$(q plan done C-2026-005 --json)" \
  "$(q open C-2026-005 --editor --json)" "$(q plan done C-2026-008 --json)" "$(q plan drop C-2026-004 --json)" \
  "$(q open C-2026-004 --editor --json)")
got=$(cat "$work/home-work/argv.log" 2>/dev/null || true)
if [[ $got == "$want" ]]; then
  pass=$((pass + 1)); echo "ok   work-live: engine argv"
else
  fail=$((fail + 1)); echo "FAIL work-live: engine argv differs"; diff <(echo "$want") <(echo "$got") | sed 's/^/     /'
fi
clean_log work-live

# 10b. Start agent (WP-022), live against the fake engine: key a on a
#     queued case does nothing; on the active C-2026-003 the first a arms
#     (hint, "Confirm start agent"), Enter re-arms the first action (Verify)
#     instead, a and a again runs `agent start C-2026-003 --json` and the
#     result line names the launcher. With the mouse on C-2026-004: a click
#     arms, a second click (Confirm) runs; the fake engine's logbook has
#     C-2026-004 queued, so the engine's refusal (with its `seldon plan
#     start` hint) is the result line and never the banner.
mkdir -p "$work/home-agent"
echo "C-2026-004 queued" >"$work/home-agent/cases"
run work-agent "" \
  "text:3;key:Down;text:a;key:Down*3;text:a;key:Return;text:a;text:a;settle;key:Down;click:Start agent;click:Confirm start agent;settle" \
  HOME="$work/home-agent" FAKE_SELDON_FIXTURE="$fx/index.sample.json"
expect work-agent 2 .view.work.cursor C-2026-005
expect work-agent 3 .view.work.card.armed ""
expect work-agent 4 .view.work.cursor C-2026-003
shows work-agent 4 "agent: claude-code"
shows work-agent 4 "Start agent"
expect work-agent 5 .view.work.card.armed agent
expect work-agent 5 .view.work.card.hint "Start agent on C-2026-003? Press a again or click Confirm start agent."
shows work-agent 5 "Confirm start agent"
expect work-agent 6 .view.work.card.armed verify
expect work-agent 7 .view.work.card.armed agent
expect work-agent 9 .view.work.result "Agent started on C-2026-003 · launcher default (omarchy)"
expect work-agent 9 .view.work.resultOk true
expect work-agent 9 .view.work.card.armed ""
shows work-agent 9 "Agent started on C-2026-003 · launcher default (omarchy)"
expect work-agent 10 .view.work.cursor C-2026-004
expect work-agent 11 .view.work.card.armed agent
expect work-agent 11 .view.work.result "Agent started on C-2026-003 · launcher default (omarchy)"
refusal='C-2026-004 is queued; start it first: `seldon plan start C-2026-004`'
expect work-agent 13 .view.work.result "$refusal"
expect work-agent 13 .view.work.resultOk false
expect work-agent 13 .view.lastError ""
shows work-agent 13 "$refusal"
want=$(printf '%s\n' "$(q --version --json)" "$(q capture --all --json --quiet)" "$(q status --json)" \
  "$(q agent start C-2026-003 --json)" "$(q agent start C-2026-004 --json)")
got=$(cat "$work/home-agent/argv.log" 2>/dev/null || true)
if [[ $got == "$want" ]]; then
  pass=$((pass + 1)); echo "ok   work-agent: engine argv"
else
  fail=$((fail + 1)); echo "FAIL work-agent: engine argv differs"; diff <(echo "$want") <(echo "$got") | sed 's/^/     /'
fi
clean_log work-agent

# 11. The engine refuses the new case (lock held, exit 4): the sheet shows
#     its message and keeps the title; Esc gives the keys back and keeps it;
#     `+` brings the sheet back with it.
mkdir -p "$work/home-work-locked"
run work-locked "" "text:+;type:keep me;key:Return;settle;key:Escape;view;text:+" \
  HOME="$work/home-work-locked" FAKE_SELDON_FIXTURE="$fx/index.sample.json" FAKE_SELDON_LOCKED=1
expect work-locked 1 .view.tab work
expect work-locked 1 .view.work.sheet.editing true
expect work-locked 4 .view.work.sheet.result "the logbook is locked by another seldon (pid 4242)"
expect work-locked 4 .view.work.sheet.title "keep me"
expect work-locked 4 .view.work.sheet.open true
shows work-locked 4 "the logbook is locked by another seldon (pid 4242)"
expect work-locked 4 .view.lastError ""
expect work-locked 5 .view.work.sheet.open false
expect work-locked 5 .view.opened true
expect work-locked 6 '.view.work.columns | join(",")' "queued 3,active 3,completed 2"
expect work-locked 7 .view.work.sheet.open true
expect work-locked 7 .view.work.sheet.title "keep me"
clean_log work-locked

# 12. The drift sheet on the sample (dev mode, read-only; WP-021): Enter on an
#     open drift row and the IPC route open it for each of the four items
#     with the right defaults (Link with the proposed case for the theme
#     item, Explain for the crises and the group), a group lists its three
#     members and offers "Only <the row's package>", a member row opens the
#     group named by that member, and a click on the red strip opens the
#     first crisis with the cursor on its row. Nothing can be sent.
THEME=01M3VTGNY0NZG4AY80814WSKGR UNIT=01M3VNJ9JGZ9169T01XCW16FT0 OLLAMA=01M3VNFTF8EVHWFFZ687N14Q0C
FIREFOX=01M3SXBQVR7AW8PJQC1YXDCQ14 NOTO=01M3SXBRV0E702XKBM22HEV1B8
run drift-sample "$fx/index.sample.json" \
  "tab:changelog;key:Down;key:Down*4;key:Return;key:Return;key:Escape;resolve:$UNIT;key:Escape;resolve:$OLLAMA;key:Escape;resolve:$FIREFOX;key:Escape;resolve:$NOTO;key:Escape;click:2 changes in the red zone need a reason"
expect drift-sample 3 .view.cursor 4
shows drift-sample 3 "Resolve…"
expect drift-sample 4 .view.drift.open true
expect drift-sample 4 .view.drift.editing true
expect drift-sample 4 .view.drift.eventId $THEME
expect drift-sample 4 .view.drift.subject tokyo-night
expect drift-sample 4 .view.drift.action link
expect drift-sample 4 .view.drift.caseId C-2026-005
expect drift-sample 4 '.view.drift.cases | join(",")' "C-2026-005,C-2026-003,C-2026-004,C-2026-008,C-2026-006,C-2026-007"
expect drift-sample 4 .view.drift.hint "Dev mode is read-only"
for text in "RESOLVE DRIFT" "proposed for C-2026-005" "kanagawa → tokyo-night · human · 2026-10-01 15:30" \
  "C-2026-005 · Theme-Wechsel auf Tokyo Night durchziehen (Zed, Neovim) · proposed" "Link" "Explain" "Dismiss" "Cancel"; do
  shows drift-sample 4 "$text"
done
expect drift-sample 5 .view.drift.armed false
expect drift-sample 5 .view.drift.result ""
expect drift-sample 6 .view.drift.open false
expect drift-sample 6 .view.drift.editing false
expect drift-sample 6 .view.cursor 4
expect drift-sample 7 .view.drift.subject "~/.config/systemd/user/ollama.service"
expect drift-sample 7 .view.drift.action explain
expect drift-sample 7 .view.drift.zone red
expect drift-sample 7 .view.drift.explainZone red
expect drift-sample 7 .view.drift.risk R1
expect drift-sample 7 '.view.drift.cases[0]' ""
shows drift-sample 7 "RESOLVE A RED-ZONE CHANGE"
shows drift-sample 7 "red · crisis"
expect drift-sample 9 .view.drift.subject ollama
expect drift-sample 9 .view.drift.action explain
expect drift-sample 11 .view.drift.subject firefox
expect drift-sample 11 .view.drift.badge +2
expect drift-sample 11 .view.drift.zone yellow
expect drift-sample 11 '.view.drift.members | join(" | ")' \
  "· upgrade firefox  143.0.1-1 → 143.0.2-1 | · upgrade noto-fonts  1:2026.09.01-1 → 1:2026.09.15-1 | · upgrade libinput  1.29.1-1 → 1.29.2-1"
shows drift-sample 11 "All 3"
shows drift-sample 11 "Only firefox"
expect drift-sample 13 .view.drift.eventId $NOTO
expect drift-sample 13 .view.drift.subject firefox
shows drift-sample 13 "Only noto-fonts"
expect drift-sample 15 .view.drift.open true
expect drift-sample 15 .view.drift.eventId $UNIT
expect drift-sample 15 .view.cursor 7
expect drift-sample 15 .view.tab changelog
clean_log drift-sample

# 13. ADR-0020: the index lists fewer drift items than the summary counts.
jq '.summary.openDrift = 250' "$fx/index.sample.json" >"$work/capped.json"
run drift-capped "$work/capped.json" "tab:changelog"
expect drift-capped 1 .view.changelog.more "+246 more open drift items not listed here"
shows drift-capped 1 "+246 more open drift items not listed here"
expect drift-capped 1 .view.pill "2 · 250"
clean_log drift-capped

# 14. The drift sheet live, against the fake engine, with real keys: Enter on
#     the theme row, Enter twice links it to the preselected C-2026-005; a
#     click on the red strip opens the first crisis, which is explained with
#     the text `--help`, risk R2 (a change disarms) and area dev-env; Open
#     case opens the new C-2026-009, which the Work tab shows as completed;
#     the firefox group is dismissed as one: three rows fold. The pill and
#     the strip follow every index.
mkdir -p "$work/home-drift"
run drift-live "" \
  "tab:changelog;key:Down;key:Down*4;key:Return;key:Return;key:Return;wait:drift.isOpen=false;key:Escape;click:2 changes in the red zone need a reason;type:--help;key:Return;key:Tab;key:Tab;key:Right;key:Return;key:Tab;type:dev-env;key:Return;key:Return;wait:drift.isOpen=false;key:Return;settle;key:Escape;text:3;text:2;key:Down*25;key:Return;key:Backtab;key:Backtab;key:Right;key:Return;key:Tab;key:Tab;type:routine update;key:Return;key:Return;wait:drift.isOpen=false;key:Escape" \
  HOME="$work/home-drift" FAKE_SELDON_FIXTURE="$fx/index.sample.json" HARNESS_RECORD="$work/drift-live.record"
expect drift-live 4 .view.drift.caseId C-2026-005
expect drift-live 4 .view.drift.editing true
expect drift-live 5 .view.drift.armed true
expect drift-live 5 .view.drift.hint "Press Enter again: Link tokyo-night to C-2026-005"
shows drift-live 5 "Press Enter again: Link tokyo-night to C-2026-005"
expect drift-live 7 .view.drift.isOpen false
expect drift-live 7 .view.drift.result "Linked 1 event to C-2026-005"
expect drift-live 7 .view.drift.resolution "linked to C-2026-005"
expect drift-live 7 .view.drift.openCase C-2026-005
shows drift-live 7 "Resolved: linked to C-2026-005"
shows drift-live 7 "Open C-2026-005"
expect drift-live 7 .view.pill "2 · 3"
expect drift-live 7 .view.crisis "2 changes in the red zone need a reason"
expect drift-live 7 '.view.changelog.resolved | map(select(startswith("tokyo-night"))) | join(",")' "tokyo-night: linked to C-2026-005"
expect drift-live 8 .view.drift.open false
expect drift-live 9 .view.drift.eventId $UNIT
expect drift-live 9 .view.drift.action explain
expect drift-live 9 .view.cursor 7
expect drift-live 10 .view.drift.intent "--help"
expect drift-live 10 .view.tab changelog
expect drift-live 11 .view.drift.armed true
expect drift-live 12 .view.drift.armed true
expect drift-live 15 .view.drift.risk R2
expect drift-live 15 .view.drift.armed false
expect drift-live 17 .view.drift.area dev-env
expect drift-live 18 .view.drift.hint "Press Enter again: Explain ~/.config/systemd/user/ollama.service as a new completed case"
expect drift-live 20 .view.drift.result "Explained 1 event · created C-2026-009 · new area dev-env"
expect drift-live 20 .view.drift.resolution "explained · C-2026-009: --help"
expect drift-live 20 .view.crisis "1 change in the red zone needs a reason"
expect drift-live 20 .view.pill "2 · 2"
shows drift-live 20 "Open C-2026-009"
expect drift-live 22 .view.openResult "Opened $work/home-drift/Seldon/work/active/C-2026-009.md in omarchy-launch-editor"
expect drift-live 23 .view.drift.open false
expect drift-live 24 .view.tab work
expect drift-live 24 '.view.work.columns | join(",")' "queued 3,active 3,completed 3"
expect drift-live 24 '.view.work.ids[2]' "C-2026-009,C-2026-002,C-2026-001"
expect drift-live 26 .view.cursor 32
expect drift-live 27 .view.drift.subject firefox
expect drift-live 31 .view.drift.action dismiss
expect drift-live 34 .view.drift.reason "routine update"
expect drift-live 35 .view.drift.hint "Press Enter again: Dismiss firefox and 2 more"
expect drift-live 37 .view.drift.result "Dismissed 3 events"
expect drift-live 37 '.view.changelog.resolved | map(select(endswith("dismissed: routine update"))) | join(",")' \
  "libinput: dismissed: routine update,noto-fonts: dismissed: routine update,firefox: dismissed: routine update"
expect drift-live 37 '.view.changelog.driftTones | join(",")' "ollama urgent"
expect drift-live 37 '.view.changelog.badges | length' 0
expect drift-live 37 .view.pill "2 · 1"
expect drift-live 38 .view.drift.open false
expect drift-live 38 .view.opened true
expect drift-live 38 .view.lastError ""
q() { printf '%q ' "$@"; }
want=$(printf '%s\n' "$(q --version --json)" "$(q capture --all --json --quiet)" "$(q status --json)" \
  "$(q drift link $THEME C-2026-005 --json)" "$(q drift explain $UNIT --risk R2 --area dev-env --json -- --help)" \
  "$(q open C-2026-009 --editor --json)" "$(q drift dismiss $FIREFOX --json -- "routine update")")
got=$(cat "$work/home-drift/argv.log" 2>/dev/null || true)
if [[ $got == "$want" ]]; then
  pass=$((pass + 1)); echo "ok   drift-live: engine argv"
else
  fail=$((fail + 1)); echo "FAIL drift-live: engine argv differs"; diff <(echo "$want") <(echo "$got") | sed 's/^/     /'
fi
clean_log drift-live

# 15. `--only` (ADR-0013 §4) and a refusal in the plugin: Link without a
#     case is refused before the engine is asked; then C-2026-004 picked in
#     the case picker by keys and "Only firefox": the leader is linked alone
#     and the rest comes back as a smaller group led by noto-fonts ("+1"),
#     whose sheet lists two members.
mkdir -p "$work/home-drift-only"
run drift-only "" \
  "tab:changelog;key:Down;key:Down*32;key:Return;key:Backtab;key:Backtab;key:Left;key:Return;key:Tab;key:Tab;key:Tab;key:Return;key:Backtab;key:Backtab;key:Return;key:Down;key:Down;key:Return;key:Tab;key:Right;key:Return;key:Tab;key:Return;key:Return;wait:drift.isOpen=false;key:Escape;key:Up;key:Return" \
  HOME="$work/home-drift-only" FAKE_SELDON_FIXTURE="$fx/index.sample.json"
expect drift-only 3 .view.cursor 32
expect drift-only 8 .view.drift.action link
expect drift-only 8 .view.drift.caseId ""
expect drift-only 12 .view.drift.result "Pick a case first"
expect drift-only 12 .view.drift.armed false
shows drift-only 12 "Pick a case first"
expect drift-only 18 .view.drift.caseId C-2026-004
expect drift-only 18 .view.drift.result ""
expect drift-only 21 .view.drift.only true
shows drift-only 21 "Only firefox"
expect drift-only 23 .view.drift.hint "Press Enter again: Link firefox only to C-2026-004"
expect drift-only 25 .view.drift.result "Linked 1 event to C-2026-004"
expect drift-only 25 .view.drift.resolution "linked to C-2026-004"
expect drift-only 25 '.view.changelog.badges | join(",")' "noto-fonts +1"
expect drift-only 25 .view.pill "2 · 4"
expect drift-only 27 .view.cursor 31
expect drift-only 28 .view.drift.eventId $NOTO
expect drift-only 28 .view.drift.badge +1
expect drift-only 28 '.view.drift.members | length' 2
shows drift-only 28 "2 packages in one transaction:"
want=$(printf '%s\n' "$(q --version --json)" "$(q capture --all --json --quiet)" "$(q status --json)" \
  "$(q drift link $FIREFOX C-2026-004 --only --json)")
got=$(cat "$work/home-drift-only/argv.log" 2>/dev/null || true)
if [[ $got == "$want" ]]; then
  pass=$((pass + 1)); echo "ok   drift-only: engine argv"
else
  fail=$((fail + 1)); echo "FAIL drift-only: engine argv differs"; diff <(echo "$want") <(echo "$got") | sed 's/^/     /'
fi
clean_log drift-only

# 16. A re-run on an item the logbook has resolved but the index still shows
#     open ($HOME/resolved): the engine writes nothing and the sheet says
#     "Already resolved"; the index, the pill and the item stay as they were.
mkdir -p "$work/home-drift-already"
echo "$THEME linked C-2026-005" >"$work/home-drift-already/resolved"
run drift-already "" "resolve:$THEME;key:Return;key:Return;settle" \
  HOME="$work/home-drift-already" FAKE_SELDON_FIXTURE="$fx/index.sample.json"
expect drift-already 4 .view.drift.result "Already resolved: linked to C-2026-005"
expect drift-already 4 .view.drift.already true
expect drift-already 4 .view.drift.resultOk true
expect drift-already 4 .view.drift.isOpen true
expect drift-already 4 .view.pill "2 · 4"
shows drift-already 4 "Already resolved: linked to C-2026-005"
clean_log drift-already

# 17. The engine refuses (lock held, exit 4): the sheet shows its message and
#     keeps the text; Esc closes the sheet, and the draft is back when the
#     sheet opens again; another item gets its own defaults.
mkdir -p "$work/home-drift-locked"
run drift-locked "" "resolve:$UNIT;type:keep this;key:Return;key:Return;settle;key:Escape;resolve:$THEME;key:Escape;resolve:$UNIT" \
  HOME="$work/home-drift-locked" FAKE_SELDON_FIXTURE="$fx/index.sample.json" FAKE_SELDON_LOCKED=1
expect drift-locked 5 .view.drift.result "the logbook is locked by another seldon (pid 4242)"
expect drift-locked 5 .view.drift.resultOk false
expect drift-locked 5 .view.drift.intent "keep this"
expect drift-locked 5 .view.drift.isOpen true
expect drift-locked 5 .view.lastError ""
shows drift-locked 5 "the logbook is locked by another seldon (pid 4242)"
expect drift-locked 6 .view.drift.open false
expect drift-locked 7 .view.drift.intent ""
expect drift-locked 7 .view.drift.action link
expect drift-locked 9 .view.drift.intent "keep this"
expect drift-locked 9 .view.drift.action explain
clean_log drift-locked

# 18. A group whose members index.events no longer all lists (CONTRACT.md
#     rule 4): the sheet shows what the index has, asks `seldon drift show`
#     and then lists all three.
jq '.events |= map(select(.id != "01M3SXBRV0E702XKBM22HEV1B8"))' "$fx/index.sample.json" >"$work/members-capped.json"
mkdir -p "$work/home-drift-show"
jq '[.events[] | select(.id == "01M3SXBRV0E702XKBM22HEV1B8")]' "$fx/index.sample.json" >"$work/home-drift-show/extra-events.json"
run drift-show "" "resolve:$FIREFOX;wait:drift.members.1=· upgrade noto-fonts  1:2026.09.01-1 → 1:2026.09.15-1" \
  HOME="$work/home-drift-show" FAKE_SELDON_FIXTURE="$work/members-capped.json"
expect drift-show 1 '.view.drift.members | join(" | ")' "· upgrade firefox  143.0.1-1 → 143.0.2-1 | · upgrade libinput  1.29.1-1 → 1.29.2-1 | … and 1 more"
expect drift-show 2 '.view.drift.members | length' 3
expect drift-show 2 '.view.drift.members[2]' "· upgrade libinput  1.29.1-1 → 1.29.2-1"
want=$(printf '%s\n' "$(q --version --json)" "$(q capture --all --json --quiet)" "$(q status --json)" \
  "$(q drift show $FIREFOX --json)")
got=$(cat "$work/home-drift-show/argv.log" 2>/dev/null || true)
if [[ $got == "$want" ]]; then
  pass=$((pass + 1)); echo "ok   drift-show: engine argv"
else
  fail=$((fail + 1)); echo "FAIL drift-show: engine argv differs"; diff <(echo "$want") <(echo "$got") | sed 's/^/     /'
fi
clean_log drift-show

# 19. The same from a member row (Enter on libinput): `drift show` is asked
#     for the group's leader, and its answer fills the sheet opened from the
#     member.
LIBINPUT=01M3SXBRV0WPNQ721VWGG2WXZ1
mkdir -p "$work/home-drift-show-member"
cp "$work/home-drift-show/extra-events.json" "$work/home-drift-show-member/"
run drift-show-member "" \
  "tab:changelog;key:Down;key:Down*30;key:Return;wait:drift.members.1=· upgrade noto-fonts  1:2026.09.01-1 → 1:2026.09.15-1" \
  HOME="$work/home-drift-show-member" FAKE_SELDON_FIXTURE="$work/members-capped.json"
expect drift-show-member 3 .view.cursor 30
expect drift-show-member 4 .view.drift.eventId $LIBINPUT
expect drift-show-member 4 .view.drift.subject firefox
shows drift-show-member 4 "Only libinput"
expect drift-show-member 5 '.view.drift.members | join(" | ")' \
  "· upgrade firefox  143.0.1-1 → 143.0.2-1 | · upgrade noto-fonts  1:2026.09.01-1 → 1:2026.09.15-1 | · upgrade libinput  1.29.1-1 → 1.29.2-1"
want=$(printf '%s\n' "$(q --version --json)" "$(q capture --all --json --quiet)" "$(q status --json)" \
  "$(q drift show $FIREFOX --json)")
got=$(cat "$work/home-drift-show-member/argv.log" 2>/dev/null || true)
if [[ $got == "$want" ]]; then
  pass=$((pass + 1)); echo "ok   drift-show-member: engine argv"
else
  fail=$((fail + 1)); echo "FAIL drift-show-member: engine argv differs"; diff <(echo "$want") <(echo "$got") | sed 's/^/     /'
fi
clean_log drift-show-member

# 20. Decisions and Memory on the sample (dev mode, read-only; WP-023): `4`
#     shows the four decisions newest first with id, status, title, date and
#     file, the proposed one first; ↑/↓ and a click on a title move the
#     cursor; Enter and `e` try to open the decision and are refused with
#     dev mode's reason; `d` opens no sheet without an engine to write. `6`
#     shows three lessons and two topics with path and date (the cursor
#     stays active across tabs, so ↓ moves at once).
run decisions "$fx/index.sample.json" \
  "text:4;key:Down;key:Down*2;key:Down*5;key:Up;click:Zed statt VS Code als Zweiteditor;key:Return;text:e;text:d;text:6;key:Down;key:Down*2;key:Down*9;key:Return"
expect decisions 1 .view.tab decisions
expect decisions 1 '.view.decisions.rows | join(",")' "ADR-0004 proposed,ADR-0003 accepted,ADR-0002 accepted,ADR-0001 accepted"
expect decisions 1 .view.decisions.cursor ADR-0004
for text in "4 decisions · 1 proposed" "New decision" "ADR-0004" "proposed" "Ollama nur als User-Service mit Case" \
  "2026-10-01 · decisions/ADR-0004-ollama-user-service.md" "ADR-0001" "accepted" "Logbuch-Sprache Deutsch, Struktur Englisch" \
  "2026-09-01 · decisions/ADR-0001-language.md" "2 changes in the red zone need a reason"; do
  shows decisions 1 "$text"
done
expect decisions 2 .view.cursorActive true
expect decisions 2 .view.decisions.cursor ADR-0004
expect decisions 3 .view.decisions.cursor ADR-0002
shows decisions 3 "Open"
expect decisions 4 .view.decisions.cursor ADR-0001
expect decisions 5 .view.decisions.cursor ADR-0002
expect decisions 6 .view.decisions.cursor ADR-0003
expect decisions 7 .view.openResult "dev mode (SELDON_INDEX): engine calls are disabled"
expect decisions 8 .view.openResult "dev mode (SELDON_INDEX): engine calls are disabled"
expect decisions 9 .view.decisions.sheet.open false
expect decisions 10 .view.tab memory
expect decisions 10 '.view.memory.rows | join(" | ")' \
  "lesson \`omarchy pkg add\` statt yay direkt | lesson Theme-Overrides nie im Omarchy-Repo | lesson Hyprland reload nach bindings.conf | topic omarchy | topic hyprland"
expect decisions 10 '.view.memory.sections | join(",")' "LESSONS,TOPICS"
for text in "3 lessons · 2 topics" "Opens the logbook folder; the files are in memory/" "LESSONS" "TOPICS" \
  "Theme-Overrides nie im Omarchy-Repo" "omarchy" "memory/omarchy.md · updated 2026-10-01" \
  "hyprland" "memory/hyprland.md · updated 2026-09-13" "Open"; do
  shows decisions 10 "$text"
done
expect decisions 12 .view.memory.cursor omarchy
expect decisions 13 .view.memory.cursor hyprland
expect decisions 14 .view.lastError "dev mode (SELDON_INDEX): engine calls are disabled"
clean_log decisions

# 21. Decisions live, against the fake engine, with real keys: `d` opens the
#     sheet; the title `--help "q"` is typed into it (no tab switch, no
#     digit); Enter arms ("Press Enter again: …"), a change to the title
#     disarms, Enter twice creates it: `decide --no-edit --json -- <title>`,
#     then `open ADR-0005 --editor --json` from the answer; the sheet closes,
#     the keys come back and the cursor sits on ADR-0005 once the index
#     lists it. Then Enter, *Open* and `e` open decisions, and on Memory
#     Enter and *Open* open the logbook. The exact argv and editor paths.
mkdir -p "$work/home-decisions-live"
run decisions-live "" \
  "text:4;text:d;type:--help \"q\";key:Return;key:Backspace;type:\";key:Return;key:Return;settle;wait:decisions.cursor=ADR-0005;key:Down;key:Down*2;key:Return;settle;click:Open;settle;key:Up;text:e;settle;text:6;key:Down;key:Down*4;key:Return;settle;click:Open;settle" \
  HOME="$work/home-decisions-live" FAKE_SELDON_FIXTURE="$fx/index.sample.json" HARNESS_RECORD="$work/decisions-live.record"
expect decisions-live 1 .view.tab decisions
expect decisions-live 2 .view.decisions.sheet.open true
expect decisions-live 2 .view.decisions.sheet.editing true
shows decisions-live 2 "NEW DECISION"
shows decisions-live 2 "Title, Enter twice creates the decision"
expect decisions-live 3 .view.decisions.sheet.title '--help "q"'
expect decisions-live 3 .view.tab decisions
expect decisions-live 4 .view.decisions.sheet.armed true
expect decisions-live 4 .view.decisions.sheet.hint 'Press Enter again: create the decision “--help "q"”'
shows decisions-live 4 'Press Enter again: create the decision “--help \"q\"”'
expect decisions-live 5 .view.decisions.sheet.armed false
expect decisions-live 5 .view.decisions.sheet.title '--help "q'
expect decisions-live 6 .view.decisions.sheet.armed false
expect decisions-live 7 .view.decisions.sheet.armed true
expect decisions-live 10 .view.decisions.sheet.open false
expect decisions-live 10 .view.decisions.sheet.editing false
expect decisions-live 10 .view.decisions.sheet.title ""
expect decisions-live 10 .view.decisions.result 'Created ADR-0005 · --help "q"'
expect decisions-live 10 '.view.decisions.rows | join(",")' \
  "ADR-0005 proposed,ADR-0004 proposed,ADR-0003 accepted,ADR-0002 accepted,ADR-0001 accepted"
shows decisions-live 10 'Created ADR-0005 · --help \"q\"'
shows decisions-live 10 "5 decisions · 2 proposed"
shows decisions-live 10 "2026-10-01 · decisions/ADR-0005-help-q.md"
expect decisions-live 11 .view.cursorActive true
expect decisions-live 12 .view.decisions.cursor ADR-0003
expect decisions-live 14 .view.openResult "Opened $work/home-decisions-live/Seldon/decisions/ADR-0003-zed.md in omarchy-launch-editor"
expect decisions-live 17 .view.decisions.cursor ADR-0004
expect decisions-live 19 .view.openResult "Opened $work/home-decisions-live/Seldon/decisions/ADR-0004-ollama-user-service.md in omarchy-launch-editor"
expect decisions-live 21 .view.memory.cursor "Theme-Overrides nie im Omarchy-Repo"
expect decisions-live 22 .view.memory.cursor hyprland
expect decisions-live 24 .view.openResult "Opened $work/home-decisions-live/Seldon in omarchy-launch-editor"
expect decisions-live 26 .view.lastError ""
want=$(printf '%s\n' "$(q --version --json)" "$(q capture --all --json --quiet)" "$(q status --json)" \
  "$(q decide --no-edit --json -- '--help "q"')" "$(q open ADR-0005 --editor --json)" \
  "$(q open ADR-0003 --editor --json)" "$(q open ADR-0003 --editor --json)" "$(q open ADR-0004 --editor --json)" \
  "$(q open logbook --editor --json)" "$(q open logbook --editor --json)")
got=$(cat "$work/home-decisions-live/argv.log" 2>/dev/null || true)
if [[ $got == "$want" ]]; then
  pass=$((pass + 1)); echo "ok   decisions-live: engine argv"
else
  fail=$((fail + 1)); echo "FAIL decisions-live: engine argv differs"; diff <(echo "$want") <(echo "$got") | sed 's/^/     /'
fi
d="$work/home-decisions-live/Seldon"
want=$(printf '%s\n' omarchy-launch-editor "$d/decisions/ADR-0005-help-q.md" -- \
  omarchy-launch-editor "$d/decisions/ADR-0003-zed.md" -- omarchy-launch-editor "$d/decisions/ADR-0003-zed.md" -- \
  omarchy-launch-editor "$d/decisions/ADR-0004-ollama-user-service.md" -- \
  omarchy-launch-editor "$d" -- omarchy-launch-editor "$d" --)
got=$(cat "$work/decisions-live.record" 2>/dev/null || true)
if [[ $got == "$want" ]]; then
  pass=$((pass + 1)); echo "ok   decisions-live: editor paths"
else
  fail=$((fail + 1)); echo "FAIL decisions-live: editor launches differ"; diff <(echo "$want") <(echo "$got") | sed 's/^/     /'
fi
clean_log decisions-live

# 22. Refusals keep the title: Enter on a blank title is refused in the
#     plugin; the engine refuses the decision (lock held, exit 4): the sheet
#     shows its message and keeps the title, nothing is opened; Esc gives the
#     keys back and the tab shows the refusal; `d` brings the sheet back with
#     the title.
mkdir -p "$work/home-decisions-locked"
run decisions-locked "" "text:4;text:d;key:Return;type:keep me;key:Return;key:Return;settle;key:Escape;view;text:d" \
  HOME="$work/home-decisions-locked" FAKE_SELDON_FIXTURE="$fx/index.sample.json" FAKE_SELDON_LOCKED=1
expect decisions-locked 3 .view.decisions.sheet.result "Give the decision a title"
expect decisions-locked 3 .view.decisions.sheet.armed false
expect decisions-locked 4 .view.decisions.sheet.result ""
expect decisions-locked 7 .view.decisions.sheet.result "the logbook is locked by another seldon (pid 4242)"
expect decisions-locked 7 .view.decisions.sheet.title "keep me"
expect decisions-locked 7 .view.decisions.sheet.editing true
expect decisions-locked 7 .view.decisions.resultOk false
shows decisions-locked 7 "the logbook is locked by another seldon (pid 4242)"
expect decisions-locked 8 .view.decisions.sheet.open false
expect decisions-locked 8 .view.decisions.sheet.editing false
shows decisions-locked 9 "the logbook is locked by another seldon (pid 4242)"
expect decisions-locked 9 .view.lastError ""
expect decisions-locked 10 .view.decisions.sheet.open true
expect decisions-locked 10 .view.decisions.sheet.title "keep me"
expect decisions-locked 10 .view.decisions.sheet.editing true
want=$(printf '%s\n' "$(q --version --json)" "$(q capture --all --json --quiet)" "$(q status --json)" \
  "$(q decide --no-edit --json -- "keep me")")
got=$(cat "$work/home-decisions-locked/argv.log" 2>/dev/null || true)
if [[ $got == "$want" ]]; then
  pass=$((pass + 1)); echo "ok   decisions-locked: engine argv"
else
  fail=$((fail + 1)); echo "FAIL decisions-locked: engine argv differs"; diff <(echo "$want") <(echo "$got") | sed 's/^/     /'
fi
clean_log decisions-locked

# 23. Label fit (WP-039): at font scale 1.0 and 1.25 (`[font] base-size`
#     12 and 15 in the user's shell.toml) the panel is Style.space(460) wide
#     and nothing that is a label is cut off on any tab: no tab or chip
#     Button narrower than its label, no Text wider than its box or past the
#     panel's edge (harness `overflow`), the Changelog header never elided.
#     Only user content may elide (Changelog row text, Work mini-card titles;
#     Enter or the card below shows it in full); Today, Decisions, System and
#     Memory elide nothing. The tab strip is one line and its cells keep
#     their widths when the selection moves. On a screen narrower than the
#     panel (HARNESS_CARD_WIDTH, the real panel's availableCardWidth) the
#     strip and the header wrap instead of clipping.
#     PANEL_FIT_SHOTS=<dir> also renders every tab at both scales in three
#     themes (work/active/WP-039/screenshots/).
fit_steps="settle;shot:today;tab:changelog;filter:seldon;shot:changelog;tab:work;shot:work;tab:decisions;shot:decisions;tab:system;shot:system;tab:memory;shot:memory"
# fit_case <case> <base size> <themes…> [-- VAR=value …]
fit_case() {
  local name=$1 base=$2 theme home
  shift 2
  local themes=() extra=()
  while [[ $# -gt 0 && $1 != -- ]]; do themes+=("$1"); shift; done
  [[ ${1:-} == -- ]] && shift
  extra=("$@")
  for theme in "${themes[@]}"; do
    home="$work/home-$name-$theme"
    mkdir -p "$home/.config/omarchy" "$home/.local/state/omarchy/current/theme"
    printf '[font]\nbase-size = %s\n' "$base" >"$home/.config/omarchy/shell.toml"
    [[ $theme == default ]] || cp "${OMARCHY_PATH:-/usr/share/omarchy}/themes/$theme/colors.toml" \
      "$home/.local/state/omarchy/current/theme/colors.toml"
    local steps=$fit_steps shots=()
    if [[ -n ${PANEL_FIT_SHOTS:-} ]]; then
      mkdir -p "$PANEL_FIT_SHOTS"
      steps=$(sed "s/shot:\([a-z]*\)/shot:$name-$theme-\1/g" <<<"$fit_steps")
      shots=(HARNESS_SHOTS="$PANEL_FIT_SHOTS")
    fi
    run "$name-$theme" "" "$steps" HOME="$home" FAKE_SELDON_FIXTURE="$fx/index.sample.json" "${shots[@]}" "${extra[@]}"
    clean_log "$name-$theme"
  done
}
# fit_expect <case> <panel width> <tab strip on one line> [narrow] — over
# every step. A narrow case lets content elide on every tab (the Decisions
# file path), never a label.
fit_expect() {
  local name=$1 width=$2 oneline=$3 narrow=${4:-} n
  n=$(wc -l <"$work/$name.steps")
  expect "$name" 1 '.view.tab' today
  expect "$name" "$n" '.view.tab' memory
  expect "$name" 1 .contentWidth "$width"
  expect "$name" 1 .view.tabStrip.oneLine "$oneline"
  # Steps 1 (Today selected) and 3 (Changelog selected): same cell widths.
  local w1 w3
  w1=$(sed -n 1p "$work/$name.steps" | jq -r .view.tabStrip.widths)
  expect "$name" 3 .view.tabStrip.widths "$w1"
  shows "$name" 4 "18 events from seldon · newest first"
  local i
  for ((i = 1; i <= n; i++)); do
    expect "$name" "$i" '[.overflow[] | select(startswith("elided:") | not)] | join(" | ")' ""
    expect "$name" "$i" '[.overflow[] | select(test("^elided:[0-9]+ events"))] | length' 0
    [[ -n $narrow ]] || expect "$name" "$i" 'if (.view.tab | IN("changelog", "work")) then 0 else (.overflow | length) end' 0
  done
}
fit_themes=(default)
[[ -n ${PANEL_FIT_SHOTS:-} ]] && fit_themes=(tokyo-night osaka-jade catppuccin-latte)
fit_case fit-100 12 "${fit_themes[@]}"
fit_case fit-125 15 "${fit_themes[@]}"
for theme in "${fit_themes[@]}"; do
  fit_expect "fit-100-$theme" 460 true
  fit_expect "fit-125-$theme" 575 true
  # WP-051: the 20 px heading gives a box of neither 24 nor 32 (twice its
  # cap height, 28–30 depending on the font): the A1 master
  expect "fit-125-$theme" 1 .view.mark.file a1-icon-mask.svg
done
fit_case fit-narrow 12 default -- HARNESS_CARD_WIDTH=300
fit_expect fit-narrow-default 300 false narrow

# 24. Offscreen renders of the Today tab in three themes (only with PANEL_SHOTS;
# the panel's counterpart of overlay-view.sh's OVERLAY_SHOTS; not live
# screenshots). plugin/preview.png is composed from these (docs/TESTING.md).
if [[ -n ${PANEL_SHOTS:-} ]]; then
  mkdir -p "$PANEL_SHOTS"
  omarchy="${OMARCHY_PATH:-/usr/share/omarchy}"
  for theme in osaka-jade tokyo-night catppuccin-latte; do
    home="$work/home-shot-$theme"
    mkdir -p "$home/.local/state/omarchy/current/theme"
    cp "$omarchy/themes/$theme/colors.toml" "$home/.local/state/omarchy/current/theme/colors.toml"
    # A live run on the fake engine: no dev-mode note (it prints the index
    # path) and the QuickEntry as a user sees it.
    run "shot-$theme" "" "settle;shot:panel-$theme-today;view" \
      HOME="$home" FAKE_SELDON_FIXTURE="$fx/index.sample.json" HARNESS_SHOTS="$PANEL_SHOTS"
    expect "shot-$theme" 1 .view.tab today
    expect "shot-$theme" 1 .view.today.quickEntry.enabled true
    expect "shot-$theme" 1 .view.mark.file a5-panel-mark-24.svg
    expect "shot-$theme" 1 .view.mark.ready true
    expect "shot-$theme" 1 .view.today.state crisis
    clean_log "shot-$theme"
    # The not-initialised banner with its pictogram (A11, WP-051).
    home="$work/home-shot-uninit-$theme"
    mkdir -p "$home/.local/state/omarchy/current/theme"
    cp "$omarchy/themes/$theme/colors.toml" "$home/.local/state/omarchy/current/theme/colors.toml"
    run "shot-uninit-$theme" "" "settle;shot:panel-$theme-uninit;view" \
      HOME="$home" FAKE_SELDON_MODE=uninit HARNESS_SHOTS="$PANEL_SHOTS"
    expect "shot-uninit-$theme" 1 .view.status notInitialised
    expect "shot-uninit-$theme" 1 .view.bannerPictogram logbook-not-initialised
    clean_log "shot-uninit-$theme"
  done
fi

# 25. A tab change gives the keys back (WP-067): a hidden tab's field keeps
#     Qt's active focus, so keys and Enter reached a field nobody sees. Type
#     in Today's QuickEntry, click the Work tab, press j and Enter: the
#     panel has the keys, j moves Work's cursor, the note is not sent and
#     its draft stays. The same for the new-case sheet left through IPC
#     `tab` (Panel.selectTabById): the sheet stays open with its title, no
#     case is created. Only the start-up calls reach the engine.
mkdir -p "$work/home-tab-focus"
run tab-focus "" \
  "text:n;type:abc;click:Work;text:j;key:Return;settle;text:1;text:n;key:Escape;text:+;type:xyz;tab:changelog;key:Return;settle;text:3;text:1;text:n;key:Tab;key:Down;tab:work" \
  HOME="$work/home-tab-focus" FAKE_SELDON_FIXTURE="$fx/index.sample.json"
expect tab-focus 2 .view.today.quickEntry.editing true
expect tab-focus 2 .view.keys false
expect tab-focus 3 .view.tab work
expect tab-focus 3 .view.today.quickEntry.editing false
expect tab-focus 3 .view.keys true
expect tab-focus 4 .view.today.quickEntry.text abc
expect tab-focus 4 .view.cursorActive true
expect tab-focus 4 .view.work.cursor C-2026-005
expect tab-focus 6 .view.today.quickEntry.result ""
expect tab-focus 6 .view.today.quickEntry.text abc
expect tab-focus 8 .view.tab today
expect tab-focus 8 .view.today.quickEntry.editing true
expect tab-focus 8 .view.today.quickEntry.text abc
expect tab-focus 11 .view.work.sheet.editing true
expect tab-focus 11 .view.work.sheet.title xyz
expect tab-focus 12 .view.tab changelog
expect tab-focus 12 .view.work.sheet.editing false
expect tab-focus 12 .view.keys true
expect tab-focus 15 .view.tab work
expect tab-focus 15 .view.work.sheet.open true
expect tab-focus 15 .view.work.sheet.title xyz
expect tab-focus 15 .view.work.result ""
expect tab-focus 19 .view.today.quickEntry.editing true
expect tab-focus 20 .view.tab work
expect tab-focus 20 .view.today.quickEntry.editing false
expect tab-focus 20 .view.keys true
want=$(printf '%s\n' "$(q --version --json)" "$(q capture --all --json --quiet)" "$(q status --json)")
got=$(cat "$work/home-tab-focus/argv.log" 2>/dev/null || true)
if [[ $got == "$want" ]]; then
  pass=$((pass + 1)); echo "ok   tab-focus: engine argv"
else
  fail=$((fail + 1)); echo "FAIL tab-focus: engine argv differs"; diff <(echo "$want") <(echo "$got") | sed 's/^/     /'
fi
clean_log tab-focus

# 26. The Changelog cursor follows its event (WP-067): the cursor on row 4
#     (tokyo-night, open drift), then "Capture now" writes an index with two
#     newer events at the top. The cursor moves to row 6 with its event, and
#     Enter opens tokyo-night's drift sheet, not row 4's snapshot. A new
#     filter still starts at the top.
jq '.events = [
    {id: "01M3W2NEWEVENT000000000001", ts: "2026-10-01T18:31:00+02:00", source: "manual", kind: "note",
     subject: "journal", detail: "Second event above the cursor", zone: "green", actor: "human", case: null},
    {id: "01M3W2NEWEVENT000000000000", ts: "2026-10-01T18:30:00+02:00", source: "manual", kind: "note",
     subject: "journal", detail: "First event above the cursor", zone: "green", actor: "human", case: null}
  ] + .events' "$fx/index.sample.json" >"$work/after-two.json"
mkdir -p "$work/home-cursor-follow"
run cursor-follow "" "tab:changelog;key:Down;key:Down*4;text:c;wait:changelog.rows=64;key:Return;key:Escape;text:f" \
  HOME="$work/home-cursor-follow" FAKE_SELDON_FIXTURE="$fx/index.sample.json" FAKE_SELDON_FIXTURE_AFTER="$work/after-two.json"
expect cursor-follow 3 .view.cursor 4
expect cursor-follow 3 .view.changelog.selected 01M3VTGNY0NZG4AY80814WSKGR
expect cursor-follow 5 .view.changelog.rows 64
expect cursor-follow 5 .view.cursor 6
expect cursor-follow 5 .view.changelog.selected 01M3VTGNY0NZG4AY80814WSKGR
expect cursor-follow 6 .view.drift.open true
expect cursor-follow 6 .view.drift.subject tokyo-night
expect cursor-follow 6 .view.changelog.expanded ""
expect cursor-follow 8 .view.changelog.filter pacman
expect cursor-follow 8 .view.cursor 0
expect cursor-follow 8 .view.changelog.selected "$(jq -r '[.events[] | select(.source == "pacman")][0].id' "$fx/index.sample.json")"
clean_log cursor-follow

# 27. *Capture now* on the Changelog replaces a pending lock retry, as the
#     bar's right click and the `c` key do (WP-078): the start-up capture
#     finds the lock held (exit 4), the retry waits 50 s and the button
#     reads "Capturing"; a click on it captures at once ("nothing new"),
#     and the retry never runs (one locked and one good capture).
mkdir -p "$work/home-capture-click"
run capture-click "" "tab:changelog;view;click:Capturing;settle;view" \
  HOME="$work/home-capture-click" FAKE_SELDON_FIXTURE="$fx/index.sample.json" \
  FAKE_SELDON_CAPTURE_LOCKED=1 SELDON_LOCK_RETRY_MS=50000
expect capture-click 2 .view.capturing true
expect capture-click 2 .view.captureResult "waiting for another seldon process; trying again shortly"
shows capture-click 2 "Capturing"
expect capture-click 5 .view.capturing false
expect capture-click 5 .view.captureResult "nothing new"
expect capture-click 5 .view.lastError ""
shows capture-click 5 "Capture now"
for counter in locked-captures:1 captures:1; do
  got=$(cat "$work/home-capture-click/${counter%%:*}" 2>/dev/null || echo 0)
  if [[ $got == "${counter#*:}" ]]; then
    pass=$((pass + 1)); echo "ok   capture-click: ${counter%%:*} = $got"
  else
    fail=$((fail + 1)); echo "FAIL capture-click: ${counter%%:*} = $got (want ${counter#*:})"
  fi
done
clean_log capture-click "jax\.seldon: seldon capture exit 4: another seldon process holds the lock "

real_home_check panel-view

echo "panel-view: $pass passed, $fail failed"
((fail == 0))
