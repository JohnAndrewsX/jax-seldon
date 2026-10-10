#!/usr/bin/env bash
# The terminal scripts behind the banners' Install, Create, Grant and Update
# (plugin/Model.js, WP-117), run the way Omarchy's presentation launcher runs
# them: `omarchy-show-logo; <script>; if (( $? != 130 )); then
# omarchy-show-done; fi` under `bash -c`, in a session of its own (setsid),
# so a stub can send SIGINT to the whole process group as ^C in a terminal
# does without reaching this test. sudo, curl, seldon and omarchy are stubs
# that record their argv; gum is a stub that prints its colour and text,
# and once the real gum, to prove it takes the flags. A scratch HOME;
# nothing on the host is touched.
#
# Checks per script: the command is shown as it is copied, then run (with
# `$USER` expanded only where it runs), the result line matches the
# outcome (green on success, red on failure, a failed download in
# `curl … | bash` included), the follow-up engine call runs only on
# success, and the wrapper prints Done. The grant says "recorded" only
# after a capture that succeeded, and stops before sudo with an empty
# USER. ^C (the stub dying of SIGINT, as sudo re-raises it, or exiting 1
# after catching it; or ^C during the announce lines) prints the
# cancelled line, skips the rest, and ends with 130: the wrapper prints no
# Done, and in a real terminal the window closes, as with Omarchy's own
# scripts.
set -euo pipefail

root=$(cd "$(dirname "$0")/../.." && pwd)
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
pass=0
fail=0

script_of() {
  node -e '
    const fs = require("fs"), vm = require("vm"), M = {}
    vm.createContext(M)
    vm.runInContext(fs.readFileSync(process.argv[1] + "/plugin/Model.js", "utf8"), M)
    process.stdout.write(M[process.argv[2]])' "$root" "$1"
}

stubs="$work/stubs"
mkdir -p "$stubs" "$work/home"
# Each stub logs `name argv…`. $STUB_<NAME>_EXITS lists the exit codes of
# its calls in order (the last one repeats; default 0). $STUB_<NAME>_SIGINT
# makes it the target of ^C: `die` sends SIGINT to the process group and
# dies of it, `exit1` catches it and exits 1.
stub() { # stub <name> [extra body before the exit]
  cat >"$stubs/$1" <<EOF
#!/usr/bin/env bash
printf '%s\n' "$1 \$*" >>"\$STUB_LOG"
n=\$(grep -c '^$1\( \|\$\)' "\$STUB_LOG")
sig=STUB_${1^^}_SIGINT
case \${!sig:-} in
  die) kill -INT 0; sleep 2 ;;
  exit1) trap 'exit 1' INT; kill -INT 0; sleep 2 ;;
esac
${2:-}
var=STUB_${1^^}_EXITS
read -r -a codes <<<"\${!var:-0}"
((n <= \${#codes[@]})) || n=\${#codes[@]}
exit "\${codes[n - 1]}"
EOF
}
stub sudo
stub seldon
stub omarchy
# curl prints a tiny installer, or fails like `curl -f` on a 404: no output.
stub curl "[[ \${STUB_CURL_EXITS:-0} == 0 ]] && echo 'echo \"installer ran\"; exit \${STUB_INSTALLER_EXIT:-0}'"
# gum prints its colour and text; with STUB_GUM_SIGINT=1 its first call
# is ^C (during the announce lines).
cat >"$stubs/gum" <<'EOF'
#!/usr/bin/env bash
if [[ ${STUB_GUM_SIGINT:-} == 1 && ! -e $STUB_LOG.gum ]]; then
  : >"$STUB_LOG.gum"
  kill -INT 0
  sleep 2
fi
fg=plain
while [[ $# -gt 1 ]]; do
  case $1 in
    --foreground) fg=$2; shift 2 ;;
    --padding | --width) shift 2 ;;
    *) shift ;;
  esac
done
printf '[%s] %s\n' "$fg" "$1"
EOF
printf '#!/usr/bin/env bash\necho LOGO\n' >"$stubs/omarchy-show-logo"
printf '#!/usr/bin/env bash\necho DONE\n' >"$stubs/omarchy-show-done"
chmod +x "$stubs"/*

# run <case> <script name> [VAR=value …]: output in $work/<case>.out, calls in .log
run() {
  local name=$1 script
  script=$(script_of "$2")
  shift 2
  : >"$work/$name.log"
  env -i PATH="$stubs:/usr/bin:/bin" HOME="$work/home" USER=tester STUB_LOG="$work/$name.log" "$@" \
    setsid -w bash -c "omarchy-show-logo; $script; if (( \$? != 130 )); then omarchy-show-done; fi" >"$work/$name.out" 2>&1 || true
}
has() { # has <case> <out|log> <line>
  if grep -qxF -- "$3" "$work/$1.$2"; then
    pass=$((pass + 1)); echo "ok   $1: $2 has: $3"
  else
    fail=$((fail + 1)); echo "FAIL $1: $2 lacks: $3"; sed 's/^/     /' "$work/$1.$2"
  fi
}
lacks() { # lacks <case> <out|log> <fixed text>
  if ! grep -qF -- "$3" "$work/$1.$2"; then
    pass=$((pass + 1)); echo "ok   $1: $2 lacks: $3"
  else
    fail=$((fail + 1)); echo "FAIL $1: $2 has: $3"; sed 's/^/     /' "$work/$1.$2"
  fi
}
log_is() { # log_is <case> <expected calls, one per line>
  if [[ $(cat "$work/$1.log") == "$2" ]]; then
    pass=$((pass + 1)); echo "ok   $1: calls"
  else
    fail=$((fail + 1)); echo "FAIL $1: calls were:"; sed 's/^/     /' "$work/$1.log"
  fi
}
# cancelled <case> <line>: the cancelled line, nothing after it, no Done
cancelled() {
  has "$1" out "[3] $2"
  if [[ $(tail -n 1 "$work/$1.out") == "[3] $2" ]]; then
    pass=$((pass + 1)); echo "ok   $1: the cancelled line is the last, no Done"
  else
    fail=$((fail + 1)); echo "FAIL $1: output after the cancelled line:"; sed 's/^/     /' "$work/$1.out"
  fi
}

install='curl -fsSL https://github.com/JohnAndrewsX/jax-seldon/releases/latest/download/install.sh | bash'
install_url='curl -fsSL https://github.com/JohnAndrewsX/jax-seldon/releases/latest/download/install.sh'
grant='sudo setfacl -m u:tester:rx /.snapshots'
recorded="[2] Snapshots are now recorded. The panel updates by itself."
granted="[2] Read access granted. The snapshots were not recorded yet; Seldon tries again at its next capture."
grant_failed="[1] Nothing changed. Snapshots stay off; Seldon works without them."
install_failed="[1] The install did not finish. Run it again; your logbook is untouched."

# Snapshot read grant.
run grant-ok SNAPPER_FIX_SCRIPT
has grant-ok out "LOGO"
has grant-ok out "[plain] Seldon: let your user read the snapshot list"
has grant-ok out '[plain] sudo setfacl -m u:$USER:rx /.snapshots'
has grant-ok out "$recorded"
lacks grant-ok out "Nothing changed"
has grant-ok out "DONE"
log_is grant-ok "$(printf '%s\n' "$grant" "seldon capture")"
# the plugin's own capture holds the lock: one more try, then recorded
run grant-locked SNAPPER_FIX_SCRIPT STUB_SELDON_EXITS="4 0"
has grant-locked out "$recorded"
has grant-locked out "DONE"
log_is grant-locked "$(printf '%s\n' "$grant" "seldon capture" "seldon capture")"
# both captures fail: the grant happened, "recorded" would overclaim
run grant-uncaptured SNAPPER_FIX_SCRIPT STUB_SELDON_EXITS="4 4"
has grant-uncaptured out "$granted"
lacks grant-uncaptured out "now recorded"
has grant-uncaptured out "DONE"
log_is grant-uncaptured "$(printf '%s\n' "$grant" "seldon capture" "seldon capture")"
# a wrong password: nothing changed, no capture
run grant-refused SNAPPER_FIX_SCRIPT STUB_SUDO_EXITS=1
has grant-refused out "$grant_failed"
lacks grant-refused out "now recorded"
has grant-refused out "DONE"
log_is grant-refused "$grant"
# an empty USER: stops before sudo instead of granting u::rx
run grant-no-user SNAPPER_FIX_SCRIPT USER=
has grant-no-user out "$grant_failed"
has grant-no-user out "DONE"
log_is grant-no-user ""
# ^C at the password prompt, both ways sudo can end
run grant-ctrl-c SNAPPER_FIX_SCRIPT STUB_SUDO_SIGINT=die
cancelled grant-ctrl-c "Cancelled. Nothing changed."
log_is grant-ctrl-c "$grant"
run grant-ctrl-c-exit1 SNAPPER_FIX_SCRIPT STUB_SUDO_SIGINT=exit1
cancelled grant-ctrl-c-exit1 "Cancelled. Nothing changed."
log_is grant-ctrl-c-exit1 "$grant"
# ^C while the announce lines print: the command never starts
run grant-ctrl-c-early SNAPPER_FIX_SCRIPT STUB_GUM_SIGINT=1
cancelled grant-ctrl-c-early "Cancelled. Nothing changed."
log_is grant-ctrl-c-early ""

# Engine install: curl | bash, with pipefail.
run install-ok INSTALL_ENGINE_SCRIPT
has install-ok out "[plain] Seldon: install the engine"
has install-ok out "[plain] $install"
has install-ok out "installer ran"
has install-ok out "[2] The engine is installed. The Seldon panel finds it by itself."
has install-ok out "DONE"
log_is install-ok "$install_url"
run install-404 INSTALL_ENGINE_SCRIPT STUB_CURL_EXITS=22
has install-404 out "$install_failed"
lacks install-404 out "is installed"
has install-404 out "DONE"
run install-failed INSTALL_ENGINE_SCRIPT STUB_INSTALLER_EXIT=1
has install-failed out "$install_failed"
run install-ctrl-c INSTALL_ENGINE_SCRIPT STUB_CURL_SIGINT=die
cancelled install-ctrl-c "Cancelled. The install did not finish. Run it again; your logbook is untouched."

# Engine update: the new engine rewrites the index once, on success only.
run update-engine-ok UPDATE_ENGINE_SCRIPT
has update-engine-ok out "[2] The engine is updated. In the Seldon panel, press Check again."
log_is update-engine-ok "$(printf '%s\n' "$install_url" "seldon status")"
run update-engine-404 UPDATE_ENGINE_SCRIPT STUB_CURL_EXITS=22
has update-engine-404 out "[1] The update did not finish. Run it again; your logbook is untouched."
log_is update-engine-404 "$install_url"

# Plugin update.
run update-plugin-ok UPDATE_PLUGIN_SCRIPT
has update-plugin-ok out "[plain] omarchy plugin update jax.seldon"
has update-plugin-ok out "[2] If the plugin was updated, the Seldon panel offers Restart shell to load it."
log_is update-plugin-ok "omarchy plugin update jax.seldon"
run update-plugin-failed UPDATE_PLUGIN_SCRIPT STUB_OMARCHY_EXITS=1
has update-plugin-failed out "[1] Nothing changed. The plugin stays at its version."

# seldon init --defaults: no question (WP-119, ADR-0033).
run init-ok INIT_SCRIPT
has init-ok out "[plain] Seldon: create your logbook"
has init-ok out "[plain] seldon init --defaults"
has init-ok out "[2] Your logbook is ready. The panel updates by itself."
log_is init-ok "seldon init --defaults"
run init-failed INIT_SCRIPT STUB_SELDON_EXITS=1
has init-failed out "[1] No logbook was created; the message above says why. When the folder is in use, the panel offers Choose a folder; else press Create logbook to try again."
has init-failed out "DONE"
run init-ctrl-c INIT_SCRIPT STUB_SELDON_SIGINT=die
cancelled init-ctrl-c "Cancelled. Press Create logbook in the panel to start again."

# plain seldon init, which asks where (the setup card's Choose a folder,
# WP-119 round 2).
run init-ask-ok INIT_ASK_SCRIPT
has init-ask-ok out "[plain] Seldon: choose where your logbook goes"
has init-ask-ok out "[plain] seldon init"
has init-ask-ok out "[2] Your logbook is ready. The panel updates by itself."
log_is init-ask-ok "seldon init"
run init-ask-failed INIT_ASK_SCRIPT STUB_SELDON_EXITS=1
has init-ask-failed out "[1] No logbook was created; the message above says why. Press Choose a folder in the panel to try again."
has init-ask-failed out "DONE"
run init-ask-ctrl-c INIT_ASK_SCRIPT STUB_SELDON_SIGINT=die
cancelled init-ask-ctrl-c "Cancelled. Press Choose a folder in the panel to start again."

# The real gum takes every flag the scripts use (it exits non-zero on an
# unknown one, and the text would be missing).
if command -v gum >/dev/null; then
  mv "$stubs/gum" "$work/gum-stub"
  run real-gum SNAPPER_FIX_SCRIPT STUB_SUDO_EXITS=1
  has real-gum out "Seldon: let your user read the snapshot list"
  has real-gum out '  sudo setfacl -m u:$USER:rx /.snapshots'
  has real-gum out "Nothing changed. Snapshots stay off; Seldon works without them."
  lacks real-gum out "gum:"
  run real-gum-ok INIT_SCRIPT
  has real-gum-ok out "Your logbook is ready. The panel updates by itself."
  run real-gum-ctrl-c SNAPPER_FIX_SCRIPT STUB_SUDO_SIGINT=die
  has real-gum-ctrl-c out "Cancelled. Nothing changed."
  lacks real-gum-ctrl-c out "DONE"
else
  echo "note terminal-scripts: gum not installed; the real-gum cases are skipped"
fi

echo "terminal-scripts: $pass passed, $fail failed"
((fail == 0))
