# WP-192 — Handover (part 1, 0.2.0)

Branch `wp/192-live-record` (from `next`), worktree `wt/WP-192`.
Part 2 (the tag refusal in `release.yml`) is WP-188's and was not touched.

## What was done

- **Format** — `docs/VERSIONING.md`, new section "Release acceptance
  record": `packaging/acceptance/vX.Y.Z.json` with `version`, `status`,
  `commit`, `date`, `omarchy` (`version`, `channel`), `engine`, `plugin`,
  `scenarios` (`id`, `title`, `where`, `result`, `counts`, `notes`) and
  `limitations`; an example record, a field table and the rules. The tag
  flow's step 3 now says: deploy the bumped commit, run the live
  scenarios, write and push the record, run the checker; the operator's
  go quotes its output. `docs/TESTING.md` ("Test host follows main")
  points to it. CHANGELOG `[Unreleased]` → Docs has one bullet.
- **Checker** — `packaging/acceptance-check.sh X.Y.Z [REF]` (bash + jq,
  no network). It reads the record **as committed in REF** (not the
  working tree), lists every problem at once, exit 0 ok / 1 refused:
  fields and types, unknown fields refused, the status rule, a `not run`
  scenario listed in `limitations`, `status: failed` refused, no private
  paths or host and user names, `commit` in the repository and an
  ancestor of REF, and the paths changed between `commit` and REF
  (`git diff-tree --no-renames`, so a rename shows both paths), each
  offending path printed. On success it prints a summary for the tag
  question.
- **Test** — `tests/release/acceptance-check.test.sh`, in
  `just check-packaging` (bash -n and, where installed, shellcheck too):
  100 cases against a scratch git repository under the checkout's
  ignored `target/` (git global/system config shut out). A good record,
  a partial record, then each broken field, each broken rule, the privacy
  checks, a code change after the commit (engine, plugin, workflow, an
  engine-compiled `.md`, a rename into `docs/`, a mode change), a commit
  that is not an ancestor, a commit not in the repository, a missing,
  malformed or doubled record. Each broken case is the good record with
  one edit and asserts its own message. Five mutants of the checker (no
  ancestor check, `.md` under `engine/` allowed, rename detection, status
  not derived, `failed` passes) are each caught.
- **Deploy** — `scripts/deploy-test-host.sh`'s summary has a new line
  `  commit   <40 hex> ("commit" in packaging/acceptance/vX.Y.Z.json)`
  for main and next builds (a `--release` deploy prints none); two new
  assertions in `tests/deploy/deploy-test-host.test.sh`.
- `release.yml` is unchanged, as the WP says.

## Decisions I took inside the WP (please confirm in review)

1. **`*.md` is allowed except under `engine/`.** The WP says "`docs/`,
   `*.md`, `work/` and `packaging/acceptance/`". Taken literally, that
   lets `engine/assets/skills/seldon/*.md` and `engine/templates/**/*.md`
   change after the live test. The engine compiles both into the binary
   (`include_str!` in `engine/src/commands/skills.rs` and
   `engine/src/logbook/rules.rs`): the shipped skill and the rules
   written into users' logbooks. That would contradict the WP's goal
   ("refuses a tag whose code changed"), so `.md` under `engine/` counts
   as code. `plugin/README.md` and `plugin/SECURITY.md` stay allowed: the
   shell does not load them. A mutant proves the rule. This is a release
   process rule, not user-visible behaviour, so I did not stop for it;
   it is one `case` line if the orchestrator wants the literal rule.
2. **A `partial` record passes (exit 0)** and prints its limitations.
   WP-126 expects the checker to pass on the 0.2.0 record even when
   WP-194's `--gate` is listed `not run`. A `failed` record is refused.
   WP-188 (part 2) must require `passed` on its own, as its section says;
   the checker's summary line `status   <status>: …` or a small flag can
   serve it.
3. **`counts`** = ADR-0027 §1: `humanSteps` and `passwordPrompts`
   required on every scenario that ran; `snapshotCoverage`,
   `r3GatesHonoured` (percent, or `null` without an R2/R3 case) and
   `agentClosesReopened` optional. A scenario `not run` has no `counts`,
   and `where` and `result` are both `not run`.
4. **A `not run` scenario is "listed in `limitations`"** when an entry
   starts with `<id>: `. A plain substring test would accept any text
   that happens to contain a short id such as `a`.
5. **`engine`** must be `X.Y.Z` or `X.Y.Z+main|next.<hex>` whose hex is a
   prefix of `commit`: an engine built from another commit is refused.
   `plugin` must be `X.Y.Z`.
6. **Privacy** (AGENTS.md §8) is checked best-effort: `/home/` and
   `/Users/`, this machine's host and user name (not `root`), and the
   names in the git-ignored `scripts/guard-hosts.local` and
   `scripts/deploy-hosts.local` when present. The test host's user name
   is not known to the dev host; the docs say to leave names out.

## How it was verified

| Check | Where |
|---|---|
| `bash tests/release/acceptance-check.test.sh`: 100 ok, 0 failed (incl. 5 mutants caught) | fixture (scratch git repository, dev host) |
| `bash tests/deploy/deploy-test-host.test.sh`: 326 passed, 0 failed, incl. the two new summary checks | fixture (fake host, dev host) |
| `just check-packaging`: ok | fixture (dev host) |
| The example record in VERSIONING.md, with a real commit put in, passes the checker | fixture (scratch repository) |
| `SELDON_FULL_CHECK=1 just check` on 4e23b232 (the last code commit; this handover commit is `work/` only): `check: ok`, `exit 0`, Quickshell harnesses run | fixture + headless (dev host; private on-disk XDG_RUNTIME_DIR 0700, TMPDIR and CARGO_TARGET_DIR on disk) |
| shellcheck of `packaging/acceptance-check.sh`, `tests/release/acceptance-check.test.sh` and the changed deploy files | not run (no local shellcheck), CI |
| The checker in CI (container as root, no host/user check for root) | not run, CI |
| A real record for 0.2.0 | not run — WP-126 writes it on the test host |

## Not done

- Part 2 (the tag refusal in `release.yml`, `workflow-pins.test.sh`):
  WP-188.
- `packaging/acceptance/v0.2.0.json`: WP-126.
- The deploy summary does not print `omarchy version` / channel of the
  host; the record writer reads them on the test host. Adding them would
  need a probe change on the host side; not asked by the WP.

## Open questions

- Decision 1 above (`.md` under `engine/` counts as code): confirm or
  ask for the literal rule.
- For WP-188: should the CI gate require `passed` only, or accept a
  `partial` record with the operator's recorded go?
