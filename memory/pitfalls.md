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
