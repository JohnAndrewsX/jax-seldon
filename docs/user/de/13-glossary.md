# Glossar

<!-- source: en/13-glossary.md @ 4ce0827 -->

Jeder Begriff von Seldon in einer Tabelle, mit dem englischen Wort, das
die Oberfläche und die englische Anleitung verwenden. Befehle,
Dateinamen und Werte im Code bleiben in beiden Sprachen englisch.

| Englisch | Deutsch | Bedeutung |
|---|---|---|
| active case | aktiver Case | der zuletzt gestartete Case; Agenten-Hooks versehen ihre Befehle mit ihm (`.seldon/active-case`) |
| area | Bereich | ein langlebiges Thema der Maschine, `areas/<area>/` (`hyprland`, `themes`, …) |
| arm, disarm | scharf schalten, entschärfen | das erste Enter einer schreibenden Aktion im Panel schaltet sie scharf, das zweite sendet sie; jede andere Taste entschärft sie |
| backfill | Nacherfassung | Änderungen von vor dem Logbuch aufzeichnen, mit `seldon init --since` |
| baseline | Baseline | das Verwerfen aller Drift, die eine Nacherfassung gefunden hat, mit dem Grund „pre-Seldon baseline“ |
| capture | Erfassung | ein Lauf der Collectors; `seldon capture` |
| case | Case | eine geplante Änderung, `C-YYYY-NNN`, eine Datei in `work/` |
| collector | Collector | ein Teil der Engine, der eine Quelle liest: `snapper`, `pacman`, `omarchy`, `plugins`, `theme`, `config` |
| crisis | Krise | Drift in der roten Zone |
| decision | Entscheidung | ein ADR in `decisions/`, angelegt mit `seldon decide` |
| deviation | Abweichung | eine Datei, die du gegenüber Omarchys Vorgabe geändert hast, gelistet in `system/deviations.md` |
| dismiss | verwerfen | Drift als nicht case-würdig auflösen, mit einem Grund |
| dossier | Dossier | `system/`, die Maschine, wie sie jetzt ist |
| drift | Drift | eine Änderung ohne Case und ohne Auflösung |
| drift sheet | Drift-Dialog | das Formular des Panels, um einen Drift-Eintrag zu verknüpfen, zu erklären oder zu verwerfen |
| engine | Engine | das Programm `seldon`, das als einziges ins Logbuch schreibt |
| event | Ereignis | eine Zeile im Ledger |
| explain | erklären | Drift auflösen, indem ein abgeschlossener Case für sie entsteht |
| hook | Hook | ein Befehl, den der Harness eines Agenten vor jedem Werkzeugaufruf oder zu Beginn und Ende einer Sitzung startet |
| index | Index | `~/.local/state/seldon/index.json`, die einzige Datei, die das Plugin liest |
| journal | Journal | Tagesnotizen, `journal/YYYY/YYYY-MM-DD.md` |
| ledger | Ledger | die Ereignisse, `ledger/YYYY-MM.jsonl`, wird nur ergänzt |
| link | verknüpfen | Drift auflösen, indem sie einem Case zugeordnet wird |
| logbook | Logbuch | der Ordner mit der Aufzeichnung, standardmäßig `~/Seldon` |
| memory | Memory | was Agenten gelernt haben, `memory/` |
| panel | Panel | das Panel der Bar mit sechs Tabs |
| pill | Pill | das Bar-Widget: das Seldon-Zeichen, dann `A · D` |
| Plan, the | der Plan | alle offenen Cases (das Wort aus *Foundation*) |
| plugin | Plugin | `jax.seldon`, das Plugin für die Omarchy-Shell |
| Prime Radiant | Prime Radiant | das bildschirmfüllende Overlay mit den Diagrammen |
| proposed case | vorgeschlagener Case | der offene Case, dessen *Plan* Paket, Pfad oder Theme eines Drift-Ereignisses nennt; im Drift-Dialog vorgewählt |
| redaction | Schwärzung | Geheimnisse entfernen, bevor ein Ereignis geschrieben wird |
| risk | Risiko | wie schwer eine Änderung rückgängig zu machen ist, `R0` bis `R3` |
| snapshot | Snapshot | ein Snapper-Snapshot des Dateisystems |
| trace | Spur | die Ereignisse eines Case, in ihrer Reihenfolge |
| transaction group | Transaktionsgruppe | die Pakete einer Paket-Transaktion, als ein Drift-Eintrag gezeigt und aufgelöst |
| verification | Prüfung | der Status eines Case zwischen aktiv und abgeschlossen; die Arbeit wartet auf deine Kontrolle |
| watched paths | beobachtete Pfade | die Dateien und Ordner, die der Config-Collector hasht (`watchPaths`) |
| zone | Zone | wie tief eine Änderung ins System greift: `green`, `yellow`, `red` (grün, gelb, rot) |

---

Zurück: [FAQ](12-faq.md) · [Übersicht](README.md)
