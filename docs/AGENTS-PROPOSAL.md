# AGENTS-PROPOSAL.md — Proposed AGENTS.md rows for the app (for the operator)

AGENTS.md "is changed only on the operator's explicit instruction". This
page is not a change: it lists the rows that
[ADR-0056](../decisions/ADR-0056-seldon-is-an-omarchy-app.md),
[ADR-0057](../decisions/ADR-0057-ui-strings-through-catalogs.md) and
[ADR-0058](../decisions/ADR-0058-seldon-open-is-the-app.md) would need,
each as *today* → *proposed*, so the operator can accept, change or
refuse them one by one. Nothing here applies until the operator says so.

## §1 Who and what (follows from the others; not in the decision's list)

- Today: "**Project:** Seldon — engine (Rust), plugin (QML), logbook spec,
  contract."
- Proposed: "**Project:** Seldon — engine (Rust), app (C++/Qt Quick),
  plugin (QML), logbook spec, contract."

## §2 Language policy

- Today: "UI labels are English for v1 (i18n is a later ADR)."
- Proposed: "UI strings are written in English, the source language, and
  go through catalogs (ADR-0057): `Commons.Translations` with
  `plugin/i18n/<code>.json` in the pill, `qsTr` and the template
  `Translator` with `app/src/i18n/<code>.json` in the app. Engine logs,
  `--json` keys and values, IPC values and identifiers stay English.
  Logbook content is never translated."

## §3 Contract first

- Today: "The plugin reads **only** `~/.local/state/seldon/index.json`
  (and the files it points to). It never parses Markdown, YAML or logs."
- Proposed: "Every Seldon surface — the app and the plugin — reads
  **only** `~/.local/state/seldon/index.json` (and the files it points
  to) and the `--json` output of the engine commands it runs. None parses
  Markdown, YAML or logs."
- Today: "both sides updated in the same PR …"
- Proposed: "every reader updated (the app and the plugin) in the same PR
  or in PRs that are merged together."
- Today: "The plugin team develops against the fixture; the engine team
  keeps it true."
- Proposed: "The app and the plugin are developed against the fixture;
  the engine keeps it true."

## §4 Repository layout

- Proposed new row, between `engine/` and `plugin/`:

  ```
  ├── app/             C++/Qt Quick app `seldon-desk` (qmake6; ADR-0056)
  ```

- And at the top level: `bin/` (`build`, `test`, `install`, the
  omarchy-app loop's wrappers, WP-230).

## §6 Zones

- Proposed sentence after the E41 clause: "Reading
  `~/.config/omarchy/language` (one code) to choose the UI language is
  display, not collection: the app reads it as every Omarchy app does,
  the plugin follows `Commons.I18n.language`, nothing of it enters the
  logbook or the index (ADR-0057 §6)."

## §7 Engineering rules

- Today (Engine): "No async runtime. No network. Static musl release
  build."
- Proposed: "No async runtime. No network. Static musl release build
  for the engine (the app links Qt dynamically, ADR-0056 §9)."
- Today (Engine): "Every command supports `--json`. Exit codes: 0 ok, 1
  user error, 2 engine error, 3 logbook not initialised, 4 lock held."
- Proposed: unchanged, plus "a recorded exception to the omarchy-app
  skill's 0/1 (ADR-0058 §5)".
- Proposed new block **App (C++/Qt Quick)**:
  - Read the omarchy-app skill (`$OMARCHY_PATH/default/agents/skills/
    omarchy-app/`) before any app work; Seldon's recorded exceptions are
    in ADR-0056 §9, everything else in the skill applies as written.
  - qmake6 and make, flat `src/`, no CMake, nothing generated committed;
    `app/bin/build` and `app/bin/test` must pass on every commit that
    touches `app/`.
  - The templates' `Theme`, `Translator` and portal file picker, with
    their MIT attribution; no hex colour outside the `Theme` fallback;
    `theme.radius` on every panel, dialog and button.
  - C++ owns the window's state, the index watch and the engine calls
    (`QProcess`, fixed argument lists, never a shell string); QML owns
    layout and keys. Never block the GUI thread: the index is parsed off
    it and bounded at 1 MB.
  - No Qt API newer than the `QT_MIN` gate (6.9); no `qs.Ui`, no import
    from Omarchy's shell tree, no symlink.
  - Tests offscreen (`QT_QPA_PLATFORM=offscreen`) with a temp `HOME` and
    a private `XDG_RUNTIME_DIR`, as §6 asks of every harness; no QML
    warning with `QT_FORCE_STDERR_LOGGING=1`.
  - The app renders the four required states with a one-click fix each,
    like the plugin today.
- Plugin block: "Plugin (QML)" keeps its rules for the pill; the line
  "The plugin must render a useful state when …" moves to the app, the
  pill keeps a tooltip state for each.
- Today (Plugin): "Read `docs/skills/omarchy-ux/SKILL.md` before any
  plugin work; SPEC-PLUGIN wins where they differ."
- Proposed: "… shipped Omarchy skills rank above Seldon's house rules
  wherever Seldon records no exception (ADR-0056 §10)."

## Not drafted here

PROJECT.md's goal and definition of done change too (the one job,
ADR-0056 §2); that is the operator's text to write.
