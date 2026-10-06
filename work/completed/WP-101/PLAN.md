# WP-101 — Plan

Order: engine commands with tests first, then fixture and contract text,
then the panel. One commit per step, `just check` (under
`flock /tmp/seldon-check.lock`) at the end. No contract change: no new
event kind, no new index field; `tags`, `snapshotBefore`, `status`,
`actor` carry everything.

## Engine

1. **Refactor `plan new` / `plan start`** into helpers that run under a
   lock the caller holds (`create`, `start`), so `agent start --new` and
   `plan reopen` create and start a case under one lock hold.
2. **`plan set <ID> [--zone] [--risk] [--area] [--actor]`**: at least one
   flag; exit 1 on a completed or dropped case; frontmatter through
   `CaseFile::save`, one Log line `set risk R1 → R3, zone …`; no ledger
   event (no fitting kind; a new one would be a contract change); a value
   equal to the current one changes nothing ("nothing changed", exit 0);
   `--json` `{case, changed, git}`; autocommit; index rebuilt.
3. **`plan snapshot <ID> <N> [--actor]`**: open case only; sets
   `snapshotBefore` when empty, else exit 1 naming the number it has (same
   number again: exit 0, nothing written); Log line `snapshot N`;
   validation as warnings (never a refusal): the snapshot exists (the
   snapper read path, ADR-0026), its instant is after the case's
   `case-started` event and before the case's first red event; without
   snapper access the existence check is a warning that says so. `plan
   start --snapshot N` gets the "before the first red event" check only.
4. **Hook fallback**: when the attribution pass gives a
   `snapper/snapshot-create` event a case whose `snapshotBefore` is empty,
   the number fills it, with a Log line (ADR-0027 §3).
5. **Pruned rollback**: a `snapshot-delete` whose number is some case's
   `snapshotBefore` → Log line `rollback for <ID> pruned (snapshot N)` on
   that case (once), and a `doctor` row `rollbacks`.
6. **R3 advisory**: a red event with an `alwaysRed` subject in a case
   below R3 → a Log line on the case (once) and an index warning (the
   existing `warnings` channel).
7. **Agent close**: `plan done` by an agent actor (flag or
   `SELDON_ACTOR`) is refused with exit 1 and the reason when *Result* is
   empty or *Plan › Verification* is unfilled; a human close unchanged;
   an agent close adds the tag `closed-by-agent`.
8. **`plan reopen <ID> [--actor]`**: completed cases only; a new active
   case `Reopen: <title>` (zone, risk, area, priority copied), *Intent*
   copied, tag `reopens:<ID>`, Log lines on both, the new case becomes
   the active case; a second reopen makes a second case and says so;
   `--json`.
9. **`agent start --new [--zone] [--risk] [--area] [--launcher] --
   "<intent>"`**: title = first sentence ≤ 72 characters, *Intent* = the
   text (redacted; heading and fence lines escaped), created and started
   under one lock hold, then launched as `agent start`. The built-in
   launcher without an Omarchy default agent is refused before anything
   is written, naming the fix (`omarchy default agent <name>`); a
   launcher that fails after the case exists leaves the case active and
   names it with the retry.
10. **Rules text**: the templates (en/de) name `plan snapshot`, `plan set
    --risk R3`, the refused agent close, `plan reopen`; the previous v2
    block hashes go into `RELEASED_BLOCKS` (a dev logbook with the old
    block is rewritten without an archive).
11. Tests per command (`--json`, exit codes, actor from the environment,
    refusals), `actor_env.rs` refusal loop, the `SELDON_ACTOR=agent:x`
    close; mutants.

## Fixture and contract text

12. `fixtures/logbook/`: a completed case closed by an agent
    (`closed-by-agent`) and an active case with `reopens:`; regenerate
    `index.sample.json`; schema validates.
13. CONTRACT.md: reserved tags (`closed-by-agent`, `reopens:<ID>`,
    `imported`), the two new plugin commands. SPEC-ENGINE §3/§4/§5,
    SPEC-LOGBOOK §3, SPEC-PLUGIN Work tab, guides 03/04/05 en/de,
    CHANGELOG.

## Panel

14. Work tab: an intent field + *Run* (`agent start --new --json --
    <intent>`, fixed argv, the text one argument), busy state, the
    engine's refusal shown; the manual form stays behind its button.
15. Completed cards: the "closed by agent" marker, a filter toggle in the
    Completed column, *Reopen* on every completed card (one click, no
    arming; key `r`).
16. Doctor on panel open (decision in the handover): `rules` outdated →
    banner with a one-click `seldon rules update --json`, if it fits; else
    left to the CLI.
17. Harness cases (panel-view, service-states, model tests), `omarchy
    plugin validate`, `qmllint`.
