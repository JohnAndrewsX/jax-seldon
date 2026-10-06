#!/usr/bin/env bash
# The terminal scripts behind the banners' Install, Create, Grant and Update
# (plugin/Model.js, WP-117), run the way Omarchy's presentation launcher runs
# them: `omarchy-show-logo; <script>; if (( $? != 130 )); then
# omarchy-show-done; fi` under `bash -c`. sudo, curl, seldon and omarchy are
# stubs that record their argv; gum is a stub that prints its colour and
# text, and once the real gum, to prove it takes the flags. A scratch HOME;
# nothing on the host is touched.
#
# Checks per script: the command is shown as it is copied, then run (with
# `$USER` expanded only where it runs), the result line matches the
# outcome (green on success, red on failure, a failed download in
# `curl … | bash` included), the follow-up engine call runs only on
# success, and the wrapper still prints Done.
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
# Each stub logs `name argv…` and exits with $STUB_<NAME>_EXIT (default 0).
for name in sudo seldon omarchy; do
  cat >"$stubs/$name" <<EOF
#!/usr/bin/env bash
printf '%s\n' "$name \$*" >>"\$STUB_LOG"
var=STUB_${name^^}_EXIT
exit "\${!var:-0}"
EOF
done
# curl prints a tiny installer, or fails like `curl -f` on a 404: no output.
cat >"$stubs/curl" <<'EOF'
#!/usr/bin/env bash
printf '%s\n' "curl $*" >>"$STUB_LOG"
[[ ${STUB_CURL_EXIT:-0} == 0 ]] || exit "$STUB_CURL_EXIT"
echo 'echo "installer ran"; exit ${STUB_INSTALLER_EXIT:-0}'
EOF
cat >"$stubs/gum" <<'EOF'
#!/usr/bin/env bash
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
    bash -c "omarchy-show-logo; $script; if (( \$? != 130 )); then omarchy-show-done; fi" >"$work/$name.out" 2>&1 || true
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

install='curl -fsSL https://github.com/JohnAndrewsX/jax-seldon/releases/latest/download/install.sh | bash'
install_url='curl -fsSL https://github.com/JohnAndrewsX/jax-seldon/releases/latest/download/install.sh'

# Snapshot read grant.
run grant-ok SNAPPER_FIX_SCRIPT
has grant-ok out "LOGO"
has grant-ok out "[plain] Seldon: let your user read the snapshot list"
has grant-ok out '[plain] sudo setfacl -m u:$USER:rx /.snapshots'
has grant-ok out "[2] Snapshots are now recorded. The panel updates by itself."
lacks grant-ok out "Nothing changed"
has grant-ok out "DONE"
log_is grant-ok "$(printf '%s\n' "sudo setfacl -m u:tester:rx /.snapshots" "seldon capture")"
# the plugin's own capture holds the lock: one more try, still the green line
run grant-locked SNAPPER_FIX_SCRIPT STUB_SELDON_EXIT=4
has grant-locked out "[2] Snapshots are now recorded. The panel updates by itself."
has grant-locked out "DONE"
log_is grant-locked "$(printf '%s\n' "sudo setfacl -m u:tester:rx /.snapshots" "seldon capture" "seldon capture")"
# a wrong password (or ^C at the prompt): nothing changed, no capture
run grant-refused SNAPPER_FIX_SCRIPT STUB_SUDO_EXIT=1
has grant-refused out "[1] Nothing changed. Snapshots stay off; Seldon works without them."
lacks grant-refused out "now recorded"
has grant-refused out "DONE"
log_is grant-refused "sudo setfacl -m u:tester:rx /.snapshots"

# Engine install: curl | bash, with pipefail.
run install-ok INSTALL_ENGINE_SCRIPT
has install-ok out "[plain] Seldon: install the engine"
has install-ok out "[plain] $install"
has install-ok out "installer ran"
has install-ok out "[2] The engine is installed. In the Seldon panel, press Check again."
has install-ok out "DONE"
log_is install-ok "$install_url"
run install-404 INSTALL_ENGINE_SCRIPT STUB_CURL_EXIT=22
has install-404 out "[1] Nothing changed. The engine is not installed."
lacks install-404 out "is installed"
has install-404 out "DONE"
run install-failed INSTALL_ENGINE_SCRIPT STUB_INSTALLER_EXIT=1
has install-failed out "[1] Nothing changed. The engine is not installed."

# Engine update: the new engine rewrites the index once, on success only.
run update-engine-ok UPDATE_ENGINE_SCRIPT
has update-engine-ok out "[2] The engine is updated. In the Seldon panel, press Check again."
log_is update-engine-ok "$(printf '%s\n' "$install_url" "seldon status")"
run update-engine-404 UPDATE_ENGINE_SCRIPT STUB_CURL_EXIT=22
has update-engine-404 out "[1] Nothing changed. The engine stays at its version."
log_is update-engine-404 "$install_url"

# Plugin update.
run update-plugin-ok UPDATE_PLUGIN_SCRIPT
has update-plugin-ok out "[plain] omarchy plugin update jax.seldon"
has update-plugin-ok out "[2] If the plugin was updated, the Seldon panel offers Restart shell to load it."
log_is update-plugin-ok "omarchy plugin update jax.seldon"
run update-plugin-failed UPDATE_PLUGIN_SCRIPT STUB_OMARCHY_EXIT=1
has update-plugin-failed out "[1] Nothing changed. The plugin stays at its version."

# seldon init.
run init-ok INIT_SCRIPT
has init-ok out "[plain] Seldon: create your logbook"
has init-ok out "[plain] seldon init"
has init-ok out "[2] Your logbook is ready. The panel updates by itself."
log_is init-ok "seldon init"
run init-failed INIT_SCRIPT STUB_SELDON_EXIT=1
has init-failed out "[1] No logbook was created; the message above says why. Press Create in the panel to try again."
has init-failed out "DONE"

# The real gum takes every flag the scripts use (it exits non-zero on an
# unknown one, and the text would be missing).
if command -v gum >/dev/null; then
  rm "$stubs/gum"
  run real-gum SNAPPER_FIX_SCRIPT STUB_SUDO_EXIT=1
  has real-gum out "Seldon: let your user read the snapshot list"
  has real-gum out '  sudo setfacl -m u:$USER:rx /.snapshots'
  has real-gum out "Nothing changed. Snapshots stay off; Seldon works without them."
  lacks real-gum out "gum:"
  run real-gum-ok INIT_SCRIPT
  has real-gum-ok out "Your logbook is ready. The panel updates by itself."
else
  echo "note terminal-scripts: gum not installed; the real-gum cases are skipped"
fi

echo "terminal-scripts: $pass passed, $fail failed"
((fail == 0))
