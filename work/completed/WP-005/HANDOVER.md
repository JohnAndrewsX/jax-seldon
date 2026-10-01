WP-005 HANDOVER

Branch `wp/005-collectors-user`, worktree `wt/WP-005`. Not pushed, no PR.
Rebased on `main` at `beb7005`, which has the full WP-004 and WP-006 merged.
The `memory/*.md` conflicts were resolved by keeping both sections. Each
commit builds; at HEAD `just check` exits 0.

## Review round 1 fix-up (APPROVE, conditional)

Four commits on top of the rebased branch:

1. **Flaky test harness (blocking).** Fixed in `79d5bb6`.
   - Before: `Bench::run` re-acquired the flock on every run. A stub process forked by another test thread holds the inherited lock fd until it execs, which gave a sporadic `LockHeld`.
   - Now: `Bench` takes the lock once in `new`, keeps it as its first field (released first) and hands it to `Ledger::append`. The manual acquire in the theme hook test is gone. Engine code is unchanged.
   - Stress run: the `collectors_user` binary 150× with `--test-threads=16` → 0 failures; the full `cargo test` 10× → 0 failures. I did not reproduce the old failure rate myself; the fix follows the reviewer's diagnosis, which matches how WP-004's `support::Bench` already holds its lock.
2. **Overrides in `Sources`.** Done in `9e2374d`.
   - New fields in `collectors/mod.rs`, read once per process in `Sources::from_env`:
     - `omarchy` (`SELDON_OMARCHY`, default `omarchy`);
     - `plugins_dir` (`SELDON_OMARCHY_PLUGINS_DIR`; `None` = `~/.config/omarchy/plugins`);
     - `theme_file` (`SELDON_THEME_FILE`; `None` = Omarchy's `theme.name`).
   - `Plugins::dir(sources, home)` and `Theme::file(sources, home)` resolve the defaults. The env readers in the collector modules are removed.
   - The rebase surfaced a failure in WP-004's `tests/idempotency.rs::capture_writes_the_ledger_once`. Its `capture --all` now runs the real plugins, theme and config collectors: they reported degraded (no `omarchy`, no theme file), and they keep cursors, so they show up in `sinceIgnored`. Fixes:
     - its `Cli` now feeds them fixture inputs (`plugin-list-before.json`, a theme file, an empty plugins dir);
     - the three `sinceIgnored` expectations list all six collectors;
     - event counts are unchanged.
   - `tests/support/mod.rs` gets `..Sources::default()` in its `Sources` literal.
3. **ADR-0018.** Done in `7e385fd`. `plugin-update` is emitted only for `firstParty: false`, read from the list's `firstParty` field.
   - First-party version changes are tracked in the cursor only.
   - Enable and disable still fire for first-party plugins.
   - Test `first_party_version_changes_are_not_events`:
     - a first-party bump → 0 events, cursor updated;
     - a first-party and a third-party bump together → exactly one `plugin-update` for the third-party plugin;
     - enabling a first-party plugin → `plugin-enable`.
   - I read ADR-0018 from the main checkout, where it is still uncommitted; it is not in `main`'s history yet.
4. **Timing of `plugin-add` and `plugin-update`.** Done in `7e385fd`.
   - `ts` = the later mtime of the plugin directory and the manifest that gave the version (else the first manifest that exists), clamped to `[last check, now]` with the same `changed_at` as theme and config.
   - The plugins cursor gains `checked`. Cursors without it fall back to the capture time.
   - Removal, enable and disable keep the capture time.
   - Test `add_and_update_are_timed_by_the_plugin_directory` covers: clone time, an mtime in the past clamped to the last check, one in the future clamped to now, a pull that only touched the manifest, and a removal at capture time. The fixture-pair tests now set the clone/pull mtime, and the 09-24 `plugin-update` still matches the fixture line including `ts`.
- **Mutation checks.** Each was caught by a test:
  - drop the first-party filter;
  - time `plugin-add` at now;
  - time `plugin-update` at now;
  - use only the directory mtime.

## Done

- **`sys::sha256` / `sha256_hex`** — SHA-256 (FIPS 180-4) in `engine/src/sys.rs`.
  - Written by hand because no hashing crate is on the allowed list.
  - Tested against the FIPS vectors (`""`, `abc`, the 448-bit message, one million `a`) and padding edges.
  - Cross-checked against coreutils `sha256sum` on a real file.
- **`collectors/plugins.rs`**
  - Runs `omarchy plugin list --json`: id and enabled state.
  - Runs `omarchy plugin catalog`, but only for `manifestPath`.
  - **Version** (ADR-0014 §3), first that works:
    1. `version` in the manifest at the catalog's `manifestPath`;
    2. `version` in `~/.config/omarchy/plugins/<id>/manifest.json`;
    3. `git -C <dir> rev-parse --short HEAD`, only when the plugin directory has its own `.git`;
    4. the last version seen, so a failing catalog never looks like an update.
  - **Cursor:** `{hash, plugins: {id: {enabled, version}}, checked}`. The hash is the SHA-256 of the snapshot.
  - **First run:** baseline, no events.
  - **Diff events:**
    - `plugin-add`: `meta.version`, `meta.enabled`, `detail` = version;
    - `plugin-remove`: `meta.version`;
    - `plugin-enable|disable`: `meta.enabled` and `meta.version`;
    - `plugin-update`: `detail` "from → to", `meta.from`/`meta.to`, as in the fixture line of 09-24; third-party plugins only (ADR-0018).
    - `plugin-add` and `plugin-update` are timed by the plugin directory (see review item 4); the others get the capture time.
  - **Degraded** (`ok: false`, cursor kept):
    - The command exits non-zero. The message carries the first stderr line, e.g. `omarchy plugin list --json: omarchy-shell is not running (it needs the running Omarchy shell)`.
    - `omarchy` is missing, the run times out, or the output is not JSON.
    - An empty list comes back after a non-empty one.
- **`collectors/theme.rs`**
  - Reads `~/.local/state/omarchy/current/theme.name`. I checked first that `omarchy-theme-set` writes it and then runs `omarchy-hook theme-set <slug>`.
  - **Cursor:** `{theme, checked}`.
  - **First run:** baseline, no events.
  - **A change** gives one `theme-set` event: `detail` "from → to", `meta.from`/`meta.to`.
  - **`ts`** = the file's mtime, clamped to `[last check, now]`.
  - **No double recording:** if the ledger already has a `theme-set` to the current slug since the last check (written by the hook), no event.
  - **Degraded:** the file is missing, empty or unreadable.
- **`collectors/config.rs`**
  - **What it hashes:** a SHA-256 manifest of every regular file under `config.toml watchPaths` (`~` expanded; missing paths are skipped). Keys are `~`-paths.
  - **Never walked:**
    - `~/.config/omarchy/plugins/` (the same override as the plugins collector);
    - `.git` directories;
    - symlinked directories. Symlinked files are followed.
  - **Not files:** sockets, FIFOs and devices are ignored.
  - **`[redaction] skipPaths`** (`SkipPaths`): matching files and directories are never opened and never named in the manifest. Pattern syntax is in the doc comment; the orchestrator moves it into SPEC-ENGINE §7:
    - a pattern with `/` matches the full path (`~/` = home), as a file or as a directory with everything below it;
    - a pattern without `/` matches a file or directory name;
    - `*` and `?` stay within one path component; `**` crosses them.
  - **Skipped, but listed:** binary files (a NUL in the first 8000 bytes) and files over 1 MiB go into `skipped`, without a hash. So a file that grows past the limit is not reported as removed.
  - **Events:**
    - `config-add|config-change|config-remove` with `meta.hashFrom`/`meta.hashTo`;
    - `detail` `sha256 3b385861 → 09688c64` (`—` for a missing side), as in the fixtures;
    - zone from `zone_for`: `~/.config/systemd/…` → red;
    - `ts` = mtime clamped to `[last check, now]`; a removal gets the capture time.
  - **`$XDG_STATE_HOME/seldon/manifest.json`** = `{hash, files: {path: sha256}, skipped: [...], previous?}`.
    - `previous` is the generation the cursor names, kept until the ledger has caught up. If the ledger write fails after the collector has saved the new manifest, the next run still diffs against the right base and nothing is lost (tested).
    - A missing or corrupt manifest while a cursor exists gives a new baseline, with a message.
  - **First run:** baseline, no events.
- **`engine/hooks/theme-set.sh`** (mode 755): `seldon event theme theme-set --subject "$1"`.
  - Silent and always exit 0.
  - Does nothing without `seldon` on PATH or without a slug.
  - The slug is one argument and is never evaluated.
  - **`engine/hooks/README.md`** documents the install, done by the wizard on opt-in only (`omarchy hook install theme-set <file>` → `~/.config/omarchy/hooks/theme-set.d/`; WP-024 makes that call), and the interplay with the collector.
- **`engine/tests/collectors_user.rs`** — 23 tests after the review fix-up: in-process `Bench`, CLI, and the hook.
  - **Fixture pairs:**
    - plugins: `plugin-list-before` → `-after` gives exactly the fixture's `plugin-add io.github.example.tyme` line, ignoring id and ts;
    - plugins: weather-plus 1.2.0 → 1.3.0 gives the fixture's 09-24 `plugin-update` line, ts included;
    - theme: osaka-jade → kanagawa gives the fixture's 09-20 line, ts from mtime. The actor differs: the fixture line came from the hook;
    - config: a before/after tree with change, removal, and a red systemd add.
  - **Idempotency:** a second run gives 0 events, for every collector.
  - **skipPaths:** the secret file, a `*.key` file and a private directory never appear in `manifest.json`.
  - **Size limit:** exactly 1 MiB is hashed; 1 MiB + 1 is skipped; growing past the limit is not a removal.
  - **Binary files**, exclusions and symlinks.
  - **Failure handling:**
    - failed-write recovery and the lost manifest;
    - shell not running (degraded, cursor kept, the add found once the shell is back);
    - missing `omarchy`, bad output, an empty list;
    - a failing catalog keeps versions;
    - the git HEAD fallback, and an enclosing repo is ignored;
    - hook dedupe, mtime clamping, a missing or empty theme file.
  - **Schema:** every written event validates against `schema/event.schema.json` (jsonschema) and through `Event::validate` (via `Ledger::append`).
  - **The hook script:** exact argv, silent, exit 0, no evaluation of the slug.
  - **`capture` module:** `seldon capture --source plugins,theme,config --json` on a tmp logbook via `common::Env` (temp HOME, PATH = stubs). The three events come once, then nothing. Plugins degrade without the shell.

## Not done

- **ADR-0014 §1 attribution for config, theme and plugins events.** The collectors always write `actor: system`. The fixture's 10-01 `bindings.conf` change carries `agent:claude-code` + case. The orchestrator assigned the shared attribution pass to WP-009.
- **The wizard step that installs the hook.** That is WP-024; the script and the docs ship here.
- **`docs/` amendments.** The orchestrator owns `docs/`: the manifest shape is amended there, and the `SkipPaths` rules go to SPEC-ENGINE §7.
- **Golden files.** None. The tests compare against the fixture ledger lines directly, which is the stronger check.

## Verified by

- `just check` → `check: ok`: fmt, clippy `-D warnings`, all tests, schema-validate, plugin-validate, qmllint, plugin-test.
  - After the fix-up, one `just check` run failed in `plugin-test` (the headless QML harness, `tests/plugin/`). This branch touches nothing under `plugin/` or `tests/plugin/`.
  - The next 5 `just plugin-test` runs and the next `just check` passed.
  - I did not keep the output of the failing run. It looks like a flake in the plugin harness, worth a look by its owner.
- `cargo test --locked` (after the fix-up, on top of main): lib 58, `collectors_user` 23, and every other suite pass, including WP-004's `collectors`/`idempotency`/`redaction` and WP-006's `commands`.
- **Mutation checks.** Six deliberate bugs, each caught by a test:
  - drop skipPaths;
  - drop the hook dedupe;
  - report skipped files as removed;
  - forget remembered versions;
  - drop `previous`;
  - no ts clamp.
- **Manual acceptance.** Temp HOME and XDG dirs, `SELDON_OMARCHY` stub over the fixtures, `SELDON_THEME_FILE`, `SELDON_OMARCHY_PLUGINS_DIR`.
  - Run 1: `written: 0`, all three `ok: true`.
  - Run 2: `written: 3`:
    ```
    {"source":"plugins","kind":"plugin-add","subject":"io.github.example.tyme","detail":"1.1.0","actor":"system","zone":"yellow","meta":{"version":"1.1.0","enabled":false}}
    {"source":"theme","kind":"theme-set","subject":"tokyo-night","detail":"kanagawa → tokyo-night","actor":"system","zone":"yellow","meta":{"from":"kanagawa","to":"tokyo-night"}}
    {"source":"config","kind":"config-change","subject":"~/.config/hypr/bindings.conf","detail":"sha256 8ecd6365 → bb123910",…}
    ```
  - Run 3: `written: 0`.
  - `sha256sum` of the file = the `hashTo` in the manifest.
- **Real host smoke run.** Inputs read-only; state, config and logbook in a scratch dir, nothing written under `~/.config` or `~/.local/state`.
  - `capture --source plugins,theme,config` → all `ok`, 64 ms.
  - 38 plugins, all with a version: first-party 1.0.0 from the catalog's `manifestPath`; the dev install `jax.seldon` 0.1.0.
  - Theme read; 25 config files hashed; second run `written: 0`.

## Learned

Appended to `memory/rust-notes.md` and `memory/pitfalls.md`:
- the `collect_from` test seam;
- `as_chunks` for clippy;
- `impl Fn + use<>`;
- Command's PATH lookup;
- a collector-owned state file runs ahead of the ledger;
- the git-HEAD-of-enclosing-repo trap;
- first-party plugins all carry version 1.0.0;
- mtime clamping;
- hook/collector dedupe;
- builtin-only stubs under `common::Env`.

## Decisions needed

None open. Settled by the orchestrator in review round 1:
- 2: keep the collector-owned manifest (spec amended);
- 3: keep `sys::sha256`;
- 4: the `SkipPaths` rules go into SPEC-ENGINE §7;
- 5: one shared attribution pass before append, in WP-009;
- 6: ADR-0018, implemented above;
- 7: the hook's actor default goes to WP-009;
- 8: all three choices confirmed;
- 1: implemented above.

## Touched outside WP scope

- `engine/src/sys.rs`: SHA-256 added, as the brief allowed.
- `memory/rust-notes.md`, `memory/pitfalls.md`: appended.
- `engine/src/collectors/mod.rs`: three `Sources` fields (review item 2).
- `engine/tests/idempotency.rs` (WP-004's): fixture inputs for the three new collectors and the six-collector `sinceIgnored`, needed after the rebase (see review item 2).
- `engine/tests/support/mod.rs` (WP-004's): `..Sources::default()`.
- Not touched: `pacman/snapper/omarchy.rs`, `ledger.rs`, `capture.rs`, `commands/*`, `tests/common`, `docs/`, `decisions/`, `fixtures/`, `schema/`.
