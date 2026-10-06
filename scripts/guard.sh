#!/usr/bin/env bash
# Red-zone guard for agent harnesses (AGENTS.md §6).
# Claude Code PreToolUse hook for Bash: reads the hook JSON on stdin,
# exits 2 (block) when the command touches the host system.
# Runs in every permission mode, including bypassPermissions.
#
# The rule (WP-130): the guard parses the command like bash and decides on
# the command position of every simple command, not on words in the text.
# - Commands are found after `;` `&` `&&` `||` `|` and newlines, inside
#   `$(…)`, backticks, `<(…)`, compound commands and functions, behind
#   wrappers (`env`, `command`, `exec`, `nice`, `nohup`, `time`, `timeout`,
#   `flock`, `xargs`, `find -exec`, `watch`, `VAR=…` prefixes), and in
#   strings that are code: `bash -c`/`sh -c` bodies, `eval` and `trap`
#   strings, heredocs and here-strings fed to a shell or to `ssh`, and the
#   remote command of `ssh` (checked by the same rules).
# - Everything else is data: quoted strings, heredoc bodies written to a
#   file or fed to any other program, `echo`/`printf`/`grep`/`jq`/`git
#   commit -m`/`herdr agent prompt` arguments.
# - Blocked at a command position: sudo/doas/su/pkexec/run0; pacman except
#   `-Q…` and `-S` with `-p`/`--print` (no -y/-u/-c/-w); yay, paru, makepkg,
#   pacstrap; systemctl except status/show/cat/is-*/list-*; loginctl,
#   reboot, shutdown, mkinitcpio, grub-*, systemd-run; Omarchy commands
#   that change the system or launch agents/apps (`omarchy … --help` only
#   prints help); writes (redirections, tee, cp/mv/ln/install, rm, mkdir,
#   touch, chmod, sed -i, dd of=, rsync/scp destinations, find -delete)
#   under /etc /usr /boot /var, under ~/.config outside
#   ~/.config/omarchy/plugins/jax.seldon, and under the real ~/Seldon and
#   ~/.local/state/seldon. `~`, `$HOME` and `cd` are resolved; a scratch
#   HOME set earlier in the command (`export HOME=/tmp/x; mkdir
#   ~/.config/…`) is not the real home.
# - Over ssh, the remote command runs on another machine: agent and app
#   launchers pass; `omarchy theme set` passes when the ssh call is the
#   whole command; writes under ~/.config/seldon pass when the ssh call is
#   the first command and the write the first remote command.
# - Test hosts: a command that is ONE ssh invocation to a host listed in
#   scripts/guard-hosts.local (git-ignored: real names stay out of the
#   repo; one host per line, `#` comments) runs on that host, which the
#   operator released to the agents (operator decision 2026-10-05). Nothing
#   may run locally: one line, the remote command one quoted string or
#   plain words without `;`, `&`, `|`; a local `$(…)`, backticks or a
#   redirection are still checked here. `GUARD_HOSTS_FILE` overrides the
#   file (the test table uses it).
# - The two makepkg forms packaging/README.md uses on the test host over
#   ssh pass as exact strings (ORCHESTRATION.md §11, WP-040 review).
# - Fail closed: input it cannot parse, a computed command name, a shell
#   reading commands from a pipe, `env -S` or an internal error is blocked
#   with the reason.
# The parser and the rules live in guard.py (Python 3 standard library);
# the expectation table is scripts/guard-test.sh.
set -u
here=$(dirname "$0")
if ! command -v python3 >/dev/null 2>&1; then
  echo "guard: blocked (fail closed, cannot check this command): python3 is missing" >&2
  exit 2
fi
python3 -I "$here/guard.py"
rc=$?
[ "$rc" -eq 0 ] && exit 0
[ "$rc" -eq 2 ] || echo "guard: blocked (fail closed, cannot check this command): guard.py exited $rc" >&2
exit 2
