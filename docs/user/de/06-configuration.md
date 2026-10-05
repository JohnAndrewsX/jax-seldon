# Konfiguration

<!-- source: en/06-configuration.md @ d5ddb85 -->

Diese Seite beschreibt alles, was du einstellen kannst: die
`config.toml` der Engine mit Collectors, beobachteten Pfaden, Schwärzung,
Drift-Liste und Agent-Launcher, danach die Einstellungen des Plugins und
die Tastenbelegung.

## Die Konfigurationsdatei

Die Engine liest `~/.config/seldon/config.toml`. `seldon init` schreibt
sie, und du kannst sie mit jedem Editor ändern. Eine Änderung wirkt ab
dem nächsten `seldon`-Befehl; nichts muss neu gestartet werden.

Bevor du sie änderst:

- `seldon init` schreibt die Datei neu. Es behält Schlüssel, die es nicht
  kennt, verwirft aber Kommentare und ordnet die Schlüssel neu. Halte
  Notizen woanders fest.
- Eine Datei, die kein gültiges TOML ist, stoppt jeden Befehl mit Exit 1.
  Der Fehler nennt die Datei und das Problem.

Für eine andere Datei bei einem einzelnen Befehl gibst du
`--config <FILE>` an oder setzt `SELDON_CONFIG`.

## Ein vollständiges Beispiel

So sieht die Datei aus, die der Assistent schreibt, wenn du seine
Vorgaben nimmst und Claude Code anhakst. Den Abschnitt `[agent]` lässt
die Datei weg, solange er die Vorgabe enthält; hier steht er, damit du
den Schlüssel siehst. In deiner Zeile `logbook` steht dein eigener Pfad.

```toml
harnesses = ["claude-code"]
language = "de"
logbook = "/home/you/Seldon"
watchPaths = ["~/.config/hypr", "~/.config/omarchy", "~/.config/waybar", "~/.bashrc", "~/.zshrc"]

[collectors]
config = true
omarchy = true
pacman = true
plugins = true
snapper = true
theme = true

[drift]
alwaysRed = ["linux*", "systemd", "glibc", "hyprland", "omarchy", "quickshell"]

[git]
autocommit = true

[redaction]
patterns = []
skipPaths = ["~/.config/omarchy/**/history.json", "~/.config/omarchy/**/history/", "~/.config/omarchy/**/state.json", "~/.config/omarchy/**/cache/", "~/.config/omarchy/**/*.log"]

[agent]
launcher = ["omarchy", "agent", "prompt", "{prompt}"]
```

## Schlüssel

| Schlüssel | Vorgabe | Bedeutung |
|---|---|---|
| `logbook` | `~/Seldon` | der Ordner des Logbuchs; ein relativer Pfad liegt unter deinem Home-Ordner |
| `language` | aus deiner Locale | `en` oder `de`: die Sprache, die `seldon init` einem neuen Logbuch gibt, siehe [Sprache](#sprache) |
| `watchPaths` | siehe [Beobachtete Pfade](#beobachtete-pfade) | Dateien und Ordner, die der Config-Collector beobachtet |
| `harnesses` | `[]` | die Agenten-Harnesses, die du im Assistenten gewählt hast; zur Information |
| `[collectors]` | alle `true` | welche Collectors eine Erfassung startet |
| `[git] autocommit` | `true` | das Logbuch nach jedem schreibenden Befehl committen; `seldon init --no-git` schreibt `false` |
| `[redaction] patterns` | `[]` | deine eigenen Muster für Geheimnisse, siehe [Schwärzung](#schwärzung) |
| `[redaction] skipPaths` | Zustandsdateien von Plugins | Dateien, die die Engine nie öffnet oder nennt, siehe [Schwärzung](#schwärzung) |
| `[drift] alwaysRed` | sechs Namen | Pakete, deren Upgrade immer eine Krise ist, siehe [Drift](#drift) |
| `[agent] launcher` | `omarchy agent prompt` | was `seldon agent start` startet, siehe [Agent-Launcher](#agent-launcher) |
| `[agent.launchers]` | keine | weitere Launcher mit Namen |

Den Pfad des Logbuchs nimmt die Engine in dieser Reihenfolge:
`--logbook`, `SELDON_LOGBOOK`, `logbook` aus der Konfiguration,
`~/Seldon`.

Pfade in der Konfigurationsdatei (`logbook`, `watchPaths`) dürfen mit
`~/` oder `$HOME/` beginnen. Ein relativer Pfad liegt unter deinem
Home-Ordner, egal in welchem Ordner `seldon` läuft: Das Plugin und die
Agenten-Hooks starten es aus verschiedenen Ordnern. `--logbook` und
`SELDON_LOGBOOK` gelten relativ zum aktuellen Ordner, wie bei jedem
Shell-Befehl.

### Sprache

`seldon init` schreibt das Logbuch in einer Sprache: die Vorlagen und
später die Texte, die die Engine ergänzt (Journal-Zeilen, `STATUS.md`).
Es nimmt `--language`, sonst `language` aus dieser Datei, sonst deine
Locale (`de` bei einer deutschen Locale, sonst `en`), und schreibt das
Ergebnis hierher.

Das Logbuch speichert seine Sprache selbst, in `.seldon/logbook.toml`,
und jeder Befehl liest sie von dort. Änderst du `language` in dieser
Datei später, ändert das ein bestehendes Logbuch nicht; es legt nur die
Sprache des nächsten Logbuchs fest, das `seldon init` anlegt.

## Collectors

| Collector | Liest | Zeichnet auf |
|---|---|---|
| `snapper` | `snapper --jsonout list` | angelegte und gelöschte Snapshots |
| `pacman` | `/var/log/pacman.log` | installierte, entfernte, aktualisierte und zurückgestufte Pakete, nach Transaktion gruppiert |
| `omarchy` | `omarchy-version` | Wechsel der Omarchy-Version |
| `plugins` | `omarchy plugin list --json` | Shell-Plugins: hinzugefügt, entfernt, aktiviert, deaktiviert, aktualisiert |
| `theme` | `~/.local/state/omarchy/current/theme.name` | Theme-Wechsel |
| `config` | die Dateien unter `watchPaths` | Dateien: hinzugefügt, geändert, entfernt |

Jeder Collector liest nur. Mit `false` schaltest du einen aus;
`seldon capture --source <name>` startet ihn trotzdem bei Bedarf.

Ein Collector, der seine Quelle nicht lesen kann, ist `degraded`: Die
Erfassung läuft weiter, und `seldon doctor` nennt die Abhilfe. Zwei Fälle
sind normal:

- `snapper` braucht Lesezugriff auf das Snapshot-Verzeichnis
  `/.snapshots`. Omarchy gibt ihn deinem Benutzer nicht. Die Abhilfe ist
  `sudo setfacl -m u:$USER:rx /.snapshots`, die du selbst ausführst, oder
  gar nicht. Damit liest Seldon die Snapshot-Liste und die Info-Dateien,
  sonst nichts: Snapshots anlegen, ändern oder löschen kann dein Benutzer
  damit nicht. Dateien in einem Snapshot behalten ihre eigenen Rechte.
- `plugins` fragt die laufende Omarchy-Shell. Erfasst du von einem TTY
  ohne Desktop-Sitzung, ist er für diese Erfassung `degraded`.

## Beobachtete Pfade

`watchPaths` listet Dateien und Ordner. Der Config-Collector bildet bei
jeder Erfassung den Hash jeder Datei darunter und zeichnet auf, was
hinzugekommen, geändert oder entfernt ist. Er zeichnet den Pfad und zwei
kurze Hashes auf, nie den Inhalt.

Vorgaben: `~/.config/hypr`, `~/.config/omarchy`, `~/.config/waybar`,
`~/.bashrc`, `~/.zshrc`. Fehlende Pfade überspringt der Collector. Ein
relativer Pfad wie `.config/nvim` bedeutet `~/.config/nvim`; der
Assistent speichert getippte Pfade in dieser Form. Ergänze eigene, zum
Beispiel:

```toml
watchPaths = ["~/.config/hypr", "~/.config/omarchy", "~/.config/waybar", "~/.bashrc", "~/.zshrc", "~/.config/nvim", "~/.config/systemd/user"]
```

Immer ausgenommen:

- `~/.config/omarchy/plugins/` (das deckt der Plugins-Collector ab);
- `.git`-Ordner und Ordner, die über einen Symlink erreicht werden;
- Binärdateien und Dateien über 1 MiB (als übersprungen gelistet, ohne
  Hash);
- Dateien, deren Name ein Steuerzeichen enthält oder deren Pfad länger
  als 512 Zeichen ist: Die Erfassung zählt sie in einer Warnung;
- alles in `[redaction] skipPaths`. Die Vorgabe enthält die Dateien, die
  Shell-Plugins in einem eigenen Ordner unter `~/.config/omarchy/`
  halten und alle paar Minuten neu schreiben: `history.json`,
  `state.json`, die Ordner `history/` und `cache/` und `*.log`-Dateien.
  Ohne sie öffnete jedes Neuschreiben einen weiteren Drift-Eintrag.
  Weitere unruhige Dateien eines Plugins ergänzt du genauso.

Eine Datei, deren Größe, Änderungszeit, Statusänderungszeit (ctime) und
Inode seit der letzten Erfassung gleich sind, liest der Collector nicht
noch einmal. Die ctime erkennt eine Änderung, deren Änderungszeit
zurückgesetzt wurde (`touch -r`).

Unit-Dateien unter `~/.config/systemd/` gehören zur roten Zone. Alles
andere hier ist gelb.

Was beobachtet wird zu ändern, ist keine Änderung an Dateien. Fügst du
einen Pfad hinzu oder nimmst ein Muster aus `skipPaths` heraus, nimmt
die nächste Erfassung die Dateien, die neu in den Blick kommen, so auf,
wie sie sind, ohne Event. Entfernst du einen Pfad oder fügst ein Muster
hinzu, gelten die Dateien, die aus dem Blick fallen, nicht als entfernt.
Die Erfassung sagt das in einer Zeile, zum Beispiel `watch scope
changed: 514 file(s) left it, 0 entered it; no events for them`. Danach
beobachtet sie die neuen Dateien wie alle anderen. Nimm einen Ordner mit
vielen Dateien nur auf, wenn du jede einzelne verfolgen willst.

## Schwärzung

Bevor die Engine etwas schreibt, entfernt sie Geheimnisse aus dem Text:
aus jedem Feld eines Ereignisses (der Befehlszeile, dem Subjekt, dem
Detailtext und den übrigen Werten) und aus dem Text und den Tags, die du
`seldon log`, `seldon plan new`, dem `--reason` eines Schritts,
`seldon decide` und `seldon drift explain` oder `dismiss` gibst. Journal,
Case- und Entscheidungsdateien und `STATUS.md` enthalten deshalb
denselben Text wie das Ledger. Ein geschwärzter Wert lautet
`‹redacted›`. Die eingebauten Regeln erfassen:

- `--password`, `--token`, `--with-token`, `--secret`, `--passphrase`
  und ähnliche Optionen sowie `token=`, `PASSWORD=`, `PGPASSWORD=`,
  `MYSQL_PWD=`, `SECRET=` und andere Zuweisungen der Form `…PASSWORD=`,
  `…SECRET=`, `…_PASS=`, mit jedem Wert;
- `--api-key`, `--access-key`, `--secret-key` sowie `API_KEY=` und
  andere Zuweisungen der Form `…KEY=`, wenn der Wert wie ein Zugangsdatum
  aussieht: mindestens 16 Zeichen, oder mindestens 8, die zwei von
  Kleinbuchstaben, Großbuchstaben, Ziffern und anderen Zeichen mischen
  (`sort --key=2` und `hotkey=Super` bleiben also, wie sie sind);
- `Authorization:`, `X-Api-Key:`, `Private-Token:` und andere Header,
  deren Name auf Key, Token, Secret oder Auth endet, sowie der Wert von
  `Cookie:` und `Set-Cookie:`;
- der Wert eines JSON-Schlüssels wie `"password"`, `"passwd"`,
  `"client_secret"` oder `"access_token"` in eingebettetem JSON
  (`curl -d '{"password": "…"}'`); `"password_hint"` bleibt;
- AWS-Zugangsschlüssel (`AKIA…`, `ASIA…`), GitHub-Tokens (`ghp_…`,
  `gho_…`, `github_pat_…` und die übrigen `gh…_`-Formen), GitLab-Tokens
  (`glpat-…`), Slack-Tokens (`xoxb-…`), API-Schlüssel (`sk-…`, `sk_…`);
- das Passwort nach `-p` bei `mysql`, `psql` und `smbclient`, nach
  `sshpass -p` und nach `docker login -p` (auch `podman`);
- Benutzer und Passwort nach `curl -u`, die Cookies nach `curl -b`
  oder `--cookie` (kein Name einer Cookie-Datei);
- Proxy-Zugangsdaten: nach `curl -U`, `--proxy-user` und
  `--proxy-password` sowie `user:pass@` im Proxy nach `curl -x`,
  `--proxy` oder in `https_proxy=`;
- Benutzer und Passwort in einer URL (`https://user:secret@host`), auch
  wenn das Passwort `/`, `?`, `#` oder `:` enthält.

Text, der geschrieben wurde, bevor es eine Regel gab, bleibt, wie er ist.

Die Regeln schwärzen lieber zu viel als zu wenig. Eigene Regeln trägst
du als reguläre Ausdrücke ein; jeder ersetzt seinen ganzen Treffer:

```toml
[redaction]
patterns = ["MYAPP_SESSION=\\S+", "acme_[0-9A-Za-z]{24}"]
```

Ein ungültiges Muster ist ein Fehler (Exit 1): Seldon schreibt lieber gar
nicht, als etwas preiszugeben.

`skipPaths` nennt Dateien, die der Config-Collector und die Hooks nie
öffnen, hashen oder nennen. Die Vorgabe deckt unruhige Plugin-Dateien ab
(siehe [Beobachtete Pfade](#beobachtete-pfade)). Eine leere Liste,
`skipPaths = []`, wie frühere Versionen von `seldon init` sie
schrieben, bedeutet ebenfalls die Vorgabe. Eine eigene Liste ersetzt die
Vorgabe; übernimm deren Muster aus dem Beispiel oben in deine Liste,
wenn du sie behalten willst.

```toml
[redaction]
skipPaths = ["~/.config/hypr/secrets.lua", "*.key", "**/tokens/**"]
```

| Muster | Trifft |
|---|---|
| mit `/`, etwa `~/.config/app/secret.conf` | genau diesen Pfad, oder diesen Ordner mit allem darin |
| ein relativer Pfad, etwa `app/secret.conf` | diesen Pfad unter jedem Ordner |
| ein Name ohne `/`, etwa `*.key` | jede Datei und jeden Ordner mit diesem Namen |

`*` und `?` bleiben innerhalb eines Pfadteils; `**` geht über Teile
hinweg und steht für mindestens einen Ordner:
`~/.config/omarchy/**/state.json` trifft
`~/.config/omarchy/app/state.json`, nicht `~/.config/omarchy/state.json`.

Die Schwärzung hilft, aber sie fängt nur, was sie erkennt. Schreib nie
ein Geheimnis in eine Notiz, einen Case oder eine Befehlszeile, wenn du
es vermeiden kannst.

## Drift

`[drift] alwaysRed` listet Paketnamen, deren Routine-Upgrade trotzdem
eine Krise ist. Ein Routine-Upgrade ist eine Transaktion, die bei einem
vollständigen System-Upgrade nur Pakete aktualisiert (`omarchy update`
macht eins). Es öffnet gelbe Drift und löst darum keinen Alarm aus. Ein
Paket auf dieser Liste macht die ganze Transaktion rot.

```toml
[drift]
alwaysRed = ["linux*", "systemd", "glibc", "hyprland", "omarchy", "quickshell", "nvidia*"]
```

`*` passt auf jedes Ende. Behalte die Vorgaben: Diese Pakete können
Boot, Login oder die Shell kaputtmachen.

## Agent-Launcher

`seldon agent start` (und *Start agent* im Panel) startet den Launcher.
Die Vorgabe startet Omarchys Standard-Agenten in einem eigenen
Terminalfenster:

```toml
[agent]
launcher = ["omarchy", "agent", "prompt", "{prompt}"]
```

Ein Launcher ist eine Liste: das Programm, dann seine Argumente. Regeln:

- Genau ein Element ist `{prompt}`. Die Engine ersetzt es durch den
  Prompt, als ein einziges Argument.
- Das Programm ist ein Name in deinem `PATH` (ohne `/`) oder ein
  absoluter Pfad.
- Vor `{prompt}` lehnt die Engine Programme ab, die ihre Argumente
  bekanntermaßen als Code ausführen: Shells (`bash`, `sh`, `zsh`, `fish`
  und andere), Interpreter (`python`, `perl`, `node` und andere),
  Programme, die einen String an eine Shell geben (`script`, `watch`,
  `flock`, `su`, `ssh`, `tmux`, `screen`, `xargs` und andere), `eval`,
  `hyprctl`, die Omarchy-Starter, die Shell-Strings bauen, `env -S` und
  `sudo -s`/`-i`. Ein Versionsanhang ändert den Namen nicht
  (`python3.12` ist `python`). Die Prüfung geht nach dem Programmnamen:
  eine Heuristik, keine Sandbox.
- Der Prompt nennt den Case und das Logbuch und enthält keinen Text aus
  dem Logbuch. Er ist ein Kommandozeilenargument und in der Prozessliste
  (`ps`) sichtbar, solange der Agent läuft.

Weitere Launcher gehören unter `[agent.launchers]`; du wählst einen mit
`seldon agent start <ID> --launcher <NAME>`. Der Name `omarchy` erreicht
immer die eingebaute Vorgabe.

```toml
[agent]
launcher = ["alacritty", "-e", "claude", "{prompt}"]

[agent.launchers]
codex = ["alacritty", "-e", "codex", "{prompt}"]
```

Der Launcher startet losgelöst im Ordner des Logbuchs. Seine
Fehlerausgabe landet in `~/.local/state/seldon/agent-launch.log`.

## Git

Mit `autocommit = true` committet jeder schreibende Befehl das Logbuch,
mit einer Nachricht wie `seldon: C-2026-004 active`. Der Commit nimmt den
ganzen Ordner, also gehen Änderungen, die du seit dem letzten Befehl im
Editor gemacht hast, mit. Die Geschichte ist dein Backup und dein
Rückgängig.

Mit `false` oder mit `--no-commit` an einem Befehl schreibt die Engine
die Dateien und überlässt dir das Committen. Seldon pusht nie.

Antwortest du in `seldon init` bei Git mit Nein (oder gibst `--no-git`
an), schreibt es `autocommit = false`: Das Logbuch ist kein Repository,
und `seldon doctor` meldet das als deine Wahl, nicht als Fehler. Willst
du Git später nutzen, führe `git -C <logbook> init` aus und setze
`autocommit = true`.

`seldon init` nimmt seine Git-Vorgabe aus `autocommit` in einer
bestehenden Datei: Bei `autocommit = false` legt es kein Repository an
und behält den Wert, außer du gibst `--git` an oder sagst im Assistenten
Ja.

## Einstellungen des Plugins

Du änderst sie in Omarchys Einstellungen (Setup, Plugins, Seldon) oder
mit `omarchy bar set`:

| Schlüssel | Vorgabe | Bedeutung |
|---|---|---|
| `captureIntervalMin` | `15` | Minuten zwischen zwei Erfassungen, solange die Shell läuft (5 bis 120) |
| `wipLimit` | `3` | aktive Cases, mit denen der Tab Work vergleicht (1 bis 20); es warnt, blockiert nie |

```sh
omarchy bar set jax.seldon captureIntervalMin 30 --json
```

Die Pill verschieben: `omarchy bar move jax.seldon --section right`.

## Eine Taste für den Prime Radiant

Das Plugin richtet keine Tastenbelegung ein. Füg diese Zeile zu deinen
Hyprland-Bindings hinzu, um den Prime Radiant mit Super+Shift+S zu
öffnen:

```text
o.bind("SUPER + SHIFT + S", "Seldon", "omarchy-shell shell toggle jax.seldon")
```

Das Panel hat ein eigenes Ziel. Um es über eine Taste oder ein Skript
umzuschalten: `omarchy-shell jax.seldon.panel toggle`.

---

Zurück: [Befehlsreferenz](05-cli-reference.md) · [Übersicht](README.md) · Weiter: [Das Logbuch](07-the-logbook.md)
