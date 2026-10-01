#!/usr/bin/env bash
# Red-zone guard for agent harnesses (AGENTS.md §6).
# Claude Code PreToolUse hook for Bash: reads the hook JSON on stdin,
# exits 2 (block) when the command touches the host system.
# Runs in every permission mode, including bypassPermissions.
set -u
input=$(cat)
cmd=$(printf '%s' "$input" | jq -r '.tool_input.command // empty' 2>/dev/null)
[[ -n $cmd ]] || exit 0

block() { echo "guard: blocked (AGENTS.md §6 red zone): $1" >&2; exit 2; }

# privilege and package management
if printf '%s' "$cmd" | grep -Eq '(^|[;&|[:space:]])(sudo|doas|su|pkexec|pacman|yay|paru|makepkg|pacstrap)([[:space:]]|$)'; then
  block "privileged or package command"
fi
# services and boot
if printf '%s' "$cmd" | grep -Eq '(^|[;&|[:space:]])(systemctl|loginctl|reboot|shutdown|poweroff|mkinitcpio|grub-)'; then
  # allow read-only systemctl queries
  if ! printf '%s' "$cmd" | grep -Eq 'systemctl[[:space:]]+(--user[[:space:]]+)?(status|list-|show|is-|cat)'; then
    block "service or boot command"
  fi
fi
# omarchy commands that change the system (observing is fine)
if printf '%s' "$cmd" | grep -Eq '(^|[;&|[:space:]])omarchy([[:space:]]+(pkg[[:space:]]+(add|aur|drop|install|remove)|update|install|theme[[:space:]]+set|plugin[[:space:]]+(add|remove|update|clone)|snapshot|migrate|refresh|hook[[:space:]]+install|dev[[:space:]]+link))'; then
  block "omarchy command that changes the system"
fi
# writes under /etc or /usr
if printf '%s' "$cmd" | grep -Eq '(>|>>|tee|cp|mv|install|rm|sed[[:space:]]+-i|chmod|chown|ln)[^|;&]*[[:space:]]/(etc|usr|boot|var)/'; then
  block "write under /etc, /usr, /boot or /var"
fi
# writes under ~/.config outside the plugin dev install
if printf '%s' "$cmd" | grep -Eq '(>|>>|tee|cp|mv|install|rm|sed[[:space:]]+-i|ln|mkdir)[^|;&]*[[:space:]](~|\$HOME|/home/[^/]+)/\.config/'; then
  if ! printf '%s' "$cmd" | grep -Eq '\.config/omarchy/plugins/jax\.seldon'; then
    block "write under ~/.config outside the jax.seldon plugin dir"
  fi
fi
exit 0
