# CONTRACT.md — Engine ⇄ Plugin

The contract is `schema/index.schema.json` (with `event.schema.json` and
`case.schema.json`). This file explains it; the schema decides.

## Rules

1. The plugin reads `${XDG_STATE_HOME:-$HOME/.local/state}/seldon/index.json`
   and nothing else; the engine writes exactly that path (both honour
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
   full text. Cases, decisions and memory topics are not cut; an index of
   1 000 000 bytes or more makes the engine warn and name the largest
   section. A field that marks a cut (`meta.truncated`) is deferred; it
   needs a bump.
6. Every field the plugin displays verbatim is user content; the plugin
   escapes it and never evaluates it.
7. Fixtures: `fixtures/index.sample.json` is the canonical example. CI
   validates it against the schema and the golden engine output.

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
seldon drift link <eventId> <caseId> [--only] --json
seldon drift explain <eventId> [--only] [--zone <z>] [--risk <r>] [--area <slug>] --json -- <text>
seldon drift dismiss <eventId> [--only] --json -- <reason>   # same rule as explain: text after `--`
seldon drift show <eventId> --json          # full member list of a group (ADR-0013)
seldon decide --no-edit --json -- <title>        # then `open <newId> --editor --json` from the result
seldon rebuild --json
seldon update-impact --json
```

IDs are validated by regex in QML before being passed. Free text is passed
as a single argv element, never interpolated into a shell string.
