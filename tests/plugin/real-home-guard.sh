# Sourced by tests/plugin/service-states.sh and panel-view.sh: proves that a
# run never touches the real user's Seldon files. Every scenario runs with
# HOME, XDG_STATE_HOME and XDG_CONFIG_HOME inside the run's temp dir; this
# fingerprints the real paths before the run, and `real_home_check` compares
# them after it (existence, type, size, mtime and ctime of every entry).
#
# A difference fails the run. It may come from another process on the
# machine (an engine run by hand at the same time); the message says which
# path changed, not who changed it.

real_home=$HOME
# Only Seldon's own dirs: files a fake engine of an older harness version
# leaves in HOME (calls.log) may change while other worktrees run their tests.
real_paths=(
  "$real_home/.local/state/seldon"
  "$real_home/.config/seldon"
)

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

real_before=$(real_fingerprint)

# real_home_check <script name> — one pass/fail line, counted in $pass/$fail.
real_home_check() {
  local after
  after=$(real_fingerprint)
  if [[ $after == "$real_before" ]]; then
    pass=$((pass + 1))
    echo "ok   $1: the real ~/.local/state/seldon and ~/.config/seldon are untouched"
  else
    fail=$((fail + 1))
    echo "FAIL $1: real paths changed during the run (before < > after):"
    diff <(echo "$real_before") <(echo "$after") | sed 's/^/     /'
  fi
}
