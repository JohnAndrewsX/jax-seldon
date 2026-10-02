```
WP-038 HANDOVER
Done: files the engine writes itself under a watched path are recorded at install time (owned.json) and the next capture that runs the config collector explains their config event with an `explained` resolution (source seldon, actor system, "installed by seldon init --theme-hook" / "installed by seldon hook install claude-code"); SPEC-ENGINE §5 rule 7 plus §2/§3/§8/§9 sentences; 7 integration tests (tests/own_writes.rs) + 2 unit tests; work/PHASE-0-EXIT.md rewritten for the shipped wizard; docs/TESTING.md "Fresh machine smoke list"
Not done: the procedure was not run on a fresh machine (operator only, AGENTS.md §6); no literal actor `seldon` (contract change, see Decisions needed)
Verified by: just check (exit 0, `check: ok`: fmt, clippy -D warnings, all engine tests incl. --features watch, packaging, validate-fixtures ok, plugin-validate, qmllint 28 files, plugin-test); cargo test --test own_writes (7 passed); cargo test --lib own_write (2 passed); no fixture or schema change
Learned: memory/rust-notes.md + memory/pitfalls.md, sections "WP-038"
Decisions needed: one small one, does not block the merge (actor wording, below)
Touched outside WP scope: engine/src/commands/capture.rs (calls rule 7 after the append, `explainedOwn` in the output), engine/src/commands/setup.rs (the theme hook install lives here; `record_own_writes` helper), engine/tests/own_writes.rs (new)
```

Branch `wp/038-self-attribution`, worktree `wt/WP-038`, from `8afe278`
(main at `bda54ab` plus the start commit). No PR, no push. Commits:

- `752b99a engine: attribute the engine's own installs (WP-038)`
- `74a09be docs: Phase 0 exit procedure for the shipped wizard, fresh machine smoke list (WP-038)`
- `4f949bb memory: WP-038 notes and pitfalls`
- this handover

## The choice: owner record + resolution at the next capture

The brief offered (a) an owner marker honoured by the config collector or
attribution pass, or (b) a resolution written at install time. I used
(a) to produce (b)'s result.

- **(b) alone cannot work.** A resolution needs `refersTo`, the ULID of
  the config event. That event does not exist at install time: the
  collector creates it at the next capture, and `Ledger::append` assigns
  its id.
- **What I built.**
  1. At install time the engine records the file's `~`-path, the sha256
     of what it wrote, and the command in
     `$XDG_STATE_HOME/seldon/owned.json`, under the state lock.
  2. The next capture that runs the config collector appends the
     collector events as before.
  3. Then, under the same lock, it appends one `explained` resolution
     for each new `config-add|config-change` whose path and `hashTo`
     match a record.
  4. Then it forgets every record.
- **Why this keeps the ledger truthful.**
  - The `config-add` stays in the ledger exactly as the collector saw
    it (actor `system`, no case). The explanation is a separate,
    append-only line, the same mechanism as `drift explain` (ADR-0008,
    ADR-0021). The index folds it, so the event is not drift.
  - The hash must match. If someone edits the hook before the capture,
    what the ledger reports is no longer Seldon's content, so it stays
    open drift. A test covers this.
  - The record is used once. A later edit of the hook is drift again.
- **Rejected alternatives.**
  - Install the theme hook before the first capture. The file would
    land in the config baseline, so there would be no drift, but also
    no event. The brief asks for the event to stay in the ledger,
    explained.
  - Keep the marker in `manifest.json`. That file is rewritten by the
    collector during `collect` and is consumed before the ledger write.
    A separate file is consumed only after the explanation is written,
    and the collector stays unchanged.
- **Harness files written by `init`** (`<logbook>/.claude/…`) are written
  before the first capture, so they are part of its config baseline even
  when a watch path covers them. They get no record; SPEC §5 rule 7 says
  so. `hook install` records whatever it writes under a watched path, for
  example `--settings ~/.claude/settings.json` with `~/.claude` watched.

## SPEC wording (docs/SPEC-ENGINE.md)

§5 rule 7:

> 7. **The engine's own writes (WP-038).** A file the engine writes itself
>    under a watched path — the theme hook script that `init --theme-hook`
>    has `omarchy hook install` copy to
>    `~/.config/omarchy/hooks/theme-set.d/seldon-theme-set.sh`, a Claude
>    Code settings file `hook install` writes under a watched path — is
>    recorded right after the write, under the lock, in
>    `$XDG_STATE_HOME/seldon/owned.json`: its `~`-path, the sha256 of its
>    content, and the command (`by`). Only paths the config collector
>    hashes are recorded (under `watchPaths`, not in `skipPaths`). The next
>    capture that runs the config collector appends, after the collector
>    events and under the same lock, one `explained` resolution per new
>    `config-add|config-change` without a case whose subject and
>    `meta.hashTo` match a record: `source: seldon`, actor `system`, no
>    case, detail `installed by <by>` (e.g. `installed by seldon init
>    --theme-hook`). Then it forgets every record, matched or not: the
>    collector has seen each file — as an event, in its baseline, or with
>    content someone else wrote, which stays drift. The config event stays
>    in the ledger with its own actor; the index folds the resolution
>    (ADR-0021: no case, so none is folded) and the event is no drift. A
>    capture without the config collector keeps the records. Files the
>    wizard writes before its first capture (the harnesses inside the
>    logbook) are part of that capture's config baseline and need no record.

§8 (after the `hook install` sentence):

> When it writes a file under a watched path (e.g. `--settings
> ~/.claude/settings.json` with `~/.claude` watched), it records the file
> as the engine's own write (§5 rule 7), so the next capture explains its
> config event; `--json` adds `ownWrites` (the recorded `~`-paths,
> `{error}` when they could not be recorded, `null` when nothing was
> added).

§9 (the theme hook step; it now also stands where the code runs it,
after the dossier; the old text placed it before the capture commit):

> → theme hook on opt-in only: … ; the installed copy is recorded as the
> engine's own write (§5 rule 7), so the next capture's `config-add` for
> it is explained and opens no drift (WP-038)

Also: §2 has a new `owned.json` row. §3 adds `sinceIgnored` and
`explainedOwn` to `capture --json` and `themeHook.ownWrites` to `init
--json`. The human `capture` output gets one line: `note: N config
event(s) explained as written by seldon itself`.

## Tests (engine/tests/own_writes.rs, through common::Env + SELDON_TEST_GUARD)

The `omarchy` stub copies the file the way `omarchy-hook-install` does.

- `init_with_the_theme_hook_then_capture_leaves_no_drift`:
  - `init --theme-hook` → `themeHook.ownWrites = [hook]`, and `owned.json`
    holds the hash;
  - `capture` → written 1, explainedOwn 1, `owned.json` gone;
  - the ledger has the `config-add` (actor system, no case) and one
    resolution (seldon/system/explained/"installed by seldon init
    --theme-hook", no case);
  - `drift` is empty, and the index row shows `resolution: explained`
    with the detail; the index validates;
  - a second capture writes 0 events, explains nothing, and leaves the
    ledger unchanged;
  - a later edit of the hook is open drift (`config-change`).
- `the_human_capture_says_what_it_explained`.
- `a_hook_changed_before_the_capture_is_drift` (hash mismatch → no
  explanation; the record is forgotten anyway).
- `without_a_first_capture_the_hook_is_part_of_the_baseline` (`init
  --no-capture --theme-hook` → baseline, empty ledger, no drift).
- `a_capture_without_the_config_collector_keeps_the_record`
  (`--source pacman` keeps it; the next `--all` explains it).
- `hook_install_under_a_watched_path_leaves_no_drift`:
  - `~/.claude` watched, `--settings ~/.claude/settings.json` →
    `config-add` explained, no drift;
  - the second install adds nothing and records nothing (`ownWrites:
    null`), and the idempotent capture writes 0;
  - a default install into the logbook (not watched) → `ownWrites: []`.
- `hook_install_merging_into_a_watched_file_is_an_explained_change`
  (`config-change`).
- Unit tests in `collectors/config.rs`: matching by path and hash
  (removal and other paths excluded, JSON round trip); `is_watched`
  respects watch paths, `skipPaths` and path-component boundaries.

## Docs

- **`work/PHASE-0-EXIT.md`** is rewritten as the fresh-machine reference:
  - install the engine: AUR once registered, release asset, or a
    checkout build;
  - optionally the plugin;
  - one `init` with the flags and the wizard answers. `init` does the
    harness, git, first capture, baseline, dossier and theme hook
    itself; there is no separate `hook install`;
  - doctor and the snapper opt-in, `sudo snapper -c root set-config
    ALLOW_USERS=$USER SYNC_ACL=yes`, marked as the operator's choice;
  - the first capture after `init` shows the explained hook event;
  - the case with Claude Code, with two reversible examples: a theme
    round trip, and a Lua comment line in
    `~/.config/hypr/looknfeel.lua` (Omarchy 4 keeps the Hyprland config
    as `~/.config/hypr/*.lua`);
  - close, look, report, and a "start over" section.
- **`docs/TESTING.md`** gains "Fresh machine smoke list": seven steps,
  each with what must be true, taken from the transcript plus the new
  explained hook event.

## Decisions needed

- **Actor wording.** The WP says "attributed to actor `seldon`". The
  contract's actor pattern is `human|system|agent:<slug>`, so a literal
  `seldon` actor would need an ADR and a `contractVersion` bump. I kept
  to the contract: the explanation line has `source: seldon`, actor
  `system`, and the config event keeps its collector actor `system`. If
  the operator wants a visible `seldon` actor, that is a separate
  contract WP. Recommendation: no, because `source: seldon` already
  says who wrote the line.

## Notes

- **Guard block (reported, not worked around).** A read-only `grep`
  whose search pattern contained a package-manager install string was
  blocked by the repository guard as a red-zone package command. I did
  not repeat or rephrase that search. I took the install line for the
  procedure from ADR-0016, which I read with the file reader, and the
  release asset name from `.github/workflows/release.yml`. Logged in
  memory/pitfalls.md.
- **Version.** 0.1.0 does not contain this change. The procedure says
  that with 0.1.0 the hook file still shows up as one drift item, to be
  explained by hand as in the transcript.
- **Real host.** I did not touch the real host: no `~/Seldon`, no real
  `~/.config` or `~/.local/state`, and no `--theme-hook` outside the
  stubbed tests.
