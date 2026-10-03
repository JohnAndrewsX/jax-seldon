```
WP-060 HANDOVER — snapper: read snapshot info files when listing is not permitted
Branch: wp/060-review (worktree wt/WP-060), 6 commits on d112d97, not pushed.
```

## Done

Scope as briefed: the read path, the text that says what the opt-in
grants, and the tests. The fix command string is **unchanged** in every
place (engine `SNAPPER_FIX`, plugin `SNAPPER_FIX_COMMAND`, docs).

1. **Read path** (`engine/src/collectors/snapper.rs`). When `snapper list`
   answers `No permissions.`, the collector reads
   `<snapshots>/<number>/info.xml` with a small hand-written parser (no XML
   crate): `type`, `num` (must equal the directory), `date` (UTC in the info
   file, converted to the local offset), `pre_num`, `description`,
   `cleanup`. `userdata` is not part of the events and is not read. The
   events and the cursor are identical to the list path, so switching
   between the two adds no events. Non-numeric entries and `0` are
   ignored. An info file that cannot be read or parsed is skipped and named
   in the collector message. Its snapshot is neither new nor deleted, and
   a known one stays in the cursor. If the directory cannot be read, or
   every numbered info file in it fails, the collector stays degraded with
   `NO_PERMISSIONS` and the unchanged fix, as before. An `ok` result from
   the info files has the message "snapper list is not permitted; N
   snapshots read from the info files in DIR[; skipped N/info.xml: why]".
2. **Injectable root**: `Sources.snapshots` (`SELDON_SNAPSHOTS_DIR`, default
   `/.snapshots`). Under `SELDON_TEST_GUARD` without the variable it is
   `<guard>/.snapshots`, so integration tests and guarded manual runs never
   read the host's snapshots.
3. **doctor / init** (`check_snapper`). Same fallback: readable info files
   give `ok` with the message above and no fix. The degraded message now
   ends with `SNAPPER_FIX_GRANTS` ("The fix adds your user to ALLOW_USERS
   of the root snapper config, which also lets your user create, change and
   delete root snapshots without a password."). `init` prints doctor's
   message, so it shows the sentence too. When listing works for the user,
   the `ok` message ends with `SNAPPER_LIST_GRANTS` ("This user may use the
   snapper config, which also lets it create, change and delete snapshots
   without a password.").
4. **Plugin banner** (`plugin/Model.js`). The detail is the engine's
   message, then a new line with `SNAPPER_FIX_GRANTS` ("The command below
   adds your user to ALLOW_USERS of the root snapper config, which also
   lets your user create, change and delete root snapshots without a
   password."). Command, actions and hint are unchanged.
5. **Docs**: user guide en/de 01 (wizard and doctor sample output plus the
   paragraph after it), 06 (snapper bullet), 10 ("Snapshots are not
   recorded"); `plugin/README.md` banner row; SPEC-ENGINE §4 snapper
   paragraph; SPEC-PLUGIN §5 snapper banner row; CHANGELOG `[Unreleased]`
   Engine and Plugin lines. The German source lines point at the new English
   commit.
6. **Fixtures**: `fixtures/logs/snapshots/<n>/info.xml`, which mirrors
   `fixtures/logs/snapper.json` (dates in UTC, userdata on 114/115).
   `scripts/validate-fixtures.py` step 3c checks that mirror: numbers,
   fields, UTC date, userdata.

## Not done

- **New opt-in command and its ADR** (superseding ADR-0011). The
  operator's decision is pending. The fix line, the `NO_PERMISSIONS`
  message, the wizard hint, `PKGBUILD` optdepends and `.SRCINFO`,
  `schema/external/snapper-list.schema.json` and
  `fixtures/index-variants/snapper-degraded.json` are untouched because
  they only change with the new command. For the same reason
  `engine/tests/index.rs` has no change.
- **Undo of an existing opt-in** (banner action, doctor fix). It belongs
  to the same ADR: without the new read-only opt-in, the undo only turns
  snapshots off. Doctor states the rights when listing works but offers no
  undo and stays `ok`.
- The user docs do not offer a read-only alternative yet. Only
  SPEC-ENGINE describes the info-file path. A user-facing sentence follows
  with the ADR.
- `docs/TESTING.md` (rows for `collectors.rs`/`doctor.rs`, "Manual runs":
  `SELDON_SNAPSHOTS_DIR` and the guard default) and `fixtures/README.md`
  (the `logs/snapshots/` tree) are not in this WP's file list, so they are
  not updated. Proposed as a docs follow-up.

## Verified by

- `just check` → `check: ok` (exit 0). shellcheck is not installed on this
  host, so `bash -n` only (as before).
- Engine tests, each failing without its part of the change (mutants run,
  then restored byte for byte):
  - `collectors::snapper_reads_the_info_files_when_listing_is_not_permitted`:
    with a list cursor from `snapper-before.json`, then the info files, the
    events equal those from `snapper.json` (7: +111…+115, −108, −109). The
    cursor is the same. A second capture gives 0 events, and so do list →
    info → list. A first run from the info files equals a first run from
    the list. Without the fallback: FAILED. With info dates read as local
    time: FAILED.
  - `collectors::snapper_info_files_skip_what_cannot_be_read`: `lost+found`
    and a file are ignored. 112 (no `<num>`) and 113 (mode 000) are
    skipped and named. 0 events (no deletes). Both stay in the cursor.
    Readable again: 0 events. All unreadable: degraded `NO_PERMISSIONS` +
    fix, cursor kept. Unreadable directory: degraded. Without keeping
    skipped numbers: FAILED.
  - `collectors::snapper_degrades_without_permissions`: unchanged
    expectations, now with a missing snapshot directory.
  - `snapper::tests::parses_info_files` (pre/post pair, missing optional
    fields, entities, `<x/>`, missing/mismatched `num`, unknown type, bad
    `pre_num`, not an info file) and `reads_only_numbered_directories`
    (`0`, `+4`, `5a`, `lost+found` ignored; a numbered directory without
    `info.xml` is skipped as "missing").
  - `doctor::green_after_init_with_snapper_degraded` and
    `human_output_lists_every_check` check the rights sentence. Without it
    both FAILED. `doctor::logbook_from_config_after_init` covers listing.
    `doctor::snapper_info_files_are_enough`: fixture tree via
    `SELDON_SNAPSHOTS_DIR` → `ok`, no fix. Guard default missing →
    degraded. `<guard>/.snapshots/7/info.xml` → `ok`, "1 snapshot".
  - `init::…` (line ~96) checks that the wizard output contains
    `SNAPPER_FIX_GRANTS`.
- Plugin: `model.test.js` (exact detail = message + `\n` + grants, and the
  fallback detail; without the Model change: FAILED). `service-states.sh`
  checks `.snapperDetail` for `snapper-degraded` and `snapper-still`.
  `panel-view.sh` checks that the rendered banner text ends with the grants
  line. Results: 205/693 passed, 0 failed.
- `validate-fixtures.py`: ok. With 112's date changed: 1 problem (step 3c).
- Nothing ran against the real `~/Seldon`, `~/.config/seldon`,
  `/.snapshots` or `/etc/snapper`. No sudo, setfacl or snapper writes.

## Learned (→ memory/pitfalls.md)

- snapper's `info.xml` stores `date` in UTC, while `snapper list` prints
  local time.
- `tests/support/mod.rs` `Bench` builds `Sources` with
  `..Sources::default()`, so host paths are the default. Any in-process
  test with a denied snapper stub must set `b.sources.snapshots`.

## Decisions needed

1. **ADR question (operator):** should the recommended opt-in change from
   `ALLOW_USERS` to a read-only grant on the snapshot directory, as an ADR
   superseding ADR-0011? The read path in this WP already works with
   read-only access. If yes, the follow-up changes the fix line (engine,
   plugin, docs, PKGBUILD/.SRCINFO, schema description, degraded fixture)
   and adds an undo for an existing `ALLOW_USERS` entry (remove only this
   user, as snapper(8) describes).
2. Follow-up for files outside this WP's list: `docs/TESTING.md` and
   `fixtures/README.md` text; the `Bench` default for `sources.snapshots`
   in `tests/support/mod.rs` (scratch path instead of the host default).

## Touched outside WP scope

- `engine/src/collectors/mod.rs`: one field `Sources.snapshots` plus its
  `from_env` line. This is the "same pattern as the existing snapper shim"
  the WP asks for, and the bench needs it to inject the root in-process.
  No other wave-1 WP touches this file. WP-073 touches it later.
- `CHANGELOG.md` `[Unreleased]` (allowed: neutral lines).
