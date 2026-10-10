#!/usr/bin/env bash
# packaging/release-notes.sh: the release body taken from CHANGELOG.md
# (WP-048; the Highlights body from 0.2.0, WP-193). Runs in
# `just check-packaging`; no network, no GitHub.
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

# expect_refusal NAME TEXT ARGS...: as expect_fail, and the message names
# the reason, so each case fails for the rule it is about
expect_refusal() {
  local name=$1 want=$2 rc=0
  shift 2
  bash "$script" "$@" > "$tmp/out" 2> "$tmp/err" || rc=$?
  if [[ $rc -eq 1 && ! -s $tmp/out ]] && grep -qF -- "$want" "$tmp/err"; then
    pass "$name"
  else
    fail "$name (exit $rc)"
    cat "$tmp/err" >&2
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

# --- the real 0.1.4: printed as before WP-193, the whole section ---
# The oracle cuts the lines between the two headings with sed and drops the
# blank lines at both ends; it shares no code with the script.
start=$(grep -n '^## \[0\.1\.4\] ' "$root/CHANGELOG.md" | cut -d: -f1)
end=$(grep -n '^## \[0\.1\.3\] ' "$root/CHANGELOG.md" | cut -d: -f1)
sed -n "$((start + 1)),$((end - 1))p" "$root/CHANGELOG.md" \
  | sed '/./,$!d' | tac | sed '/./,$!d' | tac > "$tmp/want-0.1.4"
if [[ $(wc -l < "$tmp/want-0.1.4") -gt 500 ]] \
  && head -n 1 "$tmp/want-0.1.4" | grep -q '^\*\*Highlights\.\*\* Seldon stays quiet'; then
  expect_ok "real CHANGELOG.md 0.1.4: the whole section, unchanged" "$tmp/want-0.1.4" 0.1.4 "$root/CHANGELOG.md"
else
  fail "real CHANGELOG.md 0.1.4: the oracle did not find the section"
fi

# --- the real [Unreleased] keeps the Highlights slot for the next release ---
# Its first part is `### Highlights` (docs/VERSIONING.md, "CHANGELOG.md").
first_part=$(awk '/^## \[Unreleased\]/ { f = 1; next } f && /^## / { exit } f && /^### / { print; exit }' \
  "$root/CHANGELOG.md")
if [[ $first_part == "### Highlights" ]]; then
  pass "real CHANGELOG.md [Unreleased] opens with ### Highlights"
else
  fail "real CHANGELOG.md [Unreleased] opens with '$first_part', not ### Highlights"
fi

# --- a synthetic changelog for the edge cases ---
cat > "$tmp/CHANGELOG.md" <<'EOF'
# Changelog

## [Unreleased]

### Highlights

- not released yet

## [1.2.0] - 2027-01-02

**Panel says something old?** The standing paragraph,
over two lines.

### Highlights

- **Breaking:** first point
- second point

- third point, after a blank line

### Breaking

- detail one
  continued

### Engine

- detail two

## [1.1.0]

### Highlights
- no date, no standing paragraph, no blank line after the heading

## [0.10.0] - 2026-12-01

### Highlights

- the only point

## [0.2.0] - 2026-11-01

### Highlights

- the first version with Highlights

## [0.1.9] - 2026-10-20

### Added

- feature one
  continued

- feature two

## [0.1.8]


- heading without a date, blank lines around

## [0.1.7] - 2026-10-01
## [0.1.6] - 2026-09-01

### Highlights

- a 0.1.x section prints whole, Highlights or not
  even with a wrapped bullet

[Unreleased]: https://example.org/o/r/compare/v1.2.0...HEAD
[1.2.0]: https://example.org/o/r/releases/tag/v1.2.0
[1.1.0]: https://example.org/o/r/releases/tag/v1.1.0
[0.10.0]: https://example.org/o/r/releases/tag/v0.10.0
[0.2.0]: https://example.org/o/r/releases/tag/v0.2.0
EOF

# from 0.2.0: standing paragraph, Highlights (bullets only), one link
printf '%s\n' '**Panel says something old?** The standing paragraph,' 'over two lines.' '' \
  '### Highlights' '' '- **Breaking:** first point' '- second point' \
  '- third point, after a blank line' '' \
  'Every change in 1.2.0: [CHANGELOG.md](https://example.org/o/r/blob/v1.2.0/CHANGELOG.md#120---2027-01-02)' \
  > "$tmp/want-1.2.0"
expect_ok "Highlights body: standing paragraph, bullets, link to the section" "$tmp/want-1.2.0" 1.2.0 "$tmp/CHANGELOG.md"

printf '%s\n' '### Highlights' '' '- no date, no standing paragraph, no blank line after the heading' '' \
  'Every change in 1.1.0: [CHANGELOG.md](https://example.org/o/r/blob/v1.1.0/CHANGELOG.md#110)' \
  > "$tmp/want-1.1.0"
expect_ok "Highlights body: heading without a date, no standing paragraph" "$tmp/want-1.1.0" 1.1.0 "$tmp/CHANGELOG.md"

printf '%s\n' '### Highlights' '' '- the only point' '' \
  'Every change in 0.10.0: [CHANGELOG.md](https://example.org/o/r/blob/v0.10.0/CHANGELOG.md#0100---2026-12-01)' \
  > "$tmp/want-0.10.0"
expect_ok "Highlights body: 0.10.0 is after 0.2.0" "$tmp/want-0.10.0" 0.10.0 "$tmp/CHANGELOG.md"

printf '%s\n' '### Highlights' '' '- the first version with Highlights' '' \
  'Every change in 0.2.0: [CHANGELOG.md](https://example.org/o/r/blob/v0.2.0/CHANGELOG.md#020---2026-11-01)' \
  > "$tmp/want-0.2.0"
expect_ok "Highlights body: 0.2.0 is the first such version" "$tmp/want-0.2.0" 0.2.0 "$tmp/CHANGELOG.md"

# before 0.2.0: the whole section, as before WP-193
printf '%s\n' '### Added' '' '- feature one' '  continued' '' '- feature two' > "$tmp/want-0.1.9"
expect_ok "0.1.x: middle section, inner blank lines kept" "$tmp/want-0.1.9" 0.1.9 "$tmp/CHANGELOG.md"

printf '%s\n' '- heading without a date, blank lines around' > "$tmp/want-0.1.8"
expect_ok "0.1.x: heading without a date, outer blank lines trimmed" "$tmp/want-0.1.8" 0.1.8 "$tmp/CHANGELOG.md"

printf '%s\n' '### Highlights' '' '- a 0.1.x section prints whole, Highlights or not' \
  '  even with a wrapped bullet' > "$tmp/want-0.1.6"
expect_ok "0.1.x: last section stops at the link references, Highlights not checked" \
  "$tmp/want-0.1.6" 0.1.6 "$tmp/CHANGELOG.md"

expect_fail "missing version" 9.9.9 "$tmp/CHANGELOG.md"
expect_fail "empty section" 0.1.7 "$tmp/CHANGELOG.md"
expect_fail "prefix of another version (0.1.0 vs 0.10.0)" 0.1.0 "$tmp/CHANGELOG.md"
expect_fail "malformed version (0x10x0)" 0x10x0 "$tmp/CHANGELOG.md"
expect_fail "Unreleased is not a version" Unreleased "$tmp/CHANGELOG.md"
expect_fail "tag form with v" v1.2.0 "$tmp/CHANGELOG.md"
expect_fail "missing changelog file" 1.2.0 "$tmp/none.md"
expect_fail "no arguments"

# --- the Highlights rules, one change each against a passing base ---
# mk FILE LINE...: a changelog whose 0.2.0 section is the given lines
mk() {
  local file=$1
  shift
  {
    printf '%s\n' '# Changelog' '' '## [Unreleased]' '' '## [0.2.0] - 2026-11-01' ''
    printf '%s\n' "$@"
    printf '%s\n' '' '### Engine' '' '- detail' '' '## [0.1.4] - 2026-10-08' '' '- old' ''
    printf '%s\n' '[0.2.0]: https://example.org/o/r/releases/tag/v0.2.0'
  } > "$file"
}
ten=()
for i in 1 2 3 4 5 6 7 8 9 10; do ten+=("- point $i"); done

mk "$tmp/base.md" '### Highlights' '' "${ten[@]}"
printf '%s\n' '### Highlights' '' "${ten[@]}" '' \
  'Every change in 0.2.0: [CHANGELOG.md](https://example.org/o/r/blob/v0.2.0/CHANGELOG.md#020---2026-11-01)' \
  > "$tmp/want-base"
expect_ok "base: ten bullets pass" "$tmp/want-base" 0.2.0 "$tmp/base.md"

mk "$tmp/nohl.md" '### Breaking' '' '- no Highlights at all'
expect_refusal "no ### Highlights" "no ### Highlights" 0.2.0 "$tmp/nohl.md"

mk "$tmp/late.md" '### Breaking' '' '- first' '' '### Highlights' '' '- late'
expect_refusal "### Highlights not the first part" "must open with ### Highlights" 0.2.0 "$tmp/late.md"

mk "$tmp/empty.md" '### Highlights' ''
expect_refusal "### Highlights without a bullet" "has no bullet" 0.2.0 "$tmp/empty.md"

mk "$tmp/eleven.md" '### Highlights' '' "${ten[@]}" '- point 11'
expect_refusal "eleven bullets" "has 11 bullets" 0.2.0 "$tmp/eleven.md"

mk "$tmp/wrapped.md" '### Highlights' '' "${ten[@]:0:9}" '- point 10 runs' '  over two lines'
expect_refusal "a wrapped bullet" "is one line" 0.2.0 "$tmp/wrapped.md"

mk "$tmp/nested.md" '### Highlights' '' "${ten[@]:0:9}" '- point 10' '  - a nested bullet'
expect_refusal "a nested bullet" "is one line" 0.2.0 "$tmp/nested.md"

mk "$tmp/wp.md" '### Highlights' '' "${ten[@]:0:9}" '- point 10 (WP-193)'
expect_refusal "a bullet naming a WP" "names no work package" 0.2.0 "$tmp/wp.md"

mk "$tmp/prose.md" '### Highlights' '' 'A paragraph instead of bullets.' "${ten[@]:0:9}"
expect_refusal "a line that is not a bullet" "is a \"- \" bullet" 0.2.0 "$tmp/prose.md"

mk "$tmp/star.md" '### Highlights' '' "${ten[@]:0:9}" '* point 10'
expect_refusal "a '*' bullet" "is a \"- \" bullet" 0.2.0 "$tmp/star.md"

mk "$tmp/blank.md" '### Highlights' '' "${ten[@]:0:9}" '- '
expect_refusal "an empty bullet" "an empty highlight" 0.2.0 "$tmp/blank.md"

grep -v '^\[0\.2\.0\]: ' "$tmp/base.md" > "$tmp/noref.md"
expect_refusal "no link reference" "no link reference" 0.2.0 "$tmp/noref.md"

sed 's|^\[0\.2\.0\]: https://|[0.2.0]: http://|' "$tmp/base.md" > "$tmp/http.md"
expect_refusal "a link reference that is not https" "no link reference" 0.2.0 "$tmp/http.md"

sed 's|/releases/tag/v0\.2\.0$|/releases/tag/v0.2.1|' "$tmp/base.md" > "$tmp/otherref.md"
expect_refusal "a link reference to another tag" "no link reference" 0.2.0 "$tmp/otherref.md"

if [[ $fails -gt 0 ]]; then
  echo "release-notes: $fails failure(s)" >&2
  exit 1
fi
echo "release-notes: ok"
