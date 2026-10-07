# Konzepte

<!-- source: en/02-concepts.md @ 1bae1cd -->

Diese Seite erklärt die Ideen hinter Seldon: das Logbuch, Cases, Zonen,
Risiko, Drift, die Baseline, Krisen, Entscheidungen und Memory. Lies sie
einmal. Die anderen Seiten verwenden diese Wörter, ohne sie noch einmal
zu erklären.

## Die Idee

Deine Maschine ändert sich jeden Tag. Du installierst Pakete, Omarchy
aktualisiert sich, ein Agent ändert eine Konfigurationsdatei. Ein paar
Wochen später weiß niemand mehr, warum etwas da ist. Das Systemlog hat
die Fakten, aber nicht die Gründe. Ein handgeschriebenes Änderungsprotokoll
hat die Gründe, bis du aufhörst, es zu schreiben.

Seldon hält beides fest und vergleicht es:

- Es zeichnet auf, was sich geändert hat. Collectors lesen das Paketlog,
  Snapper, die Omarchy-Version, die Shell-Plugins, das Theme und deine
  Konfigurationsdateien. Agenten-Hooks zeichnen die Befehle auf, die ein
  Agent ausführt.
- Es hält fest, was du ändern willst. Du schreibst Cases: eine
  Markdown-Datei pro geplanter Änderung.
- Es zeigt den Unterschied. Eine Änderung, die kein Case abdeckt und die
  einen Blick wert ist, ist Drift. Du darfst sagen, was sie war; du musst
  es nie.

Seldon ist ein Rekorder. Es ändert nie dein System, startet nie einen
Paketmanager oder `sudo`, hält nie einen Befehl an und schickt nie etwas
übers Netz. Der Name stammt von Hari Seldon aus Asimovs *Foundation*: Der
Plan sagt voraus, und eine Krise ist die Stelle, an der die Wirklichkeit
den Plan verlässt.

## Die drei Teile

| Teil | Was es ist | Was es tut |
|---|---|---|
| Logbuch | ein Ordner, standardmäßig `~/Seldon` | hält die Aufzeichnung als Markdown und JSON-Zeilen; ein git-Repository |
| Engine | das Programm `seldon` | schreibt als einziges ins Logbuch; du und deine Agenten rufen es auf |
| Plugin | `jax.seldon` in der Omarchy-Bar | zeigt das Logbuch; jede Aktion ruft die Engine auf |

Das Plugin liest eine einzige Datei, den Index
(`~/.local/state/seldon/index.json`). Die Engine baut ihn nach jedem
Befehl neu. Du änderst ihn nie; wenn du ihn löschst, geht nichts
verloren, denn `seldon status` schreibt ihn wieder.

## Das Logbuch

Das Logbuch ist ein gewöhnlicher Ordner mit Markdown-Dateien. Du kannst
es in jedem Editor lesen, in Obsidian oder auf GitHub. Es enthält zwei
Arten von Aufzeichnung:

- Das Ledger (`ledger/YYYY-MM.jsonl`) schreibt die Maschine. Jede Zeile
  ist ein Ereignis: ein Paket installiert, ein Theme gewechselt, eine
  Konfigurationsdatei geändert, eine Notiz geschrieben, ein Case
  gestartet. Das Ledger wird nur ergänzt. Nichts darin wird je geändert
  oder gelöscht; eine Korrektur ist ein neues Ereignis.
- Das Journal (`journal/YYYY/YYYY-MM-DD.md`) schreiben Menschen und
  Agenten. Es hält das Warum fest: was du versucht und was du gelernt
  hast.

Seldon verbindet beides über Case-IDs: Ein Journal-Eintrag und die
Ereignisse, um die es darin geht, tragen denselben Case.

Jedes Ereignis hat eine Quelle. Es gibt neun:

| Quelle | Was sie aufzeichnet |
|---|---|
| `pacman` | installierte, entfernte, aktualisierte und zurückgestufte Pakete |
| `snapper` | angelegte und gelöschte Snapshots |
| `omarchy` | Wechsel der Omarchy-Version |
| `plugins` | Shell-Plugins: hinzugefügt, entfernt, aktiviert, deaktiviert, aktualisiert |
| `theme` | Theme-Wechsel |
| `config` | Dateien unter den beobachteten Pfaden: hinzugefügt, geändert, entfernt |
| `agent` | Befehle, die ein Agent ausgeführt hat, aufgezeichnet von einem Hook |
| `manual` | Notizen (`seldon log`) und Ereignisse, die du von Hand einträgst |
| `seldon` | die Schritte der Engine selbst: Case angelegt, gestartet, abgeschlossen, Drift aufgelöst |

[Das Logbuch](07-the-logbook.md) beschreibt jeden Ordner.

## Cases

Ein Case ist eine geplante Änderung in einer Markdown-Datei unter
`work/`. Er hat eine ID wie `C-2026-004`, einen Titel, eine Zone, ein
Risiko, einen Bereich und vier Abschnitte. Ein *Bereich* (area) ist ein
langlebiges Thema der Maschine, mit einem Ordner unter `areas/`. Ein
neues Logbuch hat sechs: `hyprland`, `themes`, `packages`, `dev-env`,
`plugins` und `shell`. Ein neuer Bereichsname legt seinen Ordner beim
ersten Gebrauch an. Die vier Abschnitte sind:

- *Intent*: warum, und was danach anders ist.
- *Plan*: Ziel, Schritte, betroffene Pfade, Rollback, Prüfung.
- *Log*: datierte Zeilen, nur ergänzt. Die Engine fügt für jeden Schritt
  eine hinzu.
- *Result*: was herausgekommen ist.

Du schreibst *Intent*, *Plan* und *Result*. Den Rest pflegt die Engine:
den Status, die Liste der verknüpften Ereignisse, die Agenten, die daran
gearbeitet haben.

Ein Case durchläuft diese Status. Die Engine verschiebt die Datei
zwischen den Ordnern; du verschiebst sie nie selbst.

| Status | Ordner | Befehl, der ihn dorthin bringt |
|---|---|---|
| queued (geplant) | `work/queued/` | `seldon plan new` |
| active (aktiv) | `work/active/` | `seldon plan start` |
| verification (in Prüfung) | `work/active/` | `seldon plan verify` |
| completed (abgeschlossen) | `work/completed/` | `seldon plan done` |
| dropped (aufgegeben) | `work/completed/` | `seldon plan drop` (aus queued, active oder verification) |

Die Reihenfolge steht fest: queued, active, verification, completed. Ein
abgeschlossener oder aufgegebener Case bleibt geschlossen. Jeder Schritt
ist ein Ereignis im Ledger und ein git-Commit.

Der zuletzt gestartete Case ist der *aktive Case*. Seine ID steht in
`.seldon/active-case`. Agenten-Hooks versehen jeden Befehl, den sie
aufzeichnen, mit ihm, und so sammelt der Case seine *Spur*: die
Ereignisse, die für ihn aufgezeichnet sind, in ihrer Reihenfolge.

## Zonen

Jedes Ereignis und jeder Case hat eine Zone. Die Zone sagt, wie tief die
Änderung ins System greift.

| Zone | Was hineinfällt |
|---|---|
| red (rot) | Pakete, Omarchy selbst, systemd-Units (auch Unit-Dateien unter `~/.config/systemd/`), `/etc`, der Bootloader |
| yellow (gelb) | Konfigurationsdateien unter den beobachteten Pfaden, Themes, Shell-Plugins |
| green (grün) | alles, was die Collectors nicht verfolgen: Projektdateien, Paketmanager von Programmiersprachen, `git` |

Snapshots, Notizen und die Schritte der Engine selbst haben keine Zone.
Grüne Änderungen zeichnen nur Agenten-Hooks auf, und nur, solange ein
Case aktiv ist.

## Risiko

Jeder Case hat außerdem ein Risiko, von `R0` bis `R3`. Das Risiko ist
deine Einschätzung, was es braucht, die Änderung rückgängig zu machen.

| Risiko | Bedeutung |
|---|---|
| R0 | in Sekunden umkehrbar, nichts hängt daran; du machst es von Hand rückgängig, kein Snapshot |
| R1 | in Minuten von Hand umkehrbar, mit einem bekannten Befehl; der Plan nennt den Rollback-Schritt |
| R2 | der Rollback braucht den Plan und einen Snapshot oder ein Backup; mit `--snapshot` starten, vor dem Abschluss prüfen |
| R3 | kann Boot, Login oder die Shell kaputtmachen; Snapshot Pflicht, dein ausdrückliches Okay für jeden Schritt, nie unbeaufsichtigt |

Seldon speichert das Risiko und zeigt es an. Es setzt es nicht durch. Ein
neuer Case beginnt mit gelb und `R1`, wenn du nichts anderes angibst.

## Drift

Jede Änderung wird aufgezeichnet. Drift ist eine aufgezeichnete
Änderung, die keinen Case hat, noch nicht aufgelöst ist und einen Blick
wert ist. Nur Änderungen können Drift sein: Pakete, Omarchy, Plugins,
Theme und Konfiguration. Snapshots, Notizen und Case-Schritte sind es
nie.

Die meisten Änderungen landen von selbst bei einem Case:

- Der Befehl eines Agenten trägt den aktiven Case, weil der Hook ihn ins
  Ereignis schreibt.
- Ein Paket, das als Abhängigkeit eines Pakets aus einem Case
  mitgekommen ist, gehört zu diesem Case.

Eine Änderung ohne Case wird danach eingeordnet, was eine falsche kosten
würde, nicht danach, wer sie gemacht hat:

| Klasse | Beispiele | Was passiert |
|---|---|---|
| Routine | ein Theme-Wechsel, ein Plugin-Schalter, ein einfaches System-Upgrade (`pacman -Syu`, `omarchy update`, Kernel eingeschlossen), Omarchys eigene Kopie einer Datei, `shell.json` | Geschichte im Changelog, keine Drift; niemand wird gefragt |
| zur Kenntnis | ein Paket, mit Namen installiert oder entfernt, ein Plugin eines Dritten, hinzugefügt oder aktualisiert, eine Überschreibung unter einem beobachteten Pfad, eine entfernte Datei | offene Drift, leise: das Panel listet sie, die Pill zählt sie nicht |
| Krise | ein Paket aus `alwaysRed`, mit Namen installiert oder entfernt, eine neue Datei in einem Persistenzpfad wie `~/.config/systemd/user` oder Omarchys Hooks | siehe [Krise](#krise) |

Deine eigenen Änderungen im Terminal werden aufgezeichnet wie alle
anderen: Ein Collector sieht, dass ein Paket dazugekommen ist, aber
nicht, dass du es für einen Case gemeint hast. Nennt ein offener Case das
geänderte Paket, den Pfad oder das Theme in seinem *Plan*, schlägt Seldon
diesen Case vor (der *vorgeschlagene Case*), und das Panel wählt ihn vor,
auch für eine Routine-Änderung.

Du darfst Drift auflösen, du musst es nie. Es gibt drei Arten:

| Aktion | Befehl | Nimm sie, wenn |
|---|---|---|
| verknüpfen (link) | `seldon drift link <EVENT> <CASE>` | die Änderung zu einem Case gehört (offen oder geschlossen) |
| erklären (explain) | `seldon drift explain <EVENT> -- "<warum>"` | sie einen Grund hatte, aber keinen Case; Seldon legt einen abgeschlossenen Case mit deinem Text als Titel an |
| verwerfen (dismiss) | `seldon drift dismiss <EVENT> -- "<grund>"` | sie keine Rolle spielt (eine Abhängigkeit, Rauschen) |

Das Ereignis bleibt in jedem Fall im Ledger. Die Auflösung ist ein neues
Ereignis, das darauf verweist. Pakete aus einer Transaktion bilden einen
Drift-Eintrag, eine *Transaktionsgruppe*: Die Installation eines Pakets
mit zehn Abhängigkeiten ist ein Eintrag, und ein Befehl löst alle auf.
Mit `--only` löst du nur dieses eine Ereignis auf.

Auch Agenten sehen die offenen Punkte. Der Kontext, mit dem jede
Agentensitzung beginnt, nennt die Krisen und die Punkte zur Kenntnis der
letzten sieben Tage. Ein Agent erklärt oder verknüpft einen Punkt nur,
wenn sein eigenes *Log*, ein Hook-Ereignis oder deine Worte belegen,
warum es passiert ist; sonst lässt er den Punkt offen.

## Krise

Eine Krise ist eine Änderung, die Boot, Anmeldung, die Shell oder die
Sicherheit brechen kann und um die niemand in einem Case gebeten hat. Sie
ist die eine Änderung, die Seldon laut macht: Die Pill zählt sie in der
Fehlerfarbe deines Themes, und das Panel zeigt die Zeile „N changes that
can affect boot, login or the shell have no case“. Du wirst einmal
informiert. Mehr wird von dir nicht verlangt.

Sie hat dieselben drei Aktionen. Ein Agent darf eine Krise nicht
erklären oder verwerfen; er darf sie nur mit seinem eigenen aktiven Case
verknüpfen und sagt dir sonst in einer Zeile Bescheid.

Routine-Upgrades sind keine Krisen, auch wenn sie einen neuen Kernel
bringen. Ein Paket von der Immer-rot-Liste (standardmäßig `linux*`,
`systemd`, `glibc`, `hyprland`, `omarchy`, `quickshell`; siehe
[Konfiguration](06-configuration.md#drift)) ist nur dann eine Krise, wenn
es außerhalb eines Case mit Namen installiert oder entfernt wird. Eine
neue Datei in einem Persistenzpfad ist eine Krise, egal wer sie
geschrieben hat: Units in `~/.config/systemd/user`, Omarchys Hooks in
`~/.config/omarchy/hooks`, `~/.config/autostart`,
`~/.config/environment.d`, `~/.config/uwsm`, `~/.profile`,
`~/.bash_profile`. Diese Dateien laufen bei der Anmeldung oder bei
Ereignissen, ohne deine gewöhnliche Konfiguration zu sein.

## Baseline

Ein neues Logbuch zeichnet ab dem Moment auf, in dem du es anlegst. Der
Assistent kann auch *nacherfassen*: Änderungen seit einem früheren Datum
aufzeichnen, aus dem Paketlog und von Snapper. Keine dieser älteren
Änderungen gehört zu einem Case, also öffnen viele davon als Drift.

Die Baseline räumt das auf. Nach einer Nacherfassung fragt der
Assistent, ob er alles Gefundene als Baseline vor Seldon markieren soll.
Sagst du ja, verwirft die Engine jeden offenen Eintrag mit dem Grund
„pre-Seldon baseline“. Die Ereignisse bleiben im Ledger und in den
Diagrammen; sie verlangen nur keinen Grund mehr. Ohne Nacherfassung gibt
es keine Baseline.

## Entscheidungen

Eine Entscheidung ist eine kurze Aufzeichnung einer Wahl, die die
Maschine prägt, im ADR-Format (Architecture Decision Record): Kontext,
Entscheidung, Folgen. `seldon decide -- "<titel>"` legt
`decisions/ADR-NNNN-<slug>.md` mit dem Status *proposed* an und öffnet
sie in deinem Editor. *Accept* im Bereich Decisions des Desks (zweimal:
der erste Klick macht scharf) oder `seldon decide accept ADR-NNNN` setzt
sie auf *accepted* mit dem heutigen Datum und vermerkt das im Ledger.
Annehmen ist deine Sache: ein Agent darf eine Entscheidung vorschlagen,
nie annehmen. *Superseded* setzt du in der Datei. Der Bereich Decisions
listet sie auf.

## Memory

Memory ist das, was Agenten über diese Maschine gelernt haben, in
`memory/`. `memory/lessons.md` hat eine `##`-Überschrift pro Lektion:
was passiert ist und was beim nächsten Mal zu tun ist. Jeder Agent liest
sie zu Beginn einer Sitzung. Andere Dateien, etwa `memory/hyprland.md`,
halten Notizen zu einem Thema. Du und deine Agenten schreibt diese
Dateien direkt; die Engine liest sie nur.

## Das Dossier

Das Dossier (`system/`) beschreibt die Maschine, wie sie jetzt ist:
Pakete, Dienste, Omarchy-Version und Theme, Hardware, Plugins und die
Dateien, die du gegenüber Omarchys Vorgaben geändert hast. Die Engine
füllt die Teile zwischen den Markierungen `<!-- seldon:begin … -->` und
`<!-- seldon:end -->`; du schreibst drumherum. Siehe
[Wiederaufbau, Dossier und Update-Folgen](08-rebuild-dossier-update-impact.md).

## Erfassung

Eine Erfassung (Capture) lässt die Collectors laufen und hängt neue
Ereignisse ans Ledger an. Das passiert:

- wenn du `seldon capture` ausführst;
- alle 15 Minuten, solange das Plugin läuft, und beim Start der Shell;
- wenn du im Panel *Capture now* drückst oder mit rechts auf die Pill
  klickst;
- am Ende einer Agenten-Sitzung, die Seldons Hooks hat.

Zweimal hintereinander ausgeführt, schreibt sie beim zweiten Mal nichts.
Jeder Collector merkt sich, wie weit er gelesen hat.

---

Zurück: [Erste Schritte](01-getting-started.md) · [Übersicht](README.md) · Weiter: [Alltag](03-daily-use.md)
