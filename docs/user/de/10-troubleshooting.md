# Fehlersuche

<!-- source: en/10-troubleshooting.md @ 4e03e44 -->

Diese Seite hilft, wenn etwas falsch aussieht: Sie beginnt mit
`seldon doctor`, geht dann durch die Banner des Panels, die Exit-Codes
der Engine und die häufigsten Probleme.

## Mit doctor anfangen

```sh
seldon doctor
```

Es prüft elf Dinge und nennt für jedes, das nicht `ok` ist, eine
Abhilfe. Es liest nur: Es ändert keine Datei, nimmt keine Sperre und
führt nichts mit `sudo` aus.

| Prüfung | `ok` heißt | Wenn sie nicht ok ist |
|---|---|---|
| `engine` | die Engine läuft; ihre Version und ihr Vertrag | (wenn du `doctor` ausführen kannst, ist das ok) |
| `config` | `~/.config/seldon/config.toml` wurde gelesen, und seine `[redaction] patterns` lassen sich übersetzen | `degraded`: noch keine Konfiguration, Vorgaben in Gebrauch; Abhilfe `seldon init`. `error`: die Datei ist nicht lesbar, nicht gültig, oder ein Muster lässt sich nicht übersetzen; jeder Befehl bricht daran ab. Ist die Datei nicht lesbar oder nicht gültig, wird das Logbuch „not checked“ (sein Pfad steht in dieser Datei) |
| `logbook` | das Logbuch existiert; Maschine, Sprache, Zahlen | `error`: an diesem Pfad nicht angelegt; die Abhilfe nennt den Befehl `seldon init` |
| `cases` | jede Case-ID hat eine Datei | `error`: ein Case existiert zweimal (eine veraltete Kopie); behalte die Datei im Ordner ihres Status |
| `ledger` | jede Zeile in `ledger/*.jsonl` ist ein Ereignis | `degraded`: Zeilen, die keine Ereignisse sind (ein abgerissener Schreibvorgang, eine Handänderung), werden übersprungen; die Zeile nennt Monat, Anzahl und Zeilen |
| `fences` | die generierten Teile von `STATUS.md` und `DECISIONS.md` haben ihre Markerzeilen | `degraded`: eine Markerzeile fehlt, also lässt `seldon status` die Datei in Ruhe; oder ein End-Marker schließt keinen Abschnitt. `error`: die Datei ist nicht lesbar |
| `collectors` | der letzte Capture jedes eingeschalteten Collectors ist gelungen | `degraded`: die Zeile nennt jeden fehlgeschlagenen Collector mit Meldung und Abhilfe |
| `state` | `cursors.json`, `manifest.json` und `owned.json` in `~/.local/state/seldon` sind lesbar | `error`: die Datei ist beschädigt oder nicht lesbar; die Zeile sagt, was das kaputt macht; die Abhilfe verschiebt eine beschädigte Datei oder macht eine unlesbare lesbar. `degraded`: das nächste Capture wird einen Zustands-Reset festhalten, siehe [doctor sagt, das nächste Capture hält einen Zustands-Reset fest](#doctor-sagt-das-nächste-capture-hält-einen-zustands-reset-fest); oder das letzte Capture hat einen festgehalten, siehe [Ein Zustands-Reset wurde festgehalten](#ein-zustands-reset-wurde-festgehalten); oder ein Collector wartet seit einem Zustands-Reset auf seine Baseline (degraded oder nicht gelaufen): `seldon capture --source <name>` ausführen, sobald er laufen kann |
| `omarchy` | `omarchy-version` hat geantwortet | der Omarchy-Collector kann die Version nicht lesen |
| `snapper` | Snapshots lassen sich auflisten oder aus `/.snapshots` lesen | `degraded`: dein Benutzer darf weder Snapshots auflisten noch `/.snapshots` lesen; siehe [Snapshots werden nicht aufgezeichnet](#snapshots-werden-nicht-aufgezeichnet). Eine `ok`-Zeile mit Abhilfe: dein Benutzer steht noch im alten Snapper-Opt-in; siehe [doctor rät, das Snapper-Opt-in zurückzunehmen](#doctor-rät-das-snapper-opt-in-zurückzunehmen) |
| `git` | git ist da; das Logbuch ist ein Repository | git fehlt, oder das Logbuch ist kein Repository; dann ist Autocommit aus. `degraded`: etwas hindert jeden Autocommit (ein liegengebliebenes `.git/index.lock`, ein losgelöster HEAD, …); die Abhilfe sagt, was zu tun ist |

`doctor --json` gibt dasselbe als JSON aus. Das Plugin führt `doctor`
nicht aus: Es wählt sein Banner nach dem Index und nach seinen eigenen
Engine-Aufrufen.

## Banner im Panel

Wenn etwas nicht stimmt, zeigt das Panel oben ein Banner mit einem
Knopf, der es behebt.

| Banner | Ursache | Abhilfe |
|---|---|---|
| Install the engine (ein Einrichtungsschritt); Seldon engine missing, in Rot, wenn die Engine vorher da war | das Plugin kann `seldon` nicht starten | *Install* öffnet ein Terminal, das sagt, was es tut, und den GitHub-Installer startet; oder du installierst selbst ([Erste Schritte](01-getting-started.md#schritt-1-die-engine-installieren)); dann *Check again* |
| Create your logbook | es gibt noch kein Logbuch | *Create* öffnet ein Terminal, das `seldon init` startet; das Panel aktualisiert sich von selbst, sobald das Logbuch da ist |
| No index yet / Index unreadable | `~/.local/state/seldon/index.json` fehlt oder ist kaputt | *Build index* startet `seldon status` |
| Index is stale | der Index ist älter als zwei Stunden | *Capture now* |
| Index format mismatch | Plugin und Engine sprechen verschiedene Versionen des Index | das ältere aktualisieren. Plugin: `omarchy plugin update jax.seldon`, danach `omarchy-restart-shell`. Engine: *Update* führt den Installer noch einmal aus (bis es das AUR-Paket gibt; siehe [Aktualisieren und entfernen](11-update-and-uninstall.md)) |
| Engine too old | die Engine ist älter, als dieses Plugin sie braucht (das `engineMin` in seinem Manifest) | *Update* führt den Installer in einem Terminal noch einmal aus (bis es das AUR-Paket gibt; siehe [Aktualisieren und entfernen](11-update-and-uninstall.md)), dann *Check again* |
| Read snapshots (optional) | Snapper weist deinen Benutzer ab, und `/.snapshots` ist nicht lesbar | *Grant* öffnet ein Terminal, das sagt, was die Freigabe erlaubt, die einmalige Lesefreigabe startet (dort tippst du dein Passwort) und die Snapshots aufzeichnet; danach verschwindet das Banner von selbst. Seldon funktioniert auch ohne Snapshots |
| Restart the shell to finish the update | das Plugin wurde aktualisiert, aber die Shell führt noch den vorher geladenen Code aus (neuen Plugin-Code lädt sie erst beim Neustart) | *Restart shell* startet `omarchy-restart-shell`; Leiste und Panels sind nach wenigen Sekunden wieder da. Siehe [Das Plugin aktualisieren](11-update-and-uninstall.md#das-plugin-aktualisieren) |
| Capture warned | ein Capture, das das Plugin gestartet hat, endete mit einer Warnung, etwa [einem Zustands-Reset](#ein-zustands-reset-wurde-festgehalten); der Hinweis zeigt die erste Zeile jeder Warnung, der Mauszeiger darüber zeigt sie ganz | kein Knopf: tu, was die Warnung sagt. Der Hinweis verschwindet nach dem nächsten Capture ohne Warnungen |

Das Plugin sucht die Engine beim Start der Shell und wenn du *Check
again* drückst. Hast du die Engine installiert, drück *Check again* oder
starte die Shell mit `omarchy-restart-shell` neu.

Ein Index veraltet, wenn zwei Stunden lang keine Erfassung lief, etwa
nachdem die Maschine geschlafen hat. Die nächste planmäßige Erfassung
behebt das von selbst.

## Exit-Codes

Jeder `seldon`-Befehl endet mit einem dieser Codes:

| Code | Bedeutung | Was zu tun ist |
|---|---|---|
| 0 | ok | |
| 1 | Benutzerfehler: ein falsches Argument, ein unbekannter Case oder ein unbekanntes Ereignis, ein Schritt, den der Case nicht gehen kann | die Meldung lesen; `seldon <command> --help` |
| 2 | Fehler der Engine | den Befehl noch einmal mit `--json` ausführen und die Meldung für einen Fehlerbericht aufheben |
| 3 | das Logbuch ist nicht angelegt | `seldon init`, oder `--logbook` und `SELDON_LOGBOOK` prüfen |
| 4 | ein anderes `seldon` hält die Sperre | kurz warten und noch einmal ausführen |

## Häufige Probleme

### „another seldon process holds the lock“

Es schreibt immer nur ein `seldon` zur Zeit. Vielleicht läuft gerade eine
Erfassung des Plugins oder der Hook eines Agenten. Warte eine Sekunde
und versuche es noch einmal. Löst sich die Sperre nie, such mit
`pgrep -a seldon` nach einem hängenden Prozess. Die Sperre ist
`~/.local/state/seldon/lock`; sie wird frei, wenn der Prozess endet, du
löschst sie also nie von Hand.

### Snapshots werden nicht aufgezeichnet

`seldon doctor` meldet `snapper degraded: No permissions`. Omarchy
erlaubt deinem Benutzer weder, Snapshots aufzulisten, noch das
Snapshot-Verzeichnis zu lesen. Seldon funktioniert ohne sie; die
Zeitleiste hat dann keine Snapshot-Marken. Um es zu erlauben, führst du
einmal aus:

```sh
sudo setfacl -m u:$USER:rx /.snapshots
```

Seldon führt es nie für dich aus. Es gibt deinem Benutzer Lesezugriff
auf `/.snapshots`: Seldon liest dann die Snapshot-Liste und die
Info-Dateien. Snapshots anlegen, ändern oder löschen kann dein Benutzer
damit nicht. Dateien in einem Snapshot behalten ihre eigenen Rechte; du
kannst in einem alten Snapshot also lesen, was du lesen konntest, als er
entstand.

### doctor rät, das Snapper-Opt-in zurückzunehmen

Frühere Versionen von Seldon rieten, deinen Benutzer in `ALLOW_USERS`
der Snapper-Konfiguration einzutragen. Das erlaubt deinem Benutzer auch,
Snapshots von root ohne Passwort anzulegen, zu ändern und zu löschen.
Steht dein Benutzer noch dort, sagt `seldon doctor` das in der Zeile
`snapper` und gibt aus:

```sh
sudo snapper -c root set-config ALLOW_USERS="" SYNC_ACL=no && sudo setfacl -m u:$USER:rx /.snapshots
```

Der erste Befehl leert die Liste (trag jeden anderen Benutzer, der darin
bleiben soll, wieder ein) und hält Snapper davon ab, die Zugriffsliste
von `/.snapshots` zu verwalten, damit eine spätere Snapper-Änderung dir
den Lesezugriff nicht wieder nimmt. Er kann den Lesezugriff entfernen,
den das alte Opt-in deinem Benutzer gegeben hat; der zweite Befehl gibt
ihn. Seldon zeichnet Snapshots in beiden Fällen weiter auf.

### Eine Änderung erscheint nicht

- Führ `seldon capture` aus und sieh dir die Zeilen pro Collector an. Ein
  Collector mit 0 ist vielleicht in `config.toml` ausgeschaltet oder
  `degraded`.
- Der Collector sieht eine Konfigurationsdatei nur, wenn sie unter einem
  beobachteten Pfad liegt, nicht größer als 1 MiB ist, keine Binärdatei
  ist und nicht in `skipPaths` steht
  ([Konfiguration](06-configuration.md#beobachtete-pfade)).
- Eine Paket-Transaktion, die noch läuft, zeichnet der Collector auf,
  sobald sie endet.
- Änderungen durch ein Programm, das die Hooks nicht sehen (eine
  grafische Oberfläche, ein Skript), erscheinen bei der nächsten
  Erfassung, als Drift ohne Namen eines Agenten.

### Das Panel zeigt alte Daten

Das Plugin liest den Index, und die Engine schreibt ihn nach jedem
Befehl neu. Hast du Dateien in einem Editor geändert, holt der Index das
mit dem nächsten Befehl oder der nächsten Erfassung nach. Drück `c` im
Panel oder führ `seldon status` aus. Damit Änderungen sofort erscheinen,
nimm den [Watcher](11-update-and-uninstall.md#der-optionale-watcher).

Sagt eine Aktion im Panel, der Index sei hinter deinem Logbuch, hast du
das Logbuch in der Zwischenzeit woanders geändert. Erfasse, dann
versuche es noch einmal.

### Viel Drift nach einer Nacherfassung

Eine Nacherfassung zeichnet ältere Änderungen auf, und keine davon hat
einen Case. Markiere sie als Baseline: Verwirf sie mit einem Grund. Eine
Gruppe von Paketen ist ein Eintrag, also sind das meist nur eine Handvoll
Befehle:

```sh
seldon drift
seldon drift dismiss <EVENT> -- "pre-Seldon baseline"
```

### Drift, die ich nicht kenne

```sh
seldon drift show <EVENT> --json
```

zeigt das Ereignis mit jedem Mitglied seiner Gruppe, den Actor und bei
Paketen den Befehl, der lief. Sieh im Journal und in den Notizen der
Agenten von diesem Tag nach. Weißt du es dann immer noch nicht, lass sie
offen und schreib eine Notiz ins Journal. Eine falsche Erklärung führt
dich später in die Irre.

### Die Befehle eines Agenten werden nicht aufgezeichnet

- Claude Code: Hat Seldon es gestartet (*Run*, *Start agent*,
  `seldon agent start`), oder hast du es im Ordner des Logbuchs
  gestartet? Eine Sitzung, die du anderswo von Hand startest, wird nicht
  aufgezeichnet. Stehen die Hooks in `~/.claude/settings.json`? Die Zeile
  `hooks` von `seldon doctor` sagt es; `seldon hook install claude-code`
  ergänzt, was fehlt.
- Ist ein Case aktiv? Grüne Befehle werden nur dann aufgezeichnet.
- Befehle in `xargs`, `find -exec` oder `python -c` liest der Hook nicht.
- Andere Agenten rufen `seldon hook generic` selbst auf
  ([Mit Agenten arbeiten](04-working-with-agents.md#andere-agenten)).

### `seldon agent start` verweigert, oder nichts öffnet sich

- Der Case muss aktiv sein: erst `seldon plan start <ID>`.
- Einen Launcher, der eine Shell ist oder kein `{prompt}` hat, lehnt die
  Engine ab; die Meldung sagt, warum.
- Erscheint das Fenster des Agenten nicht, lies
  `~/.local/state/seldon/agent-launch.log`.

### doctor sagt, das nächste Capture hält einen Zustands-Reset fest

`seldon doctor` zeigt eine Zeile wie diese:

```
  degraded  state    the next capture will record a state reset for pacman, config: cursors missing in ~/.local/state/seldon, …
```

Der Zustandsordner der Engine, `~/.local/state/seldon`, hat keine
brauchbaren Cursors für dieses Logbuch (er wurde gelöscht und noch nicht
wiederhergestellt, oder ein Wert in `cursors.json` ist nicht lesbar),
während dein Ledger schon Ereignisse dieser Collectors hat. Noch ist
nichts verloren: Das nächste Capture würde diese Collectors neu anfangen
lassen, wie in
[Ein Zustands-Reset wurde festgehalten](#ein-zustands-reset-wurde-festgehalten)
beschrieben.

- Hast du eine Sicherung des Zustandsordners, stelle sie jetzt wieder
  her, vor dem nächsten Capture (siehe
  [Den Zustandsordner sichern und wiederherstellen](07-the-logbook.md#den-zustandsordner-sichern-und-wiederherstellen)).
  Mit installierten Agent-Hooks läuft auch beim Ende einer
  Agent-Sitzung ein Capture, also erledige das zuerst. Führe
  `seldon doctor` erneut aus: Die Zeile ist weg, und das nächste Capture
  hält fest, was sich seit der Sicherung geändert hat.
- Ohne Sicherung führe `seldon capture` aus, um die neue Basis zu
  übernehmen. Es hält den Zustands-Reset fest; danach zeigt sich bis zum
  folgenden Capture die Zeile aus dem nächsten Abschnitt.
- Sagt die Zeile `bound to another logbook`, gibt es nichts
  wiederherzustellen: Der Zustand gehört zu einem anderen Logbuch-Pfad
  (du hast das Logbuch verschoben oder einen Befehl mit `--logbook` für
  ein anderes ausgeführt). Führe `seldon capture` aus.
- Sagt die Zeile `the next capture will warn of the state reset for …`,
  hat ein Capture den Reset festgehalten und ist stehen geblieben, bevor
  es seinen Zustand gespeichert hat (ein Absturz, ein Kill). Es gelten
  dieselben Schritte; das nächste Capture hält den Reset kein zweites
  Mal fest, es gibt nur die Warnung aus, und danach bleibt keine Zeile
  stehen.

doctor kann nicht wissen, ob ein Collector in diesem Capture degraded
läuft (snapper ohne die Lesefreigabe zum Beispiel). So ein Collector
nimmt keine neue Basis, also nennt das Capture womöglich weniger
Collectors als die Zeile. Ein Collector, mit dem du hier noch nie ein
Capture gemacht hast, wird nicht genannt: Er hat nichts zu verlieren.

### Ein Zustands-Reset wurde festgehalten

`seldon capture` hat eine Zeile wie diese ausgegeben:

```
warning: state reset recorded: pacman, config took a new baseline because ~/.local/state/seldon was missing, unreadable or bound to another logbook, …
```

Der Zustandsordner der Engine, `~/.local/state/seldon`, fehlte, gehörte
zu einem anderen Logbuch oder enthielt eine Datei, die sie nicht lesen
konnte, während dein Ledger schon Ereignisse dieser Collectors hatte.
Sie haben beim aktuellen Stand der Maschine neu angefangen. Änderungen
seit ihrem letzten Capture können im Ledger fehlen: `pacman` und
`snapper` lesen ihre Quellen erneut und verpassen wenig, die anderen
Collectors verpassen jede Änderung dazwischen.

Das Capture hat eine `state-loss`-Zeile mit dem Betreff `state-reset`
(in 0.1.x eine Notiz) ins Ledger
geschrieben, damit die Lücke sichtbar bleibt, und `seldon doctor` zeigt
bis zum nächsten Capture eine `state`-Zeile mit `degraded`. Hat das
Plugin dieses Capture gestartet, zeigt das Panel die Warnung als Hinweis
„Capture warned“, bis ein Capture ohne Warnungen läuft.

Hast du eine Sicherung des Zustandsordners, stelle sie wieder her und
führe ein Capture aus; dieses Capture hält fest, was sich seit der
Sicherung geändert hat (siehe
[Den Zustandsordner sichern und wiederherstellen](07-the-logbook.md#den-zustandsordner-sichern-und-wiederherstellen)).
Ohne Sicherung gibt es nichts wiederherzustellen: Das nächste Capture
nimmt die Zeile weg, die Notiz bleibt im Ledger. Dasselbe gilt, wenn
der Zustand zu einem anderen Logbuch gehörte (du hast das Logbuch
verschoben oder einen Befehl mit `--logbook` für ein anderes
ausgeführt); die Warnung sagt das dann.

Ein Capture, das ein beschädigtes `owned.json` findet, verschiebt es
nach `owned.json.bad` im selben Ordner. Seldon liest diese Datei nie
wieder; sie bleibt nur zum Nachsehen, du kannst sie löschen, und das
nächste beschädigte `owned.json` ersetzt sie.

### Der Theme-Collector ist degraded

Er liest `~/.local/state/omarchy/current/theme.name`. Omarchy schreibt
diese Datei, wenn es ein Theme setzt. Setz einmal irgendein Theme, und
die nächste Erfassung ist ok.

### Der Plugins-Collector ist degraded

Er fragt die laufende Omarchy-Shell. Eine Erfassung von einem TTY, über
SSH oder während die Shell neu startet, erreicht sie nicht. Die nächste
Erfassung in deiner Desktop-Sitzung holt das nach.

## Logs

| Was | Wo |
|---|---|
| Warnungen des Plugins | `journalctl --user -t omarchy-shell` |
| Fehler des Agent-Launchers | `~/.local/state/seldon/agent-launch.log` |
| der Watcher, wenn du ihn nutzt | `journalctl --user -u seldon-watch` |
| was das Plugin sieht | `omarchy-shell jax.seldon.service status` und `omarchy-shell jax.seldon.panel view` (JSON) |

## Einen Fehler melden

Eröffne ein Issue unter <https://github.com/JohnAndrewsX/jax-seldon/issues>
mit dem Befehl, der Ausgabe von `seldon --version` und `seldon doctor` und
der Meldung desselben Befehls mit `--json`. Ersetze vor dem Posten deinen
Benutzernamen, den Namen deiner Maschine und private Pfade; die Ausgabe
kann sie enthalten. Bitte schreib das Issue auf Englisch.

---

Zurück: [Import aus omarchy-agent](09-import-from-omarchy-agent.md) · [Übersicht](README.md) · Weiter: [Aktualisieren und entfernen](11-update-and-uninstall.md)
