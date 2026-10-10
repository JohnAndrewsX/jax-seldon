# ADR-0051 — Contract forward compatibility: `contractReadableFrom`

**Status:** accepted 2026-10-09 (operator: direction E55, text E76; WP-176)
**Date:** 2026-10-09

> Adds one **optional** field to contract 2 under ADR-0035 §6, before
> 0.2.0 is tagged: nothing required is added, nothing is removed or
> changes meaning, `contractVersion` stays 2. Amends CONTRACT.md rule 3
> (the plugin's version check) and binds every later `contractVersion`
> bump. From the prototype 0.3 debate (SYNTHESIS §2 item 2, C9(a); the G3
> devil's advocate). Work package: WP-176.

## Context

1. **Any bump dims the bar.** The plugin refuses an index whose
   `contractVersion` is not its own (`Model.parseIndex`), the status
   becomes `contractMismatch`, the pill is dimmed and has no counts
   (`BarWidget.qml`), so no crisis colour shows. The desk explains it;
   the bar does not.
2. **Engine and plugin update on separate paths.** The engine through
   the install script (later pacman or the AUR), the plugin through
   `omarchy plugin update`. Mixed versions happen (E37). When 0.3 moves
   the index to contract 3, every machine with a 0.3 engine and a 0.2.0
   plugin silently loses ADR-0028's one visible signal until the user
   updates the plugin. That false calm is worse than a misread field.
3. **Most of what 0.3 adds is ignorable.** The queue for contract 3
   (crashes, reports, recipes, WP-145's and WP-149's fields) is new
   top-level sections, new optional fields, a new source, new kinds and
   a new timeline kind. The plugin already tolerates much of it: an
   unknown source gets the default glyph (`sourceGlyph`), an unknown
   timeline kind is filtered (`TIMELINE_KINDS`), unknown keys are not
   read. What it cannot know is whether a later index *changed* a field
   it keys on. Only the engine that writes the index knows that.
4. **The window is now.** A field the 0.2.0 plugin reads must ship in
   0.2.0. After the tag it needs a bump itself, and the plugins already
   released would never read it.

## Decision

### 1. The field

The index carries an optional integer **`contractReadableFrom`**,
`1 ≤ contractReadableFrom ≤ contractVersion`: *the oldest plugin
contract that can read this index without misreading a field it keys
on.* It covers `index.json` and the files it points to (the triage
proposal), as `contractVersion` does (AGENTS.md §3). Absent means
`contractVersion`, today's strict rule.

A reader **misreads** when a field it reads is absent, renamed or of
another type; when a value's meaning changed; when a closed set it keys
on (`state.status`, `cases[].status`, `drift[].resolution`,
`meta.risk`) holds a value it does not know; or when a count it reads
is **incomplete** — something the engine classifies as a crisis or as
open drift that is not counted where the reader counts it. An omission
is a misread of the count.

Engine 0.2.0 writes `contractReadableFrom: 2` on every index, the
`notInitialised` one included; `seldon index --check` validates it
(`schema/index.schema.json`: integer, minimum 1, maximum 2 while
`contractVersion` is 2). `seldon contract-version` is unchanged.

### 2. The plugin's check (CONTRACT.md rule 3)

A plugin of contract `P` reads an index of contract `V` that says `R`
(`R` = `V` when the field is absent or not an integer in `1 … V`) when

- `V = P`, as today; or
- `V > P` and `R ≤ P`.

Every other index is refused with the `contractMismatch` banner as
today: an older index (`V < P`) names the engine, a newer unreadable
one names the plugin. `manifest.json`'s `seldon.contractVersion` stays
the plugin's own `P`: it says what the plugin reads, not what it
tolerates.

A newer readable index is read as if it were the plugin's own contract:
the status is whatever the index says (`ok`, `indexStale`, …), the pill
has its counts and its crisis colour, and the desk shows one quiet
notice in the neutral tone: **"The engine writes index v`V`; this
plugin reads v`P` — update the plugin."** with *Update* (Omarchy's
`omarchy plugin update jax.seldon` in the presentation terminal: the
fixed argument list of the plugin's update script, the same as the
mismatch banner's) and *Copy*; not while the engine is missing (its
banner comes first, and the index is a leftover). The bar shows no extra mark: the pill's
job is the logbook's state, and the plugin still reads it correctly.

The preview's own check (`seldon preview --json`, ADR-0047) is another
schema and stays strict: it is read only before `init`, from the same
engine the plugin just probed, and a mismatch there costs a preview, not
the crisis signal. A 0.2.0 plugin under a later engine therefore shows
"The engine's preview could not be read." on the setup card; *Set up
Seldon* works (ADR-0047 §7).

### 3. What a later bump owes the released plugins

The engine may write `contractReadableFrom: R` with `R < V` only when the
ADR that bumps to `V` lists, **per added or changed field**, why a
plugin of contract `R` does not misread it. Allowed for a reader of `R`
are only:

- new keys — required or optional in `V` — and new sections the reader
  does not read;
- new values of an open set the reader already falls back on: a
  `source` (default glyph), a timeline `kind` (filtered), an event
  `kind` the reader shows as a plain row;
- nothing removed, renamed or changed in meaning among the fields the
  reader reads; `summary`'s counts keep their meaning and stay
  **complete**: every item the engine classifies as a crisis (ADR-0028)
  is counted in `summary.crisis` and listed in `drift[]` (crises first,
  rule 4), every open drift item in `summary.openDrift`; a crisis class
  carried anywhere else, or counted in a field of its own, keeps
  `R = V`. Nothing that is not a crisis is written where the reader
  counts or colours crises;
- the bounds of CONTRACT.md rules 4 and 5 (rows, texts, the 1 MB index)
  hold for the whole index: the reader parses it on the shell thread;
- the commands of CONTRACT.md's list that a plugin of contract `R` runs,
  with their argument lists and the `--json` it reads, keep working with
  the same meaning (E42): a reader in `ok` runs them.

Anything else lowers nothing: the engine writes `R = V`, and older
plugins show the mismatch banner as today. The bump's ADR decides `R`
once; the engine writes it as a constant beside `CONTRACT_VERSION`, and
a test holds the index to it.

The bump's ADR lists the fields as a table — field, change, where a
reader of `R` reads it (or "not read"), its fallback — and its PR runs
the released plugin of contract `R` (`Model.js` at its tag:
`parseIndex`, `counts`, `pillTone`, `deriveStatus`) against the new
`fixtures/index.sample.json`, asserting the same status, counts and
tone the new plugin computes. The table is the argument; the test is
the evidence.

A reader of contract 2 parses the triage proposal strictly (unknown keys
make the proposal unreadable, and the desk shows none). A later contract
that adds keys to the proposal file says in its ADR that a plugin of
contract 2 shows the index's `triage` head and counts but no items (the
file is unreadable to it, never misread) until the plugin updates.

### 4. What it does not do

- It does not make a newer plugin read an older index (`V < P`): that
  stays the mismatch banner, or the newer plugin's own choice to read
  both contracts (contract 3's WP decides that).
- It does not help a 0.1.x plugin: it knows no such field and reads
  only contract 1, so it shows the mismatch banner against any 0.2
  engine, as ADR-0035 §7 says.
- It is no promise of feature parity: a v2 plugin under a v3 engine
  shows only what v2 knows; the notice says so and offers the update.

## Consequences

- `schema/index.schema.json`: the optional field.
  `fixtures/index.sample.json` and its variants carry `2`; the reference
  derive (`scripts/validate-fixtures.py`) writes and checks it; a
  forward fixture (`fixtures/forward/index.contract-v3-readable.json`,
  the sample with `contractVersion: 3`, `contractReadableFrom: 2` and
  unknown keys, a source, kinds and a timeline kind) is the plugin's
  test input and the orchestrator's live check of the coloured bar; it
  must fail the v2 schema; `fixtures/invalid/` gains an index whose
  `contractReadableFrom` is above its `contractVersion`.
- Engine: `CONTRACT_READABLE_FROM = 2` beside `CONTRACT_VERSION`; the
  index writes it; the built-in schema validator learns `maximum`.
- Plugin: `parseIndex` accepts a newer readable index and reports
  `newer`; the notice `contractNewer` (Notices, `Service.fix`); tests for
  the tolerant reading (unknown keys, sources, kinds, timeline kind:
  status ok, counts equal, pill urgent) and for `R = 3` (mismatch).
- CONTRACT.md rule 3 reworded and rule 9 extended; SPEC-PLUGIN §5.6 the
  notice; CHANGELOG.
- The contract-3 ADR inherits §3 as an obligation; without it the 0.3
  engine writes `contractReadableFrom: 3`, and 0.2.0 plugins behave as
  they would have without this ADR.

## Alternatives considered

- *Keep the strict check, ship engine and plugin in lockstep.* They
  update on separate paths that Seldon does not control (E37). Rejected.
- *The plugin reads any newer index.* It cannot know what changed
  meaning; a misread crisis count is the worst failure the bar can
  have. Rejected: the writer decides, per field, in an ADR.
- *A list of the fields a reader must understand* (a "required
  features" set). More precise, but every plugin would need to know the
  names of features it predates; one integer says the same for a linear
  contract. Rejected for now; a later ADR can add it beside the integer.
- *Semantic versions for the contract (major.minor).* The same idea with
  a new type for a field the plugin already reads as an integer; it
  would itself be a bump. Rejected.
