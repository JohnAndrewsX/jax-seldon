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
- `flock /tmp/seldon-check.lock just check` on the committed HEAD
  `6bbc629` with a clean tree: **check: ok**, exit 0 (round 5, log
  `check-wp141-r5.log`). Earlier rounds on the working tree: 3 and 4 ok;
  1–2 found only harness counts that the extra fixture event moves (77
  events, `case 38`, C-2026-003's 6 linked changes, the graph's 69 nodes /
  27 edges, the 10-01 heatmap).
  Engine tests also with a temp HOME through `common::Env`.

- Mutants (`work/active/WP-141/mutants.py`, own `CARGO_TARGET_DIR`
  outside the worktree): 13 of 13 killed — the classification (never a
  crisis, always a crisis, routine, the note row removed, the suffix not
  stripped, PAM left out), `txId` versus `meta.transaction` (the note takes
  the transaction's `txId`; no `meta.transaction`; attribution ignoring
  it), the line rule (any warning accepted; a transaction of left files
  only dropped) and the plugin's hint (none; on every note).

## Not done / open

- Guard blocks, reported and not routed around: the first fixture commit
  (`omarchy command that changes the system`, from its message text), a
  `grep` whose pattern held the package manager's name and a count, and a
  second commit attempt (`privileged or package command`, message text).
  On the orchestrator's explicit instruction the 28 files were committed
  with plain content-only messages (`612936a`, `c99ec48`, `6bbc629`);
  nothing privileged ever ran. All 28 belong to WP-141; `event.rs` holds
  the intended 87 → 88 ledger-line count, no mutant.

## Open questions

- None on the contract.

## Round 2

Stage-1 review approved bf36eaa; round 2 on the orchestrator's list.

- **F2 — narrower crisis list.** `PACNEW_RED` (`class.rs`) and the
  fixture script's copy are now mkinitcpio (`.conf`, `.conf.d/`, `.d/`),
  Limine (`/etc/default/limine`, `/etc/limine*`, `/boot/limine*`) and
  `/etc/pam.d/`. `/etc/systemd/` (Omarchy uses drop-ins) and
  `/etc/security/` (Omarchy overrides `pam`'s files there) are attention,
  as are `fstab`, `crypttab` and `sudoers` (new rows in the table test).
  Round 1's text above that names systemd as crisis is superseded here.
- **Q1 — ADR-0042** (proposed; the operator accepts): the two §2 rows,
  the list, and the ownership evidence from the installed Omarchy tree
  (4.0.4-1, read-only: which `install/` scripts and migrations write
  under `/etc/mkinitcpio.conf.d/`, `/etc/default/limine`,
  `/etc/limine-entry-tool.d/`, `/etc/pam.d/`, `/etc/security/`,
  `/etc/systemd/*.d/`, `/etc/sudoers.d/`) and the reviewer's
  fstab/crypttab/sudoers finding. `DECISIONS.md` lists it; ADR-0028's
  index row names it.
- **Q2 — schema descriptions only** (`event.schema.json`): `txId` is
  "shared by the package lines of one transaction; a file pacman left …
  carries it as meta.transaction instead"; `transaction` is among the
  conventional `meta` keys. `contractVersion` unchanged.
- **F1 — Python self-checks** (`validate-fixtures.py`): a caseless note
  under `/etc/pam.d/` derives a single crisis item `pacnew-red`, one under
  `/etc/security/` a single attention item `pacnew`, neither with `txId`
  or `members` (56 self-checks). Both checks were seen failing against a
  list with `/etc/security` added and with `/etc/pam.d` broken, then the
  script was restored.
- **F4 — `.pacsave` wording.** SPEC-ENGINE §4: a `.pacsave` follows a
  removal or an upgrade that no longer ships the file, the user's file
  moved aside and nothing left in its place; the log always names
  `.pacsave`. §5 says "a merge or a restore". The plugin's `pacnew-red`
  text no longer says "until the two are merged" (wrong for a `.pacsave`):
  "… (mkinitcpio, Limine, PAM); check it with pacdiff before the next
  reboot." (SPEC-PLUGIN, CHANGELOG follow.)
- **F5 — mutant target.** `mutants.py` builds in
  `$SELDON_MUTANTS_TARGET`, else `target-mutants-wp141` in the gates dir
  (`$SELDON_GATES_DIR`, else `gates/` in the main checkout's
  `<repo>-private/` sibling, found through `git rev-parse
  --git-common-dir`; no absolute path in the file); without one it
  stops. The old `~/.cache/seldon-target-wp141-mutants` was deleted by
  explicit path. Mutants on the round-2 code: **13 of 13 killed**.
- **Q5** — the `.pacsave` stays its own item (unchanged).
- **Check:** `flock /tmp/seldon-check.lock just check` on the committed
  HEAD `cc95b69`, clean tree: **check: ok**, exit 0 (log
  `check-wp141-r6.log`).

## Stage 2 (Fable): ADR-0042 text edits

Fable approved the code (44f1a60) and asked for seven text-only edits to
ADR-0042 before the operator accepts it; applied as listed: the status
line (accepting is the operator's decision), the mkinitcpio and Limine
ownership evidence (`omarchy-settings` 4.0.4-4 backup files,
`limine-mkinitcpio-hook`'s `/etc/limine-entry-tool.conf`, what no package
owns, migrations 1786605598, 1784917531, 1789325478),
`/etc/limine-entry-tool.conf` named in the crisis row (already matched
by `/etc/limine*`, no code change), the systemd drop-ins as
`omarchy-settings` backup files, the Consequences' rarity argument
(`pambase`, `sddm`, `sudo`, and every Omarchy machine's edited
`system-auth`/`sddm-autologin`), and the mirror in `class.rs`'s
`PACNEW_RED` comment.

- **Follow-up (Fable):** a *Transaction* row in the note's event detail
  (its `meta.transaction`), and "left N files" in the detail of the
  transaction's group, so the package change and the files it left are
  seen together. Plugin only; no contract change.
- **Operator question (E20, besides accepting ADR-0042):** Soll ein
  `.pacnew` unter `/etc/pam.d/`, das nach jedem pambase-/sddm-Update auf
  jedem Omarchy-Rechner einmal auftritt, die Leiste rot machen (Krise),
  oder nur still gelistet werden?
