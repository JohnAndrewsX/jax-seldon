# WP-213 — Handover (round 2)

Branch `wp/213-roadmap-renumber` from `next` `d608611c` (the 0.3.0
freeze). Docs only. Not merged.

## Round 2: what changed after review 1

Review 1 (Opus) sent it back; the orchestrator decided:

1. **WP-214 owns the `jax.seldon.panel` shim wording.** Reverted to
   `d608611c`: `docs/KEYBINDINGS.md`, `docs/SPEC-PLUGIN.md`,
   `docs/VERSIONING.md` (also its "WP-188 for 0.3.0" and "From 0.3" lines,
   so that the file is not edited by two branches), `docs/user/en/06`,
   `en/11`, `de/06`, `de/11` (with the de source lines) and
   `plugin/README.md`. WP-213 now shares no file with WP-214 (proof below).
   Left for WP-214 or a one-line follow-up after it merges:
   VERSIONING.md:117–118 "(WP-126 for 0.2.0, WP-188 for 0.3.0)" and
   VERSIONING.md:262 "From 0.3 the release workflow runs it on the tag"
   (both mean the contract-3 release, WP-188).
2. **Labels by VERSIONING, no "0.3 minor or patch release".** A queued
   package that adds a surface (a command, flag, field, config key,
   plugin surface) says **"after 0.3.0 (a minor release)"**; a pure fix,
   test or CI package says **"a 0.3.x patch"**; the former 0.3 group says
   **"the contract-3 release (a later minor, formerly 0.3)"**. "0.4.0"
   stays only where a text needs a number (versions, `engineMin`, the
   tag, the record file), each time with "the planned next minor".
3. **Every touched line over 100 columns rewrapped** (the paragraphs I
   touched are wrapped at 72; the long one-line headers of the study WPs
   too). `git diff d608611c HEAD | grep '^+' | awk 'length > 101'` is
   empty.
4. **Stale targets fixed:** WP-131, WP-142, WP-144 ("Ships in 0.2.0"),
   WP-112 ("0.1.5"), and STATUS.md's "Queued (next up)" section, now the
   release plan after E86.

Commits: `c2738a85` (WP active), `913a185a` and `3e418f33` (round 1),
`6094c6e0` (round 2), then this handover.

## Labels, file by file (against `d608611c`)

| File | Old label | New label |
|---|---|---|
| WP-112 | Later (0.1.5 or on the operator's word) | Later (after 0.3.0, a minor release, on the operator's word) |
| WP-131 | Ships in 0.2.0 on `next` | Parked (above); if revived, after 0.3.0 (a minor release) |
| WP-134 | Release: 0.2.y | after 0.3.0, a minor release (`[hooks] guard` is a new config key) |
| WP-142, WP-144 | Ships in 0.2.0 on `next` | Ships after 0.3.0 (a minor release), on `next` |
| WP-145 … WP-153 | Ships in 0.2.x | Ships after 0.3.0 (a minor release) |
| WP-145 | in the 0.2.y order after WP-179 | in the order after 0.3.0, after WP-179 |
| WP-147, WP-153 | recipes (0.3) | recipes of the contract-3 release |
| WP-149 | Release: 0.2.y order; desk row with 0.3 | after 0.3.0 (a minor release); with the contract-3 release |
| WP-163 | Ships in 0.2.x; (0.2.x) / export stays 0.3; Later, 0.3; 0.2.x minimum, WP-189 (0.3); Stays (0.2.x); Moves to WP-189 (0.3) | after 0.3.0 (a minor release); after 0.3.0 / the contract-3 release; Later, the contract-3 release; the minimum after 0.3.0, WP-189 (the contract-3 release) |
| WP-167 | title "subtitles in 0.2.x"; mode ships in 0.3; 0.2.x (optional); Keep and build (0.2.x) | after 0.3.0; the contract-3 release (a later minor, formerly 0.3); After 0.3.0 (a minor release; optional); after 0.3.0, a minor release with WP-178 |
| WP-169 | Ships in 0.2.x | Ships in a 0.3.x patch |
| WP-170 | 0.2.x/0.3; 0.2.y order | after 0.3.0 (a minor release) or the contract-3 release; after 0.3.0 (a minor release) |
| WP-178 | 0.2.x (plugin only) | After 0.3.0 (a minor release; plugin only) |
| WP-179, WP-182 | 0.2.y; 0.2.y order (182) | After 0.3.0, a minor release; the order after 0.3.0 |
| WP-180 | part 1 0.2.x, part 2 0.2.y (header, Do headings); 0.2.y order | part 1 after 0.3.0 (a minor release), part 2 a later minor release; the order after 0.3.0 |
| WP-181 | stage 1 0.2.x, stage 2 0.3 (header, Do headings); "the 0.3 sections" | stage 1 after 0.3.0 (a minor release), stage 2 the contract-3 release; the contract-3 release's sections |
| WP-183 | (b) 0.2.y | (b) a 0.3.x patch (harness only) |
| WP-184 | 0.3 (title, header, field list, pre-tag window, plugin); `engineMin` 0.3.0 | the contract-3 release (a later minor, formerly 0.3) / that release; `engineMin` names that release (0.4.0, the planned next minor) |
| WP-185, WP-186 | 0.3; E28 stages in 0.3 | the contract-3 release (a later minor, formerly 0.3) |
| WP-187 | stage (a) 0.2.x; (b), (c) 0.3 | stage (a) after 0.3.0 (a minor release, built in WP-167); (b), (c) the contract-3 release |
| WP-188 | title "0.3 integration"; 0.3 (header, lockstep, installer, index, dependencies, tag gate); 0.3.0 (goal, versions, `engineMin`, tag, record, Highlights); "A 0.2.x user" | "Contract-3 integration"; the contract-3 release / that release; 0.4.0, the planned next minor, where a number is needed (goal, versions, tag, record file); "A user of an earlier release" |
| WP-189 | 0.3 (title, header, E49, A12, export, "Later, 0.3"); 0.2.x minimum | the contract-3 release; WP-163's minimum after 0.3.0 |
| WP-191 | 0.2.y | A 0.3.x patch (CI only) |
| WP-192 | part 2 / tag refusal 0.3 (title, header, goal, Do heading) | contract 3 / the contract-3 release (a later minor, formerly 0.3) |
| WP-197 | 0.3 | The contract-3 release (a later minor, formerly 0.3) |
| WP-208 | 0.2.x, plugin only | A 0.3.x patch, plugin only |
| WP-209 | 0.2.x | A 0.3.x patch (redaction fixes) |
| WP-215 | 0.3.x (set by the orchestrator), plugin only | After 0.3.0 (a minor release: new keys), plugin only |
| WP-216 | 0.3.x (set by the orchestrator), tests only | A 0.3.x patch, tests only |
| docs/skills/omarchy-ux/SKILL.md:68 | Reports ADR, 0.3 | Reports ADR, contract 3 |
| STATUS.md header | `next` carries 0.2.x and 0.2.y work | `next` carries the 0.3.0 work (E86); the queue's release plan is below |
| STATUS.md "Queued (next up)" | the 0.1.4 / 0.2.0 plan | the plan after E86: 0.3.0 (WP-214), 0.3.x patches, after 0.3.0 a minor release, the contract-3 release, not scheduled |

All work-package paths are under `work/queued/`.

**Not changed, no label:** `docs/PLAN.md` names no release (milestones
M0–M4 only); `docs/CONTRACT.md` says "contract 3 (WP-184)" with no
release number; `CHANGELOG.md`'s `[Unreleased]` names no future release;
`README.md` and `llms.txt` name none (and are WP-214's).

## Grep in the edited scope: what remains, all historical or correct

`grep -nE '0\.2\.x|0\.2\.y|\b0\.3\b|0\.3\.x|\b0\.4\b|0\.4\.0|0\.1\.5' work/queued/*.md STATUS.md docs/skills/omarchy-ux/SKILL.md`
returns only:

- the new labels "after 0.3.0 (a minor release)", "a 0.3.x patch"
  (WP-169, 183, 191, 208, 209, 216) and "0.4.0, the planned next minor"
  (WP-184:39, WP-188:11, 33, 41, 79), and STATUS.md's new queue section;
- version literals that are not releases: Omarchy `4.0.4-1` (WP-033:19,
  21; STATUS.md:705), Quickshell 0.3.1 (STATUS.md:260);
- "prototype 0.3" / `prototype-0.3/`: the name and folder of the private
  prototype and its debate (WP-163:5 and the source lines of the
  prototype-debate WPs); "formerly 0.3" in the new labels;
- WP-114:23 "0.2.0 form (operator, 2026-10-06: no 0.1.5; …)": a dated
  section heading that records a decision, not a target;
- WP-214 (the 0.3.0 release WP), correct as is and not touched;
- `STATUS.md` lines 617–647, 723, 744–752, 765–766, 784 (lines moved by
  four against round 1): operator decision records (E10, E28, E31,
  E33/E34, E49, E66–E72, the study splits) and 797, 812 (the 2026-10-06
  "no 0.1.5" records) as decided; records are not rewritten.
  `STATUS.md:674–676` is E86 itself.

Historical 0.2.0 labels left as they are: WP-183 "(a) 0.2.0" and WP-192
"part 1 0.2.0" (done), WP-145:39–40 "if it makes the 0.2.0 tag, else
contract 3" (resolved by history: contract 3), WP-095 (superseded by
WP-124 in 0.2.0).

**Accepted ADRs that name 0.3 or 0.2.x (not edited):** ADR-0034 (261,
264: the shim for one minor release, removal in 0.3.0), ADR-0045 (12–13,
74, 177, 210–211, 228: EASY | PRO ships in 0.3, subtitles in 0.2.x),
ADR-0051 (22–23, 26, 169: contract 3 in 0.3), ADR-0053 (244: "Rejected
for 0.2.x"), ADR-0054 (137: "Rejected for 0.2.x"). No ADR is `proposed`
(only the template).

## How it was verified

| Check | Where | Result |
|---|---|---|
| `just docs-check` (TMPDIR and JUST_TEMPDIR under the private gates dir) at `6094c6e0` | *fixture* | `docs-check: ok (514 links, 14 translated pages, 61 commands, 697 command lines)`, exit 0, no warning |
| `git merge-tree --write-tree --name-only 6094c6e0 wp/214-release-030` (WP-214 at `89719cfa`) | *fixture* | exit 0, tree `c89f30f9`, no conflict |
| files changed since `d608611c` in both branches (`git diff --name-only`, `grep -Fxf`) | *fixture* | none shared (grep exit 1) |
| lines over 100 columns added since `d608611c` | *fixture* | none |
| the grep above | *fixture* | only the mentions listed |
| `just check` | *not run* (docs only, as the WP allows) | — |

## Open questions

1. **The contract-3 release's number.** By VERSIONING the first release
   after 0.3.0 that adds a surface is 0.4.0. If a minor of the "after
   0.3.0" group ships before contract 3, the contract-3 release is 0.5.0
   or later, and the "0.4.0, the planned next minor" texts in WP-184 and
   WP-188 change with it. The labels say "a later minor" so they hold
   either way.
2. **VERSIONING.md:117–118 and :262** still say "WP-188 for 0.3.0" and
   "From 0.3"; both mean the contract-3 release. Reverted here per
   decision 1; WP-214 or a follow-up after it merges.
3. **WP-114**: the queued file holds the 2026-10-09 live case
   (mirrorlist, Omarchy's channel switch) after its 0.2.0 form; STATUS.md
   lists it as not scheduled. The orchestrator may want it in the "after
   0.3.0" group with an ADR.
4. **ADR-0034 §7** (shim removal in 0.3.0) is accepted and not edited;
   whether E86 is enough of a record or a superseding line is needed is
   the orchestrator's (review 1, question 3).
