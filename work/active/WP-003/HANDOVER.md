WP-003 HANDOVER

Branch `wp/003-engine-core`, worktree `wt/WP-003`. Not pushed, no PR.
Five commits on top of `b4a5eb3`. Each commit builds, is clippy-clean and passes its tests. This was checked in a throw-away worktree.

## Done

- **Crate shape.** `engine/` is now a library plus a thin CLI.
  - `src/lib.rs` holds the modules; `src/main.rs` only parses arguments and dispatches.
  - New modules: `error`, `sys`, `commands/{init,doctor}`.
  - `sys` holds the atomic write, a subprocess runner with a timeout (fixed argv, no shell) and the host name.
  - Exit codes come from `error::Error`: 1 user, 2 engine, 3 not initialised, 4 lock held.
  - Global flags: `--json`, `--logbook DIR`, `--quiet`, `--no-commit`.
- **`frontmatter.rs`.** Parses frontmatter and writes it back in two ways.
  - The block is kept entry by entry, with each entry's raw text. The body is preserved byte-for-byte, and CRLF works.
  - Reading uses `serde_yaml`.
  - `Frontmatter::set` updates losslessly: it rewrites only a key whose value changed and inserts a missing key in canonical order. Comments and quoting on other lines stay as they are.
  - `Frontmatter::canonical` writes a fresh block in the style of `fixtures/logbook/`.
- **`model/`.** Typed records for `case`, `journal`, `decision`, `area`, `memory` and `project` (PROJECT.md).
  - Each record has its canonical key order, plus the checks from `case.schema.json`: id patterns, `agent:<name>`, ULIDs, area slug, enums.
  - `CaseStatus::folder()` follows ADR-0012 §9.
  - `Journal::entries` parses the `## HH:MM · actor · case?` headings.
  - Generic helpers: `model::{parse, load, render_new, update}`.
- **`config.rs`.** `config.toml` with defaults (all collectors on, `watchPaths`, `git.autocommit = true`, `[redaction]`, `[drift] alwaysRed` per ADR-0013).
  - Unknown keys survive a save.
  - XDG dirs come from the environment.
  - Logbook path precedence: `--logbook` > `SELDON_LOGBOOK` > config > `~/Seldon`. The `~/Documents` option comes from XDG `DOCUMENTS` via `user-dirs.dirs`.
- **`logbook/`.**
  - `Logbook::open` returns exit 3 when the logbook is not initialised and checks `schemaVersion`. File listings for cases, decisions, journals, areas and memory.
  - `layout::create` builds the SPEC-LOGBOOK §2 layout:
    - templates, `.seldon/logbook.toml`, `.gitignore`;
    - `.gitkeep` in empty directories;
    - optional `.obsidian/` (`app.json` exclusions, `daily-notes.json` → `journal/` with format `YYYY/YYYY-MM-DD`).
    - It never overwrites a file.
  - `git.rs` runs `git init` and the first commit `seldon: init logbook`. When no git identity is set, it falls back to `-c user.name=Seldon`.
  - `lock.rs`: `flock` on `$XDG_STATE_HOME/seldon/lock`, via `File::try_lock`.
- **`engine/templates/{en,de}/`.** 17 files per language, compiled into the binary:
  - AGENTS.md, PROJECT.md, STATUS.md (with the generated header), DECISIONS.md (with the `decisions.index` fence);
  - `areas/{hyprland,themes,packages,dev-env,plugins,shell}/README.md`;
  - `memory/lessons.md`;
  - `system/{hardware,packages,deviations,services,omarchy,plugins}.md` with empty fences named as in the fixtures (`packages.summary`, `packages.history`, `plugins.list`, `deviations.table`, …).
  - The German prose is real text, not a stub.
- **`seldon init`.** A dialoguer wizard: path → language → Obsidian → collectors → watched paths → harnesses → git.
  - Path options follow ADR-0010: `~/Seldon`, `<DOCUMENTS>/Seldon`, and `<first of ~/Work ~/Projects ~/dev ~/src ~/code>/seldon` when one exists, plus a custom path.
  - Every flag skips its step. `--non-interactive` takes flags, then the existing config, then the defaults. Without a terminal and without `--non-interactive` it is a user error.
  - It refuses an existing logbook or a non-empty directory (exit 1) and leaves both untouched.
  - It takes the lock (exit 4), writes `config.toml`, and prints the snapper fix as an optional next step (ADR-0011).
  - `--json` gives a summary.
- **`seldon doctor [--path DIR]`.** Checks engine, config, logbook, omarchy, snapper and git.
  - Each check is `ok`, `degraded` or `error`, with an optional `fix`.
  - The logbook check parses every record and flags a case that sits in the wrong folder for its status.
  - omarchy uses `omarchy-version`.
  - snapper runs `snapper --jsonout list` as the user. `No permissions.` → degraded with `sudo snapper -c root set-config ALLOW_USERS=$USER SYNC_ACL=yes`, printed only, never run.
  - It is read-only. Exit 0 when there are no errors, 3 when the logbook is not initialised, 1 otherwise.
- **WP-001 follow-ups in `main.rs`.**
  - `wants_json` no longer searches argv for the string. It reads the tokens with the built clap `Command`: it skips the values of options that take one, follows subcommands, and stops at `--`.
    - `seldon bogus -- --json` → text.
    - `seldon init --language fr --json` → JSON.
    - clap's own `ignore_errors` re-parse could not be used, because it stops at invalid values and unknown subcommands.
  - The message is clap's full detail without the `error:` prefix or the usage footer, e.g. `unrecognized subcommand 'no-such-command'`.
  - The `contract-version == plugin/manifest.json` cross-check is kept.
- **Tests: 71 in total.**
  - 27 unit tests.
  - `cli.rs`: 13.
  - `frontmatter.rs`: 9. The `round_trip::` tests run every case (8), journal (10), decision (4), area, memory and PROJECT.md file of `fixtures/logbook/` through typed record → fresh canonical block → byte-identical. Writing back an unchanged record is a no-op. Closing a case changes exactly two lines.
  - `init.rs`: 15 (`init::`).
  - `doctor.rs`: 7 (`doctor::`). One of them checks that doctor leaves `fixtures/logbook/` byte-identical.
  - Integration tests run in a temporary `HOME`. PATH holds only stub `omarchy-version`/`snapper` plus a link to the host's git.
- **`docs/TESTING.md`.**
  - New "Engine tests" section: how to run, the filters, the isolation, and the monorepo-layout dependency (`include_str!` of `plugin/manifest.json`, `fixtures/logbook/`).
  - The manual acceptance procedure with redirected XDG dirs.
- **Memory.** `memory/rust-notes.md` is new; `memory/pitfalls.md` has a new section appended.

## Not done

- **The first capture is skipped.** Wizard step "run first capture" prints a skip notice and JSON `capture.ran: false`, because no collectors exist yet. WP-004/005 should wire it in.
- **`--harness claude-code` only records the choice.** It is stored in `config.toml` (`harnesses`) and the next step says `seldon hook install claude-code`. Installing hooks is WP-009; the Omarchy-Agent kit is WP-024.
- **`.seldon/templates/` is created empty.** Case and decision body templates belong to `plan new` / `decide` (WP-006/later).
- **The wizard was smoke-tested, not unit-tested.** It ran through a pty with `script`, choosing defaults, `~/Work/seldon` and German. There is no automated test of the interactive path; dialoguer needs a real terminal.
- **Nothing ran on the test host** (no `just` there, see TESTING.md).

## Verified by

On the dev host, in `wt/WP-003`:

```
$ just check                                   → exit 0
  fmt-check ok · clippy -D warnings ok · test: 71 passed
  validate-fixtures: ok — 90 instances … backend builtin
  plugin-validate: ok · qmllint: ok (3 files) · check: ok

$ export XDG_CONFIG_HOME=<scratch>/config XDG_STATE_HOME=<scratch>/state
$ seldon init --non-interactive --path /tmp/seldon-wp003     → exit 0
  Logbook created at /tmp/seldon-wp003 (machine <host>-7182, language en, 30 files).
  Git: repository initialised, first commit "seldon: init logbook"
  Snapper: degraded — No permissions. …
$ seldon doctor --path /tmp/seldon-wp003 --json              → exit 0
  "ok": true; engine ok · config ok · logbook ok ·
  omarchy ok "Omarchy 4.0.4-1" (real omarchy-version) ·
  snapper degraded + fix (real snapper) · git ok "git version 2.55.0; …"
$ git -C /tmp/seldon-wp003 log --oneline    → one commit "seldon: init logbook"
```

- `~/.config/seldon` and `~/.local/state/seldon` do not exist on the host afterwards. `/tmp/seldon-wp003` was removed so the acceptance can be rerun.
- Exit codes are covered by tests:
  - 0: init, doctor;
  - 1: parse errors, bad `--language`/`--harness` values, existing logbook, non-empty directory, broken config, no terminal;
  - 2: engine errors, via `Error::Engine` (no dedicated test; it needs an I/O failure);
  - 3: `doctor` without a logbook;
  - 4: `init` while the lock is held.
- `--json` is supported by every command that exists: `--version`, `contract-version`, `init`, `doctor`, and the errors.

## Learned (in memory/)

- `rust-notes.md`:
  - clap `ignore_errors` is not a lenient parser;
  - serde_yaml (YAML 1.2) vs YAML 1.1 booleans;
  - `File::try_lock` → `rust-version` 1.89;
  - let-chains for clippy;
  - toml loses comments and order;
  - dialoguer features;
  - testing the wizard with `script`.
- `pitfalls.md`:
  - manual engine runs write `~/.config/seldon` unless the XDG dirs are redirected;
  - PATH filtering cannot hide `/usr/bin/snapper`;
  - `init` output carries the real host name.

## Decisions needed

1. **The acceptance command touches the red zone.** As written, `seldon init --non-interactive --path /tmp/seldon-wp003` writes `~/.config/seldon/config.toml` on the dev host (AGENTS.md §6). I ran it with `XDG_CONFIG_HOME`/`XDG_STATE_HOME` redirected. The reviewer should do the same (procedure in docs/TESTING.md).
   - Alternative: should `init` skip the config when given `--path` together with a new `--no-config` flag? I did not add one.
2. **`doctor --path` is not in SPEC-ENGINE §3.** The WP's acceptance uses `seldon doctor --path`, so I added it as an alias of `--logbook`. Please add it to the spec, together with the doctor JSON shape below.
3. **Doctor output shape and exit codes are my choice.**
   - JSON: `{"ok":bool,"logbook":path,"checks":[{"name","status":"ok|degraded|error","message","fix"?}]}`.
   - Exit codes: 1 when a check is `error`, 3 when the logbook is not initialised.
   - Should SPEC-ENGINE §3 fix this like the other "JSON shapes"?
4. **The `config.toml` schema should go into SPEC-ENGINE §2.** Keys I chose:
   - `logbook`, `language`, `watchPaths`, `harnesses`;
   - `[collectors] snapper|pacman|omarchy|plugins|theme|config`;
   - `[git] autocommit`, `[redaction] patterns, skipPaths`, `[drift] alwaysRed`.

   Two open points:
   - §2 says "agent names". I stored the harnesses chosen in the wizard under `harnesses`. Per-actor names had no use yet.
   - The toml crate sorts keys and drops comments on save, so `init` rewrites the file alphabetically.
5. **Obsidian exclusion "`work/*/`" (SPEC-LOGBOOK §6).** Taken literally, it hides `work/queued|active|completed/` and with them every case. I excluded only the case workpieces, `work/<case-id>/`, with the regex filter `/^work\/C-/`, plus `.seldon/`, `.claude/`, `.codex/`. Please confirm, and fix the spec wording.
6. **Non-interactive default language.** "Takes defaults" is implemented as the language from `LC_ALL`/`LC_MESSAGES`/`LANG` (`de*` → de, else en), unless the config or `--language` says otherwise. The interactive wizard pre-selects the same. The alternative is a fixed `en`.
7. **New dependencies and toolchain.** All crates are on the allowed list: serde, serde_yaml, toml, chrono (`clock`, `std`, `serde`; no default features), anyhow, thiserror, dialoguer (no default features). Two notes:
   - `serde_yaml` is marked deprecated upstream (`0.9.34+deprecated`). It works, but a successor (e.g. `serde_norway`) would need an AGENTS.md §7 change later.
   - `rust-version` is raised from 1.85 to 1.89 so that `std::fs::File::try_lock` can do the flock without another crate. Both hosts have 1.98.
8. **No fixture contradicts the spec.** All 8 cases, 10 journal days, 4 decisions, the areas, memory files, PROJECT.md and `logbook.toml` round-trip exactly, and doctor reports the fixture logbook as `ok`. Two small observations, with nothing to fix:
   - The fixture has no `.seldon/templates/`. Doctor does not require it.
   - The fixture's `STATUS.md` is German. The generated STATUS stub follows the logbook language.

## Touched outside WP scope

The brief listed `config.rs`, `logbook/`, `frontmatter.rs`, `model/`, `templates/` and `tests/`. Outside that list I touched:
- **`engine/`:**
  - `src/lib.rs`, `src/error.rs`, `src/sys.rs` and `src/commands/{mod,init,doctor}.rs` are new; `main.rs` was changed (the follow-ups and the new commands);
  - `Cargo.toml`/`Cargo.lock` (dependencies, `rust-version`).
- **Docs and memory:**
  - `docs/TESTING.md` (the brief asked for this);
  - `memory/rust-notes.md` (new) and `memory/pitfalls.md` (appended).

`schema/`, `fixtures/`, `scripts/` and `plugin/` were not touched.
