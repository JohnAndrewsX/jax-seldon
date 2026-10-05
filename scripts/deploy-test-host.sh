#!/usr/bin/env bash
# deploy-test-host.sh — put the main build of engine and plugin on the test
# host, or bring the test host back to a release (WP-098).
#
#   scripts/deploy-test-host.sh [--dry-run] CHECK_LOG
#   scripts/deploy-test-host.sh [--dry-run] --release vX.Y.Z
#
# The test host follows main so the operator can watch development live;
# productive machines run releases only (install.sh, `omarchy plugin
# update`) and are never a target: the host comes from SELDON_TEST_HOST and
# must be listed in scripts/guard-hosts.local (git-ignored, one host per
# line, `#` comments; the same list the guard hook reads). Anything else is
# refused, and so is a host whose /etc/machine-id is this machine's.
#
# Main build (CHECK_LOG): refuses unless this checkout is on `main`,
# clean, HEAD equals origin/main, and CHECK_LOG — the orchestrator's main
# check log — starts with `head <full sha>`, ends in `exit 0`, its sha is
# HEAD or an ancestor, and nothing under engine/, plugin/, schema/ (the
# engine compiles the schemas in) or this script changed between that sha
# and HEAD (bookkeeping commits after the check pass). Before the first change the host must have the tools the install
# step uses. Then it builds the static engine as a release does
# (`--features watch`) with SELDON_BUILD=main.<short sha> (version
# 0.1.3+main.<sha>), and on the host:
#   - copies it to ~/.local/bin/seldon, the previous one kept as seldon.prev;
#   - syncs HEAD's plugin/ into ~/.config/omarchy/plugins/jax.seldon. A
#     plugin dir that is not a dev copy yet (the release git clone) is moved
#     once to ~/.local/state/seldon-dev/plugin-<git|copy>-<UTC stamp>;
#     afterwards the dir is a plain copy whose .seldon-dev-build names the
#     build and the commit;
#   - restarts the shell when the plugin files changed (or a restart is
#     still pending from an earlier deploy), and only while `omarchy-shell
#     lock status` reports neither locked nor secure; otherwise it prints
#     "restart pending" and the next deploy tries again (while it is
#     pending, a restart notice in the smoke is a note, not a failure);
#   - smoke: `seldon --version`, `seldon doctor --json` (no error),
#     `seldon capture --json` on the host's configured (test) logbook, and
#     `jax.seldon.service status`: status ok, engineVersion the new version,
#     no restart notice; without a graphical session (the shell does not
#     answer) the smoke fails and says so;
#   - appends one JSON line to ~/.local/state/seldon-dev/deploy.jsonl and
#     prints a summary.
#
# --release vX.Y.Z: installs that release on the host with its install.sh
# (checked against the release's SHA256SUMS, run with --force because the
# dev binary was not installed by it) and the plugin as a git clone of
# jax-seldon-plugin at the tag (the dev copy moved aside like above), then
# the same restart, smoke and log. State written by a newer build may not
# load in an older release: move ~/.local/state/seldon aside first.
#
# --dry-run: every refusal, one read-only look at the host, the hostname
# `ssh -G` resolves the alias to, then the plan; nothing is built, copied
# or restarted.
#
# Exit codes: 0 deployed (a restart may be pending); 1 refused, nothing
# changed; 2 build, copy, install or smoke failure.
#
# Test hooks (tests/deploy/deploy-test-host.test.sh): GUARD_HOSTS_FILE
# replaces scripts/guard-hosts.local (as in guard.sh), SELDON_DEPLOY_LOCAL_ID
# this machine's id, SELDON_DEPLOY_SETTLE the seconds the shell gets after
# the sync before a restart (default 5), SELDON_DEPLOY_WAIT the seconds the
# smoke waits for the service (default 60). Stubs first on PATH stand in for
# cargo and ssh.
#
# shellcheck disable=SC2016 # the remote scripts are single-quoted on purpose: they expand on the host
set -euo pipefail

root=$(cd "$(dirname "$0")/.." && pwd)
target=x86_64-unknown-linux-musl
release_repo=https://github.com/JohnAndrewsX/jax-seldon
plugin_repo=https://github.com/JohnAndrewsX/jax-seldon-plugin.git
settle=${SELDON_DEPLOY_SETTLE:-5}
wait_s=${SELDON_DEPLOY_WAIT:-60}

say() { printf '%s\n' "$*"; }
warn() { printf 'deploy-test-host: %s\n' "$*" >&2; }
refuse() { warn "refused: $*"; exit 1; }
fail() { warn "failed: $*"; exit 2; }
usage() { awk 'NR > 1 && /^#/ { sub(/^# ?/, ""); print; next } NR > 1 { exit }' "$0"; }

# ---- arguments ----------------------------------------------------------------

dry=0
release=""
check_log=""
while [[ $# -gt 0 ]]; do
  case $1 in
    --dry-run) dry=1; shift ;;
    --release)
      [[ $# -ge 2 ]] || refuse "--release needs vX.Y.Z"
      release=$2; shift 2 ;;
    -h | --help) usage; exit 0 ;;
    -*) refuse "unknown option '$1'" ;;
    *)
      [[ -z $check_log ]] || refuse "one check log only"
      check_log=$1; shift ;;
  esac
done
if [[ -n $release ]]; then
  [[ -z $check_log ]] || refuse "--release takes no check log"
  [[ $release =~ ^v[0-9]+\.[0-9]+\.[0-9]+$ ]] || refuse "--release wants vX.Y.Z, not '$release'"
else
  [[ -n $check_log ]] || refuse "usage: deploy-test-host.sh [--dry-run] CHECK_LOG | [--dry-run] --release vX.Y.Z"
fi
for tool in git jq ssh tar base64 sha256sum; do
  command -v "$tool" >/dev/null || refuse "needs '$tool'"
done

# ---- the host: only a listed test host -----------------------------------------

host=${SELDON_TEST_HOST:-}
[[ -n $host ]] || refuse "SELDON_TEST_HOST is not set (the test host's ssh alias, memory/local.md)"
# an ssh destination, never an option
[[ $host =~ ^[A-Za-z0-9][A-Za-z0-9._@-]*$ ]] || refuse "SELDON_TEST_HOST '$host' is not a plain ssh alias"
hosts_file=${GUARD_HOSTS_FILE:-$root/scripts/guard-hosts.local}
[[ -r $hosts_file ]] || refuse "no test host list at $hosts_file; only a host listed there is a target"
listed=0
while IFS= read -r line || [[ -n $line ]]; do
  line=${line%%#*}
  line=${line//[[:space:]]/}
  [[ $line == "$host" ]] && listed=1
done <"$hosts_file"
[[ $listed == 1 ]] || refuse "'$host' is not listed in $hosts_file; productive machines are never a target"

# ---- main: clean, pushed, checked ----------------------------------------------

git_() { git -C "$root" "$@"; }
if [[ -z $release ]]; then
  branch=$(git_ symbolic-ref --quiet --short HEAD || echo "(detached)")
  [[ $branch == main ]] || refuse "not on main (on $branch)"
  [[ -z $(git_ status --porcelain) ]] || refuse "the working tree is not clean (git status)"
  head=$(git_ rev-parse HEAD)
  upstream=$(git_ rev-parse --verify --quiet origin/main) || refuse "no origin/main"
  [[ $head == "$upstream" ]] || refuse "HEAD ${head:0:12} is not origin/main ${upstream:0:12}; push first"
  [[ -f $check_log && -r $check_log ]] || refuse "check log '$check_log' not found"
  ! grep -q $'\r' "$check_log" || refuse "the check log has Windows line endings (CRLF); use the log the check wrote"
  last=$(awk 'NF { l = $0 } END { print l }' "$check_log")
  [[ $last == "exit 0" ]] || refuse "the check log does not end in 'exit 0' (last line: '$last')"
  # the commit the check ran on: the log's first line `head <full sha>`
  checked=$(sed -n '1s/^head \([0-9a-f]\{40\}\)$/\1/p' "$check_log")
  [[ -n $checked ]] || refuse "the check log does not start with 'head <full sha>'; which tree it checked is unknown"
  git_ merge-base --is-ancestor "$checked" HEAD 2>/dev/null \
    || refuse "the checked commit ${checked:0:12} is not HEAD or an ancestor of it"
  # schema/: engine/src/index/check.rs compiles the schemas in (include_str!)
  git_ diff --quiet "$checked" HEAD -- engine plugin schema scripts/deploy-test-host.sh \
    || refuse "engine/, plugin/, schema/ or the deploy script changed since the checked commit ${checked:0:12}; run the check on this HEAD"
  short=$(git_ rev-parse --short HEAD)
  version="$(awk -F'"' '/^version *=/ { print $2; exit }' "$root/engine/Cargo.toml")+main.$short"
  mode=main
else
  head="" short=""
  version=${release#v}
  mode=release
fi

# ---- remote helpers -------------------------------------------------------------

# The hash of a plugin dir's files (names and contents; .git and the dev
# marker left out), the same here and on the host.
plugin_hash() {
  if [[ -d $1 ]]; then
    (cd "$1" && find . -type f ! -path './.git/*' ! -path ./.seldon-dev-build -print0 \
      | LC_ALL=C sort -z | xargs -0 -r sha256sum | sha256sum | cut -c1-16)
  else
    echo absent
  fi
}

# Every remote script starts with this: OMARCHY_PATH, its bin dir and the
# Wayland session, which non-interactive ssh has none of (memory/pitfalls.md).
prelude='set -uo pipefail
export OMARCHY_PATH=${OMARCHY_PATH:-/usr/share/omarchy}
export PATH="$OMARCHY_PATH/bin:$HOME/.local/bin:$PATH"
export XDG_RUNTIME_DIR=${XDG_RUNTIME_DIR:-/run/user/$(id -u)}
export WAYLAND_DISPLAY=${WAYLAND_DISPLAY:-$(ls "$XDG_RUNTIME_DIR" 2>/dev/null | grep -m 1 -E "^wayland-[0-9]+$" || echo wayland-1)}
cd "$HOME" || exit 2
plugin_dir=.config/omarchy/plugins/jax.seldon
dev_dir=.local/state/seldon-dev
marker=.seldon-dev-build
'"$(declare -f plugin_hash)"'
# neither locked nor secure; an unreadable status counts as locked
lock_free() { omarchy-shell lock status 2>/dev/null | jq -e "(.locked or .sessionLocked or .secure) | not" >/dev/null 2>&1; }
# Move a plugin dir that is not a dev copy out of the plugins dir (the shell
# scans every dir there), once, to a dated backup.
move_aside() {
  local kind=copy dest
  [[ -d $plugin_dir/.git ]] && kind=git
  [[ -f $plugin_dir/$marker ]] && kind=dev
  mkdir -p "$dev_dir" || return 1
  dest=$dev_dir/plugin-$kind-$(date -u +%Y%m%dT%H%M%SZ)
  mv -- "$plugin_dir" "$dest" || return 1
  echo "moved=$dest"
}'

# rsh <script> — run a bash script on the host. The script travels as base64
# in the argument; stdin is passed through (the install step sends a tar).
rsh() {
  local b64
  b64=$(printf '%s\n%s\n' "$prelude" "$1" | base64 -w 0)
  ssh -o BatchMode=yes -o ConnectTimeout=10 -- "$host" "bash -c \"\$(echo $b64 | base64 -d)\""
}
# value <key> <key=value lines>
value() { awk -v k="$1" 'index($0, k "=") == 1 { print substr($0, length(k) + 2); exit }' <<<"$2"; }

# ---- look at the host (read-only) ----------------------------------------------

if [[ $mode == main ]]; then
  tools="rsync jq tar mktemp install sha256sum find xargs omarchy omarchy-shell omarchy-restart-shell"
else
  tools="curl git jq sha256sum find xargs omarchy omarchy-shell omarchy-restart-shell"
fi
probe=$(rsh "tools='$tools'"'
echo "id=$(cat /etc/machine-id 2>/dev/null || hostname)"
missing=""
for t in $tools; do command -v "$t" >/dev/null 2>&1 || missing+=" $t"; done
echo "missing=${missing# }"
if [[ -L $plugin_dir ]]; then echo plugin=symlink
elif [[ -f $plugin_dir/$marker ]]; then echo plugin=dev
elif [[ -d $plugin_dir/.git ]]; then echo plugin=git
elif [[ -d $plugin_dir ]]; then echo plugin=copy
else echo plugin=absent; fi
echo "hash=$(plugin_hash "$plugin_dir")"
echo "deployed=$(sed -n "s/^build=//p" "$plugin_dir/$marker" 2>/dev/null)"
echo "engine=$(seldon --version --json 2>/dev/null | jq -r ".version // empty" 2>/dev/null)"
echo "pending=$([[ -f $dev_dir/restart-pending ]] && echo 1 || echo 0)"
echo "lock=$(lock_free && echo free || echo held)"' </dev/null) \
  || refuse "cannot reach $host over ssh"
local_id=${SELDON_DEPLOY_LOCAL_ID:-$(cat /etc/machine-id 2>/dev/null || hostname)}
[[ $(value id "$probe") != "$local_id" ]] || refuse "$host is this machine; the test host must be another one"
[[ $(value plugin "$probe") != symlink ]] || refuse "$host's plugin dir is a symlink (a dev link?); not touching it"
missing=$(value missing "$probe")
[[ -z $missing ]] || refuse "$host lacks what the deploy needs: $missing"

# ---- what would change ----------------------------------------------------------

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
if [[ $mode == main ]]; then
  # HEAD's plugin/, not the working tree: no ignored files (.qmlls.ini) go out
  mkdir -p "$work/stage"
  git_ archive --format=tar HEAD plugin | tar -x -C "$work/stage"
  new_hash=$(plugin_hash "$work/stage/plugin")
  if [[ $new_hash == "$(value hash "$probe")" ]]; then plugin_change=no; else plugin_change=yes; fi
else
  plugin_change="yes (release clone)"
fi
restart_plan="none"
if [[ $plugin_change != no || $(value pending "$probe") == 1 ]]; then
  if [[ $(value lock "$probe") == free ]]; then restart_plan="restart the shell"; else restart_plan="restart pending (session locked)"; fi
fi

if [[ $mode == release ]]; then
  warn "state written by a newer build may not load in $release: move ~/.local/state/seldon aside on $host first (mv ~/.local/state/seldon ~/.local/state/seldon.main-\$(date +%F))"
fi

if [[ $dry == 1 ]]; then
  resolved=$(ssh -G -- "$host" 2>/dev/null | awk '$1 == "hostname" { print $2; exit }' || true)
  say "deploy-test-host (dry run): $host (ssh resolves it to ${resolved:-?})"
  deployed=$(value deployed "$probe")
  engine_now=$(value engine "$probe")
  say "  now      engine ${engine_now:-not installed or not answering}, plugin dir $(value plugin "$probe")${deployed:+ ($deployed)}"
  if [[ $mode == main ]]; then
    say "  build    SELDON_BUILD=main.$short cargo build --release --features watch --target $target  → $version"
    say "  engine   copy to ~/.local/bin/seldon (previous → seldon.prev)"
  else
    say "  engine   install.sh --version $release --force (checked against the release's SHA256SUMS)"
  fi
  case $(value plugin "$probe") in
    git | copy) say "  plugin   move the $(value plugin "$probe") dir aside to ~/.local/state/seldon-dev/, then install" ;;
  esac
  if [[ $mode == main ]]; then
    say "  plugin   sync HEAD's plugin/ into ~/.config/omarchy/plugins/jax.seldon; files change: $plugin_change"
  else
    say "  plugin   clone jax-seldon-plugin at $release into ~/.config/omarchy/plugins/jax.seldon"
  fi
  say "  restart  $restart_plan"
  say "  smoke    --version, doctor, capture, jax.seldon.service status (expect $version)"
  say "  log      ~/.local/state/seldon-dev/deploy.jsonl"
  exit 0
fi

# ---- build (main) -----------------------------------------------------------------

if [[ $mode == main ]]; then
  say "== build $version"
  # as release.yml and the PKGBUILD build it; --target-dir so that an
  # exported CARGO_TARGET_DIR cannot leave an older binary at $bin
  SELDON_BUILD=main.$short cargo build --manifest-path "$root/engine/Cargo.toml" --locked --release \
    --features watch --target "$target" --target-dir "$root/engine/target" --quiet \
    || fail "the release build"
  bin=$root/engine/target/$target/release/seldon
  got=$("$bin" --version --json | jq -r .version) || fail "the new binary does not run"
  [[ $got == "$version" ]] || fail "the new binary reports '$got', not $version"
  cp -- "$bin" "$work/stage/seldon"
  printf 'build=main.%s\ncommit=%s\ndeployed=%s\n' "$short" "$head" "$(date -u +%FT%TZ)" >"$work/stage/build-info"
fi

# ---- install on the host --------------------------------------------------------

moved="" install_out="" restart="none" smoke="not run" failures=()

# log_line — one JSON line per deploy on the host, whatever the outcome.
log_line() {
  local line
  line=$(jq -cn --arg ts "$(date -u +%FT%TZ)" --arg mode "$mode" --arg version "$version" \
    --arg commit "$head" --arg change "$plugin_change" --arg moved "$moved" --arg restart "$restart" \
    --arg smoke "$smoke" --args '{ts: $ts, mode: $mode, version: $version, commit: $commit,
      pluginChanged: ($change != "no"), movedAside: $moved, restart: $restart, smoke: $smoke,
      failures: $ARGS.positional}' "${failures[@]+"${failures[@]}"}")
  rsh "mkdir -p \"\$dev_dir\" && printf '%s\n' $(printf '%q' "$line") >>\"\$dev_dir/deploy.jsonl\"" </dev/null \
    || warn "could not append to the deploy log on $host"
}
# abort <what> — a failure after the first change: log it, exit 2.
abort() { failures+=("$1"); smoke="not run"; log_line; fail "$1"; }

if [[ $mode == main ]]; then
  say "== install on $host"
  install_out=$(tar -C "$work/stage" -cf - plugin seldon build-info | rsh '
mkdir -p .cache && tmp=$(mktemp -d "$HOME/.cache/seldon-dev-deploy.XXXXXX") || { echo "error=temp dir"; exit 2; }
trap "rm -rf \"\$tmp\"" EXIT
tar -x -C "$tmp" || { echo "error=unpack" ; exit 2; }
[[ ! -L $plugin_dir ]] || { echo "error=plugin dir is a symlink"; exit 2; }
mkdir -p .local/bin "$dev_dir" .config/omarchy/plugins
# engine: the previous one stays as seldon.prev; mv keeps the swap atomic
install -m 755 "$tmp/seldon" .local/bin/seldon.new || { echo "error=copy engine"; exit 2; }
if [[ -e .local/bin/seldon ]]; then cp -pP .local/bin/seldon .local/bin/seldon.prev || { echo "error=keep seldon.prev"; exit 2; }; fi
mv -f .local/bin/seldon.new .local/bin/seldon || { echo "error=swap engine"; exit 2; }
# plugin
before=$(plugin_hash "$plugin_dir")
if [[ -e $plugin_dir && ! -f $plugin_dir/$marker ]]; then move_aside || { echo "error=move the plugin dir aside"; exit 2; }; fi
mkdir -p "$plugin_dir"
rsync -rlp --checksum --delete --exclude="/$marker" "$tmp/plugin/" "$plugin_dir/" || { echo "error=sync plugin"; exit 2; }
cp "$tmp/build-info" "$plugin_dir/$marker"
# pending before the validation: a failed one stops this deploy before the
# restart, and the next deploy that validates restarts
if [[ $(plugin_hash "$plugin_dir") != "$before" ]]; then touch "$dev_dir/restart-pending"; echo changed=yes; else echo changed=no; fi
omarchy plugin validate "$plugin_dir" >/dev/null 2>&1 || { echo "error=omarchy plugin validate"; exit 2; }') \
    || { moved=$(value moved "$install_out"); abort "install: $(value error "$install_out")"; }
else
  say "== install $release on $host"
  install_out=$(rsh "v=$(printf '%q' "$release") plugin_repo=$(printf '%q' "$plugin_repo") base=$(printf '%q' "$release_repo/releases/download")"'
mkdir -p .cache && tmp=$(mktemp -d "$HOME/.cache/seldon-dev-deploy.XXXXXX") || { echo "error=temp dir"; exit 2; }
trap "rm -rf \"\$tmp\"" EXIT
[[ ! -L $plugin_dir ]] || { echo "error=plugin dir is a symlink"; exit 2; }
curl -fsSL --proto "=https" -o "$tmp/install.sh" "$base/$v/install.sh" \
  && curl -fsSL --proto "=https" -o "$tmp/SHA256SUMS" "$base/$v/SHA256SUMS" || { echo "error=download install.sh of $v"; exit 2; }
grep -q " install.sh$" "$tmp/SHA256SUMS" && (cd "$tmp" && sha256sum -c --ignore-missing --quiet SHA256SUMS) >/dev/null 2>&1 \
  || { echo "error=install.sh does not match the release'"'"'s SHA256SUMS"; exit 2; }
git clone --quiet --branch "$v" -- "$plugin_repo" "$tmp/plugin" >/dev/null 2>&1 || { echo "error=clone the plugin at $v"; exit 2; }
omarchy plugin validate "$tmp/plugin" >/dev/null 2>&1 || { echo "error=omarchy plugin validate"; exit 2; }
bash "$tmp/install.sh" --version "$v" --force >&2 || { echo "error=install.sh --version $v"; exit 2; }
before=$(plugin_hash "$plugin_dir")
if [[ -e $plugin_dir ]]; then move_aside || { echo "error=move the plugin dir aside"; exit 2; }; fi
mkdir -p "$(dirname "$plugin_dir")" "$dev_dir"
mv -- "$tmp/plugin" "$plugin_dir" || { echo "error=put the clone in place"; exit 2; }
if [[ $(plugin_hash "$plugin_dir") != "$before" ]]; then touch "$dev_dir/restart-pending"; echo changed=yes; else echo changed=no; fi' </dev/null) \
    || { moved=$(value moved "$install_out"); abort "install: $(value error "$install_out")"; }
fi
moved=$(value moved "$install_out")
plugin_change=$(value changed "$install_out")

# ---- restart (only on a free session) ---------------------------------------------

say "== restart"
restart=$(rsh "settle=$(printf '%q' "$settle")"'
[[ -f $dev_dir/restart-pending ]] || { echo none; exit 0; }
# the sync makes the shell hot-reload once per file; a restart in the middle
# of that crashed it once (WP-013): wait until it answers, then a bit more
for i in $(seq 20); do omarchy-shell shell ping >/dev/null 2>&1 && break; sleep 0.5; done
sleep "$settle"
lock_free || { echo "pending (session locked)"; exit 0; }
if omarchy-restart-shell >/dev/null 2>&1; then rm -f "$dev_dir/restart-pending"; echo done
else echo "pending (omarchy-restart-shell failed)"; fi' </dev/null) || restart="pending (no answer)"

# ---- smoke -----------------------------------------------------------------------

say "== smoke on $host"
case $restart in pending*) restart_pending=1 ;; *) restart_pending=0 ;; esac
smoke_out=$(rsh "want=$(printf '%q' "$version") wait_s=$(printf '%q' "$wait_s") pending=$restart_pending"'
check() { if [[ $2 == "$3" ]]; then echo "ok   $1"; else echo "FAIL $1: $2 (want $3)"; fi; }
check "seldon --version --json" "$(seldon --version --json 2>/dev/null | jq -r .version 2>/dev/null)" "$want"
out=$(seldon doctor --json 2>/dev/null); rc=$?
check "seldon doctor --json exit" "$rc" 0
deg=$(jq -r "[.checks[]? | select(.status != \"ok\") | \"\(.name) \(.status)\"] | join(\", \")" <<<"$out" 2>/dev/null)
[[ -z $deg ]] || echo "note doctor: $deg"
seldon capture --json >/dev/null 2>&1; check "seldon capture --json exit" "$?" 0
# no graphical session (logged out): the shell does not answer at all
deadline=$((SECONDS + (wait_s < 10 ? wait_s : 10)))
until omarchy-shell shell ping >/dev/null 2>&1; do
  ((SECONDS < deadline)) || { echo "FAIL service: no graphical session (the shell does not answer ping)"; exit 0; }
  sleep 0.5
done
omarchy-shell jax.seldon.service refresh >/dev/null 2>&1
deadline=$((SECONDS + wait_s)); snap=""
while ((SECONDS < deadline)); do
  snap=$(omarchy-shell jax.seldon.service status 2>/dev/null)
  jq -e --arg v "$want" ".status == \"ok\" and .engineVersion == \$v and (.busy | not)" >/dev/null 2>&1 <<<"$snap" && break
  sleep 1
done
check "service status" "$(jq -r ".status // empty" <<<"$snap" 2>/dev/null)" ok
check "service engineVersion" "$(jq -r ".engineVersion // empty" <<<"$snap" 2>/dev/null)" "$want"
notice=$(jq -r ".restartNotice // empty" <<<"$snap" 2>/dev/null)
# a deliberately pending restart is why the old code shows a notice
if [[ -n $notice && $pending == 1 ]]; then echo "note restart notice while the restart is pending: $notice"
else check "service restart notice" "$notice" ""; fi' </dev/null) \
  || smoke_out+=$'\nFAIL smoke: no answer from the host'
say "$smoke_out" | sed 's/^/  /'
while IFS= read -r line; do
  [[ $line == FAIL* ]] && failures+=("${line#FAIL }")
done <<<"$smoke_out"
if ((${#failures[@]} == 0)); then smoke=ok; else smoke=failed; fi
log_line

# ---- summary ----------------------------------------------------------------------

say ""
say "deploy-test-host: $host runs $version${head:+ (commit ${head:0:12})}"
engine_was=$(value engine "$probe")
if [[ -n $engine_was ]]; then
  say "  engine   ~/.local/bin/seldon (previous kept as seldon.prev; was $engine_was)"
else
  say "  engine   ~/.local/bin/seldon (there was none before)"
fi
say "  plugin   files changed: $plugin_change${moved:+; moved aside to ~/$moved}"
case $restart in
  done) say "  restart  done" ;;
  none) say "  restart  not needed" ;;
  *)
    reason=${restart#pending (}
    say "  restart  restart pending (${reason%)}): the shell keeps the old plugin code until"
    say "           omarchy-restart-shell runs on an unlocked session; the next deploy tries again" ;;
esac
if [[ $smoke == ok ]]; then
  say "  smoke    ok"
elif [[ " ${failures[*]} " == *"no graphical session"* ]]; then
  say "  smoke    failed: no graphical session on $host (nobody logged in?), so the plugin"
  say "           was not checked; engine and plugin are installed"
else
  say "  smoke    failed: $(IFS=';'; echo "${failures[*]}")"
fi
say "  log      ~/.local/state/seldon-dev/deploy.jsonl"
[[ $smoke == ok ]] || exit 2
