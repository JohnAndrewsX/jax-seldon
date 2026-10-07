#!/usr/bin/env bash
# Expectation table for scripts/guard.sh: B = must block (exit 2), A = must allow (exit 0)
# The guard only reads the command text; every row runs with a fixed fake
# HOME and working directory, so nothing depends on this machine.
#   GUARD_SH=<path>        test another copy of guard.sh (scripts/guard-mutants.sh)
#   GUARD_TEST_FAILFAST=1  stop at the first failing row
#   GUARD_TEST_QUIET=1     print only failing rows and the summary
#   GUARD_TEST_REPORT=<f>  append one JSON line per row (section, verdict, guard
#                          message) to <f>; scripts/guard-table.py uses it
G=${GUARD_SH:-"$(dirname "$0")/guard.sh"}
# SELDON_TEST_GUARD lets GUARD_HOSTS_FILE replace the git-ignored hosts file.
export HOME=/home/tester OMARCHY_PATH=/usr/share/omarchy SELDON_TEST_GUARD=1
unset TMPDIR
cwd=/home/tester/repo
fail=0 rows=0 sec=
# section TITLE: the theme of the rows that follow (scripts/guard-table.py groups by it)
section() { sec=$1; }
check() {
  local want=$1 cmd=$2 label=${3:-$2} err
  rows=$((rows + 1))
  err=$(printf '{"tool_input":{"command":%s},"cwd":"%s"}' "$(printf '%s' "$cmd" | jq -Rs .)" "$cwd" | bash "$G" 2>&1 >/dev/null)
  local rc=$?
  local got=?; [ $rc -eq 0 ] && got=A; [ $rc -eq 2 ] && got=B
  if [ -n "${GUARD_TEST_REPORT:-}" ]; then
    local context=
    if [ -z "${SELDON_TEST_GUARD:-}" ]; then context="SELDON_TEST_GUARD unset"
    elif [ "${GUARD_HOSTS_FILE:-}" = /nonexistent ]; then context="no hosts file"
    elif [ -n "${GUARD_HOSTS_FILE:-}" ]; then context="test-host listed"; fi
    [ -n "${GUARD_TIME_BUDGET:-}" ] && context="${context:+$context, }time budget ${GUARD_TIME_BUDGET} s"
    jq -cn --arg section "$sec" --arg want "$want" --arg got "$got" --arg input "$label" --arg reason "$err" \
      --arg context "$context" '$ARGS.named' >> "$GUARD_TEST_REPORT"
  fi
  if [ "$got" != "$want" ]; then
    echo "FAIL want=$want got=$got :: $label"; fail=1
    [ -n "${GUARD_TEST_FAILFAST:-}" ] && exit 1
  elif [ -z "${GUARD_TEST_QUIET:-}" ]; then
    echo "ok   $want :: $label"
  fi
  return 0
}
section 'The original table (before WP-130)'
check B 'sudo ls'
check B 'pacman -Syu'
check B 'cd /x && pacman -S zed'
check B 'env FOO=1 pacman -S zed'
check B 'yay -S foo'
check B 'omarchy pkg add foo'
check B 'omarchy theme set tokyo-night'
check B 'omarchy plugin add https://x'
check B 'systemctl enable foo'
check B 'cp a.txt /etc/foo'
check B 'cp a.txt /usr/share/x'
check B 'mv x /var/lib/y'
check B 'echo hi > /etc/motd'
check B 'tee /usr/share/x < y'
check B 'tee -a /etc/x'
check B 'sed -i s/a/b/ /etc/x'
check B 'rm -rf /usr/share/foo'
check B 'mkdir ~/.config/foo'
check B 'cp x ~/.config/hypr/y'
check A 'cp ~/.config/omarchy/plugins/jax.seldon/x y && mkdir -p ~/.config/omarchy/plugins/jax.seldon'
check A 'cat /var/log/pacman.log | head'
check A 'cp -r /usr/share/omarchy/shell /tmp/x'
check A 'ls /usr/share/omarchy && grep foo /etc/pacman.conf'
check A 'cat > ci.yml <<EOF
run: pacman -Sy archlinux-keyring
EOF'
check A 'python3 - <<EOF
print("pacman upgrade")
EOF'
check A 'git commit -m "engine: parse pacman log"'
check A 'omarchy plugin validate plugin/'
check A 'omarchy plugin list --json'
check A 'systemctl --user status foo'
check A 'ssh test-host omarchy theme set tokyo-night'
check A 'ssh test-host "omarchy theme set tokyo-night"'
check B 'ssh test-host "omarchy theme set x; pacman -Syu"'
check B 'ssh test-host omarchy pkg add foo'
check A 'rsync -a plugin/ test:~/.config/omarchy/plugins/jax.seldon/'
check A 'echo "sudo is a word" '
check A 'ssh test-host rm -rf ~/.config/seldon'
check A 'ssh test-host "rm -rf ~/.config/seldon && ls ~/.config/seldon"'
check B 'rm -rf ~/.config/seldon'
check B 'ssh test-host rm -rf ~/.config/hypr'
check A 'cat >> engine/tests/drift.rs <<EOF
let cmd = "sed -i s/a/b/ ~/.config/hypr/bindings.conf";
EOF'
check A 'git commit -m "engine: attribute sed -i ~/.config/hypr edits"'
check B 'sed -i s/a/b/ ~/.config/hypr/bindings.conf'
check B 'cd x && cp y ~/.config/omarchy/shell.json'
check A "ssh test-host 'jq -r \".idle | select(length > 0)\" ~/.config/omarchy/shell.json'"
check A 'test "$(jq ".n > 0" x.json)" = true && cat ~/.config/omarchy/shell.json'
check A 'python3 - <<'"'"'EOF'"'"'
s = "// sources: all | pacman | snapper"
open("x.qml", "w").write(s)
EOF'
check A 'cat > notes.md <<EOF
run sudo pacman -Syu then cp x /usr/share/y
EOF'
check B 'cat > notes.md <<EOF
harmless
EOF
sudo pacman -Syu'
check B 'cat <<EOF > /etc/motd
hello
EOF'
check B 'bash <<'"'"'EOF'"'"'
sudo pacman -Syu
EOF'
check B 'ssh test-host <<EOF
omarchy pkg add foo
EOF'
check B 'grep "<<EOF" x.sh
sudo pacman -Syu'
check A 'python3 - <<EOF
print("pacman -Syu is just text here")
EOF'
check B 'omarchy agent prompt "work the case"'
check B 'omarchy-agent-prompt --prompt x'
check B 'omarchy-launch-tui claude'
check B 'cd x && omarchy launch floating-terminal-with-presentation claude'
check A 'omarchy agent usage claude --limits-only'
check A 'ssh test-host omarchy agent prompt "work the case"'
check A 'grep -rn "omarchy-launch-tui" engine/src/commands/agent.rs'
check A "ssh testhost 'cd /tmp/seldon-pkg && makepkg -f'"
check A "ssh testhost 'cd /tmp/seldon-pkg && makepkg --printsrcinfo > SRCINFO.new'"
check B "ssh testhost 'cd /tmp/seldon-pkg && makepkg -fs'"
check B "ssh testhost 'cd /tmp/seldon-pkg && makepkg -f -i'"
check B "ssh testhost 'cd /tmp/seldon-pkg && makepkg -f --syncdeps'"
check B "ssh testhost 'cd /tmp/seldon-pkg && makepkg -f \$(sudo x)'"
check B "ssh testhost 'cd /tmp/seldon-pkg && makepkg --printsrcinfo; pacman -Syu'"
check B "ssh testhost 'cd /home/x && makepkg -f'"
check B 'makepkg -f'
check A 'grep -n systemctl memory/host.md'
check A 'herdr agent prompt x "never run systemctl enable on this host"'
check A 'systemctl --user status seldon-watch'
check B 'cd /x && systemctl --user enable seldon-watch'
check B 'env FOO=1 systemctl daemon-reload'
section 'Read-only package queries'  # WP-130: read-only package queries
check A 'pacman -Q'
check A 'pacman -Qq'
check A 'pacman -Qi linux'
check A 'pacman -Ql omarchy | head'
check A 'pacman -Qqe | sort > /tmp/pkgs.txt'
check A 'pacman -Sp zed'
check A "pacman -Sp --print-format '%n %v' zed"
check A 'command -v pacman'
check B 'pacman'
check B 'pacman -U ./seldon.pkg.tar.zst'
check B 'pacman -R seldon'
check B 'pacman -Rns seldon'
check B 'pacman -Sy'
check B 'pacman -Syp zed'
check B 'pacman -Sw zed'
check B 'pacman -D --asdeps zed'
check B '/usr/bin/pacman -Syu'
check B 'yay -Syu'
check B 'yay -Q'
check B 'paru -S foo'
section 'Red-zone words only as text: echo, printf, a heredoc to a file, grep/rg patterns, commit messages, herdr prompts, jq filters'  # WP-130: the words only as text (echo/printf, heredoc to a file, grep/rg
# patterns, commit messages, herdr prompts, jq filters)
check A 'echo "run sudo pacman -Syu"'
check A 'echo sudo pacman -Syu'
check A "printf '%s\n' 'pacman -U x' 'pkexec y'"
check A 'cat > notes.md <<'"'"'EOF'"'"'
sudo pacman -Syu && systemctl enable x
EOF'
check A "grep -n 'pacman -U' engine/src/collectors/pacman.rs"
check A 'grep -n -i "jq\|pacman -S" .github/workflows/ci.yml'
check A 'rg -n "sudo|pkexec|systemctl start" docs/'
check A 'grep -rln foo /usr/share/omarchy'
check A 'git commit -m "engine: parse pacman -U and sudo lines"'
check A 'git commit -m "docs: example; pacman -Syu && omarchy update"'
check A 'herdr agent prompt engine-130 "then run: sudo pacman -Syu; omarchy update"'
check A "jq '.events[] | select(.command == \"pacman -Syu\")' fixtures/index.sample.json"
check A "jq -r '.cases[] | \"sudo \" + .title' x.json"
section 'Omarchy command names as text; help and the command list'  # WP-130: Omarchy command names as text; help and the command list
check A 'echo "omarchy update"'
check A 'grep -n "omarchy plugin add" docs/*.md'
check A 'git commit -m "docs: omarchy plugin add and omarchy update"'
check A 'omarchy update --help'
check A 'omarchy plugin add --help'
check A 'omarchy pkg add -h'
check A 'omarchy commands'
check A 'omarchy commands --json | jq length'
check B 'omarchy update'
check B 'omarchy update -y'
check B 'omarchy plugin remove jax.seldon'
check B 'omarchy update -- --help'
check B 'echo x && omarchy update'
check B 'omarchy-update'
check B 'omarchy-pkg-add foo'
section 'A scratch HOME'  # WP-130: a scratch HOME (a prefix assignment does not change the expansion
# in its own command, so that one still writes the real ~/.config)
check A 'export HOME=/tmp/seldon-t; mkdir -p "$HOME/.config/seldon"'
check A 'HOME=$(mktemp -d) && mkdir -p "$HOME/.config/omarchy" && touch ~/.config/omarchy/shell.json'
check A 't=$(mktemp -d); export HOME=$t; mkdir -p ~/.config/hypr'
check A 'export HOME="$PWD/target/home"; mkdir -p "$HOME/.config/seldon"'
check A "HOME=/tmp/h bash -c 'mkdir -p ~/.config/seldon'"
check B 'HOME=/tmp/h mkdir -p ~/.config/seldon'
check B 'HOME=/tmp/h; unset HOME; mkdir ~/.config/x'
check A 'export HOME=/tmp/h && cd /tmp; mkdir -p ~/.config/seldon'
check B 'false || export HOME=/tmp/h; mkdir -p ~/.config/x'
check B 'false && export HOME=/tmp/h; mkdir -p ~/.config/x'
check B 'export HOME=$X; mkdir -p ~/.config/x'
check B "HOME=/tmp/h sh -c 'true'; mkdir -p ~/.config/x"
check B '(HOME=/tmp/h); mkdir -p ~/.config/x'
section "sudo setfacl in a test file's text"  # WP-130: sudo setfacl in a test file's text
check A 'cat >> engine/tests/snapper.rs <<'"'"'EOF'"'"'
// the operator grants: sudo setfacl -m u:me:rx /.snapshots
EOF'
check A 'printf "%s\n" "sudo setfacl -m u:me:rx /.snapshots" > fixtures/logs/grant.txt'
check B 'sudo setfacl -m u:me:rx /.snapshots'
section 'Read-only greps over the Omarchy tree'  # WP-130: read-only greps over the Omarchy tree
check A 'grep -rn "pacman" "$OMARCHY_PATH/migrations"'
check A 'grep -n "sudo\|systemctl enable" $OMARCHY_PATH/install.sh'
check A 'rg -l "omarchy update" /usr/share/omarchy/migrations | head'
section 'The command position, wherever it is'  # WP-130: the command position, wherever it is
check B 'pkexec ls'
check B 'run0 ls'
check B 'echo x | sudo tee /etc/x'
check B 'x=$(sudo cat /etc/shadow)'
check B 'echo "$(sudo ls)"'
check B 'echo `sudo ls`'
check B 'cat <(sudo ls)'
check B "bash -c 'sudo ls'"
check B 'sh -c "pacman -Syu"'
check B "bash -lc 'omarchy update'"
check B "bash -c 'bash -c \"systemctl start x\"'"
check B 'eval "sudo ls"'
check B "trap 'sudo ls' EXIT"
check B 'env -i PATH=/usr/bin pacman -Syu'
check B 'timeout 10 sudo ls'
check B 'nohup systemctl start foo &'
check B 'xargs sudo rm < list.txt'
check B 'find . -name x -exec sudo rm {} +'
check B 'flock /tmp/x.lock sudo ls'
check B '\sudo ls'
check B '"sudo" ls'
check B "\$'\\x73udo' ls"
check B 'if true; then sudo ls; fi'
check B 'for p in zed; do pacman -S "$p"; done'
check B 'f() { sudo ls; }; f'
check B '(cd /tmp && sudo ls)'
check B '{ sudo ls; }'
check B 'cat x | bash'
check B 'echo x && bash <<'"'"'EOF'"'"'
pacman -Syu
EOF'
check B 'cat > x.txt <<EOF
$(sudo ls)
EOF'
check A 'cat > x.txt <<'"'"'EOF'"'"'
$(sudo ls)
EOF'
check B 'sudoedit /etc/pacman.conf'
check B "script -qc 'sudo ls' /dev/null"
check B 's=sudo; $s ls'
check B '/usr/bin/pacma? -Syu'
check A '[ -f x ] && echo yes || echo no'
check A 'if [ "$(id -u)" = 0 ]; then echo root; fi'
check A 'git commit -m "$(cat <<'"'"'EOF'"'"'
engine: note the sudo pacman -Syu line

Co-Authored-By: Claude <noreply@anthropic.com>
EOF
)"'
check A 'gh pr create --title x --body "$(cat <<'"'"'EOF'"'"'
## Summary
omarchy update and systemctl enable are text here
EOF
)"'
check A 'B=engine/target/debug/seldon; $B --json doctor'
check A '"$root/engine/target/debug/seldon" --version'
check A 'flock /tmp/seldon-check.lock just check'
section 'Services'  # WP-130: services
check B 'systemctl start foo'
check B 'systemctl stop foo'
check B 'systemctl --user start seldon-watch'
check B 'systemctl --user stop seldon-watch'
check B 'systemctl --user enable --now seldon-watch'
check A 'systemctl --user is-active seldon-watch'
check A 'systemctl list-timers'
section 'Writes under the real ~/.config, ~/Seldon, ~/.local/state/seldon'  # WP-130: writes under the real ~/.config, ~/Seldon, ~/.local/state/seldon
check B 'touch ~/.config/hypr/x'
check B 'echo x >> ~/.config/hypr/hyprland.conf'
check B 'cp x $HOME/.config/omarchy/shell.json'
check B 'cd ~/.config && rm -rf hypr'
check B 'env -C ~/.config rm -rf hypr'
check B 'mkdir ~/.config/omarchy/plugins/jax.seldon/../../hypr'
check B 'rm -rf ~'
check B 'rm -rf ~/.config/omarchy/plugins'
check B 'find ~/.config/hypr -name "*.conf" -delete'
check B 'dd if=x of=~/.config/hypr/x'
check B "flock -c 'sudo ls' /tmp/l"
check B 'rsync -a x/ ~/.config/hypr/'
check A 'rsync -a plugin/ ~/.config/omarchy/plugins/jax.seldon/'
check A 'rm -rf ~/.config/omarchy/plugins/jax.seldon && cp -r plugin ~/.config/omarchy/plugins/jax.seldon'
check B 'rm -rf ~/Seldon'
check B 'rm -rf ~/.local/state/seldon'
check B 'rm -rf "$HOME/Seldon"'
check B 'cd ~ && rm -rf Seldon'
check B 'rm -r ~/.local/state'
check B 'echo x > ~/Seldon/ledger/2026-10.jsonl'
check A 'rm -rf ~/Seldon-smoke'
check A 'cat ~/Seldon/STATUS.md && ls ~/.local/state/seldon'
check A 'git -C ~/Seldon log --oneline -5'
check A 'rm -rf /tmp/seldon-x target/tmp'
check A 't=$(mktemp -d); trap '"'"'rm -rf "$t"'"'"' EXIT; cp -r fixtures "$t"'
check A 'echo hi > /dev/null 2>&1'
check A "sed -i 's|/etc/x|y|' notes.md"
check B "sed -i 's/a/b/' /etc/pacman.conf"
section 'Fail closed'  # WP-130: fail closed
check B 'echo "unterminated'
check B 'cat <<EOF
no end'
check B '$CMD -Syu'
check B 'eval "$X"'
check B 'env -S "sudo ls"'
check B 'echo ok; )'
section 'ssh to a host that is not a listed test host'  # WP-130: ssh to a host that is not a listed test host runs the same rules
check B 'ssh -o ProxyCommand="sudo nc %h %p" test-host ls'
check B 'ssh other-host sudo pacman -Syu'
check B 'printf "sudo ls\n" | ssh other-host'
check B "ssh other-host 'rm -rf ~/.config/seldon && rm -rf ~/.config/hypr'"
check B "ssh other-host 'ls; rm -rf ~/.config/seldon'"
check A "ssh other-host 'systemctl --user status seldon-watch'"
check B "ssh testhost 'cd /tmp/seldon-pkg && makepkg -f'
sudo pacman -Syu"
section 'Test hosts (scripts/guard-hosts.local)'  # test hosts (scripts/guard-hosts.local; the table uses its own file)
hosts=$(mktemp); printf '# test subjects\ntest-host\n' > "$hosts"
trap 'rm -f "$hosts"' EXIT
checkh() { GUARD_HOSTS_FILE=$hosts check "$@"; }
checkh A 'ssh test-host omarchy plugin update jax.seldon'
checkh A "ssh test-host 'omarchy plugin add https://x --enable && omarchy-restart-shell'"
checkh A 'ssh test-host "sudo setfacl -m u:eandres:rx /.snapshots"'
checkh A 'ssh -o BatchMode=yes -o ConnectTimeout=10 test-host "systemctl --user restart seldon-watch"'
checkh A 'timeout 60 ssh test-host sudo pacman -Syu'
checkh B 'ssh test-host "omarchy theme set x" && sudo pacman -Syu'
checkh B 'ssh test-host x; pacman -Syu'
checkh B 'ssh other-host omarchy plugin update jax.seldon'
checkh B 'ssh test-hostx omarchy plugin update jax.seldon'
checkh B 'sudo ssh test-host ls'
GUARD_HOSTS_FILE=/nonexistent check B 'ssh test-host omarchy plugin update jax.seldon'
section 'Holes of the old line-based guard: nothing may run locally'  # WP-130: holes of the line-based grep guard; nothing may run locally
checkh B 'ssh test-host ls
sudo pacman -Syu'
checkh B 'ssh test-host "$(sudo pacman -Syu)"'
checkh B 'ssh test-host ls > ~/.config/hypr/x'
checkh B 'ssh test-host x; omarchy agent prompt y'
checkh B 'ssh test-host sudo pacman -Syu; echo done'
checkh B 'ssh test-host true
ssh test-host sudo pacman -Syu'
checkh A 'ssh test-host journalctl --user -n 50 > /tmp/test-host.log'
checkh A "ssh test-host 'echo \$(sudo pacman -Q)'"
section 'GUARD_HOSTS_FILE counts only with SELDON_TEST_GUARD set'  # WP-130 round 2: GUARD_HOSTS_FILE counts only with SELDON_TEST_GUARD set
# (the test table); otherwise the fixed git-ignored file is read
checkx() { SELDON_TEST_GUARD='' GUARD_HOSTS_FILE=$hosts check "$@"; }
checkx B 'ssh test-host omarchy plugin update jax.seldon'
checkx B 'timeout 60 ssh test-host sudo pacman -Syu'
section 'Omarchy routes that change the system'  # WP-130 round 2: Omarchy routes that change the system, as routes and binaries
check B 'omarchy plugin enable jax.seldon'
check B 'omarchy plugin disable jax.seldon'
check B 'omarchy hook post-update'
check B 'omarchy hook'
check B 'omarchy branch set dev'
check B 'omarchy channel set edge'
check B 'omarchy-plugin-enable jax.seldon'
check B 'omarchy-plugin-disable jax.seldon'
check B 'omarchy-hook post-update'
check B 'omarchy-branch-set dev'
check B 'omarchy-channel-set edge'
check B 'ssh other-host omarchy plugin enable jax.seldon'
check A 'omarchy version'
check A 'omarchy plugin list'
check A 'omarchy channel current'
check A 'omarchy-channel-current'
check A 'omarchy hook --help'
check A 'omarchy plugin enable --help'
check A 'omarchy-plugin-list --json'
section 'The size cap: 256 KB of hook input'  # WP-130 round 2: the size cap, 256 KB of hook input, checked before parsing
# (the hook has 5 s). Exactly at the cap passes, one byte more is blocked.
cap=$((256 * 1024))
over=$(printf '{"tool_input":{"command":""},"cwd":"%s"}' "$cwd" | wc -c)
big=$(yes 'echo x;' | head -c $((cap - over)) | tr '\n' ' ')
check B "${big}x" "(echo x; repeated: hook input of $((cap + 1)) bytes)"
# the allowed row at the cap is cheap to parse (one long word), so its time
# never depends on the host's load
long=": $(head -c $((cap - over - 2)) /dev/zero | tr '\0' x)"
check A "$long" "(one long word: hook input of exactly $cap bytes)"
section 'Work bounds: 256 variables per command, a 3 s time budget'  # WP-130 round 2: work bounds inside the cap (a hook that times out does not
# block): at most 256 variables per command, and a 3 s time budget that the
# test table may shorten (never lengthen) with SELDON_TEST_GUARD set
vars=$(for i in $(seq 0 255); do printf 'v%d=1; ' "$i"; done)
check A "${vars}echo ok" "(256 distinct variables)"
check B "${vars}v256=1; echo ok" "(257 distinct variables)"
mid=$(yes 'echo x;' | head -c 10000 | tr '\n' ' ')
GUARD_TIME_BUDGET=0.001 check B "$mid" "(10 KB of echo x; with a 0.001 s budget)"
SELDON_TEST_GUARD='' GUARD_TIME_BUDGET=0.001 check A "$mid" "(the same, budget variable without SELDON_TEST_GUARD)"
section 'trap --'  # WP-130 round 3 (stage-2 sweep), A1: trap --
check B "trap -- 'sudo ls' EXIT"
check A 't=$(mktemp -d); trap -- '"'"'rm -rf "$t"'"'"' EXIT'
section 'shred options'  # A2: shred's options with an argument are -n and -s, not -u
check B 'shred -u ~/Seldon/x'
check B 'shred -n 3 -u ~/.config/hypr/x'
section 'IFS'  # A3: IFS changes word splitting; only `IFS=… read` passes
check B 'IFS=,; c=sudo,ls; $c'
check B 'IFS=/; c=sudo/ls; $c'
check B 'IFS=, c=sudo,ls; $c'
check B 'export IFS=,; c=sudo,ls; $c'
check A 'IFS=, read -r a b <<< "x,y"'
check A 'while IFS= read -r l; do echo "$l"; done < notes.md'
section 'Namerefs'  # A4: namerefs
check B 'declare -n r=A; A=sudo; $r ls'
check B 'local -n r=X; X=pacman; $r -Syu'
check B 'typeset -n r=HOME; rm -rf $r/.config/hypr'
section '${HOME…} with an operator'  # A5: ${HOME…} with an operator may still be the home
check B 'rm -rf ${HOME%/}/.config/hypr'
check B 'echo x > ${HOME#x}/.config/hypr/x'
check B 'rm -rf ${HOME/x/y}/Seldon'
check B 'rm -rf ${HOME:0}/.local/state/seldon'
section 'The working directory: a failed cd, cd -, ~-, $OLDPWD, popd, functions'  # A6: the working directory: a cd may fail, cd -, ~-, $OLDPWD, popd, functions
check B 'cd ~/.config && cd /nonexistent; echo hi > hypr/x'
check B 'cd ~/.config; cd /nonexistent; echo hi > hypr/x'
check B 'cd ~/.config && cd /tmp && cd - && echo hi > hypr/x'
check B 'cd ~/.config; cd /tmp; cd -; echo hi > hypr/x'
check B 'cd ~/.config; popd; echo hi > hypr/x'
check B 'cd ~/Seldon; cd /tmp; rm -rf ~-'
check B 'cd ~/.config; cd /tmp; rm -rf $OLDPWD/hypr'
check B 'f() { cd ~/.config; }; f; echo hi > x'
check B 'g() { sudo ls; }; g'
check A 'cd /tmp && echo hi > x'
check A 'cd target; echo hi > x'
check A 'f() { echo "$1"; }; f a; f b'
section 'Links made earlier in the command'  # A7: links made earlier in the same command
check B 'ln -s ~/.config ~/cfg; echo hi > ~/cfg/hypr/x'
check B 'ln -s ~/Seldon /tmp/s; rm -rf /tmp/s/'
check B 'ln -s /usr/bin/sudo /tmp/s; /tmp/s ls'
check B 'ln -s ~/.config/hypr /tmp/h; echo hi > /tmp/h/x'
check B 'ln ~/.config/hypr/hyprland.conf /tmp/hl; echo x >> /tmp/hl'
check A 'ln -s /tmp/a /tmp/b; echo hi > /tmp/b/x'
check A 'ln -sf /home/tester/repo/engine/target/debug/seldon /tmp/seldon && /tmp/seldon --version'
section 'ssh to this machine'  # A8: ssh to this machine runs the local rules
check B 'ssh localhost omarchy agent prompt "work the case"'
check B 'ssh 127.0.0.1 omarchy-launch-tui claude'
check B "ssh ::1 'omarchy launch floating-terminal-with-presentation claude'"
check B 'ssh localhost omarchy theme set tokyo-night'
check B 'ssh user@127.0.0.1 "omarchy theme set tokyo-night"'
check B 'ssh localhost rm -rf ~/.config/seldon'
check B 'ssh -p 22 127.0.0.2 rm -rf ~/.config/seldon'
check B 'ssh $HOSTNAME omarchy agent prompt "work the case"'
check B 'ssh $(hostname) rm -rf ~/.config/seldon'
check A 'ssh localhost ls /tmp'
section "Omarchy's own scripts"  # A9: Omarchy's own scripts do not run
check B 'bash /usr/share/omarchy/install.sh'
check B 'bash $OMARCHY_PATH/install.sh'
check B 'source $OMARCHY_PATH/install/preflight/guard.sh'
check B '. /usr/share/omarchy/migrations/1751134560.sh'
check B '/usr/share/omarchy/install.sh'
check B '"$OMARCHY_PATH/migrations/1751134560.sh"'
check B 'bash -c "$OMARCHY_PATH/install.sh"'
check B "env OMARCHY_PATH=/opt/om bash -c '\$OMARCHY_PATH/install.sh'"
check B 'for f in $OMARCHY_PATH/migrations/*.sh; do bash "$f"; done'
check B 'sh /usr/share/omarchy/install/config/all.sh'
check B 'ln -s /usr/share/omarchy/install.sh /tmp/i; bash /tmp/i'
check B '/usr/share/omarchy/bin/omarchy-update'
check A 'grep -n sudo $OMARCHY_PATH/install.sh'
check A 'cat /usr/share/omarchy/migrations/1751134560.sh'
check A 'head -20 "$OMARCHY_PATH/install.sh"'
check A 'bash -n $OMARCHY_PATH/install.sh'
check A '/usr/share/omarchy/bin/omarchy-version'
section 'More write paths: patch, tar, unzip, curl, wget, git'  # A10: more write paths: patch, tar, unzip, curl, wget, git
check B 'patch ~/.config/hypr/hyprland.conf fix.diff'
check B 'patch -d ~/.config/hypr -p1 < fix.diff'
check B 'cd ~/.config && patch -p1 -i fix.diff'
check B 'tar -xzf a.tgz -C ~/.config'
check B 'tar xzf a.tgz -C ~/Seldon'
check B 'tar -C ~/.config -xf a.tar'
check B 'cd ~/.config && tar xf a.tar'
check B 'unzip a.zip -d ~/.config/hypr'
check B 'curl -o ~/.config/hypr/x https://example.org/x'
check B 'curl -sSLo ~/.config/x https://example.org/x'
check B 'cd ~/.config && curl -O https://example.org/x'
check B 'wget -O ~/.config/x https://example.org/x'
check B 'wget -P ~/.config https://example.org/x'
check B 'git clone https://example.org/foo ~/.config/omarchy/plugins/foo'
check B 'git clone https://example.org/s ~/Seldon'
check B 'git -C ~/.config/hypr checkout .'
check B 'git -C ~/Seldon commit -am x'
check B 'cd ~/.config/hypr && git reset --hard'
check B 'git init ~/.config/x'
check B 'git worktree add ~/.config/x'
check B 'git --work-tree ~/.config/hypr checkout .'
check A 'patch -p1 < fix.diff'
check A 'tar -czf /tmp/x.tgz ~/.config/hypr'
check A 'tar -tzf a.tgz'
check A 'tar -xzf a.tgz -C /tmp/x'
check A 'unzip -l a.zip'
check A 'curl -s https://example.org/x | jq .'
check A 'curl -o /tmp/x https://example.org/x'
check A 'wget -qO- https://example.org/x'
check A 'git clone https://example.org/jax-seldon-plugin ~/.config/omarchy/plugins/jax.seldon'
check A 'git clone https://example.org/foo /tmp/foo'
check A 'git status --short && git add scripts && git commit -m x'
check A 'GIT_PAGER=cat git log -3'
check A 'GIT_EDITOR=true git rebase --continue'
section 'script without -c'  # A11: script without -c reads its shell's commands from stdin
check B "printf 'sudo ls\\n' | script -q /dev/null"
check B 'script -q /dev/null <<'"'"'EOF'"'"'
sudo ls
EOF'
check A "script -q -c 'ls' /dev/null"
section 'Aliases'  # A12: aliases
check B "alias ls=sudo; ls"
check B "shopt -s expand_aliases; alias x='sudo ls'; x"
check A 'alias'
check A 'unalias ll 2>/dev/null; ls'
section 'Exec wrappers'  # A13a: exec wrappers (the stage-2 regression)
check B 'hyprctl dispatch exec omarchy-update'
check B 'hyprctl dispatch exec "[float] omarchy update"'
check B 'hyprctl --batch "dispatch exec sudo ls ; keyword general:gaps_in 5"'
check B 'hyprctl keyword exec-once sudo ls'
check B "tmux new -d 'sudo ls'"
check B 'tmux new-session -d -s x sudo ls'
check B "tmux run-shell 'pacman -Syu'"
check B "tmux send-keys -t x 'sudo ls' Enter"
check B 'foot -e sudo ls'
check B 'alacritty -e sudo ls'
check B 'ghostty -e omarchy update'
check B 'kitty sudo ls'
check B 'taskset -c 0 sudo ls'
check B 'chrt -f 99 sudo ls'
check B 'systemd-inhibit sudo ls'
check B 'systemd-cat -t x sudo ls'
check B 'ssh-agent sudo ls'
check B 'dbus-run-session sudo ls'
check B 'dbus-launch sudo ls'
check B 'uwsm app -- sudo ls'
check B 'unbuffer sudo ls'
check B 'gdb --args sudo ls'
check B "gdb -ex 'shell sudo ls'"
check B 'bwrap --bind / / sudo ls'
check B "socat - EXEC:'sudo ls'"
check B "sg wheel -c 'sudo ls'"
check B 'parallel sudo ::: ls'
check B "rsync -e 'sh -c \"sudo ls\"' a host:b"
check B "rsync --rsh='ssh -o ProxyCommand=sudo' a host:b"
check B 'scp -S /tmp/evil a host:b'
check B 'scp -o ProxyCommand=x a host:b'
check B "tar -I 'sh -c sudo' -xf a.tar"
check B "tar --to-command='sudo sh' -xf a.tar"
check B 'rg --pre ./x pattern'
check B "git -c core.pager='sudo less' log"
check B "git -c alias.x='!sudo ls' x"
check B "git -c core.sshCommand='sh -c sudo' fetch"
check B "GIT_SSH_COMMAND='sudo ssh' git fetch"
check B "EDITOR='sudo vi' git commit"
check B 'foot -e rm -rf ~/.config/hypr'
check B 'taskset -c 0 rm -rf ~/Seldon'
check B 'foot -e "$CMD"'
check B 'taskset -c 0 "$CMD"'
check A 'hyprctl dispatch workspace 2'
check A 'hyprctl clients -j'
check A 'tmux ls'
check A "tmux new -d -s x 'cargo test'"
check A 'foot -e htop'
check A 'taskset -c 0 cargo test'
check A 'rsync -e ssh -a x host:y'
check A "rsync -e 'ssh -p 2222' a host:b"
check A 'scp -o BatchMode=yes a host:b'
check A 'git -c user.name=x commit -m y'
check A 'git -c color.ui=never log'
section 'The net: an unknown program whose arguments name a red-zone command'  # A13b: the net: an unknown program whose arguments name a red-zone command
check B 'myrunner sudo ls'
check B 'strace -f pacman -Syu'
check B 'catchsegv omarchy-update'
check B "xyz bash -c 'ls'"
check A 'man sudo'
check A 'which sudo'
check A 'stat /usr/bin/sudo'
check A 'herdr agent prompt engine-130 "use sudo"'
check A 'cargo test pacman'
section 'False positives of the stage-2 sweep'  # round 3: the two false positives of the sweep
check A 'eval "$(ssh-agent -s)"'
check A 'loginctl list-sessions'
check A 'loginctl show-session 2'
check B 'loginctl terminate-session 2'
check B 'loginctl kill-user tester'
section 'git: the work tree and repository as variables'  # WP-130 round 4 (stage-2 re-look), 1: the work tree and repository as variables
check B 'GIT_WORK_TREE=$HOME/.config/hypr git checkout .'
check B 'GIT_DIR=$HOME/.config/hypr/.git git checkout .'
check B 'git --git-dir=$HOME/.config/hypr/.git fetch'
check A 'GIT_DIR=/tmp/r/.git git status'
section 'Exec wrappers among the data sinks'  # 2: exec wrappers among the data sinks
check B 'fd . -x sudo ls'
check B 'fd -X sudo ls'
check B 'fd . -x {}'
check B 'rustup run stable sudo ls'
check B "man -P 'sudo ls' ls"
check B 'sort --compress-program=sudo f'
check B "wget --use-askpass='sudo ls' https://example.org/x"
check A 'man sudo'
check A 'fd pattern'
check A 'fd -e rs -x wc -l'
check A 'rustup run stable cargo build'
check A 'man -P cat ls'
check A 'sort -u f'
section 'hash -p'  # 3: hash -p maps a name like an alias
check B 'hash -p /usr/bin/sudo ls; ls'
check A 'hash -r'
section 'hyprctl and tmux string forms'  # 4: hyprctl joins its arguments; tmux sends keys one after another
check B "hyprctl dispatch 'exec sudo ls'"
check B "hyprctl 'dispatch exec sudo ls'"
check B 'tmux send-keys -t x omarchy Space update Enter'
check B 'tmux send-keys -t x sud o Space ls Enter'
check B 'tmux send-keys -H 73 75 Enter'
check B 'tmux send-keys -t x Up Enter'
check A 'tmux send-keys -t x C-c'
check A "tmux send-keys -t x 'cargo test' Enter"
section 'xargs items'  # 5: xargs: the replstr or the appended stdin items are unknown
check B 'echo sudo | xargs -I{} {} ls'
check B "echo 'sudo ls' | xargs -I{} sh -c {}"
check B 'echo sudo | xargs env'
check A 'ls | xargs -I{} echo {}'
check A 'git ls-files | xargs wc -l'
section 'Minor write paths'  # 6: minor write paths
check B 'tar -xzf a.tgz --one-top-level=$HOME/.config'
check B 'git init --separate-git-dir=$HOME/.config/x /tmp/y'
section 'Harmless git -c and GIT_CONFIG_* forms'  # false positives of the re-look: harmless git -c and GIT_CONFIG_* forms
check A 'git -c core.pager=cat log -1'
check A 'git -c core.editor=true rebase --continue'
check A 'git -c diff.noprefix=true diff'
check A 'git -c diff.colorMoved=zebra diff'
check A 'git -c merge.conflictstyle=diff3 merge x'
check A 'git -c protocol.file.allow=always clone /tmp/a /tmp/b'
check A 'git -c receive.denyCurrentBranch=updateInstead push /tmp/x HEAD'
check A 'GIT_CONFIG_GLOBAL=/dev/null git status'
check A 'GIT_CONFIG_NOSYSTEM=1 git status'
check B "GIT_CONFIG_PARAMETERS=\"'core.pager'='sudo less'\" git log"
check B 'GIT_CONFIG_COUNT=1 GIT_CONFIG_KEY_0=core.pager GIT_CONFIG_VALUE_0=sudo git log'
check B 'GIT_CONFIG_GLOBAL=/tmp/evil.cfg git log'
check B 'git -c diff.x.textconv=sudo diff'
check B 'git -c protocol.ext.allow=always fetch'
check B 'git -c filter.x.smudge=sudo checkout .'
section 'The net, widened'  # the net, widened: shells and Omarchy scripts among an unknown program's
# arguments, and arguments that start with a known file command
check B 'strace -f sed -i s/a/b/ /etc/x'
check B 'myrunner bash /usr/share/omarchy/install.sh'
check B 'myrunner bash script.sh'
check B 'myrunner /usr/share/omarchy/install.sh'
check B 'myrunner rm -rf ~/.config/hypr'
check A 'strace -f ls'
check A 'myrunner build'
check A 'shellcheck -s bash scripts/guard.sh'
echo "rows: $rows"
exit $fail
