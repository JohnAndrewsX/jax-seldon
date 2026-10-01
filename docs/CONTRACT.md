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
3. `contractVersion` is an integer. The plugin refuses an index with a
   different version and shows the `contractMismatch` banner with both
   numbers and the update command.
4. The index is a **view**, not a database: newest 500 events, last 50
   completed cases, 366 heatmap days, 10 snapshots. Anything older is in the
   logbook, which the panel can open in the editor.
5. Size budget: < 1 MB. If a section would exceed it, the engine truncates
   that section and sets `meta.truncated` (future field; needs a bump).
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
seldon --version
seldon status --json
seldon capture --all --json --quiet
seldon log [--case <id>] --json -- <text>       # free text is one argument after `--`; --json before it
seldon open <journal|ledger|status|caseId> --editor --json   # path reported as JSON; the engine launches the editor
seldon plan new --zone <z> --risk <r> -- <title>
seldon plan start|verify|done|drop <id>
seldon drift link <eventId> <caseId> [--only]
seldon drift explain <eventId> [--only] -- <text>
seldon drift dismiss <eventId> [--only] -- <reason>   # same rule as explain: text after `--`
seldon drift show <eventId> --json          # full member list of a group (ADR-0013)
seldon decide --no-edit -- <title>
seldon rebuild --json
seldon update-impact --json
seldon open <what> --editor
```

IDs are validated by regex in QML before being passed. Free text is passed
as a single argv element, never interpolated into a shell string.
