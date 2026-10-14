# WP-230 handover

## Done

- `packaging/seldon/`: `PKGBUILD` (package `seldon`: engine via cargo, `seldon-desk` via `app/bin/build`, desk in `/usr/lib/seldon/`, plugin copy under `/usr/share/seldon/plugin/`, LICENSE, README, watcher unit, man page, completions; `provides/conflicts/replaces=('jax-seldon')`; `qt6-base>=6.9`, `qt6-declarative>=6.9`, `qt6-svg`, `qt6-wayland`, `hicolor-icon-theme`), `seldon.desktop` (`Exec=seldon open`, `StartupWMClass=seldon`, `Name[de]`, `Comment[de]`), `seldon.svg` (the existing `assets/a2-icon-brand-on-dark-container.svg`, unchanged), `seldon.install` (desktop database and icon cache only), `expected-files.txt`.
- `packaging/opr/`: `.omarchy/package.json` (`source: local`, `auto_merge`, `upstream.watch`) and a README. **Not submitted.**
- `bin/{build,test,install}` at the root. `bin/install` builds the package from `git archive HEAD` into `build/pkg` and installs only with `--install`.
- `packaging/app-era.sh VERSION`: exit 0 from 0.5.0. `release.yml` has a step `App era` (output `app_era`) and a step `Build the seldon package` with `if: steps.era.outputs.app_era == 'true'`, after the unchanged `jax-seldon` build. A 0.4.x tag skips it, so the old route is untouched. The Qt packages and `hicolor-icon-theme` are added to the build job's package list (so `makepkg` finds its dependencies); the Qt packages are added to `ci.yml`'s `check` job.
- `tests/release/seldon-package.test.sh` (in `just check-packaging`): recipe names and depends, desktop entry, icon, install script (no `seldon` call), no hex colour outside the icon, opr JSON, expected-files against `plugin/`, the gate, workflow wiring.
- `packaging/README.md`: section "The seldon package".

## Not done / decisions

- No `.SRCINFO` for `seldon` (not an AUR package). `sha256sums=('SKIP')` in the repo copy; the workflow sets the real sum, and the opr submission needs the release sum.
- At 0.5.0 the `jax-seldon` route (AUR push, `packaging/PKGBUILD`, its job steps) still runs; retiring it is a separate change.
- The justfile's "no install script" rule still covers `packaging/PKGBUILD` and `packaging/*.install`; the `seldon` install script is held to cache refreshes by the new test.
- The packaged desk binary was not started (no display step); `seldon open` launching `/usr/lib/seldon/seldon-desk` is not part of this WP.

## Verified

- `flock /tmp/seldon-check.lock just check-packaging`: green (*desktop*, dev host; shellcheck present or not as `just` reports).
- `bin/install --nodeps` (`makepkg -f --nodeps`, build dir `build/pkg` in the worktree): built both binaries and the package (*desktop*). `expected-files.txt` was generated from that package's `tar tf`.
- `makepkg` in the CI container, the release workflow steps, `ci.yml` with the new packages, the Qt 6.9 floor on the stable mirror: *not run* (no push). `desktop-file-validate`, `namcap`: *not run* when absent.
- `SELDON_FULL_CHECK=1 just check`: see below.

## Open questions

- Operator: who opens the omarchy-pkgs pull request, and after which release (needs the 0.5.0 tarball sha256)?

## Full check result

`SELDON_FULL_CHECK=1 just check` (*desktop*): every recipe before `qmllint` passed (fmt, clippy, engine tests, watch, check-packaging, install, deploy, guards, schema, docs, plugin-validate). `qmllint` **failed** with exit 255 on `plugin/` files this WP does not touch (`Style.font.family`, `Style.font.caption`: "Member not found on type QObject", e.g. `plugin/sections/Work.qml:1114`). That is the edge Omarchy shell tree on this host against the plugin (the 0.4.1 edge-compatibility work, WP-224/225), not a packaging change. `plugin-test` did not run after it. Reported, not worked around.

After the rebase onto `app` (864d8edc): `app/bin/build` rebuilt, `bin/install --nodeps` rebuilt the package and its file list equals `expected-files.txt`, `just check-packaging` green (*desktop*). `SELDON_FULL_CHECK=1 just check` failed again at `qmllint` with the same plugin errors as above (`Style.font.*` on `plugin/sections/Work.qml`); no WP-230 file is involved.
