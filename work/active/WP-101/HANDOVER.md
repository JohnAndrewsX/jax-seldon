# WP-101 HANDOVER

Branch `wp/101-one-click-close`, worktree `wt/WP-101`, from `f647b63`.
No push, no PR. Plan: `work/active/WP-101/PLAN.md` (step 16, the doctor
banner, done; see Decisions 1).

## Done

Engine (ADR-0027 §2c, §3, §5, §6):

- `seldon agent start --new [--zone] [--risk] [--area] [--launcher] [--json] -- "<intent>"`:
  title = first sentence (line break, or `.`/`!`/`?` before white space
  or the end; final `.` dropped), ≤ 72 characters cut at a word with
  `…`; Intent = the whole text, redacted, heading and fence lines
  escaped (`\#`, `` \``` ``, `\~~~`); created and started under one lock
  hold (case-created + case-started in one ledger write, one commit),
  then launched exactly like `agent start <ID>` (id-only prompt,
  `SELDON_ACTOR`, `SELDON_ATTENDED=1`); `--json` as `agent start` plus
  `created`. Refusal without a default agent (built-in launcher only,
  read-only check of `~/.config/omarchy/defaults/agent`) before
  anything is written, naming `omarchy default agent <name>`.
- `seldon plan set <ID> (--zone|--risk|--area)… [--actor]`, `seldon plan
  snapshot <ID> <N> [--actor]`, `seldon plan reopen <ID> [--actor]`, all
  with `actor_or_env(.., parse_person, ACTOR_HUMAN)` before
  `open_logbook`; `--json`; refusals exit 1.
- `plan snapshot` checks (warnings only): exists (info file decides when
  the directory lists snapshots, else the ledger's last
  snapshot/snapshot-delete of N, else "cannot check" with the read
  grant), not before the case's last `case-started`, not after its
  first red event. `plan start --snapshot` gets the red check.
- Agent close: `plan done` by the *resolved* actor (flag or
  `SELDON_ACTOR`) refused while *Result* is empty (comments do not count)
  or the Plan's `Verification:` is unfilled; agent close adds
  `closed-by-agent`; human close unchanged.
- Capture notes (`engine/src/case_notes.rs`, after the append): a new
  snapshot fills an open case's empty `snapshotBefore` (description =
  case id, else the recorded `snapper … create` / `omarchy-snapshot
  create` / `omarchy snapshot create` in the attribution window — the
  hook now records these as green commands with a case); a
  `snapshot-delete` of a case's `snapshotBefore` → `rollback for <ID>
  pruned (snapshot N)` once, and doctor row `rollbacks`; a red
  `alwaysRed` change in an open case below R3 → `advisory: …` Log line
  and an index build warning (`r3_advisories`, existing warnings
  channel, no field).
- Rules templates en/de name `plan set --risk R3`, `plan snapshot`, the
  refused close, `closed-by-agent` and `plan reopen`; WP-100's v2 block
  kept under `engine/templates/rules-v2/`, its hashes in
  `RELEASED_BLOCKS` (rewritten by `rules update` without an archive).

Plugin:

- Work tab: intent field + *Run* (`agent start --new --json -- <intent>`,
  fixed argv, the text one argument), busy "Running" with spinner,
  engine refusal in the result line (two lines, wraps), text kept until
  the case exists, cursor to the new case; key `i`.
- "by agent" on the tile, "completed by agent" on the card, *By agent*
  toggle (header) filtering the Completed column ("COMPLETED 1 / 2").
- *Reopen* on every completed card (one click, no arming; key `r`;
  `plan reopen <id> --json`), cursor to the new case, meta line
  "reopens <id>".
- Rules banner: `doctor --json` on panel open (Decisions 1), *Update
  rules* → `rules update --json`, doctor again.

Fixture and contract text: `fixtures/logbook/` C-2026-002 closed by
`agent:claude-code` (tag, ledger actors, Log lines), C-2026-003 raised to
R3 (Log line `set risk R2 → R3`); variant `case-reopened` (an active
C-2026-009 with `reopens:C-2026-002`); sample and variants regenerated,
schema validates. CONTRACT.md rule 8 (reserved tags) and the four new
plugin commands. SPEC-ENGINE §3/§5/§6/§8, SPEC-LOGBOOK §3, SPEC-PLUGIN
§3/§5, AGENT-GUIDE, guides 03/04/05 en and de (de re-stamped to
`2ca5b94`), CHANGELOG.

## Not done

- **Live check on the test host**: the orchestrator's, after the merge.
- **R3 advisory in the panel.** ADR-0027 §2c says "a Log line and a panel
  warning"; the WP says "index warning (existing warnings channel; no new
  field)". The index build warning reaches stderr and the `warnings` of
  `index`/`status`/`dossier`, not `index.json`, so the panel cannot show
  it without a field or a reserved tag (open question 1).
- `plan snapshot` keeps one number (the `root` config's, as the snapper
  collector reads only `root`); other configs' numbers stay Log lines.
- The panel screenshot `docs/images/panel-tokyo-night-work.png` predates
  the intent row and the *By agent* toggle; not regenerated.
- WP-102 (`import task`) is not part of this WP; `imported` is only
  listed as reserved.

## Decisions

1. **Doctor on panel open: yes, narrow.** `seldon doctor --json` runs in
   its own read-only process (it takes no lock), when the panel opens or
   the engine turns up while it is open, at most every 10 min, never in
   dev mode. Only the `rules` row is read. One click only when the fix is
   plain `seldon rules update` (also "(archives your copy)"); a damaged
   or newer block shows its fix as text (the `--replace` archive is the
   user's call). Reason: ADR-0027's migration names the panel banner; an
   old logbook otherwise keeps propose-and-wait rules, which costs the
   user steps on every case. The fake engine logs doctor to its own file
   so the queue's argv and overlap checks stay exact.
2. **No ledger event for `plan set` and `plan snapshot`.** No kind fits;
   a new kind would be a contract change. The Log line and the commit are
   the record. `plan reopen` writes case-created + case-started.
3. **Reopen**: completed cases only (dropped refused: start a new case);
   title "Reopen: <title>" in English (engine words, like Log lines);
   zone, risk, area, priority and Intent copied; created *and started*
   (ADR: "a new active case"); Open stays the card's first action (Enter
   opens, as before), Reopen second, key `r` without arming.
4. **`--new` creator** = `$SELDON_ACTOR` else human (no `--actor` flag:
   the launched agent's actor is the launcher's); the creator is checked
   before anything (actor_env refusal loop covers it).
5. **Launcher failure after `--new` created the case**: the case stays
   active (it holds the user's sentence); exit 1 names the case and the
   retry. The default-agent pre-check catches the common case before
   anything is written.
6. **Snapshot fallback also by description.** The rules have the agent
   write `-d "<ID>"`; a snapshot whose description is an open case's id
   fills it even without hooks. Then the recorded command (window: 10 min
   before to 2 min after the snapshot's date). `post` snapshots never.
7. **Pruned**: matched by the bare number (the collector reads `root`
   only), only for deletes on or after the case's creation day; completed
   cases get the Log line too (doctor `ok` with the words), dropped
   cases nothing; doctor `degraded` only for an open case.
8. **Advisory scope**: Log line from capture-written events (the
   package collector's attributed events carry the package subject; a
   hook's command event carries the program, so it never matches);
   index warning only while the case is active or in verification and
   below R3, once per case and subject (a completed case would warn
   forever).
9. **Fixture**: C-2026-003 (the Omarchy update) is R3 — Omarchy itself is
   R3 under ADR-0027; as R2 every fixture-based command printed two
   advisories. The reopen lives in the index variant `case-reopened`, not
   in the logbook: a ninth case moved every list the plugin harness walks
   by keys (≈40 expectations); the variant is the established pattern
   (`drift-explained-case`). Deviation from "a case with `reopens:`" in
   the logbook: it is in the fixture set, not in `index.sample.json`.
10. **Verification parsing**: the Plan's `Verification:` item,
    case-insensitive, as a list item (`-`, `*`, `+`) or a plain line;
    text after the colon or on the lines indented below it; HTML comments
    stripped. A Plan without the item counts as unfilled.

## Tests

New:

- `engine/tests/close_path.rs` (14): `plan set` (changes, Log line, no
  ledger event, same value = no change, human output, refusals incl.
  closed case and clap's "nothing to set"), `plan snapshot` (once, same
  number, other number exit 1, 0 refused; before-start and
  after-first-red warnings with another case's red change not counting;
  missing and uncheckable; `plan start --snapshot` checks only the red
  change; closed case refused), agent close (refused by `--actor` *and*
  by `SELDON_ACTOR` alone, ledger and file untouched; comment ≠ Result;
  Result alone not enough; indented verification counts; one-line
  verification; human close never refused, no tag), `plan reopen` (new
  active case, copied fields and Intent, tags, Log lines on both, events,
  active case; second reopen names the first; active and dropped
  refused).
- `engine/tests/agent.rs` `new::*` (6): create + start + launch with
  the id-only prompt, escaped Intent, area created, creator human;
  titles (sentence end, `?`, `v1.2.3`, first line, 72 cut); no default
  agent → nothing created, another launcher not checked; failing
  launcher leaves the case active with the retry; argument shapes
  (exit 1); creator from `SELDON_ACTOR`, a bad value refused.
- `engine/tests/case_notes.rs` (12): fill by description, by the
  recorded `snapper create` (hook records it green with the case), by
  `omarchy-snapshot create`; nothing without a match (window, `post`,
  `snapper -c create list`), a command outside each window of two
  snapshots; never replaced; pruned once + doctor `rollbacks` degraded;
  completed told, dropped not; a reused number is not a later case's
  rollback; advisory Log line + capture stderr + `index` warning, one
  line for a second change, gone at R3, gone when the case is closed;
  other packages nothing.
- `engine/tests/actor_env.rs`: the refusal loop lists `plan set`,
  `plan snapshot`, `plan reopen` and `agent start --new`.
- Unit: `logbook::cases` (Intent placement, escaping, comments,
  close gaps), `commands::plan::snapshot` (taken, checks),
  `logbook::rules` (WP-100 block rewritten without archive, an edited
  one archived).
- Changed by design: `agent.rs` end-to-end close, `actor_env.rs`
  plan steps, `journal.rs` German stub (each now writes the evidence an
  agent's close needs before `plan done`); `plan.rs` R2/R3 start
  (snapshot 7 now exists as an info file).
- Plugin: `model.test.js` +4 tests (argv forms and refusals, answers of
  `--new` and reopen, marker/filter/headers/reopens, rules banner);
  `panel-view.sh` cases 30–33 (work-agent, work-reopened, work-run,
  work-run-refused, work-reopen, rules-outdated, rules-damaged, and the
  sample asks no doctor), each with exact argv checks where it writes;
  the fake engine speaks `agent start --new`, `plan reopen`, `doctor
  --json` (own log) and `rules update`.

## Mutants

Script in the session scratchpad; each mutant applied alone, the named
test binaries run, the file restored; 45 mutants.

| # | Mutant | Killed by |
|---|---|---|
| M1 | done refusal ignores `SELDON_ACTOR` | close_path close::an_agent_close… |
| M2 | Result not checked | same |
| M3 | Verification not checked | same |
| M4 | comments count as Result | same |
| M5 | continuation lines not read | same |
| M6 | sibling items count as verification | cases::close_gaps… |
| M7 | no `closed-by-agent` tag | close_path, agent e2e |
| M8 | human close tagged | close::a_human_close… |
| M9 | `plan set` on a closed case | set::refusals |
| M10 | same value is a change | set::changes… |
| M11 | snapshot overwrites another number | snapshot::records_the_number_once… (first form did not compile; rewritten) |
| M12 | same number written again | same |
| M13 | start not checked | snapshot::warns_about… |
| M14 | another case's red counts | plan::snapshot unit |
| M15 | `seldon` events count as red | close_path snapshot |
| M16 | listed dir without N = unknown | snapshot::a_missing… |
| M17 | a delete ignored | plan::snapshot unit |
| M18 | reopen of an active case | reopen::only_a_completed… |
| M19 | reopen without tag | reopen::makes… |
| M20 | reopen without Intent | same |
| M21 | earlier reopens not named | reopen::a_second… |
| M22 | old case without Log line | reopen::makes… |
| M23 | no default-agent check | new::without_a_default_agent… |
| M24 | Intent not escaped | new::creates_starts… |
| M25 | no 72-character cut | new::the_title… |
| M26 | final `.` kept | same |
| M27 | failed launch restores instead of keeping | new::a_launcher_that_fails… |
| M28 | `post` snapshots fill | case_notes nothing_fills… |
| M29 | description not matched | case_notes (3 tests) |
| M30 | window filter dropped | a_command_outside_each_snapshots_window… (survived the first run; test added) |
| M31 | pruned ignores the creation day | a_reused_number… (survived; test added) |
| M32 | pruned for a dropped case | a_completed_case_is_told… |
| M33 | Log line not deduped | r3 advisory test, second change (survived; test added) |
| M34 | R3 case warned | r3 advisory test |
| M35 | closed cases warned | a_closed_case_is_no_longer_warned_about (survived; test added) |
| M36 | hook ignores snapshot commands | case_notes |
| M37 | snapper option value not skipped | case_notes |
| M38 | doctor: open pruned not degraded | pruned test |
| M39 | WP-100 block not Seldon's | rules unit |
| M40 | fence lines not escaped | new::creates_starts… |
| M41 | Omarchy snapshot route ignored | omarchys_snapshot_command… |
| M42 | advisory Log line missing | r3 advisory test |
| M43 | advisory index warning missing | same |
| M44 | refusal off | close::an_agent_close… |
| M45 | `apply` fills a filled `snapshotBefore` | **survives, equivalent**: the notes are made only for open cases whose field is empty, once per case per capture, and `apply` reads the case under the same lock; the guard stays as the write-side rule "never replaced" |

44 of 45 killed (4 after adding tests), 1 equivalent.

## Check

- `flock /tmp/seldon-check.lock just check` → `check: ok`, exit 0, at
  `0887bba` (1684 engine tests in 74 binaries passed, the watch feature
  too; validate-fixtures ok, 9 variants; docs-check ok, 430 links,
  45 commands, 525 command lines; plugin-validate ok; qmllint ok,
  29 files; model 93, service-states 314, panel-view 842, overlay-view
  326, bar-view 143, all passed). shellcheck is not installed here
  (`bash -n` only, as the recipe says).
- `omarchy plugin validate plugin/` exit 0; `qmllint` in the check.
- Not run here: the live check on the test host (the orchestrator's),
  `check-perf` (the hook path changed only by one early `if`; the index
  build gained one pass over the events that returns at once when no
  open case is below R3 — worth a look in the next perf run).

## Open questions

1. R3 advisory in the panel: a field (contract bump) or a reserved tag
   the engine sets (e.g. `r3-advisory`, no bump)? Today: Log line + build
   warning only.
2. Merge with current `main`: one conflict, `engine/src/commands/doctor.rs`
   (WP-105 added `&shown` to `check_collectors`); resolution: keep both
   lines — `checks.push(check_rollbacks(logbook));` then
   `checks.push(check_collectors(ctx, &effective, logbook, &shown));`.
   The `rollbacks` message holds case ids and numbers only (no user
   text to redact).

# Round 2

Brief: `review-0.1.1/handovers/WP-101-round-2-brief.md` (stage-1 review
`WP-101-review-1.md`, SEND BACK). Every item done. No merge of `main`
(the doctor.rs conflict with WP-105 stays for the orchestrator: keep both
pushes).

## Done

- **B1 — snapshot ownership** (`engine/src/case_notes.rs`). A
  snapshot's window is `[date − 10 min, date + 5 s]` (`SKEW`, was
  120 s). Snapshots are taken oldest first; a recorded command owns one
  snapshot (`used`), the latest unused one of a single case in the window
  owns it; a snapshot named by its case id uses that case's latest
  command in its window. Unused commands of two or more cases in one
  window: nothing filled, and each such case (open, field empty) gets
  `snapshot N was taken while the agents of <IDs> ran a snapshot
  command; if it is this case's rollback, record it: `seldon plan
  snapshot <ID> N``. Tests: the reviewer's probe (two agents, a minute
  apart → 2 and 3), a later command beyond the skew claims nothing (and
  within it does), the tie on both cases, a described snapshot uses its
  command.
- **B2 — reopen and the active-case marker** (`plan.rs`, `Spec.point`).
  `plan reopen` sets `.seldon/active-case` only when it names no open
  case; otherwise the new case is active but not the marker, the output
  says "The active case stays <ID>: commands an agent runs are still
  recorded on it. `seldon agent start <NEW>` hands the new case to an
  agent", JSON `activeCase: {kept}`. The panel's result line adds "the
  active case stays <ID>"; the fake engine mirrors it. Test: after a
  reopen, an agent's `hook generic` command without a case lands on the
  original case; a stale marker (completed case) or none: the reopen
  takes it.
- **N1** the fallback runs `plan snapshot`'s checks (`snapshot::warnings`,
  now `pub(crate)`); each warning is a Log line by `system`, the snapshot
  still fills. Test: red change at 10:05, snapshot at 10:10.
- **N2** `plan done --actor human` with `SELDON_ACTOR=agent:*`: exit 1,
  "`--actor human` in a session of agent:x (SELDON_ACTOR); an agent's
  close is never recorded as human (ADR-0027 §5) …", nothing written; a
  person's own session (no variable) still closes as human. An invalid
  variable is ignored here, as `--actor` always ignored it.
- **N3** a first sentence without a Unicode letter or digit (`.`, `!`,
  `…`, `?!`, `!` followed by a line) is refused before anything is
  written.
- **N4** control characters (a lone `\r`, a tab) are spaces in the
  title, white-space runs one. Not changed: `Install e.g. zed` still
  cuts at `e.g` (a sentence end is `.` before white space; an
  abbreviation list would be language-specific).
- **N5** no early return in `classify_segment`: a snapshot command is a
  green, cased classification folded with the files the segment writes
  (subject stays `snapper`). Test: `snapper create … > ~/.config/hypr/…`
  is recorded yellow with the case.
- **N6** `cases::has_text`: a Result (and the Verification text) counts
  only with a line that is no heading and holds a letter or digit,
  comments stripped (zero-width characters and punctuation are no
  letters); a bold label counts (`- **Verification:** x`,
  `**Verification**: x`, `__verification:__ x`). SPEC-ENGINE §3 says it
  is a guard against forgetting, not a check of the evidence.
- **N7** an agent's `drift explain` tags its completed case
  `closed-by-agent`; a person's gets none. CONTRACT rule 8 says so.
- **N8** harness: the panel can be closed and opened (`close`/`open`
  steps); a second open within 10 minutes asks doctor nothing (one line
  in `doctor.log`); the forced check after *Update rules* stays covered.
- **N9** the default-agent check reads the first line, trimmed, as
  `omarchy-default-agent` does; an empty first line is no agent (test).
- **Open question 1 (decided)**: no tag, no field in 0.1.4. SPEC-ENGINE
  §5 states the deviation from ADR-0027 §2c ("the panel shows it from
  contract v2"; ADR-0028 points the same way). Not done in the panel by
  decision.
- Docs: SPEC-ENGINE §3/§5, SPEC-LOGBOOK §3, SPEC-PLUGIN §5, CONTRACT rule
  8, guides en 03/05 and de 03/05 (re-stamped `e852f0c`), CHANGELOG.

## Mutants (round 2)

Script in the session scratchpad; each applied alone, the named tests
run, restored; 16 of 16 killed, all compiled.

| # | Item | Mutant | Killed by |
|---|---|---|---|
| R1 | B1 | window end 120 s | a_later_command_does_not_claim…, two_agents_a_minute_apart… |
| R2 | B1 | commands not consumed | two_agents_a_minute_apart… |
| R3 | B1 | a tie takes the latest | a_tie_fills_nothing_and_says_so_on_both |
| R4 | B1 | a described snapshot keeps the command | a_described_snapshot_uses_its_cases_command |
| R5 | B1 | a tie writes no Log line | a_tie_fills_nothing… |
| R6 | N1 | fallback without the checks | the_fallback_warns_like_plan_snapshot |
| R7 | B2 | reopen always takes the marker | a_reopen_keeps_the_active_case_of_an_open_case |
| R8 | B2 | any marker holds (stale too) | same |
| R9 | N2 | human close in an agent session allowed | an_agent_session_cannot_close_as_human |
| R10 | N2 | the check fires without the flag | three close tests |
| R11 | N3 | only an empty title refused (the stage-1 survivor) | new::the_argument_shapes |
| R12 | N3 | no title check | same |
| R13 | N6 | a heading is text | cases::close_gaps…, close_path |
| R14 | N6 | any character is text | same |
| R15 | N6 | bold label not read | same |
| R16 | N6 | Verification by non-blank | same |

N8's throttle is killed by its own harness case (a check without the
throttle makes the second open call doctor again: two lines).

## Check

`flock /tmp/seldon-check.lock just check` → `check: ok`, exit 0, at
`4641b06`: 1704 engine tests in 74 binaries (the watch feature too);
validate-fixtures ok (9 variants); docs-check ok (430 links, 45
commands, 525 command lines); plugin-validate ok; qmllint ok (29 files);
model 93, service-states 314, panel-view 847, overlay-view 326,
bar-view 143. shellcheck not installed (`bash -n` only). Live check on
the test host: the orchestrator's.

## Open

- Merge: `engine/src/commands/doctor.rs` conflicts with WP-105 (keep
  `checks.push(check_rollbacks(logbook));` and WP-105's
  `check_collectors(ctx, &effective, logbook, &shown)`).
- `Install e.g. zed` → title `Install e.g` (N4, left as is; say if an
  abbreviation rule is wanted).
