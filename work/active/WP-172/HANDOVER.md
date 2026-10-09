# WP-172 — Handover

Branch `wp/172-first-run-findings` (from `next`), worktree `wt/WP-172`.

## Done

1. **(3) Seldon's own plugin is routine `seldon-self`** (engine core).
   - `engine/src/index/class.rs`: `plugin-add` and `plugin-enable` of
     `jax.seldon` (`attribution::OWN_PLUGIN`) are routine `seldon-self`,
     checked before the `plugin-toggle` row. Disable, update and remove
     keep their rows (rule 8 still explains update/enable/disable; remove
     stays attention).
   - `engine/src/config.rs`: `seldon-self` in `ROUTINE_RULES` (13 rules),
     so it is in the default `[drift] routine`, in `doctor`'s `drift` line
     and in `doctor --json` `drift.routine`. A user list without it makes
     the add attention `plugin` again (the enable falls to
     `plugin-toggle`). A default config does not write the key, so an
     existing logbook gets the rule.
   - No resolution line is written for the add: routine is a statement
     about consequence, not intent, so WP-086's review F4 (no explanation
     without provenance) stands. `attribution::own_change`'s doc comment
     says so.
   - **ADR-0050 (proposed)**, an amendment of ADR-0028 §2 by one row, with
     the residual risk (no provenance check) and the alternatives (rule 8
     for the add, "first add only", an `origin` URL check, README order).
     Indexed in `DECISIONS.md`.
   - SPEC-ENGINE §2 (`[drift] routine` defaults), §5 item 4 (the plugin
     row) and §5 rule 8 (the add gets no resolution but is routine).
   - User guide en/de: `06-configuration` (routine list and example),
     `02-concepts` (routine row).
   - No contract change: `drift[].rule` is an open set in
     `index.schema.json` (ADR-0038 §1); the plugin keeps texts only for
     crisis rules. No fixture change.
2. **(1) The tour's example is drift by default.** README "A 60-second
   tour" and `docs/user/{en,de}/01-getting-started.md` steps 3–8 now use
   an alias appended to `~/.bashrc` (Omarchy's shipped `~/.bashrc` says
   "Add your own exports, aliases, and functions here"): attention
   `config`, one line to undo, and the undo is drift again (`~/.bashrc`
   is outside the `omarchy-default` rule), so the case part (plan new,
   start, log, change, capture, link, verify, done) runs as before with
   area `shell`. Every output block is from the run below. The guide now
   also says that naming the file in the case's *Plan* links the change by
   itself (rule 9; verified in the run), and step 4 says the plugin's
   installation is routine. README's step 3 says so too.
3. **(2) What the pill counts.** README tour and feature list,
   getting-started step 5 and `03-daily-use` (en/de): by default the
   number after the mark counts crises; the Today tab (*without a case*)
   and the tooltip count the rest; `driftInBar = all` counts them in the
   bar.
4. **Also: the crash inbox in the logbook's `AGENTS.md`** (WP-166 is on
   `next`). Rules block **v5** (en/de): one bullet under *Journal and
   memory* (file a diagnosed crash with `seldon inbox add --title …
   --tag crash --actor agent:<name> --file -`, never the core, memory,
   environment or `bt full`; filing needs no case; ask in one line,
   unattended: ask nothing) and `seldon inbox add` in the *Commands*
   writing list. `rules::VERSION = 5`; the v4 texts of WP-143 (round 1
   `7e206bd`, as merged `73c0943`, both on `next`, in no release) are kept
   as renderings under `engine/templates/rules-v4/` and their four block
   hashes are in `RELEASED_BLOCKS`, so a logbook with WP-143's block is
   upgraded silently by the next capture. Tests follow (`rules.rs` unit
   and integration, `init.rs`, the golden `init-skeleton.txt`), and the
   marker line quoted in `04-working-with-agents` (en/de).
5. CHANGELOG `[Unreleased]`: two Engine entries, one Docs entry. German
   source markers set to the English commits.

Commits: `6c352d2` engine, `b5e6bac` ADR/spec/guide, `12f706b` rules v5,
`d6953a3` tour and pill docs, `a24b755` German markers, then this file.

## How it was verified

- `cargo test -j 4`: `index::class` unit tests (new rows: own add/enable
  routine `seldon-self`, own disable `plugin-toggle`, own remove/update
  attention, look-alike ids `jax.seldon-extra`/`jax.seldon.x` not
  matched; dropping `seldon-self` from the config falls through),
  `tests/own_changes.rs` (rewritten
  `adding_enabling_disabling_its_plugin_is_no_drift_removing_it_is`:
  own add + a third-party add in one capture → open drift only the third
  party's, `drift --all` shows `routine · seldon-self`, no resolution,
  index valid, `openDrift 1`; enable/disable explained by rule 8; remove
  is drift; the catch-up test now expects no resolution for the add),
  `tests/drift_classes.rs` (doctor's line names `seldon-self` after
  `plugin-toggle`), `tests/drift.rs`, `tests/doctor.rs`, `tests/rules.rs`,
  `tests/init.rs`, `tests/skills.rs`.
- **The tour on a fresh temp HOME** (dev host, everything on disk under
  `jax-seldon-private/gates/tmp-WP-172/`, a private 0700
  `XDG_RUNTIME_DIR`, `env -i` per command, the branch's debug engine):
  `~/.config` copied from `$OMARCHY_PATH/config`, `~/.bashrc` from
  `$OMARCHY_PATH/default/bashrc` (as Omarchy's installer does), the theme
  file, a dummy git identity. `seldon --version`, `init
  --non-interactive`, `doctor`, the plugin install, `capture`, `drift`,
  `drift --all`, the alias, `drift explain`, `plan new|start`, `log`, the
  undo, `capture`, `drift`, `drift link`, `plan verify|done`, `status`,
  `git log --oneline`, `rebuild`, and a rule-9 check (a third case whose
  Plan names `~/.bashrc`: the capture linked the change itself). Once in
  English, once with `LANG=de_DE.UTF-8` (a German logbook) for the German
  page's case titles and slugs.
- **Acceptance "installing the plugin leaves `seldon drift` at 0":** after
  the install the capture wrote `plugin-add jax.seldon` and the
  `shell.json` change; `seldon drift` → `No open drift.`; `drift --all`
  → both `routine` (`0 open drift item(s), 0 crisis; 2 routine`).
- `python3 scripts/docs-check.py --seldon <branch engine>`: ok.
- `SELDON_FULL_CHECK=1 just check` on `a24b755` (CARGO_TARGET_DIR,
  TMPDIR and a 0700 XDG_RUNTIME_DIR on disk, `CARGO_BUILD_JOBS=4`):
  `check: ok`, exit 0, the Quickshell harnesses included (`ipc-restart:
  44 passed`). Not covered here: shellcheck is not installed on the dev
  host (`bash -n` only; CI runs it); `check-runtime-space` reports on
  `/run/user/1000` (2 %) whatever `XDG_RUNTIME_DIR` says (it only reads).

## Not done, deviations

- **The plugin install was simulated** (orchestrator decision after the
  guard block, below: no network). The `omarchy` the engine runs was a
  stub that only answers `plugin list --json` (the live shell's IPC is out
  of reach from a private runtime dir); the plugin folder was a copy of
  this worktree's `plugin/` (so its add has no "installed by git clone"
  detail), and `shell.json` got the widget by hand. The real `omarchy
  plugin add … --enable` path (clone, rescan, IPC enable) was not run;
  a T1 repeat on the test host would close that.
- **Not re-run:** the interactive `seldon init` wizard (needs a TTY; its
  example output in step 2 is unchanged) and the install script (step 1).
- **Doctor's `snapper` row** in step 3 stays `degraded` with the fix, as a
  fresh Omarchy shows it (T1); this dev host has the ACL grant, so its
  real row was `ok … 6 snapshots read from the info files`. The capture
  blocks show that granted line (the text says so). The machine id is
  `<machine>` in the docs.
- `seldon --version` says `0.1.4` on `next`; the docs keep `0.1.4`.

## Guard block (reported, not routed around)

The first setup command was blocked by `scripts/guard.sh` (fail closed:
my own wrapper script `tenv` with `bash -c` in its arguments; nothing
ran). I asked; the orchestrator allowed direct `env -i … seldon` calls in
a temp HOME on disk, without network. Everything after that ran in that
form.

## Open questions

1. **ADR-0050 is proposed.** It needs the operator's (or Fable's)
   decision; engine core, so the WP names Fable as a reviewer. The
   "first add only" variant is described there; the rule id would stay.
2. The WP names "`crash draft`" for the rules paragraph; no such command
   exists on `next`, so the paragraph names only `seldon inbox add`.
3. ADR-0050 is the next free number on `next`; WP-175 runs in parallel
   and may want a number too.
4. Should the AGENT-GUIDE and the skill's `drift.md` routine lists name
   Seldon's own plugin as well? Left unchanged (the rules block's v5 is
   only the crash inbox, as the WP says).

## Round 2 (review 1: SEND BACK, B1; N1, N2, N3 folded in)

- **B1.** The tour was run again on two fresh temp homes (en, de) **without
  the snapshot grant**: `SELDON_SNAPSHOTS_DIR` pointed at a mode-000
  directory on disk, with the real `snapper` (which refuses `list` for
  this user), so `init`'s first capture fails on snapper as on a fresh
  Omarchy. `doctor` then shows both `degraded collectors last capture
  failed: snapper: … No permissions …` and `degraded snapper …`, each
  with `fix: sudo setfacl -m u:$USER:rx /.snapshots`. Step 3 (en, de)
  shows that real `collectors` row and fix line; the sentence below says
  `snapper` and `collectors` say `degraded` until the grant. The wizard's
  non-interactive summary in that run also said `Snapshots   not readable
  yet …`, as the guide's step-2 example does.
- **N1.** Every block of the tour is now from the no-grant run: the
  step-5 capture shows the degraded `snapper` line with its fix (the text
  says the line changes after the grant), and the step-7 `status` block
  shows the extra `snapper degraded: …` line, with one sentence below it
  ("there until you run the grant of step 3. Your counts differ.").
- **N2.** The example is `alias gs='git status'` (not in Omarchy's
  `default/bash/aliases`, whose git aliases are `g`, `gcm`, `gcam`,
  `gcad`); explanation "A short git status" / "Ein kurzes git status",
  case "Remove the gs alias again" / "Den gs-Alias wieder entfernen".
  README, en and de: every id, slug, journal line and `git log` subject
  is from these runs. The rule-9 claim was checked again with `gs`
  (`note: 1 event(s) linked to the one case that planned them while it
  was open`, `No open drift.`).
- **N3.** `docs/SPEC-ENGINE.md` (doctor `rules` row `current (v5)` and
  `updates it to v5`; the init template marker `rules v5`; and two more
  of the same kind in the `rules update` synopsis: "this engine's v5
  block", "a block newer than v5") and `docs/AGENT-GUIDE.md` (marker
  `rules v5`). The historical "Rules v3 (WP-111) quote …" stays.
- Q4: no change to the AGENT-GUIDE or skill routine lists.
- Verified: `python3 scripts/docs-check.py --seldon <branch engine>` →
  `docs-check: ok (472 links, 14 translated pages, 61 commands, 630
  command lines)`. No code changed, so no full check (per brief). German
  marker of page 01 → `8ffcf717`.
- Same form as round 1: `env -i` per command, private 0700 runtime dir
  and temp homes on disk, stub `omarchy` for `plugin list` only, the
  plugin folder copied from `plugin/` (no network); temp homes deleted.
