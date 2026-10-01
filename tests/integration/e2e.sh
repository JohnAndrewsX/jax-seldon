#!/usr/bin/env bash
# End-to-end test of the engine ↔ plugin chain on a real machine (WP-013).
#
#   tests/integration/e2e.sh                 full run on the test host over ssh
#   tests/integration/e2e.sh --engine-only   engine steps on this host, scratch dirs
#
# Full run: build the static musl engine here, copy it to the test host's
# ~/.local/bin, init ~/Seldon-e2e, capture, plan new/start, log, status;
# rsync the plugin to its dev install, validate, restart the shell, read
# `jax.seldon.service status` and `jax.seldon.panel view`, write a note
# through the QuickEntry with wtype, check the shell log; then restore the
# test host to what it found (see `remote_restore`).
#
# --engine-only: the same engine steps on this host with HOME,
# XDG_CONFIG_HOME and XDG_STATE_HOME in a temp dir, then Service.qml in a
# private headless Quickshell (dev mode) against the index the engine wrote.
# It never starts or touches the running shell, and checks that the real
# ~/.local/state/seldon and ~/.config/seldon did not change.
#
# Environment:
#   SELDON_TEST_HOST        ssh alias of the test host (default: test)
#   SELDON_E2E_SINCE_DAYS   backfill of init's first capture, days back
#                           (default: 7); a fresh logbook records nothing older
#                           than its creation otherwise, and the run needs
#                           package events
#   SELDON_E2E_OUT          if set, a local dir that receives the index, the
#                           service status, the pill and the panel views of
#                           the run (they name the host: never commit them)
#
# Exit 0 when every check passes, 1 otherwise; a summary is printed either
# way. Idempotent: every path it creates is removed, every path it finds is
# put back; a run that died half-way is restored at the start of the next.
set -euo pipefail

root=$(cd "$(dirname "$0")/../.." && pwd)
mode=full
for arg in "$@"; do
  case $arg in
    --engine-only) mode=engine ;;
    -h | --help) awk 'NR > 1 && /^#/ { sub(/^# ?/, ""); print; next } NR > 1 { exit }' "$0"; exit 0 ;;
    *) echo "e2e: unknown argument: $arg" >&2; exit 1 ;;
  esac
done

host=${SELDON_TEST_HOST:-test}
since_days=${SELDON_E2E_SINCE_DAYS:-7}
[[ $since_days =~ ^[0-9]+$ ]] || { echo "e2e: SELDON_E2E_SINCE_DAYS must be a whole number of days, got '$since_days'" >&2; exit 1; }
target=x86_64-unknown-linux-musl
bin="$root/engine/target/$target/release/seldon"
run_id=$(date +%Y%m%dT%H%M%S)-$$
note="e2e note $run_id: a \"b\" c \$(true)"
qe_note="e2e quickentry $run_id: --help 'x'"
contract=$(jq -r '.seldon.contractVersion' "$root/plugin/manifest.json")

for tool in cargo jq; do
  command -v "$tool" >/dev/null || { echo "e2e: $tool not found" >&2; exit 1; }
done

pass=0
fail=0
failures=()
empty='{}'

out_dir=${SELDON_E2E_OUT:-}
[[ -z $out_dir ]] || mkdir -p "$out_dir"
# keep <name> <text> — save an artifact when SELDON_E2E_OUT is set.
keep() { [[ -z $out_dir ]] || printf '%s\n' "$2" >"$out_dir/$1"; }

ok() { pass=$((pass + 1)); echo "ok   $1"; }
bad() { fail=$((fail + 1)); failures+=("$1"); echo "FAIL $1"; }
# eq <label> <got> <want>
eq() { if [[ $2 == "$3" ]]; then ok "$1 = $3"; else bad "$1 = $2 (want $3)"; fi; }
# ge <label> <got> <min>
ge() { if [[ $2 =~ ^[0-9]+$ ]] && (( $2 >= $3 )); then ok "$1 = $2 (>= $3)"; else bad "$1 = $2 (want >= $3)"; fi; }
step() { echo; echo "== $*"; }
# After the backup the EXIT trap restores the test host and prints the summary.
die() { bad "$1"; [[ -n ${restore_pending:-} ]] || summary; exit 1; }

summary() {
  echo
  echo "== e2e summary ($mode${remote:+, test host}): $pass passed, $fail failed"
  local f
  for f in "${failures[@]+"${failures[@]}"}"; do echo "   FAIL $f"; done
}

# ---- runners ---------------------------------------------------------------

remote=""
[[ $mode == full ]] && remote=1

# The remote side runs bash with OMARCHY_PATH, its bin dir and the Wayland
# session on PATH/env; non-interactive ssh has none of them.
remote_env='set -uo pipefail
export OMARCHY_PATH=/usr/share/omarchy
export PATH="$OMARCHY_PATH/bin:$HOME/.local/bin:$PATH"
export XDG_RUNTIME_DIR=/run/user/$(id -u)
export WAYLAND_DISPLAY=$(ls "$XDG_RUNTIME_DIR" 2>/dev/null | grep -m 1 -E "^wayland-[0-9]+$" || echo wayland-1)
cd "$HOME"'

# rsh <script> — run a bash script on the test host. The script travels as
# base64 in the argument and stdin is /dev/null, so nothing it starts can
# swallow the rest of the script.
rsh() {
  local b64
  b64=$(printf '%s\n%s\n' "$remote_env" "$1" | base64 -w 0)
  ssh -n -o BatchMode=yes -o ConnectTimeout=10 "$host" "bash -c \"\$(echo $b64 | base64 -d)\""
}

# seldon <args…> — run the engine under test; stdout is the command's output,
# the exit code is the engine's.
seldon() {
  if [[ -n $remote ]]; then
    rsh ".local/bin/seldon $(printf '%q ' "$@")"
  else
    env -u SELDON_CONFIG -u SELDON_LOGBOOK -u SELDON_NOW -u SELDON_INDEX \
      HOME="$work/home" XDG_CONFIG_HOME="$work/home/.config" XDG_STATE_HOME="$work/home/.local/state" \
      "$bin" "$@"
  fi
}

# read_index — the index the engine wrote, on one line.
read_index() {
  if [[ -n $remote ]]; then
    rsh 'cat .local/state/seldon/index.json'
  else
    cat "$work/home/.local/state/seldon/index.json"
  fi
}

# ipc <target> <method> [arg] — the running shell on the test host.
ipc() { rsh "omarchy-shell $(printf '%q ' "$@")"; }

# The pill SPEC-PLUGIN §4 derives from an index: `⟡ A · D`, zero parts hidden.
pill_of() {
  jq -r '"⟡"
    + (if .summary.activeCases > 0 then " \(.summary.activeCases)" else "" end)
    + (if .summary.openDrift > 0 then " · \(.summary.openDrift)" else "" end)'
}

# ---- test host: baseline, backup, restore ------------------------------------

# Everything the run may change on the test host, and helpers every remote
# script below shares. Seldon's own paths and the backup dir are the only
# places it writes besides the plugin dev install.
remote_paths='paths=(Seldon-e2e .local/bin/seldon .local/state/seldon .config/seldon)
plugin_dir=.config/omarchy/plugins/jax.seldon
backup=.cache/seldon-e2e
plugin_hash() {
  if [[ -d $plugin_dir ]]; then
    (cd "$plugin_dir" && find . -type f -print0 | LC_ALL=C sort -z | xargs -0 sha256sum | sha256sum | cut -c1-12)
  else echo absent; fi
}
# true, false, or empty when the shell does not answer (busy rescanning):
# empty means unknown and never leads to a change.
plugin_enabled() {
  local i v
  for i in 1 2 3 4 5; do
    v=$(omarchy-shell shell listPlugins 2>/dev/null | jq -r ".[] | select(.id == \"jax.seldon\") | .enabled" 2>/dev/null)
    [[ $v == true || $v == false ]] && { echo "$v"; return; }
    sleep 1
  done
}
# The omarchy-shell instance; pgrep also finds crash-report windows.
shell_pid() { quickshell list -a -j 2>/dev/null | jq -r "[.[] | select(.config_path == \"$OMARCHY_PATH/shell/shell.qml\")] | last | .pid // empty"; }
# Every rsync into the plugin dir makes the shell hot-reload once per file;
# restarting in the middle of that crashed it once (WP-013 FINDINGS). Wait
# until it answers again, then give the reloads time to finish.
settle() {
  local i
  for i in $(seq 20); do omarchy-shell shell ping >/dev/null 2>&1 && break; sleep 0.5; done
  sleep 5
}
# A locked session: omarchy-restart-shell refuses, and wtype would type into
# the lock screen.
session_locked() { omarchy-shell lock status 2>/dev/null | jq -e ".locked or .sessionLocked" >/dev/null 2>&1; }
service_idle() {
  local i
  for i in $(seq 60); do
    omarchy-shell jax.seldon.service status 2>/dev/null | jq -e ".busy | not" >/dev/null 2>&1 && return 0
    sleep 0.5
  done
  return 1
}'

# One line per fact the restore must bring back; compared before and after.
remote_fingerprint() {
  rsh "$remote_paths"'
for p in "${paths[@]}"; do
  if [[ -e $p || -L $p ]]; then echo "$p: present $(find "$p" -printf "%p %s %T@\n" | LC_ALL=C sort | sha256sum | cut -c1-12)"
  else echo "$p: absent"; fi
done
echo "plugin dir: $(plugin_hash)"
echo "plugin enabled: $(plugin_enabled)"
echo "theme: $(omarchy theme current 2>/dev/null)"
echo "quickshell processes: $(pgrep -c -x quickshell)"
echo "shell crash reports: $(ls .cache/quickshell/crashes 2>/dev/null | wc -l)"'
}

# Put back what remote_backup saved. Safe to run twice, and the first thing a
# run does when a previous run died before its restore.
remote_restore() {
  rsh "$remote_paths"'
[[ -f $backup/found.env ]] || { echo "restore: nothing to restore"; exit 0; }
source "$backup/found.env"
omarchy-shell jax.seldon.panel close >/dev/null 2>&1 || true
running_hash=$(plugin_hash)
# put_back <path> — remove the run'"'"'s copy only when the path was absent at
# the backup or was moved aside; a path the backup had not reached yet (a run
# killed inside the loop) is the found one and stays.
put_back() {
  if [[ -e $backup/saved/$1 || -L $backup/saved/$1 ]]; then
    rm -rf "$1"; mkdir -p "$(dirname "$1")"; mv "$backup/saved/$1" "$1"
  elif [[ " ${absent-} " == *" $1 "* ]]; then
    rm -rf "$1"
  else
    echo "restore: kept ~/$1 (found there, not moved aside)"
  fi
}
# The engine binary first, so a plugin reloaded by the steps below finds no
# engine and writes nothing while the state dirs are removed.
put_back .local/bin/seldon
if [[ $plugin_existed == 1 ]]; then
  rsync -a --checksum --delete "$backup/plugin/" "$plugin_dir/"
else
  rm -rf "$plugin_dir"
fi
# Disable only when the run found it disabled for certain; unknown stays as is.
if [[ $found_enabled == false && $(plugin_enabled) == true ]]; then omarchy plugin disable jax.seldon >/dev/null 2>&1 || true; fi
if [[ -n $(shell_pid) ]]; then
  if [[ $(plugin_hash) != "$running_hash" ]]; then
    # Other plugin code than the shell runs: only a restart loads it.
    settle
    if ! omarchy-restart-shell >/dev/null 2>&1; then
      echo "restore: shell restart failed$(session_locked && echo " (session locked)"); the restored plugin code loads on the next restart"
      omarchy-shell jax.seldon.service refresh >/dev/null 2>&1 || true
    fi
  else
    omarchy-shell jax.seldon.service refresh >/dev/null 2>&1 || true
  fi
  service_idle || echo "restore: the plugin is still running an engine call"
fi
for p in "${paths[@]}"; do
  [[ $p == .local/bin/seldon ]] || put_back "$p"
done
rm -rf "$backup"
echo "restore: done"'
}

remote_backup() {
  rsh "$remote_paths"'
mkdir -p "$backup/saved"
plugin_existed=0
if [[ -d $plugin_dir ]]; then plugin_existed=1; rsync -a "$plugin_dir/" "$backup/plugin/"; fi
found_enabled=$(plugin_enabled)
absent=""
for p in "${paths[@]}"; do [[ -e $p || -L $p ]] || absent+=" $p"; done
# found.env first: a run killed inside the loop below is still restored by
# the next one (put_back keeps a found path the loop had not moved yet).
printf "plugin_existed=%s\nfound_enabled=%s\nabsent=\"%s\"\n" "$plugin_existed" "$found_enabled" "$absent" > "$backup/found.env"
for p in "${paths[@]}"; do
  if [[ -e $p || -L $p ]]; then mkdir -p "$backup/saved/$(dirname "$p")"; mv "$p" "$backup/saved/$p"; echo "backup: moved aside ~/$p"; fi
done'
}

# restore_and_verify — restore, then compare with the fingerprint taken
# before the run and wait for the service status found then.
restore_and_verify() {
  step "restore the test host"
  remote_restore
  local after snap
  after=$(remote_fingerprint)
  if [[ $after == "$before" ]]; then
    ok "test host restored (Seldon paths, plugin dir, enabled flag, theme, shell processes, crash reports)"
  else
    bad "test host restored"
    diff <(echo "$before") <(echo "$after") | sed 's/^/     /'
  fi
  if [[ -n $base_status ]]; then
    snap=$(wait_service ".status == \"$base_status\"" 60) || true
    eq "service status after restore" "$(jq -r .status <<<"${snap:-$empty}")" "$base_status"
  fi
  restore_pending=""
}

# wait_service <jq condition> <seconds> — poll `jax.seldon.service status`.
wait_service() {
  local deadline=$((SECONDS + $2)) snap
  while ((SECONDS < deadline)); do
    snap=$(ipc jax.seldon.service status 2>/dev/null || true)
    if [[ -n $snap ]] && jq -e "$1" >/dev/null 2>&1 <<<"$snap"; then
      printf '%s' "$snap"
      return 0
    fi
    sleep 1
  done
  printf '%s' "${snap:-}"
  return 1
}

# wait_view <jq condition> <seconds> — poll `jax.seldon.panel view`.
wait_view() {
  local deadline=$((SECONDS + $2)) v
  while ((SECONDS < deadline)); do
    v=$(ipc jax.seldon.panel view 2>/dev/null || true)
    if [[ -n $v ]] && jq -e "$1" >/dev/null 2>&1 <<<"$v"; then
      printf '%s' "$v"
      return 0
    fi
    sleep 1
  done
  printf '%s' "${v:-}"
  return 1
}

# ---- build -------------------------------------------------------------------

step "build the static engine ($target)"
cargo build --manifest-path "$root/engine/Cargo.toml" --locked --release --target "$target" --quiet \
  || die "musl release build"
ok "musl release build"
file_out=$(file -b "$bin" 2>/dev/null || echo "?")
[[ $file_out == *"static"* ]] && ok "binary is static" || bad "binary is static ($file_out)"

# ---- setup -------------------------------------------------------------------

if [[ -n $remote ]]; then
  step "test host: baseline and backup ($host)"
  rsh 'true' || die "ssh $host"
  # Never the dev host: an alias or localhost that leads back here would run
  # the steps above (copy to ~/.local/bin, ~/.config/seldon, shell restart)
  # on this machine.
  local_id=$(cat /etc/machine-id 2>/dev/null || hostname)
  remote_id=$(rsh 'cat /etc/machine-id 2>/dev/null || hostname')
  [[ -n $remote_id && $remote_id != "$local_id" ]] \
    || die "SELDON_TEST_HOST=$host is this machine (same machine id); the full run only targets the test host"
  ok "test host is another machine"
  rsh 'command -v omarchy-shell wtype jq rsync quickshell >/dev/null' || die "test host tools (omarchy-shell wtype jq rsync quickshell)"
  # Nothing is changed while the session is locked: the shell cannot be
  # restarted and keys would go to the lock screen.
  rsh "$remote_paths"$'\nsession_locked' && die "the test host's session is locked; unlock it and run again"
  # A run that died before its restore left the backup: restore it first.
  if rsh '[[ -f .cache/seldon-e2e/found.env ]]'; then
    echo "a previous run did not finish; restoring what it found"
    remote_restore
  fi
  before=$(remote_fingerprint)
  echo "$before" | sed 's/^/   /'
  base_status=$(ipc jax.seldon.service status 2>/dev/null | jq -r '.status // empty' || true)
  echo "   service status: ${base_status:-<none>}"
  remote_backup
  restore_pending=1
  trap 'set +e; [[ -n $restore_pending ]] && restore_and_verify; summary; exit 1' EXIT
  rsh 'mkdir -p .local/bin' && scp -q "$bin" "$host:.local/bin/seldon" && rsh 'chmod 755 .local/bin/seldon' \
    || die "copy the engine to ~/.local/bin"
  ok "engine copied to ~/.local/bin/seldon"
  logbook_arg="$(rsh 'echo "$HOME"')/Seldon-e2e"
  logbook='~/Seldon-e2e'
else
  step "dev host: scratch dirs"
  work=$(mktemp -d "${TMPDIR:-/tmp}/seldon-e2e.XXXXXX")
  mkdir -p "$work/home" "$work/bin"
  ln -s "$bin" "$work/bin/seldon"
  trap 'rm -rf "$work"' EXIT
  source "$root/tests/plugin/real-home-guard.sh"
  logbook_arg="$work/home/Seldon-e2e"
  logbook=$logbook_arg
  echo "   HOME=$work/home (XDG_CONFIG_HOME and XDG_STATE_HOME under it)"
fi

# ---- engine ------------------------------------------------------------------

step "engine: init, capture, plan, log, status"
out=$(seldon --version --json) || die "seldon --version"
eq "seldon --version: name" "$(jq -r .name <<<"$out")" seldon
out=$(seldon contract-version --json) || die "seldon contract-version"
eq "contract-version = manifest seldon.contractVersion" "$(jq -r .contractVersion <<<"$out")" "$contract"

# `init` runs the first capture (WP-024); `--since` backfills it. A fresh
# logbook records nothing older than its creation otherwise, and a later
# `capture --since` is ignored by collectors that already have a cursor.
since=$(date -d "-$since_days days" --iso-8601=seconds)
out=$(seldon init --non-interactive --path "$logbook_arg" --since "$since" --json) \
  || die "seldon init --non-interactive --path $logbook --since $since"
ok "seldon init --path $logbook --since -${since_days}d"
eq "init: first capture ran" "$(jq -r .capture.ran <<<"$out")" true
pkg_events=$(jq -r '[.capture.collectors[] | select(.name == "pacman") | .events] | add // 0' <<<"$out")
ge "init --since -${since_days}d: package events written" "$pkg_events" 1
degraded=$(jq -r '[.capture.collectors[] | select(.ok == false) | .name] | join(",")' <<<"$out")
echo "   written $(jq -r .capture.written <<<"$out"), open drift $(jq -r .capture.openDrift <<<"$out"), degraded collectors: ${degraded:-none}"
out=$(seldon capture --all --json) || die "seldon capture --all"
eq "capture after init writes nothing (idempotent)" "$(jq -r .written <<<"$out")" 0

out=$(seldon plan new --zone yellow --risk R1 --json -- "e2e integration case $run_id") || die "seldon plan new"
case_id=$(jq -r .case.id <<<"$out")
[[ $case_id =~ ^C-[0-9]{4}-[0-9]{3,}$ ]] && ok "plan new: $case_id" || bad "plan new: case id '$case_id'"
out=$(seldon plan start "$case_id" --json) || die "seldon plan start $case_id"
eq "plan start: status" "$(jq -r .case.status <<<"$out")" active

out=$(seldon log --case "$case_id" --json -- "$note") || die "seldon log"
eq "log: ledger detail" "$(jq -r .event.detail <<<"$out")" "$note"

out=$(seldon status --json) || die "seldon status"
eq "status: state.status" "$(jq -r .state.status <<<"$out")" ok
out=$(seldon index --check --json) || die "seldon index --check"
eq "index --check: valid" "$(jq -r .valid <<<"$out")" true
out=$(seldon doctor --json) || bad "seldon doctor exit $?"
eq "doctor: ok" "$(jq -r .ok <<<"$out")" true

step "engine: index.json"
index=$(read_index) || die "read index.json"
eq "index contractVersion" "$(jq -r .contractVersion <<<"$index")" "$contract"
eq "index summary.activeCases" "$(jq -r .summary.activeCases <<<"$index")" 1
eq "index: the case is active" "$(jq -r --arg id "$case_id" '[.cases | .. | objects | select(.id? == $id) | .status] | first // "missing"' <<<"$index")" active
eq "index today has the note" "$(jq -r --arg t "$note" '[.today.entries[] | select(.text == $t)] | length' <<<"$index")" 1
ge "index events from pacman" "$(jq -r '[.events[] | select(.source == "pacman")] | length' <<<"$index")" 1
expected_pill=$(pill_of <<<"$index")
echo "   summary $(jq -c .summary <<<"$index") → pill '$expected_pill'"
keep engine-index.json "$index"

# ---- plugin ------------------------------------------------------------------

if [[ -z $remote ]]; then
  step "plugin: Service.qml headless against this index (dev mode)"
  qs_bin=$(command -v quickshell || command -v qs || true)
  if [[ -z $qs_bin ]]; then
    bad "quickshell not found (needed for the headless plugin check)"
  else
    # Dev mode: renders the file, probes `seldon --version`, never runs a
    # writing command (CONTRACT rule 1); the clock is pinned to generatedAt.
    env -u SELDON_NOW -u SELDON_CONFIG -u SELDON_LOGBOOK QT_QPA_PLATFORM=offscreen \
      HOME="$work/home" XDG_CONFIG_HOME="$work/home/.config" XDG_STATE_HOME="$work/home/.local/state" \
      PATH="$work/bin:$PATH" HARNESS_PLUGIN_DIR="$root/plugin" HARNESS_UNTIL="status=ok" \
      SELDON_INDEX="$work/home/.local/state/seldon/index.json" \
      timeout 60 "$qs_bin" -p "$root/tests/plugin/harness/shell.qml" >"$work/harness.log" 2>&1 || true
    keep harness.log "$(cat "$work/harness.log")"
    snap=$(sed 's/\x1b\[[0-9;]*m//g' "$work/harness.log" | grep -a "HARNESS final " | sed 's/.*HARNESS final //' | tail -n 1)
    eq "plugin status" "$(jq -r .status <<<"${snap:-$empty}")" ok
    eq "plugin engine" "$(jq -r .engine <<<"${snap:-$empty}")" present
    eq "plugin pill = index counts" "$(jq -r .pill <<<"${snap:-$empty}")" "$expected_pill"
    errs=$(sed 's/\x1b\[[0-9;]*m//g' "$work/harness.log" | grep -a -E "ERROR|WARN" \
      | grep -a -v -E "WAYLAND_DISPLAY is present|QT_QPA_PLATFORM|--- WARNING ---|most functionality will be broken" || true)
    [[ -z $errs ]] && ok "harness log clean" || { bad "harness log has errors"; echo "$errs" | head -n 5 | sed 's/^/     /'; }
  fi
  step "dev host: real Seldon dirs"
  real_home_check e2e
  summary
  ((fail == 0))
  exit $?
fi

step "plugin: install, validate, restart the shell"
rsync -a --checksum --delete "$root/plugin/" "$host:.config/omarchy/plugins/jax.seldon/" || die "rsync plugin"
ok "plugin rsynced to the dev install"
rsh 'omarchy plugin validate .config/omarchy/plugins/jax.seldon' && ok "omarchy plugin validate" || bad "omarchy plugin validate"
enabled=$(ipc shell listPlugins | jq -r '.[] | select(.id == "jax.seldon") | .enabled')
if [[ $enabled != true ]]; then
  rsh 'omarchy-shell shell rescanPlugins >/dev/null 2>&1; omarchy plugin enable jax.seldon' >/dev/null 2>&1 || true
  enabled=$(ipc shell listPlugins | jq -r '.[] | select(.id == "jax.seldon") | .enabled')
fi
eq "plugin enabled" "$enabled" true
old_pid=$(rsh "$remote_paths"$'\nshell_pid')
rsh "$remote_paths"$'\nsession_locked' && die "the test host's session locked during the run"
rsh "$remote_paths"$'\nsettle; omarchy-restart-shell' >/dev/null || die "omarchy-restart-shell"
pid=$(rsh "$remote_paths"$'\nshell_pid')
[[ -n $pid && $pid != "$old_pid" ]] && ok "shell restarted (pid $old_pid → $pid)" || bad "shell restarted (pid '$pid', was '$old_pid')"

step "plugin: service state after its start-up capture"
# The plugin runs capture + status at start; wait until that is done.
snap=$(wait_service '.ready and .engine == "present" and (.busy | not) and (.capturing | not) and .status == "ok"' 90) \
  || bad "service settled in status ok within 90 s (last: $(jq -c '{status,engine,busy,lastError}' <<<"${snap:-$empty}" 2>/dev/null))"
eq "service status" "$(jq -r .status <<<"${snap:-$empty}")" ok
eq "service lastError" "$(jq -r .lastError <<<"${snap:-$empty}")" ""
# The index may change between two reads (a capture cycle); compare against a
# stable one.
for _ in 1 2 3; do
  index=$(read_index)
  pill=$(ipc jax.seldon.panel pill)
  snap=$(ipc jax.seldon.service status)
  [[ $(read_index | jq -r .generatedAt) == "$(jq -r .generatedAt <<<"$index")" ]] && break
done
expected_pill=$(pill_of <<<"$index")
echo "   summary $(jq -c .summary <<<"$index") → pill '$expected_pill'"
keep plugin-index.json "$index"; keep service-status.json "$snap"; keep pill.json "$pill"
eq "panel pill text = index counts" "$(jq -r .text <<<"$pill")" "$expected_pill"
eq "service pill = index counts" "$(jq -r .pill <<<"$snap")" "$expected_pill"
want_tone=$(jq -r 'if .summary.crisis > 0 then "urgent" elif .summary.activeCases > 0 then "accent" else "default" end' <<<"$index")
eq "pill tone" "$(jq -r .tone <<<"$pill")" "$want_tone"

step "plugin: panel view"
ipc jax.seldon.panel open >/dev/null
ipc jax.seldon.panel tab today >/dev/null
view=$(wait_view '.opened == true and .tab == "today"' 10) || bad "panel open on Today"
keep view-today.json "$view"
eq "view status" "$(jq -r .status <<<"$view")" ok
eq "Today entries = index today.entries" "$(jq -r .today.entries <<<"$view")" "$(jq -r '.today.entries | length' <<<"$index")"
eq "index after the plugin's capture still has the note in today" "$(jq -r --arg t "$note" '[.today.entries[] | select(.text == $t)] | length' <<<"$index")" 1
eq "view banner" "$(jq -r .banner <<<"$view")" ""
ipc jax.seldon.panel tab changelog >/dev/null
eq "filter pacman" "$(ipc jax.seldon.panel filter pacman)" ok
view=$(wait_view '.tab == "changelog" and .changelog.filter == "pacman"' 10) || bad "Changelog filtered to pacman"
keep view-changelog-pacman.json "$view"
ge "Changelog rows (pacman)" "$(jq -r .changelog.rows <<<"$view")" 1
ipc jax.seldon.panel filter all >/dev/null
view=$(wait_view '.changelog.filter == "all"' 10) || true
keep view-changelog.json "$view"
ge "Changelog rows (all)" "$(jq -r .changelog.rows <<<"$view")" 1
echo "   changelog: $(jq -c '.changelog | {rows, folded, snapshots, badges: (.badges | length), drift: (.driftTones | length)}' <<<"$view"), crisis strip: '$(jq -r .crisis <<<"$view")', snapper: '$(jq -r .snapper <<<"$view")'"

step "plugin: QuickEntry through wtype"
ipc jax.seldon.panel tab today >/dev/null
entries_before=$(read_index | jq -r '.today.entries | length')
if rsh "$remote_paths"$'\nsession_locked'; then
  bad "QuickEntry: the session is locked; not typing into the lock screen"
elif rsh 'wtype n' && view=$(wait_view '.today.quickEntry.editing == true' 10); then
  ok "n focuses the QuickEntry"
  rsh "wtype -- $(printf '%q' "$qe_note") && sleep 0.3 && wtype -k Return"
  view=$(wait_view '.today.quickEntry.result | startswith("Saved")' 30) || bad "QuickEntry result (got '$(jq -r .today.quickEntry.result <<<"${view:-$empty}" 2>/dev/null)')"
  eq "QuickEntry result" "$(jq -r '.today.quickEntry.result | startswith("Saved")' <<<"${view:-$empty}")" true
  # The engine rebuilds the index after `log`; the FileView picks it up.
  want=$((entries_before + 1))
  view=$(wait_view ".today.entries == $want" 20) || true
  eq "Today entries after the FileView refresh" "$(jq -r .today.entries <<<"${view:-$empty}")" "$want"
  index=$(read_index)
  keep view-after-quickentry.json "$view"; keep index-after-quickentry.json "$index"
  eq "index today has the QuickEntry note" "$(jq -r --arg t "$qe_note" '[.today.entries[] | select(.text == $t)] | length' <<<"$index")" 1
  eq "ledger has the QuickEntry note" \
    "$(rsh "cat Seldon-e2e/ledger/*.jsonl" | jq -r --arg t "$qe_note" 'select(.kind == "note" and .detail == $t) | .detail' | wc -l)" 1
else
  bad "n focuses the QuickEntry (view: $(jq -c .today.quickEntry <<<"${view:-$empty}" 2>/dev/null)); not typing into an unknown window"
fi
ipc jax.seldon.panel close >/dev/null || true

step "shell log"
# Lines that name the plugin at WARN or ERROR level; nothing is expected with
# the engine present (docs/TESTING.md, smoke step 7). The log must be the
# shell's: an empty or foreign log would pass vacuously.
log=$(rsh "quickshell log --pid $pid 2>/dev/null" | sed 's/\x1b\[[0-9;]*m//g' || true)
if grep -a -q "Configuration Loaded" <<<"$log"; then
  ok "read the shell's log (pid $pid, $(wc -l <<<"$log") lines)"
else
  bad "read the shell's log (pid $pid): no 'Configuration Loaded' line"
fi
errs=$(grep -a -E "WARN|ERROR" <<<"$log" | grep -a -E "jax\.seldon|plugins/jax" || true)
[[ -z $errs ]] && ok "shell log clean of jax.seldon warnings and errors" \
  || { bad "shell log names jax.seldon"; echo "$errs" | head -n 10 | sed 's/^/     /'; }
crashes=$(rsh 'ls .cache/quickshell/crashes 2>/dev/null | wc -l')
eq "shell crash reports (no crash during the run)" "$crashes" "$(sed -n 's/^shell crash reports: //p' <<<"$before")"

trap - EXIT
restore_and_verify
summary
((fail == 0))
