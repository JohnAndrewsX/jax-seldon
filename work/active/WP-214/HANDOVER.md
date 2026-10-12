# WP-214 — handover (0.3.0 release preparation)

Branch `wp/214-release-030` in `wt/WP-214`, from `next` d608611c (the
freeze). VERSIONING "Tag flow" steps 1–2 done; not merged. The live test
on the test host did **not** run: the deploy refuses this branch (see
"Blocked" below).

## What ships in 0.3.0, by WP

Everything merged into `next` up to d608611c:

| WP | What | User-visible | CHANGELOG |
|---|---|---|---|
| WP-200 | the engine cuts `index.json` under 1 MB (ADR-0053) | yes | Engine |
| WP-203 | every listed drift item keeps its event; the cut keeps a proposal's rows (ADR-0054) | yes | Engine ("proposed" dropped: E84 accepted it) |
| WP-196 | the skill's core in 120 lines, topic files | yes (agents) | Engine |
| WP-157 | redaction paths after WP-140 | yes | Engine |
| WP-202 | every desk click has a key path | yes | Plugin |
| WP-201 | a crisis inside a triage proposal by key | yes | Plugin |
| WP-204 | Work's headers "N / total" on a cut index | yes | Plugin (**added here**, no line before) |
| WP-206 | counts from the summary; an empty group keeps its header | yes | Plugin (**added here**, with WP-204) |
| WP-211 | Work's WIP line above the buttons in a list column | yes | Plugin (**added here**) |
| WP-194 | Omarchy assumptions ledger, `check-upstream [--gate]` | no (dev) | Docs |
| WP-158 | lock test robustness (doc comment in `lock.rs`, test only) | no | — |
| WP-205 | desk-view waits for its index; the full check delays the first index write | no (tests) | — |
| WP-207 | the budget test holds on every host | no (tests) | — |
| WP-210 | check-perf's synced-write rows on their own tmpfs | no (tests) | — |
| WP-212 | the real-home guard knows every engine state file | no (tests) | — |

**Breaking: none.** Checked per WP against VERSIONING's public
interface: no contract change (schema unchanged since v0.2.0, contract
2), no command, flag, field or config key removed or changed in meaning.
Two borderline points, judged compatible (E86 calls 0.3.0 a compatible
minor): WP-201 inserts the proposal's crises as key stops, so ↓ on a
shown proposal with crises reaches the first crisis before the list; and
WP-203 changes which 500 events `index.events` holds (ADR-0054, no
contract bump, E84). Reviewer: say if either belongs under Breaking.

**The `jax.seldon.panel` shim stays.** 0.2.0's CHANGELOG, ADR-0034 §7,
the plugin README, KEYBINDINGS, SPEC-PLUGIN, guide 06/11 and a BarWidget
comment said "removed in 0.3.0". Its removal is WP-188's, which E86 moves
to the contract-3 release. So 0.3.0 keeps it: a Highlight and a Plugin
entry say so, the docs now say "a later release removes it (announced in
the CHANGELOG)", SPEC-PLUGIN §8 names E86 and WP-188, and the
BarWidget.qml comment says "0.3.0 keeps it, WP-188 removes it" (comment
only). ADR-0034 is untouched (accepted ADRs are immutable).

## Done

1. **Versions** (step 1): `engine/Cargo.toml` and `Cargo.lock` (the
   `seldon` package), `plugin/manifest.json`, `PLUGIN_VERSION` in
   `plugin/Model.js`: 0.3.0. `packaging/plugin-version.sh
   plugin/manifest.json plugin/Model.js 0.3.0` → `plugin-version: 0.3.0`;
   `cargo metadata --locked` ok. SPEC-PLUGIN §1's manifest sample 0.3.0.
   **`engineMin` stays 0.2.0**: the plugin diff v0.2.0..d608611c adds no
   argv (no new `seldon` command or flag; `drift apply <id> --item` is in
   0.2.0) and reads no new index field (only `summary.queuedCases` /
   `activeCases`, required since contract 2); `seldon plan list --status`
   is shown as text, and 0.2.0 has it.
2. **CHANGELOG** (step 2): `## [0.3.0] - 2026-10-12` with the standing
   paragraph and eight Highlights (keys; the cut index; Work's whole
   counts; old open changes keep their rows; the shorter skill;
   redaction; either update order; the shim stays). `[Unreleased]` keeps
   the standing paragraph and an empty `### Highlights`. New Plugin
   entries for WP-204/206, WP-211 and the shim. Link references
   `[Unreleased]: …/compare/v0.3.0...HEAD`, `[0.3.0]: …/tag/v0.3.0`.
   `bash packaging/release-notes.sh 0.3.0` clean (below).
3. **Docs**:
   - `README.md`: the update line (0.2.x → 0.3.0 in either order), the
     project status for v0.3.0.
   - `plugin/README.md` (version-free, as VERSIONING step 6 asks): the
     shim rows, Work's row (whole counts), the states table (the engine
     keeps the index below 1 MB, an older engine may not).
   - Guide en: 01 (sample `seldon --version` and doctor lines 0.3.0), 03
     (Work: the WIP line's place; the cut index, both numbers, the empty
     group's line and command), 04 (the skill as core plus topic files),
     06 and 11 (the shim), 10 (Index unreadable), 11 (new section "What
     changed from 0.2.x"). Guide de: the same pages, source lines at
     e0df9cca (each was in step with its English page before).
   - `llms.txt`: an Agent docs entry for the skill (core and topic
     files).
   - `docs/KEYBINDINGS.md`, `docs/SPEC-PLUGIN.md` §8: the shim.
   - `docs/VERSIONING.md`: "WP-214 for 0.3.0" (was WP-188); the tag
     refusal of a non-`passed` record comes with WP-188, not "from 0.3";
     until then the operator's go quotes the checker. Neither file is on
     WP-213's list (work/queued, docs/PLAN.md).
4. **`packaging/acceptance/v0.3.0.json`**: a skeleton as WP-126's for
   0.2.0: status `partial`, scenarios `gate`, `b`, `keys`, `c`, `wip`,
   `restart` all `not run`, each limitation naming the refused deploy;
   `commit`, `date` and `omarchy` empty for the live stage. `bash
   packaging/acceptance-check.sh 0.3.0` refuses exactly those fields
   (below).

## Blocked: the live test

`SELDON_TEST_HOST=<the test host> just deploy-test-host --dry-run
--branch next <gates>/check-dev-214.log` on this branch:

```text
deploy-test-host: refused: not on next (on wp/214-release-030)
```

The script's next mode deploys only a checkout of `next` whose HEAD is
`origin/next`, after next's full check (WP-155). A branch that must not
be merged cannot pass that, by design. I stopped there and tried no
other form (no checkout of `next`, no push, no other deploy, no copy by
hand). So none of the live scenarios ran: not `gate` (WP-194's
`check-upstream --gate` from the deployed clone; there is no deployed
0.3.0 clone), not `b`, `keys`, `c` (WP-201's scenario c), `wip` (WP-211
at 2048×1152 logical, 50 %), `restart`. The record says so, scenario by
scenario.

What the live stage needs, once this branch is in `next` and `next` is
pushed: next's full check log, `deploy-test-host --branch next <log>`,
the commit from its summary into `commit`, then the scenarios, `date`,
`omarchy`, `engine` (`0.3.0+next.<short sha>`) and each scenario's
result and counts; `acceptance-check.sh 0.3.0` must then pass. The
version bump is in `engine/` and `plugin/`, so the deployed commit must
contain 4e72f96e..e0df9cca.

## How it was verified

| Check | Where | Result |
|---|---|---|
| `flock /tmp/seldon-check.lock … SELDON_FULL_CHECK=1 just check` on 89719cfa (the last commit before the record and this handover, which touch only `packaging/acceptance/` and `work/`) | headless (dev host; own `HOME`, private 0700 `XDG_RUNTIME_DIR` under the private gates folder, removed afterwards; `TMPDIR`, `JUST_TEMPDIR`, `CARGO_TARGET_DIR=gates/target-dev-214` on disk) | `check: ok`, `exit 0`; log `gates/check-dev-214.log` (first line `head 89719cfa…`): cargo tests all `ok`; check-packaging ok (release-notes, acceptance-check, plugin-version, workflow-pins, store-readme, omarchy-pin, no-network, mirror-image, audit-ignore, srcinfo); install 230/0; deploy-test-host 327/0; runtime-dir 45/0; check-upstream ok (17 files, notice mode, the dev host's tree); docs-check ok (523 links, 14 translated pages, 61 commands); plugin-validate ok; qmllint ok (53 files); model.test.js 221; plaintext.test.js ok; terminal-scripts 73/0; real-home-guard 71/0; service-states 367/0; desk-view 2452/0; bar-view 196/0; ipc-restart 44/0 (no leftover in the private runtime dir) |
| shellcheck (install.sh, deploy-test-host.sh and their tests, in the check) | dev host, the kit wrapper on `PATH` | ok |
| `bash packaging/release-notes.sh 0.3.0` | fixture | exit 0, body below |
| `bash packaging/plugin-version.sh plugin/manifest.json plugin/Model.js 0.3.0` | fixture | `plugin-version: 0.3.0` |
| `cargo metadata --locked` (Cargo.lock in step with 0.3.0) | fixture | ok |
| `bash packaging/acceptance-check.sh 0.3.0` | fixture | refused, exactly the four live-stage fields: `commit`, `date`, `omarchy.version`, `omarchy.channel` |
| deploy to the test host (`--dry-run --branch next`) | test host | not run: refused before any host contact (above) |
| live scenarios gate, b, keys, c, wip, restart | test host | not run |
| the release workflow's dry run on `next` | CI | not run (the orchestrator's step) |
| the dev host's desk | desktop | not run (E38: released builds only) |

Nothing touched `~/Seldon`, `~/.local/state/seldon`, `~/.config` or the
installed plugin on this machine; no sudo, pacman, systemd or shell
restart. No guard hook blocked anything.

## Release body (`bash packaging/release-notes.sh 0.3.0`)

```markdown
**Panel says `omarchy pkg aur add jax-seldon`?** That is plugin 0.1.0,
and the package does not exist yet. Update the plugin first:
`omarchy plugin update jax.seldon` (Omarchy shows the changes and asks
`Update jax.seldon?`; answer yes), then `omarchy-restart-shell`.

### Highlights

- Every button on the desk works by key now: the notices, the setup card, *Agent sorts*, the width and sidebar settings, and each crisis in an agent's proposal.
- A very large logbook no longer locks the desk out: the engine keeps `index.json` under 1 MB, leaves out the oldest rows first and still counts every case, change and crisis.
- On such a logbook Work's headers count every case ("QUEUED · 50 / 450"), and a group the desk shows none of names the command that lists it.
- Old open changes no longer go missing from the Changelog and *Needs you*: every change the index lists has its row.
- Your agents read less: the Seldon skill is a short core, and the details load when a task needs them; an unedited skill updates itself.
- More secrets stay out of your logbook: keys in base64, passwords in Python-style settings and mount options, pass phrases typed on the command line.
- Update the engine and the plugin in either order: the index format is the same as in 0.2.0.
- The old `jax.seldon.panel` command still forwards to the desk in this release; the changelog announces the release that removes it.

Every change in 0.3.0: [CHANGELOG.md](https://github.com/JohnAndrewsX/jax-seldon/blob/v0.3.0/CHANGELOG.md#030---2026-10-12)
```

## Open questions

1. **The deploy path for the live test** (above): deploy `next` after
   this branch is merged into it (the script's next mode as it stands),
   or another way you name. I tried no other form.
2. **CHANGELOG date** 2026-10-12; a later tag changes the heading and the
   release body's anchor (`release-notes.sh` reads both).
3. **Breaking**: the two borderline points above (WP-201's ↓, WP-203's
   events).
4. Scenario `wip` needs a 2048×1152 logical output on the test host's
   1920×1080 monitor (for example `hyprctl keyword monitor` with a mode
   and scale that give 2048×1152 logical, restored afterwards); that is a
   live-stage decision, not made here.

## Round 2 (review 1: SEND BACK, B1 B2 N1 N2)

Orchestrator decisions: B1 reword (WP-215 later), B2 reword (WP-208
stays out of 0.3.0), N1 and N2 as proposed; WP-201 and WP-203 stay out
of Breaking; the old CHANGELOG blank line stays.

| Finding | Fix | Commit |
|---|---|---|
| B1 "every button" by key | Highlight 1 "The desk's buttons work by key now: the notices, the setup card, *Agent sorts*, the width and sidebar settings, and each crisis in an agent's proposal."; Plugin heading "The desk's buttons have a key path."; README status names the buttons; guide 11 en/de: "except Work's *By agent* and *Import tasks…*, which still need a click" | 4f566f2c, de in the next |
| B2 the WP-208 fix implied | the WP-203 entry: "The desk's transaction detail, in this release as in 0.2.0, can list part of an old transaction without the 'partial' note (the plugin infers the note from the list's end); a later plugin update fixes it." | 4f566f2c |
| N1 "every case" | Highlights 2 and 3, the Plugin heading "Work counts every queued and active case on a cut index", README status, guide 11 en/de ("still counts every queued and active case, every open change and crisis") | 4f566f2c and the next |
| N2 SPEC-PLUGIN §8 wrap | rewrapped; the CHANGELOG blank line left as it was (orchestrator) | 4f566f2c |

Guide de 11's source line is 4f566f2c. The older "every button" lines in
guide 03 and SPEC-PLUGIN (from WP-202) are left for WP-215, as the
review proposes.

**Verified (fixture):** `bash packaging/release-notes.sh 0.3.0` exit 0
(the new Highlights 1–3 in the body); `scripts/docs-check.sh` →
"docs-check: ok (523 links, 14 translated pages, 61 commands, 704
command lines)". No full check this round (orchestrator); no file under
`engine/`, `plugin/`, `schema/` or `scripts/` changed since 89719cfa.

**Live test:** after the orchestrator merges, gates and pushes `next`,
from a checkout of `next` equal to `origin/next`; scenario `wip` with
the test host's 1920x1080 output set to scale 0.9375 (2048x1152
logical) and restored, both commands recorded here then.
