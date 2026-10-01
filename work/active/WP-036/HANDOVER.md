```
WP-036 HANDOVER
Done: packages.explicit origin class omarchy-base|user (Omarchy's two package lists, read-only file read, SELDON_OMARCHY_PACKAGES override, fixture copies); rebuild §2 "Before the logbook" lists only `user` packages plus one base count line; empty packages.explicit fence in the init templates (en/de) and fixtures/logbook/system/packages.md; deviations.table fills an empty case cell from a later cased config event; tests, goldens, SPEC-ENGINE §3/§4, SPEC-LOGBOOK §3, TESTING.md
Not done: nothing in scope; see "Decisions needed"
Verified by: just check (exit 0); cargo test --test dossier (11), --test rebuild (7), --test init (29), lib unit tests (111); scripts/validate-fixtures.py ok (index.sample.json unchanged); each intermediate commit tested in an exported tree; real-host read-only run in a scratch home (counts below)
Learned: memory/rust-notes.md + memory/pitfalls.md, section "WP-036"
Decisions needed: three small ones, none blocks the merge (below)
Touched outside WP scope: engine/src/commands/dossier.rs (classification, warning, `omarchyBase` count), engine/tests/common/mod.rs (Env sets SELDON_OMARCHY_PACKAGES; `omarchy_packages()`), fixtures/README.md (one row for logs/omarchy-packages/)
```

Branch `wp/036-dossier-followups`, worktree `wt/WP-036`. Main has not moved
since the branch point (`1614f2e`). No PR, no push. Five commits:

- `6be65ba engine: split Omarchy base packages in the dossier (WP-036)`
- `0f6299f engine: packages.explicit fence in the init templates and the fixture (WP-036)`
- `71d9447 engine: fill an empty deviation case from a later cased event (WP-036)`
- `2e8ffdf docs: SPEC-ENGINE §3/§4, SPEC-LOGBOOK §3 and TESTING …`
- `e9e16b0 memory: WP-036 notes …`

I tested each of the three engine commits on its own (`git checkout-index`
into the scratchpad, separate `CARGO_TARGET_DIR`; lib, dossier, rebuild
and init tests all green).

## What was done

### Origin class in `packages.explicit`

- **Format.** A line is now `- <name> · repo|aur · omarchy-base|user ·
  since <date> [[C-…]]`, or `… · pre-logbook`. The class goes third so the
  `[[case]]` stays at the end of the line.
- **Reading old lines.** `parse_explicit` still reads a WP-035 line
  without a class, and treats it as `user`. A new unit test covers this.
- **Host format (checked).** `/usr/share/omarchy/install/omarchy-base.packages`
  and `omarchy-other.packages` have one package per line, `#` comment
  lines and blank lines. `omarchy-other` also has comment section
  headers.
- **`query::omarchy_packages(dir)`** reads both lists:
  - It drops everything after a `#`, skips blank lines and skips lines
    with whitespace inside.
  - It returns the names and the list files it could not read.
  - It only reads files; no program runs.
- **Where the lists come from:** `SELDON_OMARCHY_PACKAGES` (a directory),
  else `$OMARCHY_PATH/install`, else `/usr/share/omarchy/install`.
- **Classification happens before redaction.** `Packages::classify` runs
  on the real names; then `redacted` maps the `omarchy` set together with
  the names.
- **Missing lists.** Every package of a list that cannot be read counts
  as `user`. The run gives exactly one warning:
  `packages: Omarchy's package list(s) <paths> not readable; their packages count as \`user\``.
- **Counts.**
  - `--json` `counts` gains `omarchyBase`: explicit packages on Omarchy's
    lists.
  - The human line reads `Packages: N explicit (P from before the
    logbook, B on Omarchy's lists), T total, A AUR`.
- **Fixture copies.** `fixtures/logs/omarchy-packages/` holds verbatim
  copies of the host's two files. `common::Env::command` sets
  `SELDON_OMARCHY_PACKAGES` to that directory for every test. That
  matters because `env_clear()` unsets `OMARCHY_PATH`, so without it
  `init`'s dossier run would otherwise read the host's lists.

### rebuild §2 "Before the logbook"

- **What `Before` holds:**
  - the `user` names by origin (repo, aur);
  - `omarchy`: the count of `pre-logbook` packages of class
    `omarchy-base`;
  - `version`: from `omarchy.summary`, else section 1's base. The lists
    belong to the Omarchy version the same dossier run saw.
- **New text (en):**
  - Intro: "Your own explicit packages from before the logbook (dossier
    `packages.explicit`, class `user`): R from the repositories, A from
    the AUR. The commands skip what is already installed."
  - After the `sh` block: "N more come with Omarchy <version> (class
    `omarchy-base`)."
  - The German text is parallel. The base line appears only when N > 0.
- **An empty `packages.explicit` fence counts as no fence.** The old
  sentence ("Packages from before the logbook are not listed …") stays
  until the first dossier run. Without this, the new empty fence in the
  fixture would have changed every rebuild test that does not run the
  dossier.
- **Golden diff (`tests/golden/REBUILD.md`):** the block shrinks to
  `omarchy pkg add firefox linux neovim omarchy` and `omarchy pkg aur add
  brave-bin`, followed by "6 weitere bringt Omarchy 4.0.7-1 mit (Klasse
  `omarchy-base`)."
- **`before_trace` in tests/rebuild.rs** now checks three things:
  - every `pre-logbook` + `user` line is listed under the command of its
    origin, and nothing else is (5 packages);
  - exactly one line names `omarchy-base`;
  - that line carries the count (6) and the version.

  The WP-032 `packages_trace` is unchanged (4).

### Templates and fixture

- **Templates.** `engine/templates/{en,de}/system/packages.md` end with
  `## Explicit packages` and an empty `packages.explicit` fence. Template
  headings are English in both languages, as `## History` already is.
- **Fixture.** `fixtures/logbook/system/packages.md` ends with `##
  Explizite Pakete` and the empty fence. That is the same heading and the
  same place the dossier used to append it, so the dossier golden only
  changed by the classes.
- **Tests.**
  - The init-skeleton golden gains two lines.
  - `templates_have_english_keys_and_headings_in_every_language` also
    asserts the fence at the end of both languages' `packages.md`.
- **Validator.** `scripts/validate-fixtures.py` is green: 71 ledger
  events traced to `index.sample.json`. `fixtures/index.sample.json` is
  unchanged (the index does not read `packages.explicit`).

### `deviations.table` case fill

- **When a row is filled.** A row whose case cell is blank, `—` or `-`
  gets `[[<case>]]` when:
  - its path has a cased config event in `Facts::cased_config` (the
    path's latest config event, with a case, not dismissed, not "added
    and removed again"), and
  - that event's date is not older than the row's date, when the row's
    date cell is a `YYYY-MM-DD`. This is how "later" is implemented.
- **Nothing else changes.**
  - Only the bytes of the case cell change, and they are replaced by
    ` [[C-…]] `.
  - Path, reason, date, the trailing whitespace and the line ending
    stay.
  - The body is iterated with `split_inclusive`, so CRLF and a missing
    final newline stay too.
- **How the cells are found.** The path is the first cell, the case the
  last, the date the one before the case, so a reason containing `|` is
  harmless.
- **When nothing is filled.** No row is filled when the header is not
  `path` first and `case` last.
- A row that already has a case is never touched.

## How it was verified

- `just check` → exit 0 ("check: ok"): fmt, clippy `-D warnings`, all
  engine tests, schema-validate, plugin-validate, qmllint, plugin tests.
- `cargo test --test dossier` (11 tests; all go through `common::Env`
  with the query shims):
  - **Golden test.** Both classes appear: `git · repo · omarchy-base ·
    pre-logbook` and `firefox · repo · user · pre-logbook`. `counts` is
    `{explicit 15, preLogbook 11, omarchyBase 7, total 23, aur 3, units
    9, plugins 40}`. The text outside the fences is byte-identical for
    every file, now with no "appended" special case. A second run a day
    later gives `files: []` and "Nothing changed (8 fence(s) checked)".
  - **Emptied fences.** Seven `omarchy-base`: base, base-devel, btop,
    git, hyprland, linux-firmware, yay.
  - **New: `without_omarchys_lists_every_package_is_the_users`.**
    - Setup: `SELDON_OMARCHY_PACKAGES` points at a missing directory.
    - Exactly one warning, naming both files; `omarchyBase` is 0; all 15
      lines are `user`.
    - Only the 3 package queries ran: the lists are file reads.
    - A second run gives `files: []`.
  - **New: `a_later_cased_event_fills_only_an_empty_case_cell`.**
    - Setup: cased `config-change` events, through the CLI, on
      `~/.bashrc` (the row's case is `—`) and on `monitors.conf` (the row
      already has `[[C-2026-002]]`).
    - The new `deviations.md` equals the old one with exactly the bashrc
      cell replaced; monitors.conf keeps its case.
    - A second run gives `files: []`.
- `cargo test --test rebuild` (7) and `--test init` (29): green.
- Lib unit tests (111), new ones included:
  - `omarchy_lists_skip_comments_and_report_missing_files`;
  - `deviations_fill_only_an_empty_case_cell` (reason with `|`, row date
    newer than the event, trailing spaces, no final newline, idempotent,
    other column order);
  - `explicit_lines_round_trip` (with and without a class).
- `python3 scripts/validate-fixtures.py` → ok.
- **Real-host read-only run.**
  - Setup:
    - a scratch copy of the fixture logbook;
    - `HOME` and all three `XDG_*` variables in the scratchpad;
    - `SELDON_TEST_GUARD` set; `--no-commit`;
    - the real PATH and `OMARCHY_PATH=/usr/share/omarchy`, so the real
      lists and queries were used.
  - Counts:
    - **169 explicit**: 167 from before the logbook. **157
      `omarchy-base`** (156 of them pre-logbook), **12 `user`** (11
      pre-logbook).
    - 966 total, 0 AUR, 40 units, 38 plugins, no warnings.
  - The second run printed "Nothing changed (8 fence(s) checked)".
  - `seldon rebuild`: the "Before the logbook" block shrank from 29 lines
    (WP-035) to 3 lines, 11 packages, at most 71 columns. It is followed
    by "156 weitere bringt Omarchy 4.0.4-1 mit (Klasse `omarchy-base`)."
  - `find ~/.config/seldon ~/.local/state/seldon -newer <marker>`: empty.

## SPEC wording (in this branch)

- **SPEC-ENGINE §3, `seldon dossier`.**
  - The `packages.explicit` clause now reads:

    > sorted, `- <name> · repo|aur · omarchy-base|user · since <date>
    > [[C-…]]` from the ledger's latest install, else `pre-logbook`;
    > WP-036: class `omarchy-base` when Omarchy's omarchy-base.packages
    > or omarchy-other.packages names it, read as files from
    > $SELDON_OMARCHY_PACKAGES, else $OMARCHY_PATH/install, else
    > /usr/share/omarchy/install; `#` comments skipped; a list not
    > readable → its packages `user`, one warning

  - The `deviations.table` clause gains:

    > WP-036: a row whose case cell is empty, blank, — or -, gets the
    > case of its path's latest cased config event when that event is
    > not older than the row's date; only that cell changes

  - The `--json` `counts` gain `omarchyBase`.
- **SPEC-ENGINE §3, `seldon rebuild`.** "Before the logbook" is:

  > the `pre-logbook` lines of class `user` of `packages.explicit` as one
  > `omarchy pkg add` and one `omarchy pkg aur add` block, WP-035, then
  > one line "N more come with Omarchy <version>" counting the
  > `pre-logbook` lines of class `omarchy-base`, WP-036; an empty fence
  > counts as none

- **SPEC-ENGINE §4.** The dossier sentence adds: "and reads Omarchy's
  package lists (`omarchy-base.packages`, `omarchy-other.packages`) as
  plain files".
- **SPEC-LOGBOOK §3.**
  - The `packages.explicit` row of the table:

    > `- <name> · repo|aur · omarchy-base|user · since <date> [[C-…]]`,
    > or `· pre-logbook` when the ledger has no install of it; sorted.
    > `omarchy-base`: Omarchy's package lists name it; `user`: the
    > user's own addition

  - The paragraph after the table. "…and never changes an existing row"
    becomes:

    > A new logbook's `packages.md` carries every `packages.*` fence,
    > empty until the first `seldon dossier`. … The engine adds rows for
    > config events whose case is known; in an existing row it changes
    > only an empty case cell (blank, `—` or `-`), which gets the case of
    > a later cased config event on that path; the user fills the
    > reason. `seldon rebuild` lists the `pre-logbook` packages of class
    > `user` of `packages.explicit` under "Before the logbook" and counts
    > the `omarchy-base` ones in one line.

- **docs/TESTING.md.**
  - The `tests/dossier.rs` and `tests/rebuild.rs` rows are updated.
  - The rebuild manual block notes the empty fence.
  - The dossier manual block gains `omarchyBase` and
    `SELDON_OMARCHY_PACKAGES`, plus the WP-036 dev-host counts.
- **Contract.** No schema or contract change: the plugin and the index do
  not read `packages.explicit`.

## Decisions needed (none blocks the merge)

1. **What "later" means for the case fill.** I implemented it as "the
   cased event's date is not older than the row's date". The row's date
   is the day the deviation was recorded, and an event on the same day
   counts. When the date cell is not a date, the fill happens anyway.
   Stricter alternative: only events strictly after the row's date.
   Accept?
2. **What `omarchy-base` means.** It means "named by Omarchy's package
   lists", including `omarchy-other.packages`. That list also holds
   hardware-specific packages (nvidia, T2, Surface). On another machine
   such a package may really be the user's own choice, but the WP said
   "base or other", so I followed it. `linux` and `omarchy` are in
   neither list (the lists have `linux-omarchy`), so on the fixture they
   stay `user`. The "commands skip what is installed" sentence covers
   that. Accept, or restrict to `omarchy-base.packages`?
3. **Rebuild prose changed.** The intro now says "Your own explicit
   packages … (class `user`)". The old "A fresh Omarchy install already
   has many of them" became "The commands skip what is already
   installed", since Omarchy's own packages are now counted separately.
   The base line names the version from `omarchy.summary`. On the
   fixture that is 4.0.7-1; on the dev host it is 4.0.4-1, while
   `$OMARCHY_PATH/version` says `4.0.0.alpha`, which is not used. Wording
   OK?

## Guard note

No guard block in this WP. I typed no package-manager or service-manager
command. The real-host run invoked only the engine, which runs only its
read-only queries. Edits to files that contain the package manager's name
(docs, memory) were made with the Edit tool, not in shell text.
