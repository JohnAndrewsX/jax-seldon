WP-063 HANDOVER

hook: recording scope and path exclusions for agent commands. Branch
`wp/063-review`, worktree `wt/WP-063`. Commits: `6545ee7` (engine +
tests), `b8891ae` (SPEC-ENGINE §8, README, CHANGELOG), plus this
handover. No PR, no push.

Done:

- **Session scope (`engine/src/commands/hook.rs`, `hook/context.rs`,
  `engine/src/config.rs`).** `hook claude-code`, `hook generic`, `hook
  session-start` and `hook session-stop` do nothing (no event, no context
  block, no journal line, no capture, no commit; exit 0, silent) when the
  payload's `cwd` is not the logbook or a directory below it. The check
  compares components (`/x/Seldon-other` is outside `/x/Seldon`), folds
  `.`/`..`, and also compares with symbolic links resolved as far as the
  path exists, so a logbook reached through a link counts. A `cwd` that
  is not an absolute path (also `""` or a non-string) is outside. A call
  whose stdin names no `cwd` — an agent or a person running the hook,
  `hook generic` without the field, `session-start` from a terminal — is
  served as before.
- **The switch.** `config.toml [hooks] scope = "logbook" | "all"`
  (`HooksConfig`, `HookScope`). Default `"logbook"` as the WP names it;
  `"all"` restores the previous behaviour (every session served). The
  section is not written to the file while it is the default (like
  `[agent]`). Flipping the default is a one-line change
  (`#[default]` on `HookScope`).
- **`hook install` warning.** A settings file outside the logbook (any
  `--settings` path not inside it, `~` expanded, relative paths made
  absolute) gets one warning, also when nothing was added: human report
  `warning: this settings file is outside the logbook (<root>). …`, and
  `--json` gains `warnings` (array, `[]` otherwise). The text depends on
  the scope: under `"logbook"` it says Seldon records and prints context
  only for sessions inside the logbook; under `"all"` that it records the
  commands of every session that reads the file and prints the context
  there.
- **`skipPaths` on command lines.** `bash_record` (used by `hook
  claude-code` and `hook generic`) records a mutating line that names a
  `[redaction] skipPaths` path as `<program> ‹redacted›`, where
  `<program>` is the event's subject (the same form as `Write
  ‹redacted›`). Read as paths: write targets, every word of every simple
  command (`sh -c`/`eval` strings opened), every token of the line text
  between blanks, quotes and shell operators (so files read with `<` and
  words inside `$(…)`/backticks count), each also after its first `=`
  (`VAR=PATH`, `--opt=PATH`); relative words against the payload `cwd`
  and every `cd` target in the line. Over-matching by design, like the
  redaction rules. No cost when `skipPaths` is empty. `SkipPaths` is now
  built once in `setup()` and shared with `edit_records`.
- **Follow-up (a), PostToolUse.** The already-recorded check moved from
  `claude_code` into `record()`, after `lock_patiently` (field
  `Entry::unless_recorded`). A lock still held after the 8 s wait is
  reported as `seldon hook: another seldon process holds the lock <path>;
  command not recorded`.
- **Follow-up (b), `plan show`.** Applied. Human output: `Case <id>: its
  file's path and text.`, the `DATA_NOTE` line of session-start, then the
  relative path and the rendered case file, each line through
  `context::quote` (`> `, every line-break kind splits, control chars →
  U+FFFD). The path is quoted too because a case file name after
  `<id>-` is free text. `--json` unchanged. `quote` and `DATA_NOTE` are
  re-exported from `commands::hook`.
- **Tests (`engine/tests/hooks.rs`).** New modules: `skip_paths` (12
  lines that write or read a skipped file — redirect, `tee -a`, `sed -i`,
  heredoc, `cat … >`, `sort <`, `cp "${HOME}/…"`, `$(cat …)`, `VAR=PATH`,
  `cd … && cp rel`, `bash -c '…'`, a name pattern — each recorded as
  `<program> ‹redacted›`, none of the path or value text in the ledger;
  other lines unchanged, including a `.bak` sibling; the generic hook),
  `session_scope` (outside: six `cwd` forms, Pre/PostToolUse, Write,
  generic, session-start prints nothing, session-stop writes no journal
  line and no commit; inside: a subdirectory records 4 events, the
  context prints with and without `cwd`, session-stop journals; a link to
  the logbook counts; `scope = "all"` serves an outside session; install
  warning for `~/.claude/settings.json` in JSON and the human report, none
  inside or for the default path, the `"all"` wording), `post_tool_use`
  (two PostToolUse calls for one tool call while the lock is held → one
  event; a lock held for the whole wait → stderr has "command not
  recorded", no event), `plan_show` (first two lines, every other line
  quoted after splitting at every line-break kind, the path line, a
  made-up heading behind U+2028 and `\r` stays quoted, `--json` body
  raw). Unit test `config::tests::hook_scope` (parse, unknown value
  refused, default not written).
- **Existing tests adapted.** The fixtures and `tool_call()` name
  `/home/user/Seldon` as the session directory; `FIXTURE_CWD` documents
  that it stands for the test logbook, and `Hooks::piped`/`spawn_hook`
  put the logbook path in its place; `payload()` sets every fixture's
  `cwd` to it (the secret fixture's `~/.config/hypr` session would now be
  outside; its command uses absolute paths). Generic tests use the
  logbook as `cwd`. The `green` module (sessions in `/tmp/project`) runs
  with `scope = "all"`, which also pins that `"all"` keeps the old
  behaviour. New helper `Hooks::configure`.
- **Docs.** SPEC-ENGINE §8: PostToolUse check under the lock, skipPaths
  on command lines, the held-lock message (PreToolUse paragraph); a new
  "Session scope" paragraph with both values of `[hooks] scope`; the
  install warning (install paragraph); session-start's scope clause.
  README hook bullet. CHANGELOG `[Unreleased]` › Engine: two bullets, the
  first calls out the change for existing user-wide installs.

Not done:

- Manual scratch-HOME run: the guard hook blocked it (see Decisions
  needed). Nothing ran. The integration tests make the same calls in a
  scratch HOME with made-up values.
- Paths named only through a glob (`cat ~/.config/hypr/*`) or through a
  parent directory (`cp -r ~/.config/hypr /backup`) are recorded as
  written: neither names the skipped file itself.
- Release-mode timing not re-measured. The added work per call is one
  path comparison (plus `canonicalize` calls only when the lexical
  comparison fails) and, only with `skipPaths` set, one regex pass over
  the line's words; `fast_enough` passes.
- No change to user-guide pages, AGENT-GUIDE, SPEC §2 or §3 (outside the
  file list; see Decisions needed).

Verified by:

- `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`,
  `cargo test` (all engine test binaries): ok.
- Before adapting the tests, the unchanged `tests/hooks.rs` failed 19
  tests with the new default (fixture `cwd` outside the test logbook),
  which is the expected behaviour change.
- Mutants, each reverting one part (sources restored with `cp`,
  checked with `cmp`, binary rebuilt), each failing its test:
  M1 no skipPaths check in `bash_record` → `skip_paths::{a_line…,
  the_generic_hook_too}`; M2 no token pass over the line text → `sort <`
  and `$(cat …)` rows; M3 no `cd` directories → `cd … && cp` row; M4 no
  value after `=` → `VAR=PATH` row; M5 `in_scope` always true, M8 no
  guard in session-start, M9 none in session-stop, M10 none in generic,
  M11 none in claude-code → `session_scope::sessions_outside…`; M6
  string prefix instead of path components → same test, on the
  `<logbook>-other` cwd; M7 no link resolution →
  `session_scope::a_logbook_reached_through_a_link_counts`; M12 no
  install warning → `session_scope::install_warns…`; M13 PostToolUse check
  before the lock (old placement) → `post_tool_use::two_calls…` (2
  events); M14 old held-lock message → `post_tool_use::a_lock_held_too_long…`;
  M15 old `plan show` → `plan_show::the_case_file_is_quoted`.
- `just check`: see the last line of this file.

Learned (memory/pitfalls.md): a hook test whose payload names a session
directory needs it inside the test logbook now; the guard hook reads
literal `~/.config/…` text in a manual run's payload as a write.

Decisions needed:

- **Scope default (operator).** Implemented with `"logbook"` as the
  default; `"all"` is the switch. Flipping means `#[default]` on
  `HookScope::All`, the CHANGELOG bullet and the README sentence.
- **User guide, en + de (`docs/user/*/04-working-with-agents.md` at
  "The hooks only run when Claude Code starts in the logbook folder…",
  and `11-update-and-uninstall.md` if wanted).** The en text says a
  user-wide install records every Claude Code session in any project;
  under the `"logbook"` default that needs `[hooks] scope = "all"` in
  `config.toml`. Proposed en replacement for the last three sentences:
  "Then Claude Code runs the hooks in every folder. Seldon records and
  prints its context only in sessions inside the logbook unless you set
  `[hooks] scope = "all"` in `~/.config/seldon/config.toml`; with it,
  every Claude Code session on this machine writes into your logbook, in
  any project, with the active case." Not edited: outside the file list,
  and the wording depends on the default.
- **SPEC-ENGINE §2 and §3.** §2's config key list lacks `[hooks] scope`;
  §3's `seldon plan show <ID>` line could say "human output quoted line
  by line (`> `), as `hook session-start`; `--json` unchanged". Both
  outside the named paragraphs; §8 documents both.
- **Guard false positive.** `scripts/guard.sh` blocked a manual run
  whose `HOME` was a scratch dir under `SELDON_TEST_GUARD`, because the
  command held `mkdir -p $HOME/.config/hypr` and payload text with
  `~/.config/hypr/…`. Reported, not worked around; a fix belongs in
  `scripts/guard.sh` with a row in `scripts/guard-test.sh`.
- **Session-stop included.** The WP names claude-code, generic and
  session-start; session-stop got the same guard so that a user-wide
  install does not journal, capture and commit for sessions elsewhere.
  Revert is one `if`.
- **`plan show` test location.** In `tests/hooks.rs` (the file list);
  `tests/plan.rs` would be the natural home.

Touched outside WP scope: `engine/src/config.rs` (the `[hooks]` section;
the WP lists config.rs as read-only, the brief asks for the config
switch), `engine/src/commands/plan.rs` (follow-up b, allowed by the
brief), `context.rs` beyond the entry guard (`quote` and `DATA_NOTE`
made `pub` for `plan show`).

`just check` on `b8891ae` (engine + docs): exit 0 (`check: ok`).
