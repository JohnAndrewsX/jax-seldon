# WP-221 — handover (0.4.0 release preparation)

Branch `wp/221-release-040` in `wt/WP-221`, from local `next` 37159fe3
(the freeze f36d6fc1 plus this WP's file). VERSIONING "Tag flow" steps
1–2 done; not merged. The live test on the test host did **not** run:
the deploy refuses this branch (see "Blocked" below), as it refused
WP-214's for 0.3.0.

## What ships in 0.4.0, by WP

Everything merged into `next` from v0.3.0 up to f36d6fc1:

| WP | What | User-visible | CHANGELOG |
|---|---|---|---|
| WP-208 | the plugin marks a transaction partial by its own rows; a guess says so (ADR-0054 §3) | yes | Plugin; Highlight |
| WP-215 | Work's header buttons by key; every header row checked for overlap; the desk header's subline keeps clear of the KPI strip | yes | Plugin (three entries); Highlights |
| WP-216 | leftovers of the WP-205/210/212 reviews: check-perf, guard rows, service-states waits | no (tests) | — |
| WP-218 | `hook session-start` and `hook generic` skip the parser in an unrelated session | yes (agents) | Engine; Highlight |
| WP-219 | `hook session-stop` skips the parser too; the fast-path tests complete | yes (agents) | Engine; Highlight (with WP-218) |
| WP-167 | the word table `Model.text` and the rule-4 test; PRO labels unchanged | no (labels the same) | — |
| WP-178 | status words beside every stripe, data states for empty lists, *Ask agent to sort N changes*, two glyphs from the font | yes | Plugin; Highlights |
| WP-209 | redaction paths after WP-157; a too-deep substitution fails closed | yes | Engine; Highlights |

**Breaking: none.** Checked per WP against VERSIONING's public
interface (CLI, `--json`, exit codes, logbook, `config.toml`, the
contract, the plugin's settings, IPC targets and keyboard map):

- No contract change: `schema/`, `fixtures/` and `CONTRACT_VERSION`
  unchanged since v0.3.0; contract 2.
- WP-218/219: output and exit codes unchanged (differential tests).
- WP-209: masks more, never less; the line nested deeper than 16 was a
  crash before (the hook recorded nothing), now it is a masked record.
  A fix, not a change of meaning.
- WP-178's *Agent sorts N open changes* → *Ask agent to sort N changes*
  is a label: the action, `a` twice and Enter doing nothing there are
  unchanged; the IPC id `select ask` is the same. The Highlight says
  "a new label, the same action".
- WP-215 adds keys and the IPC id `select header`. One borderline point,
  judged compatible as WP-201's ↓ was for 0.3.0: in Work, ↑ on the first
  row now reaches the header's buttons first and the notices one ↑ later.
- WP-167: PRO reads its own column with the same strings; the table's
  `argv` are data for the rule-4 test, nothing runs them.
- WP-208: the transaction note's wording and when it shows; no field.

Reviewer: say if the WP-215 ↑ belongs under Breaking.

## Done

1. **Versions** (step 1): `engine/Cargo.toml` and `Cargo.lock` (the
   `seldon` package), `plugin/manifest.json`, `PLUGIN_VERSION` in
   `plugin/Model.js`: 0.4.0; SPEC-PLUGIN §1's manifest sample 0.4.0.
   `packaging/plugin-version.sh plugin/manifest.json plugin/Model.js
   0.4.0` → `plugin-version: 0.4.0`; `cargo metadata --locked` ok.
   **`engineMin` stays 0.2.0**: the plugin diff v0.3.0..f36d6fc1 adds no
   engine command or flag that runs (the new `argv` in WP-167's `WORDS`
   are never read at run time: no `.argv` access outside the table and
   its test), and reads no new index field (no schema change; WP-208's
   `members` is in the 0.2.0 schema).
2. **CHANGELOG** (step 2): `## [0.4.0] - 2026-10-13` with the standing
   paragraph and ten Highlights (the faster hooks; Work's header keys;
   the status words and empty lists; the relabel as a label; the
   partial transaction; the narrow header; the masked forms; the
   substitutions read; the depth limit of 16 in user words; either
   update order). No security ids, no WP numbers. `[Unreleased]` keeps
   the standing paragraph and an empty `### Highlights`. The WP-215
   Plugin entry says "it ships in 0.4.0, a minor release" (was "the
   next minor release"; the Highlight's sentence was dropped). Link
   references `[Unreleased]: …/compare/v0.4.0...HEAD`,
   `[0.4.0]: …/releases/tag/v0.4.0`. `bash packaging/release-notes.sh
   0.4.0` clean (below).
3. **0.4 → 0.5 for the contract-3 release** (E91): `work/queued/WP-184.md`
   (`engineMin` names 0.5.0) and `work/queued/WP-188.md` (three places,
   0.5.0 / `v0.5.0`). A grep of `docs/`, `work/queued/`, `README.md`,
   `llms.txt` and `plugin/README.md` found no other plan naming 0.4 for
   it (the other WPs say "the contract-3 release (a later minor,
   formerly 0.3)", no number). STATUS.md (E86/E87 lines) is the
   orchestrator's and untouched.
4. **Docs**:
   - `README.md`: the update line (0.2.x or 0.3.x → 0.4.0 in either
     order), the project status for v0.4.0.
   - `plugin/README.md`: nothing version-bound to change (it is
     version-free; WP-178/215 updated its rows already).
   - Guide en: 01 (sample `seldon --version` and doctor line 0.4.0), 04
     ("What a hook records": substitutions are commands of the line, the
     depth limit, `${…}` not read), 06 (redaction: triple quotes, key
     words with `key`, nmcli's `-f` before the object word), 11 (new
     section "What changed from 0.3.x"). Guide de: the same pages,
     source lines at 343d7ac3 (each was in step with its English page
     before).
   - `llms.txt`: nothing version-bound; no new agent doc in 0.4.0.
   - SPEC-PLUGIN §8 and the `plugin/BarWidget.qml` comment: the
     `jax.seldon.panel` shim stays in 0.4.0 too (E86, E91; comment only).
5. **`packaging/acceptance/v0.4.0.json`**: a skeleton as WP-214's for
   0.3.0: status `partial`; scenarios `gate` (WP-194), `status`
   (WP-178), `header` (WP-215), `redact` (WP-209: a privileged command
   with a secret inside `$(…)`, recorded masked), `restart` (WP-162), all
   `not run`, each limitation naming the refused deploy; `commit`,
   `date` and `omarchy` empty for the live stage.

## Blocked: the live test

`SELDON_TEST_HOST=<the test host> just deploy-test-host --dry-run
--branch next <gates>/check-dev-221-r2.log` on this branch (fe4ab148,
the green check's head):

```text
bash scripts/deploy-test-host.sh --dry-run --branch next <gates>/check-dev-221-r2.log
deploy-test-host: refused: not on next (on wp/221-release-040)
error: recipe `deploy-test-host` failed on line 411 with exit code 1
```

The script deploys only a checkout of `main` or `next` whose HEAD is its
origin, after that branch's full check (WP-155, line 177). It refused
before any host contact. I stopped there and tried no other form (no
checkout of `next`, no push, no `--branch main`, no copy by hand). So
none of the live scenarios ran, and neither did WP-194's
`check-upstream --gate` on the test host. The record says so,
scenario by scenario.

Also noted: this worktree has no `scripts/deploy-hosts.local` (the
git-ignored pin file); a real deploy from a checkout without it refuses
("no pinned machine-id"). WP-214's live stage ran from `wt/next`.

What the live stage needs, once this branch is in `next` and `next` is
pushed (as for 0.3.0): next's full check log, `deploy-test-host
--branch next <log>` from a checkout of `next` equal to `origin/next`,
the commit from its summary into `commit`, then the five scenarios,
`date`, `omarchy` (`omarchy version`, `omarchy version channel`),
`engine` (`0.4.0+next.<short sha>`) and each scenario's result and
counts; `acceptance-check.sh 0.4.0` must then pass. The version bump is
in `engine/` and `plugin/`, so the deployed commit must contain
343d7ac3.

## How it was verified

| Check | Where | Result |
|---|---|---|
| `flock /tmp/seldon-check.lock env SELDON_FULL_CHECK=1 just check` on fe4ab148 (run 1) | headless (dev host; own `HOME`, private 0700 `XDG_RUNTIME_DIR` under the private gates folder, removed afterwards; `TMPDIR`, `JUST_TEMPDIR`, `CARGO_TARGET_DIR=gates/target-dev-221` on disk) | **exit 1**, one failure only: desk-view's by-id leftover check on the real runtime dir: "new entries no process holds in /run/user/…/quickshell/by-id (338 before, 338 after): … ah6r1wwlumt"; desk-view 2755 passed, 1 failed. Everything else green. The known flake (a foreign `qs` from a user service); log `gates/check-dev-221.log` |
| the same, rerun once on fe4ab148 (run 2) | headless (same setup) | `check: ok`, `exit 0`; log `gates/check-dev-221-r2.log` (first line `head fe4ab148…`): cargo tests all `ok` (112 test results); check-packaging ok (shellcheck not installed: bash -n only; release-notes, acceptance-check, plugin-version 0.4.0, workflow-pins, store-readme, omarchy-pin, no-network, mirror-image, audit-ignore, srcinfo); install 229/0; deploy-test-host 326/0; runtime-dir 45/0; check-perf 14/0; check-upstream ok (17 files, notice mode, the dev host's tree); docs-check ok (529 links, 14 translated pages, 61 commands, 704 command lines); plugin-validate ok; qmllint ok (53 files); model.test.js 246; plaintext ok; glyphs 4; terminal-scripts 73/0; real-home-guard 79/0; service-states 367/0; desk-view 2756/0; bar-view 196/0; ipc-restart 44/0 (real by-id 338 before, 338 after; private 0/0) |
| `bash packaging/release-notes.sh 0.4.0` | fixture | exit 0, body below |
| `bash packaging/plugin-version.sh plugin/manifest.json plugin/Model.js 0.4.0` | fixture | `plugin-version: 0.4.0` |
| `cargo metadata --locked` (Cargo.lock in step with 0.4.0) | fixture | ok |
| `bash scripts/docs-check.sh` | fixture | `docs-check: ok (529 links, 14 translated pages, 61 commands, 704 command lines)` |
| `bash packaging/acceptance-check.sh 0.4.0` | fixture | refused, exactly the four live-stage fields (below) |
| deploy to the test host (`--dry-run --branch next`) | not run | refused before any host contact (above) |
| WP-194's `check-upstream --gate` on the test host | not run | no deployed 0.4.0 clone (above) |
| live scenarios gate, status, header, redact, restart | not run | — |
| the release workflow's dry run on `next` | not run | the orchestrator's step (CI) |
| the dev host's desk | not run | E38: released builds only |

Nothing touched `~/Seldon`, `~/.local/state/seldon`, `~/.config` or the
installed plugin on the dev host; no sudo, pacman, systemd, shell
restart or `omarchy plugin update` anywhere. The test host was not
contacted. No guard hook blocked anything.

### `bash packaging/acceptance-check.sh 0.4.0` (exit 1)

```text
acceptance-check: v0.4.0 refused:
  - commit is ""; want the 40 lowercase hex characters of the commit deployed to the test host
  - date is ""; want a date YYYY-MM-DD
  - omarchy.version is missing or empty (`omarchy version`)
  - omarchy.channel is missing or empty (`omarchy version channel`)
```

## Release body (`bash packaging/release-notes.sh 0.4.0`)

```markdown
**Panel says `omarchy pkg aur add jax-seldon`?** That is plugin 0.1.0,
and the package does not exist yet. Update the plugin first:
`omarchy plugin update jax.seldon` (Omarchy shows the changes and asks
`Update jax.seldon?`; answer yes), then `omarchy-restart-shell`.

### Highlights

- Your agents wait less: in a session that has nothing to do with Seldon, the hooks for session start, session end and other agents now return within a millisecond.
- Work's *By agent*, *Import tasks…* and *New case* work by key too: ↑ on the first row reaches them, Enter twice runs one.
- A coloured stripe in a desk list now always has its word beside it (crisis, attention, active, proposed, failing); an empty list says what belongs there, and a search without a match offers *Clear search*.
- The Changelog's *Agent sorts N open changes* is now *Ask agent to sort N changes*: a new label, the same action.
- A pacman transaction's detail says again when it shows only part of the transaction, also for an old open change of one listed package.
- On a narrow desk the line under the Seldon mark no longer runs under the counts beside it.
- More secrets stay out of your logbook: passwords in Python triple quotes, keys such as `secret_key` or `aws_secret_access_key`, a password file given as `/dev/fd/3` from `<(…)`.
- Commands inside `$(…)`, backticks and `<(…)` are read as commands of their line, so `x=$(sudo mkpasswd …)` is recorded with its password masked.
- A command line with `$(…)` nested more than 16 deep no longer crashes the hook: it is recorded masked, and Seldon stops reading that line there, so a command after it is not recorded.
- Update the engine and the plugin in either order: the index format is the same as in 0.2.0 and 0.3.0.

Every change in 0.4.0: [CHANGELOG.md](https://github.com/JohnAndrewsX/jax-seldon/blob/v0.4.0/CHANGELOG.md#040---2026-10-13)
```

## Commits

- 3fbdf608 `work: WP-221 active`
- 343d7ac3 `release: 0.4.0 versions, CHANGELOG section and version-bound docs (WP-221)`
- fe4ab148 `docs: German guide for 0.4.0 (01, 04, 06, 11) (WP-221)`
- the record skeleton, then this handover (`packaging/acceptance/` and
  `work/` only, after the checked commit fe4ab148)

## Open questions

1. **The deploy path for the live test** (above): deploy `next` after
   this branch is merged into it (the script's next mode, as for 0.3.0),
   or another way you name. I tried no other form.
2. **CHANGELOG date** 2026-10-13; a later tag changes the heading and
   the release body's anchor (`release-notes.sh` reads both).
3. **Breaking**: WP-215's ↑ in Work (above).
4. The guide's Changelog screenshot (`docs/images/desk-tokyo-night-changelog.png`,
   alt text in guide 03 en/de) still shows *Agent sorts 6 open changes*;
   a new shot needs a desk, left as it is.
5. Scenario `redact` on the test host: the privileged command with a
   secret must be run by an agent hook in a case (e.g. `x=$(sudo mkpasswd
   -m sha-512 <made-up secret>)` through `seldon hook claude-code` with a
   PreToolUse payload, so nothing is executed and no sudo is needed);
   the guard may block the literal `sudo` in it. That is a live-stage
   decision, not made here.
