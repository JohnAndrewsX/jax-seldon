# ADR-0057 — UI strings through catalogs: English is the source, the pill and the app follow Omarchy's language

**Status:** proposed (E96 delegation; operator accepts)
**Date:** 2026-10-14

> Supersedes in part [ADR-0007](ADR-0007-language-policy.md): its
> consequence "UI i18n is deferred (future ADR)" — this is that ADR.
> ADR-0007's decision stands: the repository, code, specs, engine
> messages and `--json` keys are English; logbook content is in the
> user's language; templates ship in English and German. Companion of
> [ADR-0056](ADR-0056-seldon-is-an-omarchy-app.md). No contract change.

## Context

1. **Omarchy runs in 50 languages** (English plus 49; edge, "Quattro RS
   4.5"). The user's choice is one code in `~/.config/omarchy/language`
   (missing or unknown means `en`); `omarchy language <code>` also sets
   the session `LANG`. The registry is `$OMARCHY_PATH/default/i18n/
   locales.json` (format locale, glibc locale, decimal, zero digit,
   `direction: "rtl"` for `ar`, `he`, `fa`, `ur`). There is no plain `es`
   (`es-MX` instead); fallback runs `pt-BR` → `pt` → `en`.
2. **Two mechanisms, one per surface.**
   - Shell plugins: `Commons.Translations { directory:
     Qt.resolvedUrl("i18n") }` with flat `i18n/<code>.json` beside the
     QML; whole sentences with `%1`, `tn()` for counts,
     `Commons.I18n.formatNumber`, dates through
     `Qt.locale(Commons.I18n.formatLocale)`, `LayoutMirroring` and
     `Util.readingKey` for right-to-left. Plugins must not use
     `Commons.I18n.t()` for their own strings (Omarchy's keys can
     change). Manifest names and descriptions are not translated yet.
   - Apps (omarchy-app skill): `qsTr()`/`tr()`, `%1` with `.arg()`,
     `qsTr("%n …", "", count)` for plurals, `src/i18n/<code>.json`
     compiled into the binary, the template `Translator` reading
     `~/.config/omarchy/language` live and setting the default `QLocale`
     and layout direction; numbers and dates through `Qt.locale()`; the
     content under the template's mirrored root; translated `.desktop`
     `Name[xx]` and `Comment[xx]`.
3. **Seldon today** has no catalog, no `qsTr`, about 81 `text: "…"`
   literals in QML and about 260 English strings in `Model.js`, glued
   plurals ("min ago", "days ago") and its own English month and weekday
   names. AGENTS.md §2 says "UI labels are English for v1"; ADR-0007
   deferred UI i18n.
4. **The skill is ahead of its own apps.** None of Hype, Monologue,
   Omacut or Signed ships a `Translator` yet; Omarchy's own 49 catalogs
   were made by agents and reviewed by sampling.
5. **E41** limits what Seldon reads under `~/.config`: paths and
   modification times, no content, no hashes. Reading the one-line
   language file is not collection: nothing reaches the ledger, the
   index or any event; it decides only how the window speaks.

## Decision

1. **English is the source language** of every UI string and stays the
   repository language. UI strings go through catalogs; no visible
   string is a bare literal in QML or a concatenation in JavaScript.
2. **The pill** (`jax.seldon` after ADR-0056) uses `Commons.Translations`
   with `plugin/i18n/<code>.json`, follows `Commons.I18n.language` live,
   and never reads the language file itself. Counts use `tn()`, numbers
   `Commons.I18n.formatNumber`. The manifest's name and description stay
   English (the plugin system does not translate them).
3. **The app** (`seldon-desk`) uses `qsTr()`/`tr()` and the template
   `Translator` with `app/src/i18n/<code>.json` compiled into the binary;
   it reads `~/.config/omarchy/language` as every omarchy-app does,
   switches live, mirrors its content for right-to-left languages, and
   formats numbers and dates through `Qt.locale()`. The `.desktop` file
   carries `Name[xx]` and `Comment[xx]` for every shipped language.
4. **`Model.js` returns keys and arguments, not sentences,** wherever a
   string reaches the screen; the view translates. Its own English month
   and weekday tables and glued plurals go; dates come from the locale.
5. **The engine keeps `LANG`** (as today, `LC_ALL`/`LC_MESSAGES`/`LANG`
   choose `de` or `en` for `init`'s templates). Omarchy sets `LANG` from
   the same choice, so the three surfaces agree without the engine
   reading the file. Engine log and error messages, `--json` keys and
   values, IPC values, stored and compared values stay English.
6. **The language-file read is display, not collection.** It is not an
   extension of E41: no content of `~/.config` enters the record. AGENTS.md
   §6 should say so in one sentence (proposed row in
   [docs/AGENTS-PROPOSAL.md](../docs/AGENTS-PROPOSAL.md)).
7. **Languages by release.** 0.5.0 ships `en` and `de`, made and reviewed
   by the team (the user guide and the logbook templates are already
   German). 0.6.0 adds the other 48 of Omarchy's registry, agent-made and
   sample-reviewed, as Omarchy's own catalogs were; a missing string
   falls back to English per string. A language added later is a patch
   release.
8. **Never translated:** brand names (Seldon, Omarchy, Prime Radiant),
   units, keyboard keys, command names and their arguments, logbook
   content (it is shown verbatim in whatever language the user wrote it,
   ADR-0007), identifiers (`C-2026-004`, `ADR-0056`).

## Consequences

- AGENTS.md §2's sentence "UI labels are English for v1 (i18n is a later
  ADR)" is replaced (proposed row); the logbook-content sentence stands.
- The look loop gains a step (omarchy-app skill, loop 3): every app
  change is looked at once in another language (`omarchy language de`
  on the test host; the offscreen harness with `OMARCHY_LANGUAGE_FILE`
  pointed at a temp file) — longer words must not clip. On edge, Omarchy's
  `Ui/Button` elides a label held narrower than its natural width; the
  pill's few labels are measured in German too.
- A string-check joins the linters: the plugin and the app fail a check
  on a visible bare literal (the allow-list holds brand names and keys).
  The exact rule is a WP's job.
- `en.json` is the reference catalog; a key present in `en.json` and
  missing in another file is a fallback, never an error; a key used in
  code and missing in `en.json` is an error.
- The cost: every string of the app is written once more in German for
  0.5.0, and the 48 catalogs of 0.6.0 need a sampling review the team
  cannot do natively; their quality is Omarchy's own bar, stated as such
  in the release notes.
