```
WP-071 HANDOVER — shell parser: one parser for hook and attribution; pkexec, run0, option clusters; version/help; `>&` and heredocs
Branch: wp/071-review (worktree wt/WP-071), rebased onto main ef46863 (WP-069), round-1 commits 05df7b3, c0e1edb, 1218c5f, bcf8be0, 8f1e491 plus the fix-round commits after them; not pushed.
```

## Fix round 1 (review SEND BACK)

- **Rebase** onto main ef46863 (WP-069 landed). CHANGELOG `[Unreleased]`
  and `memory/pitfalls.md` conflicted; both sides kept (WP-065/066/069
  bullets and sections first, WP-071 after them). The suites passed
  unchanged after the rebase.
- **B1 (blocking): floating words no longer match below a pattern.** A
  word that starts with an unknown part (`$X/tail`, `"$(pwd)"/tail`) was
  compared with `{p}/**`, so it matched every path pattern. With WP-069's
  default `skipPaths` (`~/.config/omarchy/**/*.log`, …) lines such as
  `pacman -U $PKGDEST/x.pkg.tar.zst` or `yay -S zed | tee
  "$TMPDIR/yay.log"` were stored as `<program> ‹redacted›` and lost their
  package intent. Now `SkipGlob::floating_overlaps` (hook.rs) matches such
  a word only:
  - against a name pattern, by the word's last component;
  - against a path pattern, by the pattern's last components, which must
    be literal (no `*`/`?`) and match the word's known tail one by one;
  - never against `{p}/**`, and never against a pattern's glob tail.

  `$D/private.conf` and `"$(pwd)"/private.conf` still match
  `~/d/private.conf`. `$X/public.conf`, `"$(pwd)"/public.log`
  (`~/d/**/*.log` configured) and `$TMPDIR/yay.log` (defaults) do not.
  Tests:
  - two new rows in the `shell_parser.rs` table, whose end-to-end run uses
    `init`'s default `skipPaths` (`pacman -U $PKGDEST/…`, `yay -S zed |
    tee "$TMPDIR/yay.log"`): recorded unredacted with intent `[zed]`;
  - the requested controls plus `$X/private.conf` in
    `skip_paths_read_variables_and_globs`, now with the patterns
    `~/d/private.conf` and `~/d/**/*.log`.
- **B2: SPEC limits.** §8, at the "re-parsed" sentence: the commands
  inside `$(…)`, backticks and `<(…)` are not classified (their words count
  only for `skipPaths`); `env -S` strings are not opened; a heredoc fed to
  a shell is stdin, not commands. §4's intent sentence points to that
  list. The §8 `skipPaths` sentence describes the floating rule and the
  unknown-only rule.
- **N1: I kept a flag, not the SPEC wording.** `Word::Pattern` has
  `only_unknown` (nothing known but `/`: `$1`, `$NAME`, `$D/$F`) in place
  of the all-star test, so a glob of the word's own (`*`) is checked (`cd
  ~/d && sed … *` is redacted when `~/d/private.conf` is skipped). The
  SPEC sentence says both.
- **N2:** `-` (targets from stdin) is no package name in
  `PacmanCommand.targets`, so it is not in the intent either (`pacman -S
  - < list.txt` → red, intent empty; a table row and a unit test).
- **Guard block, reported:** a read-only `grep -n` over
  `engine/tests/shell_parser.rs` whose pattern contained a package
  command was blocked ("privileged or package command"). Not reworded; I
  read the file with the Read tool instead.

## Done

1. **One parser (F-530).** `pkgcmd::command_intent` is now
   `segments_intent(simple_commands(parse_shell(line)))`: the same
   `parse_shell` → `simple_commands` → `command_argv` the hook classifies
   with. The naive `& | ; \n` + whitespace split is gone.
   `attribution::causes` parses a recorded line once and takes both the
   intent (`segments_intent`) and the segments from it (the intent call is
   the only change in `attribution.rs`). The Omarchy routes for the intent
   come from one `pkgcmd::omarchy_route` (also used by the hook now), and
   the intent names only the routes the hook records: `omarchy update`
   (bare and the update routes the hook records red) and `pkg
   add|install|drop|remove`, `pkg aur add|install`, as a command or as an
   `omarchy-*` script. `omarchy-pkg-present x` and `omarchy update
   available` name nothing any more; the hook never recorded them, so no
   attribution changes in practice.
2. **Wrappers (F-531).** `command_argv` reads one table (`WRAPPERS`):
   `sudo`, `doas`, `pkexec`, `run0`, `env`, `nice`, `timeout`, `exec`,
   `command`, `nohup`, `time`. Each wrapper lists its value options (short
   and long), its probe options and its chdir option. Short options are
   read as clusters (`-Eu root`, `-uroot`, `-iu root`), long ones as
   `--opt value` or `--opt=value`. Probes run nothing: `command -v|-V`,
   `sudo -l|-v|-K|-V|--list|…`, `doas -C|-L`, `--help`/`--version` of
   pkexec, run0, env, nice, timeout, nohup, time, `run0 -h`.
   **Behaviour change:** `sudo -k <command>` is no probe any more. It
   ignores the cached credentials and still runs the command; the old
   list skipped it.
3. **Package commands (F-532).** `parse_command` models `-h`, `-V`,
   `--help` and `--version` (print only: not mutating, no full upgrade;
   `yay --version` no longer defaults to `-Syu`). It also models yay/paru
   `-Y`/`--yay` (`Op::Yay`; only `-Yc`/`--clean` is mutating, a removal
   that names no package), `--gendb`, `-P`/`--show` (`Op::Show`) and
   `-G`/`--getpkgbuild` (`Op::GetPkgbuild`).
4. **`>& word` (F-534).** After `>&` the next word (attached or after a
   blank) is a descriptor copy when it is digits, `-` or `N-`, and a
   written file otherwise (`Pending::WriteOrCopy`). `<&3` is a read.
5. **Heredocs (F-536).** `parse_shell` is now a recursive reader
   (`Reader::read` with a stop: end of line, the `)` of `$(`/`<(`, a
   closing backtick). `$(…)`, backticks and `<(…)` are read as shell
   text, also inside double quotes, so a heredoc in them is cut like a
   top-level one. The substitution stays one literal word without the
   body. `((…))` at command position, `$((…))`, `$[…]` and `${…}` are
   skipped as one unit, so their `<<` is a shift. Quotes inside a `$(…)`
   no longer end an outer double-quoted word early.
   **Found on the way:** the hook cut a heredoc's body *and* its
   delimiter line. Attribution parses the recorded text again, so
   `cat <<EOF` stayed unterminated and swallowed every later line (the
   `pacman -S x` after a heredoc was lost to attribution, also for
   top-level heredocs before this WP). The delimiter line now stays in the
   record when anything follows it. A trailing one is still cut, so the
   record pinned by `tests/hooks.rs` `heredoc_bodies_are_not_recorded` is
   unchanged.
6. **Orchestrator addition: one reading of directories.**
   `pkgcmd::workdirs` gives each simple command a `Workdir`:
   - `shell` follows `cd DIR`, `cd`, `cd -`, `pushd DIR` and `popd`;
     redirections resolve here;
   - `program` is `shell` after the wrappers' `env -C`, `sudo -D`,
     `run0 -D` and the program's own `-C DIR` (git only before its
     sub-command, so `git commit -C HEAD` is no directory; none for
     `install`, `grep`, `diff`, `ls`); a program's own write operands
     resolve here.

   The hook classifier (`classify`) and the `skipPaths` check
   (`names_skipped_path`) both use it. `git()` no longer follows `-C`
   itself; it gets the program directory.
7. **Orchestrator addition: variables and globs for `skipPaths`.**
   - `pkgcmd::Vars` reads the variables a line sets to one literal (`F=x`
     alone, or after `export|local|declare|readonly|typeset`; a variable
     set twice or to a non-literal is unknown; `F=x cmd` is not
     followed).
   - `simple_commands` puts the known values into the words and write
     targets, so the classifier and attribution see `~/d/x` for
     `F=x; … ~/d/$F`.
   - `Vars::expand` turns a word with an unknown part into a glob:
     unknown variable or substitution → `**`, `[…]` → `?`, `*`/`?` stay.
   - The `skipPaths` check matches such a glob against the configured
     patterns with `globs_overlap` (`*`/`?` within one path component,
     `**` across components, the way `SkipPaths` reads its patterns).
   - A word in which nothing but `/` is known (`$1`, `$NAME`, `$D/$F`;
     `Word::Pattern.only_unknown`) names no path. Otherwise every line with
     one, such as a `git commit -m "$(cat <<'EOF' …)"`, would be redacted
     once `skipPaths` is set. A word that starts with an unknown part
     follows the floating rule of fix round 1 (B1).
8. **Tests.**
   - `engine/tests/shell_parser.rs` (new):
     - a 60-row table read in-process by `classify` and `command_intent`,
       with exact expectations plus the parity rule "an intent that names
       anything ⇒ the hook records red";
     - the same table end to end: `seldon hook generic` writes the event,
       `attribution::causes` reads the intent back from the written
       `meta.command`;
     - heredoc-in-substitution probes (exact recorded text, the later
       `pacman -S` is the record);
     - `skipPaths` probes with variables, globs and `$(pwd)`, plus
       controls that stay unredacted;
     - the WP's hook cases one by one.
   - `engine/tests/attribution.rs`, mod `packages`:
     - seven wrapped installs (`sudo -u root`, `sudo -Eu root`, `pkexec`,
       `run0`, `timeout 600`, `bash -c`, `nice -n`) through hook generic
       and a pacman capture: each is the agent's, in its case;
     - `yay --version` followed by a person's `-Syu` two minutes later:
       the upgrade stays `system`;
     - in-process: a recorded probe line from an older ledger is no cause.
   - `pkgcmd.rs` unit tests: heredocs in substitutions and shifts, `>&`,
     more wrappers and chdirs, yay forms, intent like the hook,
     `workdirs`, `Vars`, `globs_overlap`; `heredoc_bodies_are_cut` now
     also checks that the cut text re-parses to the same segments.
9. **Docs.**
   - `docs/SPEC-ENGINE.md` §8: the classifier sentence (print-only forms,
     wrappers and clusters), the path, heredoc and `>&` sentences, and the
     `skipPaths` sentence (variables, globs, unknown values).
   - `docs/SPEC-ENGINE.md` §4: the intent sentence (see Decisions needed
     about §5).
   - `CHANGELOG.md` `[Unreleased]`: one Engine bullet.

## Not done (limits, now in SPEC §8)

- Commands *inside* `$(…)`, backticks and `<(…)` are still not segments
  of the line (`echo $(pacman -S x)` is not classified). That was the
  state before this WP; opening them would change classification
  broadly. A candidate for a later WP.
- `env -S 'cmd …'` (split string) is still read as a value, not as a
  command line.

Follow-ups (noted, not done, as the review asks):

- A heredoc body fed to a shell (`bash <<EOF … EOF`, `sh -s <<…`) is not
  read as commands.
- More wrappers: `xargs`, `flock`, `su -c`, `systemd-run` (and
  `stdbuf`, `setsid`, `ionice` if wanted).
- `((cmd) )`-style nested subshells written without a blank (`((cd x &&
  rm y))`) read as arithmetic now, as bash does when the text is valid
  arithmetic. Bash falls back to subshells only when it is not.

## Verified by

- Full engine suite before the mutants: `cargo test --no-fail-fast`
  (31 test binaries, 543 passed, 0 failed); `cargo clippy --all-targets
  -- -D warnings` and `cargo fmt` clean.
- Mutants (scratchpad script, not committed): each reverts one fix,
  runs `cargo test --no-fail-fast --lib --test shell_parser --test
  attribution --test hooks`, then restores the file (`cmp`-checked,
  mtime touched). All 14 are killed; the failing tests per mutant:

- F-530 intent from a naive split — **killed** by 6: collectors::pacman::tests::hook_intents, hook_and_intent_read_the_table_alike, packages::a_recorded_probe_is_no_cause, packages::a_wrapped_install_is_the_agents, pkgcmd::tests::intent_reads_like_the_hook, the_recorded_line_carries_the_intent
- F-531 pkexec and run0 unknown — **killed** by 6: hook_and_intent_read_the_table_alike, packages::a_wrapped_install_is_the_agents, pkgcmd::tests::intent_reads_like_the_hook, pkgcmd::tests::more_wrappers, review_cases_through_the_hook, the_recorded_line_carries_the_intent
- F-531 no option clusters — **killed** by 6: hook_and_intent_read_the_table_alike, packages::a_wrapped_install_is_the_agents, pkgcmd::tests::more_wrappers, pkgcmd::tests::probes_run_nothing, review_cases_through_the_hook, the_recorded_line_carries_the_intent
- sudo -k as a probe — **killed** by 3: hook_and_intent_read_the_table_alike, pkgcmd::tests::more_wrappers, the_recorded_line_carries_the_intent
- F-532 help/version mutate — **killed** by 6: hook_and_intent_read_the_table_alike, packages::a_recorded_probe_is_no_cause, packages::a_version_probe_claims_no_upgrade, pkgcmd::tests::yay_prints_and_operations, review_cases_through_the_hook, the_recorded_line_carries_the_intent
- F-532 -Y -P -G unknown — **killed** by 4: hook_and_intent_read_the_table_alike, packages::a_recorded_probe_is_no_cause, pkgcmd::tests::yay_prints_and_operations, the_recorded_line_carries_the_intent
- F-534 >& file is a copy — **killed** by 4: hook_and_intent_read_the_table_alike, pkgcmd::tests::redirect_to_a_file_with_and, review_cases_through_the_hook, the_recorded_line_carries_the_intent
- F-536 substitutions not read — **killed** by 2: heredoc_bodies_in_substitutions_are_cut, pkgcmd::tests::heredocs_in_substitutions_and_shifts
- F-536 (( )) not arithmetic — **killed** by 4: heredoc_bodies_in_substitutions_are_cut, hook_and_intent_read_the_table_alike, pkgcmd::tests::heredocs_in_substitutions_and_shifts, the_recorded_line_carries_the_intent
- delimiter line always cut — **killed** by 4: heredoc_bodies_in_substitutions_are_cut, pkgcmd::tests::heredoc_bodies_are_cut, pkgcmd::tests::heredocs_in_substitutions_and_shifts, the_recorded_line_carries_the_intent
- pushd not followed — **killed** by 4: hook_and_intent_read_the_table_alike, pkgcmd::tests::working_directories, skip_paths::a_line_that_names_a_skipped_path_is_recorded_redacted, the_recorded_line_carries_the_intent
- operands in the shell's dir — **killed** by 2: hook_and_intent_read_the_table_alike, the_recorded_line_carries_the_intent
- skipPaths words as written — **killed** by 1: skip_paths_read_variables_and_globs
- skipPaths and segments as written — **killed** by 5: hook_and_intent_read_the_table_alike, pkgcmd::tests::intent_reads_like_the_hook, pkgcmd::tests::variables_of_a_line, skip_paths_read_variables_and_globs, the_recorded_line_carries_the_intent

  The first mutant puts the pre-WP naive intent split back (that is the
  pre-fix attribution); `a_wrapped_install_is_the_agents` then sees
  `actor: system` for the wrapped installs, as the finding describes.
  "skipPaths words as written" is the pre-WP reading of the `skipPaths`
  check. The last mutant also drops the variable values from the
  segments, because either mechanism alone redacts the `F=…; … $F` row.
- `just check` on the committed branch (3671b80..562e4d0), run once with
  nothing else in the worktree: exit 0, `check: ok`; the cargo test
  results in it add up to 1095 passed, 0 failed, and the plugin harnesses
  pass (`bar-view: 131 passed, 0 failed`).
- Hook payloads in the tests use only the scratch HOME (`common::Env`);
  no manual run against `~/Seldon` or `~/.config/seldon`; no guard
  block occurred.

- **Fix round 1:**
  - fmt and `cargo clippy --all-targets -- -D warnings` clean;
  - `cargo test --no-fail-fast --lib --test shell_parser --test
    attribution --test hooks --test collectors`: 181 + 5 + 13 + 51 + 16
    passed, 0 failed;
  - full engine suite: `cargo test --no-fail-fast`, 576 passed, 0 failed;
  - mutants, same script with `--test collectors` added:
    - B1 floating word below a pattern (`floating_overlaps` →
      `overlaps`): **killed** by 2 (`skip_paths_read_variables_and_globs`,
      `the_recorded_line_carries_the_intent`);
    - N1 all-star test instead of `only_unknown`: **killed** by 1
      (`skip_paths_read_variables_and_globs`, the `*` row);
    - N2 `-` is a package: **killed** by 3
      (`hook_and_intent_read_the_table_alike`,
      `pkgcmd::tests::intent_reads_like_the_hook`,
      `the_recorded_line_carries_the_intent`).
  - `just check` was not rerun in this round; the review asked for the
    suites above.

## Learned

Appended to `memory/pitfalls.md` (WP-071 section):

- the recorded line is parsed a second time, so a cut must re-parse to
  the same commands;
- a glob overlap test must keep `*` inside one component;
- a word that is only an unknown value must not match;
- `sudo -k <cmd>` runs the command;
- the WP's "§5" sentence is in §4;
- fix round 1: a word with an unknown head is no path below a pattern.

## Decisions needed

- **`engine/src/commands/hook.rs` was edited** although the WP lists it as
  read only. The two sections the orchestrator appended (`pushd`/`-C` for
  the classifier's write targets; `skipPaths` variable and glob forms)
  live in `classify`, `git()` and `names_skipped_path`. The edits keep
  WP-057/063's lock order, scope and token split. Please confirm, or
  split them into a follow-up WP.
- **SPEC section:** the WP names "§5 (intent sentence)", but the
  attribution intent sentence is in §4 (pacman collector bullet); §5
  (drift) has none. I edited §4.
- **`sudo -k <command>` is now a recorded command** (it was a probe).
  Please confirm this reading.

## Touched outside WP scope

- `engine/src/commands/hook.rs` (see above).
- `memory/pitfalls.md` (append, as the brief asks).
