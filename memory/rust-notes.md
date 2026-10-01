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
