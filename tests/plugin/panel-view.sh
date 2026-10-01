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
shows() {
  expect "$1" "$2" "[.texts[] | select(. == \"$3\")] | length > 0" true
}

clean_log() {
  local bad
  bad=$(sed 's/\x1b\[[0-9;]*m//g' "$work/$1.log" | grep -a -E "ERROR|WARN|TypeError|ReferenceError|Binding loop" \
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

# 1. The sample: every tab renders its data; the strip is on every tab.
run sample "$fx/index.sample.json" \
  "view;tab:changelog;filter:pacman;text:f;filter:all;key:Down;key:Down*32;key:Return;tab:system;key:Down*40"
expect sample 1 .view.status ok
expect sample 1 .view.tab today
expect sample 1 .view.banner ""
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
expect sample 8 .view.changelog.expanded 01M3SXBQVR7AW8PJQC1YXDCQ14
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
#    h/l switch tabs; digits are fixed per tab id (Today 1, Changelog 2,
#    Work 3, System 5) and the digit of an absent tab (4 = Decisions) does
#    nothing.
run keys "$fx/index.sample.json" \
  "key:Tab;key:Backtab;key:Right;key:Right;key:Right;key:Right;key:Left;text:l;text:h;text:5;text:4;text:3;text:1;text:2;key:Down;key:Down*2;text:k;text:j;key:Return;key:Escape" \
  HARNESS_BAR=1
expect keys 1 .view.tab today
expect keys 1 '.switches | join(",")' 1
expect keys 2 .view.tab today
expect keys 2 '.switches | join(",")' "1,-1"
expect keys 3 .view.tab changelog
expect keys 4 .view.tab work
expect keys 5 .view.tab system
expect keys 6 .view.tab today
expect keys 7 .view.tab system
expect keys 8 .view.tab today
expect keys 9 .view.tab system
expect keys 10 .view.tab system
expect keys 11 .view.tab system
expect keys 12 .view.tab work
expect keys 13 .view.tab today
expect keys 14 .view.tab changelog
expect keys 14 '.switches | length' 2
expect keys 15 .view.cursorActive true
expect keys 16 .view.cursor 2
expect keys 17 .view.cursor 1
expect keys 18 .view.cursor 2
expect keys 20 .view.opened false
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

# 4. Snapper without permissions (ADR-0011): its banner on every tab.
run snapper "$fx/index-variants/snapper-degraded.json" "view;tab:changelog;tab:system"
for step in 1 2 3; do
  expect snapper $step .view.snapper "Snapshots not readable"
done
shows snapper 1 'sudo snapper -c root set-config ALLOW_USERS=$USER SYNC_ACL=yes'
shows snapper 1 "Run in terminal"
shows snapper 3 "failing · snapper: No permissions. The snapper config does not list this user in ALLOW_USERS; see \`seldon doctor\`."
clean_log snapper

# 5. Not initialised: the banner, no strip, empty tabs.
run uninit "$fx/index-variants/not-initialised.json" "view;tab:changelog;tab:system;tab:work;text:+"
expect uninit 1 .view.banner "Logbook not initialised"
expect uninit 1 .view.crisis ""
shows uninit 1 "No index to show"
expect uninit 2 .view.changelog.rows 0
shows uninit 3 "No index to show"
expect uninit 4 '.view.work.columns | join(",")' "queued 0,active 0,completed 0"
expect uninit 4 .view.work.card null
shows uninit 4 "No index to show"
expect uninit 5 .view.work.sheet.open false
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
run work "$fx/index.sample.json" "text:3;view;key:Down;key:Down*3;key:Down*2;key:Return;text:x;key:Down*5;key:Up*9"
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
expect work 4 '.view.work.card.actions | join(",")' "Verify,Drop,Open"
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
clean_log work

# 10. Work tab live, against the fake engine: a new case through the sheet
#     with real keys (Tab walks the pickers, an invalid area is refused in
#     the plugin), then start → verify → done on C-2026-005 with Enter twice
#     each, the case moving columns through the FileView and the cursor
#     following it; Open on the completed case; Done on C-2026-008, which
#     the index shows in verification but the fake engine's logbook has
#     active: the engine refuses and its message is the result line; x twice
#     drops C-2026-004; `e` opens it.
mkdir -p "$work/home-work"
echo "C-2026-008 active" >"$work/home-work/cases"
run work-live "" \
  "text:3;text:+;type:--help;key:Tab;key:Right;key:Return;key:Tab;key:Right;key:Return;key:Tab;key:Left;key:Return;key:Tab;type:Dev;key:Return;key:Backspace*3;type:dev-env;key:Return;settle;wait:work.cursor=C-2026-009;key:Up;key:Up*3;key:Return;key:Return;settle;wait:work.card.status=active;key:Return;key:Return;wait:work.card.status=verification;key:Return;key:Return;wait:work.card.status=completed;key:Return;settle;key:Up;key:Return;key:Down;key:Up;key:Return;key:Return;settle;key:Up;text:x;text:x;wait:work.card.status=dropped;text:e;settle" \
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
expect work-live 26 '.view.work.card.actions | join(",")' "Verify,Drop,Open"
expect work-live 29 .view.work.result "C-2026-005: active → verification"
expect work-live 32 '.view.work.columns | join(",")' "queued 3,active 3,completed 3"
expect work-live 32 .view.work.result "C-2026-005: verification → completed · journal journal/2026/2026-10-01.md"
expect work-live 32 '.view.work.card.actions | join(",")' "Open"
expect work-live 34 .view.openResult "Opened $work/home-work/Seldon/work/active/C-2026-005.md in omarchy-launch-editor"
expect work-live 35 .view.work.cursor C-2026-008
expect work-live 36 .view.work.card.armed done
expect work-live 37 .view.work.card.armed ""
expect work-live 38 .view.work.cursor C-2026-008
expect work-live 38 .view.work.card.armed ""
refusal='C-2026-008 is active; `seldon plan done` needs a case that is verification; run `seldon plan verify` first'
expect work-live 41 .view.work.result "$refusal"
expect work-live 41 .view.work.resultOk false
shows work-live 41 "$refusal"
expect work-live 41 '.view.work.columns | join(",")' "queued 3,active 3,completed 3"
expect work-live 41 .view.work.cursor C-2026-008
expect work-live 41 .view.lastError ""
expect work-live 42 .view.work.cursor C-2026-004
expect work-live 43 .view.work.card.armed drop
shows work-live 43 "Confirm drop"
shows work-live 43 "Drop C-2026-004? Press x again or click Confirm drop. This is final."
expect work-live 45 .view.work.result "C-2026-004: active → dropped"
expect work-live 45 '.view.work.columns | join(",")' "queued 3,active 2,completed 4"
expect work-live 45 .view.work.wip "1 / 3 active"
expect work-live 45 '.view.work.card.actions | join(",")' "Open"
shows work-live 45 "dropped"
expect work-live 47 .view.openResult "Opened $work/home-work/Seldon/work/active/C-2026-004.md in omarchy-launch-editor"
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

real_home_check panel-view

echo "panel-view: $pass passed, $fail failed"
((fail == 0))
