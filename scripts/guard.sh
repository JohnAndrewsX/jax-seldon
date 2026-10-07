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
#   `flock`, `xargs`, `find -exec`, `watch`, `taskset`, `chrt`,
#   `systemd-inhibit`, `ssh-agent`, `dbus-run-session`, `uwsm app`,
#   terminals with `-e`, `VAR=…` prefixes), in functions at their call,
#   and in strings that are code: `bash -c`/`sh -c` bodies, `eval`, `trap`,
#   `script -c`, `hyprctl dispatch exec`, `tmux new|run-shell|send-keys`
#   strings, heredocs and here-strings fed to a shell, `script` or `ssh`,
#   and the remote command of `ssh` (checked by the same rules; to this
#   machine — localhost, its own name, a computed host — by the local
#   rules).
# - An unknown program whose arguments name a red-zone command (`strace
#   pacman …`) or `<shell> -c` fails closed, unless it only reads its
#   arguments as data (`man sudo`, `which sudo`).
# - Everything else is data: quoted strings, heredoc bodies written to a
#   file or fed to any other program, `echo`/`printf`/`grep`/`jq`/`git
#   commit -m`/`herdr agent prompt` arguments.
# - Blocked at a command position: sudo/doas/su/pkexec/run0; pacman except
#   `-Q…` and `-S` with `-p`/`--print` (no -y/-u/-c/-w); yay, paru, makepkg,
#   pacstrap; systemctl except status/show/cat/is-*/list-*; loginctl,
#   reboot, shutdown, mkinitcpio, grub-*, systemd-run (read-only loginctl
#   and systemctl verbs pass); Omarchy commands
#   that change the system (pkg, update, install, theme set, plugin
#   add/remove/update/clone/enable/disable, snapshot, migrate, refresh,
#   hook, dev link, branch, channel set; also as omarchy-* binaries) or
#   launch agents/apps (`omarchy … --help` only prints help); Omarchy's own
#   scripts (`$OMARCHY_PATH/install.sh`, `migrations/…`; `bash -n` passes);
#   writes (redirections, tee, cp/mv/ln/install, rm, mkdir, touch, chmod,
#   sed -i, dd of=, rsync/scp destinations, find -delete, patch, tar -x,
#   unzip, curl -o/-O, wget, git clone/init/worktree add and git commands
#   that change a work tree) under /etc /usr /boot /var, under ~/.config
#   outside ~/.config/omarchy/plugins/jax.seldon, and under the real
#   ~/Seldon and ~/.local/state/seldon. `~`, `$HOME`, `cd` (a cd that may
#   fail keeps the old directory as a candidate, `cd -`, `$OLDPWD`) and
#   links made earlier in the command are resolved; a scratch HOME set
#   earlier in the command (`export HOME=/tmp/x; mkdir ~/.config/…`) is
#   not the real home.
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
#   redirection are still checked here. `GUARD_HOSTS_FILE` replaces the
#   file only when `SELDON_TEST_GUARD` is set (the test table), so a
#   settings `env` block cannot widen the list.
# - The two makepkg forms packaging/README.md uses on the test host over
#   ssh pass as exact strings (ORCHESTRATION.md §11, WP-040 review).
# - Fail closed: input it cannot parse, a computed command name, a shell
#   reading commands from a pipe, `env -S`, a changed IFS, an alias, an
#   exec option the guard cannot follow (`git -c core.pager=…`, `tar -I`,
#   `rg --pre`, `gdb -ex`, `bwrap`, `parallel`) or an internal error is
#   blocked with the reason. So is work the hook's 5 s timeout could cut off (a
#   timed-out hook does not block): hook input over 256 KB, more than 256
#   variables in one command, more than 3 s of checking.
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
