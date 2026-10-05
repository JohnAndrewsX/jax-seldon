#!/usr/bin/env bash
# packaging/plugin-version.sh: manifest.json `version` and Model.js
# PLUGIN_VERSION agree (WP-090), on the repository and on edited copies.
# Runs in `just check-packaging`; jq only, no host tools.
#
# Usage: bash tests/release/plugin-version.test.sh
set -euo pipefail

root=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
script=$root/packaging/plugin-version.sh
manifest=$root/plugin/manifest.json
model=$root/plugin/Model.js
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
fails=0
version=$(jq -r .version "$manifest")

pass() { echo "ok   $1"; }
fail() { echo "FAIL $1" >&2; fails=$((fails + 1)); }

# expect_ok NAME ARGS...: exit 0
expect_ok() {
  local name=$1
  shift
  if bash "$script" "$@" > "$tmp/out" 2> "$tmp/err"; then
    pass "$name"
  else
    fail "$name: $(cat "$tmp/err")"
  fi
}

# expect_fail NAME PATTERN ARGS...: exit 1 with PATTERN on stderr
expect_fail() {
  local name=$1 pattern=$2 rc=0
  shift 2
  bash "$script" "$@" > "$tmp/out" 2> "$tmp/err" || rc=$?
  if [[ $rc == 1 ]] && grep -q -- "$pattern" "$tmp/err"; then
    pass "$name"
  else
    fail "$name: exit $rc, stderr '$(cat "$tmp/err")'"
  fi
}

expect_ok "the repository's plugin agrees" "$manifest" "$model"
expect_ok "and is at its own version" "$manifest" "$model" "$version"
# the release workflow reads both from the plugin split through pipes
expect_ok "files given as pipes" <(cat "$manifest") <(cat "$model") "$version"

jq '.version = "9.9.9"' "$manifest" > "$tmp/bumped.json"
expect_fail "manifest bumped, Model.js not" "manifest.json says 9.9.9, Model.js PLUGIN_VERSION says $version" \
  "$tmp/bumped.json" "$model"
sed 's/^var PLUGIN_VERSION = .*/var PLUGIN_VERSION = "9.9.9"/' "$model" > "$tmp/bumped.js"
expect_fail "Model.js bumped, manifest not" "manifest.json says $version, Model.js PLUGIN_VERSION says 9.9.9" \
  "$manifest" "$tmp/bumped.js"
expect_ok "both bumped" "$tmp/bumped.json" "$tmp/bumped.js" 9.9.9
expect_fail "the release's version differs" "the plugin is at $version, the release at 9.9.9" \
  "$manifest" "$model" 9.9.9

grep -v '^var PLUGIN_VERSION = ' "$model" > "$tmp/none.js"
expect_fail "no PLUGIN_VERSION line" "exactly one" "$manifest" "$tmp/none.js"
{ cat "$model"; echo 'var PLUGIN_VERSION = "0.0.1"'; } > "$tmp/two.js"
expect_fail "two PLUGIN_VERSION lines" "exactly one" "$manifest" "$tmp/two.js"
sed "s/^var PLUGIN_VERSION = .*/var PLUGIN_VERSION = '$version'/" "$model" > "$tmp/quotes.js"
expect_fail "single quotes" "malformed line" "$manifest" "$tmp/quotes.js"
jq '.version = 1' "$manifest" > "$tmp/number.json"
expect_fail "a numeric manifest version" "no version string" "$tmp/number.json" "$model"
jq 'del(.version)' "$manifest" > "$tmp/missing.json"
expect_fail "no manifest version" "no version string" "$tmp/missing.json" "$model"

if ((fails > 0)); then
  echo "plugin-version.test: $fails failed" >&2
  exit 1
fi
echo "plugin-version.test: ok"
