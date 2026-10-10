# Glossary

Every Seldon term in one table, with the German word the German guide
uses. Commands, file names and values in code stay English in both
languages.

| Term | German | Meaning |
|---|---|---|
| active case | aktiver Case | the case started last; agent hooks stamp their commands with it (`.seldon/active-case`) |
| area | Bereich | a long-lived topic of the machine, `areas/<area>/` (`hyprland`, `themes`, …) |
| arm, disarm | scharf schalten, entschärfen | the first Enter of a writing action on the desk arms it, the second sends it; any other key disarms it |
| attention | zur Kenntnis | drift that is listed quietly: a package installed by name, a third-party plugin, an override; nobody has to explain it |
| backfill | Nacherfassung | recording changes from before the logbook, with `seldon init --since` |
| baseline | Baseline | the dismissal of all drift a backfill found, with the reason "before Seldon" ("pre-Seldon baseline" before 0.2.0) |
| capture | Erfassung | one run of the collectors; `seldon capture` |
| case | Case | one planned change, `C-YYYY-NNN`, a file in `work/` |
| collector | Collector | a part of the engine that reads one source: `snapper`, `pacman`, `omarchy`, `plugins`, `theme`, `config` |
| crisis | Krise | a change without a case that can break boot, login, the shell or security; the only drift the bar counts |
| decision | Entscheidung | an ADR in `decisions/`, made with `seldon decide` |
| desk | Desk (der Desk) | the plugin's one surface from 0.2.0: a header with the status chip and the KPI strip, the notices, a sidebar with the sections, a list and a detail; the pill and `omarchy-shell shell toggle jax.seldon` open it |
| deviation | Abweichung | a file you changed against Omarchy's default, listed in `system/deviations.md` |
| dismiss | verwerfen | resolve drift as not needing a case, with a reason |
| dossier | Dossier | `system/`, the machine as it is now |
| drift | Drift | a change with no case and no resolution that is worth a look: attention or crisis |
| drift sheet | Drift-Dialog | the desk's form to link, explain or dismiss a drift item; it opens in the change's detail in the Changelog |
| engine | Engine | the program `seldon`, the only writer of the logbook |
| event | Ereignis | one line in the ledger |
| explain | erklären | resolve drift by creating a completed case for it |
| graph | Graph | section 8 of the desk: areas, cases, decisions and changes as nodes with their links, replayed day by day |
| hook | Hook | a command an agent's harness runs before each tool call or at session start and end |
| index | Index | `~/.local/state/seldon/index.json`, the only file the plugin reads |
| journal | Journal | daily notes, `journal/YYYY/YYYY-MM-DD.md` |
| ledger | Ledger | the events, `ledger/YYYY-MM.jsonl`, append-only |
| link | verknüpfen | resolve drift by attaching it to a case |
| logbook | Logbuch | the folder that holds the record, `~/Seldon` by default |
| memory | Memory | what agents learned, `memory/`; also the desk's section 6 |
| notice | Hinweis | a message under the desk's header with the button that fixes it; the header's status chip shows the first one |
| overlay | Overlay | 0.1.x only: the fullscreen Prime Radiant; section 7 of the desk from 0.2.0 |
| panel | Panel | 0.1.x only: the bar popup with six tabs; the desk replaces it from 0.2.0 |
| pill | Pill | the bar widget: the Seldon mark, then `A · D` |
| Plan, the | der Plan | all open cases (the word from *Foundation*) |
| plugin | Plugin | `jax.seldon`, the Omarchy shell plugin |
| Prime Radiant | Prime Radiant | the charts over a period (30 d, 90 d, 365 d, All), section 7 of the desk; the fullscreen overlay until 0.1.x |
| proposed case | vorgeschlagener Case | the open case whose *Plan* names a drift event's package, path or theme; preselected in the drift sheet |
| redaction | Schwärzung | removing secrets before an event is written |
| risk | Risiko | how hard a change is to undo, `R0` to `R3` |
| routine | Routine | a change without a case that is history, not drift: a theme switch, a toggle, a plain system upgrade |
| section | Abschnitt | one of the desk's nine targets in the sidebar: Today (1), Changelog (2), Work (3), Decisions (4), System (5), Memory (6), Prime Radiant (7), Graph (8), Settings (`,`); the names stay English |
| setup card | Einrichtungskarte | the card on Today until Seldon is set up: install the engine, create the logbook, read snapshots (optional) |
| snapshot | Snapshot | a snapper snapshot of the file system |
| trace | Spur | the events of one case, in order |
| transaction group | Transaktionsgruppe | the packages of one package transaction, shown and resolved as one drift item |
| triage | Triage | the agent's proposal for the open changes, "Agent sorts N open changes" in the Changelog; *Apply proposals* applies it as you |
| verification | Prüfung | the case status between active and completed; the work waits for your check |
| watched paths | beobachtete Pfade | the files and folders the config collector hashes (`watchPaths`) |
| zone | Zone | how much a change touches the system: `green`, `yellow`, `red` (grün, gelb, rot) |

---

Previous: [FAQ](12-faq.md) · [Index](README.md)
