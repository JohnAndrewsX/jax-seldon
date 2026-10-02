```
WP-049 HANDOVER
Done:
- `seldon completions <bash|zsh|fish>` (clap_complete) and
  `seldon mangen` (clap_mangen), engine/src/commands/manual.rs (86c27f3).
  Both are generated from main.rs's clap definition, run before the
  Context (no home, config, logbook or guard needed) and take --json
  ({shell, script}, {manPage}). seldon(1) is clap_mangen's title, name,
  synopsis, description and global options, then a COMMANDS section of
  our own (every command and subcommand: usage line, its sentence, its
  arguments with possible values and defaults) instead of clap_mangen's
  list of seldon-<cmd>(1) pages, which we do not ship; then EXIT STATUS,
  ENVIRONMENT, FILES, SEE ALSO. Roff escaping for `-`, `\`, backticks,
  non-ASCII and leading dots (groff -ww clean).
- Removal commands, each removing exactly what was installed and
  recorded through the own-writes mechanism of WP-038:
  - `seldon hook uninstall claude-code [--settings FILE]`: takes out each
    of the three hooks install writes (same event, matcher, command);
    the user's hooks and keys stay, also a hook the user put into one of
    Seldon's groups; groups, event lists and `hooks` it leaves empty go;
    a file left as `{}` is deleted (directory stays). Nothing there:
    nothing written, exit 0. Broken file: refused unchanged, exit 1. The
    logbook's own file is committed as `seldon: hook uninstall
    claude-code`. Not an agent hook (exit 3 without a logbook).
    `generic` is not offered: it has nothing installed (WP-050 dropped
    `hook install generic` for the same reason).
  - `seldon init --remove-theme-hook`. FORM CHOSEN: the init flag, not a
    `setup theme-hook --remove` command. Why: `init --theme-hook` is the
    only way the hook gets installed, so the undo sits next to it in the
    same --help, and no new top-level command exists for one action. It
    runs no wizard, needs no logbook, conflicts with every other init
    flag (conflicts_with_all; `exclusive` would also reject the global
    --json). It deletes ~/.config/omarchy/hooks/theme-set.d/
    seldon-theme-set.sh (Omarchy 4 has no `omarchy hook remove`; checked
    in /usr/share/omarchy/bin) and $XDG_STATE_HOME/seldon/hooks/
    seldon-theme-set.sh.
  - own writes: owned.json records gain `op` (absent = install, `remove`
    = Seldon's part taken out and the file stays, `delete` = the file
    deleted, hash of the content it had, recorded under the lock before
    the deletion). The next capture explains a config-change by hashTo,
    a config-remove by hashFrom, detail `removed by <command>`.
- --help review (86c27f3): one sentence per command (plan steps were
  arrows: "Start a case: queued → active; it becomes the active case";
  plan/drift/hook top-level sentences); value names say what they are
  (<ACTOR>, <ZONE>, <RISK>, <AREA>, <PRIORITY>, <STATUS>, <TAG>,
  <SUBJECT>, <TEXT>, <NUMBER>, <FILE> for --settings like --config);
  every positional has a line (<ID> "The case id, e.g. C-2026-004",
  <EVENT> "The drift event id, as `seldon drift` prints it"); `--actor`
  everywhere "Who …: human or agent:NAME"; examples under the options
  for init (--since date form, --remove-theme-hook), capture (--since
  RFC 3339, --source), log, drift explain, drift dismiss (free text
  after `--`), completions, mangen; event --meta has an e.g.
  `--snapshot` is now on `plan start` only (StartArgs flattens StepArgs),
  so verify/done/drop no longer show it; passing it there is clap's
  exit-1 error (tests/plan.rs argument_errors still green).
- Also: stdout writes no longer panic on a closed pipe (print_line in
  main.rs; `seldon completions bash | head` used to abort with SIGABRT
  in release).
- Packaging (3b9a9e3): PKGBUILD package() runs the built binary for
  seldon.1 and the three completions (/usr/share/man/man1,
  bash-completion/completions/seldon, zsh/site-functions/_seldon,
  fish/vendor_completions.d/seldon.fish); expected-files.txt updated
  (seldon.1.gz: makepkg zipman). PKGBUILD metadata unchanged, so
  .SRCINFO NOT regenerated (check-srcinfo ok). install.sh: the new
  binary generates the man page (always) and the completion of each
  shell whose completion dir exists under /usr/share
  (SELDON_INSTALL_SHARE in tests) into <prefix>/share/...; all in the
  manifest, --uninstall removes them; a foreign file is kept unless
  --force; a release without the commands skips with a note; zsh gets
  an fpath hint. release.yml: musl binary's completions `bash -n`,
  mangen checked; seldon.1 and completions/ in the binary tarball; the
  package's completion and man page checked after makepkg.
- Docs (d96c89a, 41b8724): SPEC-ENGINE §2 owned.json, §3 synopsis and
  help-text rules, §5 rule 7, §8 hook uninstall, §9 remove-theme-hook;
  TESTING.md rows (manual.rs, hooks.rs, own_writes.rs, install test);
  docs/user/{en,de}/05 (new blocks hook uninstall, completions, mangen;
  regenerated with `docs-check.sh --write`) and 11 (installer installs
  man page and completions; uninstall steps 3/4 use the new commands
  and run before the engine is removed); de source lines stamped at
  d96c89a; engine/hooks/README.md; CHANGELOG [Unreleased].

  SPEC wording (§3 synopsis additions):
    seldon init --remove-theme-hook   # WP-049: undoes --theme-hook (§9); conflicts with
                                      # every other init flag, needs no logbook
    seldon hook uninstall claude-code [--settings PATH]
                                      # WP-049: the inverse of install (§8); `generic` has
                                      # nothing installed, so nothing to uninstall
    seldon completions bash|zsh|fish  # WP-049: the completion script (clap_complete) on stdout;
                                      # --json → {shell, script}
    seldon mangen                     # WP-049: seldon(1) in roff on stdout (…); --json → {manPage}.
                                      # Both are generated from the clap definition, read no config,
                                      # logbook or home, and are what the package and install.sh install
  §5 rule 7 addition: "The removal commands (WP-049) record the same
  way: `hook uninstall` that leaves the file records the new content
  with `op: remove`, and its `config-change` is explained; a file the
  engine deletes (`init --remove-theme-hook`, `hook uninstall` of a file
  that held only Seldon's hooks) is recorded *before* the deletion with
  `op: delete` and the sha256 of the content it had, and a
  `config-remove` whose `meta.hashFrom` matches it is explained; both
  with detail `removed by <by>`. A file someone changed after the last
  capture and before the deletion does not match (the manifest's hash
  is the older one): its removal stays drift."

  Crate justification (AGENTS.md §7; approved by the operator
  2026-10-02 with the WP set): clap_complete 4.6.11 and clap_mangen
  0.3.3 generate the completions and the man page from the clap
  definition the engine already has, so neither can drift from --help;
  writing three completion dialects and roff by hand would. clap_mangen
  pulls in `roff` 1.1.1 (transitive only, no own use). Both are pure
  Rust, no network, no async; the static musl build is unaffected.
Not done:
- No local makepkg build: the guard hook blocked `makepkg`
  ("guard: blocked (AGENTS.md §6 red zone): privileged or package
  command") on a scratch build from `git archive`; reported, not routed
  around. The package file list is therefore unverified until the
  release workflow's dry run (`gh workflow run release.yml --ref
  wp/049-cli-polish`), which diffs expected-files.txt against the real
  package.
- `zsh -n` / `fish -n` of the generated scripts and shellcheck of
  install.sh / install.test.sh did not run: zsh, fish and shellcheck are
  not installed on the dev host (tests skip with a note; CI's container
  runs shellcheck). bash -n, groff -ww and man -l did run.
- No man pages per subcommand (seldon-init.1, …): the WP asks for
  seldon.1; the COMMANDS section covers every command.
Verified by:
- `just check` → exit 0, "check: ok" (fmt, clippy -D warnings, tests,
  check-watch, check-packaging, check-install 130 passed, schema,
  docs-check "ok (378 links, 14 translated pages, 40 commands, 423
  command lines)", plugin-validate, qmllint 28 files, plugin-test).
- `seldon completions bash | bash -n` → 0; `seldon mangen > seldon.1;
  man -l seldon.1` → 0, 487 lines at MANWIDTH=80, groff -ww clean.
- Removal → no open drift after the next capture: tests/own_writes.rs
  removing_the_theme_hook_leaves_no_drift (config-remove explained
  "removed by seldon init --remove-theme-hook", openDrift 0, a second
  removal and capture write nothing),
  hook_uninstall_under_a_watched_path_leaves_no_drift (config-remove),
  hook_uninstall_from_a_shared_watched_file_is_an_explained_change
  (config-change, op remove), plus the negative cases (removed before
  any capture saw it → no event; edited then removed → drift).
- tests/hooks.rs uninstall:: (6 tests), tests/manual.rs (5 tests),
  unit tests in commands/manual.rs and collectors/config.rs.
- `just check-packaging` → "check-packaging: ok" (bash -n, check-srcinfo,
  release-notes; shellcheck absent locally).
- Everything hermetic: common::Env scratch homes with SELDON_TEST_GUARD;
  manual runs in the session scratchpad with SELDON_TEST_GUARD; the
  operator's ~/Seldon, ~/.config and ~/.local untouched.
Learned: memory/rust-notes.md and memory/pitfalls.md (2026-10-02 · WP-049):
  clap_mangen render() vs composed sections, roff escaping, `exclusive`
  vs global args, flatten for a one-step flag, println! EPIPE abort,
  hash-before-delete for own writes; makepkg guard block, cargo add needs
  crates.io, moving HOME breaks rustup, grep -q under pipefail.
Decisions needed:
- install.sh reads "when the shell dirs exist" as: the shell's
  completion dir exists under /usr/share (bash-completion, zsh,
  fish installed) — not "the user dir under the prefix already exists",
  which would almost never install anything for ~/.local. Confirm.
- install.sh also installs the man page under the prefix (not asked
  for; cheap, same manifest/uninstall path). Keep or drop.
- `cargo add` fetched clap_complete, clap_mangen and roff from crates.io
  (not in the local cache); the engine itself stays offline.
Touched outside WP scope: CHANGELOG.md ([Unreleased] entries);
  engine/hooks/README.md (removal rows); main.rs print_line (the EPIPE
  abort, found while testing `completions | head`).
```

```
WP-049 HANDOVER — fix rounds after review
Done:
- Round 1 (4623bee): `# shellcheck disable=SC2016` with a reason above the
  two `bash -c '…$1…'` checks in tests/install/install.test.sh.
- (B1) 4763207: main.rs print_line ignores only ErrorKind::BrokenPipe;
  any other stdout write error → "seldon: cannot write to stdout: …" on
  stderr, exit 2 (also for the JSON error line of `fail`). `seldon mangen
  > /dev/full` now exits 2 instead of 0 with an empty file.
- (B2) 4763207: config::delete_own_file writes the `op: delete` record
  BEFORE remove_file, as SPEC-ENGINE §5 rule 7 says (spec unchanged); a
  record whose deletion fails stays and is harmless.
- (N2) 86f3fd8: `hook install` and `hook uninstall` take the state lock
  before reading the settings file and hold it through the write, the
  own-write record and the autocommit (setup::record_own_writes_under,
  delete_own_file_under). With the lock held elsewhere they now exit 4
  and change nothing (before: wrote the file, then warned that the record
  failed). SPEC-ENGINE §8 gained one sentence saying so (dfd9523).
- (N1) 80b77eb: install.sh shell_present needs `command -v zsh` /
  `command -v fish` plus the directory; bash keeps the bash-completion
  directory check. Docs 11 unchanged. The install test links the host's
  programs without zsh and fish into its PATH and decides with fakes in
  $work/shells; TESTING.md row updated.
- (N3) dfd9523: SPEC-ENGINE `[--settings PATH]` → `[--settings FILE]`
  (4 places).
- (N4) 86f3fd8: `import omarchy-agent`: "Import the omarchy-agent kit's
  Obsidian vault, which is only read (dry run unless --apply)"; docs 05
  regenerated (dfd9523), de source line re-stamped at dfd9523 (062bf3c).
- Follow-ups left as instructed: jax-seldon symlink completion, JSON key
  re-ordering on hook install/uninstall.
Not done: nothing of the list. Note: the round took five commits instead
  of one or two (engine B1+B2, engine N2+N4, docs, de stamp, install.sh
  N1); the branch is under review, so I did not squash.
Verified by:
- New tests, each run against its mutation (temporary edit, reverted):
  tests/manual.rs a_failed_write_to_stdout_exits_2 (/dev/full → exit 2;
  fails when all errors are ignored) and
  a_reader_that_closes_early_is_not_an_error (read end closed before the
  write → exit 0, no panic; fails with println!);
  tests/own_writes.rs the_deletion_is_recorded_before_the_file_goes
  (read-only hook directory: exit 2, file still there, owned.json has
  op delete; the next capture writes nothing and forgets it; fails with
  delete-first); tests/hooks.rs uninstall::the_lock_covers_the_write
  (lock held → install and uninstall exit 4, file byte-identical; after
  the release uninstall works); install.test.sh: zsh directory without
  `zsh` → no completion and no hint, fake zsh → both, fish without its
  directory → none (dropping `command -v zsh` fails 3 checks).
- Suites: manual 7/7, own_writes 13/13, hooks 35/35, install test 132/132.
- `just check` → exit 0, "check: ok" (851 Rust tests passed over test +
  check-watch; docs-check ok; plugin checks ok).
- Still not run here: shellcheck, zsh -n, fish -n (not installed), makepkg
  (guard block).
Learned: a test for EPIPE must close the read end before the child writes
  (drop the pipe right after spawn); a short output fits the pipe buffer
  and never sees EPIPE otherwise.
Decisions needed: none.
Touched outside WP scope: none.
```
