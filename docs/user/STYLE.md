# STYLE.md — How the user guide is written

This page is for people who write or translate the user guide in
`docs/user/`. Readers of the guide never need it.

## Voice

- Talk to the reader: "you", "your logbook". German: "du", lower case
  in running text, like the logbook's own `AGENTS.md` template.
- Present tense. "The engine writes the case", not "will write".
- Short sentences. One idea per sentence. Split a sentence that needs a
  semicolon.
- Active voice. Name who does what: you, the engine, the plugin, the agent.
- Say what happens, then why, if the why helps the reader act.
- Lead with the command or the action. Background comes after it, or on
  the concepts page.
- Write for a skeptical first-time reader. Say what Seldon does not do
  as plainly as what it does.
- No sales words, no "simply", no "just", no "easy". If a step is easy,
  the reader notices.
- No dashes as connectors between clauses. Use a full stop, a comma, a
  colon or parentheses.
- Bold only for the one word a reader must not miss on a page. Never bold
  every list item.
- Headings in sentence case, no decoration, no emoji.
- Straight quotes `"…"` in English. German uses „…“ in prose. Code is
  never retyped with typographic quotes.

## Pages

- Every page starts with an H1 title and one short paragraph that says
  what the page covers.
- Pages are numbered (`01-…` to `13-…`) and keep their file names in
  every language. File names, anchors in code and commands stay English.
- The last line of every page is a navigation line: previous page, index,
  next page.
- English is the source. A translated page keeps the same headings in the
  same order and at the same levels, the same code blocks, the same
  images and the same tables. `just docs-check` compares the structure.
- A translated page starts with a source line, as an HTML comment, right
  after the title:
  `<!-- source: en/01-getting-started.md @ <commit> -->`. The commit is
  the last commit that changed the English page when the translation was
  made. `just docs-check` warns when the English page has changed since.

## Terms

Use one word for one thing. The German column is binding for `de/`.
Names of UI elements (tab labels, buttons, banner texts) stay English
in every language, because the plugin's UI is English in v1. Explain
them in the page language where it helps.

| English | German | Note |
|---|---|---|
| logbook | Logbuch | the folder, `~/Seldon` by default |
| ledger | Ledger | `ledger/*.jsonl`, the machine-written part |
| journal | Journal | daily notes, `journal/` |
| event | Ereignis | one ledger line |
| case | Case (der Case, die Cases) | as in the logbook template |
| queued, active, verification, completed, dropped | geplant, aktiv, in Prüfung, abgeschlossen, aufgegeben | case statuses; the value in code stays English |
| zone (green, yellow, red) | Zone (grün, gelb, rot) | the value in code stays English: `--zone red` |
| risk (R0–R3) | Risiko (R0–R3) | |
| drift | Drift (die Drift) | a change without a case and without a resolution |
| routine | Routine | a change without a case that is history, not drift (ADR-0028) |
| attention | zur Kenntnis (Punkt zur Kenntnis) | drift listed quietly (ADR-0028) |
| crisis | Krise | a change without a case that can break boot, login, the shell or security (ADR-0028); no longer "drift in the red zone" |
| baseline | Baseline | the pre-Seldon baseline after a backfill |
| backfill | Nacherfassung | `init --since` |
| link, explain, dismiss | verknüpfen, erklären, verwerfen | the three drift actions |
| proposed case | vorgeschlagener Case | the case whose *Plan* names a drift event's subject |
| transaction group | Transaktionsgruppe | one package transaction as one drift item |
| decision | Entscheidung | an ADR in `decisions/` |
| memory | Memory | what agents learned, `memory/`; also the panel tab |
| capture | Erfassung, erfassen | `seldon capture` |
| collector | Collector | |
| watched paths | beobachtete Pfade | `watchPaths` |
| index | Index | `~/.local/state/seldon/index.json` |
| dossier | Dossier | `system/` |
| deviation | Abweichung | a row in `system/deviations.md` |
| redaction | Schwärzung | |
| area | Bereich | `areas/<area>/` |
| trace | Spur | the events of one case, in order |
| engine | Engine | the `seldon` program |
| plugin | Plugin | `jax.seldon` |
| Prime Radiant | Prime Radiant | the fullscreen overlay |
| pill | Pill | the bar widget |
| panel | Panel | the bar panel with six tabs |
| hook | Hook | |
| snapshot | Snapshot | snapper snapshot |
| sheet (drift sheet) | Dialog (Drift-Dialog) | the plugin's form for drift, a new case or a decision |
| arm, disarm (two-press arming) | scharf schalten, entschärfen | the first Enter of a writing action |

## Commands

- A command the reader runs goes in a fenced block marked `sh`, one
  command per line, no prompt sign. A comment after `#` may explain it.
- Output goes in a separate block marked `text`. Shorten it with `…` and
  say so.
- Placeholders are in angle brackets and capitals where the help text
  uses them: `<ID>`, `<EVENT>`, `<DIR>`. Explain each one the first time
  it appears on a page.
- Free text goes after `--`: `seldon log -- "Text"`. The guide always
  shows it this way, even where the engine would accept it without.
- Commands, flags, file names, paths, config keys and values are code:
  `seldon drift`, `--json`, `config.toml`, `~/Seldon`.
- Never break a command inside a code span across two lines. Wrap the
  sentence before the backtick.
- Commands are never translated. Comments in code blocks and text in
  quotes that the reader types (a case title, a note) are translated.
- Paths are written with `~`. Private paths, host names and real package
  lists never appear. A machine name is `<machine>`.
- Case ids in examples start at `C-2026-001`. Event ids are examples and
  say so.
- Every command and flag in the guide exists in `seldon <command> --help`.
  The CLI reference is the generated proof; `just docs-check` keeps it
  equal to the engine's help.

## Screenshots

- Reuse the renders in the repository (`docs/images/`, `plugin/preview.png`).
  They show the sample index in the Tokyo Night theme. Do not take new
  screenshots of a real desktop.
- Link them by relative path. Never copy a picture into `en/` or `de/`.
- Alt text is in the page language. It says what the picture shows and
  what the reader should notice, in one or two sentences.
- Put a short caption in italics under a picture when the picture shows
  sample data. Readers must not mistake the sample for their own machine.

## The CLI reference

- `05-cli-reference.md` holds the help text of every command between
  `<!-- help: seldon … -->` and `<!-- /help -->` markers. The text is
  generated: `bash scripts/docs-check.sh --write` fills it from the
  engine. Never edit it by hand.
- In each block the global options (`--json`, `--logbook`, `--quiet`,
  `--no-commit`, `--config`, `--help`) are left out when their text is the
  same as in `seldon --help`. The page lists them once.
- The prose around the blocks is written by hand and translated.

## Checks

`just docs-check` (part of `just check`) fails when:

- a relative link or image does not resolve, or an anchor in a link to a
  Markdown file does not exist;
- `en/` and `de/` hold different pages;
- a pair of pages differs in heading levels, code blocks, tables or images;
- a German page has no source line;
- an image has no alt text;
- a help block differs from the engine's `--help`, or a command has no
  block.

It warns, and does not fail, when a German page's source commit is older
than the last change to its English page.
