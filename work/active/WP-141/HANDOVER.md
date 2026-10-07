# WP-141 — Handover: pacnew and pacsave in the record

Branch `wp/141-pacnew` from `next` (2ad42a4); merge into `next`. Plan and
the decisions the WP left open: [PLAN.md](PLAN.md).

## What was done

- **Engine, pacman collector** (`engine/src/collectors/pacman.rs`): a new
  row of the line table, `[ALPM] warning: <file> installed as
  <file>.pacnew` and `… saved as <file>.pacsave|.pacorig`, accepted only
  when the left path is the file plus the suffix its verb leaves. Each
  becomes a `source: pacman`, `kind: note` event: subject the file pacman
  left, detail pacman's words (`/etc/x installed as /etc/x.pacnew`),
  `meta.command` the transaction's command, `meta.transaction` its txId,
  no `explicit`, zone red. Attribution reads `meta.transaction`
  (`pacman::transaction`), so the note inherits actor and case from its
  transaction like every member. Dedupe and cursor unchanged.
- **Classes** (`engine/src/index/class.rs`): a pacman note is attention
  `pacnew`, **crisis** `pacnew-red` beside mkinitcpio, Limine, systemd and
  PAM files (`PACNEW_RED`); never routine. Ported to
  `scripts/validate-fixtures.py`.
- **Plugin** (`plugin/Model.js`): the Changelog's event detail of a file
  pacman left has a row *Hint*: "Merge with pacdiff (from pacman-contrib)
  in a terminal. Seldon does not read /etc, so it cannot tell whether that
  happened since." Text only: no button, no process. `CRISIS_RULE_TEXTS`
  names `pacnew-red`. A pacman note is a change in the graph (so a
  `pacnew-red` crisis is a node there too) and a linked change in a
  case's detail, not a Log line.
- **Fixtures**: `fixtures/logs/pacman.log` has both forms — a `.pacsave`
  in the 08-28 `-Rns` (before init's baseline: parsed, not emitted) and
  `/etc/mkinitcpio.conf.pacnew` in the 10-01 `-Syu` of C-2026-003 (the
  rotation fixture repeats it). One ledger line, C-2026-003's `events:`,
  the sample index and its variants (regenerated with `--write-index`;
  two overlay positions moved by one), `ledger/2026-10.md`, `STATUS.md`,
  README (offsets 6245 / 12053, 88 lines, 77 index events). The note is
  in the case (Claude's update), so the sample's drift is unchanged.
- **Docs**: SPEC-ENGINE §4 (the lines, the event, why `meta.transaction`
  and not `txId`, "whether it was merged is not known"), §5 rule 4 (the
  rows with the harm and reason test, ADR-0028 §1's demand for a new
  kind's row) and rule 5 (packages only); SPEC-PLUGIN (the hint, the
  crisis text, graph nodes); CHANGELOG (Engine, Plugin).

## Decisions (left open by the WP)

1. `kind: note`, not a new kind: a new kind is a schema change (ADR,
   `contractVersion`), beyond the WP.
2. The transaction's id is in `meta.transaction`, **not** the top-level
   `txId`. `txId` is the drift group key (ADR-0013 §1; the index schema
   tells the plugin to expand groups by `txId`). As a member, the `.pacnew`
   would be resolved by "I wanted that package", would lift a routine
   `-Syu` group to drift as a whole (led by a package row), and would need
   ADR-0013 and the index contract changed. `meta.txId` is reserved for
   resolutions by the schema; any scalar meta key is allowed, so no schema
   change.
3. The crisis list is built in (mkinitcpio incl. `.conf.d`/`.d`, Limine
   incl. `/etc/default/limine`, `/etc/limine*` and `/boot/limine*`,
   `/etc/systemd`, `/etc/pam.d`, `/etc/security`), not a `[drift]` key
   (`alwaysRedPaths` holds `~`-paths of the config collector). A key is an
   easy follow-up if wanted.
4. `.pacorig` is included (pacman's third "moved aside" form); one rule id
   for all three forms.
5. Under `[drift] attention = "all"` a pacman note is, as every pacman
   item, a crisis (red zone): the rule before ADR-0028.

## Verification

- Unit: `collectors::pacman` (`left_file_lines`, both forms and nine
  malformed or look-alike lines; `left_files_are_notes_of_their_transaction`),
  `index::class` (`every_row_of_the_table`: 15 new rows, both classes,
  every listed path family, a look-alike, another root;
  `groups_and_dependencies`: a note lends its transaction no class).
- Integration `engine/tests/drift.rs::files_pacman_left_are_their_own_items`:
  capture of three transactions; the agent's `.pacnew` inherits the case;
  the `-Syu`'s mkinitcpio `.pacnew` is a crisis while its packages stay
  routine; the `.pacsave` is attention beside its removal's group;
  explaining the group resolves 2, not the file; **capturing again and
  after a rotation to a new inode writes nothing** (idempotent).
- Fixture story and rotation tests reproduce the ledger fixture; golden
  index equals the sample; plugin model tests (144) incl. the hint.
- `flock /tmp/seldon-check.lock just check` on the working tree (all
  changes, committed and uncommitted): **check: ok**, exit 0 (round 3,
  and round 4 after the mutant run, log `check-wp141-r4.log`;
  rounds 1–2 found only harness counts that the extra fixture event
  moves: 77 events, `case 38`, C-2026-003's 6 linked changes, the graph's
  69 nodes / 27 edges, the 10-01 heatmap `33 events · pacman 8`).
  Engine tests also with a temp HOME through `common::Env`.

- Mutants (`work/active/WP-141/mutants.py`, own `CARGO_TARGET_DIR`
  outside the worktree): 13 of 13 killed — the classification (never a
  crisis, always a crisis, routine, the note row removed, the suffix not
  stripped, PAM left out), `txId` versus `meta.transaction` (the note takes
  the transaction's `txId`; no `meta.transaction`; attribution ignoring
  it), the line rule (any warning accepted; a transaction of left files
  only dropped) and the plugin's hint (none; on every note).

## Not done / open

- **Not committed: the fixture group** (`fixtures/`,
  `scripts/validate-fixtures.py`, the count and golden updates in
  `engine/tests/`, `engine/src/model/event.rs`'s 88-line count,
  `tests/plugin/`) and this file. The repository's guard hook blocked
  the commit (`guard: blocked (AGENTS.md §6 red zone): omarchy command
  that changes the system`), triggered by the commit message naming
  Omarchy's updater as the fixture story's event; nothing ran. A later
  `grep` whose pattern held the package manager's name and a count was
  blocked too (`privileged or package command`). On the orchestrator's
  instruction to commit, a second attempt (three commits, messages
  describing the content) was blocked as well (`privileged or package
  command`). Per the brief, none was reworded around further. All 28
  changed files belong to WP-141; none is a leftover. The committed commits alone do not pass `just
  check` (the engine tests expect the new fixture lines); the working
  tree does. Decision needed: commit the group as is with which
  message.

## Open questions

- None on the contract.
