# Wiederaufbau, Dossier und Update-Folgen

<!-- source: en/08-rebuild-dossier-update-impact.md @ ab44097 -->

Diese Seite behandelt die drei Ausgaben, die deine Maschine als Ganzes
beschreiben: das Dossier in `system/`, die Wiederaufbau-Anleitung
`outputs/REBUILD.md` und den Bericht über die Folgen eines Updates, der
geplant, aber noch nicht Teil der Engine ist.

## Das Dossier

Das Dossier ist der Ordner `system/`. Es beschreibt die Maschine, wie sie
jetzt ist. Jede Datei hat erzeugte Teile zwischen Markierungen und Platz
für deinen eigenen Text drumherum:

```markdown
## Explicit packages

<!-- seldon:begin packages.explicit -->
- zed · repo · user · since 2026-10-01 [[C-2026-004]]
- …
<!-- seldon:end -->

Meine Notizen zu Paketen stehen hier. Seldon behält sie.
```

| Datei | Erzeugte Teile |
|---|---|
| `packages.md` | Zahlen (explizit, gesamt, AUR); eine Verlaufszeile für jeden Tag, an dem sich die Zahlen geändert haben; jedes explizite Paket mit Quelle, Klasse und dem Case, der es installiert hat |
| `services.md` | aktivierte systemd-Units, System und Benutzer, mit dem Case, der sie aktiviert hat |
| `omarchy.md` | Omarchy-Version, Theme, letztes Update |
| `hardware.md` | CPU, Speicher, Maschine, Root-Dateisystem |
| `plugins.md` | Shell-Plugins: ID, aktiviert, First-Party, woher es kam |
| `deviations.md` | Dateien, die du gegenüber Omarchys Vorgaben geändert hast: Pfad, Grund, Datum, Case |

In `packages.md` hat jedes explizite Paket eine Klasse. `omarchy-base`
heißt, Omarchys eigene Paketlisten nennen es; `user` heißt, du hast es
hinzugefügt. Ein Paket, das schon vor dem Logbuch da war, trägt
`pre-logbook` statt Datum und Case.

`deviations.md` bekommt eine Zeile für jede Konfigurationsdatei, die ein
Case geändert hat. Die Spalte für den Grund bleibt leer, damit du sie
füllst. Deine Zeilen und Gründe bleiben, wie du sie geschrieben hast; die
Engine füllt später nur eine leere Case-Zelle.

### Auffrischen

`seldon init` füllt das Dossier einmal. Danach frischst du es selbst
auf, zum Beispiel einmal pro Woche oder bevor du die
Wiederaufbau-Anleitung schreibst:

```sh
seldon dossier
```

`--section packages,services` frischt nur diese Teile auf. Der Befehl
liest nur: Er fragt den Paketmanager und systemd nach Listen und liest
`/proc` und `/sys`. Er ändert nie Text außerhalb der Markierungen,
schreibt eine Datei nur, wenn sie sich geändert hat, committet als
`seldon: dossier` und schreibt kein Ereignis ins Ledger. Scheitert eine
Abfrage, behält dieser Teil seinen alten Inhalt, und der Befehl gibt eine
Warnung aus.

## Die Wiederaufbau-Anleitung

```sh
seldon rebuild
```

schreibt `outputs/REBUILD.md`: die Schritte, die eine frische
Omarchy-Installation auf den Stand bringen, den dein Logbuch beschreibt.
Führ vorher `seldon dossier` aus, denn die Anleitung liest die Paketliste
daraus.

| Abschnitt | Enthält |
|---|---|
| 1. Base | die Omarchy-Version, die du installierst und auf die du aktualisierst |
| 2. Packages | die Pakete, die du seit Beginn des Logbuchs installiert hast, nach Case gruppiert, als Befehle `omarchy pkg add` und `omarchy pkg aur add`; deine eigenen Pakete von vor dem Logbuch; die Zahl der Pakete, die Omarchy selbst mitbringt |
| 3. Deviations | die Dateien, die du geändert hast, mit deinen Gründen; die Dateien selbst holst du aus deinen Dotfiles oder deinem Backup |
| 4. Plugins | Shell-Plugins über die First-Party-Plugins hinaus, und First-Party-Plugins, die du deaktiviert hast |
| 5. Theme | das Theme, das du setzt |
| 6. User units | systemd-Units, die dieses Logbuch kennt |
| 7. Open questions | Drift, über die noch niemand entschieden hat; entscheide vor dem Wiederaufbau. Darunter, was du bewusst verworfen hast und nicht wieder einrichten sollst |

Jede Zeile nennt den Case oder das Ereignis, aus dem sie stammt. Die
Anleitung hält Pfade und Gründe fest, nie den Inhalt von Dateien. Sie
nennt die Dateien, die du wiederherstellst; ihr Inhalt kommt aus deinen
Dotfiles oder deinem Backup.

Die Engine erzeugt die Anleitung. Text, den du außerhalb ihrer
Markierungen ergänzt, bleibt; Text innerhalb ersetzt sie jedes Mal. Der Befehl
schreibt die Datei nur, wenn sie sich geändert hat, und committet als
`seldon: rebuild`.

### Anwenden

1. Installiere Omarchy auf der neuen Maschine und aktualisiere es auf die
   Version aus Abschnitt 1.
2. Installiere die Engine und leg ein Logbuch an, oder kopiere dein altes
   Logbuch hinüber (siehe
   [Das Logbuch](07-the-logbook.md#das-logbuch-verschieben-oder-kopieren)).
3. Arbeite die Abschnitte 2 bis 6 der Reihe nach ab. Die Paketbefehle
   überspringen, was schon installiert ist.
4. Stell die Dateien aus Abschnitt 3 aus deinen Dotfiles oder deinem
   Backup wieder her.

## Update-Folgen

`seldon update-impact` ist geplant: Vor einem `omarchy update` soll es
vergleichen, was das Update ändert, mit deinen Abweichungen, und die
Anpassungen auflisten, die brechen könnten. Die aktuelle Engine hat den
Befehl noch nicht, und das Plugin zeigt ihn nicht.

Bis dahin ein sicherer Ablauf für Omarchy-Updates:

1. Leg einen Case für das Update an. Er ist rot: Er ändert Pakete und
   Omarchy selbst.

   ```sh
   seldon plan new --zone red --risk R2 --area packages -- "Omarchy-Update"
   ```

2. Lies `system/deviations.md`. Jede Zeile ist eine Datei, die du mit
   Absicht geändert hast und die das Update berühren kann.
3. Nimm einen Snapshot und starte den Case mit seiner Nummer:

   ```sh
   sudo snapper -c root create --description "vor dem Omarchy-Update" --print-number
   seldon plan start C-2026-005 --snapshot <N>
   ```

   `<N>` ist die Nummer, die der erste Befehl ausgibt. Nimm statt
   `C-2026-005` die ID, die `plan new` ausgegeben hat.

4. Führ das Update aus, dann `seldon capture`. Verknüpf die Drift des
   Updates mit `seldon drift link` mit dem Case und prüf deine
   Abweichungen.
5. Prüf den Case und schließ ihn ab.

Der Case enthält dann das Update, den Snapshot zum Zurückrollen und
deine Notizen dazu, was kaputtgegangen ist.

---

Zurück: [Das Logbuch](07-the-logbook.md) · [Übersicht](README.md) · Weiter: [Import aus omarchy-agent](09-import-from-omarchy-agent.md)
