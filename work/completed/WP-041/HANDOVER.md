WP-041 HANDOVER

Branch `wp/041-plugin-docs`, worktree `wt/WP-041`, on top of `main` at
`1614f2e`. Not pushed, no PR. Commits `main..HEAD`:
`aa515b9` README and manifest · `e6f4891` KEYBINDINGS.md · `09f1774`
preview.png, renders, PANEL_SHOTS, TESTING · `856b76c` memory ·
`6112efc` handover · `49a861c` review follow-ups · then `work: WP-041
handover after review` (this file). `just check` exits 0 at `49a861c`
(see Verified by).

## Review follow-ups (round 2; review: APPROVE, decisions 1–4 taken)

- **(a)** `plugin/LICENSE` is a byte-identical copy of the root `LICENSE`
  (`cmp` clean). The README's licence line links it, and the Security
  section lists it among the plugin folder's files.
- **(b)** Remove section: `omarchy pkg drop jax-seldon` in its own code
  block, with the note that it runs `sudo pacman -Rns` and asks for the
  password; `yay -R jax-seldon` stays as the alternative in parentheses.
- **(c)** The version-probe footnote is gone. The Security list, the
  "When it runs the engine" bullet and the dev-mode bullet now say
  `seldon --version --json`. Until the CONTRACT.md edit at merge, the
  scripted diff of the two lists has exactly this one line (`seldon
  --version` vs `seldon --version --json`); after it, none.
- **(d)** Alt text: "Preview, 2400×1080: the Prime Radiant at 1920×1080 on
  the left, the panel's Today tab framed on the right, Tokyo Night".
- **(e)** `omarchy plugin validate plugin/` → exit 0; `just check` → exit
  0 (numbers under Verified by); no symlinks.
- **Guard false positive.** My first step was a read-only
  `grep -n -E "…|yay -R|…" plugin/README.md` to find the lines to change.
  `scripts/guard.sh` blocked it ("privileged or package command"), because
  the search pattern contains `yay -R`. I did not re-run the search under
  another wording. I read and edited the README with the file tools
  instead, since the guard rule is about executing a blocked action, and
  file content that merely contains trigger words is fine (memory
  `guard-block-stop`). I also kept trigger words out of the follow-up
  commit message. Suggested fix: a row in `scripts/guard-test.sh` and
  `guard.sh` treating `grep`/`rg` patterns as data (Decisions 5).

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
    index stay; the engine goes with `omarchy pkg drop jax-seldon`, since
    round 2), **Development**, **Project home and licence**.
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
  - the 15 `seldon` forms are copied from CONTRACT.md "Commands the plugin
    may run"; since round 2 the probe line reads `seldon --version --json`
    as the code runs it (Service.qml:166), matching the CONTRACT.md edit
    due at merge;
  - `rebuild` / `update-impact` are allowed by `Model.validateArgs` but
    never issued; the README says so;
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
- **`omarchy pkg --help` was denied** in round 1 (permission prompt
  declined); I did not retry it. In round 2 the Remove section uses
  `omarchy pkg drop jax-seldon` as the reviewer decided. That form comes
  from the review, not from a help output I read.

## Verified by

Round 2, at `49a861c`:

```
$ cmp LICENSE plugin/LICENSE                    → identical
$ omarchy plugin validate plugin/               → exit 0
$ just check                                    → exit 0
  validate-fixtures: ok — 109 instances, 8 variants, 23 self-checks
  plugin-validate: ok · tokens: ok (520 references) · qmllint: ok (28 files)
  model.test.js: 72 passed · service-states: 189 passed
  panel-view: 548 passed, 0 failed · overlay-view: 312 passed, 0 failed
  plugin-test: ok · check: ok
$ diff <(contract forms) <(README security forms)
  → one line: `seldon --version` (contract) vs `seldon --version --json`
    (README); closes with the CONTRACT.md edit at merge
$ per-table pipe count in README.md             → 10 tables consistent
$ find plugin -type l | wc -l                    → 0
```

Round 1:

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

Taken at review (round 1 items, for the record):
1. CONTRACT.md lists `seldon --version --json` (reviewer's edit at
   merge); the README states that form, without the footnote.
2. `plugin/LICENSE` is copied in this WP (done).
3. SPEC-PLUGIN §10 names `wl-copy` and the terminal launcher (reviewer's
   edit).
4. Engine removal: `omarchy pkg drop jax-seldon`, `yay -R jax-seldon` as
   the alternative (done).

Open:
5. **Guard false positive on search patterns.** `scripts/guard.sh` blocks
   a read-only `grep` whose pattern contains a package-command string
   (here `yay -R`). Proposal: a guard-test row with such a grep that must
   pass, then the fix in `guard.sh`. This is the orchestrator's call; I
   did not touch the guard.

## Touched outside WP scope

- `tests/plugin/panel-view.sh`: step 23 (`PANEL_SHOTS`, opt-in; the
  "panel equivalent" of `OVERLAY_SHOTS` the WP refers to did not exist).
- `docs/images/` (new): the two source renders of `preview.png`.
- `plugin/LICENSE` (round 2, by review decision).
