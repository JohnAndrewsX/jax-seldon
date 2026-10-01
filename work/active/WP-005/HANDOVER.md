WP-005 HANDOVER

Branch `wp/005-collectors-user`, worktree `wt/WP-005`. Not pushed, no PR.
The commits sit on `aae3ef1` (WP-004 foundation on main). Each commit builds;
at HEAD `just check` exits 0. `git merge-tree` against `wp/004-collectors-core`
and `wp/006-commands` reports no code conflicts. Expect a trivial conflict
at the end of `memory/*.md` with WP-004, because both branches append.

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
  - **Cursor:** `{hash, plugins: {id: {enabled, version}}}`. The hash is the SHA-256 of the snapshot.
  - **First run:** baseline, no events.
  - **Diff events:**
    - `plugin-add`: `meta.version`, `meta.enabled`, `detail` = version;
    - `plugin-remove`: `meta.version`;
    - `plugin-enable|disable`: `meta.enabled` and `meta.version`;
    - `plugin-update`: `detail` "from → to", `meta.from`/`meta.to`, as in the fixture line of 09-24.
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
  - **`[redaction] skipPaths`** (`SkipPaths`): matching files and directories are never opened and never named in the manifest. Pattern syntax is in the doc comment, and is my proposal (see Decisions needed):
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
- **`engine/tests/collectors_user.rs`** — 21 tests: in-process `Bench`, CLI, and the hook.
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

- **ADR-0014 §1 attribution for config, theme and plugins events.** The collectors always write `actor: system`. The fixture's 10-01 `bindings.conf` change carries `agent:claude-code` + case. ADR-0014 assigns rule 1 to WP-004 for pacman only (see Decisions needed 5).
- **The wizard step that installs the hook.** That is WP-024; the script and the docs ship here.
- **`docs/` amendments.** The orchestrator owns `docs/`. They are listed under Decisions needed.
- **`Sources` fields.** Not added; I did not edit `collectors/mod.rs` (see Decisions needed 1).
- **Golden files.** None. The tests compare against the fixture ledger lines directly, which is the stronger check.

## Verified by

- `just check` → `check: ok`: fmt, clippy `-D warnings`, all tests, schema-validate, plugin-validate, qmllint, plugin-test.
- `cargo test --locked`: lib 39 passed, `collectors_user` 21 passed. All other suites pass.
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

1. **Env overrides outside `Sources`.** The brief said not to touch `collectors/mod.rs`. `SELDON_OMARCHY`, `SELDON_OMARCHY_PLUGINS_DIR` and `SELDON_THEME_FILE` are read in `Plugins::program/dir` and `Theme::file`.
   - Recommendation: after WP-004 merges, move them into `Sources` (3 fields, read once per process).
   - The tests do not depend on where they live.
2. **The config collector writes `manifest.json` during `collect`.** `mod.rs` says collectors never write. SPEC-ENGINE §2 puts the manifest in its own file, and the two-generation scheme makes that safe.
   - Alternative: a trait change, where `Outcome` carries state files that `capture` writes after the append.
   - Recommendation: keep it as is, and amend SPEC-ENGINE §2 ("path → sha256") to the actual shape `{hash, files, skipped, previous?}`.
3. **SHA-256 implemented in `sys`** rather than a crate.
   - The alternative would be `sha2`, which needs a justification and reviewer approval.
   - Recommendation: keep `sys` (≈80 lines, vector-tested).
4. **`skipPaths` pattern syntax.** The spec does not define it; mine is above and in `SkipPaths` docs.
   - Recommendation: write it into SPEC-ENGINE §4/§7.
   - WP-006/WP-009 should reuse `collectors::config::SkipPaths` for ADR-0014 §4, which redacts hook paths that match `skipPaths`.
5. **Who implements ADR-0014 §1 attribution for config, theme and plugins?** The fixture's 10-01 `config-change` is attributed to `agent:claude-code` / C-2026-004 via the `sed -i` hook command.
   - Recommendation: one shared attribution pass in `capture` or in reconciliation (WP-008), using the ADR-0017 window rules, not per collector.
   - The collectors already use mtime timestamps, so the 10-minute window works.
6. **First-party `plugin-update`s.** Every first-party manifest says `1.0.0`. An Omarchy update that bumps them would write ~37 yellow `plugin-update` drift items next to the red omarchy `update`.
   - Option A (recommended): skip `plugin-update` for `firstParty` plugins, since the omarchy `update` event covers them.
   - Option B: keep them, and let reconciliation fold them into the update.
   - Current code: Option B (it records them).
7. **The hook's event and `seldon event` (WP-006).**
   - The hook passes only `--subject`. The fixture's hook lines carry `actor: human` and `meta.from/to`.
   - WP-006 should decide the default actor for `seldon event theme theme-set` and whether it fills `meta.from` from the latest `theme-set` in the ledger.
   - The collector's dedupe only needs `kind` + `subject`.
8. **Small choices to confirm or veto:**
   - `plugin-enable|disable` also carry `meta.version` and `detail` = version. ADR-0012 §15 only requires `meta.enabled`.
   - `.git` directories are excluded from the config manifest, and symlinked directories are not followed. Both go beyond the spec text.
   - Unreadable files are listed as `skipped`.

## Touched outside WP scope

- `engine/src/sys.rs`: SHA-256 added, as the brief allowed.
- `memory/rust-notes.md`, `memory/pitfalls.md`: appended.
- Not touched: `collectors/mod.rs`, `pacman/snapper/omarchy.rs`, `ledger.rs`, `capture.rs`, `commands/*`, `tests/common`, `docs/`, `fixtures/`, `schema/`.
