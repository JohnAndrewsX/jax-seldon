---
name: omarchy-ux
description: House-style checklist for Seldon's QML plugin (bar pill, desk, notices) on Omarchy 4. Read before any plugin work - building or reviewing a surface, its states, keys, copy or store-review boundaries.
---

# omarchy-ux — Seldon's plugin house style (checklist)

A checklist, not a rulebook: each item names the line that rules. An item marked *proposal* has
no rule yet; say so in the PR when you follow it. Applies to every agent, not one tool.

## 1. Precedence

1. [AGENTS.md](../../../AGENTS.md) — §6 zones, §7 plugin rules, §8 safety.
2. Operator decisions (STATUS.md) and accepted ADRs in [decisions/](../../../decisions/).
3. The SPECs; for the plugin [SPEC-PLUGIN](../../SPEC-PLUGIN.md) §5.3 (keys), §7 (colour,
   geometry, states, motion, glyphs). **SPEC-PLUGIN wins where it and this file differ.**
4. This checklist.
5. Generic design skills and other people's skill bundles: read as references, never installed.

Omarchy first (AGENTS.md §1): read `$OMARCHY_PATH/shell/Commons/`, `Ui/` and `plugins/`, and the
installed `omarchy` skill (`$OMARCHY_PATH/default/agents/skills/omarchy/`: `plugins.md`,
`theming.md`); never edit `/usr/share/omarchy` (that skill's `SKILL.md:55`). Omarchy skills not
installed today are cited only *when shipped*.

## 2. Surfaces

| Surface | Rule | Source |
|---|---|---|
| Bar pill | glyph + counts of what needs a human; dimmed, never hidden, while status ≠ ok | SPEC §4; `BarWidget.qml:45` |
| Desk | sidebar · list · detail, sticky action bar, stacks below 760 px | SPEC §5.1–5.2; ADR-0034 §1–2 |
| Panel (`KeyboardPanel`) | not a Seldon surface: the 0.1 popup is gone | ADR-0034 §7 |
| `ConfirmDialog` | not used; if ever, `selectedIndex: 0`, reset on each open (Omarchy's 1 confirms on Enter) | `Ui/ConfirmDialog.qml:11,32-34` |
| Notices | under the header, each with its one fix | SPEC §5.6; AGENTS.md §7 |

## 3. Colour, geometry, states (SPEC §7)

- [ ] Only `Color`'s five roles and surface groups; the desk's surface is `Color.popups.*`
      (`Commons/Color.qml:19-23,78-81`). No hex, no theme file read.
- [ ] No state by colour alone: glyph or word with it (WP-178); a selection fill gets a second cue
      (WP-177). Text tones derived for contrast: WP-177 writes the rule into SPEC §7.
- [ ] Rows and `qs.Ui` controls take `Style.cornerRadius` (`Ui/CursorSurface.qml:27`); only
      stripes, accent bars and heatmap cells are square (radius 0).
- [ ] Sizes from `Style.spacing.*` / `Style.space(N)`, type from `Style.font.*`
      (`Commons/Style.qml:234-260,327-334`); a control keeps its size in every state.
- [ ] Fills and borders from Omarchy's state tokens, never a literal alpha: pressed 0.22,
      selection 0.35, focus = hover (0.08 fill, 0.25 border) (`Commons/Style.qml:82-92`).
- [ ] Motion only Omarchy's: 140 ms `OutCubic`, 60–120 ms colour (`Ui/PopupCard.qml:160`).
      *Proposal:* no animation for key-driven movement; nothing repeats without end.
- [ ] Glyphs the `monospace` font covers, else `components/MaskIcon.qml`; ⏸ ↺ ↻ ⏰ are not in it.
- [ ] *Proposal:* at most three type sizes per view; groups set apart by space, not a border each.

## 4. States

- [ ] The four plugin states — engine missing, logbook not initialised, index missing, index
      stale — each render something useful with a one-click fix (AGENTS.md §7).
- [ ] Data states (empty, no match, loading, error) each say what and offer one action; never
      fake numbers (*proposal*; WP-178's `Model.emptyState`).
- [ ] Nothing is due: no read state, no "review needed", no nagging (ADR-0027 §1, §5).
- [ ] Never suggest Seldon can stop or limit an agent in its terminal (ADR-0027 §9).

## 5. Keys (SPEC §5.3)

- [ ] The selection is the cursor; a ring only on real `activeFocus`.
- [ ] Digits are the desk's (dispatched before `Section.textKey`); the registry is WP-181's.
- [ ] One meaning per letter (ADR-0034 §2); nothing on Super; a focused field keeps every key.
- [ ] Writing actions arm first; a held key never confirms (SPEC §5.3, WP-173).
- [ ] No undo key without a true inverse verb: `plan reopen` makes a *new* case, so it is not one;
      reports get `report reopen`, not `report undo` (*proposal*, Reports ADR, 0.3).
- [ ] Copy copies the id, never logbook text (ADR-0036 §1). Every hinted key is also a click.

## 6. Copy (*proposal* unless cited; WP-183b lints labels)

- [ ] Buttons: verb + object, ≤ 4 words ("Hand to agent"), never OK or Submit.
- [ ] Errors: what happened, why, what to do; no apology, no exclamation mark.
- [ ] UI labels English (AGENTS.md §2); external text shown verbatim, escaped (CONTRACT.md rule 6).

## 7. Verify without touching the live session (AGENTS.md §5, §6)

- [ ] `omarchy plugin validate plugin/`, `just qmllint`, `just plugin-test` (SPEC §9; tokens:
      `tests/plugin/check-tokens.py`).
- [ ] Look: `DESK_SHOTS=<dir> tests/plugin/desk-view.sh` renders three themes offscreen with a
      private HOME (`desk-view.sh:24-25,80-88`); composition first, then numbers.
- [ ] Every process gets its own HOME and a private 0700 `XDG_RUNTIME_DIR` (AGENTS.md §6). No
      `omarchy capture screenshot`, no `wtype`, no `omarchy-restart-shell` on the dev host; live
      checks only on the test host.
- [ ] *Proposal:* worst-case data too: long paths, 0 / 1 / 1,284 items, missing fields.
- [ ] Handover names where each check ran: fixture, headless, CI, test host, desktop or not run.

## 8. Store review boundaries

After T. Ballard, build-omarchy-plugins (MIT), and the marketplace's public reviews.
- [ ] `textFormat: Text.PlainText` at every sink of external text, shared child controls too
      (CONTRACT.md rule 6; [review](https://github.com/omacom/omarchy-plugin-marketplace/issues/3360#issuecomment-5464817134); WP-126 tests the whole plugin).
- [ ] Bounded reads: a size limit before parsing a file or process output, a deadline per process
      (ADR-0025's index budget; [review](https://github.com/omacom/omarchy-plugin-marketplace/issues/1667#issuecomment-5451236689); WP-126's index limit).
- [ ] Fixed argv only, `--` before free text (AGENTS.md §8; CONTRACT.md; [review](https://github.com/omacom/omarchy-plugin-marketplace/issues/6292#issuecomment-5645360550)).
- [ ] No download-to-shell in the plugin or its README ([review](https://github.com/omacom/omarchy-plugin-marketplace/issues/8407#issuecomment-5818821005); WP-042).
- [ ] Privilege wording true, in the scanner's form; never an untrue denial ([review](https://github.com/omacom/omarchy-plugin-marketplace/issues/6586#issuecomment-5648899465); WP-042).
- [ ] No agent files (`AGENTS.md`, `CLAUDE.md`) in the shipped `plugin/` tree ([review](https://github.com/omacom/omarchy-plugin-marketplace/issues/4744#issuecomment-5642689162)).
- [ ] A shortened preview never changes what an action does ([review](https://github.com/omacom/omarchy-plugin-marketplace/issues/8478#issuecomment-5819340291); ADR-0025).
