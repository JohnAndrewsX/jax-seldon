# Sourced by the plugin harness scripts (service-states.sh, desk-view.sh,
# bar-view.sh): proves that a run never touches the real
# user's Seldon files. Every scenario runs with
# HOME, XDG_STATE_HOME and XDG_CONFIG_HOME inside the run's temp dir; this
# fingerprints the real paths before the run, and `real_home_check` compares
# them after it (existence, type, size, mtime and ctime of every entry).
#
# A difference fails the run, with one exception: the operator's own engine
# (a dev host where the installed plugin runs `seldon capture` every 15
# minutes) rewrites its state files while a run is under way. That passes
# only when all of this holds:
#   - nothing under ~/.config/seldon changed and config.toml is
#     byte-identical;
#   - no path appeared or disappeared;
#   - every changed entry is the state dir itself or one of its index.json,
#     lock, cursors.json, manifest.json;
#   - the post-run index.json names the logbook config.toml names
#     (`logbook.path`) and the same machine as before the run
#     (`logbook.machine`).
# Only config.toml and those two header fields of the state index are read;
# never the logbook itself. Anything else (a test leak writing an index for
# another logbook, a new file, a config change) still fails; the message
# says which path changed, not who changed it.
#
# It also guards the session's runtime dir (WP-161): every Quickshell a
# harness starts must get a private XDG_RUNTIME_DIR, since Quickshell leaves
# a quickshell/by-id/<id> dir behind for every instance and a full
# /run/user/<uid> takes the desktop down. The entries of quickshell/by-id
# in /run/user/<uid>, and in the inherited XDG_RUNTIME_DIR when that is
# another dir, are listed before the run; a new one fails the run unless a
# running process holds a file in it (a live instance: another Quickshell
# app or a restarted shell started during the run). A harness's own
# Quickshells have exited by then, so what they leave is held by nobody;
# so is the leftover of another test run on an old harness, which fails
# here too.

real_home=$HOME
# Only Seldon's own dirs: files a fake engine of an older harness version
# leaves in HOME (calls.log) may change while other worktrees run their tests.
real_state="$real_home/.local/state/seldon"
real_config="$real_home/.config/seldon"
real_paths=("$real_state" "$real_config")
# The state files the operator's engine rewrites on every capture.
real_engine_files=(index.json lock cursors.json manifest.json)

real_fingerprint() {
  local p
  for p in "${real_paths[@]}"; do
    if [[ -e $p || -L $p ]]; then
      find "$p" -printf '%p %y %s %T@ %C@\n' 2>/dev/null | LC_ALL=C sort
    else
      echo "$p absent"
    fi
  done
}

# The logbook config.toml names (`logbook = "…"`, a leading ~ expanded), or
# nothing.
real_config_logbook() {
  local line
  [[ -f $real_config/config.toml ]] || return 0
  line=$(grep -m1 -E '^[[:space:]]*logbook[[:space:]]*=' "$real_config/config.toml" || true)
  line=${line#*=}
  line=$(sed -E 's/^[[:space:]]*"([^"]*)".*$/\1/' <<<"$line")
  [[ $line == "~/"* ]] && line="$real_home/${line#\~/}"
  printf '%s' "$line"
}

# One header field of the real state index, or nothing.
real_index_field() {
  [[ -f $real_state/index.json ]] && command -v jq >/dev/null || return 0
  jq -r "$1 // empty" "$real_state/index.json" 2>/dev/null || true
}

real_config_sum() {
  [[ -f $real_config/config.toml ]] && sha256sum "$real_config/config.toml" | cut -d' ' -f1
  return 0
}

# The runtime dirs whose quickshell/by-id is watched; a test of this guard
# sets real_runtime_session to a scratch dir before sourcing it.
real_runtime_dirs=("${real_runtime_session:-/run/user/$UID}") # live runtime dir: read only, watched for leaks
if [[ -n ${XDG_RUNTIME_DIR:-} && $XDG_RUNTIME_DIR != "${real_runtime_dirs[0]}" ]]; then # live runtime dir: read only, watched for leaks
  real_runtime_dirs+=("$XDG_RUNTIME_DIR") # live runtime dir: read only, watched for leaks
fi

# The entry names under <runtime dir>/quickshell/by-id, sorted; nothing
# when it does not exist.
real_runtime_entries() {
  [[ -d $1/quickshell/by-id ]] || return 0
  find "$1/quickshell/by-id" -mindepth 1 -maxdepth 1 -printf '%f\n' 2>/dev/null | LC_ALL=C sort
}

declare -gA real_runtime_before=()
for real_dir in "${real_runtime_dirs[@]}"; do
  real_runtime_before[$real_dir]=$(real_runtime_entries "$real_dir")
done
unset real_dir

real_before=$(real_fingerprint)
real_before_config_sum=$(real_config_sum)
real_before_logbook=$(real_config_logbook)
real_before_machine=$(real_index_field .logbook.machine)

# Empty when the difference is the operator's engine at work (see the top),
# otherwise why it is not.
real_engine_change() {
  local after=$1 path line allowed f
  [[ -n $real_before_logbook ]] || { echo "no logbook in ~/.config/seldon/config.toml before the run"; return; }
  [[ $(real_config_sum) == "$real_before_config_sum" ]] || { echo "config.toml changed"; return; }
  # The same paths before and after.
  if [[ $(cut -d' ' -f1 <<<"$real_before") != $(cut -d' ' -f1 <<<"$after") ]]; then
    echo "a path appeared or disappeared"
    return
  fi
  # Every changed entry is the state dir or one of the engine's files.
  while IFS= read -r line; do
    path=${line%% *}
    allowed=0
    [[ $path == "$real_state" ]] && allowed=1
    for f in "${real_engine_files[@]}"; do [[ $path == "$real_state/$f" ]] && allowed=1; done
    ((allowed)) || { echo "$path changed"; return; }
  done < <(LC_ALL=C comm -13 <(LC_ALL=C sort <<<"$real_before") <(LC_ALL=C sort <<<"$after"))
  [[ $(real_index_field .logbook.path) == "$real_before_logbook" ]] \
    || { echo "index.json names another logbook"; return; }
  [[ -n $real_before_machine && $(real_index_field .logbook.machine) == "$real_before_machine" ]] \
    || { echo "index.json names another machine"; return; }
}

# real_runtime_live <runtime dir> <entry> — true when a running process
# holds a file under <runtime dir>/quickshell/by-id/<entry> open. find
# exits 1 on the /proc entries of other users; only its output counts.
real_runtime_live() {
  [[ -n $(find /proc/[0-9]*/fd -lname "$1/quickshell/by-id/$2/*" -print -quit 2>/dev/null || true) ]]
}

# real_runtime_check <script name> — one pass/fail line per watched runtime
# dir, counted in $pass/$fail.
real_runtime_check() {
  local dir before after entry count_before count_after live left note
  for dir in "${real_runtime_dirs[@]}"; do
    before=${real_runtime_before[$dir]}
    after=$(real_runtime_entries "$dir")
    count_before=$(grep -c . <<<"$before" || true)
    count_after=$(grep -c . <<<"$after" || true)
    live=() left=()
    while IFS= read -r entry; do
      [[ -n $entry ]] || continue
      if real_runtime_live "$dir" "$entry"; then live+=("$entry"); else left+=("$entry"); fi
    done < <(LC_ALL=C comm -13 <(echo "$before") <(echo "$after"))
    note=""
    ((${#live[@]} == 0)) || note="; ${#live[@]} new held by a running process, not a leftover: ${live[*]}"
    if ((${#left[@]} == 0)); then
      pass=$((pass + 1))
      echo "ok   $1: no leftover in $dir/quickshell/by-id ($count_before before, $count_after after$note)"
    else
      fail=$((fail + 1))
      echo "FAIL $1: new entries no process holds in $dir/quickshell/by-id ($count_before before, $count_after after$note): a Quickshell ran in this runtime dir and left them:"
      printf '     %s\n' "${left[@]}"
    fi
  done
}

# real_home_check <script name> — one pass/fail line for the real home,
# then real_runtime_check; counted in $pass/$fail.
real_home_check() {
  real_home_files_check "$1"
  real_runtime_check "$1"
}

real_home_files_check() {
  local after why
  after=$(real_fingerprint)
  if [[ $after == "$real_before" ]]; then
    pass=$((pass + 1))
    echo "ok   $1: the real ~/.local/state/seldon and ~/.config/seldon are untouched"
    return
  fi
  why=$(real_engine_change "$after")
  if [[ -z $why ]]; then
    pass=$((pass + 1))
    echo "ok   $1: the real ~/.local/state/seldon changed by the operator's live engine (not a leak)"
  else
    fail=$((fail + 1))
    echo "FAIL $1: real paths changed during the run ($why; before < > after):"
    diff <(echo "$real_before") <(echo "$after") | sed 's/^/     /' || true
  fi
}
