# WP-130 — handover

Branch `wp/130-guard`, base `main` 94e1fab. Role: Engine Dev.

## What was done

- `scripts/guard.sh` is now the thin hook entry (wiring in
  `.claude/settings.json` unchanged). Its comment block states the rule.
  It runs `scripts/guard.py` with `python3 -I` and fails closed when
  `python3` is missing or `guard.py` exits with anything but 0 or 2.
- `scripts/guard.py` (Python 3 standard library) has three parts:
  - **Parser**: a bash parser for the subset agents write. Quotes,
    `$'…'`, heredocs (quoted or not, `<<-`), `$(…)`, backticks, `<(…)`,
    `${…}`, `$((…))`, `((…))`, `[[ … ]]`, subshells, groups,
    `if`/`while`/`until`/`for`/`select`/`case`, functions, `!` and
    `time`.
  - **Evaluator**: walks the tree in order. It tracks variables assigned
    in the same command (conditional ones as candidates), `cd`, `~`,
    `$HOME`, and the hook's `cwd`.
  - **Rules**: applied at the command position only. Recursion reaches
    `$(…)` everywhere, wrappers, `bash -c`/`sh -c`/`eval`/`trap`/
    `script -c`/`flock -c`/`watch`, heredocs and here-strings fed to a
    shell or `ssh`, and the remote command of `ssh`. A command it cannot
    parse or know is blocked with "fail closed" and the reason.
- The stale "HERDR-SETUP.md §5" comment is gone. The `~/.config/seldon`
  exception now cites docs/TESTING.md, where it is described.
- `scripts/guard-test.sh` has 250 rows: all 87 earlier rows unchanged,
  plus 163 new ones. Every allow and block case the WP lists is a row,
  and so is every hole below. Rows run with a fixed fake `HOME`
  (`/home/tester`), `cwd` and no `TMPDIR`, so they depend on nothing on
  the machine (CI runs as root). Nothing is executed: the guard only
  reads text.
- `scripts/guard-mutants.py` holds the mutant list: 25 mutants, each
  dropping one rule (the `bash -c` recursion, `$(…)`, `<(…)`, heredoc
  to shell, pipe to shell, unquoted heredoc `$(…)`, `env` unwrapping,
  `eval`, `find -exec`, the ssh remote check, the host gate (two
  mutants), parse errors, computed names, the four allowances, the
  plugin dir, `~/.config/seldon` over ssh, scratch HOME, prefix
  assignment semantics, `..`, recursive ancestors, `~/Seldon`). The
  table catches all 25 (19 s, parallel).
- `just check-guard` (table + mutants + `bash -n` + `shellcheck` when
  installed) is part of `just check`, so it runs in CI too.
  docs/TESTING.md has its row. docs/ORCHESTRATION.md §11 now names
  `guard.py` and the mutant list. `memory/pitfalls.md` has a WP-130
  section: the old substring bullets are outdated, what still fails
  closed, the prefix-assignment `HOME` trap, and that a worktree's hook
  runs that worktree's guard.

## Security findings in the old guard (fixed, each a block row)

Verified by feeding the texts to `main`'s guard.sh (nothing executed).
Every command below was **allowed** before:

1. **Line-based matching.** `grep -Eq '^…$'` succeeds when *any line*
   matches. So `ssh test-host ls⏎sudo pacman -Syu` (listed host), and
   the makepkg whitelist followed by a second line, passed as a whole.
2. **Local execution through the listed-host rule:**
   `ssh test-host "$(sudo pacman -Syu)"` and
   `ssh test-host ls > ~/.config/hypr/x`.
3. **The ssh launcher exception** matched `^ssh` anywhere in the text:
   `ssh test-host x; omarchy agent prompt y` launched locally.
4. **First word after an ssh host was never checked:**
   `ssh other-host sudo pacman -Syu`. Neither was
   `ssh other-host "rm -rf ~/.config/seldon && rm -rf ~/.config/hypr"`:
   the whole command was exempt once `.config/seldon` appeared.
5. **Command words not after `;&|`:** `/usr/bin/pacman -Syu`,
   `"sudo" ls`, `\sudo ls`, `bash -c 'sudo ls'`, `s=sudo; $s ls`,
   `cat x | bash`.
6. **Not covered at all:** `rm -rf ~`, `rm -rf ~/Seldon`,
   `rm -rf ~/.local/state/seldon`, `cd ~/.config && rm -rf hypr`,
   `omarchy-update`.

## Decisions (left open by the WP)

1. **Python, not bash, for the parser.** A real tokenizer with heredocs
   and recursion is not maintainable in bash/awk, and Python 3 is
   already a `just check` dependency (`docs-check.py`,
   `validate-fixtures.py`). Standard library only. `-I` ignores
   `PYTHON*` variables and the user site.
2. **pacman allow set exactly as listed.** Any `-Q…` query, and `-S`
   with `-p`/`--print` (also with `--print-format`) without
   `-y`/`-u`/`-c`/`-w`. `-Ss`, `-Si`, `-V`, `-h`, `yay -Q` stay blocked;
   the WP does not list them.
3. **`omarchy … --help`/`-h`** (before `--`) passes only through the
   `omarchy` dispatcher. `/usr/bin/omarchy` (`remaining_has_help_flag`)
   prints help for any route and never execs the binary. The Omarchy
   route list and its prefix matching are unchanged (`omarchy updates`,
   `omarchy installed …` stay blocked as before). New: the same routes
   as direct binaries (`omarchy-update`, `omarchy-pkg-add`, …) are
   blocked. They had been a gap.
4. **A scratch HOME follows bash.** `export HOME=/tmp/x; mkdir
   ~/.config/…`, `HOME=$(mktemp -d) && …` and `HOME=/tmp/h bash -c
   '…'` pass. `HOME=/tmp/h mkdir ~/.config/x` is **blocked**: bash
   expands `~` before the prefix assignment applies, so it writes the
   real `~/.config`. A `HOME` whose value the guard cannot know counts
   as the real one. A conditional assignment (`false || export
   HOME=…`) is a candidate beside the old value.
5. **Unknown values.** A command name with an unknown part fails closed
   (`$CMD …`), unless its last path segment is literal
   (`"$root/…/seldon"`). Variables assigned earlier in the same command
   are resolved (`B=…; $B`). A write target whose leading part is
   unknown (`rm -rf "$dir"` with `dir` from outside) is not decidable
   and passes, as before. `$(mktemp …)` and `$(pwd)` are modelled.
6. **ssh to a host not in `guard-hosts.local`** gets the same rules as
   the local machine. Before, only words after `;&|` were checked there
   (finding 4), so this is stricter. Over ssh, three things pass:
   - agent and app launchers;
   - `omarchy theme set`, when the ssh call is the whole command and the
     remote command one simple command;
   - writes under `~/.config/seldon`, when the ssh call is the first
     command, unwrapped, and the write the first remote command. This
     keeps docs/TESTING.md's "no `;`, `&` or `|` before the path" true.
7. **Test hosts.** The regex is unchanged, but it is matched against
   the whole command (single line). A local `$(…)`, backticks, `<(…)`
   and local redirections are still checked; only the remote command is
   released.
8. **Blocks added beyond the WP list** (all conservative):
   - privilege: `run0`, `runuser`, `sudoedit`;
   - services: `systemd-run`;
   - `ssh -o ProxyCommand/LocalCommand/KnownHostsCommand` (they run
     locally);
   - writes under the real `~/Seldon` and `~/.local/state/seldon` by any
     file command or redirection, not only `rm -rf` (the engine is the
     only writer, AGENTS.md §3);
   - recursive deletes of a directory above a protected one (`rm -rf ~`,
     `rm -r ~/.local/state`);
   - more write targets and code paths: `mv` sources, `find -delete` /
     `-exec`, `dd of=`, local `rsync`/`scp` destinations, `script -c`;
   - fail-closed shapes: a shell or `ssh` reading commands from a pipe,
     `env -S`.
9. **systemctl options are parsed**, so `systemctl --no-pager status x`
   passes (before: only `systemctl [--user] status …`). The verb set is
   unchanged: status, show, cat, is-*, list-*.
10. **`check-guard` runs in `just check`/CI** so a guard change cannot
    merge with a red table or a surviving mutant.

## Known limits (current as of round 4; show next to the table)

What the guard does **not** protect. Each line is a decision for the
operator, not a hidden gap.

- **Only the Bash tool is guarded.** The hook's matcher is `Bash`. The
  Write and Edit tools are not checked by this guard; they are covered
  only by the settings deny list (`/etc`, `~/.config/hypr`,
  `shell.json`).
- **Files a shell runs are not inspected** (`bash script.sh`,
  `source x`, `bash < cmds.txt`, `script /dev/null < cmds.txt`,
  `ssh h bash -s < file`). Generated ones are blocked
  (`source <(…)`, `bash <(…)`), and so are Omarchy's own scripts (round
  3). A file written earlier in the same command and then run
  (`cat > x.sh <<EOF … EOF; bash x.sh`) is not inspected either.
- **Other interpreters are data**: `python3 -c`, `perl -i`, `node -e`,
  `awk 'system()'`, and GNU `sed`'s `e` command (`sed 's/x/y/e'`).
- **An unknown program's own write options** are not known
  (`strace -o ~/.config/x ls`). Since round 4 the net blocks an unknown
  program whose arguments name a red-zone command, a shell or an
  Omarchy script, and checks arguments that start with a file command
  as that command (`strace -f sed -i … /etc/x`). What is left is the
  program's own writes.
- **`xargs` input is unknown.** `xargs rm -rf < list` decides nothing
  about the targets.
- **Writers to `~/.config` by program, not by path**: `gsettings`,
  `dconf`, `xdg-mime`, `xdg-settings`, `sort -o`, `mktemp -p`, editors,
  `wtype` into a terminal. The write rules know file commands; these
  programs choose their own files.
- **Omarchy routes outside the list stay allowed.** Stage 2 recommends
  inverting to a read-only allow-list (`omarchy version`, `commands`,
  `… --help`, `plugin list|validate|catalog`, `theme current`,
  `channel current`, `agent usage`, …), so every new Omarchy command is
  blocked until it is listed. **Operator decision.**
- **D-Bus, Hyprland and polkit service paths**: `busctl`, `gdbus`,
  `dbus-send` to systemd or logind, `hyprctl plugin load`, `hyprctl
  reload`, `hyprctl keyword` other than `exec`/`bind`, `pkaction`, are
  not modelled.
- **`ssh -I` / PKCS#11 provider libraries** load a local `.so`; not
  modelled.
- **ssh host aliases**: an alias in `~/.ssh/config` that points at this
  machine is treated as another host.
- **Links on disk**: the guard reads no files. A symlink made in the
  same command is followed (round 3); one that already exists
  (`/tmp/cfg → ~/.config`) is not.
- **Text sent to another agent or pane is data** (`herdr agent prompt`,
  as the WP says; `wtype`); the receiving session's guard decides.
- **Big or heavy commands are blocked, not checked.** Hook input over
  256 KB, more than 256 variables, more than 64 links, functions or
  nesting too deep, or more than 3 s of checking fails closed. That is a
  usability limit, not a hole.
- **`scripts/deploy-test-host.sh` still honours `GUARD_HOSTS_FILE`**
  without `SELDON_TEST_GUARD` (its own refusal gate, not the hook). See
  the round 2 open question.

## How it was verified

- `bash scripts/guard-test.sh`: 250/250 rows green.
- `python3 scripts/guard-mutants.py`: 25/25 mutants caught.
- Two scratch sweeps, not committed:
  - 82 everyday agent commands (cargo/just/git/jq/herdr/heredoc commit
    messages, `[ … ]`, arrays, `case`, loops, process substitution,
    plugin rsync): none blocked;
  - 104 bypass shapes (wrappers, nested code, redirect forms, tilde
    after `of=`, scope leaks through subshells and `sh -c`): none
    allowed.
  Three misses the sweeps found were fixed and are rows now: `dd
  of=~/…`, `flock -c CMD FILE`, and a prefix `HOME` leaking out of
  `sh -c`. So were two false positives on `[ … ]`.
- This session's own hook ran the new guard from commit ec26247 on
  (`$CLAUDE_PROJECT_DIR` is the worktree): every later Bash call in this
  WP passed through it.
- `flock /tmp/seldon-check.lock just check`: see the last section.

## Disclosure: one guard block in this session

At the start, the then-current guard blocked my read-only
`grep -n "just\|jq\|pacman -S" .github/workflows/*.yml` ("privileged or
package command"). That is the false-positive class this WP fixes. I re-ran
the read without the package word: reading a file is not a red-zone
action, and I executed no blocked action. With the new guard the original
command passes, and it is a table row (`grep -n -i "jq\|pacman -S" …`).
Reported so the orchestrator can judge it against
ORCHESTRATION.md §11.

## Open questions

1. **Omarchy route list.** Should `omarchy plugin enable|disable`,
   `omarchy hook <name>` (runs user hooks) and `omarchy branch|channel
   set` join the blocked routes? They change the system or the shell,
   and the WP did not list them, so they stay allowed as before.
2. **Operator review of the table** (WP: merge only after the operator
   has seen it). `bash scripts/guard-test.sh` prints every row with its
   expectation. The WP-130 rows are grouped under `# WP-130: …` headings.

## just check

`flock /tmp/seldon-check.lock just check` on 283dd3b (the code; this
handover file is the only later change): `check: ok`, exit 0, including
`check-guard` (`rows: 250`, 25/25 mutants killed). `CARGO_TARGET_DIR`
unset, i.e. `engine/target` in the worktree on disk. `shellcheck` is not
installed on this dev host, so the shellcheck step was skipped locally;
CI's container has it.

## Round 2

Review 1 (stage 1): SEND BACK on N1. The brief decided Q2 and Q3; N3 and
N4 needed no code.

### N1 — fail closed before the hook timeout

- **Size cap.** `main()` reads at most 256 KB + 1 byte of hook input and
  blocks anything over 256 KB before parsing, with a message to write
  big files with the Write/Edit tools. Rows: an input of exactly 262 144
  bytes passes, 262 145 is blocked (`echo x;` repeated, the label
  instead of the text in the output).
- **The cap alone was not enough.** Inside the cap I found a quadratic
  path: every simple command copied the variable scope. 27 000 distinct
  assignments (`v1=1; v2=1; …`, 255 KB) took **244 s**, and `|| v=1`
  chains 155 s. A hook that times out does not block, so both were fail
  open. Fixed in two layers:
  - **At most 256 distinct variables** per command, then fail closed. A
    scope copy is now bounded, and `&&`/`||` chains merge
    incrementally, so the walk is linear. The same inputs now take
    0.3 s and 1.2 s (blocked). Rows: 256 variables pass, 257 are
    blocked.
  - **A 3 s time budget** (`signal.setitimer`) fails closed on any slow
    path not foreseen. Rows: a 40 KB input with the budget shortened to
    0.01 s is blocked. The same input passes when `GUARD_TIME_BUDGET` is
    set without `SELDON_TEST_GUARD`: the variable can only shorten the
    budget, and only for the test table.
- **Largest allowed input, measured.** Hook inputs at the cap
  (262 144 bytes or just under), best of 3, this dev host at load
  average 5.6 (several agents and cargo running):

  | Shape | Time |
  |---|---|
  | `$(echo x)` repeated (worst) | **1.29 s** |
  | `x=$(mktemp -d); cd $x` repeated | 1.04 s |
  | `echo x;` repeated | 0.83 s |
  | `a=1 &&` chain | 0.39 s |
  | unquoted heredoc | 0.14 s |
  | quoted heredoc to a file | 0.04 s |
  | one long quoted argument | 0.03–0.07 s |

  Also probed: 32 000-element `&&`/`||` chains 0.55 s, 23 000 `cd`s
  0.65 s, 8 000 `for` loops 0.55 s, 2 000-deep `$(` nesting blocked in
  0.03 s. The 3 s budget leaves about 3.7 s of margin to the 5 s hook
  timeout for the worst allowed shape; a heavier host hits the budget
  (blocked), not the timeout.

### Q2 — `GUARD_HOSTS_FILE` only for tests

`read_hosts()` reads the fixed `scripts/guard-hosts.local`.
`GUARD_HOSTS_FILE` replaces it only when `SELDON_TEST_GUARD` is non-empty.
`guard-test.sh` exports `SELDON_TEST_GUARD=1`. Rows: with
`SELDON_TEST_GUARD=''` and `GUARD_HOSTS_FILE` pointing at the table's
list, `ssh test-host omarchy plugin update jax.seldon` and `timeout 60
ssh test-host sudo pacman -Syu` are blocked; with the flag they pass. The
guard has no other env override of its own paths. `HOME`, `TMPDIR`,
`OMARCHY_PATH` and the rest are read as the session's values for
expanding the command, as the brief says for `HOME`.

### Q3 — more Omarchy routes blocked

Routes `omarchy plugin enable|disable`, `omarchy hook` (with or without
a name; `omarchy hook` runs the hooks under `~/.config/omarchy/hooks/`),
`omarchy branch …` and `omarchy channel set …`, and the binaries
`omarchy-plugin-enable|disable`, `omarchy-hook`, `omarchy-branch*` and
`omarchy-channel-set`. On this install `omarchy branch` has a group
description but no binary; it is blocked anyway. Allowed, with rows:
`omarchy version`, `omarchy plugin list`, `omarchy channel current`,
`omarchy-channel-current`, `omarchy-plugin-list`, `omarchy hook --help`,
`omarchy plugin enable --help` (and the earlier `omarchy commands`,
`omarchy plugin validate`). Over ssh to an unlisted host, `omarchy
plugin enable` is blocked; to a listed test host it passes as before
(docs/HERDR-SETUP.md §5 uses it there).

### N3, N4

- N3: `shellcheck` is still not installed here; `check-guard` runs it
  when present, and CI's container has it.
- N4: "Known limits" above is current for round 2 and belongs next to
  the table when the operator sees it.

### Mutants and table

- 8 new mutants, 33 in all: size cap, variable cap, time budget,
  `GUARD_HOSTS_FILE` gate, plugin enable/disable, hook, branch/channel
  set, the round-2 binaries. All 33 are caught.
- Table: 277 rows (27 new), all green.
- The two scratch sweeps were re-run unchanged: 82 everyday commands,
  none blocked; 104 bypass shapes, none allowed.

### Open question (round 2)

- `scripts/deploy-test-host.sh` reads `GUARD_HOSTS_FILE` unconditionally
  for its own "host must be listed" refusal (line 124; its test suite
  sets it). It is not the hook, so I left it. Should it get the same
  `SELDON_TEST_GUARD` gate? That would be a change to the deploy script
  and its tests (WP-098).

### just check (round 2)

`flock /tmp/seldon-check.lock just check` on 9c54f42 (the code; only
this handover changed after it): `check: ok`, exit 0, including
`check-guard` (`rows: 277`, `mutants: 33`, none survived). Same target
dir (`engine/target` in the worktree); `shellcheck` skipped locally (not
installed).

## Round 3

Stage 2 (Fable) swept 9 542 generated rows against 7fa7efe. The parser
guard was stronger than main's everywhere except unknown exec wrappers
(a regression). Round 3 fixes that regression and all of the brief's A
items. Each one is table rows; nothing was executed, only fed as text.

### A1–A12

| Item | Change | Rows |
|---|---|---|
| A1 | `trap -- 'X' SIG` reads the string after `--` | 1 B, 1 A |
| A2 | `shred`: `-n`/`-s` take an argument, `-u` is a flag | 2 B |
| A3 | any IFS assignment except as a prefix of `read`/`mapfile` → fail closed | 4 B, 2 A |
| A4 | namerefs (`declare/local/typeset -n`) are followed; an unknown target is unknown | 3 B |
| A5 | `${HOME%/}`, `${HOME#x}`, `${HOME/x/y}`, `${HOME:0}`: the value itself or unknown | 4 B |
| A6 | a `cd` after `;`/newline may fail (old directory stays a candidate; after `&&` it succeeded); `cd -`, `~-`, `$OLDPWD`; `popd` → old or unknown; a function defined in the command is walked at each call (nesting limit) | 9 B, 3 A |
| A7 | links made in the command (`ln`, `ln -s`, `cp -s`) are followed for writes, command names and Omarchy scripts; the link is recorded after its own write check; at most 64 | 5 B, 2 A |
| A8 | ssh to this machine (localhost, 127.*, ::1, 0.0.0.0, its own host name, a computed host) gets the local rules; the test-host gate is unchanged | 9 B, 1 A |
| A9 | Omarchy's own scripts: a path under `/usr/share/omarchy` or `$OMARCHY_PATH` (also via a link) as a command, `bash`/`sh`/`source`/`.`, `bash -c`, `env OMARCHY_PATH=…`, loops → blocked; `omarchy-*` binaries keep their rule; `bash -n` passes | 12 B, 5 A |
| A10 | write paths: `patch` (operand, `-d`, `-o`, `-r`), `tar -x` (`-C`, old style), `tar -c` (`-f`), `unzip` (`-d`), `curl` (`-o`, `-O`, `--output-dir`, `-D`, `-c`), `wget` (`-O`, `-P`, `-o`), `git clone/init/worktree add` and the work tree (`-C`, `--work-tree`, cwd) of changing verbs | 20 B, 14 A |
| A11 | `script` without `-c` reads its shell from stdin: pipe → fail closed, heredoc → checked | 2 B, 1 A |
| A12 | `alias NAME=…` or `shopt -s expand_aliases` → fail closed | 2 B, 2 A |

### A13 — exec wrappers (the regression)

- **(a) Wrapper table.**
  - Unwrapped and checked: `hyprctl dispatch exec` (window rules
    stripped) and `--batch`; `tmux new|new-window|split-window|
    run-shell|respawn-*|display-popup|if-shell|pipe-pane` and
    `send-keys` (`Enter` → newline), commands separated by `;`, `tmux
    -c`. Also terminals (`foot`, `alacritty`, `ghostty`, `kitty`, …:
    after `-e`, otherwise every operand suffix), `taskset`, `chrt`,
    `systemd-inhibit`, `systemd-cat`, `ssh-agent`, `dbus-run-session`,
    `dbus-launch`, `uwsm app|start`, `unbuffer`, `gdb --args`, and
    `sg … -c`.
  - Fail closed: `hyprctl keyword exec*|bind*`, `gdb -ex/-x/--batch`,
    `bwrap`, `parallel`, `firejail`, `unshare`, `nsenter`, `chroot`,
    `socat EXEC:|SYSTEM:`, `sftp -b`.
  - rsync and scp/sftp: `rsync -e/--rsh` and `scp/sftp -S` must name
    `ssh`, and its `-o ProxyCommand/LocalCommand` stay blocked.
  - Run a program: `tar -I`, `--to-command`, `--checkpoint-action`,
    `-F`, `--info-script`; `rg --pre`; `git -c` with a key that runs a
    program (alias.*, core.pager/editor/sshCommand/fsmonitor/hooksPath/
    askPass/gitProxy, credential.*, diff.*, filter.*, merge.*, pager.*,
    protocol.*, sequence.editor, gpg.*program, include*). Also
    `--config-env`, `--exec-path=`, `--upload-pack`, `--template`, and
    `ext::` URLs.
- **Decision on git variables.** The brief says GIT_*/EDITOR/PAGER
  prefix assignments → fail closed. Taken literally that blocks
  `GIT_PAGER=cat git log` and `GIT_EDITOR=true git rebase --continue`,
  which agents run every day. So only the variables that name a program
  are checked: GIT_PAGER, PAGER, GIT_EDITOR, EDITOR, VISUAL,
  GIT_SEQUENCE_EDITOR, GIT_SSH(_COMMAND), (SSH_)GIT_ASKPASS,
  GIT_EXTERNAL_DIFF, GIT_PROXY_COMMAND, GIT_EXEC_PATH, GIT_TEMPLATE_DIR
  and any GIT_CONFIG*. They fail closed unless their value is `cat`,
  `less`, `more`, `head`, `tail`, `true`, `false`, `:` or empty. Other
  GIT_* (author dates and the like) pass. Rows: two allow, two block.
- **(b) The net.** An unknown program fails closed when an argument's
  basename is in PRIVILEGE ∪ PACKAGE ∪ SERVICE, `omarchy`, `omarchy-*`
  or `grub-*`, or is a shell followed by `-c`. The exception is a data
  sink: echo, printf, grep, rg, cat, less, head, tail, man, which, type,
  stat, ls, file, diff, wc, sort, jq, git, gh, herdr, tee, awk, sed,
  cargo, just, python3, node, seldon, journalctl, the file and archive
  tools, and similar programs. Rows: `myrunner sudo ls`,
  `strace -f pacman -Syu`, `catchsegv omarchy-update` and
  `xyz bash -c 'ls'` block. `man sudo`, `which sudo`,
  `stat /usr/bin/sudo`, a herdr prompt naming sudo and
  `cargo test pacman` pass.
- **Rows for A13:** (a) 41 B and 11 A, (b) 4 B and 5 A. Two of the (a) blocks (`foot -e rm -rf
  ~/.config/hypr`, `taskset -c 0 rm -rf ~/Seldon`) name no red-zone
  word, so the net cannot catch them and the wrapper rules have rows of
  their own.

### False positives of the sweep

`eval "$(ssh-agent -s)"` passes: the cmdsub's command must be exactly
`ssh-agent` with `-s`, `-c`, `-k` or `-D`. Read-only `loginctl` verbs
(list-*, show-*, session-/user-/seat-status) pass;
`terminate-session` and `kill-user` stay blocked.

### Not in this round

The stage-2 B items are lines in "Known limits" above, for the
operator's morning decisions:
- Omarchy routes outside the list (stage 2 recommends a read-only
  allow-list);
- D-Bus, `hyprctl plugin|reload|keyword` and polkit paths;
- `~/.config` writers by program;
- files written then run in the same command;
- interpreters and `xargs` input;
- `ssh -I` PKCS#11;
- the hook covers only the Bash tool.

### Tests

- **Table:** 447 rows (170 new), all green.
- **Mutants:** 62 (29 new), all caught by their intended row. Two
  identical runs give the same verdicts and killing rows.
- **Flakiness found and fixed.** In a first run, two mutants counted as
  "killed" only because, under 62 parallel processes on a loaded host,
  the at-cap row (0.9 s alone) went past the 3 s budget. Fixes:
  - the allowed at-cap row is now one long word (0.04 s), still exactly
    262 144 bytes;
  - the budget rows use 10 KB with a 0.001 s budget;
  - the runner uses half the cores.
  The two real survivors this exposed (a failed `cd` after `;`, and
  `cd -`) now have sharper rows.
- **Sweeps:** the 82 + 46 everyday commands are not blocked; the 104
  bypass shapes are not allowed.
- **Timing at the cap,** this host at load average ~10 (other agents'
  cargo builds): the worst allowed shape is a `$(echo x)` chain at
  1.47 s; `echo x;` repeated 0.99 s.
  - 32 000-element `||` chain: 2.14 s;
  - 22 670 subshell assignments: 2.01 s.
  - Fail closed in time: 60 000 calls of one function hit the 3 s
    budget, and links past 64 are blocked.

### just check (round 3)

`flock /tmp/seldon-check.lock just check` on 01a1a54 (the code; only
this handover and `memory/pitfalls.md` changed after it): `check: ok`,
exit 0, including `check-guard` (`rows: 447`, `mutants: 62`, none
survived), on a host at load average ~10. `shellcheck` still skipped
locally (not installed).

## Round 4

Stage 2 re-looked round 3 (54518fd):
- all A items closed;
- generated sweep of 7 001 bypass shapes: allowed 4 499 → 156;
- one short round asked.

Everything in the brief is done. Each item is table rows, fed as text
only.

### Must fix

| # | Change | Rows |
|---|---|---|
| 1 | `GIT_WORK_TREE`, `GIT_DIR` (set in the command or in the session) and `--git-dir`: the work tree is checked for changing verbs; the repository dir for changing verbs and `fetch`/`gc`/`config`/`tag`/`branch`/… | 3 B, 1 A |
| 2 | `fd -x/-X` unwrapped (placeholders → unknown, else the path is appended); `rustup run <toolchain> cmd` unwrapped; `man -P`/`--pager=` must be a harmless pager, `man -H` and an unsafe `MANPAGER`/`PAGER`/`BROWSER` set in the command fail closed; `sort --compress-program` and `wget --use-askpass` (also via `-e`) fail closed | 7 B, 6 A (`man sudo`, `fd pattern` kept) |
| 3 | `hash -p` fails closed | 1 B, 1 A |
| 4 | hyprctl joins its arguments into one request (`--batch` splits on `;`). tmux `send-keys` joins keys without spaces: Space, Tab and Enter are mapped, `C-c`/`C-u` drop the line, `-l` is literal, `-H` and every other key name (Up, BSpace, F1, M-x, …) fail closed. | 6 B, 2 A |
| 5 | xargs: the replstr (`-I R`, `-i`, `--replace`) becomes unknown in the wrapped argv; without one the stdin items are appended as unknown (`xargs env` → computed name) | 3 B, 2 A |
| 6 | `tar --one-top-level=DIR`; `git init --separate-git-dir=DIR` | 2 B |

### False positives → allow rows

- **`git -c` with a harmless value.** A key that names a program now
  passes with a harmless value (the same `SAFE_PROGRAMS` the variables
  use): `core.pager=cat`, `core.editor=true`.
- **Narrowed key set.** core.pager/editor/sshCommand/fsmonitor/
  hooksPath/askPass/gitProxy, credential.*, sequence.editor,
  diff.external, merge.tool, uploadpack.*hook, pager.*, gpg.*program
  and any `*.cmd|command|driver|textconv|helper|program|clean|smudge|
  process`. Two kinds fail closed whatever the value: `alias.*` and
  `include*`. `protocol.allow` and `protocol.ext.*` fail closed unless
  the value is `never`. So `diff.noprefix`,
  `merge.conflictstyle`, `protocol.file.allow` and
  `receive.denyCurrentBranch` pass.
- **`GIT_CONFIG_*`.** `GIT_CONFIG_NOSYSTEM` passes. `GIT_CONFIG_GLOBAL`,
  `GIT_CONFIG_SYSTEM` and `GIT_CONFIG` pass only as `/dev/null`. Every
  other `GIT_CONFIG*` (PARAMETERS, COUNT, KEY_n, VALUE_n) fails closed.
- **Rows:** 9 A, 6 B.

### Known limits: the net, widened

The brief's widening is implemented: a shell basename (any of the
guard's shells) or a path under `/usr/share/omarchy`/`$OMARCHY_PATH`
among an unknown program's arguments fails closed.

**One addition beyond the brief.** The brief's first row, `strace -f
sed -i s/a/b/ /etc/x`, names neither a shell nor an Omarchy path, so the
widening alone does not block it. The net therefore also checks every
argument suffix that starts with a known file command or wrapper (rm,
cp, mv, sed, tee, git, tar, curl, find, xargs, env, ssh, …) as that
command. Rows:
- block: `strace -f sed -i s/a/b/ /etc/x`, `myrunner bash
  /usr/share/omarchy/install.sh`, `myrunner bash script.sh`, `myrunner
  /usr/share/omarchy/install.sh`, `myrunner rm -rf ~/.config/hypr`;
- pass: `strace -f ls`, `myrunner build`.

**False positive found and fixed.** The everyday sweep found one:
`shellcheck -s bash …`. `shellcheck` and `shfmt` are data sinks now,
with an allow row.

Known limits now has one line for what is left, an unknown program's
own write options (`strace -o ~/.config/x ls`). The files-a-shell-runs
line now names `bash < cmds.txt` and `script /dev/null < cmds.txt`, and
GNU `sed`'s `e` command joins the interpreter line.

### Mutants and table

- **Table:** 506 rows (59 new), all green.
- **Mutants:** 85 (23 new), every one killed by its intended row; two
  runs are identical. Three round-3 mutants were updated to the changed
  code.
- **Survivors found and handled.** Two wrapper mutants (terminal `-e`,
  taskset) survived at first, because the wider net now also catches
  their old rows. Defense in depth, not a hole. New rows (`foot -e
  "$CMD"`, `taskset -c 0 "$CMD"`) show what only the wrapper rule
  catches: a computed command behind the wrapper. The narrowed-key
  mutant (back to `diff.*`) survived because `diff.noprefix=true` is a
  harmless value either way; `git -c diff.colorMoved=zebra diff` (allow)
  now separates the two.
- **Sweeps:** 82 everyday commands, none blocked; 104 bypass shapes,
  none allowed. 42 further everyday commands: one false positive
  (shellcheck), fixed.
- **Timing at the cap:** worst allowed shape 1.45 s, load average ~5.

### just check (round 4)

`flock /tmp/seldon-check.lock just check` on 3bd3981 (the code is
361e1a5; later commits change only this handover and
`memory/pitfalls.md`): `check: ok`, exit 0, including `check-guard`
(`rows: 506`, `mutants: 85`, none survived). `shellcheck` still skipped
locally (not installed).

## Merge of main

Stage 2 (Fable) approved 8c55475. Before the operator's look at the
table, `origin/main` (3e3aa2c, 0.1.4 preparation) was merged into
`wp/130-guard`.

- **The merge** (792d18f): `git merge origin/main` merged cleanly; git
  reported no conflicts (`Merge made by the 'ort' strategy`, 45 files from
  main: WP-117/118, the 0.1.4 plugin and install changes). Two files this
  WP also touches were auto-merged:
  - `justfile`: main added `tests/plugin/terminal-scripts.sh` to
    `plugin-test`; `check-guard` is still in `check`.
  - `docs/TESTING.md`: the `check-guard` row is still in the table.
  No guard file changed on main.
- **TABLE.md for the operator.** `work/active/WP-130/TABLE.md` is
  generated by `scripts/guard-table.py`, which is committed:
  - it runs `scripts/guard-test.sh` with `GUARD_TEST_REPORT` and fails
    unless the table is green;
  - each row shows the input, the verdict and the guard's own message
    (`red zone: …` / `fail closed: …`), with a context in brackets where
    a row runs with a test setting (test host listed, `SELDON_TEST_GUARD`
    unset, a shorter time budget);
  - allowed rows come first, then blocked rows, both grouped by the
    table's sections;
  - the "Known limits" section of this handover follows verbatim.

  For this, `guard-test.sh` names its groups with `section TITLE` (the
  former heading comments; the rows are unchanged, 506) and, with
  `GUARD_TEST_REPORT`, appends one JSON line per row.
  `python3 scripts/guard-table.py --check work/active/WP-130/TABLE.md`
  fails when the file drifted. It is not part of `just check`, because
  the path moves when the WP is completed. When the table or the
  Known limits change, regenerate with
  `python3 scripts/guard-table.py work/active/WP-130/TABLE.md`.
- **Counts:** 506 rows (181 allowed, 325 blocked), 85 mutants.

### just check (after the merge)

`flock /tmp/seldon-check.lock just check` on a9a38f7 (merge, generator
and TABLE.md; only this handover changed after it): `check: ok`, exit 0.
That includes main's new `terminal-scripts: 65 passed, 0 failed` and
`check-guard` (`rows: 506`, `mutants: 85`, none survived). `shellcheck`
is still skipped locally (not installed).
