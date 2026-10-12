#!/usr/bin/env bash
# Self-test of tests/plugin/real-home-guard.sh in scratch HOMEs: a change
# by the operator's own engine passes, a test leak still fails; a new
# Quickshell instance dir in a watched runtime dir fails (WP-161); every
# state file engine/src names is one the guard knows (WP-212). Needs jq.
set -euo pipefail

root=$(cd "$(dirname "$0")/../.." && pwd)
command -v jq >/dev/null || { echo "real-home-guard.test: jq not found" >&2; exit 1; }
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

total_pass=0
total_fail=0

# guard_case <name> <want: ok|live|FAIL> <config logbook, @ = HOME; "-": no config> <mutation> [runtime want: ok|FAIL] [state files]
# A fresh HOME with a config.toml (unless "-") and an engine state dir
# (index.json, cursors.json, lock and the named state files), a
# session runtime dir $HOME/run with one Quickshell instance dir (the live
# shell's), the guard sourced, the mutation run (bash, in HOME), then
# real_home_check: its first line is the home's verdict, the rest the
# runtime dirs' (all ok unless the runtime want says FAIL). The inherited
# XDG_RUNTIME_DIR is $HOME/xdg when the mutation names it, else unset.
case_script='
  set -euo pipefail
  pass=0; fail=0
  real_runtime_session=$HOME/run
  source "$1/tests/plugin/real-home-guard.sh"
  sleep 0.05
  cd "$HOME"; eval "$2"
  real_home_check guard'

guard_case() {
  local name=$1 want=$2 logbook=$3 mutation=$4 rt_want=${5:-ok} seed=${6:-} home="$work/$1" out got rt_got xdg=() f
  logbook=${logbook//@/$home}
  mkdir -p "$home/.config/seldon" "$home/.local/state/seldon/hooks" "$home/run/quickshell/by-id/shell1" "$home/xdg"
  [[ $mutation == *xdg* ]] && xdg=(XDG_RUNTIME_DIR="$home/xdg")
  [[ $logbook == - ]] || printf '# test\nlanguage = "de"\nlogbook = "%s"\n' "$logbook" >"$home/.config/seldon/config.toml"
  echo '{"logbook":{"path":"'"$home"'/Seldon","language":"de","machine":"host-1"},"contractVersion":1}' \
    >"$home/.local/state/seldon/index.json"
  echo '{}' >"$home/.local/state/seldon/cursors.json"
  : >"$home/.local/state/seldon/lock"
  for f in $seed; do echo '{}' >"$home/.local/state/seldon/$f"; done
  out=$(env -u XDG_RUNTIME_DIR HOME="$home" "${xdg[@]}" bash -c "$case_script" _ "$root" "$mutation" || true) # live runtime dir: unset; a case names its own
  rt_got=ok
  grep -q '^FAIL guard: new entries no process holds in ' <<<"$out" && rt_got=FAIL
  if [[ $rt_got != "$rt_want" ]]; then
    total_fail=$((total_fail + 1)); echo "FAIL $name runtime: $rt_got (want $rt_want)"; echo "$out" | sed 's/^/     /'
  else
    total_pass=$((total_pass + 1)); echo "ok   $name runtime: $rt_want"
  fi
  out=$(head -n 1 <<<"$out")
  case $out in
    "ok   guard: the real ~/.local/state/seldon and ~/.config/seldon are untouched") got=ok ;;
    "ok   guard: the real ~/.local/state/seldon changed by the operator's live engine (not a leak)") got=live ;;
    "FAIL guard:"*) got=FAIL ;;
    *) got="? $out" ;;
  esac
  if [[ $got == "$want" ]]; then
    total_pass=$((total_pass + 1)); echo "ok   $name: $want"
  else
    total_fail=$((total_fail + 1)); echo "FAIL $name: $got (want $want)"; echo "$out" | sed 's/^/     /'
  fi
}

# The engine's rewrite of its own state: same logbook, same machine.
rewrite='sed "s/\"contractVersion\"/\"x\":2,\"contractVersion\"/" .local/state/seldon/index.json >i && mv i .local/state/seldon/index.json && echo "{\"a\":1}" >.local/state/seldon/cursors.json && touch .local/state/seldon/lock'

guard_case untouched ok "@/Seldon" ":"
guard_case engine-rewrite live "@/Seldon" "$rewrite"
guard_case tilde-config live "~/Seldon" "$rewrite"
guard_case manifest-appears FAIL "@/Seldon" "$rewrite; echo {} >.local/state/seldon/manifest.json"
guard_case other-logbook FAIL "@/Seldon" 'echo "{\"logbook\":{\"path\":\"/tmp/test-logbook\",\"machine\":\"host-1\"}}" >.local/state/seldon/index.json'
guard_case other-machine FAIL "@/Seldon" 'sed -i "s/host-1/host-2/" .local/state/seldon/index.json'
guard_case new-file FAIL "@/Seldon" "$rewrite; echo x >.local/state/seldon/calls.log"
guard_case removed-file FAIL "@/Seldon" "$rewrite; rm .local/state/seldon/cursors.json"
guard_case hooks-changed FAIL "@/Seldon" "$rewrite; touch .local/state/seldon/hooks"
guard_case config-changed FAIL "@/Seldon" "$rewrite; echo '# edited' >>.config/seldon/config.toml"
guard_case no-config FAIL - "$rewrite"

# Every other file the engine writes during routine work (WP-212): a
# capture rewrites it alone (recent-config.json every 15 minutes) or with
# the index. A file that is never the engine's, or one it writes only on
# an operator's action, still fails.
for f in manifest.json owned.json recent-config.json autocommit.json; do
  guard_case "engine-$f" live "@/Seldon" "echo '{\"a\":1}' >.local/state/seldon/$f" ok "$f"
  guard_case "engine-rewrite-$f" live "@/Seldon" "$rewrite; echo '{\"a\":1}' >.local/state/seldon/$f" ok "$f"
done
guard_case engine-all live "@/Seldon" "$rewrite; for f in manifest.json owned.json recent-config.json autocommit.json; do echo 1 >>.local/state/seldon/\$f; done" ok "manifest.json owned.json recent-config.json autocommit.json"
guard_case other-file-changed FAIL "@/Seldon" "$rewrite; echo x >>.local/state/seldon/calls.log" ok "calls.log"
guard_case recent-config-like FAIL "@/Seldon" "$rewrite; echo x >>.local/state/seldon/recent-config.json.bak" ok "recent-config.json.bak"
guard_case launches-changed FAIL "@/Seldon" "$rewrite; echo x >>.local/state/seldon/launches.json" ok "launches.json"
guard_case recent-config-appears FAIL "@/Seldon" "$rewrite; echo {} >.local/state/seldon/recent-config.json"

# The session's runtime dir (WP-161): the live shell's instance dir stays,
# a harness Quickshell adds one.
guard_case runtime-untouched ok "@/Seldon" "ls run/quickshell/by-id >/dev/null"
guard_case runtime-new-instance ok "@/Seldon" "mkdir run/quickshell/by-id/test2" FAIL
guard_case runtime-first-instance ok "@/Seldon" "rm -r run/quickshell; mkdir -p run/quickshell/by-id/test2" FAIL
guard_case runtime-instance-gone ok "@/Seldon" "rm -r run/quickshell/by-id/shell1"
guard_case runtime-swapped ok "@/Seldon" "rm -r run/quickshell/by-id/shell1; mkdir run/quickshell/by-id/test2" FAIL
# A live instance started during the run (another Quickshell app, a shell
# restart) holds its files open and is no leftover; one held by nobody
# next to it still fails.
guard_case runtime-live-instance ok "@/Seldon" "mkdir run/quickshell/by-id/live2; exec 9>run/quickshell/by-id/live2/instance.lock"
guard_case runtime-live-and-leftover ok "@/Seldon" "mkdir run/quickshell/by-id/live2 run/quickshell/by-id/test2; exec 9>run/quickshell/by-id/live2/instance.lock" FAIL
# An inherited XDG_RUNTIME_DIR (a private one for the whole check) is
# watched as well.
guard_case runtime-xdg-untouched ok "@/Seldon" ": xdg"
guard_case runtime-xdg-new-instance ok "@/Seldon" "mkdir -p xdg/quickshell/by-id/test2" FAIL

# The guard's lists against engine/src (WP-212): every name the engine
# joins onto its state dir is in real_engine_files (routine) or
# real_engine_not_routine, and every listed name is still the engine's.

# engine_state_names <src dir> — one line per `state_dir.join(…)` in the
# Rust sources: the literal's value, or a constant's (the `const` of that
# name in the same file, else the only value of a `const` of that name in
# the tree); "?<file>: <argument>" when neither resolves.
engine_state_names() {
  local src=$1 line file arg name value
  while IFS= read -r line; do
    file=${line%%:*}
    arg=${line#*.join(}
    arg=${arg%)}
    arg=${arg//[[:space:]]/}
    if [[ $arg =~ ^\"([^\"]+)\"$ ]]; then
      echo "${BASH_REMATCH[1]}"
      continue
    fi
    name=${arg##*::}
    value=""
    if [[ $name =~ ^[A-Z][A-Z0-9_]*$ ]]; then
      value=$(grep -hoE "const $name: &str = \"[^\"]+\"" "$file" | sort -u || true)
      [[ -n $value ]] || value=$(grep -rhoE --include='*.rs' "const $name: &str = \"[^\"]+\"" "$src" | sort -u || true)
    fi
    if [[ -n $value && $value != *$'\n'* ]]; then
      value=${value#*\"}
      echo "${value%\"}"
    else
      echo "?${file#"$src"/}: $arg"
    fi
  done < <(grep -rHoE --include='*.rs' 'state_dir[[:space:]]*\.join\([^)]*\)' "$src" || true)
}

# The guard's two lists, sourced in a scratch HOME ("r <name>", "n <name>").
lists_home="$work/lists"
mkdir -p "$lists_home/run"
# shellcheck disable=SC2016 # expanded by the inner bash
guard_lists=$(env -u XDG_RUNTIME_DIR HOME="$lists_home" real_runtime_session="$lists_home/run" bash -c \
  'source "$1"; printf "r %s\n" "${real_engine_files[@]}"; printf "n %s\n" "${real_engine_not_routine[@]}"' \
  _ "$root/tests/plugin/real-home-guard.sh") # live runtime dir: unset; a scratch session dir is watched

# drift_case <name> <want: ok|FAIL> <src dir>
drift_case() {
  local name=$1 want=$2 src=$3 engine known problems=() n got
  engine=$(engine_state_names "$src" | LC_ALL=C sort -u)
  known=$(cut -d' ' -f2- <<<"$guard_lists" | LC_ALL=C sort -u)
  while IFS= read -r n; do
    [[ -n $n ]] || continue
    if [[ $n == '?'* ]]; then problems+=("unresolved state path ${n#\?}")
    else problems+=("$n: the engine writes it in its state dir, the guard knows it in neither list"); fi
  done < <(LC_ALL=C comm -23 <(echo "$engine") <(echo "$known"))
  while IFS= read -r n; do
    [[ -n $n ]] && problems+=("$n: the guard lists it, engine/src does not name it")
  done < <(LC_ALL=C comm -13 <(echo "$engine") <(echo "$known"))
  got=ok
  ((${#problems[@]} == 0)) || got=FAIL
  if [[ $got == "$want" ]]; then
    total_pass=$((total_pass + 1)); echo "ok   $name: $want ($(grep -c . <<<"$engine") state names in the engine)"
  else
    total_fail=$((total_fail + 1)); echo "FAIL $name: $got (want $want)"
  fi
  ((${#problems[@]} == 0)) || printf '     %s\n' "${problems[@]}"
}

drift_case drift-engine ok "$root/engine/src"
# A copy of engine/src with one more state file: a literal, a constant, a
# name nothing resolves, and a known file dropped from the guard's view.
for d in literal constant unresolved; do
  mkdir -p "$work/drift-$d"
  cp -r "$root/engine/src" "$work/drift-$d/src"
done
printf '\nfn wp212(d: &crate::config::Dirs) -> std::path::PathBuf {\n    d.state_dir.join("new-state.json")\n}\n' \
  >>"$work/drift-literal/src/collectors/recent.rs"
printf '\npub const WP212_FILE: &str = "new-state.json";\nfn wp212(d: &crate::config::Dirs) -> std::path::PathBuf {\n    d.state_dir.join(WP212_FILE)\n}\n' \
  >>"$work/drift-constant/src/collectors/recent.rs"
printf '\nfn wp212(d: &crate::config::Dirs, name: &str) -> std::path::PathBuf {\n    d.state_dir.join(name)\n}\n' \
  >>"$work/drift-unresolved/src/collectors/recent.rs"
drift_case drift-new-literal FAIL "$work/drift-literal/src"
drift_case drift-new-constant FAIL "$work/drift-constant/src"
drift_case drift-unresolved FAIL "$work/drift-unresolved/src"
mkdir -p "$work/drift-gone"
cp -r "$root/engine/src" "$work/drift-gone/src"
sed -i 's/state_dir.join("autocommit.json")/state_dir.join("autocommit-v2.json")/' "$work/drift-gone/src/index/autocommit.rs"
drift_case drift-renamed FAIL "$work/drift-gone/src"

echo "real-home-guard.test: $total_pass passed, $total_fail failed"
((total_fail == 0))
