# ADR-0035 — Contract v2: the case's risk in the ledger, the autocommit result, a cut marker, the state-loss kind, a decision's cases and the triage proposal

**Status:** accepted (2026-10-07, by the orchestrator under the operator's
decisions of 2026-10-06 — "contract v2 in 0.2.0, ADR first" and free rein
for 0.2.0 — after an Opus review in two rounds and a Fable stage 2 that
advised acceptance with the wording now in §6; the operator may revisit it
before 0.2.0 is tagged)
**Date:** 2026-10-06

> Implements the contract v2 bundle decided on 2026-10-04 (STATUS.md,
> *Decided 2026-10-04*: autocommit result in the index, `meta.truncated`,
> a dedicated state-loss event kind) and WP-115's stage 2 (a case's risk
> in engine-written records), plus the two index fields ADR-0034 needs
> (§5 `decisions[].cases`, §6 `triage`). Amends CONTRACT.md rule 5 (the
> deferred cut marker), ADR-0025 (the marker beside the clipped text) and
> WP-081's state-reset note (its kind). ADR-0029's harm guard keeps its
> rule; only where it reads the risk changes. Work package: WP-120.

## Context

Contract 1 has served 0.1.0–0.1.4. Five changes waited for one bump:

1. **The case's risk lives only in the case file.** ADR-0029's harm guard
   (SPEC-ENGINE §5 rule 9) needs a case's risk *at the time of a change*.
   WP-115 reads it from the case's own `## Log` (`created (zone Z, risk
   R…)`, `set … risk A → B`): minute precision, the writer's local time,
   and a file the user may edit. A hand edit is caught only by the
   fail-safe "the Log's last risk must be the frontmatter's". `plan set`
   writes no ledger line because no kind fits (SPEC-ENGINE §3).
2. **The autocommit result is invisible to the plugin.** A failed
   autocommit is one stderr line and the `git` object of the command's
   `--json` (WP-061); the plugin, which runs most commands in the
   background, never sees it.
3. **A clipped text has no flag.** ADR-0025 cuts long texts in
   `index.events` and `index.drift` and appends `… (N more characters in
   the ledger)`; the plugin can only find the cut by matching that
   English suffix. CONTRACT.md rule 5 deferred `meta.truncated` to a
   bump.
4. **State loss is a note.** WP-081 records a lost state directory as a
   `seldon` `note` with subject `state-reset` (no contract change). The
   desk wants to show it as what it is, not as one note among the user's.
5. **The desk (ADR-0034)** draws decision → case edges in the graph
   (§5), which needs the decision's `cases` (the frontmatter has them,
   the index does not), and shows the agent's triage proposal (§6),
   which the plugin may only reach through the index (AGENTS.md §3).

## Decision

`contractVersion` is **2**. Every field below is written by the engine,
checked by `seldon index --check` and shown in `fixtures/index.sample.json`.

### 1. The case's risk in the ledger

- New event kind **`case-updated`** (`source: seldon`, subject and `case`
  the case id, the actor who ran it): written by `seldon plan set` when it
  changes anything (zone, risk or area), in the same ledger write as
  before the case file. `detail` holds the words of the case's Log line
  (`zone yellow → red, risk R2 → R3`). A `plan set` that changes nothing
  writes nothing, as today. Engine-only (`seldon event` refuses it, as it
  refuses the other `case-*` kinds).
- **`meta.risk`** (`R0`–`R3`): the case's risk *after* the event, on every
  new `case-created`, `case-started` and `case-updated` (required on
  `case-updated`). `case-verified`, `case-completed` and `case-dropped`
  carry none: they never change it. No other kind carries `meta.risk`.
- **Append-only.** Old lines are never rewritten: a case created before
  v2 has no `meta.risk` in its `case-created`, and an index of such a
  ledger is valid and complete (the field is optional).
- **A `meta.risk` the engine did not write counts for nothing.** 0.1.x let
  `seldon event --meta risk=…` put any value on any kind. A `meta.risk` on
  another kind, or written by hand before v2, is ignored on read and
  dropped from the index: such a line still loads (a value that is not
  R0–R3 reads as none, the line is never skipped), and only the engine's
  `case-created`, `case-started` and `case-updated` lines (`source:
  seldon`) tell a case's risk. New lines are checked strictly on write.
- **The harm guard** (ADR-0029 §1(d), SPEC-ENGINE §5 rule 9) reads the
  risk at the change's time from the ledger when the case's
  `case-created` line carries `meta.risk`: the last `meta.risk` of the
  case's `case-created|case-started|case-updated` lines strictly before
  the change's `ts`; lines in the same second count only if they agree.
  Otherwise (a case from before v2, a retroactive or imported case
  without such a line) it falls back to the case file's Log exactly as
  WP-115 does. The fail-safe stays in both paths: when the record's last
  risk is not the frontmatter's `risk`, the record tells nothing and the
  case counts as below R3.

### 2. The autocommit result

`logbook.git.autocommit` = `{ok, at, message}`: the last autocommit the
engine *attempted* in this logbook. `ok` true: committed, `message` the
commit subject (`seldon: …`). `ok` false: not committed, `message` the
git error. `at` is when it ran. One line, at most 256 characters, through
the logbook's redaction like a collector message — when written and again
when the index is built; the git error on stderr and in `--json`
`git.error` goes through the same redaction. Skips (`--no-commit`,
`git.autocommit = false`, no repository) are no attempt and change
nothing. The engine keeps the record in its state directory
(`autocommit.json`, bound to the logbook's path like `cursors.json`); the
index shows it only when `logbook.git` is present, `git.autocommit` is on
and the record is this logbook's.

### 3. The cut marker

Beside a clipped text the index says so: an event in `index.events`
whose `detail`, `resolutionDetail` or any `meta` string was clipped
carries **`meta.truncated: true`**; a drift item whose `detail` was
clipped carries **`truncated: true`** (a drift item has no `meta`). The
marker suffix stays in the text, for humans and for the `ledger/*.md`
views' readers. `truncated` is index-only: a ledger line never carries
it (`seldon event --meta truncated=…` is refused; a hand-written one is
dropped when the index is built), and an unclipped event never shows it.
The 256-byte budget of ADR-0025 is unchanged.

### 4. The state-loss kind

New event kind **`state-loss`**, `source: seldon`, subject always
`state-reset` (schema and engine refuse another),
with WP-081's `detail` and `meta.sources`/`meta.files`: what `capture`
writes when it re-baselines collectors after a lost state directory.
Engine-only. Old `note` lines with subject `state-reset` stay what they
are; the engine reads both as the same record (each loss is still
recorded once, WP-099). Not drift (source `seldon`).

### 5. A decision's cases

`decisions[].cases`: the case ids of the ADR's frontmatter `cases:`, in
the order written, deduplicated; `[]` when it names none. Always present.
Ids are copied, not resolved: a case past the index's 50 completed may be
named.

### 6. The triage proposal

`triage` (optional) = `{id, at, actor, counts: {items, crises}, path,
applied}`, present when the state directory holds a proposal of this
logbook: the newest valid `proposals/<id>.json` by id. `path` is
relative to the directory of `index.json` (`proposals/<id>.json`), so
the plugin reads it next to the index, also in its `SELDON_INDEX` dev
mode. `counts` are the file's items and the items it marks `crisis`, as
proposed. `applied` is `null` until `seldon drift apply` marks it, then
its time. A `.json` file not named `<ULID>.json`, a file that fails its
schema, whose name is not its id, or that cannot be read is skipped with
a build warning; one that belongs to another logbook is skipped silently
(it is not this logbook's).

The file is contract too, since the plugin reads it:
**`schema/proposal.schema.json`** — `{id, at, actor, logbook, applied,
items: [{eventId, action: link|explain, caseId (link) | title + intent
(explain), crisis, evidence: [{kind: journal|event|snapshot|case|plan,
ref, text?}] (at least one)}]}`, at most 200 items. `text` is the
engine-resolved evidence (user content, at most 256 characters); the
plugin escapes it and never evaluates it (CONTRACT.md rule 6). Writing
proposals (`drift propose`) and applying them (`drift apply`) are
WP-124's and ADR-0036's; this ADR fixes only the shape the index points
to. A proposal's `crisis` is what the agent saw. `drift apply` decides a
crisis by the engine's classification at apply time, never by the
file's flag; the plugin's crisis block is a convenience, the engine
is the guard.
A proposal is read only when it is a regular file of at most 4 MiB;
anything else is skipped with a build warning.

Until 0.2.0 is tagged no released engine or plugin speaks v2. Until
then a later accepted ADR on `next` may refine `proposal.schema.json`
or add **optional** fields to the v2 schemas in the same contract
version — nothing required added, nothing removed or changed in
meaning, fixtures, the reference derive and both sides in one PR,
CONTRACT.md rule 9 extended.

### 7. Plugin

`CONTRACT_VERSION` 2 in `Model.js` and `manifest.json`
(`seldon.contractVersion`); `parseIndex` accepts the new fields and
kinds. A 0.1.x plugin meets a v2 index with the `contractMismatch`
banner (both numbers, "Update the plugin") — the existing rule 3.
`engineMin` moves to 0.2.0 with the release (WP-126).

## Consequences

- Breaking: a 0.1.x plugin shows the mismatch banner against a 0.2.0
  engine and vice versa (CHANGELOG **Breaking**). A 0.1.x engine reading
  a ledger with `case-updated` or `state-loss` lines skips each such line
  with its "not a valid event" warning, doctor reports the ledger
  degraded, nothing is lost — a downgrade is not supported.
- The harm guard becomes second-precise and immune to Log edits for every
  case created by a v2 engine; old cases behave exactly as under WP-115.
- `fixtures/`: the sample logbook gains a `case-updated` line (the raise
  of C-2026-003 to R3), `meta.risk` on the case lines written after the
  story's engine update, one `state-loss` line, one clipped detail, a
  decision with two cases and a proposal file; `invalid/index.contract-v2`
  becomes `index.contract-v3`.
- `scripts/validate-fixtures.py` (the reference derive) and the engine's
  golden test follow every field; the engine compiles
  `proposal.schema.json` into `index --check`.
- CONTRACT.md rules 3, 5, 8 and a new rule for the proposal file;
  SPEC-ENGINE §2 (the two state files), §3 (`plan set`, `capture`), §5
  rule 9, §6; SPEC-LOGBOOK §4.

## Alternatives considered

- *`meta.risk` only, no new kind for `plan set`*: a `note` or
  `correction` line would carry state the engine must keep consistent
  under a kind the user may also write. Rejected.
- *Rewriting old `case-created` lines with their risk*: breaks the
  append-only rule (AGENTS.md §7). Rejected; the Log fallback covers them.
- *Recomputing triage counts against the current drift*: the index
  would then disagree with the file the plugin opens; the desk reads the
  open state of each item from `index.drift` itself.
- *An absolute proposal path*: breaks the plugin's dev mode against a
  fixture. Relative to the index's directory instead.
