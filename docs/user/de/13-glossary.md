# Glossar

<!-- source: en/13-glossary.md @ e1563b04 -->

Jeder Begriff von Seldon in einer Tabelle, mit dem englischen Wort, das
die Oberfläche und die englische Anleitung verwenden. Befehle,
Dateinamen und Werte im Code bleiben in beiden Sprachen englisch.

| Englisch | Deutsch | Bedeutung |
|---|---|---|
| active case | aktiver Case | der zuletzt gestartete Case; Agenten-Hooks versehen ihre Befehle mit ihm (`.seldon/active-case`) |
| area | Bereich | ein langlebiges Thema der Maschine, `areas/<area>/` (`hyprland`, `themes`, …) |
| arm, disarm | scharf schalten, entschärfen | das erste Enter einer schreibenden Aktion auf dem Desk schaltet sie scharf, das zweite sendet sie; jede andere Taste entschärft sie |
| attention | zur Kenntnis | Drift, die leise aufgelistet wird: ein Paket, mit Namen installiert, ein Plugin eines Dritten, eine Überschreibung; niemand muss sie erklären |
| backfill | Nacherfassung | Änderungen von vor dem Logbuch aufzeichnen, mit `seldon init --since` |
| baseline | Baseline | das Verwerfen aller Drift, die eine Nacherfassung gefunden hat, mit dem Grund „before Seldon“ („pre-Seldon baseline“ vor 0.2.0) |
| capture | Erfassung | ein Lauf der Collectors; `seldon capture` |
| case | Case | eine geplante Änderung, `C-YYYY-NNN`, eine Datei in `work/` |
| collector | Collector | ein Teil der Engine, der eine Quelle liest: `snapper`, `pacman`, `omarchy`, `plugins`, `theme`, `config` |
| crisis | Krise | eine Änderung ohne Case, die Boot, Anmeldung, die Shell oder die Sicherheit brechen kann; die einzige Drift, die die Pill zählt |
| decision | Entscheidung | ein ADR in `decisions/`, angelegt mit `seldon decide` |
| desk | Desk (der Desk) | die eine Oberfläche des Plugins ab 0.2.0: ein Header mit dem Status-Chip und der KPI-Leiste, die Hinweise, eine Seitenleiste mit den Abschnitten, eine Liste und ein Detail; die Pill und `omarchy-shell shell toggle jax.seldon` öffnen ihn |
| deviation | Abweichung | eine Datei, die du gegenüber Omarchys Vorgabe geändert hast, gelistet in `system/deviations.md` |
| dismiss | verwerfen | Drift als nicht case-würdig auflösen, mit einem Grund |
| dossier | Dossier | `system/`, die Maschine, wie sie jetzt ist |
| drift | Drift | eine Änderung ohne Case und ohne Auflösung, die einen Blick wert ist: zur Kenntnis oder Krise |
| drift sheet | Drift-Dialog | das Formular des Desks, um einen Drift-Eintrag zu verknüpfen, zu erklären oder zu verwerfen; es öffnet sich im Detail der Änderung im Changelog |
| engine | Engine | das Programm `seldon`, das als einziges ins Logbuch schreibt |
| event | Ereignis | eine Zeile im Ledger |
| explain | erklären | Drift auflösen, indem ein abgeschlossener Case für sie entsteht |
| graph | Graph | Abschnitt 8 des Desks: Bereiche, Cases, Entscheidungen und Änderungen als Knoten mit ihren Verbindungen, Tag für Tag abgespielt |
| hook | Hook | ein Befehl, den der Harness eines Agenten vor jedem Werkzeugaufruf oder zu Beginn und Ende einer Sitzung startet |
| index | Index | `~/.local/state/seldon/index.json`, die einzige Datei, die das Plugin liest |
| journal | Journal | Tagesnotizen, `journal/YYYY/YYYY-MM-DD.md` |
| ledger | Ledger | die Ereignisse, `ledger/YYYY-MM.jsonl`, wird nur ergänzt |
| link | verknüpfen | Drift auflösen, indem sie einem Case zugeordnet wird |
| logbook | Logbuch | der Ordner mit der Aufzeichnung, standardmäßig `~/Seldon` |
| memory | Memory | was Agenten gelernt haben, `memory/`; auch Abschnitt 6 des Desks |
| notice | Hinweis | eine Meldung unter dem Header des Desks mit dem Knopf, der sie behebt; der Status-Chip im Header zeigt die erste |
| overlay | Overlay | nur 0.1.x: der bildschirmfüllende Prime Radiant; ab 0.2.0 Abschnitt 7 des Desks |
| panel | Panel | nur 0.1.x: das Popup der Leiste mit sechs Tabs; ab 0.2.0 ersetzt es der Desk |
| pill | Pill | das Bar-Widget: das Seldon-Zeichen, dann `A · D` |
| Plan, the | der Plan | alle offenen Cases (das Wort aus *Foundation*) |
| plugin | Plugin | `jax.seldon`, das Plugin für die Omarchy-Shell |
| Prime Radiant | Prime Radiant | die Diagramme über einen Zeitraum (30 d, 90 d, 365 d, All), Abschnitt 7 des Desks; bis 0.1.x das bildschirmfüllende Overlay |
| proposed case | vorgeschlagener Case | der offene Case, dessen *Plan* Paket, Pfad oder Theme eines Drift-Ereignisses nennt; im Drift-Dialog vorgewählt |
| redaction | Schwärzung | Geheimnisse entfernen, bevor ein Ereignis geschrieben wird |
| risk | Risiko | wie schwer eine Änderung rückgängig zu machen ist, `R0` bis `R3` |
| routine | Routine | eine Änderung ohne Case, die Geschichte ist, keine Drift: ein Theme-Wechsel, ein Schalter, ein einfaches System-Upgrade |
| section | Abschnitt | eines der neun Ziele des Desks in der Seitenleiste: Today (1), Changelog (2), Work (3), Decisions (4), System (5), Memory (6), Prime Radiant (7), Graph (8), Settings (`,`); die Namen bleiben englisch |
| setup card | Einrichtungskarte | die Karte auf Today, bis Seldon eingerichtet ist: Engine installieren, Logbuch anlegen, Snapshots lesen (optional) |
| snapshot | Snapshot | ein Snapper-Snapshot des Dateisystems |
| trace | Spur | die Ereignisse eines Case, in ihrer Reihenfolge |
| transaction group | Transaktionsgruppe | die Pakete einer Paket-Transaktion, als ein Drift-Eintrag gezeigt und aufgelöst |
| triage | Triage | der Vorschlag des Agenten für die offenen Änderungen, „Ask agent to sort N changes“ im Changelog; *Apply proposals* wendet ihn in deinem Namen an |
| verification | Prüfung | der Status eines Case zwischen aktiv und abgeschlossen; die Arbeit wartet auf deine Kontrolle |
| watched paths | beobachtete Pfade | die Dateien und Ordner, die der Config-Collector hasht (`watchPaths`) |
| zone | Zone | wie tief eine Änderung ins System greift: `green`, `yellow`, `red` (grün, gelb, rot) |

---

Zurück: [FAQ](12-faq.md) · [Übersicht](README.md)
