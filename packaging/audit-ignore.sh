#!/usr/bin/env bash
# Print the advisory ids `cargo audit` may skip, one per line, from the
# reviewed ignore list (CONTRIBUTING.md, "Dependency advisories").
#
# Usage: audit-ignore.sh [FILE [TODAY]]
#
# FILE defaults to audit-ignore.txt next to this script, TODAY (YYYY-MM-DD)
# to the current UTC date. Each entry is `RUSTSEC-YYYY-NNNN YYYY-MM-DD
# reason`. Exit 1, with every problem on stderr and nothing on stdout,
# when an entry is malformed, has no reason, repeats an id, has expired
# (expiry <= TODAY) or expires more than 365 days after TODAY, so the
# release build stops until the list is reviewed (release.yml).
set -euo pipefail

usage() {
  echo "usage: audit-ignore.sh [FILE [TODAY]]" >&2
  exit 1
}

[[ $# -le 2 ]] || usage
file=${1:-$(dirname "${BASH_SOURCE[0]}")/audit-ignore.txt}
today=${2:-$(date -u +%F)}

# a real calendar date in ISO form (2026-02-30 and 2026-2-1 are not)
is_date() {
  [[ $1 =~ ^[0-9]{4}-[0-9]{2}-[0-9]{2}$ ]] && [[ $(date -u -d "$1" +%F 2> /dev/null) == "$1" ]]
}

is_date "$today" || {
  echo "audit-ignore: '$today' is not a date (YYYY-MM-DD)" >&2
  exit 1
}
[[ -f $file ]] || {
  echo "audit-ignore: $file not found" >&2
  exit 1
}
latest=$(date -u -d "$today + 365 days" +%F)

errors=0
err() {
  echo "audit-ignore: $file:$n: $1" >&2
  errors=$((errors + 1))
}

declare -A seen=()
ids=()
n=0
while IFS= read -r line || [[ -n $line ]]; do
  n=$((n + 1))
  [[ $line =~ ^[[:space:]]*(#|$) ]] && continue
  read -r id expiry reason <<< "$line"
  if [[ ! $id =~ ^RUSTSEC-[0-9]{4}-[0-9]{4}$ ]]; then
    err "'$id' is not an advisory id (RUSTSEC-YYYY-NNNN)"
    continue
  fi
  if ! is_date "${expiry:-}"; then
    err "$id: '${expiry:-}' is not an expiry date (YYYY-MM-DD)"
    continue
  fi
  [[ -n ${reason:-} ]] || err "$id: no reason"
  [[ -z ${seen[$id]:-} ]] || err "$id: listed twice (line ${seen[$id]})"
  seen[$id]=$n
  # ISO dates compare as strings
  if [[ ! $expiry > $today ]]; then
    err "$id: expired on $expiry; review it again or remove it"
  elif [[ $expiry > $latest ]]; then
    err "$id: expiry $expiry is more than 365 days after $today"
  fi
  ids+=("$id")
done < "$file"

if ((errors > 0)); then
  echo "audit-ignore: $errors problem(s) in $file" >&2
  exit 1
fi
((${#ids[@]} == 0)) || printf '%s\n' "${ids[@]}"
