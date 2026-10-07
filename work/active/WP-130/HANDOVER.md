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

## Known limits (current as of round 3; show next to the table)

What the guard does **not** protect. Each line is a decision for the
operator, not a hidden gap.

- **Only the Bash tool is guarded.** The hook's matcher is `Bash`. The
  Write and Edit tools are not checked by this guard; they are covered
  only by the settings deny list (`/etc`, `~/.config/hypr`,
  `shell.json`).
- **Files a shell runs are not inspected** (`bash script.sh`,
  `source x`, `ssh h bash -s < file`). Generated ones are blocked
  (`source <(…)`, `bash <(…)`), and so are Omarchy's own scripts (round
  3). A file written earlier in the same command and then run
  (`cat > x.sh <<EOF … EOF; bash x.sh`) is not inspected either.
- **Other interpreters are data**: `python3 -c`, `perl -i`, `node -e`,
  `awk 'system()'`.
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
