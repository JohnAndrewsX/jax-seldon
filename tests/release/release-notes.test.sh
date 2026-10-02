#!/usr/bin/env bash
# packaging/release-notes.sh: the release body taken from CHANGELOG.md
# (WP-048). Runs in `just check-packaging`; no network, no GitHub.
#
# Usage: bash tests/release/release-notes.test.sh
set -euo pipefail

root=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
script=$root/packaging/release-notes.sh
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
fails=0

pass() { echo "ok   $1"; }
fail() { echo "FAIL $1" >&2; fails=$((fails + 1)); }

# expect_ok NAME EXPECTED_FILE ARGS...: exit 0 and stdout equal to the file
expect_ok() {
  local name=$1 expected=$2
  shift 2
  if bash "$script" "$@" > "$tmp/out" 2> "$tmp/err" && diff -u "$expected" "$tmp/out"; then
    pass "$name"
  else
    fail "$name"
    cat "$tmp/err" >&2
  fi
}

# expect_fail NAME ARGS...: exit 1, nothing on stdout, a message on stderr
expect_fail() {
  local name=$1 rc=0
  shift
  bash "$script" "$@" > "$tmp/out" 2> "$tmp/err" || rc=$?
  if [[ $rc -eq 1 && ! -s $tmp/out && -s $tmp/err ]]; then
    pass "$name"
  else
    fail "$name (exit $rc)"
  fi
}

# --- the real CHANGELOG.md: the 0.1.0 section a v0.1.0 tag publishes ---
# The checks below quote its first and last lines. 0.1.0 is released, so
# its section is frozen; a later edit to it is a deliberate change here.
bash "$script" 0.1.0 "$root/CHANGELOG.md" > "$tmp/real" || fail "real 0.1.0: exit $?"
first=$(head -n 1 "$tmp/real")
last=$(tail -n 1 "$tmp/real")
if [[ $first == "First release."* && $last == *"preview image." ]] \
  && grep -qx '### Engine' "$tmp/real" && grep -qx '### Plugin' "$tmp/real" \
  && grep -qx '### Packaging and docs' "$tmp/real" \
  && ! grep -q '^## ' "$tmp/real" && ! grep -q '^\[' "$tmp/real"; then
  pass "real CHANGELOG.md 0.1.0: whole section, no heading, no link references"
else
  fail "real CHANGELOG.md 0.1.0"
  cat "$tmp/real" >&2
fi
# the default path is CHANGELOG.md in the working directory
if (cd "$root" && bash "$script" 0.1.0) | cmp -s - "$tmp/real"; then
  pass "default CHANGELOG path"
else
  fail "default CHANGELOG path"
fi

# --- a synthetic changelog for the edge cases ---
cat > "$tmp/CHANGELOG.md" <<'EOF'
# Changelog

## [Unreleased]

- not released yet

## [1.2.0] - 2027-01-02

### Added

- feature one
  continued

- feature two

## [1.1.0]


- heading without a date, blank lines around

## [1.0.0] - 2026-12-01
## [0.10.0] - 2026-11-01

- the last section, before the link references

[Unreleased]: https://example.org/compare/v1.2.0...HEAD
[1.2.0]: https://example.org/releases/tag/v1.2.0
EOF

printf '%s\n' '### Added' '' '- feature one' '  continued' '' '- feature two' > "$tmp/want-1.2.0"
expect_ok "middle section, inner blank lines kept" "$tmp/want-1.2.0" 1.2.0 "$tmp/CHANGELOG.md"

printf '%s\n' '- heading without a date, blank lines around' > "$tmp/want-1.1.0"
expect_ok "heading without a date, outer blank lines trimmed" "$tmp/want-1.1.0" 1.1.0 "$tmp/CHANGELOG.md"

printf '%s\n' '- the last section, before the link references' > "$tmp/want-0.10.0"
expect_ok "last section stops at the link references" "$tmp/want-0.10.0" 0.10.0 "$tmp/CHANGELOG.md"

expect_fail "missing version" 9.9.9 "$tmp/CHANGELOG.md"
expect_fail "empty section" 1.0.0 "$tmp/CHANGELOG.md"
expect_fail "prefix of another version (0.1.0 vs 0.10.0)" 0.1.0 "$tmp/CHANGELOG.md"
expect_fail "malformed version (0x10x0)" 0x10x0 "$tmp/CHANGELOG.md"
expect_fail "Unreleased is not a version" Unreleased "$tmp/CHANGELOG.md"
expect_fail "tag form with v" v1.2.0 "$tmp/CHANGELOG.md"
expect_fail "missing changelog file" 1.2.0 "$tmp/none.md"
expect_fail "no arguments"

if [[ $fails -gt 0 ]]; then
  echo "release-notes: $fails failure(s)" >&2
  exit 1
fi
echo "release-notes: ok"
