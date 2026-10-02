#!/usr/bin/env bash
# Print the CHANGELOG.md section of one version: the release body (WP-048).
#
# Usage: release-notes.sh X.Y.Z [CHANGELOG]
#
# The section is everything under `## [X.Y.Z]` (Keep a Changelog heading,
# with or without ` - date`) up to the next `## ` heading or the link
# references at the end of the file, without leading and trailing blank
# lines. Exit 1 when the version is malformed or the section is missing
# or empty, so a tag without notes cannot ship (docs/VERSIONING.md).
set -euo pipefail

usage() {
  echo "usage: release-notes.sh X.Y.Z [CHANGELOG]" >&2
  exit 1
}

[[ $# -ge 1 && $# -le 2 ]] || usage
version=$1
changelog=${2:-CHANGELOG.md}
[[ $version =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || {
  echo "release-notes: '$version' is not X.Y.Z" >&2
  exit 1
}
[[ -f $changelog ]] || {
  echo "release-notes: $changelog not found" >&2
  exit 1
}

# Literal comparison of the heading (no regex built from the version), so
# 0.1.0 never matches 0.1.00 or 0x1x0.
notes=$(awk -v head="## [$version]" '
  function is_head(line) {
    return substr(line, 1, length(head)) == head &&
      (length(line) == length(head) || substr(line, length(head) + 1, 1) == " ")
  }
  found && /^## / { exit }
  found && /^\[(Unreleased|[0-9]+\.[0-9]+\.[0-9]+)\]: / { exit }
  found { body[++n] = $0; next }
  is_head($0) { found = 1 }
  END {
    first = 1; last = n
    while (first <= last && body[first] ~ /^[[:space:]]*$/) first++
    while (last >= first && body[last] ~ /^[[:space:]]*$/) last--
    for (i = first; i <= last; i++) print body[i]
  }
' "$changelog")

if [[ -z $notes ]]; then
  echo "release-notes: no '## [$version]' section with content in $changelog" >&2
  exit 1
fi
printf '%s\n' "$notes"
