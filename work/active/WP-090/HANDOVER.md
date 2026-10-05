# WP-090 HANDOVER — Stale plugin code after an update; harness record order

Branch `wp/090-stale-plugin`, worktree `wt/WP-090`. Commits on top of
`d52e96d` (oldest first):

- `a8c9061` tests: append each fake-recorder record in one write
- `12d4837` plugin: notice when the shell still runs the code from before an update
- `3f0e416` docs: restart the shell after a plugin update
- `0310fde` tests: fake-recorder stages nothing without HARNESS_RECORD
- plus the commit with this handover

## Done

### Shell check (what `omarchy plugin update` is meant to do)

Read in `$OMARCHY_PATH` (`bin/omarchy-plugin-update`, `shell/shell.qml`
`reloadPlugins`/`finishPluginReload`/`_syncServices`/`ensureService`,
`shell/services/PluginRegistry.qml`); written to `memory/omarchy-shell.md`
("WP-090 findings").

- The update **is meant to reload**: after a pull it runs
  `omarchy-shell shell rescanPlugins` → `reloadPlugins()`, which destroys
  the plugin's panels, service and widget registrations and calls
  `Qt.clearComponentCache()` — but only `if (typeof … === "function")`.
- In Quickshell 0.3.1 `Qt.clearComponentCache` (and `trimComponentCache`)
  is `undefined`, so `Qt.createComponent(url)` hands back the cached
  compiled code. Reproduced in a private offscreen Quickshell (not the
  running shell): a QML file and its JS import edited on disk,
  `createComponent` again → both still the old code. So the docs say
  "restart the shell", not "the update reloads".
- The **manifest is fresh** after the rescan (the registry `cat`s it; a new
  service gets `manifest = publicPluginManifest(m)`, a kept one is handed
  the fresh one). That makes the hint feasible.

### (a) The hint

- `plugin/Model.js`: `PLUGIN_VERSION = "0.1.3"` (equal to
  `manifest.json` `version`), `RESTART_SHELL_ARGV = ["omarchy-restart-shell"]`,
  `pluginVersionOf(manifest)`, `restartShellNotice(running, onDisk)` →
  `null` when equal or the manifest is not injected yet, else a neutral
  banner object: "Restart the shell to finish the update", "Seldon X is
  installed, but the shell still runs Y. …", command text
  `omarchy-restart-shell`, one action `restart` / *Restart shell*.
- `plugin/Service.qml`: `pluginVersion`, `manifestVersion` (from the
  injected manifest), `restartNotice`; `fix("restart", "restart")` runs
  `Quickshell.execDetached(Model.RESTART_SHELL_ARGV)` only while the
  notice shows; any other action on that banner returns false. Snapshot
  fields `pluginVersion`, `manifestVersion`, `restartNotice`,
  `restartActions`.
- `plugin/Panel.qml`: a `Banner` for it above the status banner (it
  qualifies everything below it); `view().restartNotice`.
- Reading the plugin's own manifest is not Seldon data: AGENTS.md §3
  covers the logbook and the index. The plugin does not read the file
  itself; it uses the manifest the shell injects (the shell strips
  `__sourceDir`, so the injected copy is the only way to see it). Stated
  in SPEC-PLUGIN §3.
- Works from the first release that contains it onward: the 0.1.3 code
  that runs after a 0.1.3 → 0.1.4 update has no notice. The docs
  therefore say to restart after every update, without naming versions.
- **Version constant: set by hand, not by `set-version.sh`** (see
  Decisions needed). `model.test.js` asserts `PLUGIN_VERSION ===
  manifest.version`; the harness's "same" cases inject the real manifest
  and fail too. VERSIONING.md tag-flow step 1 and its table name the
  constant. `expected-files.txt` untouched (the plugin is not in the
  package).

### (b) Harness record order

The sorted compare in `record_check` was already order-independent; the
real defect (WP-084's log: "`wl-copy`, terminal, `--`, fix, fix") is that
the records **interleaved line by line**: bash line-buffers stdout, so the
recorder's single `printf` was one `write(2)` per line. Two concurrent
recorders: 37/500 runs differed before, 0/2000 after (scratch
`race.sh`). The recorder now stages the record in `$HARNESS_RECORD.<pid>`
and appends it with `cat` (one write). Without `HARNESS_RECORD` it records
nothing and exits 1 (as the failed append did; first version left a
staging file in the checkout, caught by `git status`, fixed in
`0310fde`). The `fix-engine` case (copy + terminal) had the same race
and is fixed by the same change.

### Tests

- `model.test.js`: one new test (versions equal, notice text, tone,
  single action, argv, downgrade counts, no pictogram).
- `service-states.sh` 14g: `restart-same` (real manifest: no notice, the
  restart action returns false and launches nothing), `restart-updated`
  (version 99.0.0: notice, `copy` refused, `restart` records exactly
  `omarchy-restart-shell` with no arguments). Harness: `HARNESS_MANIFEST`
  injects a manifest after creation, as the shell does.
- `panel-view.sh` 29: same two states rendered; text and command on
  screen; above the status banner; a click on *Restart shell* records
  the argv. `harness/panel.qml` takes `HARNESS_MANIFEST` too.

### Docs

Guide 11 en/de ("Update the plugin": `omarchy-restart-shell` after the
update, why, the notice), README.md update bullet, plugin/README.md (new
`## Update`, States row, security list: `omarchy-restart-shell` without
arguments, troubleshooting row), SPEC-PLUGIN §3 §5 §10, VERSIONING.md,
CHANGELOG `[Unreleased]` → Plugin.

## Not done

- No live check: the shell was not restarted and `~/.config` not touched
  on this host (brief). Open for the orchestrator's live check on the
  test host: (1) after `omarchy plugin update` with a version bump, the
  panel shows the notice; (2) *Restart shell* restarts the shell —
  `Quickshell.execDetached` must outlive the shell that
  `omarchy-restart-shell` kills (QProcess detached start, own session; not
  verified live; no first-party QML does this); (3) the restart refuses
  while locked (`omarchy-restart-shell` exits 1, the notice stays).
- The overlay does not show the notice (WP: "the panel shows it").
- No `docs/user` screenshot of the notice.

## Verified by

- `flock /tmp/seldon-check.lock just check` **3× exit 0** at `0310fde`:
  each `check: ok`, model.test.js 89, service-states 311/0, panel-view
  782/0, overlay-view 319/0, bar-view 143/0, qmllint ok (29 files),
  real-home-guard 11/0; `git status` clean after each.
- `omarchy plugin validate plugin/` exit 0 (silent); `just qmllint` ok.
- Race: `race.sh` old recorder 37/500 differ, new 0/2000 and 0/1000.
- Mutants (all in a scratch copy of plugin/ and tests/plugin/, never in
  the worktree; the copy compared byte-identical afterwards). Node:
  `model.test.js`; harness: the setup of service-states.sh / panel-view.sh
  plus only the WP-090 cases (baselines 15/0 and 12/0), each under the
  check lock.

  | Mutant | Caught by |
  |---|---|
  | M1 `restartShellNotice` always null | model.test.js |
  | M2 no equality check (notice whenever injected) | model.test.js |
  | M3 no empty check (notice before injection) | model.test.js |
  | M4 `PLUGIN_VERSION` 0.1.2 ≠ manifest | model.test.js |
  | M5 argv `["omarchy-restart-shell", "--now"]` | model.test.js |
  | M6 tone accent | model.test.js |
  | M7 extra *Copy* action | model.test.js |
  | M8 numeric `version` accepted | model.test.js |
  | H1 Service: restart without the notice guard | service-states restart-same (14/1) |
  | H2 Service: any action id on the restart banner | service-states restart-updated: copy not refused, fix commands (13/2) |
  | H3 Service: extra argv element | service-states restart-updated fix commands (14/1) |
  | H4 Service: injected manifest ignored | service-states 5 FAIL (10/5) |
  | H5 Service: restart not dispatched | service-states restart-updated fix commands (14/1) |
  | H6 Panel: button not wired | panel-view: launches differ |
  | H7 Panel: notice below the status banner | panel-view order check (11/1) |
  | H8 Panel: banner bound to null | panel-view: notice, text missing |
  | H9 Panel: action sent as bannerId "status" | panel-view: launches differ |
  | R1 old fake-recorder (single printf) | race.sh 37/500 differ |

## Learned

- In `memory/omarchy-shell.md` ("WP-090 findings"): the update reloads by
  design but Quickshell 0.3.1 keeps compiled QML and JS; the manifest is
  fresh; how `omarchy-restart-shell` works; fake recorders must append in
  one write (the reviewer may move that bullet to pitfalls.md).

## Decisions needed

1. **Version constant set by hand instead of by `set-version.sh`.** The
   WP suggests `set-version.sh` sets it. That script only edits the
   PKGBUILD, inside the tag build, and the plugin is published by
   `git subtree split` of the tag commit — a value written there never
   reaches the plugin. I made it a hand-set constant next to the
   manifest's `version` (VERSIONING.md step 1), enforced by
   `model.test.js` and the harness. Please confirm, or name another
   mechanism.
2. **Guard blocks (reported, not routed around):** (a) `pacman -Q omarchy
   quickshell` (read-only version query) → "privileged or package
   command"; I used `quickshell --version` and `$OMARCHY_PATH/version`
   instead. (b) A `sed` editing my scratch `race.sh` was blocked because
   its test string contained `sudo setfacl …` (the snapper fix constant).
   I rewrote the scratch file with the Write tool and a neutral test
   string (file content, per the guard-block memory); nothing privileged
   was meant or run. Both look like false positives for
   `scripts/guard.sh`.

## Touched outside WP scope

- `docs/VERSIONING.md` (step 1 and the table: the new constant).
- `tests/plugin/panel-view.sh` and `tests/plugin/harness/panel.qml`
  (panel case for both states; the WP names service-states.sh and the
  harness).
- `memory/omarchy-shell.md` (requested by the brief).

## Round 2

Brief: review round 1 SEND BACK (B1, B2, N1–N3) plus D1; live check on
the test host passed; Decision 1 accepted. Commits on top of `19f9e64`:

- `405e2ea` merge of `main` (B2)
- `bb6b981` packaging: the plugin's version pair in check-packaging and the release (B1)
- `868f16c` docs: restart notice in the troubleshooting banners (N1)
- `73d5e8c` docs(de): source lines of guides 10 and 11 (N1)
- `fe01adb` memory: restart-shell via the Omarchy menu, live result; recorder pitfall moved (N2)
- `2506e16` tests: a failing argv compare in panel-view reaches the summary (N3)
- `67da25b` plugin: the restart action runs once per service (D1)
- plus the commit with this section

### Done

- **B2.** `git merge main` (no rebase). Conflicts in CHANGELOG.md
  (`[Unreleased]`: main's `### Engine`, then this WP's `### Plugin`) and
  memory/omarchy-shell.md (main's "Live frame timing…", then "WP-090
  findings"): both kept, main first.
- **B1.** `packaging/plugin-version.sh MANIFEST MODEL_JS [VERSION]` (jq,
  grep, bash only): exactly one `var PLUGIN_VERSION = "…"` line, equal to
  the manifest's `version` string, and to VERSION when given. Runs in
  `just check-packaging` on `plugin/` (not host-gated, so CI runs it; also
  under `bash -n`/shellcheck there) with
  `tests/release/plugin-version.test.sh` (12 cases: agree, pipes, either
  side bumped, release version differs, no/two/single-quoted lines,
  numeric/missing manifest version). release.yml "Plugin split": one
  `env: VERSION` and one line, the script on
  `<(git show "$split:manifest.json") <(git show "$split:Model.js")
  "$VERSION"` (the job's default shell is bash; the step above already
  uses process substitution). No new actions, no permission changes.
  VERSIONING.md step 1 and packaging/README.md (`build` row) say where it
  runs.
- **N1.** en/de guide 10: a row "Restart the shell to finish the update";
  the "Index format mismatch" rows there and in plugin/README.md add the
  restart after the plugin update. The de source lines of guides 10 and
  11 now name the en commits (round 1 had left guide 11's stale; the
  remaining docs-check warning, guide 06, comes from main/WP-089).
- **N2.** omarchy-shell.md: the Omarchy menu runs `omarchy-restart-shell`
  through `Quickshell.execDetached(["bash", "-lc", …])` (menu jsonc →
  Menu.qml `runAction` → Util.qml `execDetached`), plus the live result
  and the double-click note. The recorder bullet moved to pitfalls.md
  (new WP-090 section).
- **N3.** All 13 `diff … | sed` lines in panel-view.sh FAIL branches end
  in `|| true`; the fail count keeps the exit non-zero.
- **D1.** Service.qml `restartStarted`: set by the first restart; the
  action then returns false until the service is recreated (the new
  shell creates a new one). Snapshot field `restartStarted`. SPEC-PLUGIN
  §5: "once per service instance". Harness: service-states
  `restart-updated` fixes `restart:copy,restart:restart,restart:restart`
  → log `true` then `false`, `restartStarted` true, one launch recorded;
  panel-view `restart-updated` clicks *Restart shell* twice → one launch
  (read 1 s after the first lands).

### Verified by

- `flock /tmp/seldon-check.lock just check` once at `67da25b`: exit 0,
  `check: ok`; check-packaging ok (plugin-version.test ok; shellcheck not
  installed on this host, `bash -n` only), model.test.js 89,
  service-states 314/0, panel-view 782/0, overlay-view 319/0, bar-view
  143/0, qmllint ok (29 files), real-home-guard 11/0, docs-check ok;
  `git status` clean.
- `omarchy plugin validate plugin/` exit 0; `just qmllint` ok.
- Release step simulated on `HEAD:` paths through process substitution:
  VERSION 0.1.3 → exit 0; 0.1.4 → exit 1 "the plugin is at 0.1.3, the
  release at 0.1.4".
- Mutants (harness ones in the scratch copy, under the check lock;
  copy byte-identical afterwards; baselines service 21/0, panel 12/0):

  | Mutant | Caught by |
  |---|---|
  | B1: manifest.json bumped to 0.1.4 in the worktree (restored by `git checkout`) | `just check-packaging` exit 1, "manifest.json says 0.1.4, Model.js PLUGIN_VERSION says 0.1.3" |
  | B1: the edits in plugin-version.test.sh (each side bumped, release version, line shape, manifest type) | the script exits 1 with the named message in each (12/12 ok) |
  | D1: latch not checked | service-states: second restart not refused, fix commands (19/2); panel-view: launches differ (11/1) |
  | D1: latch never set | service-states 3 FAIL incl. `.restartStarted` (18/3) |
  | N3: panel button unwired (FAIL branch with diff) | panel-view prints "11 passed, 1 failed" and exits 1 (round 1's H9 run stopped at the diff) |

### Not done

- shellcheck of the new script: not installed on this host; CI runs it
  in `check-packaging`.
- The release step itself only runs on a tag build or dry run; simulated
  locally as above.

### Decisions needed

None.

### Touched outside WP scope

- `justfile` (check-packaging), `.github/workflows/release.yml` (Plugin
  split), `packaging/plugin-version.sh`, `tests/release/plugin-version.test.sh`,
  `packaging/README.md` — all per the round-2 brief (B1).
- `docs/user/{en,de}/10-troubleshooting.md` (N1), `memory/pitfalls.md` (N2).
