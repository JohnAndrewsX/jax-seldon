# WP-226 — handover (0.4.1 release preparation)

Branch `wp/226-release-041` in `wt/WP-226`, from local `next` 6a6672ef
(the freeze ea408a90 plus this WP's activation). VERSIONING "Tag flow"
steps 1–2 done; not pushed, not merged, not tagged.

## What ships in 0.4.1, by WP

Everything merged into `next` from v0.4.0 up to ea408a90:

| WP | What | User-visible | CHANGELOG |
|---|---|---|---|
| WP-191 | CI's `plugin-qml` job: qmllint, the four Quickshell harnesses and Omarchy's QML scan against the pinned Omarchy stable shell tree (ADR-0055); `packaging/omarchy-tree.sh`, the pin as stable's tree | no (CI) | Packaging and docs |
| WP-224 | Qt 6.12: `Commons.Color` in 41 QML files (Qt 6.12's own `Color` hid Omarchy's), two QML ids renamed; the channel family (`omarchy-dev`, `omarchy-settings-dev`, `linux-omarchy-<variant>` count as their stable names); a channel switch is one `omarchy-channel` item (ADR-0060); the Omarchy collector reads `omarchy-dev`; *Why loud?* for the switch; the assumption ledger's edge measurements; qmllint and harnesses on Qt 6.12 | yes | Engine (three), Plugin (two), Packaging and docs (two); Highlights 1–4 |
| WP-225 | `omarchy-written` (ADR-0059): Omarchy's language, default agent and dictation files routine by content; agents' Omarchy commands attribute the files they write; Antigravity's skill folder; pacman `--` fixture; a failed terminal script ends with status 1 (edge's "Failed" prompt); its ledger rows | yes | Engine (three), Plugin (one), Packaging and docs (with WP-224); Highlights 5–7 |
| — | `packaging: jax-seldon 0.4.0` (the AUR bump after the v0.4.0 tag, merged from `main`), orchestration and queue files | no | — |

**Breaking: none.** Checked per WP against VERSIONING's public
interface (CLI, `--json`, exit codes, logbook, `config.toml`, the
contract, the plugin's settings, IPC targets and keyboard map):

- No contract change: `schema/` and `CONTRACT_VERSION` unchanged since
  v0.4.0; `fixtures/` gains only a collector log
  (`fixtures/logs/pacman-pkg-add.log`); contract 2. The new rule ids
  `omarchy-channel` and `omarchy-written` fit ADR-0038 §1's open `rule`
  set.
- WP-224's family: `alwaysRed` keeps its key, syntax and default list;
  `omarchy` in the list now also covers `omarchy-dev` (ADR-0060 amends
  ADR-0028 §2, §4c). A named `omarchy-dev` install outside a case was
  attention and is now a crisis, exactly as `omarchy`'s. I read that as
  a fix that makes the edge twin behave as its stable name (the bar's
  count can rise on an edge host), not a changed meaning of the key.
  A channel switch was already a crisis before (the removal of
  `omarchy` by name). **Reviewer: say if this belongs under Breaking.**
- WP-225's `omarchy-written`: a new default-on routine rule; the
  `routine` key keeps its meaning (a list written without the id leaves
  the rule off, as for every rule). Files that were attention or (the
  uwsm session file) crisis are routine now when they hold exactly
  Omarchy's content. Quieter, never louder.
- WP-225's terminal exit status 1 after a failure: the script's own
  status inside Omarchy's wrapper, not a Seldon CLI exit code.
- WP-224's two QML ids are private; the public alias `form` keeps its
  name; no IPC target or key changed.
- WP-191: CI only.

## Done

1. **Versions** (step 1): `engine/Cargo.toml` and `engine/Cargo.lock`
   (the `seldon` package), `plugin/manifest.json`, `PLUGIN_VERSION` in
   `plugin/Model.js`: 0.4.1; SPEC-PLUGIN §1's manifest sample 0.4.1.
   `bash packaging/plugin-version.sh plugin/manifest.json plugin/Model.js
   0.4.1` → `plugin-version: 0.4.1`; `cargo metadata --locked` ok.
   **`engineMin` stays 0.2.0**: the plugin diff v0.4.0..ea408a90 adds no
   `Process`, no `argv` and no index field (a grep of the added lines
   finds none); the new `CRISIS_RULE_TEXTS["omarchy-channel"]` is a text
   looked up by a rule id an older engine never writes. The shim's
   "0.3.0 and 0.4.0 keep it" says 0.4.x now (SPEC-PLUGIN §8 and the
   `plugin/BarWidget.qml` comment; comment only).
2. **CHANGELOG** (step 2): `## [0.4.1] - 2026-10-15` with the standing
   paragraph and eight one-line Highlights. WP-224 and WP-225 had left
   their lines in `[Unreleased]`'s Highlights only (three of them wrapped
   over several lines, one with an ADR number); they are rewritten as
   one-line user points and the detail moved into new `### Engine`,
   `### Plugin` and `### Packaging and docs` parts, by WP. One
   correction against WP-225's line: crash mutes were routine
   (`toggle-flag`) in 0.4.0 already; the Highlight says they "stay
   routine" instead of "no longer ask you why". No security ids, no WP
   numbers in the Highlights. `[Unreleased]` keeps the standing
   paragraph and an empty `### Highlights`. Link references
   `[Unreleased]: …/compare/v0.4.1...HEAD`, `[0.4.1]:
   …/releases/tag/v0.4.1`. `bash packaging/release-notes.sh 0.4.1` clean
   (below). The date is the expected tag day (the tag is due by
   morning); a later tag changes the heading and the anchor.
3. **Docs**:
   - `README.md`: the update line (0.2.x, 0.3.x or 0.4.0 → 0.4.1 in
     either order) and the project status for v0.4.1.
   - Guide en: 01 (sample `seldon --version` and doctor's engine line
     0.4.1), 06 (Drift: the edge twins and the channel switch), 11 (new
     section "What changed from 0.4.0"). Guide de: the same pages, source
     lines at ce4e1c26. de 04 and de 05 already carried WP-225's
     Antigravity folder but kept their old source lines (docs-check
     warned); their source lines now name ce4e1c26 too.
   - `plugin/README.md`: nothing version-bound (version-free; WP-224/225
     changed no row there).
   - `llms.txt`: nothing version-bound; no new agent doc.
4. **`packaging/acceptance/v0.4.1.json`**: a skeleton, status
   `partial`; scenarios `gate`, `desk` (the theme's colours after
   `Commons.Color`, on the test host's Qt), `written` (a file Omarchy
   writes itself is routine), `edge` (not run: no edge host in the
   yellow zone) and `restart`; `commit`, `date` and `omarchy` empty for
   the live stage.

## How it was verified

| Check | Where | Result |
|---|---|---|
| `flock /tmp/seldon-check.lock env SELDON_FULL_CHECK=1 just check` on c00b0922 (the release commits; only the record skeleton and this handover follow) | headless (dev host, Omarchy edge, Qt 6.12; own `HOME`, private 0700 `XDG_RUNTIME_DIR` under the private gates folder, removed afterwards; `TMPDIR`, `JUST_TEMPDIR`, `CARGO_TARGET_DIR=gates/target-dev-226` on disk) | `check: ok`, `exit 0`, first run; log `gates/check-dev-226.log`, first line `head c00b0922216b756ef981bcb9f93fc0c5974368eb`. Cargo: 112 test results ok; check-packaging ok (release-notes, plugin-version 0.4.1, acceptance-check.test, workflow-pins, omarchy-pin, omarchy-tree, mirror-image, no-network 72 crates, audit-ignore, srcinfo, store-readme); install 229/0; deploy-test-host 326/0; runtime-dir 45/0; check-perf 14/0; assumptions.test ok; validate-fixtures ok; docs-check ok (533 links); plugin-validate ok; qmllint ok (53 files), tokens ok (1042 references); model.test.js 246; check-tokens.test 20/0; upstream-warnings.test 35/0; glyphs 4; terminal-scripts 78/0; real-home-guard 79/0; service-states 367/0; desk-view 2756/0; bar-view 196/0; ipc-restart 44/0 |
| `check-upstream` in that check | headless (dev host, edge tree) | exit 0 (notice mode): "10 of 25 files differ" — the same ten shell and validator files WP-224 left without an edge measurement (its open question); plugin-validate's notice that the installed validator is edge's, not the pin's. Not a gate on the dev host; the release gate is `--gate` on the test host |
| `bash packaging/release-notes.sh 0.4.1` | fixture | exit 0, body below |
| `bash packaging/plugin-version.sh plugin/manifest.json plugin/Model.js 0.4.1` | fixture | `plugin-version: 0.4.1` |
| `cargo metadata --locked` | fixture | ok |
| `bash scripts/docs-check.sh` | fixture | `docs-check: ok (533 links, 14 translated pages, 61 commands, 706 command lines)`, no warning |
| `bash packaging/acceptance-check.sh 0.4.1` | fixture | refused, exactly the four live-stage fields (below) |
| test host, read only: `omarchy version`, `omarchy version channel`, `seldon --version`, the plugin's manifest | test host | Omarchy 4.0.4-1, channel rc; engine `0.4.0+next.a088e38b`, plugin 0.4.0 (WP-221's deploy) |
| deploy, `check-upstream --gate`, the live scenarios | not run | waiting: origin/next does not carry the release commit (below) |
| the release workflow's dry run | not run | the orchestrator's step (CI) |
| desktop | not run | E38: the dev host runs released builds only |

Nothing touched `~/Seldon`, `~/.local/state/seldon`, `~/.config` or
the installed plugin on the dev host; no sudo, pacman, systemd or shell
restart anywhere. The test host was only read. No guard hook blocked
anything.

### `bash packaging/acceptance-check.sh 0.4.1` (exit 1)

```text
acceptance-check: v0.4.1 refused:
  - commit is ""; want the 40 lowercase hex characters of the commit deployed to the test host
  - date is ""; want a date YYYY-MM-DD
  - omarchy.version is missing or empty (`omarchy version`)
  - omarchy.channel is missing or empty (`omarchy version channel`)
```

## Waiting: the live stage

At 2026-10-14 ~21:50 UTC origin/next is 6a6672ef (the freeze plus
WP-226's activation), without this branch. The version bump changes
`engine/` and `plugin/`, so `acceptance-check.sh` accepts a record only
for a deployed commit that contains ce4e1c26; a deploy of ea408a90 or
6a6672ef could not give 0.4.1 a passing record. And `deploy-test-host`
deploys only a checkout whose HEAD is origin/next. So the live stage
needs this branch merged into `next` and `next` pushed (as for 0.4.0).
Then, in a fresh clone of origin/next in the private gates folder with
the two git-ignored host files: the full check there (log starting with
`head <full sha>`), `just deploy-test-host --branch next <log>` (dry run
first), `check-upstream --gate` on the test host from a bundle of
`next`, the scenarios, the record filled in and
`acceptance-check.sh 0.4.1`.

Planned scenarios on the test host (rc):

- `gate`: `OMARCHY_PATH=/usr/share/omarchy bash
  scripts/check-omarchy-assumptions.sh --gate` in the bundle's clone.
  On rc the edge-only rows read `absent` and must match.
- `desk`: the desk summoned, a shot, the view's colours against the
  theme's: `Commons.Color` on the rc host's Qt.
- `written`: `~/.config/omarchy/defaults/agent` set to another of
  `omarchy-default-agent`'s names by writing exactly Omarchy's content
  (`<agent>\n`), `seldon capture`, the event routine `omarchy-written`;
  then back to its value before, routine again; a name not in the list
  stays attention. Not by `omarchy default agent` itself: that installs
  the agent with mise and launches it. The mark is content equality
  (ADR-0059 §1), so the hand write tests the rule as shipped.
- `edge`: not run (no edge host in the yellow zone).
- `restart`: three `omarchy restart shell`, crash reports and
  quickshell cores before and after.

## Release body (`bash packaging/release-notes.sh 0.4.1`)

```markdown
**Panel says `omarchy pkg aur add jax-seldon`?** That is plugin 0.1.0,
and the package does not exist yet. Update the plugin first:
`omarchy plugin update jax.seldon` (Omarchy shows the changes and asks
`Update jax.seldon?`; answer yes), then `omarchy-restart-shell`.

### Highlights

- Seldon works on Omarchy edge (Qt 6.12) as on stable: the desk, the pill and the sheets take the theme's colours again.
- On Omarchy's edge channel, `omarchy-dev`, `omarchy-settings-dev` and Omarchy's other kernels (such as `linux-omarchy-bore`) count exactly like `omarchy`, `omarchy-settings` and `linux-omarchy`.
- Switching Omarchy's channel, stable to edge or back, is one change, "Omarchy changed channel", instead of a removal, an install and a lower version number.
- A channel switch outside a case is a crisis with the default crisis list, like any install of `omarchy` by name; plan it in a case to keep it quiet.
- Omarchy's own settings files are routine: setting Omarchy's language, your default agent or the dictation backend no longer asks you why, and crash mutes stay routine.
- A Seldon terminal step that failed ends with Omarchy's red "Failed" prompt instead of "Done!", on Omarchy versions that have it (edge today).
- The Seldon agent skill also goes into Antigravity's skill folder, `~/.gemini/config/skills`.
- Update the engine and the plugin in either order: the index format is the same as from 0.2.0 to 0.4.0.

Every change in 0.4.1: [CHANGELOG.md](https://github.com/JohnAndrewsX/jax-seldon/blob/v0.4.1/CHANGELOG.md#041---2026-10-15)
```

## Commits

- ce4e1c26 `release: 0.4.1 versions, CHANGELOG section and version-bound docs (WP-226)`
- c00b0922 `docs: German guide for 0.4.1 (01, 06, 11; source lines of 04, 05) (WP-226)`
- 86cb75b3 `release: 0.4.1 acceptance record skeleton, every scenario not run (WP-226)`
- this handover (`work/` only)

## Open questions

1. **Breaking**: the named `omarchy-dev` install, attention before,
   now a crisis (above).
2. **CHANGELOG date** 2026-10-15, the expected tag day; say if the tag
   moves.
3. **The deploy path**: merge this branch into `next` and push, then I
   run the live stage as above; or another way you name. I tried no
   other form.
4. `written` writes the test host's default-agent file by hand instead
   of running `omarchy default agent` (above). Say if you want the real
   command, with the agent install it brings.
