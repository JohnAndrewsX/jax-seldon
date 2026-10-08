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
#   - any mention of the name XDG_RUNTIME_DIR other than an assignment
#     (an expansion with or without a default, `printenv XDG_RUNTIME_DIR`,
#     `v=XDG_RUNTIME_DIR` for a `${!v}`) and any /run/user path, unless the
#     line carries the marker `# live runtime dir: <reason>` (a remote
#     prelude that drives the test host's live shell, the leak guard that
#     only reads it);
#   - a Quickshell ("$qs_bin", ${qs_bin}, quickshell, qs) started with
#     -p/--path whose command (continuation lines joined) does not set
#     XDG_RUNTIME_DIR, itself or through an array expanded there
#     ("${envs[@]}") whose definition sets it;
#   - a file that makes a private runtime dir (`rt=$(mktemp …`) without an
#     EXIT trap that names "$rt", itself or in the body of the function it
#     calls (`trap cleanup EXIT`).
# It also proves that the engine never reads XDG_RUNTIME_DIR (engine and
# cargo tests inherit the session's). Then it proves itself on mutants:
# each of a set of copies with the old pattern brought back, the setting
# dropped, a bypass spelling, the trap or the marker removed must fail for
# the right reason; the unchanged copies must pass.
set -euo pipefail

root=$(cd "$(dirname "$0")/../.." && pwd)
self="tests/plugin/runtime-dir.test.sh"
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

pass=0
fail=0

# indent <text> — each line of <text> indented under a FAIL line.
indent() {
  local l
  while IFS= read -r l; do printf '     %s\n' "$l"; done <<<"$1"
}

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
      function flush(   k, set) {
        set = cmd ~ /XDG_RUNTIME_DIR=/
        for (k in xdgarr) if (index(cmd, "${" k "[@]}")) set = 1
        if (cmd ~ start && !set)
          printf "%s:%d: Quickshell started without its own XDG_RUNTIME_DIR\n", f, first
        cmd = ""; first = 0
      }
      # The name before the first "=(" of a line: an array definition.
      function arrayname(line,   n) {
        n = substr(line, 1, index(line, "=(") - 1)
        sub(/^.*[[:space:]]/, "", n)
        return n ~ /^[A-Za-z_][A-Za-z0-9_]*$/ ? n : ""
      }
      # A mention of the name that is not an assignment NAME=… (after the
      # line start, a blank, a quote or ; ( & |).
      function reads(line,   rest, i, before) {
        rest = line
        while ((i = index(rest, "XDG_RUNTIME_DIR")) > 0) {
          before = i > 1 ? substr(rest, i - 1, 1) : ""
          if (substr(rest, i + 15, 1) != "=" || before !~ /^([[:space:]"'\'';(&|]|)$/) return 1
          rest = substr(rest, i + 15)
        }
        return 0
      }
      BEGIN {
        start = "(\\$\\{?qs_bin\\}?|(^|[[:space:]/\"])(quickshell|qs))\"?([[:space:]]+-[-a-z]+)*[[:space:]]+(-p|--path)([[:space:]=]|$)"
      }
      {
        comment = $0 ~ /^[[:space:]]*#/
        if (!comment && $0 !~ /# live runtime dir: / && (reads($0) || index($0, "/run/user")))
          printf "%s:%d: the session runtime dir without the \"# live runtime dir:\" marker\n", f, FNR
        if (!comment && $0 ~ /(^|[^[:alnum:]_])rt=\$\(mktemp/) { made = FNR }
        if (!comment && $0 ~ /^[[:space:]]*trap / && $0 ~ /EXIT/) {
          if (index($0, "\"$rt\"")) trapped = 1
          h = $0
          sub("^[[:space:]]*trap[[:space:]]+\047?", "", h)
          sub("\047?[[:space:]].*$", "", h)
          if (h ~ /^[A-Za-z_][A-Za-z0-9_]*$/) handler[h] = 1
        }
        # Function bodies (a one-liner or up to a "}" line) that name "$rt".
        if (!comment && $0 ~ /^[[:space:]]*[A-Za-z_][A-Za-z0-9_]*[[:space:]]*\(\)[[:space:]]*\{/) {
          fn = $0
          sub(/^[[:space:]]*/, "", fn)
          sub(/[[:space:]]*\(.*$/, "", fn)
        }
        if (fn != "" && !comment && index($0, "\"$rt\"")) fnrt[fn] = 1
        if (fn != "" && $0 ~ /}[[:space:]]*$/) fn = ""
        # Arrays whose definition sets XDG_RUNTIME_DIR (up to the line with ")").
        if (!comment && arr == "" && index($0, "=(") && (arr = arrayname($0)) != "")
          rest = substr($0, index($0, "=(") + 2)
        else
          rest = $0
        if (arr != "") {
          if (!comment && index(rest, "XDG_RUNTIME_DIR=")) xdgarr[arr] = 1
          if (index(rest, ")")) arr = ""
        }
        if (comment) next
        if (!first) first = FNR
        cmd = cmd " " $0
        if ($0 !~ /\\$/) flush()
      }
      END {
        flush()
        for (h in handler) if (fnrt[h]) trapped = 1
        if (made && !trapped)
          printf "%s:%d: private runtime dir not removed by an EXIT trap naming \"$rt\"\n", f, made
      }' "$file" >>"$work/scan.out"
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
  indent "$out"
fi

# The engine and the cargo tests inherit the session's XDG_RUNTIME_DIR;
# harmless only while the engine never reads it.
if engine=$(grep -rln --include='*.rs' --exclude-dir=target XDG_RUNTIME_DIR "$root/engine"); then
  fail=$((fail + 1))
  echo "FAIL the engine reads XDG_RUNTIME_DIR (its tests inherit the session's):"
  indent "$engine"
else
  pass=$((pass + 1))
  echo "ok   the engine never reads XDG_RUNTIME_DIR"
fi

# 2. Mutants. mutant <name> <want> <file> <sed script>; want is clean, or
# the finding it must be caught by: session (the session runtime dir
# without the marker), unset (Quickshell without its own runtime dir) or
# trap (the private dir not removed).
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
      session) grep -q ': the session runtime dir without the' <<<"$out" && got="session" ;;
      unset) grep -q ': Quickshell started without its own XDG_RUNTIME_DIR$' <<<"$out" && got="unset" ;;
      trap) grep -q ': private runtime dir not removed by an EXIT trap' <<<"$out" && got="trap" ;;
    esac
  fi
  if [[ $got == "$want" ]]; then
    pass=$((pass + 1))
    echo "ok   mutant $name: $want"
  else
    fail=$((fail + 1))
    echo "FAIL mutant $name: $got (want $want)"
    indent "$out"
  fi
}

old='XDG_RUNTIME_DIR="$''{XDG_RUNTIME_DIR:-$rt}"'
for f in tests/plugin/panel-view.sh tests/plugin/overlay-view.sh tests/plugin/bar-view.sh \
  tests/plugin/service-states.sh tests/plugin/ipc-restart.sh tests/integration/e2e.sh; do
  name=$(basename "$f" .sh)
  mutant "$name" clean "$f" ''
  mutant "$name-default-back" session "$f" "s|XDG_RUNTIME_DIR=\"\$rt\"|$old|"
  mutant "$name-session-passed" session "$f" 's|XDG_RUNTIME_DIR="$rt"|XDG_RUNTIME_DIR="$XDG_RUNTIME_DIR"|'
done
mutant service-states-unset unset tests/plugin/service-states.sh 's|XDG_RUNTIME_DIR="$rt" ||'
mutant e2e-unset unset tests/integration/e2e.sh 's| XDG_RUNTIME_DIR="$rt" \\$| \\|'
mutant panel-view-unset unset tests/plugin/panel-view.sh '/^    XDG_RUNTIME_DIR="$rt" \\$/d'
mutant bar-view-real-dir session tests/plugin/bar-view.sh 's|XDG_RUNTIME_DIR="$rt"|XDG_RUNTIME_DIR=/run/user/1000|'
# Other spellings of the start, the setting dropped.
drop='/^    XDG_RUNTIME_DIR="$rt" \\$/d'
braces='"${qs_bin}" -p'
mutant panel-view-braces unset tests/plugin/panel-view.sh "s|\"\$qs_bin\" -p|$braces|; $drop"
mutant panel-view-unquoted unset tests/plugin/panel-view.sh "s|\"\$qs_bin\" -p|\$qs_bin -p|; $drop"
mutant panel-view-path unset tests/plugin/panel-view.sh "s|\"\$qs_bin\" -p |\"\$qs_bin\" --path |; $drop"
mutant panel-view-path-eq unset tests/plugin/panel-view.sh "s|\"\$qs_bin\" -p |\"\$qs_bin\" --path=|; $drop"
mutant panel-view-quickshell unset tests/plugin/panel-view.sh "s|\"\$qs_bin\" -p|quickshell -p|; $drop"
# Other ways to pass the session's dir on.
mutant service-states-printenv session tests/plugin/service-states.sh 's|XDG_RUNTIME_DIR="$rt"|XDG_RUNTIME_DIR="$(printenv XDG_RUNTIME_DIR)"|'
mutant overlay-view-indirect session tests/plugin/overlay-view.sh 's|^chmod 700 "$rt"$|&\nrv=XDG_RUNTIME_DIR|; s|XDG_RUNTIME_DIR="$rt" \\$|XDG_RUNTIME_DIR="${!rv}" \\|'
mutant overlay-view-default-assign session tests/plugin/overlay-view.sh 's|^chmod 700 "$rt"$|: "${XDG_RUNTIME_DIR:=$rt}"|'
# The private dir left behind.
mutant bar-view-trap trap tests/plugin/bar-view.sh 's|; rm -rf "$rt"||'
mutant panel-view-trap trap tests/plugin/panel-view.sh "s|^trap 'rm -rf \"\$work\" \"\$rt\"' EXIT|trap 'rm -rf \"\$work\"' EXIT|"
# ipc-restart.sh (WP-162) passes the dir through its env array and removes
# it in its cleanup function.
mutant ipc-restart-unset unset tests/plugin/ipc-restart.sh 's| XDG_RUNTIME_DIR="$rt"||'
mutant ipc-restart-trap trap tests/plugin/ipc-restart.sh '/^  \[\[ -z \$rt \]\] || rm -rf "\$rt"$/d'
mutant ipc-restart-handler trap tests/plugin/ipc-restart.sh 's|^trap cleanup EXIT$|trap other EXIT|'
mutant e2e-trap trap tests/integration/e2e.sh "s|trap 'rm -rf \"\$work\" \"\$rt\"' EXIT|trap 'rm -rf \"\$work\"' EXIT|"
for f in scripts/deploy-test-host.sh tests/plugin/real-home-guard.sh; do
  name=$(basename "$f" .sh)
  mutant "$name" clean "$f" ''
  mutant "$name-marker-dropped" session "$f" 's| # live runtime dir: .*$||'
done

echo "runtime-dir.test: $pass passed, $fail failed"
((fail == 0))
