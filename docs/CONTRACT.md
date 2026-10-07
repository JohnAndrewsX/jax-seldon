# CONTRACT.md — Engine ⇄ Plugin

The contract is `schema/index.schema.json` (with `event.schema.json` and
`case.schema.json`) and `schema/proposal.schema.json` for the file
`index.triage` points at. This file explains it; the schema decides.
`contractVersion` is **2** since 0.2.0 (ADR-0035); 0.1.x spoke 1.

## Rules

1. The plugin reads `${XDG_STATE_HOME:-$HOME/.local/state}/seldon/index.json`
   and the files it points to (the triage proposal of `triage.path`,
   relative to the index's directory, rule 9), nothing else; the engine
   writes exactly that path (both honour
   `XDG_STATE_HOME`, both default to `~/.local/state`; decided 2026-10-01,
   plugin side lands in WP-011). Development override:
   `SELDON_INDEX=/path/to/index.json` puts the plugin into a **read-only dev
   mode**: it renders the file and probes `seldon --version`, but never runs
   capture/status or any writing command, so a fixture can never touch a
   real logbook. `SELDON_NOW=<RFC 3339>` pins the plugin clock for staleness
   tests; without it, dev mode pins the clock to the index's `generatedAt`.
2. The engine writes the index atomically (temp file + rename) after every
   command that changes the logbook, and on `seldon status` / `seldon index`.
   The file is compact JSON on one line (the fixture stays pretty-printed
   for humans); the `ledger/*.md` views are refreshed by `index`/`status`,
   so they may lag a writing command until the next `status`.
3. `contractVersion` is an integer. The plugin refuses an index with a
   different version and shows the `contractMismatch` banner with both
   numbers and the update command.
4. The index is a **view**, not a database: newest 500 events, last 50
   completed cases, 366 heatmap days, 10 snapshots, and the newest 200 open
   drift items with crises first (ADR-0020; `summary.openDrift` and
   `summary.crisis` always count all). Anything older is in the logbook,
   which the panel can open in the editor.
5. Size budget: < 1 MB. The engine keeps it with the counts of rule 4 and
   by clipping long texts (ADR-0025): in `events` and `drift`, a `detail`,
   `resolutionDetail` or `meta` string longer than 256 bytes of JSON is cut
   and ends in `… (N more characters in the ledger)`; the ledger keeps the
   full text. Cases, decisions and memory topics are not cut, except a
   case's `intent` and `result` and a decision's `lead` (below); an index of
   1 000 000 bytes or more makes the engine warn and name the largest
   section. Since contract 2 the cut is marked beside the text (ADR-0035
   §3): an event with a clipped text has `meta.truncated: true`, a drift
   item with a clipped `detail` has `truncated: true`; the suffix stays
   for humans. `truncated` is index-only, never in a ledger line. The
   first paragraphs a case's `intent` and `result` and a decision's
   `lead` carry (rule 9, ADR-0038) are clipped the same way, with `…
   (N more characters in the file)` and no flag.
6. Every field the plugin displays verbatim is user content; the plugin
   escapes it and never evaluates it.
7. Fixtures: `fixtures/index.sample.json` is the canonical example. CI
   validates it against the schema and the golden engine output.
8. Reserved case tags (ADR-0027, WP-101). `tags` is a free string array in
   the case schema and the index; the engine writes these values, and the
   plugin may read meaning into them (no schema change; added under
   contract 1):
   - `closed-by-agent` — an agent actor ran `seldon plan done` (the engine
     refuses that close without a *Result* and a *Plan › Verification*),
     or an agent's `drift explain` made the completed case.
     The panel marks the case and filters the Completed column by it.
   - `reopens:<caseId>` — `seldon plan reopen <caseId>` made this case.
   - `imported` — `seldon import task` made this case (WP-102).
   - `proposed-by:agent:<name>` — `seldon drift apply` made this completed
     case from that agent's proposed explanation (ADR-0036 §3): the words
     are the agent's, though the user applied them.
   A user's own tag with one of these values means the same to the plugin.
9. Contract 2 (ADR-0035). The index adds, all written by the engine:
   - kinds `case-updated` (`seldon plan set`: zone, risk or area changed;
     `detail` the change, `meta.risk` the risk after it) and `state-loss`
     (a capture re-baselined collectors after a lost state directory;
     subject `state-reset`, `meta.sources`, `meta.files`). A ledger from
     before contract 2 has a `note` `state-reset` instead;
   - `meta.risk` on `case-created`, `case-started` and `case-updated`
     (absent on lines written before contract 2: append-only). A
     `meta.risk` on another kind, or written by hand before v2 (0.1.x
     `seldon event --meta risk=…`), is ignored on read and dropped from the
     index;
   - `logbook.git.autocommit` `{ok, at, message}`: the last autocommit
     attempted (absent while `[git] autocommit` is off, without a
     repository, or before the first attempt);
   - `meta.truncated` / drift `truncated` (rule 5);
   - `decisions[].cases`: the ADR's cases, possibly ones the index does
     not list (rule 4);
   - `triage` `{id, at, actor, counts: {items, crises}, path, applied}`:
     the newest triage proposal, absent when there is none. `path` is
     `proposals/<id>.json` relative to the directory of `index.json`
     (so the plugin's dev mode reads a fixture's proposal next to the
     fixture). The proposal file (`proposal.schema.json`) holds items
     `{eventId, action: link|explain, caseId | title + intent, crisis,
     evidence: [{kind, ref, text?}]}`; every text in it is user content
     (rule 6), and a crisis item is applied only one by one (ADR-0028 §3,
     ADR-0034 §6). The plugin reads the file; it never writes it.
     A proposal's `crisis` is what the agent saw. `drift apply` decides a
     crisis by the engine's classification at apply time, never by the
     file's flag; the plugin's crisis block is a convenience, the engine
     is the guard. A proposal is read only when it is a regular file of
     at most 4 MiB; anything else is skipped with a build warning.
   - Until 0.2.0 is tagged, a later accepted ADR on `next` may add
     **optional** fields to the v2 schemas or refine
     `proposal.schema.json` within contract 2 (ADR-0035 §6): nothing
     required added, nothing removed or changed in meaning; fixtures,
     the reference derive and both sides in one PR, and this rule
     extended with the field.
   - Optional, ADR-0038 (WP-127); an index of an earlier contract-2
     build lacks them and is valid:
     - `drift[].rule`: the ADR-0028 §2 rule that classified the item,
       the `rule` of `seldon drift show` (`attention-all` under `[drift]
       attention = "all"`); a lowercase slug of at most 64 characters, an
       open set. The plugin reads it and runs `drift show` only without
       it;
     - `cases[].intent`, `cases[].result`: the first paragraph of the
       case's *Intent* (an imported case's after its `Imported from …`
       line) and *Result*; `decisions[].lead`: of the decision's
       *Decision*. User content (rule 6): control characters other than
       line breaks and tabs as spaces, direction and format characters
       dropped (the set of SPEC-ENGINE §6, ADR-0038 as amended by
       WP-140: also U+00AD, U+0600–U+0605, U+061C, U+180E,
       U+2061–U+2064, U+206A–U+206F, U+FFF9–U+FFFB, U+1BCA0–U+1BCA3,
       U+1D173–U+1D17A and the tags U+E0000–U+E007F),
       then redacted by the logbook's redaction on every build,
       then clipped as rule 5 says; absent without text, all withheld
       while `[redaction] patterns` do not compile;
     - `cases[].source`: an imported case's task, `~/…/file.md#line` (or
       `~/…/file.md` for a file imported whole), from the frontmatter key
       `source` that `seldon import task` writes; redacted, no control,
       bidi or format characters, at most 512 bytes (a value out of
       that shape is left out with a build warning). Display only: never
       an argument of any command. The import's marker stays the only
       idempotency key.

## Changing the contract

ADR → bump `contractVersion` → update schema → update fixture → update
engine and plugin → one coordinated merge → `seldon contract-version` and
`manifest.json.seldon.contractVersion` agree.

## Commands the plugin may run (fixed argument lists)

```
seldon --version --json
seldon status --json
seldon capture --all --json --quiet
seldon log [--case <id>] --json -- <text>       # free text is one argument after `--`; --json before it
seldon open <journal|ledger|status|logbook|caseId|ADR-NNNN> --editor --json   # path reported as JSON; the engine launches the editor
seldon plan new --zone <z> --risk <r> [--area <slug>] [--priority <p>] --json -- <title>
seldon plan start|verify|done|drop <id> --json   # id validated by the schema regex; the engine enforces the transition
seldon agent start <caseId> --json              # active case only; the engine launches the configured agent launcher detached (WP-022)
seldon agent start --new --json -- <intent>     # WP-101: creates and starts a case from the sentence, then launches as above
seldon plan reopen <caseId> --json              # WP-101: a completed case only; a new active case, nothing destroyed
seldon drift link <eventId> <caseId> [--only] --json
seldon drift explain <eventId> [--only] [--zone <z>] [--risk <r>] [--area <slug>] --json -- <text>
seldon drift dismiss <eventId> [--only] --json -- <reason>   # same rule as explain: text after `--`
seldon drift show <eventId> --json          # full member list of a group (ADR-0013)
seldon agent ask triage --json                  # WP-124, ADR-0036: launches the default agent to sort the open changes; ids only in its prompt
seldon agent ask drift|case <id> --json          # Ask agent on an event (open drift only) or a case detail
seldon drift apply <proposalId> [--item <eventId>]… --json   # applies index.triage's proposal as the user; a crisis only by --item
seldon drift discard <proposalId> --json         # removes the proposal file; the logbook is untouched
seldon decide --no-edit --json -- <title>        # then `open <newId> --editor --json` from the result
seldon rebuild --json
seldon update-impact --json
seldon doctor --only rules --json               # WP-101: read-only, on panel open (own process, not the queue); runs no probe
seldon rules update --json                      # WP-101: the rules banner's one click; rewrites only the engine's block
```

The plugin never runs `seldon drift propose` (the agent's command; ADR-0036
§2). IDs are validated by regex in QML before being passed. Free text is passed
as a single argv element, never interpolated into a shell string.
