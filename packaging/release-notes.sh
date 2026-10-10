#!/usr/bin/env bash
# Print the release body of one version from CHANGELOG.md (WP-048, WP-193).
#
# Usage: release-notes.sh X.Y.Z [CHANGELOG]
#
# The section is everything under `## [X.Y.Z]` (Keep a Changelog heading,
# with or without ` - date`) up to the next `## ` heading or the link
# references at the end of the file, without leading and trailing blank
# lines. Exit 1 when the version is malformed or the section is missing
# or empty, so a tag without notes cannot ship (docs/VERSIONING.md).
#
# Before 0.2.0 the body is the whole section. From 0.2.0 the section opens
# with `### Highlights` (after the standing paragraph, if any) and the body
# is that paragraph, the Highlights and one link to the whole section in
# CHANGELOG.md at the tag. Exit 1 when Highlights is missing or not the
# first part, has no bullet or more than ten, a bullet runs over one line,
# names a WP, or a line is not a `- ` bullet, or when the link reference
# `[X.Y.Z]: …/releases/tag/vX.Y.Z` is missing.
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
is_head='
  function is_head(line) {
    return substr(line, 1, length(head)) == head &&
      (length(line) == length(head) || substr(line, length(head) + 1, 1) == " ")
  }'
notes=$(awk -v head="## [$version]" "$is_head"'
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

IFS=. read -r major minor _ <<< "$version"
if (( 10#$major == 0 && 10#$minor < 2 )); then
  printf '%s\n' "$notes"
  exit 0
fi

# From 0.2.0: the standing paragraph, the Highlights, a link to the section.
body=$(printf '%s\n' "$notes" | awk -v version="$version" '
  function bad(msg) { print "release-notes: " version ": " msg > "/dev/stderr"; failed = 1 }
  part == 0 && $0 == "### Highlights" { part = 1; next }
  part == 0 && /^### / { part = 3; opener = $0; next }
  part == 3 && $0 == "### Highlights" { late = 1; exit }
  part == 3 { next }
  part == 0 { lead[++nl] = $0; next }
  part == 1 && /^### / { part = 2; exit }
  part == 1 && /^[[:space:]]*$/ { next }
  part == 1 && /^[[:space:]]/ { bad("a highlight is one line; this continues one: " $0); next }
  part == 1 && /^- *$/ { bad("an empty highlight"); next }
  part == 1 && /^- / {
    if ($0 ~ /WP-[0-9]/) bad("a highlight names no work package: " $0)
    bullet[++nb] = $0; next
  }
  part == 1 { bad("a highlight is a \"- \" bullet: " $0) }
  END {
    if (late) bad("the section must open with ### Highlights, not " opener)
    else if (part != 1 && part != 2) bad("no ### Highlights; from 0.2.0 every section opens with one")
    if (failed) exit 1
    if (nb == 0) { bad("### Highlights has no bullet"); exit 1 }
    if (nb > 10) { bad("### Highlights has " nb " bullets; at most ten"); exit 1 }
    while (nl > 0 && lead[nl] ~ /^[[:space:]]*$/) nl--
    for (i = 1; i <= nl; i++) print lead[i]
    if (nl > 0) print ""
    print "### Highlights"
    print ""
    for (i = 1; i <= nb; i++) print bullet[i]
  }
')

# The repository from the link reference `[X.Y.Z]: <repo>/releases/tag/vX.Y.Z`
# (VERSIONING.md, "Before a tag"), and GitHub's anchor of the heading.
repo=$(awk -v ref="[$version]: " -v tail="/releases/tag/v$version" '
  substr($0, 1, length(ref)) == ref {
    url = substr($0, length(ref) + 1)
    n = length(url) - length(tail)
    if (n > 0 && substr(url, n + 1) == tail && url ~ /^https:\/\/[^[:space:]]+$/) {
      print substr(url, 1, n); exit
    }
  }
' "$changelog")
if [[ -z $repo ]]; then
  echo "release-notes: no link reference '[$version]: https://…/releases/tag/v$version' in $changelog" >&2
  exit 1
fi
heading=$(awk -v head="## [$version]" "$is_head"' is_head($0) { print substr($0, 4); exit }' "$changelog")
anchor=$(printf '%s' "$heading" | tr '[:upper:]' '[:lower:]' | tr -cd 'a-z0-9 _-' | tr ' ' '-')

printf '%s\n\n' "$body"
printf 'Every change in %s: [CHANGELOG.md](%s/blob/v%s/CHANGELOG.md#%s)\n' \
  "$version" "$repo" "$version" "$anchor"
