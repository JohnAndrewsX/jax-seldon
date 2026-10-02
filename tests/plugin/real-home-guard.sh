# Sourced by tests/plugin/service-states.sh and panel-view.sh: proves that a
# run never touches the real user's Seldon files. Every scenario runs with
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

# real_home_check <script name> — one pass/fail line, counted in $pass/$fail.
real_home_check() {
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
