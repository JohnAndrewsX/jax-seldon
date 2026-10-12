#!/usr/bin/env bash
# Self-test of `just check-perf`'s tmpfs directory (WP-210, WP-216): the
# recipe runs with a stub cargo and a scratch parent for its directory
# (SELDON_PERF_SHM, instead of /dev/shm). A finished run removes the
# directory; one interrupted by SIGINT or SIGTERM to its process group
# removes it too and exits 130 or 143; one killed with SIGKILL leaves it,
# and the next run sweeps this user's seldon-perf.* directories older than
# a day and nothing else. Needs just and setsid.
set -euo pipefail

root=$(cd "$(dirname "$0")/../.." && pwd)
just_bin=$(command -v just) || { echo "check-perf.test: just not found" >&2; exit 1; }
command -v setsid >/dev/null || { echo "check-perf.test: setsid not found" >&2; exit 1; }
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
mkdir -p "$work/bin" "$work/home" "$work/rt"
chmod 700 "$work/rt"

# `cargo bench` passes; `cargo test` writes the directory it was given to
# $STUB_DIR_FILE, then passes, or with STUB_HOLD waits to be signalled.
cat >"$work/bin/cargo" <<'STUB'
#!/usr/bin/env bash
[[ $1 == test ]] || exit 0
printf '%s\n' "$SELDON_PERF_TMPFS" >"$STUB_DIR_FILE.part"
mv "$STUB_DIR_FILE.part" "$STUB_DIR_FILE"
[[ -n ${STUB_HOLD:-} ]] || exit 0
exec sleep 60
STUB
chmod 755 "$work/bin/cargo"

pass=0
fail=0

# check <name> <got> <want>
check() {
  if [[ $2 == "$3" ]]; then
    pass=$((pass + 1)); echo "ok   $1: $3"
  else
    fail=$((fail + 1)); echo "FAIL $1: $2 (want $3)"
    [[ ! -f $work/$case_name.log ]] || sed 's/^/     /' "$work/$case_name.log"
  fi
}

# run_recipe <case> <parent dir> [signal] — `just check-perf` in its own
# process group; with a signal, sent to that group once the stub cargo
# test holds the directory. Sets $code and $dir (the directory the run
# made, "" when none).
run_recipe() {
  local sig=${3:-} pid pgid i hold=()
  case_name=$1
  [[ -z $sig ]] || hold=(STUB_HOLD=1)
  env HOME="$work/home" XDG_RUNTIME_DIR="$work/rt" PATH="$work/bin:$PATH" \
    SELDON_PERF_SHM="$2" STUB_DIR_FILE="$work/$1.dir" "${hold[@]}" \
    setsid "$just_bin" --justfile "$root/justfile" --working-directory "$root" check-perf \
    >"$work/$1.log" 2>&1 </dev/null &
  pid=$!
  if [[ -n $sig ]]; then
    for ((i = 0; i < 200; i++)); do [[ -s $work/$1.dir ]] && break; sleep 0.05; done
    pgid=$(ps -o pgid= -p "$pid" | tr -d ' ')
    kill -s "$sig" -- "-$pgid" 2>/dev/null || true
  fi
  code=0
  { wait "$pid"; } 2>/dev/null || code=$? # no "Killed" notice
  dir=""
  [[ ! -s $work/$1.dir ]] || dir=$(<"$work/$1.dir")
}

# A run that passes: the directory is under the parent, and gone after.
mkdir -p "$work/shm-pass"
run_recipe pass "$work/shm-pass"
check pass-exit "$code" 0
dir_name=${dir#"$work/"}
check pass-dir "${dir_name%.*}" shm-pass/seldon-perf
check pass-removed "$([[ -e $dir ]] && echo left || echo removed)" removed

# Interrupted: SIGINT exits 130, SIGTERM 143, the directory removed.
for sig in INT TERM; do
  mkdir -p "$work/shm-$sig"
  run_recipe "sig-$sig" "$work/shm-$sig" "$sig"
  want=130
  [[ $sig == INT ]] || want=143
  check "sig-$sig-exit" "$code" "$want"
  check "sig-$sig-made" "$([[ -n $dir ]] && echo made || echo none)" made
  check "sig-$sig-removed" "$([[ -n $dir && ! -e $dir ]] && echo removed || echo left)" removed
done

# SIGKILL runs no trap: the directory stays; the next run sweeps it once
# it is older than a day, with any other stale one of this user's, and
# keeps a young one (a run in progress) and every other name or type.
mkdir -p "$work/shm-kill"
run_recipe kill "$work/shm-kill" KILL
check kill-exit "$code" 137
check kill-left "$([[ -n $dir && -d $dir ]] && echo left || echo gone)" left
killed=$dir
check kill-young-kept "$(run_recipe kill-next "$work/shm-kill"; [[ -d $killed ]] && echo kept || echo swept)" kept
mkdir "$work/shm-kill/seldon-perf.stale1" "$work/shm-kill/seldon-perf.young" "$work/shm-kill/other.stale"
: >"$work/shm-kill/seldon-perf.file"
touch -d '25 hours ago' "$killed" "$work/shm-kill/seldon-perf.stale1" "$work/shm-kill/other.stale" "$work/shm-kill/seldon-perf.file"
touch -d '23 hours ago' "$work/shm-kill/seldon-perf.young"
run_recipe sweep "$work/shm-kill"
check sweep-exit "$code" 0
left=$(find "$work/shm-kill" -mindepth 1 -maxdepth 1 -printf '%f\n' | LC_ALL=C sort | paste -sd' ')
check sweep-left "$left" "other.stale seldon-perf.file seldon-perf.young"

echo "check-perf.test: $pass passed, $fail failed"
((fail == 0))
