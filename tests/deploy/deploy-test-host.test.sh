#!/usr/bin/env bash
# scripts/deploy-test-host.sh against a fake test host (WP-098).
#
# VERBOSE=1 prints the script's output for the main cases.
# Everything runs in a temp dir: a scratch git repository with a bare
# origin holds a copy of the script, a tiny engine/ and plugin/; a `cargo`
# stub "builds" a fake seldon that reports 0.1.3+$SELDON_BUILD. An `ssh`
# stub runs the remote scripts here under `env -i` with a scratch HOME, and
# a PATH of stubs (omarchy-shell, omarchy-restart-shell, omarchy, curl, git
# clone) plus a whitelist of plain tools linked one by one: the host's real
# omarchy-*, quickshell, hyprctl and systemctl are not reachable, so the
# running shell is never touched. The host name ends in .invalid, so a real
# ssh would not resolve it either. The real ~/.local/bin/seldon, the
# jax.seldon plugin dir and ~/.local/state/seldon-dev are fingerprinted
# before and compared after.
#
# shellcheck disable=SC2016 # stub and fake-release text is single-quoted on purpose
set -euo pipefail

root=$(cd "$(dirname "$0")/../.." && pwd)
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

pass=0
fail=0
ok() { pass=$((pass + 1)); echo "ok   $*"; }
bad() { fail=$((fail + 1)); echo "FAIL $*"; }
check() { # name command...
  local name=$1
  shift
  if "$@"; then ok "$name"; else bad "$name"; fi
}

# ---- the real home, before ------------------------------------------------------
real_home=$HOME
real_fingerprint() {
  local p
  for p in "$real_home/.local/bin/seldon" "$real_home/.config/omarchy/plugins/jax.seldon" \
    "$real_home/.local/state/seldon-dev"; do
    if [[ -e $p || -L $p ]]; then
      find "$p" -printf '%p %y %s %T@ %l\n' 2>/dev/null | LC_ALL=C sort | sha256sum
    else
      echo "$p absent"
    fi
  done
}
real_before=$(real_fingerprint)

host=seldon-deploy-test.invalid
L=$work/local-bin   # stubs on the local PATH: cargo, ssh
R=$work/remote      # the fake host: home, stubs, switches
repo=$work/repo
mkdir -p "$L" "$R/omarchy/bin" "$R/bin" "$R/run"
printf '# test hosts\n%s  # the fake one\nother-host\n' "$host" >"$work/hosts"
# the fake host's machine-id, as the probe reads it (the stub runs here)
remote_id=$(cat /etc/machine-id 2>/dev/null || cat /proc/sys/kernel/hostname)
printf '# alias machine-id\n# %s 0000-a-commented-pin\nother-host 1111\n%s %s  # pinned\n' "$host" "$host" "$remote_id" >"$work/pins"

# make-seldon <version> prints a seldon that answers what the smoke asks
# (the stubs below call it too).
cat >"$work/make-seldon" <<EOF
#!/bin/bash
cat <<SELDON
#!/bin/bash
case "\\\$1 \\\${2:-}" in
  "--version --json") echo '{"name":"seldon","version":"\$1"}' ;;
  "--version ") echo "seldon \$1" ;;
  "doctor --json") echo '{"ok":true,"checks":[{"name":"snapper","status":"degraded"}]}'; exit \\\$(cat "$R/doctor_rc" 2>/dev/null || echo 0) ;;
  "capture --json") echo '{}'; exit \\\$(cat "$R/capture_rc" 2>/dev/null || echo 0) ;;
  *) exit 1 ;;
esac
SELDON
EOF
chmod 755 "$work/make-seldon"
fake_seldon() { "$work/make-seldon" "$1"; }

# ---- local stubs ------------------------------------------------------------------
cat >"$L/cargo" <<EOF
#!/bin/bash
echo "cargo \$* SELDON_BUILD=\${SELDON_BUILD-unset}" >>"$work/cargo.log"
[[ ! -f "$work/cargo_fail" ]] || exit 101
manifest="" dir=""
while [[ \$# -gt 0 ]]; do
  [[ \$1 == --manifest-path ]] && manifest=\$2
  [[ \$1 == --target-dir ]] && dir=\$2
  shift
done
# as cargo: --target-dir, else CARGO_TARGET_DIR, else next to the manifest
[[ -n \$dir ]] || dir=\${CARGO_TARGET_DIR:-\$(dirname "\$manifest")/target}
out=\$dir/x86_64-unknown-linux-musl/release
mkdir -p "\$out"
v="0.1.3+\$SELDON_BUILD"
[[ ! -f "$work/cargo_version" ]] || v=\$(cat "$work/cargo_version")
"$work/make-seldon" "\$v" >"\$out/seldon"
chmod 755 "\$out/seldon"
EOF

cat >"$L/ssh" <<EOF
#!/bin/bash
# ssh -G: the resolved config, no connection
for a; do [[ \$a == -G ]] && { echo "user test"; echo "hostname 192.0.2.7"; exit 0; }; done
while [[ \$# -gt 0 ]]; do
  case \$1 in -o) shift 2 ;; --) shift; break ;; -*) shift ;; *) break ;; esac
done
echo "\$1" >>"$work/ssh.log"
shift
exec env -i HOME="$R/home" PATH="$R/bin" OMARCHY_PATH="$R/omarchy" XDG_RUNTIME_DIR="$R/run" LC_ALL=C bash -c "\$*"
EOF

# ---- remote stubs -----------------------------------------------------------------
# plain tools the remote scripts use, nothing else
for t in bash sh cat chmod mkdir mv cp rm install touch date sed awk grep find sort xargs sha256sum \
  cut mktemp tar rsync jq base64 id ls seq sleep dirname stat env tr head tail wc; do
  p=$(command -v "$t") || { echo "deploy-test-host.test: needs $t on PATH" >&2; exit 1; }
  ln -s "$p" "$R/bin/$t"
done
cat >"$R/omarchy/bin/omarchy-shell" <<EOF
#!/bin/bash
echo "omarchy-shell \$*" >>"$R/calls"
case "\$1 \${2:-}" in
  "lock status") [[ -f "$R/lock.json" && ! -f "$R/no_session" ]] && cat "$R/lock.json" || exit 1 ;;
  "shell ping") [[ ! -f "$R/no_session" ]] ;;
  "jax.seldon.service refresh") echo ok ;;
  "jax.seldon.service status")
    v=\$(seldon --version --json | jq -r .version)
    [[ ! -f "$R/service_version" ]] || v=\$(cat "$R/service_version")
    printf '{"status":"ok","engineVersion":"%s","busy":false,"restartNotice":"%s"}\n' "\$v" "\$(cat "$R/restart_notice" 2>/dev/null)" ;;
  *) exit 1 ;;
esac
EOF
cat >"$R/omarchy/bin/omarchy-restart-shell" <<EOF
#!/bin/bash
echo "restart" >>"$R/calls"
exit \$(cat "$R/restart_rc" 2>/dev/null || echo 0)
EOF
cat >"$R/omarchy/bin/omarchy" <<EOF
#!/bin/bash
echo "omarchy \$*" >>"$R/calls"
[[ "\$1 \${2:-}" == "plugin validate" ]] || exit 1
exit \$(cat "$R/validate_rc" 2>/dev/null || echo 0)
EOF
# curl -fsSL --proto … -o FILE URL: serves $R/release/<tag>/<name>
cat >"$R/bin/curl" <<EOF
#!/bin/bash
echo "curl \$*" >>"$R/calls"
while [[ \$# -gt 1 ]]; do [[ \$1 == -o ]] && out=\$2; shift; done
url=\$1; name=\${url##*/}; tag=\${url%/*}; tag=\${tag##*/}
[[ -f "$R/release/\$tag/\$name" ]] || exit 22
cp "$R/release/\$tag/\$name" "\$out"
EOF
# git clone --quiet --branch TAG -- URL DIR: a clone with the release's plugin
cat >"$R/bin/git" <<EOF
#!/bin/bash
echo "git \$*" >>"$R/calls"
[[ \$1 == clone ]] || exit 1
for a; do dest=\$a; done
for a; do [[ \$prev == --branch ]] && tag=\$a; prev=\$a; done
[[ -d "$R/release/\$tag/plugin" ]] || exit 128
cp -r "$R/release/\$tag/plugin" "\$dest" && mkdir -p "\$dest/.git"
EOF
# systemctl: only `--user is-active --quiet` and `--user restart` of the
# watcher unit; the restart records the engine version it would start
cat >"$R/bin/systemctl" <<EOF
#!/bin/bash
case "\$*" in
  "--user is-active --quiet seldon-watch.service") [[ -f "$R/watch_active" ]] ;;
  "--user restart seldon-watch.service")
    echo "watch restart on \$(seldon --version)" >>"$R/calls"
    exit \$(cat "$R/watch_restart_rc" 2>/dev/null || echo 0) ;;
  *) echo "systemctl \$*" >>"$work/trap.log"; exit 1 ;;
esac
EOF
for t in quickshell hyprctl wtype; do
  printf '#!/bin/bash\necho "%s $*" >>"%s/trap.log"\nexit 1\n' "$t" "$work" >"$R/bin/$t"
done
chmod 755 "$L"/* "$R/omarchy/bin"/* "$R/bin/curl" "$R/bin/git" "$R/bin/quickshell" "$R/bin/hyprctl" \
  "$R/bin/systemctl" "$R/bin/wtype"

# A release the fake GitHub serves: install.sh installs a fake seldon of
# that version and records its arguments; SHA256SUMS covers it.
make_release() { # tag
  local d="$R/release/$1" v=${1#v}
  mkdir -p "$d/plugin"
  printf '{"id":"jax.seldon","version":"%s"}\n' "$v" >"$d/plugin/manifest.json"
  printf 'Item { /* release %s */ }\n' "$v" >"$d/plugin/Panel.qml"
  {
    echo '#!/bin/bash'
    echo "echo \"install.sh \$*\" >>\"$R/calls\""
    echo 'mkdir -p "$HOME/.local/bin"'
    echo "$work/make-seldon $v >\"\$HOME/.local/bin/seldon\""
    echo 'chmod 755 "$HOME/.local/bin/seldon"'
  } >"$d/install.sh"
  (cd "$d" && sha256sum install.sh >SHA256SUMS)
}
make_release v0.1.2

# reset_remote — the host as the operator left it: release engine 0.1.3, the
# plugin as a release git clone, unlocked.
reset_remote() {
  rm -rf "${R:?}/home" "${R:?}/calls" "${R:?}/lock.json" "${R:?}/restart_rc" "${R:?}/doctor_rc" "${R:?}/validate_rc" \
    "${R:?}/service_version" "${R:?}/restart_notice" "${R:?}/capture_rc" "${R:?}/no_session" "${R:?}/watch_active" \
    "${R:?}/watch_restart_rc"
  mkdir -p "$R/home/.local/bin" "$R/home/.config/omarchy/plugins/jax.seldon/.git"
  fake_seldon 0.1.3 >"$R/home/.local/bin/seldon"
  chmod 755 "$R/home/.local/bin/seldon"
  echo '{"id":"jax.seldon","version":"0.1.3"}' >"$R/home/.config/omarchy/plugins/jax.seldon/manifest.json"
  echo 'Item { /* release 0.1.3 */ }' >"$R/home/.config/omarchy/plugins/jax.seldon/Panel.qml"
  echo 'ref: refs/heads/main' >"$R/home/.config/omarchy/plugins/jax.seldon/.git/HEAD"
  unlock
}
unlock() { echo '{"locked":false,"sessionLocked":false,"secure":false,"requested":false}' >"$R/lock.json"; }
remote_fingerprint() { find "$R/home" -printf '%p %y %s %T@\n' | LC_ALL=C sort | sha256sum; }
# the same without the deploy log, which a failure after the build appends to
installed_fingerprint() { find "$R/home" ! -type d ! -name deploy.jsonl -printf '%p %y %s %T@\n' | LC_ALL=C sort | sha256sum; }
jqe() { jq -e "$@" >/dev/null; }

# ---- the repository -------------------------------------------------------------------
hour_ago=$(($(date +%s) - 3600))
export GIT_AUTHOR_DATE="@$hour_ago +0000" GIT_COMMITTER_DATE="@$hour_ago +0000"
export GIT_CONFIG_GLOBAL=/dev/null GIT_CONFIG_NOSYSTEM=1
git init -q -b main "$repo"
git -C "$repo" config user.email test@example.invalid
git -C "$repo" config user.name test
mkdir -p "$repo/scripts" "$repo/engine" "$repo/plugin"
cp "$root/scripts/deploy-test-host.sh" "$repo/scripts/"
printf '[package]\nname = "seldon"\nversion = "0.1.3"\nedition = "2024"\n' >"$repo/engine/Cargo.toml"
echo '{"id":"jax.seldon","version":"0.1.3"}' >"$repo/plugin/manifest.json"
echo 'Item { /* main */ }' >"$repo/plugin/Panel.qml"
echo 'Item { /* goes away */ }' >"$repo/plugin/Old.qml"
printf 'target/\n.qmlls.ini\n' >"$repo/.gitignore"
git -C "$repo" add -A
git -C "$repo" commit -q -m init
git init -q --bare "$work/origin.git"
git -C "$repo" remote add origin "$work/origin.git"
git -C "$repo" push -q origin main
# an ignored file in plugin/: the tree is clean, but it must not go out
echo '[General]' >"$repo/plugin/.qmlls.ini"
# commit <path> <text> — a commit to the repo, pushed
commit() {
  mkdir -p "$(dirname "$repo/$1")"
  echo "$2" >"$repo/$1"
  git -C "$repo" add -A
  git -C "$repo" commit -q -m "change $1"
  git -C "$repo" push -q origin main
  short=$(git -C "$repo" rev-parse --short HEAD)
}
log=$work/check.log
# fresh_log — the orchestrator's main check log for HEAD
fresh_log() { printf 'head %s\ncheck: ok\nexit 0\n' "$(git -C "$repo" rev-parse HEAD)" >"$log"; }
fresh_log

out="" rc=0
# deploy [args…] — the script in the scratch repo, stubs first on PATH.
deploy() {
  rc=0
  : >"$R/calls"
  out=$(cd "$repo" && env -u SELDON_BUILD PATH="$L:$PATH" SELDON_TEST_HOST="${TEST_HOST-$host}" \
    GUARD_HOSTS_FILE="${HOSTS_FILE:-$work/hosts}" SELDON_DEPLOY_LOCAL_ID="${LOCAL_ID:-dev-host-id}" \
    SELDON_DEPLOY_PINS="${PINS_FILE:-$work/pins}" \
    SELDON_DEPLOY_SETTLE="${SETTLE:-0}" SELDON_DEPLOY_WAIT=2 bash scripts/deploy-test-host.sh "$@" 2>&1) || rc=$?
}
has() { grep -qF -- "$1" <<<"$out"; }
called() { grep -qF -- "$1" "$R/calls" 2>/dev/null; }
count() { grep -cF -- "$1" "$R/calls" 2>/dev/null || true; }
show() { local l; while IFS= read -r l; do printf '     | %s\n' "$l"; done <<<"$out"; }
pdir=$R/home/.config/omarchy/plugins/jax.seldon
short=$(git -C "$repo" rev-parse --short HEAD)

# refused <name> <message> [args…] — exit 1, the message, and nothing ran:
# no build, nothing changed on the host.
refused() {
  local name=$1 msg=$2
  shift 2
  reset_remote
  rm -f "$work/cargo.log"
  local before
  before=$(remote_fingerprint)
  deploy "$@"
  if [[ $rc == 1 ]] && has "$msg" && [[ ! -e $work/cargo.log ]] && [[ $(remote_fingerprint) == "$before" ]]; then
    ok "refused: $name"
  else
    bad "refused: $name (rc $rc)"
    show
  fi
}

# ---- 1. refusals: the host ------------------------------------------------------------
TEST_HOST="" refused "SELDON_TEST_HOST unset" "SELDON_TEST_HOST is not set" "$log"
TEST_HOST=prod-box refused "a host not in the list" "is not listed" "$log"
TEST_HOST=other refused "a prefix of a listed host" "is not listed" "$log"
TEST_HOST=the refused "a word of a comment" "is not listed" "$log"
TEST_HOST=-oProxyCommand=x refused "an ssh option as host" "not a plain ssh alias" "$log"
HOSTS_FILE=$work/no-such-file refused "no host list" "no test host list" "$log"
LOCAL_ID=$(cat /etc/machine-id 2>/dev/null || cat /proc/sys/kernel/hostname) refused "the host is this machine" "is this machine" "$log"
# the machine-id pin (scripts/deploy-hosts.local)
printf '%s 0123456789abcdef0123456789abcdef\n' "$host" >"$work/pins-wrong"
rm -f "$work/ssh.log"
PINS_FILE=$work/pins-wrong refused "the host's machine-id does not match the pin" "does not match the pin; the alias may point elsewhere" "$log"
check "pin mismatch: one ssh call (the probe)" test "$(wc -l <"$work/ssh.log")" = 1
check "pin mismatch: neither id printed" test -z "$(grep -e "$remote_id" -e 0123456789abcdef <<<"$out")"
PINS_FILE=$work/pins-wrong refused "pin mismatch, dry run too" "does not match the pin" --dry-run "$log"
printf 'other-host 1111\n# %s %s\n' "$host" "$remote_id" >"$work/pins-none"
PINS_FILE=$work/pins-none refused "no pin for the host (a commented one does not count)" \
  "has no pinned machine-id in scripts/deploy-hosts.local; pin it once: ssh -- $host cat /etc/machine-id" "$log"
PINS_FILE=$work/no-such-pins refused "no pin file" "has no pinned machine-id" "$log"
check "no pin: the id not printed" test -z "$(grep -e "$remote_id" <<<"$out")"
reset_remote
PINS_FILE=$work/pins-none deploy --dry-run "$log"
check "dry run without a pin: exit 0" test "$rc" = 0
check "dry run without a pin: says not pinned" has ", machine-id not pinned)"
rm -f "$work/ssh.log"
TEST_HOST=prod-box deploy "$log"
check "an unlisted host is never contacted" test ! -e "$work/ssh.log"

# ---- 2. refusals: the tree and the check log ------------------------------------------
refused "no check log argument" "usage:"
refused "check log missing" "not found" "$work/missing.log"
head_line="head $(git -C "$repo" rev-parse HEAD)"
printf '%s\ncheck: ok\nexit 1\n' "$head_line" >"$log"
refused "check log says exit 1" "does not end in 'exit 0'" "$log"
printf '%s\nexit 0\nmore\nexit 2\n' "$head_line" >"$log"
refused "exit 0 not on the last line" "does not end in 'exit 0'" "$log"
printf '%s\nexit 0\n\n\n' "$head_line" >"$log"
reset_remote
deploy --dry-run "$log"
check "trailing blank lines after exit 0 are fine" test "$rc" = 0
printf '%s\nplugin-test: Quickshell harnesses skipped (nothing changed)\ncheck: ok\nexit 0\n' "$head_line" >"$log"
refused "a log that skipped the Quickshell harnesses" "skipped the Quickshell harnesses" "$log"
printf '%s\r\ncheck: ok\r\nexit 0\r\n' "$head_line" >"$log"
refused "a log with Windows line endings" "has Windows line endings (CRLF)" "$log"
# which tree the log checked: `head <full sha>` on the first line (S3)
printf 'check: ok\nexit 0\n' >"$log"
refused "check log without a head line" "does not start with 'head <full sha>'" "$log"
printf 'cargo fmt\n%s\nexit 0\n' "$head_line" >"$log"
refused "head line not first" "does not start with 'head <full sha>'" "$log"
printf 'head %s\nexit 0\n' "$(git -C "$repo" rev-parse --short HEAD)" >"$log"
refused "a short sha" "does not start with 'head <full sha>'" "$log"
printf 'head %s\nexit 0\n' "0123456789abcdef0123456789abcdef01234567" >"$log"
refused "an unknown sha" "not HEAD or an ancestor" "$log"
git -C "$repo" checkout -q -b elsewhere
echo 'Item { /* elsewhere */ }' >"$repo/plugin/Elsewhere.qml"
git -C "$repo" add -A
git -C "$repo" commit -q -m elsewhere
printf 'head %s\nexit 0\n' "$(git -C "$repo" rev-parse HEAD)" >"$log"
git -C "$repo" checkout -q main
refused "a sha on another branch (another tree's check)" "not HEAD or an ancestor" "$log"
for path in engine/lib.rs plugin/Service.qml schema/index.schema.json scripts/deploy-test-host.sh; do
  fresh_log
  if [[ $path == scripts/* ]]; then
    cp "$repo/$path" "$work/script.keep"
    echo '# a change after the check' >>"$work/script.keep"
    commit "$path" "$(cat "$work/script.keep")"
  else
    commit "$path" "changed after the check"
  fi
  refused "$path changed after the checked commit" "changed since the checked commit" "$log"
done
fresh_log
commit docs/NOTES.md 'bookkeeping after the check'
reset_remote
deploy --dry-run "$log"
check "a docs-only commit after the checked commit passes" test "$rc" = 0
[[ $rc == 0 ]] || show
fresh_log
git -C "$repo" checkout -q -b side
refused "not on main" "not on main" "$log"
git -C "$repo" checkout -q main
echo dirty >>"$repo/plugin/Panel.qml"
refused "a modified file" "not clean" "$log"
git -C "$repo" checkout -q -- plugin/Panel.qml
echo new >"$repo/plugin/Untracked.qml"
refused "an untracked file" "not clean" "$log"
rm "$repo/plugin/Untracked.qml"
echo x >"$repo/notes.txt"
git -C "$repo" add notes.txt
git -C "$repo" commit -q -m local
refused "HEAD not pushed" "is not origin/main" "$log"
git -C "$repo" push -q origin main
short=$(git -C "$repo" rev-parse --short HEAD)
refused "unknown option" "unknown option" --frobnicate "$log"
refused "release without a version" "--release needs" --release
refused "release, bad version" "wants vX.Y.Z" --release 0.1.2
refused "release with a check log" "takes no check log" --release v0.1.2 "$log"

# the host lacks a tool the install step needs (N2): refused before any change
mv "$R/bin/rsync" "$work/rsync.link"
refused "the host has no rsync" "lacks what the deploy needs: rsync" "$log"
mv "$work/rsync.link" "$R/bin/rsync"
mv "$R/omarchy/bin/omarchy" "$work/omarchy.stub"
refused "the host has no omarchy" "lacks what the deploy needs: omarchy" "$log"
mv "$R/bin/curl" "$work/curl.stub"
refused "release: the host has neither curl nor omarchy" "lacks what the deploy needs: curl omarchy" --release v0.1.2
mv "$work/curl.stub" "$R/bin/curl"
mv "$work/omarchy.stub" "$R/omarchy/bin/omarchy"

# ---- 3. dry run ----------------------------------------------------------------------
reset_remote
before=$(remote_fingerprint)
rm -f "$work/cargo.log"
deploy --dry-run "$log"
[[ -z ${VERBOSE:-} ]] || show
check "dry run: exit 0" test "$rc" = 0
check "dry run: no build" test ! -e "$work/cargo.log"
check "dry run: the host unchanged" test "$(remote_fingerprint)" = "$before"
check "dry run: no restart" test "$(count restart)" = 0
check "dry run: names the version" has "0.1.3+main.$short"
check "dry run: the hostname ssh resolves the alias to" has "(ssh resolves it to 192.0.2.7, machine-id pinned)"
check "dry run: what the host runs now" has "now      engine 0.1.3, plugin dir git"
check "dry run: the clone would move aside" has "move the git dir aside"
check "dry run: files change" has "files change: yes"
check "dry run: restart planned" has "restart  restart the shell"
[[ $rc == 0 ]] || show

reset_remote
rm "$R/home/.local/bin/seldon"
deploy --dry-run "$log"
check "dry run, no engine on the host: said so" has "now      engine not installed or not answering, plugin dir git"
deploy "$log"
check "first deploy without an engine: exit 0" test "$rc" = 0
check "first deploy without an engine: the summary says there was none" has "(there was none before)"
check "first deploy without an engine: no seldon.prev" test ! -e "$R/home/.local/bin/seldon.prev"

# ---- 4. first deploy: the release clone moves aside --------------------------------------
reset_remote
rm -f "$work/cargo.log"
deploy "$log"
[[ -z ${VERBOSE:-} ]] || show
check "deploy: exit 0" test "$rc" = 0
[[ $rc == 0 ]] || show
check "deploy: cargo built with SELDON_BUILD=main.<sha>" grep -q " SELDON_BUILD=main.$short\$" "$work/cargo.log"
check "deploy: built as a release (--features watch)" grep -q -- "--release --features watch --target x86_64-unknown-linux-musl " "$work/cargo.log"
check "deploy: into the repo's target dir" grep -q -- "--target-dir $repo/engine/target " "$work/cargo.log"
check "deploy: the host's seldon is the marked build" \
  grep -q "\"version\":\"0.1.3+main.$short\"" "$R/home/.local/bin/seldon"
check "deploy: the previous seldon kept as seldon.prev" grep -q '"version":"0.1.3"}' "$R/home/.local/bin/seldon.prev"
backup=$(find "$R/home/.local/state/seldon-dev" -maxdepth 1 -name 'plugin-git-*' | head -1)
check "deploy: the release clone moved to seldon-dev/plugin-git-<stamp>" test -d "$backup/.git"
check "deploy: the backup is outside the plugins dir" test "$(find "$R/home/.config/omarchy/plugins" -mindepth 1 -maxdepth 1 | wc -l)" = 1
check "deploy: the plugin dir is a plain copy" test ! -e "$pdir/.git"
check "deploy: the plugin dir has HEAD's files" diff -r -x .seldon-dev-build -x .qmlls.ini "$repo/plugin" "$pdir"
check "deploy: an ignored file in plugin/ does not go out" test ! -e "$pdir/.qmlls.ini"
check "deploy: .seldon-dev-build names the build" grep -qx "build=main.$short" "$pdir/.seldon-dev-build"
check "deploy: .seldon-dev-build names the commit" grep -qx "commit=$(git -C "$repo" rev-parse HEAD)" "$pdir/.seldon-dev-build"
check "deploy: plugin validated on the host" called "omarchy plugin validate .config/omarchy/plugins/jax.seldon"
check "deploy: the lock was checked" called "omarchy-shell lock status"
check "deploy: the shell restarted once" test "$(count restart)" = 1
first_ping=$(grep -n -m1 -x "omarchy-shell shell ping" "$R/calls" | cut -d: -f1)
check "deploy: the shell answered ping before the restart" test "${first_ping:-999}" -lt "$(grep -n -m1 -x restart "$R/calls" | cut -d: -f1)"
check "deploy: no restart pending" test ! -e "$R/home/.local/state/seldon-dev/restart-pending"
check "deploy: smoke ok" has "smoke    ok"
check "deploy: smoke checked the service" called "omarchy-shell jax.seldon.service status"
check "deploy: doctor's degraded rows are named" has "note doctor: snapper degraded"
jsonl=$R/home/.local/state/seldon-dev/deploy.jsonl
check "deploy: one log line" test "$(wc -l <"$jsonl")" = 1
check "deploy: the log line" jqe --arg v "0.1.3+main.$short" \
  '.mode == "main" and .version == $v and .pluginChanged and .restart == "done" and .smoke == "ok" and (.movedAside | test("plugin-git-")) and .failures == []' "$jsonl"
check "deploy: summary" has "runs 0.1.3+main.$short"
check "deploy: an inactive watcher is left alone" has "seldon-watch.service not active; left as it is"
check "deploy: no watcher restart" test "$(count "watch restart")" = 0

# ---- 5. engine-only change: no restart; the dev copy stays in place ---------------------
panel_mtime=$(stat -c %Y "$pdir/Panel.qml")
# a later commit time: git archive stamps every file with it (S1)
GIT_COMMITTER_DATE="@$((hour_ago + 600)) +0000" commit engine/src.rs 'fn main() {}'
fresh_log
# an exported CARGO_TARGET_DIR must not change where the binary is taken from (S2)
CARGO_TARGET_DIR=$work/other-target deploy "$log"
check "engine change: exit 0" test "$rc" = 0
check "engine change: an unchanged plugin file keeps its mtime" test "$(stat -c %Y "$pdir/Panel.qml")" = "$panel_mtime"
check "engine change: CARGO_TARGET_DIR set, the new build ships" grep -q "\"version\":\"0.1.3+main.$short\"" "$R/home/.local/bin/seldon"
[[ $rc == 0 ]] || show
check "engine change: plugin unchanged" has "files changed: no"
check "engine change: no restart" test "$(count restart)" = 0
check "engine change: seldon.prev is the previous main build" grep -q '"version":"0.1.3+main.' "$R/home/.local/bin/seldon.prev"
check "engine change: no second backup" test "$(find "$R/home/.local/state/seldon-dev" -maxdepth 1 -name 'plugin-*' | wc -l)" = 1
check "engine change: the marker follows the commit" grep -qx "build=main.$short" "$pdir/.seldon-dev-build"
check "engine change: two log lines" test "$(wc -l <"$jsonl")" = 2
check "engine change: logged as restart none" jqe -s '.[-1].restart == "none" and (.[-1].pluginChanged | not)' "$jsonl"

git -C "$repo" rm -q plugin/Old.qml
commit plugin/New.qml 'Item { /* new */ }'
fresh_log
started=$SECONDS
SETTLE=2 deploy "$log"
check "the restart waits SELDON_DEPLOY_SETTLE after ping" test $((SECONDS - started)) -ge 2
check "a file removed from plugin/ is removed on the host" test ! -e "$pdir/Old.qml"
check "a file added to plugin/ arrives" test -f "$pdir/New.qml"
check "the restart for it ran" test "$(count restart)" = 1

# ---- 6. plugin change on a locked session: restart pending, then caught up --------------
commit plugin/Panel.qml 'Item { /* main 2 */ }'
fresh_log
for lock in '"locked":true,"sessionLocked":true,"secure":true' '"locked":false,"sessionLocked":false,"secure":true' \
  '"locked":true,"sessionLocked":false,"secure":false' '"locked":false,"sessionLocked":true,"secure":false'; do
  echo "{$lock}" >"$R/lock.json"
  : >"$R/calls"
  deploy "$log"
  [[ -z ${VERBOSE:-} ]] || show
  check "locked ($lock): exit 0" test "$rc" = 0
  check "locked ($lock): no restart" test "$(count restart)" = 0
  check "locked ($lock): restart pending" has "restart pending"
  check "locked ($lock): pending flag kept" test -e "$R/home/.local/state/seldon-dev/restart-pending"
done
echo "Restart the shell" >"$R/restart_notice"
deploy "$log"
check "restart notice while the restart is pending: exit 0" test "$rc" = 0
check "restart notice while pending: a note" has "note restart notice while the restart is pending: Restart the shell"
check "restart notice while pending: still restart pending" has "restart  restart pending"
rm "$R/restart_notice"
rm "$R/lock.json"
: >"$R/calls"
deploy "$log"
check "lock status unreadable: no restart" test "$(count restart)" = 0
check "lock status unreadable: restart pending" has "restart pending"
unlock
: >"$R/calls"
deploy "$log"
check "unlocked again, no new plugin change: the pending restart runs" test "$(count restart)" = 1
check "unlocked again: flag cleared" test ! -e "$R/home/.local/state/seldon-dev/restart-pending"
: >"$R/calls"
deploy "$log"
check "and the next deploy has nothing to restart" test "$(count restart)" = 0

# ---- 7. failures ----------------------------------------------------------------------------
commit plugin/Panel.qml 'Item { /* main 3 */ }'
fresh_log
echo 1 >"$R/restart_rc"
deploy "$log"
check "restart fails: exit 0, restart pending" test "$rc" = 0
check "restart fails: says so" has "pending (omarchy-restart-shell failed)"
check "restart fails: flag kept" test -e "$R/home/.local/state/seldon-dev/restart-pending"
rm "$R/restart_rc"

echo 1 >"$R/doctor_rc"
deploy "$log"
check "doctor error: exit 2" test "$rc" = 2
check "doctor error: named" has "FAIL seldon doctor --json exit: 1 (want 0)"
check "doctor error: logged as failed" jqe -s '.[-1].smoke == "failed" and (.[-1].failures | length) == 1' "$jsonl"
rm "$R/doctor_rc"

echo 1 >"$R/capture_rc"
deploy "$log"
check "capture error: exit 2" test "$rc" = 2
check "capture error: named" has "FAIL seldon capture --json exit: 1 (want 0)"
rm "$R/capture_rc"

touch "$R/no_session"
deploy "$log"
check "no graphical session: exit 2" test "$rc" = 2
check "no graphical session: the smoke names it" has "FAIL service: no graphical session"
check "no graphical session: the summary names it" has "smoke    failed: no graphical session on $host"
check "no graphical session: the engine checks still ran" has "ok   seldon capture --json exit"
rm "$R/no_session"

echo 0.1.2 >"$R/service_version"
deploy "$log"
check "service runs another engine version: exit 2" test "$rc" = 2
check "service runs another engine version: named" has "FAIL service engineVersion: 0.1.2"
rm "$R/service_version"

echo "Restart the shell" >"$R/restart_notice"
deploy "$log"
check "restart notice: exit 2" test "$rc" = 2
check "restart notice: named" has "FAIL service restart notice"
rm "$R/restart_notice"

deploy "$log"
check "settled: nothing pending" test ! -e "$R/home/.local/state/seldon-dev/restart-pending"
commit plugin/Panel.qml 'Item { /* main 4 */ }'
fresh_log
echo 1 >"$R/validate_rc"
deploy "$log"
check "plugin validation fails on the host: exit 2" test "$rc" = 2
check "plugin validation fails: named" has "install: omarchy plugin validate"
check "plugin validation fails: no restart" test "$(count restart)" = 0
check "plugin validation fails: logged" jqe -s '.[-1].failures == ["install: omarchy plugin validate"]' "$jsonl"
rm "$R/validate_rc"
deploy "$log"
check "validates again, no new plugin change: the restart runs" test "$(count restart)" = 1

touch "$work/cargo_fail"
before=$(remote_fingerprint)
deploy "$log"
check "build fails: exit 2" test "$rc" = 2
check "build fails: the host unchanged" test "$(remote_fingerprint)" = "$before"
rm "$work/cargo_fail"
echo 0.1.3 >"$work/cargo_version"
deploy "$log"
check "the build lacks the marker: exit 2" test "$rc" = 2
check "the build lacks the marker: named" has "reports '0.1.3', not 0.1.3+main.$short"
check "the build lacks the marker: the host unchanged" test "$(remote_fingerprint)" = "$before"
rm "$work/cargo_version"

reset_remote
rm -rf "$pdir"
ln -s "$R/elsewhere" "$pdir"
before=$(remote_fingerprint)
deploy "$log"
check "plugin dir is a symlink: refused" test "$rc" = 1
check "plugin dir is a symlink: the host unchanged" test "$(remote_fingerprint)" = "$before"

# a plain copy that is not a dev copy (e.g. an rsync by hand) moves aside too
reset_remote
rm -rf "$pdir/.git"
deploy "$log"
check "plain copy: moved to plugin-copy-<stamp>" test -n "$(find "$R/home/.local/state/seldon-dev" -maxdepth 1 -name 'plugin-copy-*')"

# the watcher: an active seldon-watch.service is restarted on the new binary
reset_remote
touch "$R/watch_active"
deploy "$log"
check "watcher active: exit 0" test "$rc" = 0
check "watcher active: restarted once, on the new binary" test "$(grep -c "^watch restart on seldon 0.1.3+main.$short\$" "$R/calls")" = 1
check "watcher active: the summary says so" has "seldon-watch.service restarted on the new binary"
check "watcher active: logged" jqe -s '.[-1].watch == "restarted"' "$jsonl"
echo 1 >"$R/watch_restart_rc"
deploy "$log"
check "watcher restart fails: exit 2" test "$rc" = 2
check "watcher restart fails: named" has "seldon-watch.service: restart failed"
check "watcher restart fails: logged" jqe -s '.[-1].watch == "restart failed" and .[-1].smoke == "failed"' "$jsonl"
rm "$R/watch_restart_rc"

# ---- 8. back to a release -----------------------------------------------------------------
reset_remote
deploy "$log" >/dev/null
: >"$R/calls"
deploy --release v0.1.2 --dry-run
check "release dry run: exit 0" test "$rc" = 0
check "release dry run: warns about newer state" has "move ~/.local/state/seldon aside"
check "release dry run: nothing installed" test "$(count "install.sh --")" = 0
deploy --release v0.1.2
check "release: exit 0" test "$rc" = 0
[[ $rc == 0 ]] || show
check "release: warns about newer state" has "state written by a newer build may not load"
check "release: install.sh --version v0.1.2 --force" called "install.sh --version v0.1.2 --force"
check "release: engine 0.1.2" grep -q '"version":"0.1.2"' "$R/home/.local/bin/seldon"
check "release: plugin cloned at the tag" called "git clone --quiet --branch v0.1.2 -- https://github.com/JohnAndrewsX/jax-seldon-plugin.git"
check "release: the plugin dir is the clone" test -d "$pdir/.git"
check "release: no dev marker" test ! -e "$pdir/.seldon-dev-build"
check "release: the plugin has the release's files" grep -q 'release 0.1.2' "$pdir/Panel.qml"
check "release: the dev copy moved aside" test -n "$(find "$R/home/.local/state/seldon-dev" -maxdepth 1 -name 'plugin-dev-*')"
check "release: restarted" test "$(count restart)" = 1
check "release: smoke expects 0.1.2" has "ok   service engineVersion"
check "release: logged" jqe -s '.[-1].mode == "release" and .[-1].version == "0.1.2" and .[-1].smoke == "ok"' "$jsonl"
reset_remote
deploy "$log" >/dev/null
touch "$R/watch_active"
deploy --release v0.1.2
check "release, watcher active: restarted on the release binary" grep -qx "watch restart on seldon 0.1.2" "$R/calls"
rm "$R/watch_active"

reset_remote
deploy "$log" >/dev/null
echo 1 >"$R/validate_rc"
deploy --release v0.1.2
check "release, the clone fails validation: exit 2" test "$rc" = 2
check "release, the clone fails validation: named" has "install: omarchy plugin validate"
check "release, the clone fails validation: install.sh not run" test "$(count "install.sh --")" = 0
check "release, the clone fails validation: the dev copy stays" test -f "$pdir/.seldon-dev-build"
rm "$R/validate_rc"

reset_remote
echo "0000  install.sh" >"$R/release/v0.1.2/SHA256SUMS"
before=$(installed_fingerprint)
deploy --release v0.1.2
check "release, checksum mismatch: exit 2" test "$rc" = 2
check "release, checksum mismatch: named" has "does not match the release's SHA256SUMS"
check "release, checksum mismatch: install.sh not run" test "$(count "install.sh --")" = 0
check "release, checksum mismatch: engine and plugin unchanged" test "$(installed_fingerprint)" = "$before"
check "release, checksum mismatch: logged" jqe -s '.[-1].failures == ["install: install.sh does not match the release'"'"'s SHA256SUMS"]' "$jsonl"
(cd "$R/release/v0.1.2" && sha256sum install.sh >SHA256SUMS)
reset_remote
deploy --release v0.9.9
check "release that does not exist: exit 2" test "$rc" = 2
check "release that does not exist: engine unchanged" grep -q '"version":"0.1.3"}' "$R/home/.local/bin/seldon"

# ---- 9. never the real session; the real home untouched -------------------------------------
check "no quickshell, hyprctl or wtype call, no other systemctl call" test ! -e "$work/trap.log"
[[ ! -e $work/trap.log ]] || sed 's/^/     /' "$work/trap.log"
check "every ssh call went to the test host" test "$(sort -u "$work/ssh.log")" = "$host"
check "real ~/.local/bin/seldon, plugin dir and seldon-dev untouched" test "$(real_fingerprint)" = "$real_before"

if command -v shellcheck >/dev/null; then
  check "shellcheck deploy-test-host.sh" shellcheck "$root/scripts/deploy-test-host.sh"
  check "shellcheck deploy-test-host.test.sh" shellcheck "$0"
else
  echo "note: shellcheck not installed; bash -n only"
  check "bash -n deploy-test-host.sh" bash -n "$root/scripts/deploy-test-host.sh"
fi

echo "deploy-test-host.test: $pass passed, $fail failed"
[[ $fail -eq 0 ]]
