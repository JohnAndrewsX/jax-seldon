#!/usr/bin/env bash
# packaging/acceptance-check.sh (WP-192) against a scratch git repository
# in the checkout's ignored target/ (never /tmp), with git's global and
# system config shut out: a good record passes, and each broken field,
# each broken rule, a code change after the tested commit and a commit
# that is not an ancestor is refused with its own message. Every broken
# case is the good record with one edit, so the message names the cause.
# A few mutants of the checker prove the guards are needed. Runs in
# `just check-packaging`; git and jq only, no network.
#
# Usage: bash tests/release/acceptance-check.test.sh
set -euo pipefail

root=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
script=$root/packaging/acceptance-check.sh
mkdir -p "$root/target"
work=$(mktemp -d "$root/target/acceptance-check.XXXXXX")
trap 'rm -rf "$work"' EXIT
fails=0

export GIT_CONFIG_GLOBAL=/dev/null GIT_CONFIG_NOSYSTEM=1
export GIT_AUTHOR_NAME=tester GIT_AUTHOR_EMAIL=tester@example.invalid
export GIT_COMMITTER_NAME=tester GIT_COMMITTER_EMAIL=tester@example.invalid

pass() { echo "ok   $1"; }
fail() { echo "FAIL $1" >&2; fails=$((fails + 1)); }

repo=$work/repo
rec=packaging/acceptance/v1.2.3.json
g() { git -C "$repo" "$@"; }

# put FILE TEXT: write a file of the scratch repository
put() {
  mkdir -p "$(dirname "$repo/$1")"
  printf '%s\n' "$2" >"$repo/$1"
}

# commit_all MESSAGE: commit everything, print the commit
commit_all() {
  g add -A
  g commit -q -m "$1"
  g rev-parse HEAD
}

g_init() {
  git init -q -b main "$repo"
  put engine/src.rs 'fn main() {}'
  put engine/assets/skills/seldon/SKILL.md '# skill'
  put plugin/Panel.qml 'Item {}'
  put plugin/README.md '# plugin'
  put .github/workflows/release.yml 'name: release'
  put docs/VERSIONING.md '# versions'
  put README.md '# readme'
  put CHANGELOG.md '# changelog'
}
g_init
# the real repository ignores these (.gitignore)
printf 'scripts/*.local\n' >>"$repo/.git/info/exclude"
tested=$(commit_all "release: 1.2.3")
short=${tested:0:7}

# after the live test: bookkeeping only
put docs/VERSIONING.md '# versions, edited'
put CHANGELOG.md '# changelog, edited'
put README.md '# readme, edited'
put plugin/README.md '# plugin, edited'
put work/active/WP-1/HANDOVER.md 'handover'
put packaging/acceptance/v1.2.2.json '{}'
after=$(commit_all "docs after the test")

good=$(jq -n --arg commit "$tested" --arg short "$short" '{
  version: "1.2.3",
  status: "passed",
  commit: $commit,
  date: "2026-10-10",
  omarchy: {version: "4.0.4-1", channel: "rc"},
  engine: "1.2.3+main.\($short)",
  plugin: "1.2.3",
  scenarios: [
    {id: "a", title: "clean home to the desk", where: "test host", result: "passed",
     counts: {humanSteps: 1, passwordPrompts: 0}, notes: "desk at 100 %"},
    {id: "b", title: "plugin update from 1.2.2", where: "test host", result: "passed",
     counts: {humanSteps: 2, passwordPrompts: 1, snapshotCoverage: 100, r3GatesHonoured: null, agentClosesReopened: 0}},
    {id: "check", title: "just check", where: "CI", result: "passed",
     counts: {humanSteps: 0, passwordPrompts: 0}}
  ],
  limitations: []
}')

# record BASE [JQ_FILTER]: the good record edited by JQ_FILTER, committed
# on top of BASE (detached); prints the new commit
record() {
  local base=$1 filter=${2:-.}
  g checkout -q --detach "$base"
  mkdir -p "$repo/packaging/acceptance"
  jq "$filter" <<<"$good" >"$repo/$rec"
  commit_all "record"
}

# run ARGS...: the checker in the scratch repository; exit code in $rc
run() {
  rc=0
  (cd "$repo" && bash "$script" "$@") >"$work/out" 2>"$work/err" || rc=$?
}

# expect_ok NAME PATTERN ARGS...: exit 0 with PATTERN on stdout
expect_ok() {
  local name=$1 pattern=$2
  shift 2
  run "$@"
  if [[ $rc == 0 ]] && grep -qF -- "$pattern" "$work/out"; then
    pass "$name"
  else
    fail "$name: exit $rc, stdout '$(cat "$work/out")', stderr '$(cat "$work/err")'"
  fi
}

# expect_refused NAME PATTERN ARGS...: exit 1 with PATTERN on stderr
expect_refused() {
  local name=$1 pattern=$2
  shift 2
  run "$@"
  if [[ $rc == 1 ]] && grep -qF -- "$pattern" "$work/err"; then
    pass "$name"
  else
    fail "$name: exit $rc, stderr '$(cat "$work/err")'"
  fi
}

# broken NAME PATTERN JQ_FILTER: the good record with one edit is refused
broken() {
  local ref
  ref=$(record "$after" "$3")
  expect_refused "$1" "$2" 1.2.3 "$ref"
}

# ---- the good record --------------------------------------------------------------

ok=$(record "$after")
expect_ok "a good record after bookkeeping commits" "acceptance-check: ok" 1.2.3
expect_ok "the summary names the tested commit" "commit   $tested, 2 later commit(s)" 1.2.3
expect_ok "the summary counts the scenarios" "status   passed: 3 passed, 0 not run" 1.2.3
expect_ok "the summary has each scenario's counts" "scenario b: passed (test host); 2 human step(s), 1 password prompt(s)" 1.2.3
expect_ok "the summary names Omarchy" "omarchy  4.0.4-1, channel rc" 1.2.3
expect_ok "a v prefix on the version" "acceptance-check: ok" v1.2.3
expect_ok "REF given" "at $ok (${ok:0:12})" 1.2.3 "$ok"
expect_ok "a release engine without build suffix" "acceptance-check: ok" 1.2.3 "$(record "$after" '.engine = "1.2.3"')"
expect_ok "record directly on the tested commit" "commit   $tested, 1 later commit(s)" 1.2.3 "$(record "$tested")"

# the committed record counts, not the working tree's
g checkout -q --detach "$ok"
jq '.status = "nonsense"' <<<"$good" >"$repo/$rec"
expect_ok "an uncommitted edit of the record is not read" "acceptance-check: ok" 1.2.3
g checkout -q -- "$rec"

# partial: a scenario not run, listed in limitations
partial=$(record "$after" '.status = "partial"
  | .scenarios[2] = {id: "check", title: "just check", where: "not run", result: "not run"}
  | .limitations = ["check: CI was down that day"]')
expect_ok "a partial record passes" "status   partial: 2 passed, 1 not run" 1.2.3 "$partial"
expect_ok "and lists its limitation" "limit    check: CI was down that day" 1.2.3 "$partial"

# ---- arguments and the record file ------------------------------------------------

expect_refused "no version" "usage: packaging/acceptance-check.sh X.Y.Z [REF]"
expect_refused "an empty version" "'' is not a version X.Y.Z" ""
expect_refused "three arguments" "usage:" 1.2.3 HEAD x
expect_refused "a version that is not X.Y.Z" "'1.2' is not a version X.Y.Z" 1.2
expect_refused "a REF that is no commit" "'nope' is not a commit" 1.2.3 nope
expect_refused "no record in REF" "no $rec in $tested" 1.2.3 "$tested"
expect_refused "the record of another version" "no packaging/acceptance/v1.2.4.json" 1.2.4 "$ok"

g checkout -q --detach "$after"
printf '{"version": ' >"$repo/$rec"
expect_refused "malformed JSON" "is not one JSON object" 1.2.3 "$(commit_all bad)"
g checkout -q --detach "$after"
printf '[]\n' >"$repo/$rec"
expect_refused "a list, not an object" "is not one JSON object" 1.2.3 "$(commit_all list)"
g checkout -q --detach "$after"
printf '%s\n%s\n' "$good" "$good" >"$repo/$rec"
expect_refused "two JSON objects" "holds more than one JSON value" 1.2.3 "$(commit_all two)"

# ---- each broken field ------------------------------------------------------------

broken "unknown top-level field" 'unknown field "limitation"' '.limitation = []'
broken "version differs" 'version is "1.2.4", not "1.2.3"' '.version = "1.2.4"'
broken "version missing" 'version is null' 'del(.version)'
broken "status not a known word" 'status is "ok"' '.status = "ok"'
broken "status missing" 'status is null' 'del(.status)'
broken "commit shortened" 'commit is "'"${tested:0:12}"'"' '.commit |= .[0:12]'
broken "commit in capitals" 'want the 40 lowercase hex' '.commit |= ascii_upcase'
broken "commit missing" 'commit is null' 'del(.commit)'
broken "date in another format" 'date is "10.10.2026"' '.date = "10.10.2026"'
broken "a date that does not exist" 'date is "2026-02-30"' '.date = "2026-02-30"'
broken "date missing" 'date is null' 'del(.date)'
broken "omarchy missing" 'omarchy is null' 'del(.omarchy)'
broken "omarchy a string" 'omarchy is "4.0.4-1"' '.omarchy = "4.0.4-1"'
broken "omarchy.version empty" 'omarchy.version is missing or empty' '.omarchy.version = ""'
broken "omarchy.channel missing" 'omarchy.channel is missing or empty' 'del(.omarchy.channel)'
broken "omarchy unknown field" 'unknown field "omarchy.host"' '.omarchy.host = "x"'
broken "engine of another version" 'engine is "1.2.2"' '.engine = "1.2.2"'
broken "engine from another commit" 'was built from 0000000, not from commit' '.engine = "1.2.3+main.0000000"'
broken "engine missing" 'engine is null' 'del(.engine)'
broken "plugin of another version" 'plugin is "1.2.2"' '.plugin = "1.2.2"'
broken "plugin missing" 'plugin is null' 'del(.plugin)'
broken "scenarios empty" 'scenarios is []' '.scenarios = []'
broken "scenarios missing" 'scenarios is null' 'del(.scenarios)'
broken "a scenario not an object" 'scenarios[0] is not an object' '.scenarios[0] = "a"'
broken "scenario unknown field" 'unknown field "scenarios[0] (a).steps"' '.scenarios[0].steps = 1'
broken "scenario id missing" 'scenarios[0]: id is null' 'del(.scenarios[0].id)'
broken "scenario id with a space" 'scenarios[0] (a b): id is "a b"' '.scenarios[0].id = "a b"'
broken "scenario id twice" 'scenario id "a" is used 2 times' '.scenarios[1].id = "a"'
broken "scenario title empty" 'scenarios[0] (a): title is missing or empty' '.scenarios[0].title = ""'
broken "where not an evidence word" 'scenarios[0] (a): where is "dev host"' '.scenarios[0].where = "dev host"'
broken "where missing" 'scenarios[0] (a): where is null' 'del(.scenarios[0].where)'
broken "result not a known word" 'scenarios[0] (a): result is "ok"' '.scenarios[0].result = "ok"'
broken "counts missing on a passed scenario" 'scenarios[0] (a): counts is null' 'del(.scenarios[0].counts)'
broken "humanSteps missing" 'counts.humanSteps is null' 'del(.scenarios[0].counts.humanSteps)'
broken "humanSteps negative" 'counts.humanSteps is -1' '.scenarios[0].counts.humanSteps = -1'
broken "passwordPrompts a fraction" 'counts.passwordPrompts is 0.5' '.scenarios[0].counts.passwordPrompts = 0.5'
broken "passwordPrompts a string" 'counts.passwordPrompts is "0"' '.scenarios[0].counts.passwordPrompts = "0"'
broken "counts unknown field" 'unknown field "scenarios[0] (a).counts.steps"' '.scenarios[0].counts.steps = 1'
broken "snapshotCoverage over 100" 'counts.snapshotCoverage is 120' '.scenarios[1].counts.snapshotCoverage = 120'
broken "r3GatesHonoured a string" 'counts.r3GatesHonoured is "all"' '.scenarios[1].counts.r3GatesHonoured = "all"'
broken "agentClosesReopened negative" 'counts.agentClosesReopened is -2' '.scenarios[1].counts.agentClosesReopened = -2'
broken "notes not a string" 'scenarios[0] (a): notes is 3' '.scenarios[0].notes = 3'
broken "limitations missing" 'limitations is null' 'del(.limitations)'
broken "limitations a string" 'limitations is "none"' '.limitations = "none"'
broken "an empty limitation" 'limitations is [""]' '.limitations = [""]'

# ---- the rules ----------------------------------------------------------------------

not_run='.scenarios[2] = {id: "check", title: "just check", where: "not run", result: "not run"}'
broken "a not run scenario under passed" 'status is "passed", but the scenarios say "partial" (2 passed, 0 failed, 1 not run)' \
  "$not_run | .limitations = [\"check: not run\"]"
broken "a failed scenario under passed" 'status is "passed", but the scenarios say "failed"' \
  '.scenarios[0].result = "failed"'
broken "a failed scenario under partial" 'status is "partial", but the scenarios say "failed"' \
  '.status = "partial" | .scenarios[0].result = "failed"'
broken "partial with every scenario passed" 'status is "partial", but the scenarios say "passed"' \
  '.status = "partial"'
broken "a not run scenario missing from limitations" 'scenario "check" was not run, but no limitations entry starts with "check: "' \
  ".status = \"partial\" | $not_run"
broken "a limitation that does not start with the id" 'no limitations entry starts with "check: "' \
  ".status = \"partial\" | $not_run | .limitations = [\"the check was not run\"]"
broken "where not run, result passed" 'where "not run" with result "passed"' \
  '.scenarios[0].where = "not run"'
broken "result not run, where test host" 'where "test host" with result "not run"' \
  '.status = "partial" | .scenarios[0] |= (.result = "not run" | del(.counts)) | .limitations = ["a: no host"]'
broken "counts on a scenario not run" 'scenarios[2] (check): counts on a scenario that was not run' \
  ".status = \"partial\" | $not_run | .scenarios[2].counts = {humanSteps: 0, passwordPrompts: 0} | .limitations = [\"check: x\"]"
broken "a failed live test" 'the live test failed: a' \
  '.status = "failed" | .scenarios[0].result = "failed"'

# ---- privacy -------------------------------------------------------------------------

broken "a private path" 'a private path (/home/… or /Users/…)' '.scenarios[0].notes = "see /home/someone/log"'
broken "a macOS home path" 'a private path' '.limitations = ["/Users/someone/x"]'
put scripts/guard-hosts.local $'# test hosts\nseldon-scratch-host'
broken "a host listed in guard-hosts.local" 'names this machine, its user or a listed host' \
  '.scenarios[0].notes = "ran on seldon-scratch-host"'
rm "$repo/scripts/guard-hosts.local"
put scripts/deploy-hosts.local 'seldon-pinned-host 0123456789abcdef'
broken "a host listed in deploy-hosts.local" 'names this machine, its user or a listed host' \
  '.limitations = ["Seldon-Pinned-Host was slow"]'
rm "$repo/scripts/deploy-hosts.local"
host=$(uname -n)
if [[ ${#host} -ge 3 && $host != localhost ]]; then
  broken "this machine's host name" 'names this machine' ".scenarios[0].notes = \"on $host\""
fi

# ---- the commit and what changed after it -------------------------------------------

g checkout -q --detach "$after"
put engine/src.rs 'fn main() { changed() }'
code=$(commit_all "engine change after the test")
expect_refused "engine code changed after the tested commit" "      engine/src.rs" 1.2.3 "$(record "$code")"
expect_refused "and the refusal says why" "1 path(s) outside docs/, work/, packaging/acceptance/ and *.md (not under engine/) changed" 1.2.3 "$(record "$code")"

g checkout -q --detach "$after"
put plugin/Panel.qml 'Item { id: changed }'
put .github/workflows/release.yml 'name: release, changed'
two=$(record "$(commit_all "plugin and workflow")")
expect_refused "every offending path is printed: plugin" "      plugin/Panel.qml" 1.2.3 "$two"
expect_refused "every offending path is printed: workflow" "      .github/workflows/release.yml" 1.2.3 "$two"
expect_refused "and counted" "2 path(s) outside" 1.2.3 "$two"

g checkout -q --detach "$after"
put engine/assets/skills/seldon/SKILL.md '# skill, changed'
skill=$(record "$(commit_all "skill")")
expect_refused "a Markdown file the engine compiles in" "      engine/assets/skills/seldon/SKILL.md" 1.2.3 "$skill"

g checkout -q --detach "$after"
g mv engine/src.rs docs/src.rs
renamed=$(record "$(commit_all "rename")")
expect_refused "code renamed into docs/ counts as the old path" "      engine/src.rs" 1.2.3 "$renamed"

g checkout -q --detach "$after"
chmod +x "$repo/plugin/Panel.qml"
expect_refused "a mode change" "      plugin/Panel.qml" 1.2.3 "$(record "$(commit_all "mode")")"

# a commit on another line of history, as after a rebase; it changes only
# docs, so the ancestor check alone refuses it
g checkout -q --detach "$tested"
put docs/SIDE.md 'side'
side=$(commit_all "side branch")
aside=$(record "$after" ".commit = \"$side\" | .engine = \"1.2.3+main.${side:0:7}\"")
expect_refused "a tested commit that is not an ancestor" "commit ${side:0:12} is not" 1.2.3 "$aside"
expect_refused "a tested commit not in the repository" "commit 0123456789abcdef0123456789abcdef01234567 is not in this repository" 1.2.3 \
  "$(record "$after" '.commit = "0123456789abcdef0123456789abcdef01234567" | .engine = "1.2.3"')"

# several problems at once are all reported
several=$(record "$code" '.plugin = "1.2.2" | .date = "x"')
expect_refused "several problems: the field" 'plugin is "1.2.2"' 1.2.3 "$several"
expect_refused "several problems: the date" 'date is "x"' 1.2.3 "$several"
expect_refused "several problems: the path" "      engine/src.rs" 1.2.3 "$several"

# ---- mutants: each guard is needed --------------------------------------------------

# mutant NAME SED_EXPR NAME_OF_CASE ARGS...: the checker with SED_EXPR
# applied must pass the case the real checker refuses
mutant() {
  local name=$1 expr=$2 copy=$work/mutant.sh rc=0
  shift 2
  sed -e "$expr" "$script" >"$copy"
  if cmp -s "$copy" "$script"; then
    fail "mutant $name: the sed expression changed nothing"
    return
  fi
  (cd "$repo" && bash "$copy" "$@") >/dev/null 2>&1 || rc=$?
  if [[ $rc == 0 ]]; then pass "mutant $name: caught"; else fail "mutant $name: still refuses (exit $rc), the test would not notice"; fi
}
# the sed expressions are the checker's text, `$` included (SC2016 is the point)
# shellcheck disable=SC2016
mutant "no ancestor check" 's/elif ! git merge-base --is-ancestor "$commit" "$ref_sha"; then/elif false; then/' 1.2.3 "$aside"
# shellcheck disable=SC2016
mutant "Markdown under engine/ allowed" '/        engine\/\*) offending+=("$changed") ;;/d' 1.2.3 "$skill"
mutant "renames detected" 's/ --no-renames / -M /' 1.2.3 "$renamed"
# shellcheck disable=SC2016
mutant "status not derived from the scenarios" 's/and .status != $derived then/and false then/' \
  1.2.3 "$(record "$after" "$not_run | .limitations = [\"check: not run\"]")"
# shellcheck disable=SC2016
mutant "failed status passes" 's/if \[\[ $status == failed/if [[ $status == never/' \
  1.2.3 "$(record "$after" '.status = "failed" | .scenarios[0].result = "failed"')"

if ((fails > 0)); then
  echo "acceptance-check.test: $fails failed" >&2
  exit 1
fi
echo "acceptance-check.test: ok"
