#!/usr/bin/env bash
# packaging/acceptance-check.sh — the release acceptance record of
# vX.Y.Z (docs/VERSIONING.md, "Release acceptance record"; WP-192):
# packaging/acceptance/vX.Y.Z.json as committed in REF (default HEAD).
#
# Refuses (exit 1) unless
#   - the record is in REF's tree, is one JSON object with exactly the
#     documented fields, and every field is well-formed;
#   - `status` follows the scenarios: `failed` when one failed, else
#     `partial` when one was not run, else `passed`; each scenario not
#     run has a `limitations` entry starting with "<id>: ";
#   - `status` is not `failed`;
#   - the record has no key twice (jq would read only the last one);
#   - the record names no private path (/home/…, /Users/…, /root/…), and
#     none of this machine's host name, user name, the hosts listed in the
#     git-ignored scripts/guard-hosts.local and scripts/deploy-hosts.local
#     or the machine-ids pinned in the latter;
#   - `commit` is in the repository and an ancestor of REF (or REF);
#   - between `commit` and REF only docs/, work/, packaging/acceptance/
#     and *.md outside engine/ changed (the engine compiles its skills
#     and templates in); every other path is printed. Renames count as
#     both paths.
# A `partial` record passes and says what was not run; the operator's
# go for the tag reads it.
#
# Prints a summary to quote in the tag question. Needs git and jq, no
# network; reads the repository of the current directory.
#
# Usage: packaging/acceptance-check.sh X.Y.Z [REF]
set -euo pipefail

usage="usage: packaging/acceptance-check.sh X.Y.Z [REF]"
if (($# < 1 || $# > 2)); then
  echo "acceptance-check: $usage" >&2
  exit 1
fi
version=${1#v}
ref=${2:-HEAD}
errors=()

# refuse MESSAGE...: each message a bullet; one starting with two spaces
# (a path) goes under the bullet before it
refuse() {
  local m
  echo "acceptance-check: v$version refused:" >&2
  for m; do
    if [[ $m == "  "* ]]; then printf '      %s\n' "${m#  }" >&2; else printf '  - %s\n' "$m" >&2; fi
  done
  exit 1
}

[[ $version =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || refuse "'$version' is not a version X.Y.Z ($usage)"
command -v jq >/dev/null || refuse "jq is not installed"
top=$(git rev-parse --show-toplevel 2>/dev/null) || refuse "$PWD is not in a git repository"
ref_sha=$(git rev-parse --verify --quiet "$ref^{commit}") || refuse "'$ref' is not a commit"
path=packaging/acceptance/v$version.json
record=$(git show "$ref_sha:$path" 2>/dev/null) \
  || refuse "no $path in $ref (${ref_sha:0:12}); commit the record first"
jq -e 'type == "object"' >/dev/null 2>&1 <<<"$record" \
  || refuse "$path is not one JSON object"
# one object, not several: `jq -s length` counts the documents
[[ $(jq -s length <<<"$record") == 1 ]] || refuse "$path holds more than one JSON value"
# A key given twice: jq keeps the last value, a reader may see the first.
# The text's leaf events outnumber the parsed record's then; name them.
leaf_name='map(if type == "number" then "[\(.)]" else ".\(.)" end) | join("")'
raw_leaves=$(jq -r --stream "select(length == 2) | .[0] | $leaf_name" <<<"$record")
parsed_leaves=$(jq -r "tostream | select(length == 2) | .[0] | $leaf_name" <<<"$record")
if [[ $(wc -l <<<"$raw_leaves") != "$(wc -l <<<"$parsed_leaves")" ]]; then
  twice=$({ sort <<<"$raw_leaves" | uniq -d; comm -23 <(sort -u <<<"$raw_leaves") <(sort -u <<<"$parsed_leaves"); } \
    | sort -u | paste -sd ' ')
  refuse "$path has a key twice (at ${twice:-?}); jq reads only the last one, a reader may see the first"
fi

# ---- fields and rules ---------------------------------------------------------------

# One message per line; empty when the record is well-formed.
fields=$(jq -r --arg want "$version" '
  def nonempty: type == "string" and length > 0;
  def count: type == "number" and . == floor and . >= 0;
  def percent: . == null or (count and . <= 100);
  def oneof($xs): . as $x | any($xs[]; . == $x);
  def unknown($allowed; $at): (keys - $allowed)[] | "unknown field \"\($at)\(.)\"";
  def iso_date:
    type == "string" and test("^[0-9]{4}-[0-9]{2}-[0-9]{2}$")
    and ((try (strptime("%Y-%m-%d") | mktime | strftime("%Y-%m-%d")) catch "") as $d | $d == .);
  def where_words: ["fixture", "headless", "CI", "test host", "desktop", "not run"];
  def results: ["passed", "failed", "not run"];
  def count_keys: ["humanSteps", "passwordPrompts", "snapshotCoverage", "r3GatesHonoured", "agentClosesReopened"];
  . as $r
  | ($want | gsub("\\."; "\\.")) as $v
  | [
      unknown(["version", "status", "commit", "date", "omarchy", "engine", "plugin", "scenarios", "limitations"]; ""),
      (if .version == $want then empty
       else "version is \(.version | tojson), not \"\($want)\"" end),
      (if .status | oneof(["passed", "failed", "partial"]) then empty
       else "status is \(.status | tojson); want passed, failed or partial" end),
      (if (.commit | type) == "string" and (.commit | test("^[0-9a-f]{40}$")) then empty
       else "commit is \(.commit | tojson); want the 40 lowercase hex characters of the commit deployed to the test host" end),
      (if .date | iso_date then empty
       else "date is \(.date | tojson); want a date YYYY-MM-DD" end),
      (if (.omarchy | type) != "object" then
         "omarchy is \(.omarchy | tojson); want {\"version\": `omarchy version`, \"channel\": `omarchy version channel`}"
       else
         (.omarchy | unknown(["version", "channel"]; "omarchy.")),
         (if .omarchy.version | nonempty then empty else "omarchy.version is missing or empty (`omarchy version`)" end),
         (if .omarchy.channel | nonempty then empty else "omarchy.channel is missing or empty (`omarchy version channel`)" end)
       end),
      (if (.engine | type) == "string" and (.engine | test("^\($v)(\\+(main|next)\\.[0-9a-f]{7,40})?$")) then
         ([.engine | capture("\\+(main|next)\\.(?<h>[0-9a-f]+)$") | .h] | first) as $h
         | if $h == null or (($r.commit | type) == "string" and ($r.commit | startswith($h))) then empty
           else "engine \(.engine | tojson) was built from \($h), not from commit \($r.commit | tojson)" end
       else "engine is \(.engine | tojson); want \"\($want)\" or \"\($want)+main.<short commit>\" (`seldon --version` on the test host)" end),
      (if .plugin == $want then empty
       else "plugin is \(.plugin | tojson), not \"\($want)\" (the installed manifest version)" end),
      (if (.scenarios | type) != "array" or (.scenarios | length) == 0 then
         "scenarios is \(.scenarios | tojson); want a non-empty list"
       else
         (.scenarios | to_entries[] | .key as $i | .value as $s
          | ($s | if type == "object" and (.id | type) == "string" then "scenarios[\($i)] (\(.id))" else "scenarios[\($i)]" end) as $at
          | if ($s | type) != "object" then "\($at) is not an object"
            else
              ($s | unknown(["id", "title", "where", "result", "counts", "notes"]; "\($at).")),
              (if ($s.id | type) == "string" and ($s.id | test("^[A-Za-z0-9][A-Za-z0-9._-]*$")) then empty
               else "\($at): id is \($s.id | tojson); want letters, digits, . _ -" end),
              (if $s.title | nonempty then empty else "\($at): title is missing or empty" end),
              (if $s.where | oneof(where_words) then empty
               else "\($at): where is \($s.where | tojson); want one of \(where_words | join(", ")) (AGENTS.md §5)" end),
              (if $s.result | oneof(results) then empty
               else "\($at): result is \($s.result | tojson); want passed, failed or not run" end),
              (if ($s.where == "not run") != ($s.result == "not run") and ($s.where | oneof(where_words)) and ($s.result | oneof(results)) then
                 "\($at): where \($s.where | tojson) with result \($s.result | tojson); a scenario not run has both \"not run\""
               else empty end),
              (if $s.result == "not run" then
                 (if $s.counts == null then empty else "\($at): counts on a scenario that was not run" end)
               elif ($s.counts | type) != "object" then
                 "\($at): counts is \($s.counts | tojson); want {\"humanSteps\": n, \"passwordPrompts\": n, …} (ADR-0027 §1)"
               else
                 ($s.counts | unknown(count_keys; "\($at).counts.")),
                 (if $s.counts.humanSteps | count then empty
                  else "\($at): counts.humanSteps is \($s.counts.humanSteps | tojson); want a whole number >= 0" end),
                 (if $s.counts.passwordPrompts | count then empty
                  else "\($at): counts.passwordPrompts is \($s.counts.passwordPrompts | tojson); want a whole number >= 0" end),
                 ($s.counts | to_entries[] | select(.key == "snapshotCoverage" or .key == "r3GatesHonoured")
                  | select(.value | percent | not)
                  | "\($at): counts.\(.key) is \(.value | tojson); want a percentage 0-100, or null without an R2/R3 case"),
                 (if ($s.counts | has("agentClosesReopened") | not) or ($s.counts.agentClosesReopened | count) then empty
                  else "\($at): counts.agentClosesReopened is \($s.counts.agentClosesReopened | tojson); want a whole number >= 0" end)
               end),
              (if $s.notes == null or ($s.notes | type) == "string" then empty
               else "\($at): notes is \($s.notes | tojson); want a string" end)
            end),
         ([.scenarios[] | objects | .id | strings] | group_by(.)[] | select(length > 1)
          | "scenario id \(.[0] | tojson) is used \(length) times")
       end),
      (if (.limitations | type) != "array" or any(.limitations[]; nonempty | not) then
         "limitations is \(.limitations | tojson); want a list of non-empty strings ([] when there are none)"
       else empty end),
      # the rules, once the parts they read are well-formed
      (if (.scenarios | type) == "array" and all(.scenarios[]; type == "object" and (.result | oneof(results))) and (.scenarios | length) > 0 then
         [.scenarios[] | .result] as $res
         | (if any($res[]; . == "failed") then "failed" elif any($res[]; . == "not run") then "partial" else "passed" end) as $derived
         | (if (.status | oneof(["passed", "failed", "partial"])) and .status != $derived then
              "status is \"\(.status)\", but the scenarios say \"\($derived)\" (\([$res[] | select(. == "passed")] | length) passed, \([$res[] | select(. == "failed")] | length) failed, \([$res[] | select(. == "not run")] | length) not run); passed needs every scenario passed"
            else empty end),
           (if (.limitations | type) == "array" then
              (.limitations | map(strings)) as $lim
              | .scenarios[] | select(.result == "not run" and (.id | type) == "string") | .id as $id
              | select(any($lim[]; startswith("\($id): ")) | not)
              | "scenario \($id | tojson) was not run, but no limitations entry starts with \"\($id): \""
            else empty end)
       else empty end)
    ] | .[]
' <<<"$record") || refuse "jq could not read $path"
while IFS= read -r line; do
  [[ -n $line ]] && errors+=("$line")
done <<<"$fields"

# ---- privacy (AGENTS.md §8) ---------------------------------------------------------

text=$(jq -r '.. | strings' <<<"$record")
if grep -Eq '/(home|Users|root)/' <<<"$text"; then
  errors+=("a private path (/home/…, /Users/… or /root/…) in the record; write ~/… or leave it out (AGENTS.md §8)")
fi
names=()
host=$(uname -n 2>/dev/null || true)
[[ ${#host} -ge 3 && $host != localhost ]] && names+=("$host")
user=$(id -un 2>/dev/null || true)
[[ ${#user} -ge 3 && $user != root ]] && names+=("$user")
for list in "$top/scripts/guard-hosts.local" "$top/scripts/deploy-hosts.local"; do
  [[ -f $list ]] || continue
  # the last line may lack its newline (deploy-test-host.sh reads it too);
  # deploy-hosts.local's second column is the host's pinned machine-id
  while read -r name id _ || [[ -n $name ]]; do
    [[ -n $name && $name != \#* ]] || continue
    names+=("$name")
    [[ -z $id ]] || names+=("$id")
  done <"$list"
done
for name in ${names[@]+"${names[@]}"}; do
  if grep -Fqiw -- "$name" <<<"$text"; then
    errors+=("the record names this machine, its user, a listed host or a pinned machine-id; leave them out (AGENTS.md §8)")
    break
  fi
done

# ---- the commit and what changed after it -------------------------------------------

commit=$(jq -r '.commit | strings' <<<"$record")
later=""
if [[ $commit =~ ^[0-9a-f]{40}$ ]]; then
  if ! git cat-file -e "$commit^{commit}" 2>/dev/null; then
    errors+=("commit $commit is not in this repository")
  elif ! git merge-base --is-ancestor "$commit" "$ref_sha"; then
    errors+=("commit ${commit:0:12} is not $ref (${ref_sha:0:12}) or an ancestor of it; the live test ran on another line of history")
  else
    later=$(git rev-list --count "$commit..$ref_sha")
    offending=()
    while IFS= read -r -d '' changed; do
      case $changed in
        docs/* | work/* | packaging/acceptance/*) ;;
        engine/*) offending+=("$changed") ;;
        *.md) ;;
        *) offending+=("$changed") ;;
      esac
    done < <(git diff-tree -r -z --name-only --no-renames "$commit" "$ref_sha")
    if ((${#offending[@]} > 0)); then
      errors+=("${#offending[@]} path(s) outside docs/, work/, packaging/acceptance/ and *.md (not under engine/) changed between the tested commit ${commit:0:12} and $ref; test that code live and record it:")
      for changed in "${offending[@]}"; do
        errors+=("  $changed")
      done
    fi
  fi
fi

status=$(jq -r '.status | strings' <<<"$record")
if [[ $status == failed && ${#errors[@]} == 0 ]]; then
  errors+=("the live test failed: $(jq -r '[.scenarios[] | select(.result == "failed") | .id] | join(", ")' <<<"$record")")
fi

((${#errors[@]} == 0)) || refuse "${errors[@]}"

# ---- summary ------------------------------------------------------------------------

summary=$(jq -r --arg ref "$ref" --arg ref_sha "${ref_sha:0:12}" --arg later "$later" --arg path "$path" '
  def n($r): [.scenarios[] | select(.result == $r)] | length;
  "acceptance-check: v\(.version) at \($ref) (\($ref_sha))",
  "  record   \($path), \(.date)",
  "  status   \(.status): \(n("passed")) passed, \(n("not run")) not run",
  "  commit   \(.commit), \($later) later commit(s), none outside docs and records",
  "  omarchy  \(.omarchy.version), channel \(.omarchy.channel)",
  "  engine   \(.engine); plugin \(.plugin)",
  (.scenarios[] | "  scenario \(.id): \(.result)" + (if .where == "not run" then "" else " (\(.where))" end)
    + (if .counts then "; \(.counts.humanSteps) human step(s), \(.counts.passwordPrompts) password prompt(s)" else "" end)
    + " — \(.title)"),
  (.limitations[] | "  limit    \(.)")
' <<<"$record")
printf '%s\n' "$summary"
echo "acceptance-check: ok"
