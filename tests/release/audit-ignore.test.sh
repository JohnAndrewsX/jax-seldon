#!/usr/bin/env bash
# packaging/audit-ignore.sh: the advisory ids cargo audit may skip, taken
# from the reviewed list packaging/audit-ignore.txt. Runs in
# `just check-packaging`; no network, no cargo.
#
# Usage: bash tests/release/audit-ignore.test.sh
set -euo pipefail

root=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
script=$root/packaging/audit-ignore.sh
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
fails=0
today=2026-10-03

pass() { echo "ok   $1"; }
fail() { echo "FAIL $1" >&2; fails=$((fails + 1)); }

# list NAME LINES...: write a list file, one argument per line
list() {
  local name=$1
  shift
  printf '%s\n' "$@" > "$tmp/$name"
}

# expect_ids NAME FILE "ID ID…": exit 0 and exactly these ids, in order
expect_ids() {
  local name=$1 file=$2 want=$3
  if bash "$script" "$tmp/$file" "$today" > "$tmp/out" 2> "$tmp/err" \
    && [[ $(tr '\n' ' ' < "$tmp/out") == "${want:+$want }" ]]; then
    pass "$name"
  else
    fail "$name: got '$(tr '\n' ' ' < "$tmp/out")'"
    cat "$tmp/err" >&2
  fi
}

# expect_fail NAME PATTERN ARGS...: exit 1, nothing on stdout, PATTERN on stderr
expect_fail() {
  local name=$1 pattern=$2 rc=0
  shift 2
  bash "$script" "$@" > "$tmp/out" 2> "$tmp/err" || rc=$?
  if [[ $rc -eq 1 && ! -s $tmp/out ]] && grep -q -- "$pattern" "$tmp/err"; then
    pass "$name"
  else
    fail "$name (exit $rc)"
    cat "$tmp/out" "$tmp/err" >&2
  fi
}

# --- the real list: valid today, and the default path ---
if bash "$script" "$root/packaging/audit-ignore.txt" > "$tmp/real" 2> "$tmp/err"; then
  pass "packaging/audit-ignore.txt is valid today"
else
  fail "packaging/audit-ignore.txt: $(cat "$tmp/err")"
fi
# no arguments, from another directory: the list next to the script
list one 'RUSTSEC-2024-0099 2099-01-01 never read'
mkdir "$tmp/pkg"
cp "$script" "$tmp/pkg/"
cp "$tmp/one" "$tmp/pkg/audit-ignore.txt"
if (cd "$tmp" && bash "$script") | cmp -s - "$tmp/real" \
  && ! (cd "$tmp/pkg" && bash "$tmp/pkg/audit-ignore.sh" > /dev/null 2>&1); then
  pass "default path is the list next to the script"
else
  fail "default path"
fi

# --- accepted entries ---
list empty '# only a comment' '' '   '
expect_ids "comments and blank lines only: no ids" empty ""
list two \
  '# header' \
  'RUSTSEC-2024-0001  2027-01-31  not reachable: the crate is only used in tests' \
  '' \
  "RUSTSEC-2025-0123	2026-10-04	tab-separated, expires tomorrow"
expect_ids "two entries, in file order" two "RUSTSEC-2024-0001 RUSTSEC-2025-0123"
printf 'RUSTSEC-2024-0002 2027-01-01 no newline at the end' > "$tmp/nonl"
expect_ids "last line without a newline" nonl "RUSTSEC-2024-0002"
list year 'RUSTSEC-2024-0003 2027-10-03 exactly 365 days ahead'
expect_ids "expiry 365 days ahead" year "RUSTSEC-2024-0003"

# --- rejected entries: the gate stays closed ---
list today 'RUSTSEC-2024-0004 2026-10-03 expires today'
expect_fail "expiry today" "expired on 2026-10-03" "$tmp/today" "$today"
list past 'RUSTSEC-2024-0005 2026-01-01 long gone'
expect_fail "expiry in the past" "expired on 2026-01-01" "$tmp/past" "$today"
list far 'RUSTSEC-2024-0006 2027-10-04 one day too far'
expect_fail "expiry more than 365 days ahead" "more than 365 days" "$tmp/far" "$today"
list noreason 'RUSTSEC-2024-0007 2027-01-01'
expect_fail "no reason" "RUSTSEC-2024-0007: no reason" "$tmp/noreason" "$today"
list noexpiry 'RUSTSEC-2024-0008'
expect_fail "no expiry" "is not an expiry date" "$tmp/noexpiry" "$today"
list baddate 'RUSTSEC-2024-0009 2027-02-30 no such day'
expect_fail "impossible date" "'2027-02-30' is not an expiry date" "$tmp/baddate" "$today"
list shortdate 'RUSTSEC-2024-0010 2027-2-1 not ISO'
expect_fail "date not YYYY-MM-DD" "'2027-2-1' is not an expiry date" "$tmp/shortdate" "$today"
list reasonfirst 'RUSTSEC-2024-0011 because 2027-01-01'
expect_fail "fields out of order" "'because' is not an expiry date" "$tmp/reasonfirst" "$today"
list badid 'rustsec-2024-0012 2027-01-01 lower case'
expect_fail "lower-case id" "is not an advisory id" "$tmp/badid" "$today"
list ghsa 'GHSA-xxxx-yyyy-zzzz 2027-01-01 other id scheme'
expect_fail "non-RustSec id" "is not an advisory id" "$tmp/ghsa" "$today"
list indented '  RUSTSEC-2024-0013 2027-01-01 leading blanks are fine' 'RUSTSEC-2024-0013 2027-02-01 again'
expect_fail "id listed twice" "listed twice (line 1)" "$tmp/indented" "$today"
# one bad entry among good ones still fails, and every problem is named
list mixed \
  'RUSTSEC-2024-0014 2027-01-01 fine' \
  'RUSTSEC-2024-0015 2026-01-01 expired' \
  'RUSTSEC-2024-0016 2027-01-01'
expect_fail "every problem is reported" "2 problem(s)" "$tmp/mixed" "$today"

# --- arguments ---
expect_fail "missing file" "not found" "$tmp/does-not-exist" "$today"
expect_fail "malformed TODAY" "is not a date" "$tmp/empty" "3 Oct 2026"
expect_fail "too many arguments" "usage" "$tmp/empty" "$today" extra

if ((fails > 0)); then
  echo "audit-ignore.test: $fails failure(s)" >&2
  exit 1
fi
echo "audit-ignore.test: ok"
