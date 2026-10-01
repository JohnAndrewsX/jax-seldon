# Rust notes — engine conventions and crate behaviour worth knowing

Append-only. One bullet per finding, newest section last.

## 2026-10-01 · WP-003 (engine core)

**Layout and conventions**
- **Crate shape.** `engine/` is a library (`src/lib.rs`) plus a thin binary
  (`src/main.rs`), so `engine/tests/` can use the types directly.
  - Commands live in `src/commands/<name>.rs`. Each returns
    `Result<Output, Error>`, where `Output` = human text + JSON value + exit code.
  - `main.rs` prints either the text or the JSON.
  - `error::Error` decides the exit code: `User` 1, `Engine` 2,
    `NotInitialised` 3, `LockHeld` 4.
- **Read the environment once.** `commands::Context::from_env` reads it once
  (XDG dirs, `SELDON_LOGBOOK`, global flags). Pass the context down; do not
  read env vars deep inside commands.
- **Frontmatter: read through serde, write entry by entry.**
  - `frontmatter::Document` keeps each top-level key with its raw text.
  - Reading = `serde_yaml` on the whole block, into the record struct.
  - `model::update` = lossless: only changed keys are rewritten, and a
    missing empty key is not added.
  - `model::render_new` = the canonical style of `fixtures/logbook/`. Free
    text (title, description) is always double-quoted. Ids and dates are
    plain when YAML reads them back unchanged. Null is `key:`, lists are
    `[a, b]`.
  - New record types implement `model::Record` (`TYPE`, `KEYS`, `to_values`,
    `validate`).
- **Round-trip tests use the strong form.** Parse into the typed record,
  write a fresh canonical block, append the body, and compare bytes. Pass-through
  alone proves nothing.
- **Templates are compiled into the binary.** `logbook/templates.rs` pulls
  them in with `include_str!`. A new template file must also be added to
  `TEMPLATES`. A unit test parses every rendered template.

**Crate behaviour**
- **serde_yaml (YAML 1.2)** reads `created: 2026-10-01` as a string; chrono's
  `NaiveDate` deserialises from it. It reads `yes`/`off` as strings, but
  YAML 1.1 readers (Obsidian, PyYAML) read them as booleans, so the writer
  quotes them. Flow lists with `agent:claude-code` items parse fine. The
  crate is `0.9.34+deprecated` upstream; it is still on the allowed list.
- **clap `ignore_errors(true)` is not a lenient parser.** It stops at an
  invalid value (`--language fr`) and at an unknown subcommand, and returns
  whatever it had parsed so far.
  - For "did the user ask for `--json`?", `main.rs::wants_json` lexes argv
    against the built `Command`, using `get_arguments`,
    `get_action().takes_values()`, `is_allow_hyphen_values_set` and
    `find_subcommand`. It stops at `--`.
  - `err.to_string()` starts with `error: ` and ends with a usage footer;
    `detail()` strips both.
- **`std::fs::File::try_lock`** (Rust 1.89) gives `flock` without a libc
  crate. The lock belongs to the open file description, so a second `open`
  plus `try_lock` in the same process conflicts. That makes exit 4
  testable in-process. Because of this API, `rust-version` is 1.89.
- **Edition 2024 + clippy `collapsible_if` → let-chains.** Clippy asks for
  let-chains (`if a && let Some(x) = y {`) instead of nested `if let`.
- **Derives inside `macro_rules!` need absolute paths.** A derive used in a
  macro that is called from another module must be written
  `::serde::Serialize` and `::std::fmt::Display`; otherwise it does not
  resolve at the call site.
- **toml (1.x) has no `preserve_order`.** `toml::Table` is sorted, so a
  re-saved `config.toml` has alphabetical keys and loses its comments.
  `Config::save` carries unknown keys over from the old file.
  `toml::value::Datetime` round-trips `created = 2026-09-01T19:00:42+02:00`
  unquoted, exactly like the fixture.
- **dialoguer with `default-features = false`** still has `Select`,
  `MultiSelect`, `Input`, `Confirm` and `ColorfulTheme`. The default
  features only add `editor`/`password` (tempfile, zeroize).
- **Random suffix without the rand crate.** std's
  `RandomState::new().build_hasher()` is randomly keyed per process: good
  enough for the machine-id suffix, not for cryptography.

**Testing**
- **Driving the wizard without a terminal:**
  `(sleep 1; printf '\r'; …) | script -qec "seldon init" /dev/null`.
  `script` provides the pty that dialoguer needs. Arrow down is `\033[B`.

## 2026-10-01 · WP-004 (collectors core)

**Layout and conventions**
- **Events reach the logbook only through the ledger.** Build an event with
  `Event::new(ts, source, kind, subject)` and the builder methods, then call
  `Ledger::append(&lock, events)`. `append`:
  - assigns ULIDs, monotonic within one call (an id you set is overwritten);
  - redacts `detail` and `meta.command`;
  - validates each event, and writes nothing if one is invalid;
  - appends to the month file of `ts`, in the event's own offset.

  See `work/active/WP-004/FOUNDATION.md`.
- **Collectors never write.** `Collector::collect(ctx, cursor)` returns an
  `Outcome`. `capture` appends first and saves `cursors.json` after, so a
  failed write never moves a cursor. A collector without a cursor takes a
  baseline at `Ctx::baseline`: the logbook's `created`, or `--since`.
- **Host paths and programs are in `collectors::Sources`.** They are read once
  from `SELDON_PACMAN_LOG`, `SELDON_PACMAN_DB_LOCK`, `SELDON_SNAPPER`,
  `SELDON_OMARCHY_VERSION` and `SELDON_PACMAN`. Tests and acceptance runs
  point them at fixtures.
- **Time zones are injected.** Naive local times (snapper `date`, old log
  lines) go through `collectors::Tz`. Tests use `Tz::Fixed(+02:00)`. The CLI
  test sets `TZ=Europe/Berlin` on the child process, and chrono's `Local`
  honours it.
- **The test bench is `engine/tests/support/`**, separate from WP-003's
  `common/`. It runs collectors in-process at a chosen `now`, with stub
  programs in a scratch `bin/`. `SELDON_BLESS=1 cargo test` rewrites
  `tests/golden/`.
- **Superseded:** the "random suffix without the rand crate" note above.
  `sys::random_hex` now takes ULID randomness, and `rand` arrives with `ulid`.

**Crate behaviour**
- **ulid 3:**
  - The constructor is `Ulid::generate()`; `Ulid::new` does not exist in 3.x.
  - `Generator::generate()` is monotonic.
  - `Ulid::nil()` passes a pattern check (`000…`), so test `is_nil()` explicitly.
  - The `serde` feature serialises a ULID as the 26-character string.
- **serde_json without `preserve_order` sorts `Value` maps.** To keep the
  ledger's key order, `meta` is a typed struct: the conventional keys in
  fixture order, plus `#[serde(flatten)] extra: BTreeMap`. All 67 fixture
  lines round-trip byte for byte.
- **`regex` has no look-around.** To keep a delimiter, capture it and use a
  replacement template (`${1}‹redacted›${2}`). A "keep prefix" flag silently
  ate the `@` of `https://user:pw@host`.
- **jsonschema 0.58 with `default-features = false`** works offline:
  `jsonschema::options().should_validate_formats(true).build(&schema)`, then
  `iter_errors(&v)`. It is a dev-dependency only (about 40 transitive crates).
- **`DateTime<FixedOffset>` equality compares instants.** Dedupe keys use
  `ts.timestamp()`. `to_rfc3339_opts(AutoSi, false)` keeps `+00:00` (no `Z`)
  and prints no fraction for whole seconds.
## 2026-10-01 · WP-006 (log, event, plan, decide, open)

**Conventions**
- **Command arguments live with the command.** `#[derive(clap::Args)]`
  structs sit in `commands/<name>.rs` (lib), so `main.rs` adds exactly one
  variant `Log(commands::log::LogArgs)` and one dispatch arm per command.
  Enums made with `str_enum!` (`Zone`, `Risk`, `CaseStatus`, `Source`,
  `Kind`) work as clap value types directly: they implement `FromStr` with a
  `String` error.
- **One clock per invocation.** `Context::now` (`DateTime<FixedOffset>`,
  `SELDON_NOW` overrides) is used for every `ts`, journal heading, Log line
  and case-id year of one command, so they agree and tests are exact.
- **Writing events.** `commands::emit` / `emit_one` wrap
  `Ledger::append`: the ledger assigns ids, redacts, validates. A command
  that must record the id (a note in `case.events`) appends to the ledger
  first and writes the case file after.
- **Case files change in two ways only**: `model::update` (changed keys)
  and `cases::append_log` (a line after the last non-blank line of `## Log`).
  Section boundaries are level-1/2 headings outside code fences.
- **Free text**: positional with `allow_hyphen_values = true`, so
  `seldon log -x foo` is text; options taking free text (`--reason`,
  `--detail`, `--subject`) too.

**Crate behaviour**
- **clap: a known flag always beats a hyphen-accepting positional.**
  `allow_hyphen_values` on a single-value positional only turns *unknown*
  `-x`/`--foo` tokens into the value (`parse_long_arg` checks the keymap
  first). So `seldon log --json` is the flag and the text is missing; the
  text `--json` needs `--` (`seldon log -- --json`) — hence CONTRACT.md's
  `-- <text>` form. An *option* with `allow_hyphen_values` does take
  `--json` as its value (`--reason --json`).
- **`wants_json` mirrors that rule**: positional slots per (sub)command via
  `get_positionals()`, unknown hyphen tokens fill a hyphen-accepting slot,
  `--` ends everything.
- **`std::slice::from_ref(&x)`** instead of `[x.clone()]` when comparing a
  `Vec` with one element: clippy `cloned_ref_to_slice_refs` (Rust 1.98).
- **jsonschema 0.58 in tests**: `jsonschema::options()
  .should_validate_formats(true).build(&schema)`, then `iter_errors`.
  `case.schema.json` `$ref`s `event.schema.json#/$defs/…`; the test helper
  inlines the event `$defs` instead of building a registry.

## 2026-10-01 · WP-005 (collectors: plugins, theme, config)

**Conventions**
- **`collect_from` for tests.** Each WP-005 collector has
  `collect_from(ctx, cursor, <explicit sources>)`. `collect` only reads the
  env overrides (`SELDON_OMARCHY`, `SELDON_OMARCHY_PLUGINS_DIR`,
  `SELDON_THEME_FILE`) and calls it. In-process tests pass temp paths and
  stub programs directly, so they need no env and no `Sources` field.
- **Cursor timestamps.** `DateTime<FixedOffset>` serialises as RFC 3339 with
  chrono's `serde` feature, so a cursor struct can hold `checked: DateTime<…>`
  directly.
- **mtime → event ts.** `DateTime::<Local>::from(SystemTime).fixed_offset()`
  (or `DateTime::<Utc>::from(t).with_timezone(&off)` for `Tz::Fixed`), then
  `with_nanosecond(0)`. Clamp to `[last check, now]` (`config::changed_at`).
- **SHA-256 lives in `sys::sha256{,_hex}`** (no hashing crate is allowed).
  Tested against the FIPS vectors and coreutils' `sha256sum`.

**Crate and compiler behaviour**
- **clippy rejects `chunks_exact(N)` with a constant N** ("using
  `chunks_exact` with a constant chunk size"): use `slice.as_chunks::<N>()` (stable since 1.88,
  inside `rust-version = 1.89`). It gives `&[[T; N]]`, so
  `u32::from_be_bytes(*word)` needs no indexing.
- **Returning a closure that must not borrow its argument** (edition 2024
  captures every lifetime in `impl Trait`): write
  `-> impl FnOnce(&Ctx, Option<&Value>) -> Outcome + use<>` and move owned
  copies into it.
- **`std::process::Command` searches the child's PATH** when the test sets
  `.env("PATH", …)`. With PATH = a stub dir, `Command::new("bash")` fails;
  resolve the host binary first and pass the absolute path.
- **`File::set_modified`** (std 1.75) sets an mtime in tests; open the file
  with `.write(true)`.
- **`#[serde(flatten)]` on a struct field** keeps a nested struct's keys at
  the top level (`Manifest { current: Generation, previous }` →
  `{"hash", "files", "skipped", "previous"}`).

## 2026-10-01 · WP-007 (index, status, views)

**Conventions**
- **The index is typed structs** (`index::model`), not `serde_json::Value`:
  without `preserve_order` a `Value` map is sorted, a struct keeps the
  fixture's key order. `IndexEvent` implements `Serialize` by hand
  (`serialize_map`) so `resolutionDetail` sits between `resolution` and
  `meta` without adding an index-only field to the ledger `Event`.
- **Absent, null or a value:** `Option<Option<String>>` with
  `skip_serializing_if = "Option::is_none"` (timeline `end`: absent for
  releases, `null` for an open case).
- **Index pipeline:** `index::load::load` (read once, skip broken files with
  a warning) → `index::build::build` (pure, testable in-process by
  mutating `Loaded`) → `index::write` (atomic). Tests port the mutation
  self-checks of `scripts/validate-fixtures.py` this way, without touching
  the fixture files.
- **Writing commands call `index::rebuild_if_initialised(ctx)` after the
  autocommit**, still under the lock; before it, `logbook.git.dirty` would
  always be true.
- **Generated Markdown is written only when its text changed**
  (`views::write_if_changed`), so `status` commits only real changes.

**Crate behaviour**
- **jsonschema 0.58 with `$ref`s across files:** implement
  `jsonschema::Retrieve` over the schema files keyed by `$id` and pass it
  with `.with_retriever(...)` (`tests/common::index_errors`).
- **The engine binary cannot use jsonschema** (test-only crate):
  `index::check` is a small Draft 2020-12 validator over the
  `include_str!`-ed schemas that fails on any keyword it does not know;
  `tests/index.rs` holds it to jsonschema on the sample and on
  `fixtures/invalid/`.
- **Benches without a bench crate:** `[[bench]] harness = false` plus a
  `fn main()` timed with `std::time`; share test helpers with
  `#[path = "../tests/common/scale.rs"] mod scale;`.
- **`regex` has no look-around**, so the ADR-0015 token rule is a scan over
  every match with explicit neighbour checks (`index::drift::names_token`).
- **clippy 1.98:** `unnecessary_sort_by` wants
  `sort_by_key(|x| Reverse(..))`; `type_complexity` fires on a tuple of
  boxed closures in a test, so use a local `type` alias.
- **`[profile.bench]` inherits `release`.** Overriding only `lto = "thin"`
  and `codegen-units = 16` halves the CI bench compile (58 s → 28 s here)
  and leaves the index timing where it was (×10 median 4.7 ms).
- **Reading `HEAD` without git** (`index::git_head_fast`): `.git/HEAD` is
  `ref: refs/heads/x` or a bare hash; the ref is a loose file or a line
  `<sha> <ref>` in `packed-refs`; a `.git` *file* (`gitdir:`) and
  `commondir` cover worktrees. The first 7 hex characters match
  `git rev-parse --short=7`.

## 2026-10-01 · WP-009 (hooks, pkgcmd, attribution)

**Conventions**
- **A command that must never fail is dispatched before `run()`.** `main`
  matches `Some(Command::Hook(h)) = &cli.command` with
  `h.command.is_agent_hook()` and calls `hook::run_agent_hook(|| Context::from_env(…), cmd)`:
  the context is built inside the closure, so a bad `SELDON_NOW`, a missing
  HOME or a broken config also lands on stderr with exit 0.
- **Moving code without touching its tests:** `pub use crate::pkgcmd::{…}`
  and `pub use crate::attribution::{…}` in `collectors/pacman.rs` keep every
  old path (`pacman::parse_command`, `super::*` in its unit tests) valid.
- **One owned value into a `&mut [T]` API:** `std::slice::from_mut(&mut event)`.
- **Hook payloads are typed with only the fields read** (`#[serde(default)]`
  on each, `tool_input: Value`); `tool_response` is never named, so the
  command's output never reaches a Rust value Seldon keeps.

**Crate behaviour**
- **clippy `filter_next`-style lint on `DoubleEndedIterator`:** write
  `.rfind(pred)`, not `.filter(pred).next_back()`.
- **serde_json without `preserve_order` sorts object keys** (`Map` is a
  `BTreeMap`). Rewriting a user's JSON file (`.claude/settings.json`) keeps
  every value but reorders keys; `preserve_order` needs `indexmap`, which is
  not on the allowed list.
- **`std::io::IsTerminal` on stdin** before `read_to_end`: a hook run by
  hand from a terminal would otherwise wait for EOF.
- **Release timing** of `seldon hook claude-code` (musl not needed for the
  measure): about 2.0 ms per mutating call, 1.1 ms per non-mutating call,
  of which about 0.65 ms is process spawn (`env -i … true`). The
  non-mutating path compiles no redaction regex and touches no ledger.
  With WP-007's fast index rebuild after a write: about 3.1 ms on a fresh
  logbook, 3.8 ms at about 200 ledger lines (it grows linearly).
- **`catch_unwind` is a no-op under `panic = "abort"`** (our release
  profile). To make a process exit 0 on any panic, install
  `std::panic::set_hook` that writes to stderr with `writeln!` (not
  `eprintln!`, which panics again on a closed stderr) and calls
  `std::process::exit(0)`; the hook runs before the abort.
- **`print!` panics on EPIPE** (Rust ignores SIGPIPE, so the write
  returns an error and `print!` unwraps it). Output whose reader may go
  away: `stdout().lock().write_all(..)` and ignore the result. Test it by
  dropping the child's stdout pipe before it writes.
- **Test-only triggers behind `#[cfg(debug_assertions)]`** (an env var
  that forces a panic) never reach the release binary.

## 2026-10-01 · WP-008 (drift commands, reconcile, detached editor)

- **Reuse the index model instead of re-deriving.** `index::derive`
  returns `Built { folded, open_drift, ledger, index }`. A group's open
  members are `folded` events in `open_drift` with the same `txId`
  (pacman only); the row is the `index.drift` item whose `eventId` is one
  of them (it is absent when the 200-item cap cut it).
- **clap: a bare command plus subcommands.** `#[command(
  args_conflicts_with_subcommands = true)]` on the args struct with
  `command: Option<Sub>` gives `drift [--crisis-only]` and
  `drift link …` from one variant. Free text is a positional with
  `allow_hyphen_values = true`, so `-- --user unit…` passes.
- **Detached child:** `Command::process_group(0)` (std
  `os::unix::process::CommandExt`) plus `Stdio::null()` on all three
  streams, then poll `try_wait` for a short grace period and drop the
  `Child` without waiting. A test caller's `output()` blocks until every
  inherited pipe closes, so a test that times `output()` proves that no
  pipe leaked.
- **chrono overflow:** `ledger.read_range(from, MAX)` panics, because it
  adds a day of slack. Use a far but finite bound (`from + 100 years`).
- **`/proc/<pid>/stat`**: split after the last `)`; then field 0 is the
  state and field 2 is the pgrp. That lets a test check "own process
  group" and "still alive (not Z)" without a libc crate.

## 2026-10-01 · WP-022 (agent start)

- **A detached child's stderr without a pipe:** hand `Command::stderr` a
  `Stdio::from(File::create(log)?)`. A file never blocks the caller (a
  pipe would keep `output()` of a test, or the plugin's process queue,
  waiting until the child exits), and a launcher that fails inside the
  grace period can still be reported with its own message (read the
  file's last lines). `open::launch_detached(cmd, stderr)` now takes a
  prepared `Command`, so the caller sets `current_dir` and `env` too.
- **`#[serde(skip_serializing_if = "AgentConfig::is_default")]` on a
  config section** keeps `seldon init`'s config.toml unchanged when a
  new section is added (no fixture or init test moves), and
  `Config::save`'s `merge_unknown` still carries a user's own section
  over when the loaded value equals the default.
- **Recording argv in a `/bin/sh` stub:** `printf '%s\0' "$@" >> file`
  then split on `\0` in the test; one argument with newlines, quotes or
  `$(…)` stays one element. Append a line per call to a second file to
  prove "exactly one launch".
- **Matching a slice pattern on a filtered `Vec`:**
  `match v.iter().enumerate().filter(..).collect::<Vec<_>>().as_slice()
  { [(i, a)] if … => …, [] => …, [_] => …, _ => … }` reads the
  "exactly once, as a whole element" rule in one expression.
## 2026-10-01 · WP-024 (wizard steps, templates)

- **One command calling another in-process** (`init` → `capture::run`,
  `setup::baseline`): clone the `Context` and pin `logbook_flag` to the
  logbook just created, or `--logbook`/`SELDON_LOGBOOK` would send the
  capture elsewhere. Release your own flock first: a second
  `lock::acquire` in the same process is a new open file description and
  sees `WouldBlock` (exit 4).
- **Bulk resolutions reuse WP-008:** iterate `Built::open_drift` (every
  open member; `index.drift` is capped at 200, ADR-0020), oldest first,
  `reconcile::select(built, id, false)` per event not yet covered, then
  `reconcile::resolutions`; one `emit` for all lines.
- **Files outside `src/` in the binary:** `include_str!(concat!(env!(
  "CARGO_MANIFEST_DIR"), "/hooks/theme-set.sh"))`; a unit test pins the
  shebang and the one command so a moved file fails loudly.
- **Copying a tree:** `DirEntry::file_type()` does not follow symlinks, so
  `is_file()`/`is_dir()` skip links for free; `std::fs::copy` keeps the
  permission bits (an executable guard stays executable).
- **clap `requires`/`conflicts_with`** turn flag combinations into parse
  errors (exit 1, the flag names in the message) before anything is
  written; cheaper than checking in `run`.
- **dialoguer `Input::validate_with`** needs the closure typed:
  `|s: &String| -> std::result::Result<(), String>`.
- **Snapshot of a template skeleton:** frontmatter keys, `#` headings,
  fence names and table header rows per file, the machine id normalised;
  equal across languages and to `tests/golden/init-skeleton.txt`
  (`SELDON_BLESS=1` rewrites). Prose may change without touching it.
- **Environment as a function for testable setup:**
  `Dirs::from_vars(|name| …)` lets a unit test feed `HOME`/`XDG_*`/
  `SELDON_TEST_GUARD` without `std::env::set_var` (unsafe in edition 2024,
  and racy across test threads); `from_env` passes `std::env::var_os`.
- **Path containment checks:** `starts_with` on raw paths is fooled by `..`
  and symbolic links. Fold `.`/`..` component by component and
  `canonicalize` each prefix that exists (the tail may not exist yet), for
  both the guard and the checked path.
- **A rebuild after the commit, not before:** the index's `logbook.git`
  (head, dirty) is taken at rebuild time. A command that commits must
  rebuild after the commit, under the same lock, or the index shows the
  old head and `dirty: true` until the next write. A negative control
  needs a step that actually commits: with nothing to commit the stale and
  the fresh index agree.
