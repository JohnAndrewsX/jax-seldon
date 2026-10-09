#!/usr/bin/env bash
# Omarchy's own plugin validator (bin/omarchy-plugin-validate, bash and jq)
# at the commit packaging/omarchy-pin names, for hosts without the omarchy
# CLI (WP-190): `just plugin-validate` in CI runs it on plugin/, the
# release workflow on the plugin split before anything is pushed. Fetches
# the one file over HTTPS, refuses it unless its sha256 equals the pin's,
# then runs it on PLUGIN_DIR. The dev host keeps `omarchy plugin
# validate` (the installed tree is the reference, AGENTS.md §1).
#
# Usage: packaging/omarchy-validate.sh PLUGIN_DIR
#        packaging/omarchy-validate.sh --print-pin   (repo, commit, sha256)
#
# Tests only (tests/release/omarchy-pin.test.sh): SELDON_OMARCHY_PIN names
# another pin file; SELDON_OMARCHY_RAW replaces
# https://raw.githubusercontent.com and may then be a file:// URL.
set -euo pipefail

root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
pin=${SELDON_OMARCHY_PIN:-$root/packaging/omarchy-pin}
raw=${SELDON_OMARCHY_RAW:-https://raw.githubusercontent.com}

fail() { echo "omarchy-validate: $*" >&2; exit 1; }

arg=${1:?usage: omarchy-validate.sh PLUGIN_DIR | --print-pin}

# The pin: comments, blank lines and exactly one of each key, nothing else.
[[ -f $pin ]] || fail "pin file not found: $pin"
repo='' commit='' sha=''
while IFS= read -r line || [[ -n $line ]]; do
  [[ $line =~ ^[[:space:]]*(#.*)?$ ]] && continue
  if [[ $line =~ ^repo=([A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+)$ && -z $repo ]]; then
    repo=${BASH_REMATCH[1]}
  elif [[ $line =~ ^commit=([0-9a-f]{40})$ && -z $commit ]]; then
    commit=${BASH_REMATCH[1]}
  elif [[ $line =~ ^validator_sha256=([0-9a-f]{64})$ && -z $sha ]]; then
    sha=${BASH_REMATCH[1]}
  else
    fail "$pin: unexpected line: $line"
  fi
done < "$pin"
[[ -n $repo && -n $commit && -n $sha ]] \
  || fail "$pin: needs repo=<owner/name>, commit=<40 hex> and validator_sha256=<64 hex>"

if [[ $arg == --print-pin ]]; then
  echo "$repo $commit $sha"
  exit 0
fi

dir=$arg
[[ -d $dir ]] || fail "plugin folder not found: $dir"

# HTTPS only, redirects included; a file:// base only when a test set one
proto='=https'
[[ $raw == file://* && -n ${SELDON_OMARCHY_RAW:-} ]] && proto='=file'
url="$raw/$repo/$commit/bin/omarchy-plugin-validate"
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
validator=$tmp/omarchy-plugin-validate
curl --fail --silent --show-error --location --proto "$proto" --proto-redir "$proto" \
  --retry 3 --max-time 60 --output "$validator" "$url" \
  || fail "cannot fetch $url"
got=$(sha256sum "$validator" | cut -d' ' -f1)
[[ $got == "$sha" ]] \
  || fail "refusing $url: sha256 $got, the pin says $sha (packaging/README.md, \"The Omarchy pin\")"

echo "omarchy-validate: $repo@${commit:0:12} validator (sha256 ${sha:0:12}) on $dir"
bash "$validator" "$dir" || fail "$dir is not a valid plugin (Omarchy's validator, above)"
echo "omarchy-validate: ok"
