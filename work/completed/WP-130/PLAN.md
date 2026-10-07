# WP-130 — plan

## Problem

`scripts/guard.sh` greps the whole command text. Words after any `;&|`,
quote or space count as commands, so `grep -n 'x\|pacman -S' f`, an `echo`,
a `git commit -m` text or a `jq` filter is blocked. The same line-based
grep also lets real commands through: `grep -Eq '^…$'` succeeds when *any
line* matches. A two-line command whose first line is a listed-host ssh
(or the makepkg whitelist) therefore passes as a whole.

## Approach

`scripts/guard.sh` stays the hook entry (wiring in `.claude/settings.json`
unchanged) and runs `scripts/guard.py` (Python 3 stdlib only, like
`docs-check.py`). If `python3` is missing, guard.sh fails closed.

guard.py does three things:

1. **Lexer + parser for the bash subset agents write.** Quotes (`'…'`,
   `"…"`, `$'…'`), backslashes, comments, `;` `&` `&&` `||` `|` `|&`
   newline, redirections (`>` `>>` `>|` `&>` `<>` `N>&M` `<<` `<<-` `<<<`),
   heredoc bodies (quoted delimiter = pure data; unquoted = data whose
   `$(…)`/backticks still run), `$(…)`, backticks, `<(…)`/`>(…)`,
   `${…}`, `$((…))`, `((…))`, `[[ … ]]`, `( … )`, `{ …; }`, `if`/`while`/
   `until`/`for`/`select`/`case`, functions, `!`, `time`. The result is a
   tree of simple commands. Anything outside this subset, or input that does
   not parse (unterminated quote or heredoc, unbalanced parens), is
   **blocked with the reason**.
2. **Evaluator.** Walks the tree in order. It keeps a scope of literal
   variable values from assignments in the same command (`X=…`, `export`,
   `for`, `cd` as the working directory), with candidates for conditional
   assignments. Unknown values stay a marker, never a guess. Every nested
   command runs through the same checks: `$(…)` anywhere (also inside
   `"…"`), process substitutions, heredoc bodies fed to a shell or to
   `ssh`, `bash`/`sh`/`zsh -c` bodies, `eval` and `trap` strings, the
   remote command of `ssh` (remote context). Wrappers are unwrapped down
   to the real command: `env`, `command`, `exec`, `nice`, `nohup`, `time`,
   `timeout`, `stdbuf`, `setsid`, `ionice`, `flock`, `xargs`,
   `find -exec`, `watch`, and `VAR=…` prefixes.
3. **Rules on the command position only.** Privilege (`sudo`, `doas`,
   `su`, `pkexec`, `run0`); package tools (`pacman` except the read-only
   forms the WP lists, `yay`, `paru`, `makepkg`, `pacstrap`); services and
   boot (`systemctl` except read-only verbs, `loginctl`, `reboot`, …);
   Omarchy commands that change the system or launch agents/apps (same set
   as today, `--help`/`-h` excepted, which the dispatcher answers without
   running anything); writes (redirections, `tee`, `cp`/`mv`/`install`/
   `ln` destinations, `rm`/`mkdir`/`touch`/`chmod`/… targets, `sed -i`
   files, `dd of=`, `rsync`/`scp` local destinations, `find -delete`)
   under `/etc /usr /boot /var`, under the real `~/.config` outside
   `~/.config/omarchy/plugins/jax.seldon`, and under the real `~/Seldon`
   and `~/.local/state/seldon`. Paths are resolved (`~`, `$HOME`, `cd`,
   the hook's `cwd`, `..`) before they are compared.

Test hosts and the makepkg whitelist keep their regexes, matched against
the **whole** command (single line), plus a structural check: no local
`$(…)`, backticks or process substitution, and local redirections are
still checked.

## Tests

`scripts/guard-test.sh` keeps every row and grows by every row the WP
lists (allow and block). `scripts/guard-mutants.sh` copies the guard,
applies a list of mutations (drop the `bash -c` recursion, drop `$(…)`
recursion, …) and requires each mutant to fail at least one row. Both run
in a new `just check-guard`, part of `just check`.
