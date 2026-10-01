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
