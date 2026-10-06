#!/usr/bin/env bash
# Expectation table for scripts/guard.sh: B = must block (exit 2), A = must allow (exit 0)
# The guard only reads the command text; every row runs with a fixed fake
# HOME and working directory, so nothing depends on this machine.
#   GUARD_SH=<path>        test another copy of guard.sh (scripts/guard-mutants.sh)
#   GUARD_TEST_FAILFAST=1  stop at the first failing row
#   GUARD_TEST_QUIET=1     print only failing rows and the summary
G=${GUARD_SH:-"$(dirname "$0")/guard.sh"}
export HOME=/home/tester OMARCHY_PATH=/usr/share/omarchy
unset TMPDIR
cwd=/home/tester/repo
fail=0 rows=0
check() {
  local want=$1 cmd=$2
  rows=$((rows + 1))
  printf '{"tool_input":{"command":%s},"cwd":"%s"}' "$(printf '%s' "$cmd" | jq -Rs .)" "$cwd" | bash "$G" >/dev/null 2>&1
  local rc=$?
  local got=?; [ $rc -eq 0 ] && got=A; [ $rc -eq 2 ] && got=B
  if [ "$got" != "$want" ]; then
    echo "FAIL want=$want got=$got :: $cmd"; fail=1
    [ -n "${GUARD_TEST_FAILFAST:-}" ] && exit 1
  elif [ -z "${GUARD_TEST_QUIET:-}" ]; then
    echo "ok   $want :: $cmd"
  fi
  return 0
}
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
# WP-130: read-only package queries
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
# WP-130: the words only as text (echo/printf, heredoc to a file, grep/rg
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
# WP-130: Omarchy command names as text; help and the command list
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
# WP-130: a scratch HOME (a prefix assignment does not change the expansion
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
# WP-130: sudo setfacl in a test file's text
check A 'cat >> engine/tests/snapper.rs <<'"'"'EOF'"'"'
// the operator grants: sudo setfacl -m u:me:rx /.snapshots
EOF'
check A 'printf "%s\n" "sudo setfacl -m u:me:rx /.snapshots" > fixtures/logs/grant.txt'
check B 'sudo setfacl -m u:me:rx /.snapshots'
# WP-130: read-only greps over the Omarchy tree
check A 'grep -rn "pacman" "$OMARCHY_PATH/migrations"'
check A 'grep -n "sudo\|systemctl enable" $OMARCHY_PATH/install.sh'
check A 'rg -l "omarchy update" /usr/share/omarchy/migrations | head'
# WP-130: the command position, wherever it is
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
# WP-130: services
check B 'systemctl start foo'
check B 'systemctl stop foo'
check B 'systemctl --user start seldon-watch'
check B 'systemctl --user stop seldon-watch'
check B 'systemctl --user enable --now seldon-watch'
check A 'systemctl --user is-active seldon-watch'
check A 'systemctl list-timers'
# WP-130: writes under the real ~/.config, ~/Seldon, ~/.local/state/seldon
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
# WP-130: fail closed
check B 'echo "unterminated'
check B 'cat <<EOF
no end'
check B '$CMD -Syu'
check B 'eval "$X"'
check B 'env -S "sudo ls"'
check B 'echo ok; )'
# WP-130: ssh to a host that is not a listed test host runs the same rules
check B 'ssh -o ProxyCommand="sudo nc %h %p" test-host ls'
check B 'ssh other-host sudo pacman -Syu'
check B 'printf "sudo ls\n" | ssh other-host'
check B "ssh other-host 'rm -rf ~/.config/seldon && rm -rf ~/.config/hypr'"
check B "ssh other-host 'ls; rm -rf ~/.config/seldon'"
check A "ssh other-host 'systemctl --user status seldon-watch'"
check B "ssh testhost 'cd /tmp/seldon-pkg && makepkg -f'
sudo pacman -Syu"
# test hosts (scripts/guard-hosts.local; the table uses its own file)
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
# WP-130: holes of the line-based grep guard; nothing may run locally
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
echo "rows: $rows"
exit $fail
