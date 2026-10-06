# WP-118 — handover

Branch `wp/118-setup-texts`, base `main` 9458c8a. Engine Dev + Docs.
No contract change (`schema/`, `fixtures/` untouched).

## What was done

1. **Wizard (`engine/src/commands/init.rs`).**
   - Every prompt is a constant in `init::prompts`, each ≤ 60 columns,
     so none wraps in the presentation terminal (the wrap made dialoguer
     draw the theme-hook prompt twice, F5). Explanations are plain lines
     above a question: `THEME_HOOK_NOTE`, `BACKFILL_NOTE`, the baseline
     count line.
   - `BACKFILL_NOTE` no longer promises "the bar pill starts red, often
     with crises" (F7): older changes are mostly routine history (ADR-0028),
     the rest can be marked as the pre-Seldon baseline in one step.
   - Theme hook: note + "Record theme switches instantly? (installs an
     Omarchy hook)".
   - Agent items come from `harness_items(kit_present)`: "Claude Code hooks
     (user-wide)", "Seldon agent skill (into the agent skill folders that
     exist)", and "Omarchy-Agent kit (private template)" only when the kit
     directory exists (F2). The prompt reads "Agent setup".
   - Baseline: "The backfill opened N drift item(s) (M crisis): changes
     from before Seldon." / the reason line / "Mark them as the pre-Seldon
     baseline?".
   - Result: six aligned rows (Logbook, Config, Recording, Agents, History,
     Snapshots), then "Next steps:" only with what is left to do, else
     "Seldon is recording. Nothing else to do."; the optional snapshot
     grant last, with what it grants. Gone: machine id, file count, hook
     counts, fence count, ADR numbers, `seldon doctor` on a clean run.
     `--json` keeps every key, `nextSteps` holds only what is left, the new
     `optionalSteps` the snapper command(s). SPEC-ENGINE §9 and the
     `init --json` line describe it; docs/TESTING.md's wizard notes follow.
2. **`install.sh`.** Two announce lines before the download ("Installing
   the Seldon engine into ~/.local/bin as your user, no password; the
   download is checked against the release checksums before anything is
   written."). Next steps from two read-only tests: no `seldon init` when
   Seldon's `config.toml` exists, no plugin line when
   `~/.config/omarchy/plugins/jax.seldon` exists, "Your logbook is already
   set up; nothing else to do." when nothing is left (F3). Update/remove
   after a blank line, last.
3. **Docs.** Guide 01 en/de (install output sentence, step 2 table,
   backfill paragraph, the new result block); guide 10 en/de banner table
   (WP-117's titles and buttons); guide 11 en/de (what the installer says
   on an update, "update the plugin too", Omarchy's diff and
   `Update jax.seldon?`, the plugin 0.1.0 way out); CHANGELOG (standing
   paragraph at the top of `[Unreleased]`, an Engine and a Packaging entry);
   VERSIONING.md (the standing paragraph stays until the AUR package
   exists).
4. **Tests.** Unit (`init.rs`): prompt and note widths and pinned texts,
   no "red"/"crises" in the backfill note, the kit item hidden without the
   kit and shown with it, the summary with and without next steps.
   Integration (`tests/init.rs`, `doctor.rs`, `idempotency.rs`,
   `collectors_user.rs`): the rows, the optional and the recommended
   snapper block, `nextSteps` without `seldon doctor`, `optionalSteps`.
   `tests/install/install.test.sh`: the announce lines, next steps on a
   fresh home, with a config, with the plugin, with both, with the config
   under `XDG_CONFIG_HOME`.

## Decisions (the WP left them open)

- **No intro line in the wizard.** The review's §1b asked for "Seldon
  creates your logbook…" before the first prompt; WP-117's `INIT_SCRIPT`
  already prints that announce in the panel's terminal, and a user who
  types `seldon init` asked for it. Saying it twice is the noise the WP
  removes.
- **install.sh keeps its own announce** although WP-117's
  `INSTALL_ENGINE_SCRIPT` announces too: the guide's path (`bash
  install.sh`, the one-liner) has no wrapper. From the panel the two
  lines say the same in different words; see open questions.
- **The skipPaths hint stays, in the Config row** (F-250 asked that the
  first run says where noisy files go; a test pins it). The review's
  "move it to `seldon doctor`" would change doctor's output; not in this
  WP.
- **The snapshot grant is no next step** but an "Optional" block after
  the result: Seldon works without it (ADR-0026: printed as an optional
  step). It still says what it grants, shorter than `SNAPPER_FIX_GRANTS`.
  For a user in `ALLOW_USERS` the block is "Recommended: …" with the
  revert and the grant.
- **Next steps are only commands that change something:** `seldon capture
  --all`, `seldon dossier` (also after dossier warnings), `seldon drift`
  with open drift, `seldon doctor` for a degraded collector other than
  snapper or a failed git, `seldon hook install <harness>`, the theme
  hook's manual command.
- **`nextSteps` changes content** (no `seldon doctor`, no snapper fix;
  those are in `optionalSteps`). `init --json` is not the plugin contract
  (`index.json`); nothing in the repo reads it besides the tests.
- **The kit item is hidden whenever the directory is missing**, even when
  `config.toml` names `omarchy-agent`; without the kit that harness
  copies nothing anyway. `--harness omarchy-agent` still works and still
  says what it would copy.
- **Guide 01 recommends a backfill of about three months with the
  baseline**, no longer "leave it empty": under ADR-0028 a backfill is
  mostly routine, and ADR-0033 (accepted, implemented in WP-119) makes
  90 days the default. The wizard keeps the question in 0.1.4.
- **The Logbook row names the language by `Language::name()`**
  ("Deutsch", "English"), as the wizard's own language list does.
- **"Release notes template line":** there is no template; the release
  body is the CHANGELOG section (`packaging/release-notes.sh`). The
  standing paragraph sits at the top of `## [Unreleased]`; VERSIONING.md
  says to keep it there for every release until the AUR package exists.
- **`install.sh`, not `packaging/install.sh`:** the installer lives at
  the repository root; the WP's path was a slip.
- **The config test follows the engine:** `SELDON_CONFIG`, else an
  absolute `XDG_CONFIG_HOME`, else `~/.config`. The plugin folder is
  always under `$HOME/.config` (`omarchy-plugin-add` hard-codes it).

## What was not done

- **`plugin/README.md` §Troubleshooting** ("update the plugin first"):
  `plugin/` belongs to WP-117 in this round. WP-117 updated *States* and
  *Security* but has no Troubleshooting line for plugin 0.1.0 users. One
  sentence, to add in WP-117's review round or after both merge.
- ADR-0033 (90-day default, no backfill question): WP-119, as the WP says.

## How it was verified

- `flock /tmp/seldon-check.lock just check`: see the result line at the
  end of this file.
- Engine: `cargo test` 971 passed, 0 failed; clippy with and without
  `watch` clean; `cargo fmt` clean.
- `bash tests/install/install.test.sh`: 226 passed, 0 failed.
- The interactive wizard, driven through `script` in a 70-column pty with
  a scratch HOME/XDG and `SELDON_TEST_GUARD` (docs/TESTING.md recipe):
  every prompt on one line, the theme-hook prompt drawn once, the kit
  item absent without the kit, the result as above. The real `snapper`
  was only read (degraded: not readable).
- `docs-check`: ok (14 translated pages; the German source lines name
  2120ee8).
- **Not run here: shellcheck.** It is not installed on this machine (red
  zone: no pacman). `tests/install/install.test.sh` runs it when present,
  and CI installs it (`.github/workflows/ci.yml`), so CI checks
  `install.sh` and the test. `bash -n` passed locally.

## Open questions (for the orchestrator)

1. **Two announces from the panel's *Install*.** WP-117's terminal script
   says "Seldon: install the engine / Downloads seldon … Runs as your
   user, no password.", then install.sh says "Installing the Seldon engine
   into ~/.local/bin as your user, no password; …". Acceptable for 0.1.4
   in my view; WP-119's setup card could shorten the plugin's `what` to
   the title only. Same for WP-117's install `ok` line ("press Check
   again") and install.sh's next steps, which agree.
2. **Merge order.** Guide 10's banner table describes WP-117's labels
   (read from its branch at 654a363); both WPs touch `CHANGELOG.md`
   (different sections) and `docs/TESTING.md` (different paragraphs).
   Merge them together; if WP-117's labels change in review, guide 10
   follows.

## Gate result

`flock /tmp/seldon-check.lock just check` on 0087503 (+ this file):
exit 0, `check: ok` (fmt, clippy, tests, watch, packaging, install
226/0, deploy, schema, docs-check, plugin-validate, qmllint 29 files,
plugin-test). shellcheck was not installed here, so check-packaging and
check-install ran `bash -n` only; CI runs shellcheck.
