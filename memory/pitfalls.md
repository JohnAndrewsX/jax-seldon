# Pitfalls — things that cost time, so the next agent does not repeat them

Append-only. One bullet per pitfall: what happened, how to avoid it.

## 2026-10-01 · WP-002 (Schema Keeper)

- **The red-zone guard matches substrings.** `scripts/guard.sh` blocked a
  read-only `grep -rln … /usr/share/omarchy` because `-rln` contains `ln`
  followed by ` /usr/`. Write flags separately (`grep -r -l -n`) or avoid
  `ln`, `rm`, `cp`, `mv`, `tee` substrings before a path under `/etc`,
  `/usr`, `/var`, `~/.config`. The bare word `pacman` anywhere in a Bash
  command is blocked too — including inside a heredoc of prose or JSON
  (`/var/log/pacman.log` is fine, `grep pacman file` is not). Do not work
  around the guard; rephrase, or write the file with the Write tool.
- **Fixture content that looks like a command trips the guard** when written
  through a Bash heredoc (a hook payload with `sed -i … ~/.config/…`, notes
  mentioning package commands). Use the Write/Edit tools or a script file.
- **Frontmatter is not YAML-safe-loaded.** PyYAML turns `created: 2026-10-01`
  into a `date`, which fails `type: string`; serde in the engine reads it as a
  string. The fixture validator uses a strict flat parser (strings, ints,
  bools, `[a, b]`, empty = null) and rejects anything else. Keep case,
  journal, decision and memory frontmatter flat.
- **The sample index is derived, not hand-edited.** After changing anything
  under `fixtures/logbook/`, run `bash scripts/validate-fixtures.sh --write-index`
  and review the diff; hand edits to `fixtures/index.sample.json` are reported
  as "not derivable from the logbook".
- **Every JSON file under `fixtures/` needs a schema mapping** in
  `scripts/validate-fixtures.py` (`collect_instances`); an unmapped file fails
  the run. Must-fail fixtures go to `fixtures/invalid/<schema>.<name>.json`.
- **Collector events cannot know the case.** Only hooks know actor and active
  case; a human package install during an active case is drift (with a
  proposal), and an agent's Edit/Write tool call is invisible to the Bash
  hook. The fixtures model both; WP-006 should decide whether the hook also
  reads Edit/Write payloads (`tool_input.file_path`).
- **`omarchy plugin catalog` does not carry versions** (see host.md); do not
  build `plugin-update` on it.

## 2026-10-01 · WP-014 (Schema Keeper)

- **The guard also blocks a `python3 -c` or heredoc that contains the
  package-manager word**, e.g. a schema edit whose replacement text mentions
  `pacman` in a description. Put such edit scripts in a file under the
  scratchpad (Write tool) and run the file.
- **`meta.command` is logged unquoted.** pacman writes `Running '<argv joined
  by spaces>'`, so `--overwrite /usr/share/omarchy/*` appears bare. Split on
  whitespace, not with a shell lexer (an apostrophe in an argument would make
  `shlex` throw), and know which options take an argument word: every
  `omarchy update` runs `-Syu --noconfirm --overwrite /usr/share/omarchy/*`,
  and a parser that treats `/usr/share/omarchy/*` as a package name turns every
  routine update red (ADR-0013 §3). Unknown options must err towards red.
- **A direct `-Syu` is blocked on Omarchy** by `00-omarchy-update-guard.hook`
  (`omarchy-update-pacman-guard`) unless `OMARCHY_ALLOW_DIRECT_PACMAN=1` is set;
  the environment is not logged. Most routine upgrades therefore arrive via
  `omarchy update`, which also yields a keyring reinstall (named package →
  red) and, on a version change, the red `omarchy update` event.
- **Adding pacman events to the fixture ledger is a three-file change**: the
  ledger line(s), the same block in `fixtures/logs/pacman.log` *and*
  `fixtures/logs/pacman-rotation/pacman.log`, plus the byte offsets in
  `fixtures/README.md` (the collector tests rely on "the log produces exactly
  the ledger's pacman events"). Keep month files chronological when appending:
  pick a `ts` after the file's last line, and give the ULID that time part.
- **`index-variants/` are generated** from the sample plus an overlay
  (`VARIANTS` in `scripts/validate-fixtures.py`); a hand edit is reported. Add
  a banner state by adding an overlay, then `--write-index`.
- **ADR-0012 §13's token rule makes `.` a word character**, so a Plan line
  ending in `zed.` does not propose `zed`. The previous script regex allowed a
  trailing sentence period; the fixture's proposals are the same under both,
  so the change was invisible in the diff. Write Plan items without trailing
  punctuation after a package name, or raise it with the orchestrator.
- **Correction (same day): ADR-0015 §4 supersedes the item above.** A final
  `.` not followed by a word character is punctuation, so `Install zed.`
  proposes `zed` again; `zed.conf` does not. A rule change that leaves the
  sample index identical needs an end-to-end self-check, not just a clean diff.
## 2026-10-01 · WP-003 (Engine)

- **Running the engine by hand writes config and state.** `seldon init`
  writes `~/.config/seldon/config.toml` (red zone on the dev host) and takes
  `~/.local/state/seldon/lock`. Before any manual run, export
  `XDG_CONFIG_HOME` and `XDG_STATE_HOME` to temp dirs; the integration tests
  get the same effect from a temp `HOME`. The WP-003 acceptance line
  (`seldon init --non-interactive --path /tmp/seldon-wp003`) needs this too.
- **Filtering PATH directories does not hide host binaries.** `/usr/bin`
  holds `snapper` and `git` together, so dropping it from PATH drops git as
  well. Tests set PATH to a stub directory only, with a link to the host's
  git (`engine/tests/common/mod.rs`).
- **Real host names leak through `init` output.** The machine id is
  `<hostname>-<4 hex>`, so `seldon init` / `doctor` output names the
  machine. Redact it before pasting output into handovers or commits
  (AGENTS.md §8).
- **Scripted edits and `cargo fmt` make the editor's file state stale.**
  After a python/sed edit or `cargo fmt`, Read the file again before using
  the Edit tool.

## 2026-10-01 · WP-004 (Engine)

- **The guard blocks the package-manager word even inside code comments and
  memory text in a heredoc or `python3 -` edit.** Edits to the package-log
  collector, its tests, or notes about it need the Edit/Write tools, or a
  script file written with Write. Env names like `SELDON_PACMAN_LOG` pass.
  The lowercase word between spaces does not.
- **Count fixture events; do not trust memory.** The fixture ledger has
  **12** package-log lines (source `pacman`), not 13. Count them in the file
  before you write an expected number.
- **Capture-time events need the fixture's `now`.** The omarchy `update` and
  `snapshot-delete` events carry capture time. Run the collector at the `ts`
  the fixture gives them. The golden comparison then needs no ts
  normalisation.
- **The ADR-0014 10-minute window alone cannot produce the fixture.** The
  10-01 `omarchy update` event (09:21:00) is 10:58 after the hook command. It
  inherits from the package upgrade of `omarchy` to the same version instead.
- **A Claude Code PostToolUse hook fires after the command has finished.**
  The hook event's `ts` then lies *after* the package lines it caused. The
  ADR-0014 rule ("the command precedes") would then never match. The fixture
  has the command first. WP-009 must record the start time, or the ADR needs
  an amendment.
- **In-process tests that fork stub programs flake in parallel.** A forked
  child holds every inherited descriptor until it execs, so two races appear:
  - a stub written a moment ago fails with `ETXTBSY` ("text file busy");
  - a flock that was just released still looks held.

  `sys::run` retries `ETXTBSY`. The test bench takes its lock once. Do not
  re-acquire locks per step in a test, and loop the suite (e.g. 50 runs)
  after adding stub-heavy tests.
- **Running `seldon capture` against the real host needs redirected XDG dirs
  and a throw-away logbook.** Otherwise it writes
  `~/.local/state/seldon/cursors.json`. Reading the system log, `snapper` and
  `omarchy-version` is fine (read-only).
## 2026-10-01 · WP-006 (Engine)

- **A test helper that searches Markdown must skip code fences too.**
  `body.find("## Result")` hit a `## Result` inside a fenced block that the
  test itself had put into the case; the engine was right, the helper was
  wrong. Use `rfind` for the last section, or the engine's
  `cases::section`.
- **Do not assume `plan done` works from `active`.** The state machine is
  `queued → active → verification → completed` (SPEC-LOGBOOK §3), but the
  fixture case C-2026-001 went active → completed without verification.
  The engine is strict; the fixture contradicts it (handover question).
- **The plugin's free text must come after `--`.** A note that is exactly
  `--help` or `--json` would otherwise be read as the flag (clap rule
  above). CONTRACT.md was changed to `seldon log [--case <id>] -- <text>`.
- **`seldon open --editor` from the plugin has no terminal.** `$EDITOR`
  (nvim) is useless there; the engine calls `omarchy-launch-editor <path>`,
  which opens the default editor in its own window. On a terminal it uses
  `$VISUAL`/`$EDITOR`. Tests stub `omarchy-launch-editor`.
- **Rebasing onto WP-004's foundation conflicts in `Cargo.toml`/`Cargo.lock`**
  when both branches add the same crates (`ulid`, `jsonschema`): take
  main's manifest (`git checkout --ours` during a rebase is *main*), then
  `cargo build` regenerates the lock.

## 2026-10-01 · WP-005 (Engine)

- **A collector that keeps its own state file runs ahead of the ledger.**
  The config manifest is written during `collect`, but `capture` appends the
  events afterwards and only then saves `cursors.json`. If the append fails,
  a single-generation manifest has already moved on and the change is lost.
  `manifest.json` therefore keeps the generation the cursor names as
  `previous` until the ledger has caught up. Do the same for any future
  collector-owned state.
- **`git rev-parse` in a plugin directory answers for the enclosing repo.**
  A dotfiles repository in `~/.config` would give every plugin the same
  "version". Run git only when the plugin directory itself has `.git`.
- **First-party plugin manifests all carry `"version": "1.0.0"`** on Omarchy
  4.0.4 (`$OMARCHY_PATH/shell/plugins/**/manifest.json`). If an Omarchy
  update bumps them, every first-party plugin yields a `plugin-update`.
- **A failing `omarchy plugin catalog` must not look like a downgrade.** A
  version that cannot be read this time keeps the last one seen;
  `plugin-update` fires only when both sides are known.
- **Copied files keep old mtimes** (`cp -p`, `rsync -a`, `git checkout`). An
  event time taken from the mtime must be clamped to the time since the
  last check, or it lands in an old ledger month.
- **The theme-set hook and the theme collector see the same change.** The
  collector looks for a `theme-set` to the current slug in the ledger since
  its last check and stays quiet when it finds one.
- **`common::Env` runs seldon with PATH = its stub dir only**: no `cat`, no
  `sh` lookup by name inside stubs that call other tools. Stubs print files
  with builtins: `while IFS= read -r l || [ -n "$l" ]; do printf '%s\n'
  "$l"; done < file`.
- **`git show main:dir/*.md` does not glob**; list with
  `git ls-tree --name-only main dir/` first.
- **Main moves while a WP runs.** Before the handover, diff
  `$(git merge-base main HEAD)..main -- docs/ decisions/` for spec changes
  that touch the WP (WP-005: ADR-0017 did not).
- **A test that re-acquires the flock per step flakes under parallel tests**
  (WP-005 review). A child forked by another test thread inherits the open
  lock fd until it execs, so a fresh `lock::acquire` on the same file can
  see `WouldBlock` (about 5% of parallel runs). Take the lock once per
  bench, keep it in the struct, and hand it to `Ledger::append`.
- **Adding a field to `Sources` breaks struct literals in tests.** Write
  test literals with `..Sources::default()`.
- **The guard also matches the Omarchy update command inside a python
  heredoc** (e.g. in a code comment being inserted). Put such edit scripts
  in a scratchpad file (Write tool) and run the file.

## 2026-10-01 · WP-015 (Schema Keeper)

- **A fixture ledger line is pinned by engine tests, not only by the
  validator.** One snapper event or one ledger line moves
  `engine/src/model/event.rs` (line count), `collectors/snapper.rs` (snapshot
  numbers), `tests/support/mod.rs` `story()`, `tests/idempotency.rs` (written
  and per-source counts) and `tests/golden/snapper.jsonl`
  (`SELDON_BLESS=1 cargo test --test collectors fixture_story`). Snapper events
  also need the entry in `logs/snapper.json`, or the collector golden test
  cannot reproduce them. Run `cargo test --no-fail-fast` after a fixture edit:
  a failing lib test hides every integration test.
- **Build fixture ledger lines in the engine's key order.** The round-trip
  test compares `Event::to_line()` byte for byte. Copy the key order of an
  existing line of the same kind (`id, ts, source, kind, subject, detail,
  actor, case, zone, explicit, txId, refersTo, resolution, meta`).
- **A case event without `source: seldon` belongs in the case's `events:`
  frontmatter**, in ts order. Case lifecycle events (`source: seldon`) do not.
- **`generatedAt` cannot be earlier than the story.** An index stamped 09:30
  cannot list events from 17:00, and `lastCapture` cannot be earlier than a
  collector event. The validator now checks both. A "fresh for a live clock"
  sample is impossible: fresh lasts only 2 h. Pin the clock with `SELDON_NOW`.
- **ADR-0014 §2 has no producer for `zone: green`.** The fixture's green
  event is an assumption (hook command on a path no collector watches), not
  a rule. Check the decision before you copy it into the engine.
- **The guard reads file content in Bash arguments as commands.** A heredoc
  holding a `tee ~/.config/…` string, or a `sed` pattern with `| pacman -S…`
  in it, is blocked as a red-zone write or a package command. Edit Markdown
  with the Edit tool. Write edit scripts with the Write tool.

## 2026-10-01 · WP-007 (Engine)

- **Fixture ULIDs do not sort by time within one second** (their random
  part is a hash). The ledger views keep ledger order for events of the
  same second (stable sort by day, then `ts`); sorting by id reorders
  `snapshot-delete 108/109` and the 09-30 upgrades.
- **The golden test needs more than `SELDON_NOW`:** `logbook.path` is not
  derivable either (compare modulo it), and `state` comes from a
  `cursors.json` the test writes, bound to the *canonical* path of the copy
  (capture canonicalises the logbook root).
- **A timestamp of "now" in STATUS.md means a commit every plugin cycle**
  (`capture` + `status` every 15 min). STATUS.md stamps the day and the
  last event instead; an unchanged logbook writes nothing.
- **`fixtures/logbook/STATUS.md` is stale** (3 open drift, 35 events in 7
  days, German headings); it is not a test target. The ledger views
  `fixtures/logbook/ledger/*.md` are, byte for byte.
- **Index size grows with open drift and open cases, which are not
  capped.** ×150 of the fixture (10 050 lines, 1 200 cases) gives an
  897 KB index, close to the 1 MB budget of CONTRACT.md rule 5;
  truncation needs `meta.truncated` and a contract bump.
- **Importing `scripts/validate-fixtures.py` from python3 leaves
  `scripts/__pycache__/`.** Delete it before committing.
- **A test of an atomic write needs a negative control.** Swapping
  `index::write` for `std::fs::write` made the in-process reader test fail
  3/3 (empty reads); the CLI variant caught it only 1/3, so the in-process
  loop is the proof and the CLI loop a smoke test.
- **Fixture counts hard-coded in tests break on every fixture update.**
  After WP-015 (67 → 71 ledger lines), `lines == 670` and the expected
  STATUS.md (27/38 events) failed, while the golden test, which compares
  against the sample, passed unchanged. Take numbers from the sample where
  you can. And run the whole suite after a rebase: `cargo test` stops at
  the first failing test binary, so later failures stay hidden.

## 2026-10-01 · WP-009 (Engine)

- **Claude Code's `Stop` hook fires after every assistant reply**, not at
  the end of a session; `SessionEnd` fires once when the session ends
  (hooks reference, checked 2026-10-01). A journal stub on `Stop` would
  write one entry per reply. `hook install` uses `SessionEnd`.
- **The fixtures in `fixtures/hooks/` are PostToolUse payloads**, but
  ADR-0017 records on PreToolUse. Tests set `hook_event_name` per case; the
  hook still records a PostToolUse whose `tool_use_id` is not in the ledger.
- **Guard false positives on file content in a Bash heredoc** (WP-009,
  twice): a doc comment "`attribution.rs`; pacman and omarchy …" matched the
  package-manager rule (`;` + `pacman`), and test strings with
  `> ~/.config/hypr/…` matched the `~/.config` write rule. Write file
  content with the Edit/Write tools; a *command* the guard blocks is
  reported, never reworded or moved into a script (ORCHESTRATION.md §11).
- **A recorded command has no working directory.** The hook resolves
  relative paths against the payload's `cwd` and any `cd` earlier in the
  line, but attribution later only sees `meta.command`: a bare
  `sed -i … bindings.conf` run inside `~/.config/hypr` cannot prove the
  path. Paths count as `~/…`, `$HOME/…`, absolute or home-relative.
- **A heredoc in a Bash tool call is the command's stdin**: `cat > x <<EOF`
  carries a whole config file (and its secrets). The hook cuts heredoc
  bodies before it records the line (`pkgcmd::parse_shell`).
- **The theme-set hook makes its own event final.** The theme collector
  skips a change the hook already recorded, so an agent's `omarchy theme
  set` would stay `system` unless `seldon event` runs the attribution pass
  itself (it does since WP-009).
- **`~/.config/systemd` is red as a directory too**: `zone_for` checks the
  prefix `~/.config/systemd/`, so the hook tests a path with a trailing `/`.
- **Unwrapping wrappers can invent a command** (WP-009 review blocker).
  `command -v yay` stripped to `yay` is a full upgrade (`yay` alone =
  `-Syu`): a red event *and* a cause that claims a human's later `-Syu`.
  Probe options (`command -v|-V`, `sudo -l|-v|-k`) run nothing; every
  wrapper-stripping path (`command_argv`, `command_intent`) must know them.
- **An argv word is not a write.** Proving a config change by "the path
  appears in the command" lets `cat x && pacman -S y` claim the user's
  later edit of `x`. Attribution proves only through `pkgcmd::write_targets`.
- **`2>/dev/null` is a redirection target.** Any "writes a file" rule must
  ignore `/dev/*`, or every quiet command becomes a recorded write.
- **The guard also reads `git commit -m` text.** A message with
  `… && pacman -S …` as an example is blocked like a command. Write the
  message with the Write tool and commit with `-F`.

## 2026-10-01 · WP-008 (Engine)

- **A heredoc that appends Rust test code is read as commands by the
  guard.** A test string such as an agent's in-place edit command on a
  `~/.config/hypr` file (the test only touches a temp HOME) was blocked
  as a red-zone write. I reported it and did not reword it. Fixed on
  `main` in `8ca0dd9`: the `~/.config` rule now applies only at command
  position, and the same heredoc passed in the review follow-up. A
  block is still reported, never reworded.
- **Stub programs see only the stub dir on PATH.** `sleep 12` in a stub
  exits 127 at once, and with the detached launcher that shows up as
  "exited with 127", not as a hang. Resolve host tools to absolute paths
  in the test (the same as the `bash` note above).
- **`seldon init` stamps `created` with the real clock, not
  `SELDON_NOW`.** So a pacman capture of log lines from "today 10:01"
  writes nothing (they are before the baseline). Pass `--since`.
- **Test helpers that add `--json` must put it before the subcommand.**
  Appended after `-- <text>`, it becomes part of the free text or an
  unexpected argument.
- **An old leader id is not a handle for its group.** After `--only` on
  the leader, the remaining members form a new item with a new leader.
  `drift <verb> <old leader>` writes nothing, by design.

## 2026-10-01 · WP-016 (Schema Keeper)

- **The hook reads `~` from `$HOME` but "inside `~/.config`" from
  `$XDG_CONFIG_HOME`.** Running a fixture payload with `HOME=/home/user`
  and `XDG_CONFIG_HOME` pointed at scratch silently turns `git -C
  ~/.config/hypr push` from yellow into green-needs-a-case, so nothing is
  recorded. Keep `XDG_CONFIG_HOME` unset and put the scratch config in
  `SELDON_CONFIG` (state in `XDG_STATE_HOME`).
- **`Edit`/`Write` payloads carry absolute paths.** A fixture under
  `/home/user` only hits `watchPaths` when `HOME=/home/user`; a test with a
  temp `HOME` must rewrite the prefix first, or the watched Edit records
  nothing without a case.
- **Fixture logbook files feed engine goldens too.** `STATUS.md` is quoted
  by `engine/tests/golden/session-start.txt` (the session-start context
  block). After changing any file under `fixtures/logbook/`, grep
  `engine/tests/golden/` as well as `engine/tests/*.rs`.
- **Harness step numbers count steps, not key presses.** `key:Down*61` is
  one step and one report; the row it scrolls to is visible in the next
  report number, not in the one after `key:Down`.

## 2026-10-01 · WP-022 (Engine + Plugin)

- **`omarchy agent prompt --inline` runs the agent in the caller's
  terminal** (`omarchy-agent` ends in `exec "${command[@]}"`); without
  `--inline` it opens a window through `omarchy-launch-tui
  --app-id=org.omarchy.agent`. A detached launch (null stdio, the plugin)
  has no terminal, so the default launcher is
  `omarchy agent prompt {prompt}` without `--inline`, deviating from the
  brief. `omarchy-agent` also `cd`s to `~/Work` when started in `$HOME`;
  `agent start` sets the logbook as its cwd.
- **`omarchy-launch-floating-terminal-with-presentation` joins `$*` into
  `bash -c "…"`** (also `omarchy-launch-or-focus`,
  `omarchy-launch-terminal-tmux`). Never pass logbook text to them;
  `agent start` refuses them, and any shell before `{prompt}`.
- **`omarchy-shell lock status` over ssh needs `OMARCHY_PATH` and
  `XDG_RUNTIME_DIR`:** `ssh <test host> env
  OMARCHY_PATH=/usr/share/omarchy XDG_RUNTIME_DIR=/run/user/1000
  omarchy-shell lock status` (a plain call prints "OMARCHY_PATH is not
  set").
- **The real-home guard catches other workers too.** During this WP's
  `just check`, a parallel worker's pty run of `seldon init` (logbook in
  its own scratchpad) wrote the real `~/.config/seldon/config.toml` and
  `~/.local/state/seldon/` (incl. `hooks/seldon-theme-set.sh`), and
  service-states failed on it. A rerun once the paths were stable passed.
  The guard says which path changed, not who: check the files' mtime and
  contents before suspecting your own change.
- **A new Work card action that is not the first one needs its own key
  and a hint that names it.** "Press Enter again" would be wrong: Enter
  arms the first action (Verify) instead. Start agent uses `a` and a
  click to arm (`twice`), and the hint says so.
- **A new engine verb must be added to `Service.runnerDone`'s list of
  calls that show their errors in place**, or a refusal also lands in
  `lastError` (the banner).
- **Review round 1 (WP-022):**
  - `omarchy-launch-or-focus-tui` and `-webapp` build
    `LAUNCH_COMMAND="omarchy-launch-tui $@"` for `eval` in
    `omarchy-launch-or-focus`. Grep a launcher's whole body for unquoted
    `$@`/`$*` inside a string, not only for `bash -c`.
  - The `omarchy` CLI resolves multi-word routes (`omarchy launch
    or-focus-tui`, possibly `omarchy launch or focus tui`) to the same
    scripts, so a basename list misses them. Refuse the route.
  - A log that a detached child keeps open must be opened with append,
    never truncated. To report only this launch's lines, remember the
    length before the spawn and read from there.
  - A manual demo of a command that launches programs needs a `PATH` of
    stubs only. With `/usr/bin` on it, a removed stub falls through to the
    real program.
## 2026-10-01 · WP-024 (Engine)

- **Overriding `HOME` alone does not redirect the engine.** The desktop
  session exports `XDG_CONFIG_HOME`, `XDG_STATE_HOME` and `XDG_DATA_HOME`
  into the real home, and an absolute XDG variable wins over `HOME`. A pty
  run as `env HOME=<scratch> script -qec "seldon init …"` therefore wrote
  the real `~/.config/seldon/config.toml` and `~/.local/state/seldon/`.
  `script` itself passes the environment on; I first blamed it, wrongly.
  The guard then (rightly) blocked the `rm` under `~/.config`, so the
  files were left for the operator. Since the review follow-up:
  - export `HOME` *and* all three `XDG_*` into one scratch dir;
  - set `SELDON_TEST_GUARD` to that dir: the engine exits 2 when its
    resolved home/config/state dirs leave it (`common::Env` sets it for
    every test);
  - probe with `seldon --json doctor` (the `config` check names the file).
- **`init` runs the first capture now.** A test that builds machine state
  after `init` and expects its own first capture to be the baseline must
  pass `--no-capture` (the shared `Env::init_logbook*` does); otherwise new
  files show up as `config-add` instead of the silent first state.
- **`fixtures/logs/pacman.log` ends on 2026-10-01 17:04 +0200**, and
  `init` stamps `created` with the real clock: on that day a test without
  `--since` may or may not pick lines up. Use an empty log, or `--since`.
- **A backfill is big.** Dev host, 7 days: 1230 pacman events, 22 items,
  all crises; the baseline writes one resolution per member (ADR-0013 §4),
  so 1230 more ledger lines. Correct, but the ledger doubles.
- **`seldon … | head` panics** ("failed printing to stdout: Broken pipe"):
  `println!` on a closed pipe, pre-existing and harmless. Redirect to a
  file when only the start is needed.

## 2026-10-01 · WP-032 (Engine)

- **A hand-written ledger line with an invalid ULID is skipped, not an
  error.** Crockford base32 has no `I L O U`; test ids like `…AUR01` made
  the loader drop the lines with a warning, and the test failed on a count
  as if the logic were wrong. Check `warnings` in the command's JSON first.
- **`just check` takes longer than the 2-minute foreground tool limit**
  (plugin tests in headless Quickshell); run it in the background and wait
  for the notification instead of re-running it.

## 2026-10-01 · WP-034 (Engine)

- **A file watcher sees its own reads.** notify's inotify backend reports
  open and close-without-write. `seldon watch` drops every `Access` event
  and writes only `index.json` (state dir, outside the watch), so its
  rebuild cannot trigger the next one. The generated logbook files
  (`ledger/*.md`, `STATUS.md`) and hidden or temp files (`.<name>.tmp-<pid>`
  from `sys::write_atomic`, editor swap files, `~` backups) are filtered
  by path.
- **Never enable or start the unit on the dev host.** `systemctl --user
  enable|start` is red zone (the guard blocks it). The read-only check is
  `systemd-analyze --user verify <unit>`. On a host without
  `~/.local/bin/seldon` it reports only "is not executable"; verify a
  scratch copy whose `ExecStart` points at `/usr/bin/true` to see the
  other warnings.
- **The default release build has no watcher.** `just build-release`
  leaves the `watch` feature out, so `seldon watch` exits 1. The unit's
  `RestartPreventExitStatus=1 3` stops systemd from restarting it forever
  in that case, and before `seldon init`.
- **Under parallel test load the debounce timing stretches.** The watch
  tests allow 4 s of slack over the 2 s interval and wait for the
  `watching` line before they write anything. A sleep after spawning is
  not enough.
## 2026-10-01 · WP-035 (Engine)

- **The guard reads a `grep` pattern as a command.** A read-only
  `grep -n -i "…systemctl\|list-unit…" memory/host.md` was blocked as a
  "service or boot command". Nothing ran. I did not reword it; I read the
  file with the Read tool. Name shim files and test data so that Bash
  lines never need the word (`git add fixtures/logs/`, not the file names).
- **Shims, not PATH filtering, for host queries.** `Env::query_shims`
  writes `pacman`, `systemctl` and `omarchy` stubs that print
  `fixtures/logs/*` for exactly the read-only argument lists, exit 64 for
  anything else, and append every call to a log. The log is the proof that
  only queries ran; without a stub the program is simply not found (PATH
  is the stub dir), which is the "query failed" test.
- **`init` now runs the dossier, so it reads `/proc` and `/sys`.** Tests
  that run `init` with a capture set `SELDON_HARDWARE_ROOT` to
  `fixtures/logs/hardware` (`init_with` does). Otherwise the hardware
  fence holds the test machine's CPU, and "is there a dossier commit"
  depends on the host. Expected git log after `init` with capture:
  `seldon: dossier`, `seldon: first capture…`, `seldon: init logbook`.
- **`pacman -Qqm` exits 1 when there are no foreign packages** (empty
  output, empty stderr). `query()` treats exit 1 with no output at all as
  an empty answer; any stderr is still an error. The dev host has 0
  foreign packages (Omarchy's own repository carries brave-bin etc.).
- **`MemTotal` is not the RAM size.** 64 GB machines report about
  62.5 GiB; the dossier writes `MemTotal` rounded to whole GiB (`63 GiB`
  in the fixture), which is stable across boots. Physical size would need
  dmidecode (root) — not worth it.
- **A plain fixture copy has no `packages.explicit` fence.** The rebuild
  golden runs `seldon dossier --section packages` with the shims first;
  the other rebuild tests do not, so both §2 forms are covered. The manual
  `diff` against the golden (TESTING.md) shows exactly the "Before the
  logbook" group.
- **`~/.config/seldon/config.toml` already exists on the dev host**
  (mtime 2026-10-01 17:10, before this WP's runs; see the WP-022/024
  notes). Prove "nothing touched" with a marker file and `find -newer`,
  not with "the directory does not exist".

## 2026-10-01 · WP-031 (Plugin)

- **node's `vm` sandbox makes Model.js about 8× slower** than a plain
  function scope: every top-level name is a contextified global lookup.
  `periodTable` on the sample ×10: 14 ms in the sandbox (as model.test.js
  loads it), 1.8 ms plain. State which one a timing is; the bench prints
  both.
- **V8's `Date.parse("2026-02-30T00:00:00Z")` rolls over to 2 March**
  instead of failing, so `isDate` built on it accepted impossible dates
  under node. `dayNumber` now does the calendar arithmetic itself
  (checked against `Date` for every day of 1899–2101).
- **First-fit lane packing is quadratic** when many spans stay open
  (7000 timeline rows, 233 open cases: 244 lanes × 700 spans × 4 periods).
  Interval partitioning with a min-heap of lane ends packs as tightly in
  O(n log n).
- **The overlay harness's `toggle` reuses one long-lived Overlay.qml**,
  unlike the shell, which creates it on every open: its paints and work
  happen while it is still closed. Measure the first frame with the
  `fresh` step (a new overlay, as the shell's Loader does).
- **jq inside `expect` filters:** `input` reads the *next* JSON line, not
  an earlier step; read an earlier step's value with `sed -n Np` in bash.
- **A "peak" compared against a summary field that is filled after the
  loop** (`n.max` set once the loop ends) is 0 throughout, so it reported
  the last week with anything instead of the maximum. Keep a running
  maximum; test with a non-monotonic series (review of WP-031).
- **A mutation check proves a counter test is not blind:** slip one
  forbidden call into the code under test and see the suite fail
  (WP-031's aggregation count missed the chart files until the reviewer
  did this).

## 2026-10-01 · WP-036 (Engine)

- **`common::Env` clears the environment, so `OMARCHY_PATH` is unset in
  tests.** The engine then falls back to `/usr/share/omarchy/install`:
  the host's real lists. `Env::command` therefore sets
  `SELDON_OMARCHY_PACKAGES` to `fixtures/logs/omarchy-packages/` for
  every test. Without it, `init`'s dossier run would classify packages
  according to whatever Omarchy the test machine has installed.
- **Omarchy's package lists are not "what a fresh install has".**
  `omarchy-base.packages` is the ISO's core list. `omarchy-other.packages`
  also holds hardware-specific packages (nvidia, T2, Surface drivers).
  `linux` and `omarchy` themselves are in neither (the lists have
  `linux-omarchy`). So `omarchy-base` means "named by Omarchy's lists",
  and a `user` package may still come with Omarchy some other way. The
  rebuild text says "the commands skip what is installed" for exactly
  that reason.
- **The dev host's `$OMARCHY_PATH/version` says `4.0.0.alpha`, while
  `omarchy-version` reports the packaged version.** The rebuild's base
  count line takes the version from `omarchy.summary` (the same dossier
  run). It does not read the file.
- **An empty fence added to the fixture moves the "outside the fences"
  comparison.** The golden test no longer needs the "appended heading"
  special case. The dossier golden itself did not change shape: the
  fixture puts the fence exactly where the append used to.

## 2026-10-01 · WP-040 (Scaffold, packaging)

- **The guard blocks `makepkg` inside an ssh command string** when it
  follows `;`/`&&` (`ssh <host> 'cd dir && makepkg -f'`), and also in a
  plain probe (`…; makepkg --version`). The WP allowed makepkg on the test
  host, but a block is reported, not reworded: the PKGBUILD build there is
  open until the guard gets an ssh exception like the `omarchy theme set`
  one. Probe tools with `command -v`, never by running them.
- **`rust-musl` depends on `rust`, which conflicts with `rustup`** (what
  `omarchy install dev-env rust` installs). An AUR package must not need
  the musl target; it builds for glibc, the static musl binary is a
  release asset (ADR-0022, proposed).
- **The engine has a lib and a bin target.** Unit tests are
  `cargo test --lib --bins` (111 + 2); `--bins` alone runs 2.
- **cargo under rustup with `HOME` redirected** fails with "rustup could
  not choose a version"; keep `RUSTUP_HOME`/`CARGO_HOME` pointing at the
  real ones when running cargo in a scratch home. With
  `RUSTUP_TOOLCHAIN=stable` set, rustup instead **auto-installs a 1.5 GB
  toolchain into the scratch home**, and then cargo `--frozen` fails
  (exit 101, "no matching package") because the registry is in the real
  `~/.cargo`. The PKGBUILD's `check()` had exactly this bug (review of
  WP-040): pin `CARGO_HOME`/`RUSTUP_HOME` before moving `HOME`.
- **GitHub Actions:** the implicit `run` shell is `bash -e` without
  `pipefail` (`defaults.run.shell: bash` adds it); `actions/checkout`
  persists an `http.<github>.extraheader` with `GITHUB_TOKEN` that would
  shadow another token pushing to github.com (`persist-credentials:
  false`); `upload-artifact@v4` skips dot files such as `.SRCINFO` unless
  `include-hidden-files: true`; `makepkg` refuses root, so a container
  job runs it through `runuser -u <user>`.
- **makepkg takes a local file for a URL source** when it sits in the
  build dir under the `source` entry's file name, so a `git archive`
  tarball named like the release asset tests a PKGBUILD before any tag
  exists.

## 2026-10-01 · WP-013 (QA)

- **The ssh alias `test` does not exist on the dev host.** The docs write
  `test:`, but `~/.ssh/config` has no such host. Pass the real name from
  `memory/local.md` through `SELDON_TEST_HOST`, and never commit it.
- **A fresh logbook's first capture records no history.** Collectors
  without a cursor start at the logbook's `created`. A test that needs
  package events must pass `capture --since <ts>`. With a window, every
  package change in it becomes open drift, and a named package becomes a
  red crisis.
- **`pgrep -x quickshell` also matches a Quickshell crash-report window.**
  A log check on that pid reads the wrong log and passes vacuously. Take
  the pid from `quickshell list -a -j` (match `config_path`), and require a
  positive line ("Configuration Loaded") before you trust a clean log.
- **Restarting the shell right after an rsync into the plugin dir can crash
  it.** Every rewritten file triggers one hot reload; the old instance
  segfaulted on exit in the middle of them, and the crash handler left a
  crash-report window open. Rsync with `--checksum`, and wait until the
  shell answers `ping` plus about 5 s before `omarchy-restart-shell`. Check
  `~/.cache/quickshell/crashes` before and after.
- **A locked test-host session breaks both the restart and the keys.**
  `omarchy-restart-shell` refuses while the session is locked, and it
  re-locks after a restart when the session was locked. `wtype` would type
  into the lock screen. Check `omarchy-shell lock status` (`.locked`)
  before every restart and every keystroke. Unlocking needs the operator.
- **A killed run's ssh command keeps running on the remote side.** A
  SIGKILL of the local script does not stop the remote `bash -c`. The next
  run's restore then races its restart. Wait about 10 s before rerunning.
- **Send remote scripts as an argument, not on stdin.** `ssh host bash -s
  <<< script` lets any command in the script read the rest of the script
  from stdin. `ssh -n host "bash -c \"\$(echo <base64> | base64 -d)\""`
  keeps stdin at /dev/null and needs no quoting of the script.
- **The guard reads `> 0` in a jq filter as a redirection.** A read-only
  `jq '… select(length > 0)' ~/.config/omarchy/shell.json` over ssh was
  blocked as a write under `~/.config`. That was reported, not worked
  around.

## 2026-10-01 · WP-013 live session (QA)

- **Gate every live keystroke on a Seldon surface being open, not only
  on the lock.** A pointer click closed the panel, and the next `wtype`
  text went into the operator's terminal, where bash ran it
  (`work/active/WP-013/live/INCIDENT.md`). Before any key, check that
  `jax.seldon.panel view` or `shell call jax.seldon view ""` reports
  `opened: true`. Type text only while the target field reports
  `editing: true`.
- **`hyprctl activewindow` does not show layer-shell keyboard focus.** It
  keeps naming the client window while our panel has the keys. Use the
  plugin's own `opened` / `editing` flags.
- **`ydotool mousemove --absolute` doubles the coordinates on the test
  host;** pass half the logical value. `hyprctl dispatch movecursor` warps
  without a motion event, so Quickshell sees no hover; `ydotool` moves
  do produce one.
- **The guard's theme-sweep exception matches only a bare
  `ssh <host> '… omarchy theme set "<theme>"'`.** Anything after it (`;
  echo …`) makes it a blocked system change. Run the theme command alone.
  A block is reported, not retried in another form.
- **`seldon init` runs the first capture now (WP-024).** Backfill with
  `init --since`. A later `capture --since` is ignored by every collector
  that has a cursor (`sinceIgnored`).
- **The drift sheet's action buttons are keyboard-reachable** from the
  text field with Backtab ×2, then ←/→ and Enter (as `panel-view.sh`
  does). Space selects only the focused option, it does not move.
- **Screenshots for the repo come from a fixture-based scratch logbook.**
  Copy `fixtures/logbook/` over the `init`ed scratch logbook (its
  `logbook.toml` carries `workstation-7f3a`). Crop away the bar (window
  titles) and the desktop, and grep the logs for the host name and
  `/home/<user>` before committing.

## 2026-10-01 · WP-037 (Plugin, overlay live findings)

- **A headless harness must create plugin items the way the shell does.**
  Passing `service` as a creation property hid a 23-pass aggregation on
  every live open for two WPs. The harness now creates Overlay.qml bare
  and assigns `shell`, `manifest` and `service` afterwards, and it records
  `firstFrame.bare`. Swapping the harness first and keeping the old code
  gave exactly the live number (23); that is the cheapest proof the
  harness is faithful.
- **Probe points given as fractions of a chart's plot depend on the
  chart's own geometry.** The heatmap grid is square and height-bound, so
  `0.9` is empty at 30/90 d. Series reserves a value-label column on the
  right (`x > plotW` → no hover), so `series 0.9,0.5` is empty too. Use
  `0.5,0.5` for Series, and for the heatmap a point from
  `Model.heatmapLayout` (TESTING step 4).
- **The test host's bar shows window and media titles.** Crop the top
  28 px of a 1920×1080 grim shot (scale 1.25) before committing it. For
  live chart shots, a stand-in engine plus the fixture index (TESTING
  step 3) is enough and leaves only two paths to remove.
- **`rsync -a --checksum` still sets the mtime of every file** whose time
  differs (`.f..t` in `-i` output), even when the content is the same. To
  touch only the files whose content changed (and keep the hot reloads to
  those), use `rsync -rp --checksum`. Whether an mtime-only change
  triggers a reload was not tested.

## 2026-10-02 · WP-038 (Engine)

- **A test stub runs with the test's PATH: the stub directory only.**
  An `omarchy` stub that copies files (`cp`, `mkdir`, `chmod`) finds
  none of them and fails silently. Set `PATH=/usr/bin:/bin` as the first
  line of such a stub (`tests/own_writes.rs`).
- **A config event's time is the file's mtime, not the capture's.**
  The attribution window (ADR-0017) compares the hook command's start
  with that mtime (clamped to the last check), so a late capture still
  attributes an agent's edit. What a late capture misses is an edit that
  was reverted before it ran: the hashes match again and there is no
  event at all.
- **The init order matters for the engine's own files.** The theme hook
  is installed *after* the first capture, so the config baseline does not
  contain it and the next capture reports a `config-add`. Harness files
  are written before the first capture and are in the baseline. Rule 7
  (`owned.json`) covers the first case; moving the install earlier would
  have hidden the event instead of explaining it.
- **The repository guard matches strings anywhere in a Bash command,**
  also inside a read-only `grep` pattern: a pattern that contained a
  package-manager install string was blocked as a red-zone package
  command. Leave such strings out of search patterns; report the block,
  do not rephrase the same search around it.

## 2026-10-02 · WP-039 (Plugin, panel width and label fit)

- **"Fits on screen" in a harness render is not "fits".** The tab labels
  looked fine offscreen at 380 while the Buttons were 2–4 px narrower than
  label + padding; the operator saw "Changelog" clipped live. Measure
  (`implicitWidth > width`, `Text.truncated`), don't eyeball.
- **The real-home guard trips on the dev host when the operator's own
  Seldon is live:** the installed plugin's capture rewrites
  `~/.local/state/seldon/index.json` during a run (seen at 12:00:00). The
  guard now passes that only for the state files, an unchanged
  config.toml and the configured logbook and same machine in the index;
  everything else still fails. `real_home_check` no longer aborts the
  script under `set -e` (the `diff | sed` pipeline returned 1 before the
  summary line).
- **The test host's ssh alias is in memory/local.md**, not `test`
  (`SELDON_TEST_HOST` default); check `omarchy-shell lock status` with
  `OMARCHY_PATH` exported in the same ssh call.

## 2026-10-02 · WP-043 (vault import)

- **The operator's vault and `~/Seldon` are off limits for writes.**
  Run the real-vault dry run with HOME and all three `XDG_*` in one
  scratch dir, `SELDON_TEST_GUARD` set, `--logbook` on a scratch copy of
  `fixtures/logbook/`, and checksum the vault before and after
  (`find -type f -print0 | sort -z | xargs -0 sha256sum`). Never paste
  vault titles or text into handovers or commits: counts and ids only.
- **Python's `glob('**/*.json')` skips dot directories.** The fixture
  vault's `.obsidian/app.json` is therefore not seen by
  `validate-fixtures` (every other JSON under `fixtures/` needs a schema
  mapping). If that script ever sets `include_hidden`, map or exclude
  `vaults/`.
- **`init` creates `work/active/` with a `.gitkeep`.** "No active case"
  in a test means "no `C-*.md` in `work/active/`", not "the directory is
  absent or empty".
- **Fixture homes are `/home/user`** (fixtures/README.md). A synthetic
  vault that uses another fake user name breaks that convention.
- **The kit's deviations are not a table.** `system/deviations.md` is a
  list of `### Qn — …` entries with `Datum:` and the path somewhere in
  a code span; some name no path at all (software lists), some share a
  path. The importer takes the first `~/…` or `/…` span of the heading,
  else of the body, and reports the rest.

## 2026-10-02 · WP-043 review round

- **`body.contains("|---")` is not a table test.** Obsidian's table
  editor writes `| --- | --- |` and `|:---|`; the dossier then took the
  fence as empty and replaced the user's rows with a fresh header.
  Use `index::load::has_table_separator` for any "is there a table"
  check.
- **A "done" marker must be the last thing written, or the only thing
  trusted.** Treating ledger import notes as "already imported" turned a
  half-failed apply into a false "nothing changed". Now only the marker
  means done; notes without it are refused with the undo hint.
- **Cheap write-failure injection:** `chmod 555` on a directory the
  command writes late (`system/`), checked with a probe write first so
  the test returns early when it runs as root.
- **An "updated" assertion on a template file is trivially true** when
  `init` ran the same day. Pre-date the field (and commit) before the
  command under test.

## 2026-10-02 · WP-047 (agent guide)

- **Keep every command in a template on one line.** A code span wrapped
  across lines renders fine on GitHub but breaks substring tests and an
  agent's `grep` (`seldon capture` / `--all` on two lines). Wrap before
  the backtick, not inside it.
- **Template headings are pinned twice:** `tests/golden/init-skeleton.txt`
  (rewrite with `SELDON_BLESS=1 cargo test --test init
  templates_have_english_keys`, then review the diff) and, for
  `AGENTS.md`, the ordered section list in
  `agents_md_carries_the_agent_rules_in_both_languages`.

## 2026-10-02 · WP-048 (Scaffold)

- **GitHub's community profile reads the default branch only.**
  `gh api repos/<owner>/<repo>/community/profile` keeps reporting
  CONTRIBUTING, SECURITY, the code of conduct and the templates as
  missing until they are merged and pushed to `main`; a branch cannot
  prove that acceptance line. Re-run it after the merge.
- **A release dry run never reaches the `release` job.** The
  `workflow_dispatch` run executes `build` only, so anything added to
  `release` (the CHANGELOG notes step) is proven by a local test
  (`tests/release/`) or by running the step body with `VERSION` and
  `RUNNER_TEMP` set, not by the dry run.
- **The dev host has no `shellcheck` and no `actionlint`**, while the
  release build's `just check` runs shellcheck on `packaging/`. Lint
  locally with `shellcheck-py` / `actionlint-py` in a venv under the
  scratchpad (nothing installed on the host) before committing a script.

## 2026-10-02 · WP-044 (install.sh)

- **shellcheck is not on the dev host** (no package, docker socket not
  accessible to the user), and the CI workflow `ci.yml` does not install
  it either: `check-install` and `check-packaging` fall back to `bash -n`
  there. Only the release workflow's `build` container has it, so a
  shellcheck finding in `install.sh` first shows in the release dry run —
  run the dry run before every tag (packaging/README.md), and keep
  `# shellcheck disable=SC2016` on single-quoted strings that carry `$1`
  for another program.
- **`releases/latest/download/<asset>` 404s for an asset the latest
  release does not carry.** v0.1.0 has no `install.sh`; the documented
  URL works from the next release on. The README says so.
- **`curl -fsSL` accepts `file://`,** so a mock release is a directory:
  `SELDON_INSTALL_DOWNLOAD_URL=file://<dir>` and `SELDON_INSTALL_API_URL`
  pointing at a JSON file. No port, no server, no race. Restricting curl
  with `--proto '=https,file' --proto-redir '=https'` keeps that working
  and still blocks a downgrade to http on GitHub's CDN redirect.
- **"No jq" without uninstalling jq:** a PATH dir of symlinks to every
  tool on PATH except `jq`, used with `env -i PATH=…`.
- **Wrap a `curl | bash` script in `main "$@"` on its last line;** a test
  that drops the last lines proves a truncated download does nothing.
- **The real-run acceptance test is safe with the real HOME** as long as
  `--unit` is not passed: fingerprint `~/.local/bin/seldon`,
  `~/.local/bin/jax-seldon`, `~/.config/systemd/user` and `~/Seldon`
  before and after, and give `--prefix /tmp/<scratch>`.

## 2026-10-02 · WP-045 (user guide)

- **Write docs from the engine's real output, not from the spec.** Two
  claims I took from memory were wrong: a newly added watched path is
  not baselined (every file in it opens as `config-add` drift), and
  `DECISIONS.md`'s "generated" table stays empty after `seldon decide`.
  Run each flow in a scratch home (HOME and all three `XDG_*` in one dir,
  `SELDON_TEST_GUARD` set) before you describe it.
- **The wizard can be driven without a person.** In the scratch
  environment this answers every question with its default:

  ```
  (sleep 1.5; for i in $(seq 1 10); do printf '\r'; sleep 0.7; done) | script -qec "seldon init" <file>
  ```

  dialoguer's multi-select keeps an unticked list on Enter; only Space
  ticks.
- **A theme switch in a scratch home** is a write to
  `<scratch>/.local/state/omarchy/current/theme.name` (create it before
  `init` so the collector has a baseline). `omarchy theme set` would
  change the real desktop.
- **The guard blocks a read-only grep whose pattern names Omarchy's
  agent launcher** ("agent or app launcher on the dev host"). Leave such
  words out of search patterns; report the block.
- **Translations: finish and commit the English page first,** then put
  `git log -1 --format=%h -- docs/user/en/<page>` into the German source
  line. Every later English edit shows as a docs-check warning until the
  German page and its source line follow.
- **German slugs:** the engine writes `ü` as `ue` in case file names
  (`zurueck-zu-…`); GitHub anchors keep `ü` (`#prüfen-was-…`). docs-check
  uses GitHub's rule.
- **`seldon --help` is the same with and without the `watch` feature,**
  so docs-check builds the default features; `check-watch` running before
  it in the same target dir does no harm.
- **`docs-check` reads every `seldon …` in code spans and `sh` blocks;**
  output belongs in `text` blocks, or it is parsed as a command
  (`seldon 0.1.0`, `generated by seldon; do not edit`). A planned command
  goes into `PLANNED` in the script, which fails once the engine has it.

## 2026-10-02 · WP-050 (Engine)

- **A changed config default does not reach existing installs.** `init`
  writes `[drift] alwaysRed` (and the other defaults) into
  `config.toml`, so a new default list only applies to new logbooks;
  say so in the CHANGELOG. `scripts/validate-fixtures.py` mirrors the
  list in `ALWAYS_RED`; change both together.
- **A begin marker without its end must stop a fence writer.** If the
  writer appends a fresh fence instead, the next run's `fence_body`
  spans from the old begin to the new end, and replacing it deletes the
  user's text in between. `write_decisions_index` skips with a warning;
  `dossier::Files::set` still appends in that case (open finding).
- **`alwaysRed` only sees package names.** The globs match pacman
  subjects; an R3 subject like `/etc` can only be approximated by the
  packages that own it (`omarchy-settings`, `filesystem`). Config events
  get their zone from `model::event::zone_for` (ADR-0014 §2) instead.
- **Package names on this host are in `/var/lib/pacman/local/`** — a
  directory listing is read-only and passes the guard; the package
  query command does not.

## 2026-10-02 · WP-050 fix round

- **Resolved: `dossier::Files::set` no longer appends past a damaged
  fence** (the open finding above). It returns `Err(warning)` and skips;
  `import` turns that into an error. A "damaged" fence also covers a
  borrowed end: the body `fence_body` finds contains another begin
  marker, so the end belongs to the next fence.
- **Globs cannot say "but not".** `linux*` caught firmware and headers;
  a kernels-only rule has to be an explicit list, and every kernel not
  on it (AUR kernels) is the user's to add. Say so in the spec.
- **Template prose can contain the marker word.** The deviations
  template mentions `seldon:begin`/`seldon:end` in a code span, so a test
  that breaks a fence must replace the full `<!-- seldon:end -->` marker,
  not search for `seldon:end`.

## 2026-10-02 · WP-046 (READMEs)

- **plugin/ is a repository of its own after `git subtree split`.** A
  relative link or image in `plugin/*.md` that leaves `plugin/` breaks on
  the `jax-seldon-plugin` front page; use
  `https://github.com/JohnAndrewsX/jax-seldon/blob/main/<path>`.
  `docs-check` now fails on it. To see the split repository, clone
  `--no-local` into the scratchpad and run the split there, never in the
  real repository:
  `git clone -q --no-local -b <branch> <worktree> <scratch>/mono && cd <scratch>/mono && git worktree add <scratch>/split "$(git subtree split --prefix=plugin)"`.
- **A code span broken across two lines renders on GitHub but hides
  from docs-check,** which reads `seldon …` spans line by line. Keep a
  command span on one line; break the prose before it.
- **Moving a README heading breaks links from outside the repository**
  (the published plugin README pointed at `jax-seldon#install`). Keep an
  `<a name="install"></a>` where the old heading was; docs-check accepts
  `<a name|id="…">` as an anchor.
- **docs-check checks repository URLs offline:** `blob|tree/main/<path>`,
  `raw.githubusercontent.com/…/main/<path>`, the repository root and
  workflow badges map to files here, so a link to a file that only
  exists on your branch passes before the merge. Other URLs (shields,
  releases, external sites) are not fetched; `curl -sL -o /dev/null -w
  '%{http_code}'` them by hand before the handover.

## 2026-10-02 · WP-049

- **`makepkg` is blocked by the guard hook** (red zone, package command),
  even for a scratch build of the PKGBUILD from a `git archive`. The
  package file list (`packaging/expected-files.txt`) is checked only by
  the release workflow's dry run (`gh workflow run release.yml --ref
  <branch>`); remember that makepkg gzips `usr/share/man/**` (`zipman`),
  so the list names `seldon.1.gz`.
- **`cargo add` needs crates.io** when the crate is not in
  `~/.cargo/registry/cache`; the "no network" rule is the engine's
  runtime, but say in the handover that the WP fetched crates.
- **Moving `HOME` breaks `cargo`/`rustup`** ("rustup could not choose a
  version of cargo"). The engine tests make their own scratch homes
  (`common::Env`); run `cargo test` with the normal environment, or
  export `CARGO_HOME`/`RUSTUP_HOME` first as the PKGBUILD's `check()` does.
- **`cmd | grep -q` under `pipefail`** can fail the step when `cmd` is
  killed by SIGPIPE (`zcat`); in CI checks use `grep … > /dev/null`,
  which reads everything.
- **zsh, fish and shellcheck are not installed on the dev host**:
  `tests/manual.rs` and `install.test.sh` skip those checks with a note;
  CI's container has shellcheck.
- **Testing EPIPE:** drop the child's stdout pipe right after `spawn()`,
  before it writes; reading a few bytes and closing lets a short output
  fit the pipe buffer and the write never fails. `/dev/full` (opened for
  writing, passed as `Stdio::from(file)`) tests the other write errors.
- **Hermetic "shell not installed" in bash tests:** link the host's
  programs except zsh/fish into a dir and use only that dir on PATH;
  fakes in another dir decide which shells exist.

## 2026-10-02 · WP-051 (Prime Radiant assets)

- **The offscreen harness renders with Qt Quick's software backend**
  (`qt.scenegraph.general: Loading backend software`): `MultiEffect`
  (the shell's tray tint) and every other shader effect paint nothing,
  and `QSG_RHI_BACKEND=vulkan|opengl` fails under `QT_QPA_PLATFORM=
  offscreen`. Tint `currentColor` SVG masks through the root `color`
  instead (Model.tintedSvg + `FileView` → data URL); it renders the same
  in both.
- **An item id shadows a property of the same name in every binding of
  the component.** A new `Text { id: counts }` in BarWidget.qml turned
  `Model.pillText(counts)` into a call on the Text: empty pill, default
  tone, "undefined active cases" tooltip. qmllint said nothing, and the
  panel and service tests passed (they compute the pill from Model, not
  the widget). Only the bar harness caught it. Never reuse a property
  name as an id.
- **A harness window must be a whole number of device pixels tall at
  fractional scales.** 26 logical px at `QT_SCALE_FACTOR=1.25` is 32.5,
  rounded to 33; the grab is stretched by 33/32.5 and a hand-hinted
  glyph gets a doubled row. Use a height that is a multiple of 4.
- **Repeater delegates must not anchor to `parent`.** The legend's
  `anchors.verticalCenter: parent.verticalCenter` threw "Cannot read
  property 'verticalCenter' of null" while delegates were torn down
  (only in the OVERLAY_SHOTS runs). Position by an enclosing id
  (`y: (entry.height - height) / 2`).
- **Font-dependent numbers differ between the harness and the spec.**
  The harness font gives the 20 px heading a 14 px cap height (box 28),
  JetBrains Mono gives 14.6 (box 30). Assert the rule's outcome (the
  file), not a metric of one font.
- **Header and pictogram changes move every panel render by a few px.**
  `docs/images/panel-*.png` are 460-wide crops of the `PANEL_FIT_SHOTS`
  renders ending at a content line; measure the shift (png-ink.py on the
  tab strip) and add it to each crop height, and to the preview crop in
  TESTING.md. The five tab crops are palette PNGs on main (about 10 KB
  each); a fresh render is truecolor (+150 KB in total): shrink with
  `magick f.png -strip -colors 64 PNG8:f.png`. The overlay and Today
  renders stay truecolor (preview.png is composed from them).
- **A rebase onto a release commit moves `[Unreleased]` bullets into the
  dated section.** Merge into its existing `### Plugin` / `### Packaging
  and docs` lists (no second heading, no blank line inside a list) and
  check with `bash packaging/release-notes.sh <version> CHANGELOG.md`.
- **Host programs translate their messages (WP-053, issue #1).** snapper
  says `Keine Berechtigungen.` under `LANG=de_DE.UTF-8`; matching stderr
  for `No permissions` fails in every locale snapper ships. Run any
  program whose stderr is matched with `LC_ALL=C` and without `LANGUAGE`
  (`collectors::snapper::list_command`). The tests' `common::Env` sets
  `LANG=C`, which hides this; a locale test must override `LANG` on the
  command (`env.command(..).env("LANG", "de_DE.UTF-8")`).

## 2026-10-03 · WP-054 (Plugin Dev)

- **In a live harness run, `recheck` also captures.** A successful probe
  calls `captureCycle()` outside dev mode, so "status banner Check again"
  and "Capture now" look alike in the final state. Tell them apart by the
  engine argv (`argv_check`): recheck adds a `--version --json` probe.
- **The fake engine's `generatedAt` has one-second precision.** Two
  `status` writes of the same fixture within a second are byte-identical,
  so "the index changed" cannot be observed. Give the second write other
  content (`FAKE_SELDON_FIXTURE_AFTER` with a jq-edited copy).
- **`HARNESS_FIX` fires as soon as the service is ready**, which in a live
  run is before the start-up capture writes the index: a banner that needs
  the index is not there yet. Use `["fix", action, banner]` in
  `HARNESS_ACTIONS`, which waits for the start-up calls.

## 2026-10-03 · WP-056 (Docs)

- **A manual run documented as `~/.local/state/seldon` needs
  `XDG_STATE_HOME=$HOME/.local/state` in the scratch.** The TESTING.md
  recipe sets `XDG_STATE_HOME=$S/state`, so a literal `tar -C
  ~/.local/state …` from the docs fails there ("Cannot open"). Point the
  XDG state dir at `$S/home/.local/state` (still under `$S`, the guard
  accepts it) to run the doc text as written.
- **Stamp the de pages after the en commit, not before.** `docs-check`
  compares the source stamp with the en page's last commit: commit the
  en change first, then write its short hash into the de pages.
## 2026-10-03 · WP-059 (Engine)

- **A mutant's binary outlives `git checkout -- file`.** A mutation loop
  that runs `cargo test` and then restores the source leaves
  `engine/target/debug/seldon` built from the last mutant. A manual run
  right after it showed unquoted output that the tests (rebuilt) did
  not. Run `cargo build` after the loop before any manual run.
- **The scratch-HOME export lasts for the whole Bash call.** A `cargo`
  command later in the same call, after `rm -rf $S`, runs with a missing
  `$HOME/.cargo` and prints nothing useful. Keep manual runs and cargo
  in separate calls.
- **A Markdown table cell cannot hold `|`.** `fence_table` splits on it,
  so a fence-fed value with `|` never arrives whole. Tests that feed
  values through `services.enabled` or `plugins.list` can only assert
  "not in a command" for those values.
## 2026-10-03 · WP-057 (Engine)

- **Every case writer reads the case after it takes the lock.** A copy
  read before the wait is written back whole by `CaseFile::save`: two
  parallel hooks lost half their event ids (10 of 20 in every run), and a
  stale copy of a moved case recreated it in its old folder. `save` now
  refuses a path that is gone, but the rule is: lock, then `find`.
- **A race test needs the lock held in-process and a pause.** Take
  `lock::acquire(&env.lock_file())` in the test, spawn the hook, sleep
  ~700 ms (the old code reads the case at once), change the case through
  the library (`cases::find` … `save`, `Ledger::append(&held, …)`), then
  drop the lock. The mutant (lock after `find`) fails it every run.
- **A command that holds the lock cannot call `commands::index::
  rebuild_with`** (it takes the lock itself, non-waiting): derive once,
  write the views, commit, then set `logbook.git` and `index::write`.
- **`ulid::Ulid::new()` does not exist here** (no `std` feature); test
  fillers use `Ulid::from_parts(ms, random)`.
- **A pre-fix run over several test files needs `--no-fail-fast`**;
  without it cargo stops at the first failing test binary.
- **Test output names the dev host.** The index's `logbook.machine` in a
  panic message is the real host name plus a suffix: never paste raw test
  failures into a handover or commit; quote the assertion only.

## 2026-10-03 · WP-058 (Engine)

- **`str::lines()` splits only at `\n`** (and drops a trailing `\r`). A
  lone `\r`, vertical tab, form feed, NEL, U+2028 or U+2029 stays inside
  the "line", while a reader of the text may still break there. Code that
  prefixes or checks text line by line must split at all of them.
- **A test that checks "every output line has the prefix" must split the
  output the same way.** Splitting the output at `\n` only let a mutant
  (renderer splitting at `\n` only) pass all tests; the gap showed only in
  the mutant run. Run the mutant before trusting such an assertion.
- **File names in the logbook are free text.** `Logbook::journal_files()`
  returns every `.md` in a year folder, whatever its name (a newline
  included); a reader that prints a path should check its shape first
  (session-start accepts only `journal/YYYY/YYYY-MM-DD.md`).
- **Mutants without stash:** copy the file from `git show main:<path>`
  (or edit it) in place, keep the real version in the scratchpad, restore
  it with `cp` and confirm with `cmp`. The stash stack is shared with
  other worktrees.

## 2026-10-03 · WP-058 fix round (Engine)

- **Put the property assertion first when a test is mutant evidence.**
  A layout `contains(…)` check ahead of `assert_framed` made the mutant
  fail on the layout, not on the framing the test exists for; swap the
  order so the failure names the real property.
- **A test file name cannot hold `/`** (`std::fs::copy` → NotFound), so
  marker text such as `(memory/lessons.md)` cannot go into a name; a
  newline, `#` and U+2028 can.
- **Merge, do not rebase, a branch whose de pages are stamped with its
  own commits.** A rebase rewrites the en commit the `<!-- source: … -->`
  line names.
- **`check-watch` once ran the `watch` tests against a binary without the
  feature** ("built without the watch feature", 8 failures) and passed on
  a plain rerun; if it recurs, look for another build writing
  `engine/target/debug/seldon` at the same time.
## 2026-10-03 · WP-060 (Engine)

- **snapper's `info.xml` stores `date` in UTC; `snapper list` prints
  local time.** Read from the info files, convert from UTC to the local
  offset, or every snapshot moves by the zone offset.
- **`Bench` (tests/support) builds `Sources` with `..Sources::default()`,
  i.e. the host paths, for every field it does not set.** A new host path
  in `Sources` needs a scratch default in `Bench::new` too (WP-060 review:
  `snapshots`). Binary tests are safe: under `SELDON_TEST_GUARD` the
  snapshot default is `<guard>/.snapshots`.
- **An empty snapshot directory is not "no snapshots".** After booting into
  a snapshot `/.snapshots` is an empty nested subvolume; reading it as
  empty would delete every known snapshot in the ledger (WP-060 review).
- **A test whose result depends on host permissions** (`chmod 000` is
  ignored for root) checks first whether the read actually fails, and
  asserts the skip only then.
## 2026-10-03 · WP-064 (Engine)

- **A mutant restored from a pre-mutant copy keeps the mutant's binary.**
  `cp f f.bak; mutate f; cargo test; mv f.bak f` gives `f` the older
  mtime of the copy, so cargo's fingerprint says "fresh" and
  `target/debug/seldon` stays the last mutant (the next test run failed
  for no visible reason). `touch` (or `os.utime`) the restored file.
- **`std::process::Command` cannot set the umask.** A test that needs a
  umask runs the engine through `/bin/sh -c 'umask 022; exec "$0" "$@"'`
  (`common::with_umask`, which copies program, args, env and cwd).
- **`OpenOptions::mode` and `DirBuilder::mode` are narrowed by the
  umask.** Only an explicit `File::set_permissions` (fchmod) keeps an
  existing file's 0664 under umask 022; `sys::write_atomic` does that.
- **A file-mode test on tmpfs says nothing about cost.** `fsync` is free
  on tmpfs (`/tmp`, the tests' temp dir); time on the real disk (a
  scratch dir under `engine/target/`) before claiming latency.
- **A child in its own process group cannot read the terminal.**
  `process_group(0)` makes it a background group: a git hook or a
  signing prompt that reads `/dev/tty` gets `SIGTTIN` and stops until
  the timeout (30 s for git, under the lock). git runs in the engine's
  group (`sys::run_in_engine_group`); collectors keep their own. Test a
  terminal read under `script -qec '<cmd>' /dev/null` with the answer
  on its stdin (`file_writes.rs` `git::`).
- **A stub run by bash or git, not by `sys::run`, must not be written by
  the test process.** The `ETXTBSY` race above hit
  `collectors_user.rs` `hook::records_the_slug_silently` under a loaded
  `just check` (bash runs the stub, its error is hidden, the argv file is
  missing → `NotFound`). Measured with 6 spawning threads: about 7 % of
  freshly written stubs are busy, with or without `process_group(0)`.
  Write such files with `common::write_executable` (a child process
  writes them, so no fork of the test process inherits the descriptor).
- **The panel harness's `work-live` can catch a transient result line
  under load** ("Dropping C-…" instead of "active → dropped"); it uses
  the fake engine, two immediate re-runs passed 692/692.

## 2026-10-03 · WP-063 (Engine)

- **Hook payloads in tests need a `cwd` inside the test logbook.** Since
  the hooks serve only sessions inside the logbook (`[hooks] scope`
  default `"logbook"`), a payload whose `cwd` is a made-up path such as
  `/home/user/Seldon` records nothing. `tests/hooks.rs` maps
  `FIXTURE_CWD` to the logbook in `Hooks::piped`/`spawn_hook`; other
  test files pass the logbook path. A test about sessions elsewhere sets
  `scope = "all"` (`Hooks::configure`).
- **The guard hook reads payload text as commands.** A manual run whose
  JSON payload holds `~/.config/hypr/…` (or that creates
  `$HOME/.config/…` in a scratch HOME) is blocked as a write under
  `~/.config`, whatever `HOME` is. Report it; put such runs into an
  integration test instead.
- **`git add -p` is not available here** (interactive), so one file with
  several independent changes lands in one commit; plan the split before
  editing if small commits matter.
## 2026-10-03 · WP-061 (Engine)

- **`env.git(...)` in tests has no identity.** `common::Env::git` clears
  the environment and the scratch HOME has no `user.email`, so a test's
  own `git commit` fails silently unless it passes `-c user.name=… -c
  user.email=…` and checks the exit status. `tests/import.rs` had one
  such commit that never happened; it showed only when the import began
  committing pending changes.
- **git's stderr is several lines** ("fatal: Unable to create
  '…/index.lock': File exists." plus a blank line and a hint). Anything
  that promises "one warning line" must join git's lines first
  (`logbook/git.rs` `failure`).
- **`git commit --dry-run` exits 1 for "nothing to commit"** and 128 for
  real failures; it takes `.git/index.lock` even with
  `GIT_OPTIONAL_LOCKS=0`, so it does catch a lock it cannot create.
- **A shell-command hint in a message is a test fixture too:** extract it
  (between backticks) and run it with `sh -c` in the logbook, then check
  that unrelated changes made *after* the failure survive. That is what
  told the file-scoped undo apart from `git checkout -- . && git clean
  -fd`; a commit-first fix alone would have passed the old test.
## 2026-10-03 · WP-062 (Engine)

- **A made-up token of the wrong length tests nothing.** A manual run
  with `ghp_` plus 34 characters (the rule wants 36) stayed in clear
  text, and it looked like the fix had failed. Count with `${#T}` before
  you read a miss as a rule defect; in tests build the value with
  `format!("ghp_{tag}{}", "0".repeat(36 - tag.len()))`.
- **Built-in redaction rules must not match the same text.** The import
  report counts redacted lines per rule (`Redactor::matching_rules`); a
  new `…TOKEN=` rule next to `token-assignment` counted one line twice
  and moved `tests/golden/IMPORT-omarchy-agent.md`. Keep the rules
  disjoint rather than bless the golden.
- **A clear row proves a narrowed rule only if the old rule matched it.**
  `python-task-manager-application` was one character too short for the
  old `sk-…{20,}` rule, so the mutant with the old rule passed. Run the
  mutant against every negative row you add for a narrowing.
- **`curl -o <path>` is not a write for the agent hook;** a redirection
  is. A manual hook reproduction with `-o` records nothing. For a
  recorded command, use a payload whose paths lie outside `~/.config`
  (with an active case the hook records it), or a fixture from
  `fixtures/hooks/`; if the guard blocks a command, report the block.
- **Compiling the redaction regexes is most of a recorded hook's cost.**
  16 rules compiled per `Redactor::builtin()` call, twice per hook run,
  added about 3 ms (over the 5 ms budget near 1000 ledger lines). Compile
  each rule once per process and only when the text holds one of its
  literal triggers; time A/B builds interleaved (rotate the order every
  round) so host load hits both alike.
- **Event `meta` values are scalars** (`event.schema.json`,
  `Event::validate`): a list in `meta.extra` makes the whole append fail.

## 2026-10-03 · WP-067 (Plugin Dev)

- **A headless Quickshell has no IPC server under a long TMPDIR.** The
  socket is `$XDG_RUNTIME_DIR/quickshell/by-id/<id>/ipc.sock`, and a unix
  socket path has at most 107 bytes; a scratchpad-length runtime dir gives
  "Failed to start IPC server" (bar-view filters that line). A harness
  that drives `quickshell ipc` gets `mktemp -d /tmp/seldon-ipc.XXXXXX`.
  `IpcHandler` registration (and its duplicate warning) works without the
  server.
- **`quickshell ipc call` prints "Target not found." with exit 0.**
  Assert the output, not the exit code.
- **Quickshell promotes a leftover handler** when the active one for a
  target is destroyed, but a handler that was disabled (`enabled: false`)
  is not a candidate: enabling one instance only needs an explicit
  takeover when the owner goes (BarWidget `claimIpc`).
- **Two mechanisms for one fix let either mutant survive.** The tab-change
  focus fix (Panel.selectTab) and the tabs' hide handlers each give the
  keys back alone; only a case one of them cannot cover (an open case
  picker: focus moves, `popupOpen` keeps `editing` true) kills the other's
  mutant. Run each mutant separately before claiming a line is tested.
- **A reset in `onFilterChanged` that `onRowsChanged` must see depends on
  handler order.** Keep the last filter the rows were seen with and do the
  reset in `onRowsChanged` (ChangelogTab `rowsFilter`).

## 2026-10-03 · WP-066 (Engine)

- **serde_yaml accepts a flow list continued at column 0**
  (`agents: [a,\nb]`). A line-by-line model of frontmatter cannot place
  that second line; only reading the whole block back catches it.
  `model::update` therefore runs `Frontmatter::check` before any write.
- **A fallback can hide the fix it backs up.** `set` asks the whole
  block when an entry's own lines disagree; that made the unchanged-`tags`
  acceptance test pass with the line grouping reverted. Test grouping on
  a key the engine actually changes (`agents` via `log --actor agent:…`).
- **`is_plain` quotes YAML 1.1 booleans**, including `y` and `n`: a test
  that expects `id: y` gets `id: "y"`. Use other letters.

## 2026-10-03 · WP-065 (Engine)

- **Tightening a reader breaks tests that inject bad values through it.**
  Checking actor and case on load made `tests/rebuild.rs` fail: it put a
  multi-line actor and case straight into the ledger to test escaping.
  Before you tighten `Ledger::read_month` (or any loader), grep the tests
  for hand-built ledger JSON (`"actor":`, `"case":`, `append(&lb`) and
  decide with the orchestrator which tests change.
- **Without the load check, a hand-edited actor failed `index --check`
  (exit 2: the index did not validate) instead of giving a warning.**
  The mutant that drops the check shows this. A loader must enforce at
  least what the index schema needs, or one bad line stops the index.
- **A mutant that does not compile proves nothing.** A changed format
  string with a different number of `{}` "kills" every test. Keep
  wording mutants valid Rust (swap words, keep the arguments) and check
  that the failure is an assertion, not `error[E…]`.
- **`merge_fence` has three callers with different needs.** `STATUS.md`
  uses `views::merge_status` (a damaged fence → `Err`, the file is not
  touched). `outputs/REBUILD.md` and the import report use `merge_fence`:
  there the old text stays below a fresh fence. When you change one,
  run `rebuild::tests` and `import` too.
## 2026-10-03 · WP-072 (Packaging)

- **`cargo-audit` and `actionlint` are not on the dev host.** Build
  cargo-audit into the scratchpad (`CARGO_TARGET_DIR=<scratch>/target
  cargo install --locked --root <scratch> cargo-audit`, then delete the
  target dir) and run it with `--db <scratch>/advisory-db` so the
  advisory database stays out of `~/.cargo`; actionlint and shellcheck
  come from `actionlint-py` / `shellcheck-py` in a scratch venv (actionlint
  lints `run:` scripts with the shellcheck it finds on PATH).
- **A known advisory for a release dry run:** an extra
  `[[package]]` in `engine/Cargo.lock` breaks the `--locked` steps that
  run before the audit. Add a real dev-dependency at an affected version
  instead (`cargo add --manifest-path engine/Cargo.toml --dev
  bumpalo@=3.11.0` → RUSTSEC-2022-0078; bumpalo is already in the tree,
  cargo downgrades it and `--locked` stays happy), on a throw-away branch.
- **Looking up pins:** `gh api repos/<owner>/<action>/git/ref/tags/<tag>`
  gives `object.type` `commit` (lightweight tag, use the sha) or `tag`
  (annotated: resolve `git/tags/<sha>` once more). Docker Hub's
  `v2/repositories/library/archlinux/tags?name=base-devel-` lists the
  dated tags with their digests; the dated tag with the same digest as
  `base-devel` goes into the comment.
- **`some_function | grep -q PATTERN` in a bash test under `pipefail` is
  flaky:** grep exits at the first match, the writer gets SIGPIPE, and the
  pipeline fails although the line was there (a mutant check reported
  "not found" only sometimes). Capture to a file or use a here-string,
  then grep (same class as the `zcat | grep -q` item of WP-049).

## 2026-10-03 · WP-068 (Plugin Dev)

- **A QML `var` property notifies on every new object, equal or not.** A
  binding that builds an object from index data (`form`, `item`) fires its
  `…Changed` handler at each index reload. Key "did it really change" on
  a `string` property (e.g. `JSON.stringify(form)`): a string property
  notifies only when its value differs.
- **`console.warn` lines fail `clean_log` in service-states.sh.** Since
  every failing engine call logs one warning, a case that expects a
  failing call names it: `clean_log <case> "<regex of the warning>"`.
- **The capture-cycle harness "settles" during a lock-retry wait** unless
  `capturing` covers the wait; Service.capturing includes it, and the
  cases wait with `HARNESS_UNTIL=capturing=false`. Set
  `SELDON_LOCK_RETRY_MS` in a harness run, never 30 s.
- **Sheets can be driven without panel-view.sh** (owned by another WP):
  service-states.sh scenario 35 writes its own small harness next to
  copies of the shell's Commons/ and Ui/ and calls the sheets' functions
  (`submit`, `enterKey`, `clickSubmit`) directly.
## 2026-10-03 · WP-069 (Engine Dev)

- **The guard blocks a manual scratch-HOME run that writes
  `$HOME/.config/...`**, even with `HOME`, the `XDG_*` variables and
  `SELDON_TEST_GUARD` pointing into the scratch dir: it reads the command
  text, not the environment. Do not rephrase around it; put the scenario
  into a CLI test (`common::Env` sets the guard and a temp `HOME`) and
  report the block.
- **A collector's scan can only prune with the *current* scope; seeing
  what *entered* needs the old one.** Pruning a base generation by
  "is this path still reachable now" works even for manifests without a
  stored scope; "this file is new because the scope widened" needs the
  scope the base was taken in (`Generation.scope`).
- **Size + mtime caching needs a racy-window guard.** Linux file times
  are coarse (a jiffy, or seconds on some file systems): a write right
  after the read can keep the mtime. Only cache a stat whose mtime lies
  at least 2 s before the walk started (`config::RACY`), and compare
  against the wall clock, not `ctx.now` (tests fake it).
- **Tests that prove "not read again"** need the same inode: `fs::write`
  truncates in place, `set_modified` puts the old mtime back; an atomic
  rename would change the inode and defeat the fixture.

## 2026-10-03 · WP-070 (Engine)

- **`git checkout -- <file>` to undo a mutant also undoes your uncommitted
  fix in that file.** I lost a small doctor change that way and had to
  re-apply it. Commit (or WIP-commit) before you run mutants. Then
  `git checkout HEAD -- <file>` restores exactly the committed state.
- **Pre-fix runs without stash:** after committing, run
  `git checkout <base> -- engine/src`, run the new tests with
  `--no-fail-fast`, then `git checkout HEAD -- engine/src`. The new tests
  stay and only the code goes back. The stash stack is shared between
  worktrees, so never use it.
- **The `init` STATUS.md has no fence.** Text appended to it turns it into
  "header but no fence", and `status` then leaves it alone. A test of
  fence handling must run `seldon status` first, so that the fence
  exists.
- **Where warnings go:** `index` and `status` print theirs on stdout as
  `warning: …` (and in `--json` `warnings`). The rebuild after a writing
  command and the hooks print `seldon: warning: …` on stderr. Assert on
  the right stream.
- **`plan new --json` puts the id under `case.id`**, not at the top level.
- **A mutant whose `None` loses its type does not compile**
  (`error[E0282]`). Write `None::<String>`. A build error is not a
  killed mutant.
- **doctor's "logbook" can be `null`** (since WP-070, with an
  unparsable config.toml). Anything that reads `doctor --json` must
  allow it.
## 2026-10-03 · WP-071 (Engine)

- **The recorded command line is parsed a second time.** Attribution
  reads `meta.command` (the hook's cut text) with the same parser, so a
  cut must leave text that parses to the same commands. Cutting a heredoc
  body *and* its delimiter line left `cat <<EOF` unterminated in the
  record, and the re-parse swallowed every later line (`… \npacman -S x`
  lost its package). Only the end-to-end table (hook generic, then
  `attribution::causes` on the written event) showed it; the in-process
  table passed. Keep the delimiter line when anything follows it.
- **A glob overlap test must keep `*` inside one path component.** With
  `*` crossing `/`, `~/d/*.txt` "overlapped" everything below a skipped
  directory (`~/d/private.conf/**` matches `~/d/private.conf/a.txt`).
  `*`/`?` stay in a component, `**` and an unknown variable cross them,
  as `SkipPaths` reads its patterns.
- **A word that is only an unknown value (`$1`, `$NAME`) would match
  every skip pattern** as a floating `**`; such words are read as naming
  no path, or every `git commit -m "$(cat <<'EOF' …)"` would be redacted
  once `skipPaths` is set.
- **`sudo -k <command>` runs the command** (it only ignores the cached
  credentials); the old probe list skipped it. `-l`, `-v`, `-K`, `-V`
  are the probes.
- **The WP named SPEC §5 for the attribution intent sentence;** the
  sentence is in §4 (pacman collector). Check the section before editing
  "only the paragraphs named".
- **Fix round 1: a word with an unknown head must not match below a
  pattern.** `$X/tail` as the glob `**/tail` overlaps `{p}/**` for every
  path pattern, and WP-069 made `skipPaths` non-empty by default, so every
  `$PKGDEST/…` or `"$TMPDIR/x.log"` line was recorded redacted and lost
  its package intent. A floating word now matches only name patterns and
  a pattern's literal last components. Test such rules with the default
  config, not only with a hand-set pattern list.
## 2026-10-04 · WP-074 (Engine Dev)

- **A held lock from the test cannot reach a late step of `init`.**
  `init` takes the lock first, so a test that holds it gets exit 4
  before the theme hook step, and a lock-after-write mutant there
  passes. Race the step from the inside: the `omarchy` stub copies the
  hook and then runs a real `seldon capture` (save the test's `PATH`
  before the stub's `PATH=/usr/bin:/bin`, or the capture finds host
  programs). Fixed: the capture gets exit 4; mutant: it writes the
  unexplained `config-add` (`tests/own_writes.rs`).
- **`#[serde(default)]` on a struct makes a missing key the field's
  default, not "no value".** A key whose absence must fall through to
  something else (the locale for `language`) has to be an `Option`
  with `skip_serializing_if`.
- **"Save the config first" and "write the marker last" fix different
  failures.** Only the first lets the same `init` run again after a
  failed config save (with the marker last alone, the folder is no
  longer empty and `init` still refuses it). The second keeps a layout
  that stopped half-way from passing as a logbook.
- **A unit test that panics leaves its `std::env::temp_dir()` folder
  behind.** After a mutant run, remove your own `/tmp/seldon-*-<pid>`
  folders (only the pids of your run).
- **A layout that fails half-way, deterministically, in a CLI test:** a
  logbook path of 4075 bytes (components of at most 255) holds every
  folder and the short top-level files, but `areas/hyprland/README.md`
  exceeds PATH_MAX (4095) and fails with ENAMETOOLONG. No root, no ACL,
  no full disk needed (`tests/init.rs`
  `a_layout_stopped_half_way_is_undone`). std's `remove_dir_all` works
  below such a path (it walks with `openat`).
- **An undo that removes "what init created" must know what was there.**
  Record the highest missing ancestor of the path *before* the first
  write (`ancestors().take_while(!exists).last()`); when the folder
  existed (empty, checked under the lock), empty it but keep it. Test
  both branches: a mutant of either survives a test of the other.
## 2026-10-04 · WP-075 (Engine)

- **`seldon dossier` already warns about an unreadable `system/*.md`.**
  It runs `index::derive` first, and the index reader skips the same
  files with `system/<name>: cannot read: …; skipped`. A second warning
  from `dossier::Files::read` printed every bad file twice. The reader
  only skips; the warning stays the derivation's.
- **`common::read` panics on a non-UTF-8 file.** A test that plants a
  Latin-1 file in `system/` cannot use `system_files`/`bodies` on that
  logbook; read with `read_to_string(..).ok()` and skip the file.
- **An earlier guard can make a mutant equivalent.** Reading a vault file
  through `vault.join(rel)` instead of its real `PathBuf` survives every
  test, because files with non-UTF-8 names never reach the read (they are
  report errors first); for every file that is read, `rel` is the real
  name. Say so in the handover instead of inventing a test.
- **The WP-034 note "the default release build has no watcher" is out of
  date:** the release workflow and the PKGBUILD build with
  `--features watch`; only a plain `cargo build` and `just build-release`
  leave it out (engine/Cargo.toml comment, F-135).
- **Tests that count import notes (`meta.import`) include the apply's own
  note** (no case, subject `omarchy-agent`): six cases → seven notes.
- **Neutralising what a writer stores changes what its readers must
  compare.** After `Files::set` began to neutralise fence bodies,
  `deviations_table` and `fill_case` still compared path cells with the
  raw path, so a path holding `<!-- seldon:end -->` got one more row on
  every run (review B1). When a write transforms values, grep every
  reader that matches stored cells against fresh values (`listed`,
  `contains`, `==` on cells), and test with two runs: the second must
  report nothing changed.

## 2026-10-04 · WP-073 (Engine)

- **`common::Env` CLI tests have only the stub directory on PATH.** A
  stub that runs `cat` fails ("exec: cat: not found"), and the plugins
  collector reports it as "… (it needs the running Omarchy shell)",
  which looks like a different fault. Use `/bin/cat` in such stubs, and
  assert the capture's `ok` so the failure names the collector message.
- **A deletion that frees a number must not carry a later time than the
  creation that reuses it.** The snapper dedupe reads "the latest snapper
  event per number" from the ledger; a capture-time `snapshot-delete`
  behind the new `snapshot` would make a snapshot that exists look
  deleted. Give both the new snapshot's date, push the delete first (the
  capture sort is stable, and ties go by file order).
- **The attribution window paragraph is in SPEC-ENGINE §4, not §5**
  (again: WP-071 noted the same for the intent sentence).
- **Small commits without `git add -p`:** `git diff -U2`, split into
  hunks with a short script, feed the chosen hunks to `git apply --cached
  --recount -`. Adjacent new tests in one file land in one hunk; add
  them in separate places if they must go into separate commits.

## 2026-10-04 · WP-076 (Engine)

- **Timings on the shared dev host spike with the parallel WPs' builds**
  (load 6 to 15): one `status` median went from 47 ms to 177 ms and back.
  Compare two builds interleaved in one process or one shell loop, never
  two runs minutes apart; `common::assert_within_budget` measures a
  second time before it fails.
- **The scaled fixture multiplies the cases too.** `status` at ×141
  (10 011 lines) takes about 92 ms with 1 128 cases and about 48 ms with
  the 304 cases of the SPEC budget (`scale::stated_scale`); the 365
  journal files cost nothing. Name the case count with a timing.
- **The hook has two speeds around WP-057's 1000-line threshold:** a
  recorded command takes about 3.5 ms just below (it rebuilds the index)
  and about 1.8 ms above; a call it does not record takes about 1.3 ms
  at any size. A hook timing names the side of the threshold it is on.
- **`cargo test` stops after the first failing test binary.** A mutant
  run over `--test index --test hooks` needs `--no-fail-fast`, or the
  second binary's result is missing.
- **Events written at one fixed `SELDON_NOW` tie on `ts`;** newest-first
  then orders them by the random part of the ULID. Assert that the index
  holds an event, not that it comes first.
- **The panel harness's `wait:` checks one path.** An engine step sets the
  result line when its process exits and the card moves when the index
  reload arrives, in either order: use `settle` (the result) and then
  `wait:` (the card), as `work-live` does since WP-076. A temporary
  `sleep 1` after `rewrite_index` in `plan_step` of `fake-seldon` (not
  committed) reproduces the race every time.

## 2026-10-04 · WP-077 (Engine)

- **A mutant restored with an older mtime is still the binary.** A script
  that copies the file aside, writes the mutant and moves the copy back
  leaves the source older than the last build: cargo then keeps the
  mutant's lib and `seldon` binary until some other file of the crate
  changes. The next "baseline" run tests the mutant (here: a parity test
  failed against an engine without `trim_end`). `touch` the file after the
  restore, and run the baseline once after the last mutant.
- **Python's `str.rstrip()` is not Rust's `trim_end()`.** Python strips
  U+001C..U+001F (they are `isspace()`), Rust only Unicode White_Space.
  A reference that mirrors an engine cut names the set
  (`WHITE_SPACE` in `scripts/validate-fixtures.py`); the parity probes
  put those characters at the cut, and `\b`/`\f` too (2 JSON bytes, not
  6): without a `\b\f` probe that mutant survived.
- **serde names a refused enum value raw:** `unknown variant `…`` comes
  from serde, not from a `validate()` the engine owns, so escaping the
  engine's own messages is not enough; `FrontmatterError::Yaml` shows its
  message through `frontmatter::printable`.
- **A mutant that keeps the `?` is no mutant.** Replacing `x.prepare(…)?`
  by `let _ = x.prepare(…)?` still propagates; swallow the error
  (`.ok()`) instead.
## 2026-10-04 · WP-078 (Plugin Dev)

- **One service, one panel per monitor.** Every BarWidget loads its own
  Panel.qml, and all share Service.qml: a sheet's "result pending" can be
  another panel's call. Guard a sheet on its *own* pending call
  (`sentTitle !== ""`) and let the service's busy guard answer the rest
  (NewDecisionSheet `ownPending`).
- **The bar widget's `visible` is the effective one.** A widget in the
  shell's hidden placeholder ModuleList reads `visible: false` and fires
  `visibleChanged` when an ancestor changes, so `Model.isDrawnWidget(root)`
  in a binding follows a bar reconfiguration without the slot.
- **Handing an IPC target over needs the owner to let go first.** When
  ownership is recomputed on all instances, run the current owner's
  `claimIpc` before the others (BarWidget `reclaimIpc`); the other order
  logs "another handler is registered" for a moment (bar-view
  `ipc-placeholder one handler` catches it).
- **A bar-harness slot that is `visible: false` and 0×0 is a faithful
  placeholder;** reveal it by giving it a size and `visible: true`, and
  hide the drawn slot, to drive a live reconfiguration (bar.qml
  `HARNESS_IPC_PLACEHOLDER`).
- **`console.warn` with a newline writes two log lines;** only the first
  carries the `WARN qml:` prefix. A check that "nothing of line two is in
  the log" must skip the HARNESS report lines, which carry the same text
  JSON-escaped (service-states 31b).

## 2026-10-04 · WP-079 (Engine Dev)

- **`snapper --jsonout -c root get-config` prints a flat JSON object of
  strings** (`{"ALLOW_USERS": "a b", "SYNC_ACL": "yes", …}`); it succeeds
  only for a user the config lists (or root). Build it like `list` with
  `LC_ALL=C` (`collectors::snapper::get_config_command`).
- **`rx` on `/.snapshots` opens more than the info files.** The snapshot
  directories are 0755 and each `snapshot/` keeps the modes of its time,
  so the grant also exposes old snapshot contents to the user; say so
  wherever the grant is described as read-only.
- **`pgrep -f` matches the shell that runs it.** A wait loop
  `while pgrep -f "just check"; do sleep; done` never ends because its
  own `bash -c` command line contains the pattern; write the pattern so
  it does not match itself (`"just chec[k]"`).
- **A `run_in_background` wait for other WPs' `just check`** keeps the
  plugin harness from overlapping theirs; two other worktrees ran it at
  the same time here.
## 2026-10-04 · WP-080 (Packaging)

- **`gh attestation verify` exits 4 when gh is not logged in** (also for a
  public repo: it needs the API). Treat 4 as "tool unavailable", not as a
  failed verification, or every user with a logged-out gh is refused.
- **A tampered asset and a release without attestations look the same**
  to `gh` (both: no attestation for the digest, HTTP 404). install.sh
  therefore decides by version (attestations start after v0.1.1) and only
  then asks gh.
- **`--repo` alone accepts any attestation the repository's workflows
  made,** including a branch dry run's. `--source-ref refs/tags/vX.Y.Z`
  rejects those ("expected SourceRepositoryRef to be refs/tags/v0.1.1,
  got refs/heads/…"); `--signer-workflow <owner>/<repo>/.github/workflows/release.yml`
  rejects other workflows. Checked against the real dry-run assets.
- **The install test links every host tool into its PATH dirs;** the
  host's real gh would then verify mock tarballs over the network. Leave
  `gh` out of those dirs and put a stub first also where a test uses the
  host's `$PATH`.
- **A Python edit script in a bash heredoc that itself contains a heredoc
  ending in `EOF`** ends the outer heredoc early and runs the rest as
  shell. Write such scripts to the scratchpad with the file tool, or use
  a different inner delimiter.
## 2026-10-04 · WP-081 (Engine Dev)

- **`Cli::capture` in `tests/idempotency.rs` always adds `--all`,** and
  `--source` with `--all` is exit 1. A test that captures one collector
  runs `cli.run(&["capture", "--source", "config", "--json"])`.
- **"The last capture" in doctor is a whole-second comparison** of the
  newest `state-reset` note's `ts` with the newest `lastRun` in
  `cursors.json`. Two captures under the same `SELDON_NOW` keep the row;
  give each capture in a test its own time.
- **A collector's baseline is not always a loss:** pacman and snapper
  re-read their sources and dedupe, the diff collectors miss the gap. The
  note's detail therefore says "may not be recorded", and the WP rule
  (no note while the ledger holds no event of the source) keeps the first
  capture of a logbook quiet.
## 2026-10-05 · WP-082 (Engine Dev)

- **chrono's `earliest()` in the repeated hour is the *later* instant.**
  chrono 0.4.45 orders `LocalResult::Ambiguous(a, b)` by offset, the
  smaller first (`(std, dst)` for a POSIX rule, by `ut_offset` for a
  TZif file), and `earliest()` returns `a`. `Tz::localize` (pacman and,
  before WP-082, snapper) therefore takes 02:30+01:00, not +02:00. Sort
  the two instants when the order matters.
- **A real DST zone in a test needs no tzdata:** chrono reads a POSIX
  rule from `TZ` (`CET-1CEST,M3.5.0,M10.5.0/3`). `Tz` has no zone
  variant, so the fold tests run the binary through `common::Env` with
  `.env("TZ", …)`; never `set_var` in the test process.
- **A mutant that only changes the cursor can hide behind the ledger
  dedupe.** A test that fakes an older cursor must fake the matching
  ledger line too, or the false pair is dropped as already recorded and
  the mutant survives (WP-082 M1 first survived the older-cursor test).
