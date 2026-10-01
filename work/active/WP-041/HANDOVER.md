WP-041 HANDOVER

Branch `wp/041-plugin-docs`, worktree `wt/WP-041`, on top of `main` at
`1614f2e`. Not pushed, no PR. Commits `main..HEAD`:
`aa515b9` README and manifest · `e6f4891` KEYBINDINGS.md · `09f1774`
preview.png, renders, PANEL_SHOTS, TESTING · `856b76c` memory · then
`work: WP-041 handover` (this file). `just check` exits 0 on this tree
(see Verified by).

## Done

- **`plugin/README.md`** rewritten along the marketplace template. The
  template is the example on https://plugins.omarchy.org/develop.html:
  title and description → Install → Usage → Configure → Remove, plus its
  one rule ("Document every external dependency, setup step, privilege
  boundary, service, installer, or remote build used by your plugin"). The
  built-in READMEs (`agents/`, `bar/`) have no common order, so the
  marketplace order wins. The WP's extra sections are slotted in:
  - title, what it is, `preview.png` with a caption saying it is an offscreen
    render;
  - **Requirements** (Omarchy 4, `omarchy pkg aur add jax-seldon`,
    `seldon init`);
  - **Install** (`omarchy plugin add https://github.com/JohnAndrewsX/jax-seldon-plugin.git --enable`,
    ADR-0009; the dev install by copy with `omarchy plugin enable` and
    `omarchy-restart-shell`);
  - **Usage**: The pill (clicks), The panel (six tabs with numbers, shows
    and actions; QuickEntry, Capture now, Open in editor, Work card
    actions, New case, New decision, Resolve drift), **Keys** (the full
    panel table: `1`–`6`, ←/→ `h`/`l`, ↑/↓ `k`/`j`, Tab/Shift-Tab, Enter,
    Space, `x`, `a`, `f`/`F`, `c`, `n`, `+`, `d`, `e`, Esc; two-press
    arming; keys in fields and sheets; the Prime Radiant table `1`–`4`,
    ←/→ `h`/`l`, Esc), The Prime Radiant (open, header, periods, the six
    charts, hover, narrow grid, banner), **States** (all four AGENTS.md §7
    states plus contract mismatch and snapper, each with its fix);
  - **Configure**: settings as a key table (`captureIntervalMin`,
    `wipLimit`) with `omarchy bar set … --json` and `omarchy bar move`, the
    suggested binding, the IPC table;
  - **Security, privacy, privileges** (below), **Troubleshooting**,
    **Remove** (`omarchy plugin remove jax.seldon`; engine, logbook and
    index stay), **Development**, **Project home and licence**.
  - The README was checked against the code, not only the old README: key
    handling in `Panel.qml` and the shell's `PanelKeyCatcher.qml` (`x`/`X`
    is the shell's delete key; the first ↑/↓ only shows the cursor), banner
    labels and commands in `Model.js`, the arming rules (the QuickEntry and
    the new-case sheet send on one Enter; the Work card actions, the drift
    sheet and the new-decision sheet need two; the old README's "Enter in a
    text field creates the case" was right, and I first wrote it wrong,
    then fixed it before the commit).
- **Security section**, cross-checked with grep over `plugin/Service.qml`
  and `plugin/Model.js`:
  - the 15 `seldon` forms are copied verbatim from CONTRACT.md "Commands
    the plugin may run" (a scripted diff of the two lists is empty);
  - two notes where the code differs: the probe runs `seldon --version
    --json` (Service.qml:166), and `rebuild` / `update-impact` are allowed
    by `Model.validateArgs` but never issued;
  - every engine call goes through `Service.run()` → `Model.validateArgs`;
    dev mode refuses everything but the probe (Service.qml:199);
  - besides `seldon`: `wl-copy -- <command>` **and**
    `omarchy-launch-floating-terminal-with-presentation <command>`
    (Service.qml:467/470), both only on a banner click, `<command>` one of
    the five constants in Model.js:41–48. The launcher runs its argument
    with `bash -c` in a visible terminal (read from
    `$OMARCHY_PATH/bin/…`), and the README says so; the snapper fix is a
    `sudo` command that asks for the password in that terminal.
  - reads one file (`FileView` on the index only); no network, no units,
    no binaries, no symlinks, no writes, no privileges; dev mode read-only.
- **`plugin/preview.png`**: 2400×1080, 150 KB. On the left the Prime
  Radiant at 1920×1080 (90 d), on the right the panel's Today tab (cropped
  to the panel and framed in the theme's background and accent), both in
  **Tokyo Night**, both offscreen renders of the real QML on
  `fixtures/index.sample.json`. Composed with ImageMagick 7 (`magick` is
  installed on the dev host). **Source renders are in `docs/images/`**
  (`overlay-tokyo-night-1920x1080.png`, `panel-tokyo-night-today.png`),
  which keeps them out of the plugin repository split; the recipe is in
  `docs/TESTING.md`.
- **`tests/plugin/panel-view.sh`**: new step 23, `PANEL_SHOTS=<dir>`
  (opt-in, like `OVERLAY_SHOTS`): the Today tab in Osaka Jade, Tokyo Night
  and Catppuccin Latte. It is a **live run on the fake engine**, not dev
  mode, because the dev-mode footer prints the absolute index path, which
  was `/home/<operator>/Work/…` in my first render (AGENTS.md §8: no private
  paths in the repository). Without `PANEL_SHOTS` nothing changes.
- **Keybinding docs:** the README's **Keys** section is canonical (only
  `plugin/` ships in the plugin repository); `docs/KEYBINDINGS.md` points
  to its sections, names the spec as normative and repeats the suggested
  binding.
- **`plugin/manifest.json`**: `barWidget.description` "Active cases and
  open drift at a glance" → "Active cases and unexplained changes at a
  glance" (the user-facing term, as the pill's tooltip uses it). The
  top-level `description` stays: it is good for the listing and SPEC-PLUGIN
  §1 quotes it verbatim. No id, kind or entry-point changes.
- **`docs/TESTING.md`**: `PANEL_SHOTS` sentence (layer 3) and the
  `preview.png` recipe (layer 3b).
- **`memory/omarchy-shell.md`**: WP-041 findings.
- **No plugin code change**, no UI wording fix.

## Not done

- **The bar pill is not in the preview.** The harnesses render the panel
  and the overlay; the pill (`BarWidget.qml`) needs a bar host that no
  harness provides, and the test host is locked. The README shows the
  pill as text (`⟡ A · D`). A live `grim` of the bar on an unlocked host
  could be added to the composition later.
- **No live screenshots, no test host.** I did not touch the test host
  (locked per the brief) and did not install anything on the dev host.
  `omarchy plugin add --enable` and `omarchy plugin remove` are documented
  from `omarchy plugin --help`, not run.
- **`omarchy pkg --help` was denied** (permission prompt declined). I did
  not retry it or work around it. So the README does not name an `omarchy
  pkg` remove command: it says to remove the engine "with your AUR helper
  (`yay -R jax-seldon`)". Replace that if there is an `omarchy pkg aur
  remove`-style command.
- **`plugin/LICENSE` does not exist** (only the root `LICENSE`).
  SPEC-PLUGIN §2 lists it, and the subtree-split plugin repository will
  have no licence file without it. It was not in my outputs; see
  Decisions 2.

## Verified by

```
$ just check                                    → exit 0
  fmt-check, clippy, engine tests ok · check-watch ok
  validate-fixtures: ok — 109 instances, 8 variants, 23 self-checks
  plugin-validate: ok · tokens: ok (520 references) · qmllint: ok (28 files)
  model.test.js: 72 passed · service-states: 189 passed
  panel-view: 548 passed, 0 failed · overlay-view: 312 passed, 0 failed
  plugin-test: ok · check: ok
$ PANEL_SHOTS=<scratch> bash tests/plugin/panel-view.sh
                                                → 554 passed, 0 failed (dev-mode render, first try)
  then the shot step alone (live mode, scratch copy of the script's setup
  plus step 23)                                 → 10 passed, 0 failed, 3 renders
$ OVERLAY_SHOTS=<scratch> (setup plus step 9 of overlay-view.sh)
                                                → 31 passed, 0 failed, 12 renders
$ identify plugin/preview.png                   → PNG 2400x1080, 149757 bytes (≤ 1 MB)
$ diff <(contract forms) <(README security forms)  → empty ("security list == contract")
$ every SPEC-PLUGIN §5 key and the overlay keys grep'd in README Keys → all present
$ every AGENTS.md §7 state as a row of the States table          → all four present
$ per-table pipe count in README.md and KEYBINDINGS.md            → 11 tables consistent
$ find plugin -type l | wc -l                    → 0
$ grep -rn "open drift at a glance" .            → no other reference
```

I checked the preview by eye: no private path, machine name
`workstation-7f3a` from the fixture, theme colours applied.

## Learned

In `memory/omarchy-shell.md` (WP-041): the marketplace template and its
documentation rule; the `omarchy plugin` / `omarchy bar` CLI forms; the
floating-terminal launcher runs its argument with `bash -c`; dev-mode
panel renders print the index path, so marketplace renders come from a
live run on the fake engine (`PANEL_SHOTS`).

## Decisions needed

1. **Version probe vs CONTRACT.md.** The plugin runs `seldon --version
   --json`; the contract lists `seldon --version`. The README lists the
   contract form plus a note. Either the contract line gains `[--json]`
   (contract owner) or the probe drops `--json` (plugin WP). I recommend
   the contract edit; the validator already allows a trailing `--json`
   on every command.
2. **`plugin/LICENSE`.** Copy the root `LICENSE` into `plugin/` (SPEC §2
   lists it) so the generated `jax-seldon-plugin` repository carries its
   licence. Here, or in WP-040's split step?
3. **SPEC-PLUGIN §10** says "Opens a terminal only on explicit click".
   True, but it should also name `wl-copy` and the terminal launcher as
   the only non-engine programs (spec owner's edit; the README now does).
4. **Engine removal command** in the README's Remove section (see Not
   done): keep `yay -R jax-seldon` or name an `omarchy pkg` command.

## Touched outside WP scope

- `tests/plugin/panel-view.sh`: step 23 (`PANEL_SHOTS`, opt-in; the
  "panel equivalent" of `OVERLAY_SHOTS` the WP refers to did not exist).
- `docs/images/` (new): the two source renders of `preview.png`.
