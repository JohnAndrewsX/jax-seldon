# WP-141 — Plan: pacnew and pacsave in the record

Branch `wp/141-pacnew` from `next` (2ad42a4); merge into `next`.

## Inputs read

AGENTS.md §6–§8; ADR-0013 §1–§2 (grouping by `txId`), ADR-0014 §1–§2,
ADR-0028 (§1 tests, §2 table, §4c, §7), ADR-0037; SPEC-ENGINE §4 (pacman),
§5 (class rows, grouping); `engine/src/collectors/pacman.rs` and its tests
(`engine/tests/collectors.rs`, the fixture story); `engine/src/index/class.rs`
and `build.rs` (`drift_items`); `schema/event.schema.json`,
`schema/index.schema.json` (`drift[].txId`: "expand the group from
index.events by txId"); the plugin's event detail (WP-122:
`Model.js` `eventDetail`, `whyLoud`, `CRISIS_RULE_TEXTS`);
`scripts/validate-fixtures.py` (the ported class rules); `fixtures/README.md`.

## What pacman writes

libalpm logs, under the `[ALPM]` tag and before the package's own
`installed|upgraded|removed` line:

- `warning: <file> installed as <file>.pacnew` — the package's new default
  was not applied (the user's file stays in place);
- `warning: <file> saved as <file>.pacsave` — on removal, the user's
  modified file was moved aside;
- `warning: <file> saved as <file>.pacorig` — on install or upgrade, an
  untracked file was moved aside for the package's own.

Only `/var/log/pacman.log` is read (AGENTS.md §6); nothing under `/etc`.

## Decisions (what the WP leaves open)

1. **Kind and subject.** `source: pacman`, `kind: note` (the schema's
   existing kind for a recorded statement; a new kind would be a contract
   change: ADR + `contractVersion`, beyond this WP). `subject` = the file
   pacman left (`/etc/x.pacnew`), so every list names it without a lookup.
   `detail` = pacman's words without `warning: ` (`/etc/x installed as
   /etc/x.pacnew`). `meta.command` = the transaction's command line, as on
   its package events. No `explicit` (not a package).
2. **The transaction's id goes into `meta.transaction`, not the top-level
   `txId`.** The top-level `txId` is the group key: ADR-0013 §1 groups
   linkable pacman events by it, and the index contract tells the plugin
   to "expand the group from index.events by txId". A `.pacnew` inside the
   group would (a) be resolved with the packages when the user explains
   "I wanted that package" although the merge is still to do, (b) lift a
   routine `-Syu` group to attention or crisis as a whole, led by a package
   row, and (c) need ADR-0013 and the index contract changed to keep it out.
   `meta.txId` is reserved for resolutions by the schema, so the key is
   `transaction` (any scalar `meta` key is allowed). Attribution reads it:
   the note inherits actor and case from its transaction as every member
   does (ADR-0017 §2), so an agent's case that left a `.pacnew` has it
   linked (in case, not drift).
3. **Class (ADR-0028 §2, new rows).** A pacman `note` is never routine:
   attention `pacnew`; **crisis** `pacnew-red` when the original file (the
   subject without its suffix) lies in a built-in list of boot, login and
   security files: `/etc/mkinitcpio.conf`, `/etc/mkinitcpio.conf.d/`,
   `/etc/mkinitcpio.d/`, `/etc/default/limine`, `/etc/limine*` (incl.
   `/etc/limine-entry-tool.d/`, the paths Omarchy's tree writes),
   `/boot/limine*`, `/etc/systemd/`, `/etc/pam.d/`, `/etc/security/`. One
   rule id for all three forms. The list is built in, not a `[drift]` key:
   `alwaysRedPaths` holds `~`-paths of the config collector, and a new key
   is config surface the WP does not ask for (follow-up if wanted). A path
   under another root (`pacman -r /mnt`) is not the running system →
   attention. `[drift] attention = "all"`: as every pacman item, crisis
   (red zone).
4. **Not known: whether it was merged.** Seldon does not read `/etc`; the
   event records that pacman left the file, never that it is still there.
   SPEC says so; the plugin's hint says so.
5. **Plugin.** The event detail of a pacman note gets a row *Hint*: "Merge
   it with pacdiff (pacman-contrib) in a terminal. Seldon does not read
   /etc, so it cannot tell whether that happened since." Text only, no
   button, no command run. `CRISIS_RULE_TEXTS["pacnew-red"]` for the "why
   loud" callout.
6. **Fixture.** The story's 10-01 `omarchy update` (C-2026-003, Claude)
   leaves one `.pacnew` under `/etc/mkinitcpio.conf.d/` → an in-case event
   (no new drift item, so the sample's drift counts stay); the
   pre-baseline part of `fixtures/logs/pacman.log` gets a `.pacsave` on a
   removal (the parse test covers it; offsets in README and tests move).

## Steps

1. Engine collector: line rule, `Tx` file lines, note events,
   `meta.transaction`, attribution by transaction; unit tests (both forms,
   malformed lines, outside a transaction, idempotent re-read).
2. Class rows `pacnew`/`pacnew-red` in `class.rs` + table test rows.
3. Integration: capture twice → no new event; the class through `drift
   show` (attention and crisis); group stays packages only.
4. Fixtures: pacman.log, ledger line, case C-2026-003 `events:`,
   regenerate sample index and variants (`validate-fixtures.py
   --write-index`), port the rows to the script, golden files, README.
5. Plugin: hint row, crisis text, model tests.
6. Docs: SPEC-ENGINE §4, §5; SPEC-PLUGIN (event detail, crisis texts);
   CHANGELOG.
7. `flock /tmp/seldon-check.lock just check`; HANDOVER.md.
