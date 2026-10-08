#!/usr/bin/env bash
# The mutants' sed scripts spell shell expansions literally.
# shellcheck disable=SC2016
# Static guard (WP-161): no test or script reaches the session's runtime dir
# by accident. On 2026-10-08 the plugin harnesses passed
# XDG_RUNTIME_DIR="${XDG_RUNTIME_DIR:-$work}" to their Quickshells; in a
# desktop session that is the real one, Quickshell leaves a
# quickshell/by-id/<id> dir behind for every instance, and tens of
# thousands of runs filled it until Hyprland died.
#
# Under tests/ and scripts/, outside comments, it fails on
#   - an expansion of XDG_RUNTIME_DIR (with or without a default) and any
#     /run/user path, unless the line carries the marker
#     `# live runtime dir: <reason>` (a remote prelude that drives the test
#     host's live shell, the leak guard that only reads it);
#   - a Quickshell started with -p/--path whose command (continuation lines
#     joined) does not set XDG_RUNTIME_DIR.
# Then it proves itself on mutants: each of a set of copies with the old
# pattern brought back, the setting dropped or the marker removed must fail;
# the unchanged copies must pass.
set -euo pipefail

root=$(cd "$(dirname "$0")/../.." && pwd)
self="tests/plugin/runtime-dir.test.sh"
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

pass=0
fail=0

# scan <base> <dir>... — every finding as "<path>:<line>: <why>", paths
# relative to <base>; exit 1 when there is one. This file is left out (its
# mutants spell the old pattern).
scan() {
  local base=$1 file found=0
  shift
  while IFS= read -r -d '' file; do
    [[ ${file#"$base"/} == "$self" ]] && continue
    grep -Iq . "$file" || continue
    awk -v f="${file#"$base"/}" '
      function flush() {
        if (cmd ~ start && cmd !~ /XDG_RUNTIME_DIR=/)
          printf "%s:%d: Quickshell started without its own XDG_RUNTIME_DIR\n", f, first
        cmd = ""; first = 0
      }
      BEGIN {
        start = "(\"\\$qs_bin\"|(^|[[:space:]/])(quickshell|qs))([[:space:]]+-[-a-z]+)*[[:space:]]+(-p|--path)([[:space:]]|$)"
      }
      {
        if ($0 !~ /^[[:space:]]*#/ && $0 !~ /# live runtime dir: / &&
            ($0 ~ /\$\{?XDG_RUNTIME_DIR/ || index($0, "/run/user")))
          printf "%s:%d: the session runtime dir without the \"# live runtime dir:\" marker\n", f, FNR
        if ($0 ~ /^[[:space:]]*#/) next
        if (!first) first = FNR
        cmd = cmd " " $0
        if ($0 !~ /\\$/) flush()
      }
      END { flush() }' "$file" >>"$work/scan.out"
  done < <(find "$@" -type f -print0 | LC_ALL=C sort -z)
  [[ -s $work/scan.out ]] && found=1
  cat "$work/scan.out"
  rm -f "$work/scan.out"
  ((found == 0))
}

# 1. The repository.
if out=$(scan "$root" "$root/tests" "$root/scripts"); then
  pass=$((pass + 1))
  echo "ok   tests/ and scripts/ keep Quickshell out of the session's runtime dir"
else
  fail=$((fail + 1))
  echo "FAIL tests/ and scripts/ reach the session's runtime dir:"
  echo "$out" | sed 's/^/     /'
fi

# 2. Mutants. mutant <name> <want> <file> <sed script>; want is clean, or
# the finding it must be caught by: session (the session runtime dir
# without the marker) or unset (Quickshell without its own runtime dir).
mutant() {
  local name=$1 want=$2 file=$3 script=$4 dir="$work/m-$1" out got
  mkdir -p "$dir/$(dirname "$file")"
  sed "$script" "$root/$file" >"$dir/$file"
  if [[ $want != clean ]] && cmp -s "$root/$file" "$dir/$file"; then
    fail=$((fail + 1))
    echo "FAIL mutant $name: the sed script changed nothing in $file"
    return
  fi
  got=clean
  if ! out=$(scan "$dir" "$dir"); then
    got="caught (other)"
    case $want in
      session) grep -q ': the session runtime dir without the' <<<"$out" && got=session ;;
      unset) grep -q ': Quickshell started without its own XDG_RUNTIME_DIR$' <<<"$out" && got=unset ;;
    esac
  fi
  if [[ $got == "$want" ]]; then
    pass=$((pass + 1))
    echo "ok   mutant $name: $want"
  else
    fail=$((fail + 1))
    echo "FAIL mutant $name: $got (want $want)"
    echo "$out" | sed 's/^/     /'
  fi
}

old='XDG_RUNTIME_DIR="$''{XDG_RUNTIME_DIR:-$rt}"'
for f in tests/plugin/panel-view.sh tests/plugin/overlay-view.sh tests/plugin/bar-view.sh \
  tests/plugin/service-states.sh tests/integration/e2e.sh; do
  name=$(basename "$f" .sh)
  mutant "$name" clean "$f" ''
  mutant "$name-default-back" session "$f" "s|XDG_RUNTIME_DIR=\"\$rt\"|$old|"
  mutant "$name-session-passed" session "$f" 's|XDG_RUNTIME_DIR="$rt"|XDG_RUNTIME_DIR="$XDG_RUNTIME_DIR"|'
done
mutant service-states-unset unset tests/plugin/service-states.sh 's|XDG_RUNTIME_DIR="$rt" ||'
mutant e2e-unset unset tests/integration/e2e.sh 's| XDG_RUNTIME_DIR="$rt" \\$| \\|'
mutant panel-view-unset unset tests/plugin/panel-view.sh '/^    XDG_RUNTIME_DIR="$rt" \\$/d'
mutant bar-view-real-dir session tests/plugin/bar-view.sh 's|XDG_RUNTIME_DIR="$rt"|XDG_RUNTIME_DIR=/run/user/1000|'
for f in scripts/deploy-test-host.sh tests/plugin/real-home-guard.sh; do
  name=$(basename "$f" .sh)
  mutant "$name" clean "$f" ''
  mutant "$name-marker-dropped" session "$f" 's| # live runtime dir: .*$||'
done

echo "runtime-dir.test: $pass passed, $fail failed"
((fail == 0))
