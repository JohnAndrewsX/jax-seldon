# WP-042 — Handover

Branch `wp/042-store-prep` from `next` (`9d776f9a`, WP-042 queued). Not
pushed (the orchestrator pushes). Nothing filed in the store, nobody
contacted.

## Store commit read

`omacom/omarchy-plugin-marketplace` @
`5d22a9fbee57f7b22652e09df128ddf12f14a65b` (2026-10-09 19:20 UTC),
shallow read-only clone in the private `gates/tmp-042/store/`. Compared
with the study's clone at `4da3ed3e665c7376da83130445ba908865a68f7a`:
`SUBMISSION.md`, `SECURITY.md`, `VERIFICATION.md`, `README.md`,
`AGENTS.md` and all of `scripts/` are byte-identical; only `registry.json`
and `site/` data changed. Line numbers the WP cites from the study
(`SUBMISSION.md:5-20`, `:57-126`; `SECURITY.md:49-110`;
`security-baseline-analysis.mjs:272-311, 624-678, 704-725, 1340-1352`)
hold. Three things the study did not note:

- The scanner reads the README **one physical line at a time**
  (`analysis:52-68`), so a negation must sit on one line, and its
  negation patterns need the plain words: "No `sudo`" in backticks is not
  recognised, "No sudo or pkexec is required" is. "never runs" is not
  recognised either (the pattern wants "never run").
- `.qml` and `.js` files are scanned for capabilities too
  (`scope:39-58`), so `Model.js` adds evidence to `privilege`
  (`sudo setfacl`) and `package-manager` (`pacman -S` in an advice text).
- The web form's tag labels are capitalised, the CLI guide's values
  lower-case (`submission.md` says which the draft uses).

## What was done

- **`plugin/README.md`** (`0f10c970`): both `curl … | bash` fences gone.
  Install keeps "download, read, verify, run"; a short paragraph says what
  the checksum shows and that the panel's *Install* pipes `install.sh` to
  `bash` without that first check (ADR-0024, in words). Removal from
  GitHub is the same four steps ending in `bash install.sh --uninstall`.
  The AUR paragraph says pacman asks for a password. Privilege wording in
  the scanner's form, one sentence per line: "No sudo or pkexec is
  required for these steps." (Install) and the bullet **Privileges**: "No
  sudo or pkexec is required to use it.", then the two password steps the
  user starts (the ADR-0026 grant, `omarchy pkg drop` → `sudo pacman
  -Rns`). The `systemctl` denial is gone; `--unit` is still named "for the
  optional watcher", true without `systemctl`. The inline mentions
  (States row, the command list, "No network") keep the one-liner in code
  spans: README prose is not a runtime file for the scanner, and hiding it
  would hide behaviour.
- **`plugin/SECURITY.md`**: unchanged. The scanner does not read it (not
  the root README, `.md` is not a scanned extension); it has no command.
- **`tests/release/store-readme.test.sh`** (`395bf49f`, in
  `check-packaging`, documented in `docs/TESTING.md`): fails on a
  downloader piped to a shell or interpreter (also through sudo/env
  wrappers, continued lines, `<(…)`, `$(…)`) in a command of
  `plugin/README.md` or `plugin/SECURITY.md` (fenced code of any language,
  or a line starting with curl/wget), and on agent files under `plugin/`
  (`AGENTS.md`, `CLAUDE.md`, `HANDOFF.md` in any case and folder,
  `.claude/`, `.codex/`, `.agents/`). 17 mutants each fail (the two
  removed fences verbatim among them); 3 controls pass (a prose code span,
  curl piped to `sha256sum`/`less`, a pipe after a finished download).
  Also run against the pre-WP README: it fails on both old fences.
- **`packaging/store/baseline.md`**: outcome, findings and each capability
  with its README lines and scanner lines; why there is no finding and no
  `service-management`; how the run was made.
- **`packaging/store/submission.md`**: title `[Plugin]: JAX Seldon`, the
  six headings in order, category `System`, tags `system, bar, ai`
  (proposal; the study's `quickshell, system` noted), maintainer notes
  (the engine dependency and how `install.sh` verifies it, the one-click
  banner `Model.js:64` until the AUR package, the ADR-0026 grant, both
  capabilities), the five attestations verbatim and unchecked, notes for
  the operator per attestation, and a line-by-line table against
  `SUBMISSION.md` and `submit-plugin.yml` at `5d22a9f`.
- **`docs/PLAN.md:69`**: points to the WP and `packaging/store/`;
  "plugins.omarchy.org and omahub.dev" and "security scan clean" dropped.
- `packaging/README.md`: the two new files in its table.

## How it was verified

| Check | Where | Result |
|---|---|---|
| `bash tests/release/store-readme.test.sh` with its mutants and controls | *fixture* | 22 ok |
| The same test against the pre-WP `plugin/README.md` | *fixture* | fails on both old fences, as it should |
| Split: `git subtree split --prefix=plugin` → `689dc5db4d997f2e716e9466e006edcb0f254b60` (tree `77e833be…` = `HEAD:plugin`), `git archive \| tar -x` into a scratch dir on disk | *fixture* | root `manifest.json`, `README.md` with Install and Remove, `LICENSE` (MIT), the engine named under Requirements; `preview.png` 2480×1080 (2.7 MP), 155 KB; no agent files, no symlinks, no executable files |
| `omarchy plugin validate` on the split (Omarchy 4.0.4-1's validator) | *fixture* | exit 0 |
| ID `jax.seldon` in the store's `registry.json` at `5d22a9f` | *fixture* | free: 0 of 5 255 listed IDs, not among 75 retired IDs |
| The store's baseline (`runSecurityBaseline`, scripts at `5d22a9f`, unchanged) on the split, network-free through a local fetch driver | *test host* | `review-required`, no findings, capabilities `privilege`, `package-manager` |
| The same on the pre-WP plugin tree | *test host* | `needs-fixes` (`curl-pipe-shell`), capabilities `privilege`, `package-manager`, `service-management` |
| `bash scripts/docs-check.sh` | *fixture* | ok (475 links) |
| `SELDON_FULL_CHECK=1 just check` (CARGO_TARGET_DIR, TMPDIR and a 0700 XDG_RUNTIME_DIR on disk, `CARGO_BUILD_JOBS=4`) | *headless* (Quickshell harnesses) and *fixture* | `check: ok` at `f8b2d347` (the last code commit; this handover commit only adds this file): engine tests, check-packaging with `store-readme.test: ok`, docs-check, plugin-validate, qmllint (49 files), plugin-test. shellcheck is not installed here, so the new test got `bash -n` only (*not run*: shellcheck); log in the private `gates/check-wp042.log` |
| CI | *not run* | the branch is not pushed |

The dev host ran nothing from the store clone. The driver and both JSON
reports are in the private `gates/wp042-store-scan/`; the scan folder on
the test host was removed afterwards (checked).

## Not done (by the WP)

- Filing, contacting anyone, answering bot reports; the scan of similar
  plugins (E75).
- The root `README.md` (`:218`) and the user guides
  (`docs/user/*/11-update-and-uninstall.md`) still show the
  `… | bash -s -- --uninstall` one-liner. The store does not scan them;
  WP-126's docs pass may align them with the plugin README.

## Open questions

1. Tags: `system, bar, ai` or the study's `quickshell, system`?
2. Attestation 4 ("does not overwrite user configuration without
   explicit consent"): the plugin's own settings are written by the shell
   into `~/.config/omarchy/shell.json` on the user's click, and the shell
   rewrites the whole file in its own formatting. `submission.md` says so;
   the operator decides whether to tick it as is.
3. The baseline must be rerun on the exact `jax-seldon-plugin` commit
   that is filed (after `v0.2.0`; later WPs change `plugin/`).
