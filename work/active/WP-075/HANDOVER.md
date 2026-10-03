WP-075 HANDOVER

Branch `wp/075-review`, worktree `wt/WP-075`. No PR, no push.

Done:
- **F-141** (`engine/src/commands/import.rs`): `--apply` writes one ledger
  note of its own before the first file. It goes first in the same `emit`
  as the case notes: subject `omarchy-agent`, no case, `ts` = the apply,
  actor human, `meta.import: omarchy-agent`, and a detail with the counts.
  Now `already_imported` finds a note for every apply, with or without
  cases, so a case-less vault is refused the second time (marker deleted,
  or a failed apply) with the existing "is missing" message and undo.
- **F-143** (`engine/src/import/omarchy_agent.rs`, `commands/import.rs`):
  `walk` keeps every file's real `PathBuf` and a flag for whether all name
  parts are UTF-8 (`VaultFile`). If a file to import has a non-UTF-8 name,
  or sits in a folder with one, it becomes a report error ("file name is
  not UTF-8", path shown with U+FFFD). A listed file (inbox, templates, …)
  is listed whatever its name. Files are read through the real path. A
  file that cannot be read is now a report error (`cannot read: …`)
  instead of aborting the plan. The VAULT argument stays a path
  (`vault_path`: `~` = home, all other bytes kept), so a vault folder with
  a non-UTF-8 name works.
- **Orchestrator item** (`engine/src/import/mod.rs`): `split_frontmatter`
  skips a leading UTF-8 BOM (in the no-frontmatter case too) and accepts
  spaces or tabs after either `---` fence, like
  `frontmatter::Document::parse` (WP-066). `---x`, `--- x` and `----` are
  still not fences.
- **F-550** (`engine/src/dossier/mod.rs`, file reader and `set`):
  `Files::read` no longer stops at a file that is not UTF-8 or cannot be
  read. It records the file as `unread` and never writes it. The warning
  comes from the index derivation that `seldon dossier` runs first: same
  reader, same file set, same `system/<name>: cannot read: …; skipped`
  line. My first version also warned from `Files::read`, and every file
  showed up twice, so I took that out. `set` will not append a missing
  fence when its default file could not be read, or when an unread file
  may hold it: lossy bytes contain its begin marker, or the file could not
  be read at all. Such a fence is skipped with a warning, so a user's
  Latin-1 `plugins.md` is never replaced. A file whose name is not UTF-8
  is written back under its real name (`File.path`).
- **WP-065 fold-in** (dossier fence neutralising): `Files::set` runs
  `views::neutralise` on the content, as the views do, so a value with
  `<!-- seldon:end -->` (an imported kit heading, a host string) can
  neither end nor open a dossier fence. Rows kept verbatim from the old
  body pass through it too. They cannot hold a begin or end marker (that
  would be a damaged or shorter fence), so only a harmless
  `<!-- seldon:…` gets the zero-width space. SPEC-ENGINE §6 sentence
  updated.
- **F-135** (`engine/src/commands/watch.rs`): `DIRS` now includes `areas`
  (7 folders; module doc and comments updated). The `engine/Cargo.toml`
  comment now says which builds have the feature: the release workflow
  and PKGBUILD do; plain `cargo build` and `just build-release` do not.
- SPEC-ENGINE §3: import lines (apply note, non-UTF-8 names, unreadable
  files, BOM/padded fences, VAULT is a path) and the watch line
  (`areas/`). §6: the dossier neutralising sentence. CHANGELOG
  `[Unreleased]` › Engine: four entries. memory/pitfalls.md: WP-075
  entry.

Not done:
- `engine/systemd/README.md` line 16-17 still lists the watched folders
  without `areas/`. Not in this WP's files; a one-line follow-up.
- `docs/TESTING.md` rows for `tests/import.rs`, `tests/dossier.rs` and
  `tests/watch.rs` do not name the new tests yet (not in this WP's files).
- SPEC-ENGINE §3 `seldon dossier` lines do not yet say "an unreadable
  system file is skipped" (the WP names only the watch and import lines).
  The code now does what the spec's "a failed part is skipped with a
  warning" promises.
- A vault folder that cannot be *listed* still aborts the plan
  (`cannot list …`), as before. Out of scope.
- Not done: the finding's optional "extra check" (refuse an apply when a
  day or memory file already has the same source line). The ledger note
  covers both reproduced paths.

Verified by:
- New tests, each failing without its fix (mutants applied to the source,
  `cargo test`, source restored; full output in the session):
  | Item | Test(s) | Mutant | Result |
  |---|---|---|---|
  | F-141 | `tests/import.rs::a_vault_without_cases_is_never_imported_twice` | apply note removed (case notes only) | FAILED (note count 0). With that assertion skipped, the second `--apply` exits 0 and imports the journal again, which reproduces the finding |
  | F-143 | `a_file_name_that_is_not_utf8_is_a_report_error` | the UTF-8 name check removed | FAILED |
  | F-143 | same | a read error aborts the plan (old code) | FAILED |
  | F-143 | `a_vault_path_that_is_not_utf8_is_read` | vault through `to_string_lossy` (old code) | FAILED |
  | F-143 | — | read through `vault.join(rel)` instead of the real path | **survives: equivalent mutant.** A non-UTF-8 name is a report error before any read, so `rel` is the real name for every file that is read |
  | BOM/padded | `a_bom_and_padded_fences_import`, unit `import::tests::frontmatter_split_and_lenient_yaml` | old `split_frontmatter` | both FAILED |
  | F-550 | `tests/dossier.rs::a_file_that_is_not_utf8_is_skipped_and_the_fences_are_built`, `a_default_file_that_is_not_utf8_keeps_its_fences`; unit `a_file_that_is_not_utf8_is_skipped_and_never_written`, `an_unreadable_file_blocks_only_appending` | reader stops at the first bad file (old code) | all four FAILED |
  | F-550 | unit `a_file_that_is_not_utf8_…`, integration `a_default_file_…` | default-file check dropped | FAILED |
  | F-550 | unit `a_file_that_is_not_utf8_…`, `an_unreadable_file_…` | `may_hold` always false | FAILED |
  | F-550 | unit `a_name_that_is_not_utf8_is_written_back_to_itself` | write to `dir.join(name)` (old code) | FAILED |
  | neutralise | unit `fence_bodies_are_neutralised`; `tests/import.rs::a_marker_in_a_kit_heading_cannot_end_the_deviations_fence` | `neutralise` dropped in `set` | both FAILED |
  | F-135 | `tests/watch.rs::a_new_area_triggers_a_rebuild` (`--features watch`; also `areas/` renamed away and recreated) | `areas` removed from `DIRS` | FAILED ("no rebuild in 6s") |
- Updated existing assertions: the import-note count in
  `apply_writes_the_plan_once…` and `a_failed_apply_says_how_to_undo_it`
  is now 7 (the apply note plus 6 cases); the first also checks the apply
  note's subject, actor, ts and detail. The watch unit test
  `relevant_path` gets `areas` paths, and the "reads do not trigger" test
  reads `areas/` too.
- All tests ran as uid 1000, so the permission branches (chmod 000/555)
  really ran.
- `just check`: exit 0 (`check: ok`; fmt, clippy, tests incl. 60 test
  binaries ok, check-watch, packaging, install, schema, docs-check,
  plugin-validate, qmllint, plugin-test). After it, only a doc comment in
  a `dossier/mod.rs` unit test changed (fmt and that module's tests
  re-run).
- `just check-rss`: exit 0 (`rss_stays_under_10_mb_on_the_x10_fixture ... ok`).

Learned: memory/pitfalls.md "2026-10-04 · WP-075": the dossier already
warns through the index derivation; `common::read` panics on Latin-1
test files; an equivalent mutant behind an earlier guard; the WP-034 "no
watcher in the release build" note is out of date; import-note counts
include the apply note.

Decisions needed: none. One for the reviewer: the apply note is a new,
visible ledger event (`manual/note`, no case) on every `--apply`. The
finding recommended it; the plugin's changelog will show it.

Touched outside WP scope: none of another WP's files. Inside the WP but
beyond its literal list: `engine/src/dossier/mod.rs` `Files::set` (the
neutralising and the append guard; the brief adds the neutralising),
`engine/src/import/mod.rs` (orchestrator item), `docs/SPEC-ENGINE.md` §6
(one sentence, named by the brief), `memory/pitfalls.md` (append).
`engine/src/commands/dossier.rs` was changed and then reverted; it is not
in the diff.
