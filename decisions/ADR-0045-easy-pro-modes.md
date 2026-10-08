# ADR-0045 — EASY | PRO: one desk, two views; the mode is the plugin setting `deskMode`, the user's deliberate choice

**Status:** proposed (WP-167; operator decisions E33 and E34, 2026-10-08)
**Date:** 2026-10-08

> No contract change: `contractVersion` stays 2, no schema, fixture or
> engine change; no new row in CONTRACT.md. One desk setting and three
> state keys on the plugin's `shell.json` entry. Builds on ADR-0028 §3–§4
> (crises are told, attention is quiet, the bar counts crises), ADR-0034
> (the desk, settings in `barWidget`), ADR-0036 §1 (ids and fixed words in
> prompts, never logbook text), ADR-0040 and ADR-0044 §5 (the user's acts),
> AGENTS.md §2 (English UI, i18n later), §3, §8. Ships in **0.3**; the word
> table may ship in 0.2.x (§10).

## Context

Seldon is complete for a power user and opaque for a Linux beginner: nine
sections, case, drift, crisis, R3, triage. The operator wants both users
served by one product ("Omakase pur"), decided EASY | PRO in concept v2
(E33; the private report's §7 binding) and, after testing the prototype,
that a fresh install starts in EASY, an existing user stays in PRO with a
one-time "Try EASY", the PRO switch draws attention until its first click
(E34), and that the mode is changed only by the header switch or Settings
and then *stays* (prototype test 2026-10-08, item 32). Omarchy has no
simple/advanced mode; its pattern is Omakase defaults plus staged
disclosure, and its bar-settings panel renders `integer | enum | string |
path`, so the mode is an enum, not a boolean.

## Decision

### 1. Four rules (not negotiable; the acceptance tests of §9 prove them)

1. **Safety reads the same in both modes.** What PRO shows loud —
   `drift[].crisis`, a transaction that did not complete (`meta.txStatus`,
   ADR-0043, its "Before you reboot …" text verbatim), every notice of
   SPEC-PLUGIN §5.6 with its fix, an R3 case, a privileged command in a
   case log (ADR-0039) — EASY shows in the same tone, in plainer words,
   never later. The bar is the same widget in both modes (§8).
2. **Nothing in EASY is a dead end.** Every EASY item has *Details*, which
   opens inside EASY everything PRO knows about it (rule, source, zone,
   id, members, steps, log). PRO's tools are named in EASY's sidebar as a
   hint ("More tools: switch to PRO in the header"), never as a button.
3. **The switch loses nothing.** `selectedId` per section is shared, so the
   item stays selected (History ↔ Changelog, Help ↔ Work, Today ↔ Today);
   the switch disarms every armed action and closes an inline form (its
   text stays in the field while the desk is loaded); running engine calls
   run on. Nothing but the header switch and Settings › Mode changes the
   mode: not a link, not a payload, not *Details*, not a scenario.
4. **EASY knows no action PRO does not have.** Every EASY action is a PRO
   action id mapped to an existing row of CONTRACT.md ("Commands the
   plugin may run"), with fixed words and ids only (ADR-0036 §1); EASY
   never runs a privileged command and never builds a shell string
   (AGENTS.md §8, ADR-0031). There is no "undo with a snapshot" button: no
   engine verb exists for it.

### 2. The setting

| key | type | default | written by |
|---|---|---|---|
| `deskMode` | enum `easy` / `pro` | `easy` | the header switch `EASY \| PRO` (right, beside the gear) and Settings › Mode, in both modes |

Stored like `deskWidth`: in `barWidget.defaults` and `schema`, inline on
the plugin's `shell.json` entry, visible in Omarchy's bar settings; the
plugin writes it through the one settings path of SPEC-PLUGIN §5.5
(`updateEntryInline`, the whole entry, the desk's own entry only). A
switch is one write and one routine `config-change` event (ADR-0028
`routinePaths`); a value already stored is not written; without a bar
entry (`Service.entryKnown` false) it holds until the shell restarts, with
§5.5's sentence. No session mode, no "Remember" line (Alternatives).

### 3. Start rule (E34) and the first open

Three state keys live on the same entry, written **once, in one write**,
at the first open of a desk that knows `deskMode` (plugin 0.3) when the
entry has no `deskMode` key:

- **existing** user — an index file is present, or `seldon status --json`
  reports an initialised logbook (what the service already reads; the
  plugin reads nothing else and writes nothing to the logbook) →
  `deskMode: "pro"`, `deskIntro: "try-easy"`;
- **fresh** install — otherwise → `deskMode: "easy"`, `deskIntro: "pro-cue"`;
- both: `deskFirstOpen: "<YYYY-MM-DD>"`.

The decision is taken once and stored, so a logbook created later never
turns a new user into an existing one. `deskIntro` (enum `try-easy` /
`pro-cue` / `done`) and `deskFirstOpen` (string) are state, not
preferences: they are not in `barWidget.schema` (Omarchy's settings panel
does not show them), and they ride in the entry because the plugin writes
every key of the entry (§5.5; A1). Writing `deskMode` explicitly at the
first open is the one named exception to "defaults the user never set are
not written". Without a bar entry the decision holds in the service and
is taken again at the next shell restart.

### 4. The PRO cue and "Try EASY"

- **Fresh install (`pro-cue`):** the PRO segment of the switch carries a
  ring in `Color.accent` (the theme's accent; an outline plus the same
  colour as a soft glow at low alpha; no other colour, no hard-coded
  value). Motion is a slow opacity pulse in short bursts (about one
  second, then a pause of several seconds) and runs only while Hyprland
  reports `animations:enabled` (read once per desk open with the fixed
  argv `hyprctl -j getoption animations:enabled`; A3); otherwise, and
  wherever the plugin cannot read it, the ring is still — the still ring
  alone satisfies E34. The tooltip names what PRO is and the day the cue
  ends.
- **It ends for good** on the first switch to PRO (header or Settings) —
  the same write that stores `deskMode: "pro"` stores `deskIntro:
  "done"` — or 30 days after `deskFirstOpen` (operator, proto-7.1; A4),
  written at the next open. It never returns.
- **Existing user (`try-easy`):** once, under the switch, a popover "New:
  EASY, a plainer view of the same record. Switch with EASY | PRO here."
  with one button, *Got it*, which writes `deskIntro: "done"`; the
  popover points at the switch and does not switch (rule 3). Any switch
  ends it too.

### 5. What EASY shows

Three sections, from the same `Model` results as PRO (`Model.deskSections(mode)`,
`Model.text(key, mode)`, `Model.primaryAction(item, mode)`; one data path):

| EASY | Holds | PRO source |
|---|---|---|
| **Today** | "Needs you": the cards of §6, at most three shown, "+N more need you" opens the list (a cap on the display, never on the count); "What changed" today and yesterday in plain words, each line a link into History; "Note for later" (one line) | Today, Changelog, Drift |
| **History** | one timeline in plain words; a transaction is one row that unfolds; search "What happened with …?" | Changelog |
| **Help** | "Something is wrong" / "I want to change something" → a sentence → confirm → agent; running tasks, tasks waiting for your check, done tasks | Work, agent start |

Header: "N things need you" counts Today's cards (crises, stopped
transactions, open setup steps, notices with a fix, tasks waiting for a
check). PRO's KPI "crises" counts crises only. **The numbers differ on
purpose**; neither is corrected to match the other.

**Words.** One table in `Model.js` with two columns, both modes read it;
nothing scattered. Omarchy's words stay in both: snapshot, update, theme,
plugin, package, agent. Translated is Seldon's own jargon only: case →
task; open drift / attention → "a change nobody explained"; crisis →
"needs your attention now"; R3 / risk → "needs your OK"; verification →
"waiting for your check"; explain → "It was me"; event kinds → plain text
("package updated", "setting changed", "plugin added"). The case id stays
visible, small (the agent names it in its terminal).

**Keys in EASY:** `Esc`, `↑`/`↓`, `Enter` (opens; never starts an agent,
E6), `?` (the keys), `,` (Settings). Every other key is PRO's.
**Settings in EASY:** Mode, Desk width. "Agent" is Omarchy's setting: shown,
with the command `omarchy default agent`, never written by Seldon.

### 6. EASY card → PRO action → CONTRACT row (normative; every row tested)

| EASY card / action | PRO action | CONTRACT.md row |
|---|---|---|
| Set up Seldon — the same three-step card as PRO (WP-119, engine → logbook → snapshots optional; "N of 3 steps to go"; *Not now*) | the setup card, the §5.6 notices | the terminal scripts of §5.6 (`INSTALL_ENGINE_SCRIPT`, `INIT_SCRIPT`, `SNAPPER_FIX_SCRIPT`); no argv row |
| Engine missing (index present), index stale, contract, "Restart the shell", rules outdated | the same notices, same fixes | `capture --all --json --quiet`; `rules update --json`; `["omarchy-restart-shell"]`; §5.6 scripts |
| Pick your agent (the engine's refusal "no default agent") — a card in **both** modes, one button that opens Omarchy's terminal with `omarchy default agent` | new notice in PRO too (change 5) | a sixth fixed script constant; Seldon writes no Omarchy setting |
| Needs your attention now (crisis): primary *Ask agent to explain*; second *It was me…*, two steps, "This writes 'explained by you' into the record for good."; no *Later*, no dismiss | Ask agent; Explain… | `agent ask drift <id> --json`; `drift explain <id> --json -- "It was me."` (fixed words) |
| An update stopped halfway (ADR-0043), with "Before you reboot …" verbatim: primary *Ask agent to finish it*, confirm names the agent's powers | New case (Today) | `agent start --new --json -- "Finish the interrupted update (event <id>)"` (fixed words + id) |
| Your agent works in a terminal window and asks you there before anything that can break boot or login: *Show* | Focus | `agent focus <id> --json` |
| Task waiting for your check: primary *Done* (armed twice); *Not done yet* under Details | Complete; Back to active | `plan done <id> --json`; `plan start <id> --json` |
| Help › "I want to change something" / "Something is wrong": sentence → confirm ("Your agent may install, remove and change things on this machine and asks you first only before anything that can break boot or login.") → click, never Enter | New case → agent | `agent start --new --json -- <sentence>` (the user's own words, as PRO's intent field) |
| Note for later | the journal field | `log --json -- <text>` |
| A change nobody explained (History row; Details: *Ask agent*, *It was me*) | Ask agent; Explain… | `agent ask drift <id> --json`; `drift explain <id> --json -- "It was me."` |
| Details › Open in editor | Open in editor | `open <id> --editor --json` |
| Capture now (header) | Capture now | `capture --all --json --quiet` |

An imported case (ADR-0044) appears in Help only as "Open in PRO to
start"; no action. A later CONTRACT row (e.g. WP-149's `agent ask
symptom`) may take over "Something is wrong" by a SPEC-PLUGIN §5.8 edit;
rule 4 is kept either way. Rule 1 requires the stopped transaction in
PRO's "Needs you" as well (prototype item 26); ADR-0043's class stays
routine (A5).

### 7. What EASY never shows

Import tasks and the Start of an imported case; Dismiss, Discard, Drop,
"Agent sorts N open changes" and the triage proposal (`drift
apply|discard`); Accept and New decision; `plan new` by hand, Hand to
agent on a queued case, Reopen; Rebuild, Update impact; Open in editor
outside Details; the sections Decisions, System, Memory, Prime Radiant,
Graph and every 0.3 section (Reports, Crashes) — until a card for a new
kind is added to §6's table in its WP, which the safety test then covers.

### 8. Bar, `view`, payloads

The bar widget is **unchanged in both modes** (same pill, count, colour,
tooltip, clicks; `view pill` byte-identical). `view` gains `mode:
"easy" | "pro"`; in EASY `sections` lists `today`, `history`, `help` and
the section's own `view()` reports its EASY state. A payload `section`
naming a PRO section opens its EASY counterpart (`changelog` → History,
`work` → Help, anything else → Today) and `select` still selects; no
payload key switches the mode. The harness sets the mode through the
entry (`pushSettings`), as it sets `deskWidth`.

### 9. Acceptance: equal safety

No doubled suite (E29). PRO keeps its harness; EASY gets one scenario
file (about ten cases: the three sections; **safety equality** — the
fixture's crisis on a hook path, the transaction with `txStatus`, an R3
case, a case-log line with a privileged command, each visible in EASY in
the tone PRO uses and counted in EASY's header; switch round trip with
the selection kept and the arm disarmed; the first-open write for fresh
and for existing; the cue's end on the first click; the settings write;
the theme sweep in both modes; the pill identical in both) and
`model.test.js` for the projections: every EASY action id maps to a PRO
action id that maps to a row of CONTRACT.md; every word row has both
columns; `deskSections("easy")` names no section outside the three.

### 10. Out of scope, release

i18n stays out (F3, AGENTS.md §2): the word table is the seam
(`text(key, mode)`, later a language); an own decision after a beginner
test on the test host. No mini graph in EASY (F4). The mode ships in
**0.3** (0.2.0 is not tagged, WP-134/135/137 still move the sections).
Optional in 0.2.x: PRO shows the EASY words as subtitles from the same
table, and the "Pick your agent" notice — cheap, and the table lands
tested before EASY is built.

## Consequences

- One product, one data path, one word table: the engine, logbook,
  index, collectors, hooks and agents do not know the mode.
- Every switch is a `shell.json` write and a routine event; a user who
  flips often sees routine rows in History. Accepted: the choice is
  deliberate (operator) and the record stays honest.
- Three state keys in the entry that the settings panel does not show;
  `doctor` and the desk's Settings › Mode name the start decision so it is
  not invisible.
- SPEC-PLUGIN gains §5.8 (EASY), §1 (`deskMode`, the state keys), §5.3
  (EASY keys), §5.5 (Mode), §8 (`view.mode`, payload mapping); the manifest
  schema; `KEYBINDINGS.md`; user guide 01 becomes the EASY guide (en/de);
  CHANGELOG 0.3 names the start rule for existing users.
- Open: an engine form `agent start --new --about <eventId>` would keep
  case titles readable where EASY passes fixed words plus an id (item
  28); not needed for this ADR.

## Alternatives considered

- **Session switch plus "Open in PRO next time? [Remember]" (F1, concept
  v2).** Rejected after the operator's prototype test: the mode is a
  deliberate choice that stays; two values (session, start) and a line
  under the switch were a second thing to understand for the user EASY is
  for; E34's "first click" and "stay in PRO" fall out naturally from one
  persisted value.
- **"More" opens the same place in PRO** (rule 2 of concept v2, change
  10). Rejected: a click somewhere in the content would switch the mode
  (operator); *Details* inside EASY keeps rule 2 without it.
- **Manifest default `pro`; the setup card writes `easy` on a fresh
  `init`** (change 12's alternative). Rejected: the first-open rule of §3
  decides from what the plugin already reads and needs no engine hook.
- **A mode-dependent bar widget** (concept v2 §2). Rejected (change 6): the
  pill is already minimal; a second widget costs harness and reads as two
  products.
- **A boolean `easyMode`.** Rejected: Omarchy's bar settings render no
  boolean; an enum names both modes.
- **Plugin-private state file for the start decision.** Rejected: a
  Seldon-private mechanism beside Omarchy's plugin settings (AGENTS.md §1,
  Omarchy first); the entry already carries unknown keys.
