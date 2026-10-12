# WP-213 — Handover

Branch `wp/213-roadmap-renumber` from `next` `d608611c` (the 0.3.0
freeze). Docs only. Not merged.

## What was done

Every forward-looking release label after operator decision E86:

- what ships from `next` now → **0.3.0** (no queued file named it; the
  packages merged at the freeze are completed, WP-214 lists them);
- queued "0.2.x" and "0.2.y" packages → **0.3.x**; where the package adds
  a surface (a command, field, collector, plugin surface) the header says
  "a 0.3 minor or patch release", as the WP asks (see open question 1);
- the former "0.3" (contract 3, crashes, reports, recipes, graph layouts,
  EASY | PRO, the panel shim removal, the tag gate) → **0.4**, `0.3.0` →
  `0.4.0`, `v0.3.0` → `v0.4.0`.

Commits: `c2738a85` (WP moved to `work/active/`), `913a185a` (the
renumbering), then the de translations' source lines and this file.

| File | Old label | New label |
|---|---|---|
| work/queued/WP-134.md | Release: 0.2.y | 0.3.x |
| work/queued/WP-145.md | Ships in 0.2.x; "in the 0.2.y order" | 0.3.x (a 0.3 minor or patch release); 0.3.x order |
| work/queued/WP-146.md, WP-148.md, WP-150.md, WP-151.md, WP-152.md | Ships in 0.2.x | 0.3.x (a 0.3 minor or patch release) |
| work/queued/WP-147.md | Ships in 0.2.x; recipes (0.3) | 0.3.x (a 0.3 minor or patch release); 0.4 |
| work/queued/WP-149.md | Ships in 0.2.x; "0.2.y order …, desk row with 0.3" | 0.3.x (a 0.3 minor or patch release); 0.3.x order, 0.4 |
| work/queued/WP-153.md | Ships in 0.2.x; recipes in 0.3 | 0.3.x (a 0.3 minor or patch release); 0.4 |
| work/queued/WP-163.md | Ships in 0.2.x; (0.2.x) / stays 0.3; Later, 0.3; 0.2.x minimum, WP-189 (0.3); Stays (0.2.x); Moves to WP-189 (0.3) | 0.3.x (a 0.3 minor or patch release); 0.3.x / 0.4; 0.4; 0.3.x, 0.4; 0.3.x; 0.4 |
| work/queued/WP-167.md | title "subtitles in 0.2.x"; mode ships in 0.3; 0.2.x (optional); Keep and build (0.2.x) | 0.3.x; 0.4; 0.3.x; 0.3.x |
| work/queued/WP-169.md | Ships in 0.2.x | 0.3.x (a fix, no surface) |
| work/queued/WP-170.md | 0.2.x/0.3; 0.2.y order | 0.3.x/0.4; 0.3.x order |
| work/queued/WP-178.md | 0.2.x (plugin only) | 0.3.x |
| work/queued/WP-179.md, WP-182.md | 0.2.y; "in the 0.2.y order" (182) | 0.3.x, a 0.3 minor or patch release; 0.3.x order |
| work/queued/WP-180.md | part 1 0.2.x, part 2 0.2.y (header and both "Do" headings); 0.2.y order | 0.3.x and a later 0.3.x, each a 0.3 minor or patch release; 0.3.x order |
| work/queued/WP-181.md | stage 1 0.2.x, stage 2 0.3 (header and "Do" headings); "the 0.3 …" digits | 0.3.x (a 0.3 minor or patch release), 0.4; 0.4 |
| work/queued/WP-183.md | (b) 0.2.y | 0.3.x |
| work/queued/WP-184.md | 0.3 (title, header, pre-tag window, "every 0.3 field", Plugin 0.3); `engineMin` 0.3.0 | 0.4; 0.4.0 |
| work/queued/WP-185.md, WP-186.md | 0.3; E28 stages in 0.3 (185) | 0.4 |
| work/queued/WP-187.md | stage (a) 0.2.x; (b), (c) 0.3 | 0.3.x; 0.4 |
| work/queued/WP-188.md | 0.3 (title, header, lockstep, installer, index, dependencies, tag gate); 0.3.0 (goal, versions, record, highlights); `v0.3.0`, `v0.3.0.json`; "A 0.2.x user upgrades" | 0.4; 0.4.0; `v0.4.0`, `v0.4.0.json`; "A 0.3.x user upgrades" |
| work/queued/WP-189.md | 0.3 (title, header, E49 "0.3, not 0.2.x", A12's part, export, "Later, 0.3"); 0.2.x minimum | 0.4 ("0.4, not 0.3.x"); 0.3.x minimum |
| work/queued/WP-191.md | 0.2.y | 0.3.x |
| work/queued/WP-192.md | part 2 / tag refusal 0.3 (title, header, goal, Do heading) | 0.4 |
| work/queued/WP-197.md | 0.3 | 0.4 |
| work/queued/WP-208.md, WP-209.md | 0.2.x | 0.3.x |
| docs/VERSIONING.md | "WP-188 for 0.3.0" (Highlights); "From 0.3 the release workflow runs it on the tag" | "WP-214 for 0.3.0, WP-188 for 0.4.0"; "From 0.4" |
| docs/SPEC-PLUGIN.md §8 | shim "kept for one minor release (removed in 0.3.0 …)" | "kept until 0.4.0 (WP-188; the 0.2.0 CHANGELOG announced 0.3.0, before E86 …)" |
| docs/KEYBINDINGS.md | shim "forwarded to the desk until 0.3.0" | until 0.4.0 |
| docs/skills/omarchy-ux/SKILL.md | Reports ADR, 0.3 | 0.4 |
| docs/user/en/06-configuration.md, de/06 | shim forwards until 0.3.0 | until 0.4.0 |
| docs/user/en/11-update-and-uninstall.md, de/11 | shim until 0.3.0, which removes it | 0.4.0 |
| docs/user/de/06, de/11 (line 3) | `<!-- source: … @ e69d7954 / 488725cf -->` | `@ 913a185a` (the en change, so docs-check has no stale-translation warning) |
| plugin/README.md (lines 381, 552) | shim removed in / forwards until 0.3.0 | 0.4.0 |
| STATUS.md (header) | "`next` carries 0.2.x and 0.2.y work" | "`next` carries the 0.3.0 work (E86); queued packages are 0.3.x, the contract-3 release is 0.4" |

**Edits in WP-214's files** (each names a future release; one line each):
`docs/user/en/06-configuration.md`, `docs/user/de/06-configuration.md`,
`docs/user/en/11-update-and-uninstall.md`,
`docs/user/de/11-update-and-uninstall.md` (plus the de source lines),
`plugin/README.md` (two lines). Not touched: `CHANGELOG.md` (its
`[Unreleased]` names no future release), `README.md`, `llms.txt` (no
future release named).

**Not changed, no label:** `docs/PLAN.md` names no release (milestones
M0–M4 only); `docs/CONTRACT.md` says "contract 3 (WP-184)" without a
release number.

## Grep in the edited scope: what remains, all historical

`grep -rnE '0\.2\.x|0\.2\.y|\b0\.3\b|0\.3\.0' work/queued docs README.md plugin/README.md llms.txt STATUS.md`
plus the CHANGELOG's `[Unreleased]` returns only:

- `STATUS.md` lines 613–643, 719, 740–748, 761–762, 780: operator
  decision records (E10, E28, E31, E33/E34, E49, E66–E72, the study
  splits) as decided; records are not rewritten.
- `STATUS.md:670` E86 itself; `STATUS.md:10` the new header.
- "prototype 0.3" / `prototype-0.3/`: the name and folder of the private
  prototype and its debate (WP-163:5 and the source lines of WP-134,
  145, 149, 163, 167, 169, 170, 178–189).
- `work/queued/WP-214.md`: the 0.3.0 release WP, correct as is;
  `WP-215.md`, `WP-216.md` already said 0.3.x.
- `docs/VERSIONING.md:118`, `docs/SPEC-PLUGIN.md:1814`: the new texts
  name 0.3.0 as the release WP-214 prepares and as what the 0.2.0
  CHANGELOG announced.

Historical 0.2.0 labels left as they are: WP-183 "(a) 0.2.0" and WP-192
"part 1 0.2.0" (done), WP-145:39–40 "if it makes the 0.2.0 tag, else
contract 3" (resolved by history: contract 3), WP-095 (superseded in
0.2.0).

**Accepted ADRs that name 0.3 or 0.2.x (not edited):**
ADR-0034 (lines 261, 264: the shim for one minor release, removal in
0.3.0), ADR-0045 (12–13, 74, 177, 210–211, 228: EASY | PRO ships in 0.3,
subtitles in 0.2.x), ADR-0051 (22–23, 26, 169: contract 3 in 0.3),
ADR-0053 (244: "Rejected for 0.2.x"), ADR-0054 (137: "Rejected for
0.2.x"). No ADR is `proposed` (only the template).

## How it was verified

| Check | Where | Result |
|---|---|---|
| `just docs-check` (TMPDIR and JUST_TEMPDIR under the private gates dir) | *fixture* | `docs-check: ok (514 links, 14 translated pages, 61 commands, 697 command lines)`, no warning after the de source lines |
| the grep above | *fixture* | only the historical mentions listed |
| `just check` | *not run* (docs only, as the WP allows) | — |

## Open questions

1. **"A 0.3 minor or patch release" against VERSIONING.** Before 1.0 a
   compatible addition is a minor bump, so the first release after 0.3.0
   that adds a surface would be 0.4.0 by VERSIONING — the number E86
   gives the contract-3 release. The WP's wording is used; the operator
   should decide which holds: surface packages wait for 0.4, the
   contract-3 release takes whatever minor number is next, or additions
   ride in 0.3.y patches by exception.
2. **The panel shim outlives its announcement.** The 0.2.0 CHANGELOG
   (lines 148, 182) and the 0.2.0 plugin README promised removal in
   0.3.0; `next` still ships the shim (WP-188's job). The docs now say
   0.4.0. WP-214's 0.3.0 CHANGELOG should say so in one line (not
   Breaking). `plugin/BarWidget.qml:32` still says "removed in 0.3.0" in a
   code comment, not edited here (docs only): a one-line fix for WP-214 or
   WP-188.
3. **Conflicts with WP-214.** The one-line edits in `docs/user/` and
   `plugin/README.md`, and the de source lines (line 3), will conflict
   if WP-214 touches the same lines; take WP-214's source sha if it is
   later.
4. **Stale targets outside this WP's labels:** WP-131, WP-142 and WP-144
   still say "Ships in 0.2.0 on `next`" (queued, so they missed 0.2.0
   and 0.3.0); WP-112 says "0.1.5"; STATUS.md's "Queued (next up)"
   section still describes the 0.1.4/0.2.0 queue. Left for the
   orchestrator.
