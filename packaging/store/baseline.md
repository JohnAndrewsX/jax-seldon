# Store security baseline: jax-seldon-plugin (WP-042)

What the Omarchy plugin store's **Automated Security Baseline** reports for
the plugin split, with the README line and the scanner line behind each
result. The store runs it on the exact commit of the
`JohnAndrewsX/jax-seldon-plugin` repository named at filing; this file is
the prediction for that, plus one local run. The split at the `v0.2.0` tag
will differ (later WPs change `plugin/`): rerun before filing.

## What was read and scanned

| What | Value |
|---|---|
| Store rules | `omacom/omarchy-plugin-marketplace` @ `5d22a9fbee57f7b22652e09df128ddf12f14a65b` (2026-10-09 19:20 UTC), read-only shallow clone |
| Differences to the study's read (`4da3ed3e665c7376da83130445ba908865a68f7a`) | none in `SUBMISSION.md`, `SECURITY.md`, `VERIFICATION.md`, `README.md`, `AGENTS.md`, `scripts/` (byte-identical); only `registry.json` and `site/` data changed |
| Baseline | version `3`, enforcement `selective` (`SECURITY.md:85-90`) |
| Plugin split | `git subtree split --prefix=plugin` at branch commit `3b951145` (`wp/042-store-prep`, review round 2): split commit `4374707833cc6ee4431e4beb6055e9c1f538c3ff`, tree `64260354471ee9e3cb3ca3a9a34650e10d922787` (round 1: `0f10c970`, split `689dc5db4d997f2e716e9466e006edcb0f254b60`, same outcome) |
| Before WP-042 | `plugin/` tree `03f2715938f4d0e2fe497cca5f5afd42d113df39` at `9d776f9a` |

Scanner references below are `scripts/security-baseline-analysis.mjs`
(`analysis`) and `scripts/security-baseline-scope.mjs` (`scope`) at that
commit; README lines are `plugin/README.md` at `3b951145`.

## Result

| | Before WP-042 (*test host*) | Prediction (from the code) | Run (*test host*) |
|---|---|---|---|
| Outcome | `needs-fixes` | `review-required` | `review-required` |
| Disposition | `review-required` (remote execution only, `SECURITY.md:88`) | `review-required` | `review-required` |
| Findings | `curl-pipe-shell` | none | none |
| Capabilities | `privilege`, `package-manager`, `service-management` | `privilege`, `package-manager` | `privilege`, `package-manager` |

`review-required` needs a maintainer to accept the two capabilities through
`approved-and-verified` (`SECURITY.md:87`, `:92`); a capability is not a
finding (`SECURITY.md:65`, `:75`).

## Capabilities

### `privilege`

Rule: `SECURITY.md:69`, a non-negated `sudo` or `pkexec`;
`analysis:1340-1354` (`invokesPrivilegeBoundary`), applied to every line of
every scanned file (`analysis:1403-1421`). The root README and every
`.qml` and `.js` file are scanned (`SECURITY.md:37`; `scope:39-58`,
`scope:70-83`).

| Evidence | What it is |
|---|---|
| README 340 (States, "Snapshots not readable"), 454, 456 | the snapshot grant `sudo setfacl -m u:$USER:rx /.snapshots` (ADR-0026), started by the user's *Grant* in a terminal they see |
| README 491, 493, 494, 559 | the same grant; pacman running through sudo for the engine's AUR package, and `omarchy pkg drop` running `sudo pacman -Rns` when the user removes it |
| `Model.js:74`, `:182`, `:190` | the grant's constant command, its comment, its run line |

Recognised negations, not evidence (`analysis:1349`, the form
`SECURITY.md:69` documents): README 55 "No sudo or pkexec is required for
these steps." and 488 "No sudo or pkexec is required to use it." Each is
true where it stands: the three install steps and the plugin's own use run
as the user; the grant is optional. The scanner reads one physical line at
a time (`analysis:52-68`), so each sentence keeps to one line, without
backticks around the names.

The bot's report lists at most five evidence lines per capability
(`analysis:1370`); the run listed `Model.js:74`, `Model.js:190`, README 332
(the States table, read as one line from its header because each row ends
in `|`), 454 and 456.

### `package-manager`

Rule: `analysis:1422-1432` (`omarchy pkg add|drop|remove|update`,
`pacman|yay|… -S/-R/-U`, …).

| Evidence | What it is |
|---|---|
| README 100 | `yay -S jax-seldon`, updating the engine from the AUR |
| README 494, 556, 559, 560 | removing the engine's AUR package: `omarchy pkg drop`, `sudo pacman -Rns`, `yay -R` |
| `Model.js:5776` | advice text in the transaction view: reinstall packages with `pacman -S` |

README 99 `omarchy pkg aur add jax-seldon` does not match (`aur` stands
between `pkg` and `add`); the capability is the same either way. The
plugin itself never starts a package manager.

## No findings, and why

- **`curl-pipe-shell`** (`SECURITY.md:53`; `analysis:280-312`,
  `analysis:500-526`) is checked on runtime files only (`analysis:1305-1320`):
  shell paths, executables, and the root README's shell fences, which the
  scanner turns into executable files (`analysis:624-674`). After WP-042
  the README has no fence with a downloader piped to a shell (before: README
  76 and 525, reported as one evidence line because both are line 1 of
  their fence).
- The two "download, read, verify, run" fences (install, README 66-74;
  removal, 542-550) download with `curl -fsSLO` and run `bash install.sh`
  only after `sha256sum -c`. The download-then-run rule reads a target
  from `-o FILE`, `--output` or `>` (`analysis:500-506`), not from `-O`,
  so it does not evaluate these lines; they verify the script against the
  release's `SHA256SUMS` anyway, which is what the rule asks for ("without
  verification"). The checksum shows that `install.sh` matches the release,
  not who built it; the optional `gh attestation verify install.sh` line in
  both fences checks that this project's release workflow built it
  (`release.yml` attests `install.sh`), and `install.sh` checks the
  engine's build provenance the same way when `gh` is logged in.
- **A review risk the scan cannot show.** Store maintainers have objected
  to exactly this shape in other plugins: "Moving a mutable
  download-to-shell command into the UI does not authenticate the required
  backend. A checksum from the same movable tag as a root bootstrap supplies
  no independent trust" (Omatalk, store issue #8407, as quoted in
  T. Ballard's `omarchy-plugin-release/references/release-contract.md:97-101`;
  not re-read at the store). The panel's *Install* (`Model.js:64`) and the
  `releases/latest` + `SHA256SUMS` path are that shape; Seldon's answers
  are the attestation of `install.sh` and the engine (above), no root, and
  the switch to the AUR package under ADR-0024. `submission.md`'s
  maintainer notes say this; the operator decides before filing.
- **The panel's one-liner stays documented.** README 334 (States),
  449 (the list of commands the plugin may start) and 479 name
  `curl -fsSL …/install.sh | bash` in prose code spans (ADR-0024). README
  prose is not a runtime file (`analysis:618-622`: mode `100644`, no
  shebang), so the rule does not read it. `Model.js:64`
  (`INSTALL_ENGINE_COMMAND`) is a `var` string, not an
  `exec(…)`/`spawn(…)`/`command(…)` call or a `command: […]` array
  (`analysis:703-725`), so it is not expanded either. A human reviewer
  reads both; `submission.md`'s maintainer notes name them.
- **No `service-management`**: no `systemctl` or `systemd-run` in the
  README, the QML or the JavaScript, and no `.service` file. The README's
  denial sentence "never runs … `systemctl`" (before, README 479) is gone;
  README 86-87 names `--unit` "for the optional watcher" without `systemctl`,
  which stays true (the project README describes the unit).
- No `installer` (no file named `install`, `setup` or `uninstall`,
  `analysis:1334-1338`), no `remote-build` (no `curl`, `wget` or `git` URL
  to the submission repository, `analysis:1417-1420`; no `git clone` in a
  fence), no `bundled-executable-binary` (no file with mode `100755`), no
  sudoers policy.

## How the run was made

- *Test host*, Node v26.7.0. The store's `scripts/` at `5d22a9f`, unchanged:
  `runSecurityBaseline` from `security-baseline-scanner.mjs`, with
  `requiredPaths` set to the manifest's entry points as
  `scripts/security-baseline.mjs:53-56` does for a submission.
- A local driver (not in this repository) passed it a `fetchImpl` that
  answers the three GitHub API requests and the raw file reads from the
  extracted split; nothing went to the network. 53 files were read, as
  the scope selects them.
- The dev host ran nothing from the clone; the scan folder on the test
  host was removed afterwards.
- *Fixture*: `tests/release/store-readme.test.sh` (in `just
  check-packaging`) keeps downloader-to-shell commands and agent files out
  of the split; it does not run the store's scanner.

## Before filing (not WP-042)

Rerun this scan on the exact `jax-seldon-plugin` commit to be filed and
compare it with this file; a later commit to that repository invalidates
the result (`SECURITY.md:94-96`).
