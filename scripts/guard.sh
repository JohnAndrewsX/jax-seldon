#!/usr/bin/env bash
# Red-zone guard for agent harnesses (AGENTS.md §6).
# Claude Code PreToolUse hook for Bash: reads the hook JSON on stdin,
# exits 2 (block) when the command touches the host system.
# Runs in every permission mode, including bypassPermissions.
set -u
input=$(cat)
cmd=$(printf '%s' "$input" | jq -r '.tool_input.command // empty' 2>/dev/null)
[[ -n $cmd ]] || exit 0

# Heredoc bodies are data (the command's stdin), never commands: drop every
# line between `<<[-]['"]?WORD['"]?` and its terminator before matching, so
# file contents written through a heredoc cannot trigger the rules below.
# The heredoc operator line itself stays and is still checked.
# Exceptions: a heredoc fed to a shell, `eval`, `su`, `ssh` or `sudo` IS
# executed, so its body stays and is matched; a `<<` inside quotes (an odd
# number of quote characters before it on the line) is text, not a heredoc.
cmd=$(printf '%s\n' "$cmd" | awk '
  BEGIN { term = "" }
  term != "" { if ($0 == term || ($0 ~ /^[[:space:]]+/ && dash && $0 ~ ("^[[:space:]]*" term "$"))) { term = ""; keep = 0 } ; if (keep) print; next }
  {
    print
    if (match($0, /<<-?[[:space:]]*["'"'"']?[A-Za-z_][A-Za-z0-9_]*["'"'"']?/)) {
      pre = substr($0, 1, RSTART - 1)
      nq = gsub(/["'"'"']/, "", pre)
      if (nq % 2 == 1) next
      s = substr($0, RSTART, RLENGTH); dash = (s ~ /<<-/)
      gsub(/^<<-?[[:space:]]*["'"'"']?/, "", s); gsub(/["'"'"']$/, "", s); term = s
      keep = ($0 ~ /(^|[;&|][[:space:]]*|[[:space:]])(bash|sh|zsh|dash|ksh|eval|su|ssh|sudo)([[:space:]]|$)/)
    }
  }')

block() { echo "guard: blocked (AGENTS.md §6 red zone): $1" >&2; exit 2; }

# privilege and package management
# only as the first word of a command segment (after ; & | && || or at the
# start), optionally behind `env`/`command`/`nice`/`time` — not as a word inside
# heredoc text, comments or file contents
if printf '%s' "$cmd" | grep -Eq '(^|[;&|][[:space:]]*|\$\([[:space:]]*|`[[:space:]]*)((env|command|nice|time|exec)[[:space:]]+|[A-Za-z_][A-Za-z0-9_]*=[^[:space:]]*[[:space:]]+)*(sudo|doas|su|pkexec|pacman|yay|paru|makepkg|pacstrap)([[:space:]]|$)'; then
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
# exception: `ssh <test-host> ... omarchy theme set ...` — the theme sweep on
# the test host is allowed (docs/HERDR-SETUP.md §5); everything else stays
if printf '%s' "$cmd" | grep -Eq '(^|[;&|[:space:]])omarchy([[:space:]]+(pkg[[:space:]]+(add|aur|drop|install|remove)|update|install|theme[[:space:]]+set|plugin[[:space:]]+(add|remove|update|clone)|snapshot|migrate|refresh|hook[[:space:]]+install|dev[[:space:]]+link))' \
   && ! printf '%s' "$cmd" | grep -Eq '^[[:space:]]*ssh[[:space:]][^;&|]*omarchy[[:space:]]+theme[[:space:]]+(set|current)([^;&|]*)$'; then
  block "omarchy command that changes the system"
fi
# writes under /etc or /usr
# redirections and tee: the system path right after the operator; file
# commands: only when the system path is the LAST argument of the segment
# (the destination) — a /usr or /var path used as a read-only source is fine
if printf '%s' "$cmd" | grep -Eq '(>|>>|tee([[:space:]]+-[a-z]+)*)[[:space:]]*/(etc|usr|boot|var)/' \
   || printf '%s' "$cmd" | grep -Eq '(^|[;&|][[:space:]]*)(ssh[[:space:]]+[^[:space:]]+[[:space:]]+["'\'']?)?(sudo[[:space:]]+)?(cp|mv|install|rm|rmdir|sed[[:space:]]+-i[^[:space:]]*|chmod|chown|ln|mkdir|touch|truncate)([[:space:]]+[^|;&[:space:]]+)*[[:space:]]+/(etc|usr|boot|var)/[^|;&[:space:]]*[[:space:]]*($|[|;&])'; then
  block "write under /etc, /usr, /boot or /var"
fi
# writes under ~/.config outside the plugin dev install
# same shape as the system-path rule: redirections/tee anywhere, file commands
# only at a command position — not inside heredoc bodies or quoted test data
if printf '%s' "$cmd" | grep -Eq '(>|>>|tee([[:space:]]+-[a-z]+)*)[[:space:]]*(~|\$HOME|/home/[^/]+)/\.config/' \
   || printf '%s' "$cmd" | grep -Eq '(^|[;&|][[:space:]]*)(ssh[[:space:]]+[^[:space:]]+[[:space:]]+["'\'']?)?(sudo[[:space:]]+)?(cp|mv|install|rm|rmdir|sed[[:space:]]+-i[^[:space:]]*|chmod|chown|ln|mkdir|touch|truncate)([[:space:]]+[^|;&[:space:]]+)*[[:space:]]+(~|\$HOME|/home/[^/]+)/\.config/'; then
  # exceptions: the plugin dev install, and — over ssh to the test host only —
  # Seldon's own config dir there (smoke-test restore, docs/HERDR-SETUP.md §5)
  if ! printf '%s' "$cmd" | grep -Eq '\.config/omarchy/plugins/jax\.seldon' \
     && ! printf '%s' "$cmd" | grep -Eq '^[[:space:]]*ssh[[:space:]][^;&|]*\.config/seldon(/[^[:space:]]*)?([[:space:]]|$|["'"'"'])'; then
    block "write under ~/.config outside the jax.seldon plugin dir"
  fi
fi
exit 0
