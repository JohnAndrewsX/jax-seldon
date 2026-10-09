#!/usr/bin/env bash
# scripts/deploy-test-host.sh against a fake test host (WP-098, WP-155).
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
# The temp dir is on disk, under the checkout's ignored target/, never
# /tmp (RAM). On the fake host, cp and du refuse any path outside it: a
# mutant without the logbook guards once copied / into /tmp (WP-155).
#
# shellcheck disable=SC2016 # stub and fake-release text is single-quoted on purpose
set -euo pipefail

root=$(cd "$(dirname "$0")/../.." && pwd)
mkdir -p "$root/target"
work=$(mktemp -d "$root/target/deploy-test.XXXXXX")
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
mkdir -p "$L" "$R/omarchy/bin" "$R/bin" "$R/run" "$work/tmp"
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
  "doctor --json") jq -cn --arg l "\\\$(cat "$R/logbook_path" 2>/dev/null || echo "\\\$HOME/Seldon")" '{ok: true, logbook: \\\$l, checks: [{name: "snapper", status: "degraded"}]}'; exit \\\$(cat "$R/doctor_rc" 2>/dev/null || echo 0) ;;
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
# a hook between the probe (first call) and the install (second call)
n=\$(wc -l <"$work/ssh.log")
[[ ! -x "$R/before_ssh_\$n" ]] || "$R/before_ssh_\$n"
exec env -i HOME="$R/home" PATH="$R/bin" OMARCHY_PATH="$R/omarchy" XDG_RUNTIME_DIR="$R/run" LC_ALL=C bash -c "\$*"
EOF

# ---- remote stubs -----------------------------------------------------------------
# plain tools the remote scripts use, nothing else
for t in bash sh cat chmod mkdir touch date sed awk grep find sort xargs sha256sum \
  cut mktemp jq base64 id ls seq sleep dirname stat env tr head tail wc realpath; do
  p=$(command -v "$t") || { echo "deploy-test-host.test: needs $t on PATH" >&2; exit 1; }
  ln -s "$p" "$R/bin/$t"
done
# cp, du, mv, rm, rsync, install and tar only inside the test dir: every
# argument that is not an option must resolve below $work, else the call
# fails and lands in trap.log (a mutant run stays harmless). Switches, which
# work as root too (CI): $R/cp_fail names a substring; a cp whose source
# contains it fails. $R/du_empty: du prints nothing.
for t in cp du mv rm rsync install tar; do
  p=$(command -v "$t") || { echo "deploy-test-host.test: needs $t on PATH" >&2; exit 1; }
  cat >"$R/bin/$t" <<EOF
#!/bin/bash
for a; do
  [[ \$a == -* ]] && continue
  r=\$(realpath -m -- "\$a")
  [[ \$r == "$work"/* ]] || { echo "$t outside the test dir: \$a" >>"$work/trap.log"; exit 1; }
done
if [[ $t == cp && -f "$R/cp_fail" ]]; then
  f=\$(cat "$R/cp_fail") n=0
  for a; do [[ \$a == -* ]] || n=\$((n + 1)); done
  i=0
  for a; do
    [[ \$a == -* ]] && continue
    i=\$((i + 1))
    ((i < n)) && [[ \$a == *"\$f"* ]] && { echo "cp \$a: failed (cp_fail)" >&2; exit 1; }
  done
fi
[[ $t != du || ! -f "$R/du_empty" ]] || exit 0
exec "$p" "\$@"
EOF
  chmod 755 "$R/bin/$t"
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
# systemctl: only `--user is-active --quiet`, `restart`, `stop` and `start`
# of the watcher unit; restart and start record the engine version they
# would start, stop and start how many next backups exist then
cat >"$R/bin/systemctl" <<EOF
#!/bin/bash
backups() { ls "\$HOME/.local/state/seldon-dev" 2>/dev/null | grep -c backup-before-next-; }
case "\$*" in
  "--user is-active --quiet seldon-watch.service") [[ -f "$R/watch_active" ]] ;;
  "--user restart seldon-watch.service")
    echo "watch restart on \$(seldon --version)" >>"$R/calls"
    exit \$(cat "$R/watch_restart_rc" 2>/dev/null || echo 0) ;;
  "--user stop seldon-watch.service")
    echo "watch stop, backups \$(backups)" >>"$R/calls"
    [[ ! -f "$R/watch_stop_rc" ]] || exit 1
    rm -f "$R/watch_active" ;;
  "--user start seldon-watch.service")
    echo "watch start, backups \$(backups), on \$(seldon --version)" >>"$R/calls"
    [[ ! -f "$R/watch_start_rc" ]] || exit 1
    touch "$R/watch_active" ;;
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
    "${R:?}/watch_restart_rc" "${R:?}/watch_stop_rc" "${R:?}/watch_start_rc" "${R:?}/logbook_path" \
    "${R:?}/cp_fail" "${R:?}/du_empty"
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
# commit <path> <text> — a commit to the repo's branch, pushed
commit() {
  mkdir -p "$(dirname "$repo/$1")"
  echo "$2" >"$repo/$1"
  git -C "$repo" add -A
  git -C "$repo" commit -q -m "change $1"
  git -C "$repo" push -q origin HEAD
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
    SELDON_DEPLOY_SETTLE="${SETTLE:-0}" SELDON_DEPLOY_WAIT=2 SELDON_DEPLOY_BACKUP_MAX_MB="${BACKUP_MAX_MB:-}" \
    TMPDIR="$work/tmp" bash scripts/deploy-test-host.sh "$@" 2>&1) || rc=$?
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

# ---- 9. next (WP-155) -------------------------------------------------------------------------
# on <branch> — check out a branch of the scratch repo, its check log fresh
on() {
  git -C "$repo" checkout -q "$1"
  short=$(git -C "$repo" rev-parse --short HEAD)
  fresh_log
}
on main
deploy "$log"
check "main onto a main host: no next warning" test -z "$(grep -F "runs a next build" <<<"$out")"
refused "--branch next while on main" "not on next (on main)" --branch next "$log"
refused "--branch without a value" "--branch needs main or next" "$log" --branch
refused "--branch, another value" "--branch wants main or next, not 'beta'" --branch beta "$log"
refused "--branch with --release" "--release takes no --branch" --release v0.1.2 --branch next
git -C "$repo" checkout -q -b next
fresh_log
refused "next not pushed yet" "no origin/next" --branch next "$log"
commit plugin/Next.qml 'Item { /* next */ }'
fresh_log
refused "--branch main while on next" "not on main (on next)" --branch main "$log"
refused "next without --branch (main is the default)" "not on main (on next)" "$log"
echo dirty >>"$repo/plugin/Next.qml"
refused "next: a modified file" "not clean" --branch next "$log"
git -C "$repo" checkout -q -- plugin/Next.qml
echo x >"$repo/next-notes.txt"
git -C "$repo" add next-notes.txt
git -C "$repo" commit -q -m local
refused "next: HEAD not pushed" "is not origin/next" --branch next "$log"
git -C "$repo" push -q origin next
fresh_log
commit plugin/Next.qml 'Item { /* next, after the check */ }'
refused "next: plugin/ changed after the checked commit" "changed since the checked commit" --branch next "$log"
# a check log of main's tip, which next does not contain
on main
commit docs/MAIN.md 'main only'
fresh_log
git -C "$repo" checkout -q next
refused "next: main's check log (not an ancestor)" "not HEAD or an ancestor" --branch next "$log"
on next
printf 'head %s\nplugin-test: Quickshell harnesses skipped (nothing changed)\nexit 0\n' "$(git -C "$repo" rev-parse HEAD)" >"$log"
refused "next: a log that skipped the harnesses" "run SELDON_FULL_CHECK=1 just check on next" --branch next "$log"

# the host runs main, with Omarchy's shell.json, Seldon's config and state
# dirs and a logbook at ~/Seldon (where the fake engine's doctor says)
reset_remote
on main
main_short=$short
deploy "$log" >/dev/null
mkdir -p "$R/home/.local/state/seldon" "$R/home/.config/seldon" "$R/home/Seldon/ledger"
echo '{"version":"0.2"}' >"$R/home/.config/omarchy/shell.json"
echo '{"contractVersion":3}' >"$R/home/.local/state/seldon/index.json"
echo 'logbook = "~/Seldon"' >"$R/home/.config/seldon/config.toml"
echo '{"kind":"note"}' >"$R/home/Seldon/ledger/2026-10.jsonl"
on next
before=$(remote_fingerprint)
rm -f "$work/cargo.log"
deploy --dry-run --branch next "$log"
[[ -z ${VERBOSE:-} ]] || show
check "next dry run: exit 0" test "$rc" = 0
check "next dry run: no build, the host unchanged" test ! -e "$work/cargo.log" -a "$(remote_fingerprint)" = "$before"
check "next dry run: the build line" has "SELDON_BUILD=next.$short cargo build"
check "next dry run: names the version" has "→ 0.1.3+next.$short"
check "next dry run: a backup planned" has "backup   shell.json, ~/.config/seldon, ~/.local/state/seldon, the logbook ($R/home/Seldon), the engine and"
check "next dry run: where the backup goes" grep -qE "the plugin dir to ~/.local/state/seldon-dev/backup-before-next-[0-9]{8}T[0-9]{6}Z/, with RESTORE.txt;" <<<"$out"
check "next dry run: the dev copy is synced, not moved" test -z "$(grep -F "aside" <<<"$out")"

deploy --branch next "$log"
[[ -z ${VERBOSE:-} ]] || show
[[ $rc == 0 ]] || show
check "next: exit 0" test "$rc" = 0
check "next: cargo built with SELDON_BUILD=next.<sha>" grep -q " SELDON_BUILD=next.$short\$" "$work/cargo.log"
check "next: the host's seldon is the next build" grep -q "\"version\":\"0.1.3+next.$short\"" "$R/home/.local/bin/seldon"
check "next: seldon.prev is the main build" grep -q "\"version\":\"0.1.3+main.$main_short\"" "$R/home/.local/bin/seldon.prev"
check "next: the plugin dir has next's files" diff -r -x .seldon-dev-build -x .qmlls.ini "$repo/plugin" "$pdir"
check "next: .seldon-dev-build names the next build" grep -qx "build=next.$short" "$pdir/.seldon-dev-build"
bk=$(find "$R/home/.local/state/seldon-dev" -maxdepth 1 -name 'backup-before-next-*')
check "next: one backup dir, named backup-before-next-<UTC stamp>" \
  grep -qxE '.*/backup-before-next-[0-9]{8}T[0-9]{6}Z' <<<"$bk"
check "next: the backup has shell.json" cmp -s "$bk/shell.json" "$R/home/.config/omarchy/shell.json"
check "next: the backup has the state dir" cmp -s "$bk/state-seldon/index.json" "$R/home/.local/state/seldon/index.json"
check "next: the backup has ~/.config/seldon" cmp -s "$bk/config-seldon/config.toml" "$R/home/.config/seldon/config.toml"
check "next: the backup has the logbook" cmp -s "$bk/logbook/ledger/2026-10.jsonl" "$R/home/Seldon/ledger/2026-10.jsonl"
t='~'
b=$t/${bk#"$R/home/"}
check "next: RESTORE.txt puts back the state dir" grep -qxF "  mv ~/.local/state/seldon ~/.local/state/seldon.next && cp -a $b/state-seldon ~/.local/state/seldon" "$bk/RESTORE.txt"
check "next: RESTORE.txt puts back ~/.config/seldon" grep -qxF "  mv ~/.config/seldon ~/.config/seldon.next && cp -a $b/config-seldon ~/.config/seldon" "$bk/RESTORE.txt"
check "next: RESTORE.txt puts back the logbook" grep -qxF "  mv $R/home/Seldon $R/home/Seldon.next && cp -a $b/logbook $R/home/Seldon" "$bk/RESTORE.txt"
check "next: RESTORE.txt puts back shell.json" grep -qxF "  mv ~/.config/omarchy/shell.json ~/.config/omarchy/shell.json.next && cp -pP $b/shell.json ~/.config/omarchy/shell.json" "$bk/RESTORE.txt"
check "next: RESTORE.txt puts back the engine" grep -qxF "  mv ~/.local/bin/seldon ~/.local/bin/seldon.next && cp -pP $b/seldon.engine ~/.local/bin/seldon" "$bk/RESTORE.txt"
# the order: stop the watcher, engine, logbook, config, state, shell.json
# back first, then deploy main, then start the watcher
line_of() { grep -nF -m1 -- "$1" "$bk/RESTORE.txt" | cut -d: -f1; }
order=""
for k in "systemctl --user stop seldon-watch.service" "$b/seldon.engine" "$b/logbook" "$b/config-seldon" \
  "$b/state-seldon" "$b/shell.json" "then, right away, from the dev host: deploy main (or --release vX.Y.Z)" \
  "systemctl --user start seldon-watch.service"; do
  order+="$(line_of "$k") "
done
check "next: RESTORE.txt order: stop, engine, logbook, config, state, shell.json, deploy main, start" \
  test "$order" = "$(tr ' ' '\n' <<<"$order" | grep . | sort -n | tr '\n' ' ')" -a "$(wc -w <<<"$order")" = 8
check "next: RESTORE.txt says first" grep -qF "To go back from next, first put back on $host what next writes" "$bk/RESTORE.txt"
check "next: the summary prints RESTORE.txt" has "             mv $R/home/Seldon $R/home/Seldon.next && cp -a $b/logbook $R/home/Seldon"
check "next: an inactive watcher is neither stopped nor started" test "$(count "watch st")" = 0
check "next: the backup has the engine before the swap (main)" grep -q "\"version\":\"0.1.3+main.$main_short\"" "$bk/seldon.engine"
check "next: the backup has the plugin before the sync (main)" grep -qx "build=main.$main_short" "$bk/plugin-jax.seldon/.seldon-dev-build"
check "next: the backup is outside the plugins dir" test "$(find "$R/home/.config/omarchy/plugins" -mindepth 1 -maxdepth 1 | wc -l)" = 1
check "next: the summary names the backup" has "backup   ~/.local/state/seldon-dev/backup-before-next-"
check "next: smoke ok" has "smoke    ok"
check "next: the log line" jqe -s --arg v "0.1.3+next.$short" \
  '.[-1].mode == "next" and .[-1].version == $v and (.[-1].backup | test("^.local/state/seldon-dev/backup-before-next-")) and .[-1].smoke == "ok"' "$jsonl"

# next onto next: no second backup
commit plugin/Next.qml 'Item { /* next 2 */ }'
fresh_log
deploy --dry-run --branch next "$log"
check "next onto next, dry run: no backup" has "backup   none: the host runs next already (plugin next."
deploy --branch next "$log"
check "next onto next: exit 0" test "$rc" = 0
check "next onto next: still one backup" test "$(find "$R/home/.local/state/seldon-dev" -maxdepth 1 -name 'backup-before-next-*' | wc -l)" = 1
check "next onto next: logged without a backup" jqe -s '.[-1].mode == "next" and .[-1].backup == ""' "$jsonl"
check "next onto next: no backup line" test -z "$(grep -F "backup   ~" <<<"$out")"

# a release onto next: the same pointer (dry run, the host stays on next)
deploy --release v0.1.2 --dry-run
check "release onto next: warns about next's state" has "runs a next build (plugin next."
check "release onto next: names RESTORE.txt" has "v0.1.2 cannot read next's state and ledger lines: before this deploy put back what RESTORE.txt in the newest ~/.local/state/seldon-dev/backup-before-next-*"

# back to main from next: a warning that names the backup
on main
deploy --branch main "$log"
check "main onto next: exit 0" test "$rc" = 0
check "main onto next: warns about next's state" has "runs a next build (plugin next."
check "main onto next: names RESTORE.txt" has "main cannot read next's state and ledger lines: before this deploy put back what RESTORE.txt in the newest ~/.local/state/seldon-dev/backup-before-next-*"
check "main onto next: the main build" grep -q "\"version\":\"0.1.3+main.$short\"" "$R/home/.local/bin/seldon"

# from a release host: the backup holds what exists (no shell.json, no state)
reset_remote
on next
deploy --dry-run --branch next "$log"
check "next onto a release, dry run: the logbook named, absent" has "the logbook ($R/home/Seldon, absent)"
deploy --branch next "$log"
check "next onto a release: exit 0" test "$rc" = 0
bk=$(find "$R/home/.local/state/seldon-dev" -maxdepth 1 -name 'backup-before-next-*')
check "next onto a release: the release engine and clone backed up" \
  test -d "$bk/plugin-jax.seldon/.git" -a "$(grep -c '"version":"0.1.3"}' "$bk/seldon.engine")" = 1
check "next onto a release: no shell.json, config, state or logbook in the backup" \
  test ! -e "$bk/shell.json" -a ! -e "$bk/state-seldon" -a ! -e "$bk/config-seldon" -a ! -e "$bk/logbook"
check "next onto a release: RESTORE.txt only for what was there (the engine)" \
  test "$(grep -c '^  mv ' "$bk/RESTORE.txt")" = 1 -a "$(grep -c '^  mv ~/.local/bin/seldon ' "$bk/RESTORE.txt")" = 1

# the host without ~/.local/state/seldon-dev, where the backup and the log go
host_fingerprint() { find "$R/home" ! -type d ! -path '*/seldon-dev/*' -printf '%p %y %s %T@\n' | LC_ALL=C sort | sha256sum; }

# an active watcher: stopped for the copy, started again before the swap,
# restarted on the new binary after it
reset_remote
on main
deploy "$log" >/dev/null
main_short=$short
touch "$R/watch_active"
on next
deploy --branch next "$log"
check "watcher active: exit 0" test "$rc" = 0
check "watcher active: stopped before the copy, started after it, on the old engine, then restarted on next" \
  test "$(grep '^watch ' "$R/calls")" = "watch stop, backups 1
watch start, backups 1, on seldon 0.1.3+main.$main_short
watch restart on seldon 0.1.3+next.$short"
bk=$(find "$R/home/.local/state/seldon-dev" -maxdepth 1 -name 'backup-before-next-*')
check "watcher active: the backup is complete" test -f "$bk/RESTORE.txt" -a -f "$bk/seldon.engine"
check "watcher active: running at the end" test -f "$R/watch_active"
reset_remote
touch "$R/watch_active" "$R/watch_stop_rc"
before=$(installed_fingerprint)
deploy --branch next "$log"
check "watcher does not stop: exit 2, named" has "failed: install: backup: cannot stop seldon-watch.service"
check "watcher does not stop: exit 2" test "$rc" = 2
check "watcher does not stop: nothing copied, engine and plugin unchanged" \
  test "$(installed_fingerprint)" = "$before" -a -z "$(find "$R/home/.local/state/seldon-dev" -mindepth 2 -path '*backup-before-next-*')"
reset_remote
touch "$R/watch_active" "$R/watch_start_rc"
before=$(host_fingerprint)
deploy --branch next "$log"
check "watcher does not start again: exit 2" test "$rc" = 2
check "watcher does not start again: named" has "failed: install: backup: seldon-watch.service did not start again"
check "watcher does not start again: engine and plugin unchanged" \
  test "$(host_fingerprint)" = "$before"

# a logbook that holds seldon-dev (the home, /): the backup would copy itself
# a logbook that is not a directory strictly below the home, or that holds
# seldon-dev: refused from the probe, before any build, copy or size check
# (cp and du above refuse every path outside the test dir anyway)
# (home-sibling: the home's path is its prefix, it is not below the home)
mkdir -p "$work/outside-logbook" "$R/home-sibling"
for lb in / "$R" "$R/home" "$R/home/.local" "$work/outside-logbook" "$R/home/link-out" "$R/home-sibling"; do
  reset_remote
  ln -s "$work/outside-logbook" "$R/home/link-out"
  echo "$lb" >"$R/logbook_path"
  before=$(remote_fingerprint)
  rm -f "$work/cargo.log"
  deploy --branch next "$log"
  check "logbook ${lb#"$work"/}: refused" test "$rc" = 1
  check "logbook ${lb#"$work"/}: named" has "is not a directory below the home, or holds ~/.local/state/seldon-dev; not copying it"
  check "logbook ${lb#"$work"/}: no build, the host unchanged" test ! -e "$work/cargo.log" -a "$(remote_fingerprint)" = "$before"
done
check "an unsafe logbook is never measured or copied (cp, du)" test ! -e "$work/trap.log"
reset_remote
echo "$R/home/.local" >"$R/logbook_path"
deploy --dry-run --branch next "$log"
check "a logbook above seldon-dev: refused in the dry run too" test "$rc" = 1
# the backup's size cap
reset_remote
mkdir -p "$R/home/Seldon"
head -c $((2 * 1024 * 1024)) /dev/zero >"$R/home/Seldon/big"
before=$(remote_fingerprint)
rm -f "$work/cargo.log"
BACKUP_MAX_MB=1 deploy --branch next "$log"
check "backup over the cap: refused" test "$rc" = 1
check "backup over the cap: says how large and the cap" grep -qE "the backup on $host would copy [23] MiB \(logbook $R/home/Seldon, .*the cap is 1 MiB \(SELDON_DEPLOY_BACKUP_MAX_MB\)" <<<"$out"
check "backup over the cap: no build, the host unchanged" test ! -e "$work/cargo.log" -a "$(remote_fingerprint)" = "$before"
# du tells nothing: the size is unknown, refused
reset_remote
touch "$R/du_empty"
before=$(remote_fingerprint)
rm -f "$work/cargo.log"
deploy --branch next "$log"
check "the host's du prints nothing: refused" test "$rc" = 1
check "the host's du prints nothing: named" has "cannot tell how large the backup on $host would be"
check "the host's du prints nothing: no build, the host unchanged" test ! -e "$work/cargo.log" -a "$(remote_fingerprint)" = "$before"
rm "$R/du_empty"
# an engine that names no logbook, or no engine: refused, with the fix
for case in "no engine" "an engine that names no logbook"; do
  reset_remote
  if [[ $case == "no engine" ]]; then rm "$R/home/.local/bin/seldon"; else : >"$R/logbook_path"; fi
  before=$(remote_fingerprint)
  rm -f "$work/cargo.log"
  deploy --branch next "$log"
  check "$case: refused" test "$rc" = 1
  check "$case: named, with the fix" has "engine does not name a logbook (seldon doctor --json, .logbook), so the backup would miss it; install or fix the engine there (a main deploy, or install.sh) until \`seldon doctor --json\` names the logbook, then deploy next again"
  check "$case: no build, the host unchanged" test ! -e "$work/cargo.log" -a "$(remote_fingerprint)" = "$before"
done
reset_remote
BACKUP_MAX_MB=lots deploy --branch next "$log"
check "a cap that is not a number: refused" has "SELDON_DEPLOY_BACKUP_MAX_MB wants a number of MiB, not 'lots'"
deploy --branch next "$log"
check "backup under the default cap (1024 MiB): deploys" test "$rc" = 0

# the engine says next, the plugin marker does not (a first next deploy that
# failed after the engine swap): no second backup
reset_remote
on main
deploy "$log" >/dev/null
fake_seldon "0.1.3+next.0000000" >"$R/home/.local/bin/seldon"
deploy --dry-run "$log"
check "engine next, marker main: a main deploy warns too" \
  has "runs a next build (plugin main.$main_short, engine 0.1.3+next.0000000); main cannot read next's state"
on next
deploy --dry-run --branch next "$log"
check "engine next, marker main, dry run: no backup" has "backup   none: the host runs next already (plugin main.$main_short, engine 0.1.3+next.0000000)"
deploy --branch next "$log"
check "engine next, marker main: exit 0, no backup" \
  test "$rc" = 0 -a -z "$(find "$R/home/.local/state/seldon-dev" -maxdepth 1 -name 'backup-before-next-*')"

# the plugin marker says next, the engine does not (an engine put back by
# hand): the host still counts as running next, no backup
fake_seldon "0.1.3+main.$main_short" >"$R/home/.local/bin/seldon"
deploy --dry-run --branch next "$log"
check "marker next, engine main: no backup" has "backup   none: the host runs next already (plugin next.$short, engine 0.1.3+main.$main_short)"

# a dangling shell.json link is still backed up (as the link) and restored
reset_remote
on main
deploy "$log" >/dev/null
ln -s "$work/outside-files/no-such.json" "$R/home/.config/omarchy/shell.json"
on next
deploy --branch next "$log"
bk=$(find "$R/home/.local/state/seldon-dev" -maxdepth 1 -name 'backup-before-next-*')
check "a dangling shell.json link: backed up as the link" test -L "$bk/shell.json"
check "a dangling shell.json link: RESTORE.txt puts it back" grep -qF '~/.config/omarchy/shell.json.next && cp -pP' "$bk/RESTORE.txt"

# the engine and shell.json are links (to files in the test dir, outside the
# fake home): the backup copies the links, never what they point to
reset_remote
on main
deploy "$log" >/dev/null
mkdir -p "$work/outside-files"
fake_seldon "0.1.3+main.$main_short" >"$work/outside-files/seldon-real"
chmod 755 "$work/outside-files/seldon-real"
echo '{"outside":true}' >"$work/outside-files/shell-real.json"
rm "$R/home/.local/bin/seldon"
ln -s "$work/outside-files/seldon-real" "$R/home/.local/bin/seldon"
ln -s "$work/outside-files/shell-real.json" "$R/home/.config/omarchy/shell.json"
on next
deploy --branch next "$log"
check "engine and shell.json links: exit 0" test "$rc" = 0
bk=$(find "$R/home/.local/state/seldon-dev" -maxdepth 1 -name 'backup-before-next-*')
check "the engine link is backed up as the link" \
  test -L "$bk/seldon.engine" -a "$(readlink "$bk/seldon.engine")" = "$work/outside-files/seldon-real"
check "the shell.json link is backed up as the link" \
  test -L "$bk/shell.json" -a "$(readlink "$bk/shell.json")" = "$work/outside-files/shell-real.json"
check "the link targets are where they were, not copied" \
  test "$(find "$R/home/.local/state/seldon-dev" -type f \( -name seldon-real -o -name shell-real.json \) | wc -l)" = 0

# a logbook path with a newline: refused (the key=value lines would carry
# only its first line, the measured path would not be the copied one)
reset_remote
nl_logbook="$R/home/Seldon"$'\n'"x"
mkdir -p "$R/home/Seldon" "$nl_logbook"
printf '%s' "$nl_logbook" >"$R/logbook_path"
before=$(remote_fingerprint)
rm -f "$work/cargo.log"
deploy --branch next "$log"
check "a newline in the logbook path: refused" test "$rc" = 1
check "a newline in the logbook path: named" has "is not a directory below the home, or holds ~/.local/state/seldon-dev; not copying it"
check "a newline in the logbook path: no build, the host unchanged, nothing measured" \
  test ! -e "$work/cargo.log" -a "$(remote_fingerprint)" = "$before" -a ! -e "$work/trap.log"

# the logbook turns into a link out of the home after the probe: the
# install step checks again before the copy
reset_remote
mkdir -p "$R/home/Seldon"
printf '#!/bin/bash\nrm -rf "%s/home/Seldon"\nln -s "%s/outside-logbook" "%s/home/Seldon"\n' "$R" "$work" "$R" >"$R/before_ssh_2"
chmod 755 "$R/before_ssh_2"
rm -f "$work/ssh.log"
deploy --branch next "$log"
rm -f "$R/before_ssh_2"
check "logbook moved out after the probe: exit 2" test "$rc" = 2
check "logbook moved out after the probe: named" has "failed: install: backup: the logbook $R/home/Seldon is not below the home"
check "logbook moved out after the probe: nothing copied, engine unchanged" \
  test -z "$(find "$R/home/.local/state/seldon-dev" -path '*backup-before-next-*' 2>/dev/null)" -a "$(grep -c '"version":"0.1.3"}' "$R/home/.local/bin/seldon")" = 1

# the backup fails (the fake cp fails on the state dir, as root too):
# nothing changed but the partial backup and the log line; the watcher
# runs again
reset_remote
touch "$R/watch_active"
mkdir -p "$R/home/.local/state/seldon"
echo '{}' >"$R/home/.local/state/seldon/index.json"
mkdir -p "$R/home/.config/seldon"
echo 'x = 1' >"$R/home/.config/seldon/config.toml"
echo .local/state/seldon >"$R/cp_fail"
before=$(host_fingerprint)
deploy --branch next "$log"
check "backup fails: exit 2" test "$rc" = 2
check "backup fails: named" has "failed: install: backup to .local/state/seldon-dev/backup-before-next-"
check "backup fails: engine, plugin and state unchanged" test "$(host_fingerprint)" = "$before"
check "backup fails: logged with the partial backup" \
  jqe -s '(.[-1].failures[0] | startswith("install: backup to")) and (.[-1].backup | test("backup-before-next-"))' "$jsonl"
check "backup fails: no restart" test "$(count restart)" = 0
check "backup fails: the watcher started again" test -f "$R/watch_active" -a "$(count "watch start")" = 1
check "backup fails: only the state dir failed (the config before it was copied into seldon-dev)" \
  test "$(find "$R/home/.local/state/seldon-dev" -name config.toml | wc -l)" = 1 \
  -a "$(find "$R/home/.local/state/seldon-dev" -name state-seldon | wc -l)" = 0
rm "$R/cp_fail"
sleep 1 # a new stamp
deploy --branch next "$log"
check "backup fixed: the retry takes a new backup and deploys" \
  test "$rc" = 0 -a "$(find "$R/home/.local/state/seldon-dev" -maxdepth 1 -name 'backup-before-next-*' | wc -l)" = 2
on main

# ---- 10. never the real session; the real home untouched -------------------------------------
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
