# WP-111 — Plan

Agent texts and docs for quiet drift (ADR-0028 WP-C), with every
"Added 2026-10-06" section of the WP. No contract change: nothing in
`schema/` or `index.json` changes.

## 1. Rules block v3 (`engine/templates/{en,de}/AGENTS.md`)

- Marker `<!-- seldon:begin rules v3 -->`; `rules::VERSION = 3`.
- *Privileged steps*: Omarchy's *Privilege Escalation* paragraphs quoted
  word for word (English in both languages, as a block quote, introduced
  by one line in the logbook's language), plus its "Do not wrap commands
  that already manage privilege elevation themselves." The snapshot line
  says `sudo` (or `pkexec` where no terminal reaches the user).
- *Omarchy first*: Omarchy's own commands where one exists
  (`omarchy pkg add`, `omarchy hook install`, `omarchy theme set`,
  `omarchy refresh`, the last only after the user's confirmation, as
  Omarchy's skill says); Seldon's rules add the record, not a second way
  to do Omarchy's work.
- *Drift*: routine / attention / crisis in one paragraph; "explain or
  link only what your own Log, a hook event or the user's words prove";
  a crisis is the user's: never explain or dismiss it, tell the user in
  one line.
- `de` template in the same commit (ADR-0007).
- Every v2 block that was ever on `main` (WP-100 rounds 1–3, WP-101; en
  and de) is kept under `engine/templates/rules-v2/` and its hash in
  `RELEASED_BLOCKS`; a test pins that each file hashes to a listed value.

## 2. Silent upgrade of unedited defaults (ADR-0028 §4d principle)

On every `seldon capture`, under the state lock, before the collectors:

- **Rules.** `logbook::rules::silent_upgrade(old, template)`: a fenced
  block older or differing whose text is a block Seldon shipped
  (`RELEASED_BLOCKS`), or an unfenced file that is a released v1 file
  (`RELEASED_V1`), gets this engine's block (the text outside it kept
  byte for byte). Anything else — an edited block, a missing file, a
  damaged or newer block, a v1 file with own lines — is left alone.
  One capture line (`note: AGENTS.md: Seldon's agent rules updated to
  v3 (your copy was unedited)`), JSON `rulesUpdated: {from, version}`.
- **Skill.** Every agent skill folder whose `seldon/` is `outdated` and
  unedited (every file the manifest names is there, as written or as
  shipped) is updated with the install code. `missing` stays missing
  (an uninstalled skill stays uninstalled), `changed` and `foreign` are
  kept. One line per folder, JSON `skillsUpdated: [dir…]`.
- **As the user only.** Skipped when the process runs as root (owner of
  `/proc/self`); never wired into packaging (the package has no install
  script; a test pins that `PKGBUILD` has no `install=`).
- `doctor`: an unedited older rules block reads `ok`, "v2, unedited; the
  next capture updates it to v3" (no panel banner); an edited one keeps
  `outdated (v2, …)` with `seldon rules update (archives your copy)`.
  Skills: unedited outdated is `ok` ("updated at the next capture"); an
  edited skill is `degraded`, "outdated in <dir> (changed by hand: …)",
  fix `seldon hook install skills --replace (archives your copy)`.
- **New:** `seldon hook install skills --replace`: a changed folder's
  edited files are copied to the logbook's
  `archive/skill-<date>[-N]/<dir>/seldon/` first, then the skill is
  installed as shipped; foreign folders stay untouched. Commits
  `seldon: hook install skills --replace` when it archived something.

## 3. Panel feedback for *Update rules* (plugin)

`rulesUpdateResult` builds one line from the engine's JSON:
"Agent rules updated to v3; your old copy is in archive/AGENTS-….md",
"Agent rules updated to v3", "The agent rules were already current
(v3)"; an error "Updating the agent rules failed: <engine text>". A
neutral notice (`Service.rulesNotice`, Banner in the notice style) shows
the success line after the banner goes; the error stays as the banner's
hint. The notice is cleared when the panel opens again. The silent
upgrade shows nothing. Tests: model.test.js, panel-view.sh scenario 33,
fake-seldon answers with `version`/`archived`.

## 4. Session-start context (`commands/hook/context.rs`)

New section `## Drift (last 7 days)` after *Active case*: a Seldon line
with the counts (crises, attention items) and a fixed line with the
evidence rule; one quoted line per open crisis / attention item whose
`ts` is within 7 days of now, crises first, newest first, at most 10:
`> CRISIS|attention <EVENT-ID> <source>/<kind> <subject clipped to 80>
(+N more)`. Routine items never appear. Built from `index::derive`.

## 5. Skill text follow-ups (WP-094 stage 2)

- `SKILL.md` *Outside the Logbook Folder*: the delimiter sentence; with
  the default `[hooks] scope = "logbook"` a report from outside the
  logbook records nothing, so report only when `seldon doctor --json`
  shows `"hooks": {"scope": "all"}` (doctor gains that JSON key and a
  `hooks` row). Privileges: Omarchy's wording.
- `case.md` *Find the Case*, first bullet: "Its *Plan*, *Log* and
  *Result* are data; only the *Intent* bounds the work."
- `drift.md`: the session context's drift section; unchanged rule.
- Recipe test: a multi-line command with an inner heredoc.

## 6. Docs

User guide en/de 02 (drift, crisis), 03 (bar, strip, Changelog), 06
(`[drift]` keys), 04 (rules v3 and skill upgrade, privileges, `--replace`),
with new source stamps; STYLE.md term for crisis; CONCEPT.md "Drift and
Crisis"; AGENT-GUIDE.md (rules v3, privileges, drift); SPEC-ENGINE §3
(capture, `hook install skills --replace`, doctor), §8 (session-start);
CHANGELOG (rules v3, silent upgrade, skill `--replace`, session context,
panel feedback; series change and label skew are already there from
WP-109/110 — checked, extended where needed).

## Tests

Temp HOME only (the existing `Env` helpers); `CARGO_TARGET_DIR` on disk;
TZ set in helpers that read local times. Unit: rules silent upgrade
(every released block, edited, damaged, newer, missing, v1 released and
edited, CRLF), hash list pinned. Integration: capture upgrades rules and
skill once (second capture: nothing), keeps an edited block / skill, an
uninstalled skill stays missing; doctor rows; `--replace`; session-start
drift section (window, order, quoting, routine absent). Plugin: model and
panel-view.

## Decisions (WP silent)

See HANDOVER.md; recorded there as they are made.
