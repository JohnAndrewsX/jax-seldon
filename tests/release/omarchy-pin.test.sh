#!/usr/bin/env bash
# packaging/omarchy-validate.sh and packaging/omarchy-pin (WP-190): the
# pin is well-formed, a file whose sha256 differs from the pin is refused
# and never run, a malformed pin, a failed fetch and a non-HTTPS URL are
# refused, the validator's verdict is the script's. Offline: a scratch
# mirror of raw.githubusercontent.com served as file:// URLs. Where the
# installed Omarchy validator equals the pin (the dev host), it is served
# from the mirror and run on plugin/ and on broken copies. Runs in
# `just check-packaging`.
#
# Usage: bash tests/release/omarchy-pin.test.sh
set -euo pipefail

root=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
script=$root/packaging/omarchy-validate.sh
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
fails=0

pass() { echo "ok   $1"; }
fail() { echo "FAIL $1" >&2; fails=$((fails + 1)); }

# --- the real pin ---
read -r repo commit sha <<< "$(bash "$script" --print-pin)"
if [[ $repo == omacom/omarchy && $commit =~ ^[0-9a-f]{40}$ && $sha =~ ^[0-9a-f]{64}$ ]]; then
  pass "packaging/omarchy-pin: omacom/omarchy, a 40-hex commit, a 64-hex sha256"
else
  fail "packaging/omarchy-pin: '$repo' '$commit' '$sha'"
fi

# A fake validator in the mirror at REPO/COMMIT: records its argument in
# $tmp/ran, exits with FAKE_EXIT.
mirror=$tmp/raw
fake() {
  install -d "$mirror/$1/$2/bin"
  cat > "$mirror/$1/$2/bin/omarchy-plugin-validate" <<EOF
#!/bin/bash
echo "\$1" > "$tmp/ran"
echo "fake validator: \$1"
exit \${FAKE_EXIT:-0}
EOF
}
fake_commit=0123456789abcdef0123456789abcdef01234567
fake omacom/omarchy "$fake_commit"
fake_sha=$(sha256sum "$mirror/omacom/omarchy/$fake_commit/bin/omarchy-plugin-validate" | cut -d' ' -f1)
mkdir -p "$tmp/plugin"
echo '{}' > "$tmp/plugin/manifest.json"

# pin FILE LINES...: a pin file with a comment and LINES
pin() {
  local file=$1
  shift
  { echo "# test pin"; printf '%s\n' "$@"; } > "$file"
}
pin "$tmp/good" repo=omacom/omarchy "commit=$fake_commit" "validator_sha256=$fake_sha"

# run PIN [ENV...]: the script on $tmp/plugin with PIN and the mirror;
# sets rc, leaves stdout and stderr in $tmp/out and $tmp/err
run() {
  local pin=$1
  shift
  rm -f "$tmp/ran"
  rc=0
  env SELDON_OMARCHY_PIN="$pin" SELDON_OMARCHY_RAW="${RAW:-file://$mirror}" "$@" \
    bash "$script" "$tmp/plugin" > "$tmp/out" 2> "$tmp/err" || rc=$?
}

# expect_refused NAME PATTERN PIN [ENV...]: exit 1, PATTERN on stderr, the
# validator never ran
expect_refused() {
  local name=$1 pattern=$2
  shift 2
  run "$@"
  if [[ $rc == 1 && ! -e $tmp/ran ]] && grep -q -- "$pattern" "$tmp/err"; then
    pass "$name"
  else
    fail "$name: exit $rc, ran: $([[ -e $tmp/ran ]] && echo yes || echo no), stderr '$(cat "$tmp/err")'"
  fi
}

run "$tmp/good"
if [[ $rc == 0 && $(cat "$tmp/ran" 2> /dev/null) == "$tmp/plugin" ]] && grep -q '^omarchy-validate: ok$' "$tmp/out"; then
  pass "a validator with the pinned sha256 runs on the plugin folder"
else
  fail "good pin: exit $rc, stderr '$(cat "$tmp/err")'"
fi

run "$tmp/good" FAKE_EXIT=1
if [[ $rc == 1 && -e $tmp/ran ]] && grep -q 'is not a valid plugin' "$tmp/err"; then
  pass "the validator's failure fails the script"
else
  fail "validator failure: exit $rc, stderr '$(cat "$tmp/err")'"
fi

# a wrong sha256: in a test pin, and in a copy of the real pin whose
# commit holds a file that is not the pinned validator
pin "$tmp/wrong" repo=omacom/omarchy "commit=$fake_commit" "validator_sha256=${fake_sha:0:63}0"
[[ $fake_sha != "${fake_sha:0:63}0" ]] || pin "$tmp/wrong" repo=omacom/omarchy "commit=$fake_commit" "validator_sha256=${fake_sha:0:63}1"
expect_refused "a wrong sha256 is refused, the file never runs" "refusing .*sha256" "$tmp/wrong"
fake "$repo" "$commit"
expect_refused "packaging/omarchy-pin refuses a file that is not the pinned validator" "refusing .*sha256" "$root/packaging/omarchy-pin"
rm -r "${mirror:?}/${repo%%/*}"
fake omacom/omarchy "$fake_commit"

# a malformed pin is refused before any fetch
expect_refused "no pin file" "pin file not found" "$tmp/none"
pin "$tmp/p" repo=omacom/omarchy "commit=${fake_commit:0:12}" "validator_sha256=$fake_sha"
expect_refused "a short commit" "unexpected line" "$tmp/p"
pin "$tmp/p" repo=omacom/omarchy "commit=${fake_commit^^}" "validator_sha256=$fake_sha"
expect_refused "an upper-case commit" "unexpected line" "$tmp/p"
pin "$tmp/p" repo=omacom/omarchy "commit=$fake_commit"
expect_refused "no sha256" "needs repo=" "$tmp/p"
pin "$tmp/p" repo=omacom/omarchy "commit=$fake_commit" "validator_sha256=$fake_sha" "commit=$fake_commit"
expect_refused "a key twice" "unexpected line" "$tmp/p"
pin "$tmp/p" repo=omacom/omarchy "commit=$fake_commit" "validator_sha256=$fake_sha" "url=https://example.org"
expect_refused "an unknown key" "unexpected line" "$tmp/p"
pin "$tmp/p" repo=omacom/omarchy "commit=$fake_commit " "validator_sha256=$fake_sha"
expect_refused "a trailing space" "unexpected line" "$tmp/p"
pin "$tmp/p" "repo=omacom/omarchy/../x" "commit=$fake_commit" "validator_sha256=$fake_sha"
expect_refused "a repo with a path" "unexpected line" "$tmp/p"

# fetching: a missing file, and a URL that is not HTTPS (curl refuses the
# protocol before it connects; no network)
pin "$tmp/p" repo=omacom/omarchy "commit=${fake_commit:0:39}8" "validator_sha256=$fake_sha"
expect_refused "a commit the mirror does not have" "cannot fetch" "$tmp/p"
RAW=http://127.0.0.1:9 expect_refused "a plain http URL" 'Protocol "http" is disabled' "$tmp/good"
rm -rf "$tmp/plugin"
expect_refused "no plugin folder" "plugin folder not found" "$tmp/good"
mkdir -p "$tmp/plugin"

# --- the pinned validator itself, where it is installed ---
installed=${OMARCHY_PATH:-/usr/share/omarchy}/bin/omarchy-plugin-validate
if [[ -f $installed && $(sha256sum "$installed" | cut -d' ' -f1) == "$sha" ]]; then
  install -d "$mirror/$repo/$commit/bin"
  cp "$installed" "$mirror/$repo/$commit/bin/"
  # check NAME WANT DIR: WANT ok|fail
  check() {
    local rc=0
    SELDON_OMARCHY_RAW="file://$mirror" bash "$script" "$3" > "$tmp/out" 2> "$tmp/err" || rc=$?
    if [[ $2 == ok && $rc == 0 ]] || [[ $2 == fail && $rc == 1 ]]; then
      pass "pinned validator: $1"
    else
      fail "pinned validator: $1: exit $rc, stderr '$(cat "$tmp/err")'"
    fi
  }
  check "plugin/ is valid" ok "$root/plugin"
  cp -r "$root/plugin" "$tmp/no-entry"
  rm "$tmp/no-entry/$(jq -r '.entryPoints | to_entries[0].value' "$root/plugin/manifest.json")"
  check "a missing entry point file fails" fail "$tmp/no-entry"
  cp -r "$root/plugin" "$tmp/no-key"
  jq 'del(.entryPoints.service)' "$root/plugin/manifest.json" > "$tmp/no-key/manifest.json"
  check "a kind without its entry point fails" fail "$tmp/no-key"
  cp -r "$root/plugin" "$tmp/omarchy-id"
  jq '.id = "omarchy.seldon"' "$root/plugin/manifest.json" > "$tmp/omarchy-id/manifest.json"
  check "an omarchy.* id fails" fail "$tmp/omarchy-id"
else
  echo "note omarchy-pin.test: no installed validator with the pinned sha256 at $installed; the real-validator cases are skipped (CI runs it in just plugin-validate)"
fi

if ((fails > 0)); then
  echo "omarchy-pin.test: $fails failure(s)" >&2
  exit 1
fi
echo "omarchy-pin.test: ok"
