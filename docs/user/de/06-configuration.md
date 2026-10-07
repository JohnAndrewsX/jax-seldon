# Konfiguration

<!-- source: en/06-configuration.md @ d79db7a -->

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
watchPaths = ["~/.config/hypr", "~/.config/omarchy", "~/.config/waybar", "~/.bashrc", "~/.zshrc", "~/.local/share/applications", "~/.config/systemd/user", "~/.config/autostart", "~/.config/environment.d", "~/.config/uwsm", "~/.profile", "~/.bash_profile", "~/.local/state/omarchy/toggles"]

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
| `[drift] alwaysRed` | sechs Namen | Pakete, die Boot, Anmeldung oder die Shell brechen können, siehe [Drift](#drift) |
| `[drift] attention` | `"normal"` | `"all"` macht jede Änderung ohne Case wieder zu Drift, siehe [Drift](#drift) |
| `[drift] routine` | jede Regel | die Routine-Regeln, die gelten, siehe [Drift](#drift) |
| `[drift] routinePaths`, `routinePackages`, `alwaysRedPaths` | siehe [Drift](#drift) | Pfade und Pakete, die Routine sind, und die Persistenzpfade |
| `[agent] launcher` | `omarchy agent prompt` | was `seldon agent start` startet, siehe [Agent-Launcher](#agent-launcher) |
| `[agent.launchers]` | keine | weitere Launcher mit Namen |
| `[agent] workdir` | `"inherit"` | wo der Launcher startet; `"logbook"` startet ihn im Ordner des Logbuchs, siehe [Agent-Launcher](#agent-launcher) |

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
  ohne Desktop-Sitzung, ist er für diese Erfassung `degraded`. Über ssh
  oder aus cron klappt es, solange die Shell auf dem Rechner läuft: Ist
  dort `OMARCHY_PATH` nicht gesetzt, gibt Seldon den Omarchy-Befehlen
  `/usr/share/omarchy` mit; einen Wert, den du setzt, nimmt es, wie er
  ist.

## Beobachtete Pfade

`watchPaths` listet Dateien und Ordner. Der Config-Collector bildet bei
jeder Erfassung den Hash jeder Datei darunter und zeichnet auf, was
hinzugekommen, geändert oder entfernt ist. Er zeichnet den Pfad und zwei
kurze Hashes auf, nie den Inhalt.

Vorgaben: `~/.config/hypr`, `~/.config/omarchy`, `~/.config/waybar`,
`~/.bashrc`, `~/.zshrc`, `~/.local/share/applications` (die
Desktop-Einträge deiner Web-Apps und TUIs) und die Persistenzpfade
`~/.config/systemd/user`, `~/.config/autostart`,
`~/.config/environment.d`, `~/.config/uwsm`, `~/.profile`,
`~/.bash_profile` (Dateien, die bei der Anmeldung laufen; eine neue dort
ist eine Krise, siehe [Drift](#drift)) sowie Omarchys Toggle-Ordner
`~/.local/state/omarchy/toggles` (die Schalter aus Omarchys
*Toggle*-Menü und Hyprlands Flags; ein- oder ausschalten ist Routine,
alles andere dort wird gelistet).
Fehlende Pfade überspringt der Collector. Ein relativer Pfad wie `.config/nvim` bedeutet
`~/.config/nvim`; der Assistent speichert getippte Pfade in dieser Form.
Ergänze eigene, zum Beispiel:

```toml
watchPaths = ["~/.config/hypr", "~/.config/omarchy", "~/.config/waybar", "~/.bashrc", "~/.zshrc", "~/.local/share/applications", "~/.config/systemd/user", "~/.config/autostart", "~/.config/environment.d", "~/.config/uwsm", "~/.profile", "~/.bash_profile", "~/.local/state/omarchy/toggles", "~/.config/nvim"]
```

Der Assistent schreibt die Liste in die `config.toml`. Eine Liste, die
noch die Vorgabe eines früheren Release ist, bekommt die neuen Vorgaben
bei der nächsten Erfassung, die das einmal in einer `note:`-Zeile sagt;
nur die Zeile `watchPaths` der Datei ändert sich. Eine Liste, die du
selbst geändert hast, bleibt, wie sie ist: `seldon doctor` nennt die
Pfade, die ihr fehlen, und du ergänzt sie genauso:

```toml
watchPaths = ["~/.config/hypr", "~/.config/omarchy", "~/.config/waybar", "~/.bashrc", "~/.zshrc", "~/.local/share/applications", "~/.config/systemd/user", "~/.config/autostart", "~/.config/environment.d", "~/.config/uwsm", "~/.profile", "~/.bash_profile", "~/.local/state/omarchy/toggles"]
```

Die Dateien, die schon dort liegen, wenn der Pfad in die Liste kommt,
nimmt der Collector, wie sie sind: Die nächste Erfassung zeichnet für
sie nichts als hinzugefügt auf und meldet `watch scope changed: 0 file(s)
left it, N entered it`.

`~/.ssh/authorized_keys` und `~/.ssh/authorized_keys2` (die beiden
Dateien, die sshd standardmäßig liest) werden nur beobachtet, wenn du sie
ergänzt; ergänze beide Zeilen. Stehen sie in der Liste, ist eine Änderung
an einer davon ohne Case eine Krise (beide stehen in der Vorgabe von
`alwaysRedPaths`); aufgezeichnet werden nur die Hashes:

```toml
watchPaths = ["~/.config/hypr", "~/.config/omarchy", "~/.config/waybar", "~/.bashrc", "~/.zshrc", "~/.local/share/applications", "~/.config/systemd/user", "~/.config/autostart", "~/.config/environment.d", "~/.config/uwsm", "~/.profile", "~/.bash_profile", "~/.local/state/omarchy/toggles", "~/.ssh/authorized_keys", "~/.ssh/authorized_keys2"]
```

Immer ausgenommen:

- `~/.config/omarchy/plugins/` (das deckt der Plugins-Collector ab: Er
  hasht den Ordner jedes Drittanbieter-Plugins als Ganzes und zeichnet
  eine Änderung als ein Plugin-Update auf);
- `~/.local/share/applications/mimeinfo.cache`: ein Cache, der aus den
  Desktop-Einträgen gebaut und bei vielen Paket-Updates neu geschrieben
  wird;
- `.git`-Ordner und Ordner, die über einen Symlink erreicht werden —
  außer in den Persistenzpfaden (`alwaysRedPaths`): Dort folgt der
  Collector einem verlinkten Ordner, jedem Ordner einmal, mit höchstens
  4096 Einträgen unter Links pro Erfassung. Ein Link mit mehr wird als
  abgeschnitten aufgezeichnet, und das ist eine Krise: Niemand sieht,
  was von dort läuft. Links in dein Logbuch oder Seldons eigene Ordner
  verfolgt er nie;
- Binärdateien und Dateien über 1 MiB (als übersprungen gelistet, ohne
  Hash) — außer in den Persistenzpfaden, wo jede Datei gehasht wird: Ein
  Hook läuft, egal was er enthält. Eine Datei dort über 64 MiB wird aus
  Größe, Zeiten und Inode gehasht statt gelesen, und eine, die sich nicht
  lesen lässt, behält ihren letzten Hash, bis sie es wieder tut. Jede
  Datei im Toggle-Ordner wird genauso gehasht;
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
andere hier ist gelb. Die Zone sagt, wo eine Änderung wirkt; ob eine
Änderung ohne Case eine Krise ist, entscheidet [Drift](#drift).

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
  deren Name auf Key, Token, Secret oder Auth endet, sowie die Cookies
  nach `Cookie:` und `Set-Cookie:` (ein `name=value`; `cookie: banner
  fixed` bleibt);
- der Wert eines JSON-Schlüssels wie `"password"`, `"passwd"`,
  `"client_secret"`, `"access_token"`, `"api_key"` oder `"apiKey"` in
  eingebettetem JSON (`curl -d '{"password": "…"}'`); `"password_hint"`
  bleibt;
- AWS-Zugangsschlüssel (`AKIA…`, `ASIA…`), GitHub-Tokens (`ghp_…`,
  `gho_…`, `github_pat_…` und die übrigen `gh…_`-Formen), GitLab-Tokens
  (`glpat-…`), Slack-Tokens (`xoxb-…`), API-Schlüssel (`sk-…`, `sk_…`);
- das Passwort nach `-p` bei `mysql`, `psql` und `smbclient`, nach
  `sshpass -p` und nach `docker login -p` (auch `podman`);
- Benutzer und Passwort nach `curl -u`, die Cookies nach `curl -b`
  oder `--cookie` (kein Name einer Cookie-Datei);
- das Zertifikat und Passwort nach `curl -E`/`--cert`, der Wert nach
  `http`/`xh -a`;
- der `pass:…`-Wert von openssls `-pass`, `-passin`, `-passout` und
  ähnlichen Optionen; `env:`, `file:`, `fd:` und `stdin` bleiben stehen;
- Proxy-Zugangsdaten: nach `curl -U`, `--proxy-user` und
  `--proxy-password` sowie `user:pass@` im Proxy nach `curl -x`,
  `--proxy` oder in `https_proxy=`;
- Benutzer und Passwort in einer URL (`https://user:secret@host`), auch
  wenn das Passwort `/`, `?`, `#` oder `:` enthält.
- der Teil vor dem `@` einer E-Mail-Adresse: `me@example.com` lautet
  `‹redacted›@example.com`. Ein SSH-Remote (`git@github.com:owner/repo`),
  `user@host` ohne Punkt, Paketversionen (`pkg@1.2.3`) und
  systemd-Units (`getty@tty1.service`) bleiben; `ssh me@host.example`
  sieht wie eine Adresse aus und wird ebenfalls geschwärzt.

Text, der geschrieben wurde, bevor es eine Regel gab, bleibt, wie er ist.

Die Regeln schwärzen lieber zu viel als zu wenig. Eigene Regeln trägst
du als reguläre Ausdrücke ein; jeder ersetzt seinen ganzen Treffer:

```toml
[redaction]
patterns = ["MYAPP_SESSION=\\S+", "acme_[0-9A-Za-z]{24}"]
```

Die Domain einer Adresse bleibt sichtbar. Nennt deine dich, trag ein
Muster für sie ein: `"@smith\\.example\\b"` macht aus
`jo@smith.example` den Text `‹redacted›‹redacted›`.

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

Der Name einer Datei unter einem beobachteten Pfad ist das Subjekt
ihrer Ereignisse. Ein Desktop-Eintrag, den du nach einem Konto benannt
hast, etwa eine Web-App `Mail (me@example.com).desktop`, kommt deshalb
als `Mail (‹redacted›@example.com).desktop` ins Logbuch. Soll so eine
Datei gar nicht hinein, trag ihren Namen in `skipPaths` ein, zum
Beispiel `"*@*.desktop"` für jeden Desktop-Eintrag mit einem `@` im
Namen; schon geschriebene Ereignisse behalten den Namen, mit dem sie
geschrieben wurden.

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

Jede Änderung wird aufgezeichnet. `[drift]` entscheidet, welche
Änderungen ohne Case Drift sind und welche davon eine Krise (siehe
[Konzepte](02-concepts.md#drift)). Die Vorgaben sind leise:
Routine-Änderungen sind Geschichte, Pakete und Überschreibungen werden
ohne Aufforderung aufgelistet, und nur, was Boot, Anmeldung oder die
Shell brechen kann, ist eine Krise.

| Schlüssel | Vorgabe | Bedeutung |
|---|---|---|
| `alwaysRed` | `linux*`, `systemd`, `glibc`, `hyprland`, `omarchy`, `quickshell` | Pakete, die Boot, Anmeldung oder die Shell brechen können: außerhalb eines Case mit Namen installiert oder entfernt eine Krise; mit dem System aktualisiert Routine |
| `attention` | `"normal"` | `"all"`: jede Änderung ohne Case ist Drift, eine Krise, wenn ihre Zone rot ist (das Verhalten bis 0.1.3) |
| `routine` | alle Regeln | die Routine-Regeln, die gelten: `sysupgrade`, `upgrade`, `keyring`, `omarchy-update`, `plugin-toggle`, `theme`, `omarchy-default`, `system-link`, `routine-paths`, `theme-assets`, `theme-repo`, `toggle-flag` |
| `routinePaths` | `~/.config/omarchy/shell.json`, `**/*.bak.*` | Konfigurationsdateien, deren Änderungen Routine sind |
| `routinePackages` | `archlinux-keyring`, `omarchy-keyring` | Pakete, deren eigene Transaktionen Routine sind |
| `alwaysRedPaths` | `~/.config/systemd/user/**`, `~/.config/omarchy/hooks/**`, `~/.config/autostart/**`, `~/.config/environment.d/**`, `~/.config/uwsm/**`, `~/.profile`, `~/.bash_profile`, `~/.ssh/authorized_keys`, `~/.ssh/authorized_keys2` | Persistenzpfade: eine Änderung dort ohne Case ist eine Krise (die `authorized_keys`-Dateien erst, wenn du sie beobachtest) |

Willst du mehr? Ein paar Beispiele:

```toml
[drift]
# theme switches are drift again
routine = ["sysupgrade", "upgrade", "keyring", "omarchy-update", "plugin-toggle", "omarchy-default", "system-link", "routine-paths", "theme-assets", "theme-repo", "toggle-flag"]
# a kernel from NVIDIA counts too
alwaysRed = ["linux*", "systemd", "glibc", "hyprland", "omarchy", "quickshell", "nvidia*"]
```

```toml
[drift]
# everything without a case is drift, as up to 0.1.3
attention = "all"
```

`*` passt auf jedes Ende. Behalte die Vorgaben von `alwaysRed`: Diese
Pakete können Boot, Anmeldung oder die Shell kaputtmachen. Die Engine
schreibt diese Schlüssel nur in die Datei, wenn du sie änderst, und
`seldon doctor` gibt die geltenden Regeln aus und markiert, was nicht
der Vorgabe entspricht. `seldon drift --all` listet auch die
Routine-Änderungen.

Die Pill zählt nur Krisen. Soll sie jede Änderung ohne Case zählen oder
gar nichts, setz die Plugin-Einstellung `driftInBar` (siehe
[Einstellungen des Plugins](#einstellungen-des-plugins)).

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

Der Launcher startet losgelöst dort, wo `omarchy agent prompt` den
Agenten starten würde: im Ordner, in dem `seldon agent start` läuft, und
in `~/Work` (dein Home, wenn es keins gibt), wenn dieser Ordner dein Home
oder `/` ist, wie aus dem Panel. Agenten vertrauen `~/Work`, und Seldons
Hooks zeichnen die Sitzung dort auf, weil Seldon sie gestartet hat. Damit
Agenten wie vor 0.1.4 im Ordner des Logbuchs starten und die Hooks in den
Einstellungen des Logbuchs bleiben (es wird keine nutzerweite Kopie
angelegt):

```toml
[agent]
workdir = "logbook"
```

Seine Fehlerausgabe landet in `~/.local/state/seldon/agent-launch.log`.

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
| `driftInBar` | `crisis` | was die zweite Zahl der Pill zählt: `crisis`, `all` (jede Änderung ohne Case, wie bis 0.1.3) oder `none`; bei einer Krise nimmt die Pill in jedem Modus die Fehlerfarbe an |

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
