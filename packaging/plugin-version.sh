#!/usr/bin/env bash
# The plugin's two version strings must agree (docs/VERSIONING.md): the
# manifest's `version` and the `var PLUGIN_VERSION = "…"` line in
# Model.js, which the restart notice compares with the manifest the shell
# injects (WP-090). A mismatch would show "Restart the shell" to every
# user, and no restart could clear it. With VERSION, both must also equal
# it (the release workflow passes the tag's version). Runs in `just
# check-packaging` on the repository and in the release workflow on the
# plugin split; needs jq only, no host tools.
#
# Usage: packaging/plugin-version.sh MANIFEST MODEL_JS [VERSION]
set -euo pipefail

manifest=${1:?usage: plugin-version.sh MANIFEST MODEL_JS [VERSION]}
model=${2:?usage: plugin-version.sh MANIFEST MODEL_JS [VERSION]}
want=${3:-}

fail() { echo "plugin-version: $*" >&2; exit 1; }

declared=$(jq -r 'if (.version | type) == "string" then .version else "" end' "$manifest") \
  || fail "cannot read the manifest's version"
[[ -n $declared ]] || fail "the manifest has no version string"
lines=$(grep -E '^var PLUGIN_VERSION = ' "$model" || true)
[[ -n $lines && $lines != *$'\n'* ]] || fail "Model.js needs exactly one 'var PLUGIN_VERSION = \"…\"' line"
[[ $lines =~ ^var\ PLUGIN_VERSION\ =\ \"([^\"]+)\"$ ]] || fail "malformed line: $lines"
running=${BASH_REMATCH[1]}
[[ $running == "$declared" ]] || fail "manifest.json says $declared, Model.js PLUGIN_VERSION says $running"
[[ -z $want || $declared == "$want" ]] || fail "the plugin is at $declared, the release at $want"
echo "plugin-version: $declared"
