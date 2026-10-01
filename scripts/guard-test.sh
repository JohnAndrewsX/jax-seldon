#!/usr/bin/env bash
# Expectation table for scripts/guard.sh: B = must block (exit 2), A = must allow (exit 0)
G="$(dirname "$0")/guard.sh"
fail=0
check() {
  local want=$1 cmd=$2
  printf '{"tool_input":{"command":%s}}' "$(printf '%s' "$cmd" | jq -Rs .)" | bash "$G" >/dev/null 2>&1
  local rc=$?
  local got=A; [ $rc -eq 2 ] && got=B
  if [ "$got" != "$want" ]; then echo "FAIL want=$want got=$got :: $cmd"; fail=1; else echo "ok   $want :: $cmd"; fi
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
exit $fail
