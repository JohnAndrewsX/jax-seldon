```
WP-046 HANDOVER
Done: root README.md rewritten along the GitHub checklist; developer content moved to docs/DEVELOPMENT.md; plugin/README.md on the same skeleton inside the marketplace template; docs-check extended to both READMEs, plugin/SECURITY.md, docs/DEVELOPMENT.md and llms.txt (links, anchors, image size, subtree-split rule, public repository URLs, `seldon …` lines); CHANGELOG [Unreleased] line, docs/TESTING.md row, justfile comment, docs/user/README.md pointer
Not done: no new images (the round-2 mark is not delivered; placeholder comments mark the hero and favicon spots); no PR, no push (as briefed)
Verified by: `just check` exit 0 (see "Verification"); `just docs-check` → ok (376 links, 14 translated pages, 37 commands, 410 command lines); negative test of the new checks (8 seeded faults, 8 reported); subtree split simulated in a scratch clone; every external URL fetched (one expected 404, see below)
Learned: memory/pitfalls.md, section "WP-046"
Decisions needed: 3 small ones, below
Touched outside WP scope: CHANGELOG.md, docs/TESTING.md, justfile (comment only), docs/user/README.md (one sentence), memory/pitfalls.md
```

Branch `wp/046-readme`, worktree `wt/WP-046`, from `48096ea`. No PR, no push.

## Commits

| Commit | What |
|---|---|
| `97fefb5` | docs: developer reading order and layout move to docs/DEVELOPMENT.md |
| `70421d4` | docs: root README along GitHub best practice (+ CHANGELOG line) |
| `024b759` | docs: plugin README on the same skeleton |
| `f92b9d3` | tests: docs-check covers both READMEs, DEVELOPMENT.md and llms.txt (+ TESTING.md, justfile) |
| `bd277c4` | memory: WP-046 pitfalls |
| (this file) | work: WP-046 handover |

## What each file has now

**README.md** (root), in order: a placeholder comment for the mark /
favicon; title and one-liner; badges (CI workflow `ci.yml`, latest
release via shields.io, licence); a hero placeholder comment over
`plugin/preview.png` (panel + Prime Radiant, 153 KB); a one-paragraph
pitch; the name's origin in one sentence; a one-line TOC (the page is
longer than two screens); **Why**; **Features** (six bullets); **Quick
start** in three steps (engine, `seldon init` + `seldon doctor`,
`omarchy plugin add … --enable`); **A 60-second tour** (theme switch →
`capture` → `drift` → `drift explain` → `plan new/start` → `log` →
`drift link` → `verify/done` → panel and Prime Radiant → `git log`);
**Install options, update and removal** (options table, update, remove,
"Engine from the AUR"); **Documentation** table (user guide en/de,
getting started en/de, plugin README, agent guide, `llms.txt`, concept
and specs, decisions, contributing, development, changelog,
versioning); **Project status** (v0.1.0, pre-1.0 rules, next steps);
**Contributing**; **Security**; **Licence**; **Acknowledgements**.

**WP-044 install wording.** Kept word for word and once per file: the
bold "AUR package: coming soon …" sentence, the `install.sh` paragraph,
"Checked form — download, read, verify, run:" with its block, "One-liner
— …" with its block, the v0.1.0 paragraph, the options table, the
Update/Remove bullets, the AUR block. Two changes of place, not of
words: the v0.1.0 paragraph moved **above** the checked form into a
`> [!NOTE]`, with the runnable `main`-branch commands from
docs/user/en/01-getting-started.md added inside it. Today
`releases/latest/download/install.sh` is a 404 (v0.1.0 does not carry
it), so a newcomer reading top to bottom would hit the 404 first. The
`--unit` row now links `engine/systemd/README.md`. Same in the plugin
README: its step 1 wording is unchanged, the v0.1.0 sentence moved into
a NOTE above the forms with the same commands.

**Anchors kept.** `<a name="install"></a>` sits where "## Install" was,
so the published plugin repository's link `jax-seldon#install` (v0.1.0
front page) still lands on the install steps. In plugin/README.md every
heading that other files link to stays (`#install`, `#keys`,
`#the-pill`, `#configure`, `#states`, `#development`,
`#security-privacy-privileges`).

**docs/DEVELOPMENT.md**: what the repository is, the parts table (moved
from the README), the subtree-split rule, the reading order (moved), the
layout (AGENTS.md §4 plus the newer files), build and test (`just
check`, host-only steps, TESTING.md), how the team works (WPs,
worktrees, handover, contract first, memory), links to ORCHESTRATION,
PLAN, HERDR-SETUP, STATUS, TESTING, VERSIONING, packaging, KEYBINDINGS,
DESIGN-BRIEF, AGENT-GUIDE, STYLE.

**plugin/README.md**: mark and hero placeholder comments, one-liner,
badges, preview, pitch, six feature bullets that link into Usage,
compact TOC; then the marketplace sections unchanged in name and order
(Requirements, Install, Usage, Configure, Security…, Troubleshooting,
Remove, Development); Install step 1 now carries both engine paths
(GitHub now, AUR commands "once the package is live"), step 2 shows
`seldon init` as a block; a new **Documentation** table (user guide
en/de, getting started en/de, agent guide, `llms.txt`, plugin spec,
contract, changelog), all absolute monorepo URLs; "Project home,
contributing and licence" now names CONTRIBUTING and SECURITY. The old
"For AI agents" section is folded into Documentation (same sentences,
shortened).

## Subtree split

Simulated in the scratchpad, not in the repository: `git clone
--no-local` of this branch, `git subtree split --prefix=plugin`, a
worktree of the split commit. Its root holds `README.md`, `LICENSE`,
`SECURITY.md`, `preview.png`, manifest and QML; every relative link in
the split README (`LICENSE`, `preview.png`, `SECURITY.md`) resolves
there. Everything outside `plugin/` is an absolute
`github.com/JohnAndrewsX/jax-seldon/blob/main/…` URL. docs-check now
enforces this for every page under `plugin/`.

## docs-check extension

`scripts/docs-check.py` gains `FRONT_PAGES` (`README.md`,
`plugin/README.md`, `plugin/SECURITY.md`, `docs/DEVELOPMENT.md`,
`llms.txt`). On them, and on `docs/user/`:

- relative links, images and anchors resolve (as before for docs/user);
- every image has alt text and is at most 1 MB (new for all pages);
- a page under `plugin/` may only link or embed files inside `plugin/`
  by relative path;
- an absolute link into the public repositories
  (`github.com/JohnAndrewsX/jax-seldon[-plugin]` root, `blob|tree/main`,
  `raw.githubusercontent.com/…/main`, workflow and badge URLs) must name
  a file that exists here, and its anchor a heading there (this checks
  all of `llms.txt`'s anchored links too);
- every `seldon …` in their code spans and `sh` blocks is checked
  against `--help`.

Other URLs are not fetched (no network in `just check`).

Negative test (a copy of the tree in the scratchpad, `SELDON_BIN` set):
a missing anchor in an absolute link, a `../SECURITY.md` link from
`plugin/`, a missing file behind a `blob/main` URL, a wrong workflow in
the badge, a missing relative file, a missing anchor in DEVELOPMENT.md,
a bogus `seldon init` option, and a 1.1 MB `preview.png` were each
reported; exit 1.

## Verification

`just check` on the dev host at `f92b9d3` (host steps run, none
skipped), exit 0:

```
check-packaging: ok
validate-fixtures: ok — 109 instances (109 incl. 8 expected failures), …
docs-check: ok (376 links, 14 translated pages, 37 commands, 410 command lines)
plugin-validate: ok
tokens: ok (520 references)
qmllint: ok (28 files)
plugin-test: ok
check: ok
```

fmt-check, clippy, test, check-watch and check-install ran before these
and passed (the gate stops on the first failure). `bd277c4` only
touches memory/.

External URLs, fetched once with `curl -sL -o /dev/null -w '%{http_code}'`:
all 200 (CI badge and workflow, shields release and licence badges,
releases/latest, SHA256SUMS, security advisories, both repositories,
`raw…/main/install.sh`, omarchy.org, quickshell.org, hyprland.org,
keepachangelog.com, contributor-covenant.org, and the 11 `blob/main`
targets of the plugin README) except
`releases/latest/download/install.sh`: **404, expected** until v0.1.1;
both READMEs put the NOTE with the working path above it. The root
README anchor `#install-options-update-and-removal` and
`docs/DEVELOPMENT.md` exist on GitHub only after the merge (docs-check
checks them against this tree).

## Quality chain

1. **Newcomer read.** I read both READMEs top to bottom as someone who
   has never seen Seldon and installed nothing else. Findings fixed:
   the v0.1.0 path came after the forms that 404 today (moved up, made
   runnable); the tour linked the switched-back theme without a second
   `seldon capture` first (added); the plugin README said "install from
   the AUR instead" right before "install from one source only"
   (reworded); the agent-hook feature claimed hooks for Omarchy's agent,
   which has none of its own (now: Claude Code's hooks, or
   `seldon hook` for any other agent, per docs/user/en/04); an
   unverified "runs daily on the maintainer's machine" was removed.
   Install, logbook and plugin need no other file; the tour links
   Getting started only for the full output.
2. **Accuracy.** Every claim checked against the user guide (WP-045,
   written from real engine output), CHANGELOG, SECURITY.md, the shell
   README (`omarchy plugin add` asks before cloning) and the engine's
   `--help` via docs-check (410 command lines).
3. **Render check** (mentally, GitHub rules): tables have header and
   separator rows and no unescaped `|` in cells; the NOTE alert holds a
   fenced block with `>` on every line; badge links nest one image each;
   TOC anchors follow GitHub's slugs (docs-check verifies same-file
   anchors); no code span crosses a line break any more in either README
   or DEVELOPMENT.md (four pre-existing ones in plugin/README.md were
   rewrapped without word changes; they were also invisible to
   docs-check's `seldon` check).
4. **Humanizer** (blader/humanizer SKILL.md, read-only), all 26
   patterns over both READMEs and DEVELOPMENT.md:

| Pattern | Found | Action |
|---|---|---|
| §1 not X but Y | "the facts but not the reasons" (Why) | kept: it is the argument, not staging |
| §2 one-line closers | "No network, no account, no telemetry." | replaced by one plain fact: "Nothing leaves your machine." |
| §3 deep-sounding sayings | none (the old "Seldon makes the deviations visible" is gone) | — |
| §4 staged run-up | none | — |
| §5 arguing with no one | none | — |
| §6 forced triads | "never installs …, never changes …, never blocks …"; "follows your theme, runs … without a shell, never writes a file" | kept: three distinct, checkable facts each |
| §7 repeated openings | pitch: three of five sentences start "Seldon" | kept one of each kind; features rewritten so no two bullets open alike |
| §8 dashes as connector | two "—" in "Checked form — …" and "One-liner — …" | kept: WP-044 wording is fixed by the brief |
| §9 stacked qualifiers | none | — |
| §10 hyphenated pairs | "plain-Markdown", "append-only", "one-click" | kept: technical compounds before a noun |
| §11 passive / missing subjects | "is checked the same way" (WP-044 text) | kept (fixed wording); new text uses active voice |
| §12 AI words | none (no robust, seamless, leverage, landscape …) | — |
| §13 inflated significance | none | — |
| §14 vague association | none | — |
| §15 -ing riders | none | — |
| §16 sales language | none (no "powerful", "beautiful", "seamless") | — |
| §17 borrowed authority | none | — |
| §18 avoiding is/has | "Features" is a heading, not a verb; none in prose | — |
| §19 bold as decoration | bold-label feature bullets in both READMEs; bold "v0.1.0" and "Breaking" in status | removed; bold left only in the fixed WP-044 sentence, its Update/Remove labels and the plugin README's existing step and usage labels |
| §20 decorative headings | none: sentence case, no emoji, no rules | — |
| §21 curly quotes | none (straight quotes in new text) | — |
| §22 chatbot residue | none | — |
| §23 knowledge-limit guesses | the "runs daily" claim | removed |
| §24 heading repeated in first sentence | none | — |
| §25 writing about the document | none beyond link labels | — |
| §26 re-explaining | none | — |

## Decisions needed

1. **Flip list (ADR-0024, work/completed/WP-044/HANDOVER.md §3) moved
   places.** When the AUR goes live: the bold sentence is now under
   README "Quick start → 1. Install the engine"; "Engine from the AUR"
   is now under "Install options, update and removal" and should move
   into step 1 above the GitHub forms; in plugin/README.md step 1's first
   sentence and the "Once the AUR package is live" paragraph flip. The
   NOTE blocks go with v0.1.1 in any case. I did not edit the completed
   WP-044 handover; please carry this into the flip WP.
2. **Hero and mark.** Placeholder comments only. When round 2 lands in
   `assets/`: README hero = A6 (1280×640); the mark next to the title =
   A1/A9. For the plugin README any image must be **copied into
   `plugin/`** (subtree split), and preview.png stays the marketplace
   preview unless A8 replaces it. Images ≤ 1 MB is now enforced.
3. **llms.txt in FRONT_PAGES.** It is WP-047's file; I only added it to
   the check (it passes unchanged). Say if you would rather keep it out.

## Not done / notes

- No PR, no push. The repo's description on GitHub still points the
  homepage at `tree/main/docs`; `docs/DEVELOPMENT.md` or the user guide
  might be a better target (operator's setting, not changed).
- The plugin README is long (marketplace manual); a TOC line under the
  feature bullets covers it. No content in Usage/Configure/Security was
  changed except the four rewrapped code spans.
