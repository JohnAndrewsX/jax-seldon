WP-009 HANDOVER

Branch `wp/009-hooks`, worktree `wt/WP-009`, rebased on `main` at
`f7a9a13` (WP-007 and WP-015 merged). Not pushed, no PR. At HEAD
`just check` exits 0. The round-0 commits were rebased, so their hashes
changed (table below); the round-0 sections after "Review round 1" keep
their original text except where marked *(round 1)*.

| Commit | What |
|---|---|
| `e41fe6e` | `pkgcmd.rs` (parser moved, plus a shell lexer), `attribution.rs` (causes moved); pacman re-exports both |
| `2f0ca31` | `capture` runs the shared attribution pass before `append` |
| `8ad5344` | `commands/hook.rs`: `claude-code`, `generic`, `session-start`, `session-stop`, `install claude-code` |
| `7b6f400` | `tests/hooks.rs`, golden `tests/golden/session-start.txt` |
| `054988d` | `tests/attribution.rs` |
| `5b90d24` | `seldon event`: actor default `system`, `theme-set` `meta.from`, attribution pass; `engine/hooks/README.md` |
| `30199f1` | round-0 handover, memory |
| `e2e37bd` | review 1: `command -v`/`sudo -l` are probes (blocker) |
| `8d06ae9` | review 2: config proof only through write targets; `write_targets`, `simple_commands` in `pkgcmd.rs` |
| `e644913` | review 3–5: ADR-0019 green class, writer and nested-shell classification, panic/EPIPE safety, `SessionEnd` 60 s |
| `0742db9` | review 6: index rebuilds (hook: fast, only after a write; session-stop: full, after the commit); `index/drift.rs` imports `pkgcmd` |

Each commit was checked to build; `8d06ae9` was also checked alone in a
temporary worktree (`cargo check --all-targets`).

## Review round 1

1. **Blocker: `command -v yay` read as `yay`** (`e2e37bd`).
   - New `pkgcmd::is_probe`. `command -v|-V|-pv…` and `sudo -l|-ll|-v|-k|-K` (and their long forms) run nothing.
   - The hook's `command_argv` returns no program for them. The attribution's `command_intent` skips the segment, so they are never a full-upgrade cause.
   - `type`, `which`, `hash` and `whereis` are not wrappers, so their argument is never the program.
   - My round-0 test asserted the bug (`command -v x` → `x`); it is replaced.
   - Unit rows (`pkgcmd::tests::probes_run_nothing`):
     - `command -v yay`, `command -V pacman`, `command -pv yay`, `sudo -l pacman -Syu`, `sudo -v`;
     - `type yay`, `which pacman`, `hash yay`, `whereis paru`;
     - `command -v yay && yay -S zed` → package `zed`, no full upgrade.
   - Hook rows (`hook::tests::package_commands`): `command -v yay`, `command -v pacman`, `type yay`, `which pacman`, `command -v yay >/dev/null && echo ok` → nothing.
2. **Config proof only through write targets** (`8d06ae9`).
   - `pkgcmd::write_targets(argv, writes)` is shared by the hook and the attribution pass. It returns:
     - `File`: redirections, `tee`, `sed -i`, `mv` sources, `install -d`, `unlink`, `truncate`, a hook's `Edit|Write|MultiEdit <path>`;
     - `Into`: the destination of `cp|mv|install|ln`; it proves the path itself or a file directly in it;
     - `Tree`: `rm|rmdir`; it proves the path and everything below.
   - Words a command only reads never prove anything. Tests:
     - `attribution::rules::a_read_is_no_proof` is the reviewer's scenario (`cat ~/.config/hypr/bindings.conf && sudo pacman -S zed`, plus `grep`, a copy *out of* the file, `sed` without `-i`, `vim`) → `system`;
     - `rules::writers_prove_what_they_write`: `cp new.conf ~/.config/hypr/` proves `~/.config/hypr/new.conf` but not `…/sub/new.conf`; `mv` source; `rm -rf` dir; `truncate`; `bash -c`; `eval`.
   - The fixture reproduction and the end-to-end test still pass. This closes the round-0 "copy into a watched directory" gap.
3. **Hook classifier** (`e644913`):
   - `SessionEnd` timeout 60. `engine/hooks/README.md` says Claude Code allows at most 60 s for a `SessionEnd` hook.
   - An `mv` source inside a watched path is mutating (removal), and so is `install -d <watched>`. Both come through `write_targets`.
   - `pkgcmd::simple_commands` opens `sh|bash|zsh|dash -c '<script>'` (also `-lc`, `-o opt -c`) and `eval '…'`, up to 3 levels. The script goes through the same classifier and the same attribution proofs. An outer redirection stays a segment of its own, and `bash script.sh` is not opened.
   - `xargs`, `find -exec` and interpreters (`python -c`, `node -e`) are a documented follow-up (module doc and README).
4. **Panic and EPIPE** (`e644913`):
   - `run_agent_hook` installs a panic hook that writes `seldon hook: internal error: …` to stderr (ignoring write errors) and exits 0.
   - I did not use `catch_unwind`: the release profile has `panic = "abort"`, under which `catch_unwind` catches nothing. The panic hook runs before the abort, so it covers debug and release alike.
   - `session-start` writes its block with `write_all`/`flush` and ignores the error.
   - Tests:
     - `robustness::a_panic_exits_zero` uses a debug-only trigger, `SELDON_TEST_HOOK_PANIC`, compiled only under `debug_assertions`;
     - `robustness::session_start_survives_a_closed_pipe` closes the reader before the hook writes. Mutation check: with the old `print!` that test fails.
5. **ADR-0019 green class** (`e644913`), with the reviewer's scope:
   - Green, recorded only while a case is set (`.seldon/active-case`; on the generic hook `--case ID` or a `"case"` field, `--case` first):
     - a file writer whose targets are all outside `watchPaths`. `/dev/*` and the logbook itself are not news, so `cargo test 2>/dev/null` stays silent and writing the logbook's own notes is not recorded;
     - a foreign package manager (`npm pnpm yarn bun pip pip3 pipx uv cargo go`) with `install|i|add|remove|rm|uninstall|un|update|upgrade|up|get|ci`, `uv pip|tool <verb>`, or bare `yarn`;
     - a changing `git` sub-command outside `~/.config`.
   - `git` in the logbook stays recorded without a case (SPEC-ENGINE §8); its zone is now green instead of none.
   - `zone_for(Source::Agent, _)` = green. Red and yellow rules are unchanged, and a tracked change always wins over a green one in the same line.
   - An unknown explicit case is reported on stderr. A tracked command is still recorded without the case; a green one is dropped.
   - Tests:
     - `green::recorded_with_a_case`: `tee ~/.config/zed/settings.json` (the WP-015 fixture line), `npm install`, `git push origin main` in a project, `Edit ~/Work/notes.md` → green, all with the case; `tee -a ~/.bashrc` → yellow; `npm run build 2>/dev/null` → nothing;
     - `green::nothing_green_without_a_case`: only the yellow one;
     - `green::the_generic_hook_takes_a_case`;
     - unit rows `hook::tests::green_needs_a_case`.
6. **WP-007 wiring** (`0742db9`), after rebasing on `main`:
   - The hook calls `index::rebuild_if_initialised_fast(ctx)` once, at the end of `record`, under the lock. It runs only when an event was written; non-mutating calls and dropped green calls return before it.
   - `session-stop` calls `index::rebuild_if_initialised(ctx)` after its autocommit, under the lock (WP-007's convention). `capture --all` inside it already rebuilds before the commit.
   - `index/drift.rs` imports `crate::pkgcmd::{parse_command, split_logged}`.
   - The rebase conflicts in `main.rs`, `commands/mod.rs` and `memory/*.md` were additive; I kept both sides.
   - Test `index::a_hook_write_rebuilds_the_index`:
     - a non-mutating call leaves `index.json` byte-for-byte unchanged;
     - a mutating call puts the new event in `index.json` with no `git.dirty` (no git spawned);
     - `session-stop` leaves `dirty: false`.

### Verified by (round 1)

- `just check` → `check: ok` (exit 0), on this host with the host-only checks.
- `cargo test`: lib 84, `hooks` 28, `attribution` 10. All other binaries are green:
  - cli 17, collectors 14, collectors_user 23, commands 9, doctor 7, frontmatter 9, idempotency 6, init 15, journal 3, log 10, plan 12, redaction 6;
  - WP-007's index 16 and status 6.
- Release timing, 100–200 runs each, temp HOME, `env -i` (process spawn ≈ 0.65 ms):

  | Logbook | Mutating, with fast rebuild | Non-mutating |
  |---|---|---|
  | fresh | ≈ 3.1 ms | ≈ 1.1 ms |
  | fixture logbook copy (≈ 70 lines at the start, ≈ 200 after the run) | ≈ 3.8 ms | — |

  Before the rebuild was wired, a mutating call took ≈ 2.0 ms.

### Decisions needed (round 1)

1. **Hook rebuild cost grows with the ledger** (WP-007 decision 2). By WP-007's numbers and mine, the fast rebuild keeps a recording hook under 5 ms only up to roughly 500–700 ledger lines. Choose:
   - keep a rebuild per hook write (as now);
   - a deferred rebuild (the next `capture`/`status`; the plugin captures every 15 min);
   - an incremental index later.

   The switch is one line at the end of `hook.rs` `record`.
2. **Guard false positive #3** (same class as round-0 item 9). A `git commit -m` whose message text contained `… && pacman -S y` was blocked by the `[;&|]\s*pacman` rule. I did not re-run it in another form. Both review commits were made with `git commit -F <file>`, the message written with the Write tool (content, not an action); the final message also no longer contains that example.

### Settled by the review (no action)

`SessionEnd`, the PostToolUse fallback, `meta.sessionId`, the classifier extensions, `seldon event` attribution, the `meta.from` rule, sorted `settings.json` keys and PreToolUse/Edit/Write fixtures (schema track) are all settled. `pacman -Sy` recorded red is accepted.

## Done

- **`pkgcmd.rs`** — the command parser, moved out of `collectors/pacman.rs` with no behaviour change:
  - moved: `Op`, `PacmanCommand`, `parse_command`, `split_logged`, `is_plain_full_upgrade`, `is_mutating`, `Intent`, `command_intent`, `OMARCHY_UPDATE_NAMES`;
  - `pacman.rs` re-exports them (`pub use crate::pkgcmd::{…}`), so its unit tests and every WP-004 integration test are byte-for-byte unchanged and green;
  - the hook does not import `collectors::pacman`;
  - new: `parse_shell`, a read-only lexer for agent command lines:
    - handles quotes, `\`, `&& || ; | &`, newlines, `( )`, `$(…)` and backticks (kept inside their word), and comments;
    - records redirection targets (`> >> >| &>`) separately as `writes`;
    - cuts heredoc bodies out of the recorded text (a heredoc is the command's stdin);
  - new: `command_argv`, which skips `VAR=…` and the wrappers `sudo` (with options that take a value, e.g. `-u root`), `doas`, `env`, `command`, `exec`, `nohup`, `time`, `nice` and `timeout`.
- **`attribution.rs`** — one shared module:
  - moved from pacman: `ATTRIBUTION_WINDOW`, `Cause`, `causes`, `find_cause`. `Cause` gains `segments` (the argv words and the redirection targets per simple command).
  - pacman and omarchy keep their transaction logic in their collectors, unchanged.
  - New `attribute(events, known, home)` and `attribute_from_ledger`. They apply to config, theme and plugins events that still have `actor: system` and no case. A cause attributes an event only when:
    - its `ts` lies in `[event ts − 10 min, event ts]`;
    - the command proves the subject, per source:
      - **config:** *(round 1: only a path the command writes, see Review round 1 item 2)*, written as `~/…`, `$HOME/…`, `${HOME}/…`, absolute, or home-relative (normalised lexically). An `Edit`/`Write` hook event proves it through its `meta.command` `Edit ~/…`;
      - **theme:** `omarchy theme set <name>` or `omarchy-theme-set <name>`, slugged the way `omarchy-theme-set` does it (tags stripped, lowercase, spaces → `-`);
      - **plugins:** `omarchy plugin add|remove|enable|disable|update <w>` or `omarchy-plugin-<verb> <w>`, where `w` is the id or a URL/path whose last component is the id (trailing `/` and `.git` ignored);
    - the latest proving cause wins.
  - Called once by `capture` (one line, before `append`). `seldon event` also calls it; see Decisions needed 5.
- **`seldon hook claude-code`** (stdin):
  - **`PreToolUse` + `Bash`:** a mutating command becomes one `agent/command` event:
    - `ts` = now, which is when the command starts (ADR-0017 §1);
    - `subject` = the program word of the most severe mutating simple command (`yay`, `omarchy`, `sed`, `git`, …);
    - `zone` from ADR-0014 §2;
    - `meta.command` = the line with heredoc bodies cut, redacted, then cut to 4096 characters (redacted first, so a cut never leaves half a secret);
    - `meta.toolUseId` and `meta.sessionId` (scalars in `extra`);
    - actor `agent:claude-code`;
    - case from `.seldon/active-case`, but only if that case file exists.
    - The event id is attached to the case file's `events`/`agents`, as `seldon event --case` does.
  - **Classification** (`classify`, SPEC-ENGINE §8 plus Decisions needed 4):
    - pacman/yay/paru via `parse_command().is_mutating()`, so query forms are never mutating (ADR-0017 §5);
    - `omarchy` routes and `omarchy-*` scripts:
      - mutating: `update` (and the `update-system|aur|keyring|firmware|orphan|pkg|mise` scripts), `pkg add|install|drop|remove|aur add|aur install`, `install …`, `remove …`, `reinstall`, `plugin add|clone|enable|disable|remove|update`, `theme set|install|remove|update`;
      - queries (`list`, `current`, `update available`, `pkg present`, …) are not mutating;
    - `systemctl enable|disable|start|stop|mask|unmask`;
    - `cp|mv|install|ln` whose target (`-t`, `--target-directory`, or the last operand) is watched;
    - `tee` operands, `sed -i/--in-place` files, and `rm|rmdir|unlink|truncate` (also a path that *contains* a watched path);
    - any redirection into a watched path;
    - `git` with a changing sub-command (`add commit push pull checkout switch reset restore merge rebase …`) whose directory (cwd, `-C`, `--work-tree`, `--git-dir`) lies in the XDG config home or the logbook. `status`/`log`/`diff` are not mutating; the non-mutating fixture needs exactly that.
    - "Watched" = `config.toml watchPaths` plus `~/.config/systemd`. Relative paths resolve against the payload's `cwd` and any earlier `cd` in the line.
  - **`PreToolUse` + `Edit|Write|MultiEdit`** (ADR-0014 §4):
    - a watched `file_path` gives `subject: edit|write|multiedit`, `meta.command "<Tool> ~/…"` and the config zone;
    - a path matching `[redaction] skipPaths` (`collectors::config::SkipPaths`) is recorded as `‹redacted›`;
    - only the path is read, never `content`/`old_string`/`new_string`.
  - **`PostToolUse`:** writes nothing when its `tool_use_id` is already in the ledger. If it is not, the command is recorded with the arrival time (Decisions needed 2).
  - Non-mutating commands and other hook events produce nothing. `tool_response` is never deserialised.
  - The hook waits up to 2 s for the state lock (a running capture), then gives up on stderr.
- **`seldon hook generic`** — stdin `{"command","actor","cwd","startedAt"?}`, the same classification:
  - `ts` = `startedAt`, else now;
  - `actor` must be `human` or `agent:<name>`;
  - case from `.seldon/active-case`.
- **`seldon hook session-start`** prints the context block, checked by a golden test against the fixture logbook with active case C-2026-004:
  - STATUS summary: the first section of `STATUS.md`, without the generated header and blank lines;
  - the active case: one line (id, title, status, zone/risk, done/total steps), then its `## Plan` checkboxes;
  - the last 5 non-blank lines of the newest journal day ≤ today;
  - the `##` headings of `memory/lessons.md`.
- **`seldon hook session-stop [--actor agent:NAME]`** (default `agent:claude-code`):
  1. Appends `## HH:MM · agent:NAME · CASE` / `session ended; N events recorded` to today's journal, under the lock. N = ledger events of that actor carrying the stdin `session_id` (no session id: the actor's events today).
  2. Runs `capture --all`.
  3. *(round 1)* Runs the full index rebuild after the commit (Review round 1, item 6).
  4. Autocommits `seldon: session ended (agent:NAME)`.
  - Every step runs even if an earlier one failed; failures go to stderr.
- **Agent hooks always exit 0 and stay silent** (`run_agent_hook`, dispatched in `main` before `run`). Covered:
  - malformed stdin;
  - a missing logbook;
  - a broken `config.toml`;
  - a bad `SELDON_NOW` (the `Context` is built inside the catch);
  - a held lock.
  - Only `session-start` prints, and only its block.
- **`seldon hook install claude-code [--settings PATH]`** — default `<logbook>/.claude/settings.json`; adds what is missing:
  - `PreToolUse` with matcher `Bash|Edit|Write|MultiEdit` → `seldon hook claude-code`, timeout 10 s;
  - `SessionStart` → `seldon hook session-start`, timeout 10 s;
  - `SessionEnd` → `seldon hook session-stop`, timeout 60 s *(round 1; was 120)*.
  - Existing hooks and keys stay. Idempotent: a second run leaves the file byte-for-byte unchanged.
  - Prints `added`/`present` per entry (`--json`: `{settings, added, present, git}`).
  - Refuses (exit 1, file untouched) invalid JSON, a non-object root, `hooks` that is not an object, or an event entry that is not a list.
  - Autocommits when it wrote into the logbook.
- **`seldon event`:**
  - the default actor is `system`;
  - `theme-set` without `--meta from` gets `meta.from` (plus `meta.to` = subject and `detail` `from → to` when not given). The source is the theme cursor, or the newest ledger `theme-set` when that is newer than the cursor's `checked`; it is never the subject itself (Decisions needed 6).
  - The WP-006 test that reproduces the fixture's `human` theme-set line now passes `--actor human` explicitly. This one-line change in `tests/commands.rs` is the only edit to an existing test.
- **`engine/hooks/README.md`** — the Claude Code hooks table, and the theme hook's new actor and `meta.from` behaviour.

## Not done

- **`seldon hook install generic`** (SPEC-ENGINE §3 lists it). Only `claude-code` is accepted; other agents call `hook generic` themselves (README).
- ~~Index/status rebuild in `session-stop`.~~ *(round 1: done, Review round 1 item 6.)*
- **Case files and attributed collector events.** Collector events attributed by the pass (or by pacman) are not added to the case file's `events`. `capture` never did this, and the fixture's C-2026-004 lists them, so this belongs to reconciliation (WP-008).
- **Relative paths in a recorded command** cannot prove a config change later: a hook event stores no cwd, so e.g. `sed -i … bindings.conf` after `cd ~/.config/hypr` in an earlier tool call does not count.
- **`config-remove` events** are timed at capture time (WP-005), so they rarely fall inside the 10-minute window.
- ~~A copy *into* a watched directory is not attributed.~~ *(round 1: closed, an `Into` target proves files directly in the directory.)*

## Verified by

- `just check` → `check: ok` (exit 0): fmt, clippy `-D warnings`, all tests, schema-validate, plugin-validate, qmllint, plugin-test. The host-only checks ran on this machine.
- `cargo test`: lib 73; `hooks` 22; `attribution` 8. All earlier binaries are unchanged and green:
  - collectors 14, collectors_user 23, idempotency 6, redaction 6, commands 9, cli 17, plan 12, log 10, init 15, doctor 7, frontmatter 9, journal 3.
- Acceptance mapping:
  - **Fixtures classify** (`hooks::claude_code::*`):
    - mutating → one event `yay`, `yay -S --noconfirm zed`, `agent:claude-code`, active case, red;
    - non-mutating → nothing, as Pre and as Post;
    - secret → `git`, `‹redacted›`; none of `AKIAIOSFODNN7EXAMPLE`, `ghp_EXAMPLE`, `user:`, `hunter2`, `sk-EXAMPLE` or the `tool_response` text appears anywhere in the ledger file;
    - plus Edit/Write/MultiEdit, skipPaths, heredoc and PostToolUse-dedupe cases.
  - **Always exit 0:** `hooks::claude_code::always_exits_zero`, `generic::bad_input_is_reported_not_raised`, `claude_code::a_held_lock_is_waited_for`.
  - **`session-start` golden:** `sessions::session_start_prints_the_context_block`, against `tests/golden/session-start.txt`.
  - **`hook install`:** `install::merges_into_existing_hooks_and_is_idempotent` uses a settings file that already holds a `PreToolUse` guard, a `Stop` hook, `permissions` and `model`. Also `default_path_is_the_logbook` and `refuses_a_broken_file`.
  - **Attribution:**
    - `attribution::fixture::reproduces_the_fixture_ledger` resets every agent-attributed config/theme/plugins event of the fixture ledger to `system` and runs the pass over the whole fixture ledger. The result equals the fixture, including the 10-01 `bindings.conf` change (`agent:claude-code`, C-2026-004, via `sed -i`) and Codex's `ollama.service` (`tee`, no case). Exactly 2 events are reset, so the test is not vacuous.
    - `rules::*`: proximity without the path, exactly 10 min in / 10 min 1 s out, a command after the event, the latest proving cause wins, slug and plugin-id rules, pacman events and already-cased events untouched, `system` commands never attribute.
    - `end_to_end::a_hooked_sed_attributes_the_config_change` runs `seldon hook claude-code` → file change → `seldon capture`. The config change and the theme switch get the agent and case; an unnamed change stays `system`.
    - The ADR-0017 pacman scenarios in `tests/collectors.rs` are unchanged and green.
  - **Tests never touch the real XDG dirs:** everything goes through `common::Env` (temp HOME, PATH = stub dir, `SELDON_PACMAN_LOG`/`SELDON_THEME_FILE`/… pointing at temp files).
- **< 5 ms:** release build, temp HOME, 200 runs each:
  - mutating PreToolUse ≈ 2.0 ms wall per call;
  - non-mutating ≈ 1.1 ms;
  - spawn baseline (`env -i … true`) ≈ 0.65 ms.
  - The debug test `claude_code::fast_enough` asserts a generous 400 ms.
- **Payload shapes** come from the Claude Code hooks reference (code.claude.com/docs/en/hooks), checked read-only through a docs lookup:
  - PreToolUse carries `tool_use_id`;
  - `Stop` fires after every reply, `SessionEnd` once at session end;
  - SessionStart stdout (exit 0) is injected as context;
  - matchers are regexes and optional for SessionStart/SessionEnd.
  - The lookup was inconsistent about MultiEdit's input shape, so the hook reads `file_path` and, defensively, `edits[]`/`file_edits[]` `.file_path`.

## Learned (→ memory/)

- `memory/rust-notes.md` § WP-009:
  - a dispatch that must never fail is done before `run()`, with the Context built inside the closure;
  - re-exports keep moved code's tests untouched;
  - `slice::from_mut`, `rfind`;
  - serde_json sorts keys without `preserve_order`;
  - `IsTerminal` on stdin;
  - the release timings.
- `memory/pitfalls.md` § WP-009:
  - `Stop` vs `SessionEnd`;
  - the fixtures are PostToolUse;
  - guard false positives on heredoc *content*;
  - a recorded command has no cwd;
  - heredocs are stdin;
  - the theme-set hook makes its event final;
  - `~/.config/systemd` as a directory.

## Decisions needed

1. **`SessionEnd` instead of `Stop`.** The WP and SPEC-ENGINE §8 say `Stop`, but Claude Code runs `Stop` after every assistant reply. That would write one journal stub, one capture and one commit per reply. `hook install` writes `SessionEnd` (one constant, `CLAUDE_HOOKS`). Please confirm, and amend SPEC-ENGINE §8 (it also still says "reads the PostToolUse JSON"; ADR-0017 says PreToolUse).
2. **PostToolUse fallback.** A PostToolUse whose `tool_use_id` is not in the ledger is recorded with its arrival time (an end time, so it rarely attributes). It covers settings files with only a PostToolUse hook, and the fixtures, which are PostToolUse. Alternative: PostToolUse never writes. Keep it?
3. **`meta.sessionId`.** Besides `meta.toolUseId` (ADR-0017 §1), hook events carry `meta.sessionId` so `session-stop` can count this session's events. It is a scalar in `extra`, so there is no schema change. Keep, or count differently?
4. **Classifier beyond SPEC-ENGINE §8** (the spec needs an amendment either way):
   - redirections into watched paths;
   - `ln`, `rm|rmdir|unlink|truncate`;
   - `systemctl unmask`;
   - the `omarchy remove|reinstall` groups;
   - `~/.config/systemd` always counted as watched (red by ADR-0014 §2, even when not in `watchPaths`);
   - `git` only for changing sub-commands (required by the non-mutating fixture's `git status`);
   - "`git` inside `~/.config`" read as the XDG config home.
5. **`seldon event` runs the attribution pass** (outside "actor default + theme meta.from only"). Without it, an agent's `omarchy theme set` stays `system` whenever `theme-set.sh` is installed: the hook writes the event first, and the theme collector then skips it as already recorded. One call in `event.rs`, test `hooks::event::an_agents_theme_switch_is_attributed`. Keep, or revert and accept the gap?
6. **`theme-set` `meta.from`.** Implemented as "cursor, unless the ledger holds a newer `theme-set`". The literal rule "cursor, else ledger" gives the wrong `from` after two switches between captures. It also fills `meta.to` and `detail` (`from → to`) when not given, matching the fixture and collector lines. Confirm.
7. **`.claude/settings.json` key order.** serde_json without `preserve_order` writes the merged file with its keys sorted; all values are kept. `preserve_order` needs `indexmap`, which is not on the allowed crate list. Accept, or approve `indexmap` via that feature?
8. ~~WP-007 hand-off.~~ *(round 1: done.)*
9. **Guard false positives**, reported, not worked around. Two Bash calls in this WP were blocked by `scripts/guard.sh` because of file *content* in a heredoc, not because of an action:
   - (a) a doc comment "…(`attribution.rs`; pacman and omarchy …)" matched `[;&|]\s*pacman`;
   - (b) Rust test strings with `> ~/.config/hypr/a.conf` matched the `~/.config` write rule.
   - Neither command was re-run in another form. The same file content was written with the Edit tool, which `memory/guard-block-stop.md` allows for content; the `cargo` steps ran as separate commands.
   - Suggest guard-test rows for both patterns, or a rule that skips heredoc bodies.
10. **Fixtures (WP-015).** `fixtures/hooks/` holds only PostToolUse Bash payloads. PreToolUse variants plus one `Edit` and one `Write` payload would let the tests use the fixtures without rewriting `hook_event_name`.

## Touched outside WP scope

- `engine/src/commands/capture.rs`: one call to the attribution pass (requested by the WP notes; the import and module-doc lines come with it).
- `engine/src/main.rs`: the `Hook` variant, its dispatch arm, and the exit-0 pre-dispatch for agent hooks.
- `engine/src/commands/mod.rs`: `pub mod hook;`.
- `engine/tests/commands.rs` (WP-006): `--actor human` added to one test, because the default actor changed as the WP asks.
- `engine/src/commands/event.rs`: the attribution call (Decisions needed 5) besides the actor default and `meta.from`.
- Not touched: `index/`, `commands/{index,status}.rs`, `schema/`, `fixtures/`, `docs/`, `decisions/`, the plugin.
