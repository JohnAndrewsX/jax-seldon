# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Engine

- `seldon agent start` launches the agent with `SELDON_ACTOR=agent:`
  and the launcher's name, and `SELDON_ATTENDED=1` (ADR-0027). `plan`,
  `log`, `drift` and `event` record `SELDON_ACTOR` when `--actor` is not
  given, and `hook generic` when its JSON has no `"actor"`, so a write
  by a launched agent is recorded as the agent even when it forgets
  `--actor`. An explicit `--actor` wins; a value the command does not
  accept is exit 1 and names the variable. `event` still takes the agent
  command found in the ledger first. `SELDON_ATTENDED` is for the
  agent's rules; the engine never reads it (WP-096).
- **New agent rules (ADR-0027).** A logbook's `AGENTS.md` now tells an
  agent to do the work instead of handing steps back: a case the user
  started, or work the user asked for in the session, is the agent's
  authorisation; the plan is a running note, not a gate; the agent runs
  `sudo` itself, takes the snapper snapshot of an R2 or R3 case itself
  (without Omarchy's cleanup pass) and records its number, installs the
  way the software documents (`omarchy pkg add` is a recommendation, no
  longer the only route), and verifies and closes the case. It asks
  first only outside the case's Intent, for a destructive step without
  rollback, and for an R3 step (boot, login, shell: the `alwaysRed`
  list, checked against the whole resolved transaction without
  refreshing the sync database; a system upgrade is R3 as such), one go
  per step. An unattended session records and reports only. The user's
  and area rules can only add limits. The rules sit in a block
  `<!-- seldon:begin rules v2 -->` … `<!-- seldon:end -->`; the user's
  own rules follow it (WP-100).
- `seldon rules update` brings an existing logbook's `AGENTS.md` to the
  new rules: it rewrites the block and nothing else (an edited block is
  archived first); a file from an earlier release that someone edited
  is archived to `archive/AGENTS-<date>.md`, and only the lines that
  were not Seldon's follow the new rules under `## Your rules (kept)`
  (a file nobody edited is replaced whole); `--replace` archives the
  old file and writes the new rules alone. It prints the diff, commits
  `seldon: rules update`, and changes nothing on a second run.
  `seldon doctor` has a `rules` row (`current`, `outdated (v1)`,
  `missing`, …) whose fix is that command (WP-100).

- A capture over ssh, from cron or from a systemd unit no longer
  degrades the plugins collector with "OMARCHY_PATH is not set": when
  the variable is unset or empty, Omarchy's programs (`omarchy plugin
  list|catalog`, `omarchy-version`, also in `doctor` and `dossier`) get
  `OMARCHY_PATH=/usr/share/omarchy`; a set value is passed on unchanged
  (WP-089).
- The desktop entries in `~/.local/share/applications` are watched by
  default; `mimeinfo.cache` there is excluded, as it is rebuilt on many
  package updates. A `config.toml` the wizard wrote keeps its own
  `watchPaths`; guide 06 gives the line to add. Files already present
  when the path enters the list record no addition (WP-089).
- A collector that degrades in the capture that records a state reset
  (snapper without permission, a failing `omarchy`) no longer looks like
  "never ran here" afterwards: `cursors.json` marks it `pendingBaseline`
  with what was lost (`cursors`, or `logbook` when the state was another
  logbook's), and its first successful run records its own state reset
  note with that kind, once.
  This also holds when it was the only collector that lost its state
  (WP-088).
- The same holds for a collector that the capture which loses the state
  does not run (`capture --source` without it, or disabled): it gets an
  entry in `cursors.json` with only the mark, which the index shows as a
  collector that has not run yet, and its first successful run, also
  after it is enabled again, records its gap (WP-091).
- `seldon doctor` shows a collector whose baseline waits in its own
  `state` row: degraded or not run since a state reset, and that its
  next successful capture records the gap; it says when the state was
  another logbook's instead of "cursors unreadable", and the fix is a
  capture of that collector rather than a restore (WP-091).
- When the snapper collector goes from degraded to ok or back between
  two captures (you ran the read grant, or a snapper `set-config` with
  `SYNC_ACL=yes` removed it), the capture records a `seldon` note with
  the subject `snapper` and the collector's message, once (WP-091).
- A `cursors.json` that holds an entry with only the `pendingBaseline`
  mark (a collector not run since a state reset) cannot be read by an
  engine before 0.1.4; after a downgrade move `cursors.json` aside, which
  records a state reset (WP-091).
- A capture that stops between its ledger write and the save of
  `cursors.json` (a crash, a kill) no longer makes the next capture
  write the `state-reset` note or the snapper note a second time: such a
  capture first marks the notes in `cursors.json` (`pendingNotes`), and
  the next one skips a note the ledger already holds, but still prints
  the state-reset warning the crash hid (WP-099).
- Such a crash no longer makes the next capture record a state reset for
  a source the crashed capture recorded first (its first capture, or a
  source without events before): the capture also marks those sources in
  `cursors.json` (`silentBaselines`), and the next one does not count
  their new events as lost. `seldon doctor` after a crashed state reset
  no longer says the next capture will record it; it says the next
  capture will warn of it. A `--source` capture right after such a crash
  no longer leaves the collectors it does not run waiting for a baseline
  the crashed note already recorded, which gave a second reset note
  later (WP-104).
- A collector's message (snapper's error output, for example) now goes
  through redaction before `capture` saves it in `cursors.json` or prints
  it, as the ledger's copy already did; `index.json` (`state.collectors`)
  and `STATUS.md` show it redacted too, and so do `seldon doctor` and
  the snapper line of `seldon init`. Messages an earlier release saved
  in `cursors.json` are redacted by the next capture, and by the index
  build and `doctor` until then. While a `[redaction] patterns` entry is
  invalid, the index and `doctor` show a fixed text in place of each
  collector message (WP-105).
- Seldon's own plugin and package changes that an earlier capture left
  open (the engine stopped between the two writes, or 0.1.2 and before
  recorded them) are explained by the next capture, as rule 8 explains
  new ones; a row you dismissed or resolved keeps its resolution, and
  adding or downgrading Seldon stays drift. Such a resolution is dated at
  the capture or at the event, whichever is later, so an event dated
  after a clock that moved back is no longer left as drift (WP-088).
- Redaction of command options: the rules for `curl -u`, `-U`, `-x`,
  `-b` (and `--user`, `--proxy-user`, `--proxy`, `--cookie`),
  `sshpass -p` and `docker … login -p` treat a `;`, `&` or `|` inside
  quotes as part of the command, and mask an option given twice in one
  command each time; an unquoted separator still ends the command
  (WP-087).
- Redaction: long lines with many masked values are checked against
  earlier markers by binary search.
- A build with `SELDON_BUILD=main.<sha>` says what it is: `seldon
  --version`, `--version --json` and the index's `engineVersion` report
  `0.1.3+main.<sha>`; release builds stay plain, and a value that is not
  semver build metadata fails the build. The plugin reads the marked
  form as its version (WP-098).
- The agent hook is faster: a call it does not record takes about half
  the time, a recorded command near 1000 ledger lines about 0.7 ms less.
  `skipPaths` and `alwaysRed` globs compile only when a path or package
  could match them, `seldon hook claude-code` skips the full command-line
  parser, and the index rebuild allocates less (WP-092).
- Redaction masks e-mail addresses: `me@example.com` becomes
  `‹redacted›@example.com` in the logbook and the index, so a desktop
  entry named after an account stays recognisable by its domain. An SSH
  remote (`git@github.com:owner/repo`), `user@host` without a dot,
  package versions (`pkg@1.2.3`), npm scopes and systemd units
  (`getty@tty1.service`) stay as they are; `ssh me@host.example` is
  masked like an address. Guide 06 says how `skipPaths` keeps a file
  name out of the logbook altogether, and how a pattern of your own
  hides a personal domain (WP-093).
- Two watched files whose names differ only in a masked part (two
  desktop entries named after addresses) share one subject in the
  ledger. The config collector now tells them apart by their hashes when
  it checks the ledger, after a failed cursor save and after a restored
  older state directory alike, so it records each change once and none
  for the wrong file. A file removed after another one with the same
  masked name and content is no longer taken for that one and is
  recorded, and a restore after two changes of one file no longer adds a
  change that skips the middle one (WP-103).
- The config collector's cursor marks how far the ledger's config
  events at its last check go, so the check of the ledger no longer
  depends on whole seconds. Three rare cases are fixed: a removal by a
  capture in the same second as the one before it, whose cursor save
  then failed, is no longer recorded twice (nor a file back since then
  missed); after a restored state directory, a change stamped with the
  backed-up check (an older mtime, or the same second) is no longer
  recorded again; and after a failed cursor save, the previous
  capture's change is no longer taken for a file with the same masked
  name and content. A cursor from an older version is read as before
  until the next capture saves the mark (WP-107).
- Redaction covers more forms of credentials on command lines: a
  command continued over lines with `\`, or with a quoted string that
  spans lines, is read as one command, also between an option and its
  value; redirections such as `2>&1` and `&>file`, and ANSI-C strings
  (`$'…'`), no longer end it. An option's value is read as one shell
  word, so `-u admin:'p w'`, `-u "a\"b"` and `--password $'…'` are
  masked whole. New: curl's `--pass`, `--proxy-pass` and
  `--oauth2-bearer` (and xh's `--bearer`), a client certificate with its
  password after `curl -E`/`--cert`/`--proxy-cert` (the value with `:`,
  file name included), the value after `http`/`https`/`xh`/`xhs`
  `-a`/`--auth`, and wget's `--http-password`/`--ftp-password` before a
  space (WP-097).
- Redaction covers openssl's pass phrase options: the value of `-pass`,
  `-passin`, `-passout` and the other options of that form (`-password`,
  `-keypass`, easyrsa's `--passin=…`) is masked when it gives the
  password itself (`pass:…`), so `-passin pass:…` is recorded as
  `-passin ‹redacted›`. A source such as `env:VAR`, `file:path`, `fd:N`
  or `stdin` stays as it is (WP-106).
- Redaction compiles fewer of its rules for a common curl line: a URL
  without a user and password in it, `curl -u` (without `-U`), an `-E`
  with no `:` after it (`set -e`, `curl … | sudo -E bash`) and an option
  given only once no longer compile the rules that could not match. The
  agent hook takes about 1 ms less for such a line near 1000 ledger
  lines. What is masked is unchanged (WP-108).

- **One sentence starts the work (ADR-0027 §6).** `seldon agent start
  --new -- "<what to do>"` creates a case from the sentence (title: its
  first sentence, at most 72 characters; *Intent*: the whole text),
  starts it and launches the agent on it. Without an Omarchy default
  agent nothing is created and the message names
  `omarchy default agent <name>`; a launcher that fails afterwards leaves
  the case active with the retry (WP-101).
- **The agent closes, the engine checks (ADR-0027 §5).** `plan done` by
  an agent (`--actor` or `SELDON_ACTOR`) is refused while the case's
  *Result* is empty or its *Plan* has no `Verification:` text; an
  agent's close tags the case `closed-by-agent`. `seldon plan reopen
  <ID>` makes a new active case "Reopen: <title>" with the same *Intent*
  and the tag `reopens:<ID>`; the completed case stays as it is, and
  the active-case marker stays on an open case an agent may be working.
  `--actor human` in an agent's session cannot close a case, and an
  agent's `drift explain` tags its case `closed-by-agent` (WP-101).
- `seldon plan set <ID> --zone|--risk|--area` changes an open case (one
  *Log* line), e.g. to R3 before a step that can break boot.
  `seldon plan snapshot <ID> <N>` records the rollback snapshot and
  checks it (it exists, after the start, before the first red change),
  with warnings only; `plan start --snapshot` gets the red-change check.
  A forgotten number is filled by the next capture from a snapshot whose
  description is the case id, or from the agent's recorded
  `snapper … create` (the hook now records it, with a case) in the
  snapshot's window; when two agents' commands share that window, both
  cases are told how to record it instead. A deleted
  rollback snapshot writes `rollback for <ID> pruned (snapshot N)` into
  the case and shows in `seldon doctor` (row `rollbacks`). A red change of
  an `alwaysRed` package in an open case below R3 gets an `advisory:`
  line in the case and an index warning. The agent rules name the new
  commands; `rules update` rewrites the earlier v2 block without an
  archive (WP-101).
- **Attention is earned by consequence (ADR-0028).** Every change is
  still recorded, but only what a wrong one would cost decides whether it
  needs attention. Each drift-eligible event gets a class at index time:
  *routine* (a plain full upgrade such as `pacman -Syu`, `-Syyuu`, bare
  `yay` or `omarchy update`, kernels included; an upgrade of what is
  installed, also `-U` from the cache; the keyrings; Omarchy's own update;
  a plugin toggle; a theme switch; Omarchy's own copy of a file;
  `shell.json`; `<file>.bak.<epoch>` backups; a link into `/usr/`; a
  theme's colours and backgrounds) is history in the Changelog and no
  longer drift; *attention* (a package installed, removed or downgraded
  by name, a third-party plugin added, removed or updated, an override
  under a watched path, a removed file) stays open drift, quietly;
  *crisis* is now the harm test, no longer "the zone is red": a named
  install, removal or downgrade of an `alwaysRed` package, or a new file
  in a persistence path (`~/.config/systemd/user`, Omarchy's hooks,
  `~/.config/autostart`, `environment.d`, `uwsm`, `~/.profile`,
  `~/.bash_profile`). Nothing is written to the ledger: on the first
  index build, open theme switches, toggles, routine upgrades, `omarchy
  update` rows and `shell.json` changes leave the drift list. A routine
  event an open case's Plan names is still shown, with its proposal.
  `drift[].zone` is now the ledger zone (pacman items are red) and
  `crisis` the class; contract version 1 is unchanged, only the schema
  descriptions of `crisis` and `zone` say so (WP-109).
- `config.toml [drift]` gains `attention` (`"all"` restores the rules
  before ADR-0028: the rollback), `routine` (the routine rule ids; drop
  `"theme"` to make theme switches attention again), `routinePaths`,
  `routinePackages` and `alwaysRedPaths`, written only when changed.
  `seldon doctor` prints the effective rule set and marks what is not
  the default (WP-109).
- `seldon drift --all` lists routine items too; `drift` and `drift show`
  report each item's `class` and the `rule` that gave it. `drift link`
  also takes a routine event; `drift explain|dismiss` of one exits 1.
  An agent may no longer explain or dismiss a crisis, and may link one
  only to an active case that lists it in `agents`; a human is never
  refused (WP-109).
- Six new default `watchPaths`: `~/.config/systemd/user`,
  `~/.config/autostart`, `~/.config/environment.d`, `~/.config/uwsm`,
  `~/.profile`, `~/.bash_profile`. A `config.toml` whose list is still the
  default of an earlier engine gains them at the next capture, which says
  so once; files already there record nothing. A list you wrote yourself
  is kept, and `seldon doctor` names the paths it lacks with the line to
  add (WP-109).
- New config events carry capture-time evidence in `meta.matches`
  (`omarchy-default`, `system-link`, `theme-repo`; only the fact, never
  a link target or content). The theme hook and the watcher unit (any
  `ExecStart` prefix) are recognised by their built-in templates, so a
  lost state directory or `install.sh --unit` no longer leaves a crisis.
  Omarchy's own copies count only from a root-owned `$OMARCHY_PATH` that
  is neither group- nor world-writable; `seldon doctor` says when they
  do not (WP-109).
- `series.drift` changes for the past too: a routine change opens
  nothing, and a resolution counts only when the event it resolves
  opened an item, so the curve can no longer go negative. Old charts
  show fewer opened and resolved items (WP-109).

### Plugin

- **Quiet surfaces (ADR-0028).** The bar's second number now counts
  crises only: changes that can affect boot, login or the shell and have
  no case. Other changes without a case no longer show in the bar; the
  new setting `driftInBar` (`crisis`, the default; `all`, the behaviour
  up to 0.1.3; `none`) changes that, e.g. `omarchy bar set jax.seldon
  driftInBar all`. The bar still turns to the error colour on a crisis in
  every mode. The tooltip reads "Seldon — 2 active cases, 1 crisis, 7
  changes without a case, last capture …". The red strip appears only for
  a crisis and reads "N changes that can affect boot, login or the shell
  have no case". The Changelog shows a quiet "N changes without a case"
  line under its header; open rows say "Crisis · no case" or "No case"
  instead of "Needs a reason" / "Unexplained", and are coloured by
  whether they are a crisis (urgent) or not (accent), no longer by zone;
  every other row (resolved, with a case, routine) has a muted stripe
  whatever its zone. The Today counts and the Changelog's "+N more …"
  line say "without a case" instead of "open drift"; Today counts the
  changes without a case that are no crisis.
  The drift sheet says "RESOLVE A CRISIS" and "<zone> · crisis" for a
  crisis in any zone and keeps a slot for *Ask agent* above Link /
  Explain / Dismiss. The Today pictogram no longer changes for changes
  without a case. Label skew: a plugin up to 0.1.3 with a 0.1.4 engine
  still says "red zone" where "crisis" is meant (the strip, the tooltip,
  the sheet); behaviour is the same, only the labels are wrong. Update
  the plugin with the engine (WP-110).
- **Run (WP-101).** The Work tab starts a case with one sentence: type
  what to do, Enter or *Run*; the engine makes and starts the case and
  launches your agent, the cursor goes to the new case, and a refusal
  (no default agent) says how to fix it. A case an agent closed reads
  "by agent"; *By agent* narrows Completed to those cases for a spot
  check. Every completed case has *Reopen* (one click, key `r`). When
  the logbook's agent rules are outdated, the panel says so and *Update
  rules* runs `seldon rules update`.
- After `omarchy plugin update jax.seldon` the shell keeps running the
  old plugin code until it restarts. The panel now notices this: when
  the installed manifest names another version than the code running,
  it shows "Restart the shell to finish the update" with one button,
  *Restart shell*, which runs `omarchy-restart-shell` (no arguments).
  Plugins up to 0.1.3 do not show it; the update guide and both READMEs
  say to restart the shell after every plugin update (WP-090).

### Packaging and docs

- CONTRACT.md lists the reserved case tags (`closed-by-agent`,
  `reopens:<ID>`, `imported`) and the plugin's new commands; no schema
  change. The sample logbook's C-2026-002 was closed by an agent and its
  Omarchy update case is R3; the variant `case-reopened` shows a reopen
  (WP-101).

- `just deploy-test-host <main check log>`
  (`scripts/deploy-test-host.sh`) puts the main build of engine and
  plugin on the test host after a green main check: only a host listed
  in the git-ignored `scripts/guard-hosts.local` whose machine-id
  matches its pin in the git-ignored `scripts/deploy-hosts.local`, only
  from a clean, pushed `main` whose check log ends in `exit 0` and names
  a commit with the same engine, plugin, schemas and script. It builds
  as a release does (`--features watch`), keeps the previous engine as
  `seldon.prev`, restarts an active `seldon-watch.service` on the new
  binary, moves the release plugin clone aside once, restarts the shell
  only when the plugin changed and the session is unlocked ("restart
  pending" otherwise), runs a smoke check and logs one JSON line on the
  host; `--dry-run` shows the plan, `--release vX.Y.Z` brings the host
  back to a release. Productive machines keep running releases only
  (WP-098).

## [0.1.3] - 2026-10-05

### Engine

- `doctor` warns before the capture that would record a state reset:
  while the cursors in `~/.local/state/seldon` are missing, unreadable
  or bound to another logbook and the ledger already holds events of
  those collectors, a degraded `state` row says that the next capture
  will record a state reset, with the fix to restore the state
  directory from a backup now or run `seldon capture` to accept the new
  baseline (for another logbook: nothing to restore). It uses the
  capture's own rule, so it names what that capture would record, and
  never shows for a fresh logbook or a collector that never ran here
  (WP-083).
- snapper keeps one date per snapshot: a snapshot made in the repeated
  hour when summer time ends no longer gets a false `snapshot-delete`
  plus `snapshot` when the collector switches between `snapper list`
  (local time) and the info files (UTC). Such a list time is resolved
  against the snapshot's info file, the cursor, then the number order;
  cursors written by earlier versions move to the info file's date
  without an event, and the ledger dedupe counts either instant, also
  after a failed cursor save or a lost state directory (WP-082).
### Plugin

- A capture the plugin runs (the timer, *Capture now*, *Check again*, the
  `c` key, the bar's right click) that returns warnings shows them in a
  neutral "Capture warned" notice on every tab: the first line of each
  warning, all of it on hover, such as the state reset and the engine's
  restore hint. A later capture without warnings clears it (WP-085).
- Redaction covers proxy credentials (`curl -U`, `--proxy-user`, wget's
  `--proxy-password`, `user:pass@` without a scheme after `curl -x`,
  `--proxy` or `https_proxy=`), secrets in inline JSON (`"password"`,
  `"passwd"`, `"…secret"`, `"…token"`, `"…api_key"`, `"…apiKey"` keys;
  not `"password_hint"`) and cookies (`Cookie:`/`Set-Cookie:` values that
  start with `name=`, `curl -b`/`--cookie` with `name=value`). The
  option, key or header name stays visible (WP-084).
- Redaction stays fast on long lines with non-ASCII text, the
  `‹redacted›` marker included: the rules use ASCII word boundaries
  (a 16 KB curl line 0.13 ms instead of 1.2 ms), and a curl line
  compiles a curl rule only when it holds that rule's option (WP-084).
- Seldon updating itself is no drift: a capture explains every new event
  of its own plugin `jax.seldon` (updated, enabled, disabled) and of its
  own package `jax-seldon` (upgraded, reinstalled) with an `explained`
  resolution (`seldon's own plugin`, `seldon's own package`). The event
  stays in the Changelog with its actor. Adding the plugin, installing or
  downgrading the package, and removing either stay drift: only the id
  is matched, nothing checks where the code came from. `capture --json`
  adds `explainedSelf` (WP-086).

## [0.1.2] - 2026-10-04

### Engine

- A lost or corrupt state directory leaves a trace: when a collector
  has to start over because `~/.local/state/seldon` (its cursor,
  `manifest.json` or `owned.json`) was missing, unreadable or bound to
  another logbook although the ledger already holds its events,
  `capture` writes one `seldon` note `state-reset` naming the
  collectors and files, prints a warning (also in `--json` `warnings`,
  and on stderr from `hook session-stop`) that points at the restore
  steps, and `doctor` shows a degraded `state` row until the next
  capture. A corrupt `owned.json` is moved to `owned.json.bad`. The
  first capture of a logbook, and the first successful run of a
  collector that was degraded or disabled until then, is no reset
  (WP-081).
- snapper's permission error is recognised in every locale: snapper now
  runs with `LC_ALL=C`, so under `LANG=de_DE.UTF-8` the snapper collector
  reports `NO_PERMISSIONS` again and `doctor` and `init` show the
  `set-config` fix line (fixes #1).
- `outputs/REBUILD.md` quotes the names in its commands: package, unit
  and theme names, plugin ids and URLs are checked first and
  single-quoted when they are not plain; an item with an invalid name is
  listed as "not reproduced: invalid name" without a command, and
  `seldon rebuild` warns about it (WP-059).
- The agent hook reads the case under the state lock: two parallel tool
  calls, or a `plan` step while an agent works, no longer drop event ids
  from the case, undo a status change or leave the case in two folders.
  It waits up to 8 s for the lock (was 2 s), rebuilds the index outside
  its critical section, and leaves the rebuild to the next `capture` or
  `status` once the ledger has more than 1000 lines, so a recorded
  command stays under 5 ms up to 1000 lines and costs about 2 ms above. `index --check` reports a
  case id found in two files (WP-057).
- `hook session-stop` runs every step even when one fails (each failure
  is one line on stderr) and writes `STATUS.md` before its commit
  (WP-057).
- A journal day file without frontmatter (Obsidian's daily note) gets the
  block in front instead of failing `log`, `plan done` and session-stop;
  `log` and `plan done` read the day before they write the ledger, so a
  day file with broken frontmatter fails them before anything changes
  (WP-057).
- `hook session-start` prints the logbook context as a quoted block: each
  line from the logbook starts with `>`, under one line that says these
  lines are data, not instructions; only day files are read as the
  journal. `agent start` passes only the case id, the logbook path and the
  commands to read the context to the launcher. The launcher check refuses
  more programs that run their arguments as code (`script`, `watch`,
  `flock`, `ssh`, `tmux`, `xargs`, interpreters, `env -S`, …), compares
  names without a version suffix, and its error calls it a heuristic
  (WP-058).
- `seldon log --actor agent:…` refuses a note that contains a line break
  (exit 1, nothing written); a person's note may still have several lines
  (WP-058).
- Snapshots are read from the info files (`/.snapshots/<number>/info.xml`)
  when `snapper list` is not permitted; the events are the same, so
  switching between the two adds none. `doctor` and `init` say what the
  snapper fix grants (WP-060).
- Atomic writes follow symbolic links, keep the file's mode and create
  new logbook, config and state files 0600 in 0700 directories, whatever
  the umask; a failed write removes its temp file, and a new logbook's
  `.gitignore` ignores `.*.tmp-*`. Rewrites are synced to disk, except
  files the engine rebuilds (`index.json`, `STATUS.md`, ledger views,
  `outputs/REBUILD.md`); a recorded agent command therefore takes about
  13 ms on a btrfs disk (was 7 ms). A program the engine runs gets its own
  process group: at the timeout the whole group is stopped, and a helper
  that keeps the output pipe open no longer holds the engine past it.
  git stays in the engine's group, so a commit hook or signing prompt can
  still use the terminal (WP-064).
- Agent hooks record and print context only for sessions inside the
  logbook: `hook claude-code`, `hook generic`, `hook session-start` and
  `hook session-stop` do nothing when the session's directory
  (`CLAUDE_PROJECT_DIR`, else the payload's `cwd`) lies outside it.
  This changes existing installs in a user-wide settings file such as
  `~/.claude/settings.json`: sessions in other projects are no longer
  recorded; `[hooks] scope = "all"` in `config.toml` keeps the old
  behaviour. `hook install --settings` warns when the file is outside the
  logbook. `skipPaths` also applies to recorded commands: a command line
  that names a matching path is recorded as `<program> ‹redacted›`
  (WP-063).
- A PostToolUse hook checks for an already recorded tool call under the
  state lock, so two calls for one tool call write one event; when the
  lock stays held past the wait, stderr says the command was not recorded.
  `seldon plan show` prints the case file as quoted lines (`> `) under
  the same note `hook session-start` uses; `--json` is unchanged (WP-063).
- The autocommit commits only into the logbook's own repository: git
  runs with `GIT_DIR`, `GIT_WORK_TREE`, `GIT_INDEX_FILE` and the other
  repository variables removed, so a `seldon` started from a git hook or
  with an exported `GIT_DIR` no longer commits the logbook into another
  repository, and an empty or broken `.git` no longer lets git commit
  into a repository around the logbook. A detached HEAD is not committed.
  A commit that is not made is one warning line on stderr and the `error`
  of `--json` `git` (exit stays 0). `doctor` reports an unusable `.git`,
  a stale `.git/index.lock`, a read-only `.git`, a detached HEAD and a
  committer git cannot resolve as degraded, each with a fix line, without
  writing into `.git` (WP-061).
- `import omarchy-agent --apply` commits the logbook's pending changes
  first (`seldon: before import omarchy-agent`) and refuses a work tree
  it cannot commit. The undo of a failed apply names only the files the
  import wrote, instead of `git checkout -- . && git clean -fd`, which
  also removed uncommitted data that was not the import's; a kept undo
  that names other files is not printed (WP-061).
- More built-in redaction rules: `--token`/`--secret`-style options and
  `…PASSWORD=`/`…_PWD=`-style assignments (any value), `--api-key` and
  `…KEY=` (when the value looks like a credential, so `sort --key=2`
  stays as it is), `X-…-Key:`-style headers, GitHub, GitLab and Slack
  tokens, `sk_` keys, `curl -u`, `sshpass -p` and registry `login -p`;
  a URL password may contain
  `/ ? # :`. Notes and their tags, case and decision titles, step
  reasons, drift explanations, event subjects and every `meta` value are
  redacted before they are written, so the journal, case and decision
  files and `STATUS.md` hold the same masked text as the ledger. The
  rules are compiled once per process and only when a text can match
  them, so a recorded agent command is faster than before. Existing files
  and history are not rewritten (WP-062).
- Saving a case keeps valid hand-edited frontmatter valid: a blank line
  or a column-0 comment inside a block list stays with the list, a quoted
  key (`"title": …`) is the same key, and every frontmatter update is
  read back before the file is written; one that would not read back is
  refused with exit 1 and the file stays as it was. A file that starts
  with a UTF-8 BOM (kept on save) or has spaces or tabs after a `---`
  fence now parses (WP-066).
- `STATUS.md` and `DECISIONS.md` no longer grow on each `status` when a
  case or decision title (or a drift subject, a collector message)
  contains a fence marker: `<!-- seldon:` in a value is written with a
  zero-width space after `<!--` (WP-065).
- `status` no longer deletes the user's text in `STATUS.md` when a marker
  line was removed by hand: the file stays as it is, with a warning, until
  the markers are restored. A `STATUS.md` with CRLF line ends merges
  (WP-065).
- One ledger line torn inside a multi-byte character (`ü`) no longer stops
  `status`, `index`, `drift` and `capture`: the month is decoded line by
  line, the bad line skipped, and one warning names the month and the
  count. A ledger line with an actor or a case the engine would refuse to
  write is skipped the same way (WP-065).
- Changing `watchPaths` or `[redaction] skipPaths` no longer floods the
  drift list: files that leave the watched scope are not recorded as
  removed, files that enter it are taken as they are, and the capture
  says so in one line. `manifest.json` keeps the scope of each
  generation (WP-069).
- `[redaction] skipPaths` has a default for the state, history, cache
  and log files that shell plugins rewrite under `~/.config/omarchy/*/`,
  and `init` points at `skipPaths`. An empty list, as `init` wrote it
  before, means the default; a list of one's own replaces it (WP-069).
- A relative `logbook` or `watchPaths` value in `config.toml`, and
  `$HOME/…`, lie under the home folder instead of the folder `seldon`
  runs in, so the plugin and the agent hooks watch the same files; the
  wizard stores typed watch paths as `~/…` (WP-069).
- The config collector drops events the ledger already has, so a
  capture whose cursor save failed after the ledger write repeats no
  config event; it reuses the stored hash of a file whose size, mtime,
  ctime and inode are unchanged, and SHA-256 no longer copies its input. A
  watched file whose name holds a control character is skipped with a
  warning (WP-069).
- `doctor` finds what makes captures and commands fail: an invalid
  `[redaction] patterns` entry, a corrupt or unreadable `cursors.json`,
  `manifest.json` or `owned.json` (each an error with its fix), ledger
  lines that are skipped (per month, with the count), a `STATUS.md` or
  `DECISIONS.md` fence that `status` leaves alone, an end marker that
  closes no fence, a case id in two files, and every enabled collector
  whose last capture failed (with the collector's message and fix). With
  a `config.toml` that cannot be read or does not parse it exits 1 with
  a fix line and reports the logbook as "not checked" instead of checking
  the default path and exiting 3 (or a bare exit 2 for an unreadable
  file). The omarchy and snapper probes honour `SELDON_OMARCHY_VERSION`
  and `SELDON_SNAPPER`. doctor stays read-only (WP-070).
- A corrupt `cursors.json` is shown in the index: every enabled collector
  is `ok: false` with the message, and `index` and `status` warn
  (WP-070).
- A case id in two files makes `index --check` exit 1 (was 2) with a
  plain message and the fix; `index` and `status` warn about it
  (WP-070).
- Package attribution reads an agent's command line with the hook's own
  parser, so `sudo -u root pacman -S x`, `timeout 600 yay -S x`, `bash -c
  'yay -S x'` and the other lines the hook records as package commands
  now attribute the transaction to the agent and its case. The hook and
  attribution know `pkexec`, `run0` and option clusters such as `sudo -Eu
  root`; `sudo -k <command>` counts as running the command. `yay
  --version`, `-V`, `-h`, `--help` and yay's `-P` and `-G` change no
  package (no event, no full upgrade that claims a person's `-Syu`);
  `yay -Yc` is a removal. `>& file` is a write. Heredoc bodies inside
  `$(…)`, backticks and `<(…)` are cut from the record like top-level
  ones, `<<` inside `((…))` is no heredoc, and a heredoc's delimiter line
  stays in the record when commands follow it. The classifier follows
  `pushd`, `popd`, `env -C` and a program's `-C DIR` like the `skipPaths`
  check; a variable the line sets (`F=x; … $F`) is read with its value,
  and a path with a glob or an unknown part is checked against
  `skipPaths` for every path it can name, where a path under an unknown
  folder (`$TMPDIR/yay.log`) only matches a pattern's literal last
  components (WP-071).
- `init` takes the language from the locale when `config.toml` has no
  `language` key (a file written by hand before `init`, e.g. with
  redaction patterns only, used to give an English logbook). The key
  sets the language of a new logbook; an existing logbook keeps its own
  in `.seldon/logbook.toml`, and the user guide no longer says otherwise
  (WP-074).
- `init` saves `config.toml` before it writes the logbook, and the
  layout writes `.seldon/logbook.toml` last: a config that cannot be
  saved no longer leaves a logbook that the next `init` refuses. When
  the layout fails (read-only parent, disk full, a mistyped path),
  `init` puts `config.toml` back as it was and removes what it created,
  so the working logbook stays the configured one (WP-074).
- `init --no-git` writes `[git] autocommit = false`, so `doctor` no
  longer reports the chosen setup as degraded with a `git init` fix.
  Without `--git`/`--no-git`, `init` takes the git choice from an
  existing config's `autocommit`, so a hand-set `false` stays; configs
  that `init` does not rewrite keep their value (WP-074).
- `init --theme-hook` takes the state lock before it writes the hook
  and holds it through the own-write record: a capture that starts
  meanwhile gets exit 4 instead of reporting the hook as drift. While
  another `seldon` holds the lock, the step writes nothing (WP-074,
  WP-052).
- `import omarchy-agent --apply` writes one ledger note of its own before
  the first file, so a vault without cases (only journal and knowledge)
  is no longer imported a second time after a failed apply or a deleted
  `.seldon/` (WP-075).
- `import omarchy-agent` lists a vault file whose name is not UTF-8, or
  that cannot be read, as an error of the report instead of stopping
  with a wrong "No such file" error; a vault folder with such a name is
  read. A kit file with a UTF-8 BOM or spaces after a `---` fence
  imports (WP-075).
- One `system/*.md` that is not UTF-8 or not readable no longer stops
  `seldon dossier`: the file is skipped with a warning and never written,
  and the other fences are built. Dossier fence bodies get the same
  zero-width space after `<!--` in `<!-- seldon:` as `STATUS.md`, so a
  value can neither end nor open a fence (WP-075).
- `seldon watch` also watches `areas/`: a new, renamed or removed area
  rebuilds the index (WP-075).
- The plugins collector drops events the ledger already has since its
  last check, so a capture whose cursor save failed after the ledger
  write repeats no plugin event. When the config collector's cursor is
  behind the manifest (a failed cursor save), the config events recorded
  since are taken into account, so a file that went back to its old
  content before the next capture is recorded as changed back (WP-073).
- A plugin removal, enabling or disabling, and a config file removal,
  carry the capture time; such a change happened after the collector's
  last check, so the agent command that caused it may now lie up to
  10 minutes before that check. With the default 15-minute capture
  interval these changes are no longer drift by `system` (WP-073).
  Within that window a cause must be later than the newest recorded
  event of the same subject: an agent's recorded `omarchy plugin
  disable x` does not claim a second disabling of `x` by a person
  (WP-073).
- `omarchy plugin <verb> <id>` proves only a plugin event of its own
  kind (`add` also an enabling): an agent's `omarchy plugin update x`
  no longer claims a person's enabling of `x` (WP-073).
- `seldon capture` runs on `SELDON_NOW` like every other command: event
  times, month files, `lastRun` and the collectors' last check follow
  it (WP-073).
- The snapper collector notices a snapshot number that was deleted and
  used again before the next capture (another date): it records the
  deletion of the old snapshot and the new snapshot, both at the new
  snapshot's date. Cursors from before keep their snapshots and learn
  the dates without events (WP-073).
- The agent hooks ignore an empty `watchPaths` entry, as the config
  collector does, instead of watching the whole home folder (WP-073).
- The index stays under its 1 MB budget with long texts: in
  `index.events` and `index.drift`, a `detail`, `resolutionDetail` or
  `meta` value longer than 256 bytes is cut and ends in
  `… (N more characters in the ledger)`; the ledger, the ledger views and
  the member events of `drift show` keep the full text, while `drift
  list` and the item of `drift show` show the clipped value. An index that
  still reaches 1 MB (many open cases) makes `index` and `status` warn and
  name the largest section (WP-076, ADR-0025).
- `log`, `event`, the agent hooks (claude-code, generic) and `drift link`
  check a case's save before they write the ledger: a case whose
  frontmatter would not read back after the change (WP-066) now fails the
  command with nothing written, instead of leaving a ledger line or
  journal entry without the case update (WP-077).
- Validation errors name a refused id or value escaped (`\u{1b}`), so a
  control character in a case, decision, journal, area or kit-case
  value, in a `hook generic` payload or in a case file name never
  reaches the terminal (WP-077).
- `plan list` lists the other cases when one case file does not load, and
  names that file in a warning line (`warnings` in `--json`); it exited 1
  before (WP-077).
- The snapper fix is a read grant on the snapshot directory,
  `sudo setfacl -m u:$USER:rx /.snapshots`, instead of adding your user
  to the snapper config's `ALLOW_USERS`, which also allowed creating,
  changing and deleting root snapshots; Seldon reads the snapshot info
  files with it. `doctor`, `init` and the collector's message print the
  new line and say what it grants. When your user is still listed in
  `ALLOW_USERS`, `doctor` (and `init`) print the revert
  `sudo snapper -c root set-config ALLOW_USERS="" SYNC_ACL=no` followed
  by the read grant (WP-079, ADR-0026 supersedes ADR-0011).

### Plugin

- The "Snapshots not readable" banner has a third action, *Check again*
  (WP-054), which runs a capture (the same call as *Capture now*), so the
  banner clears right after the snapper fix instead of at the next
  automatic capture; after *Run in terminal* it says "When the command
  has finished, press Check again" (fixes #2).
- The "Snapshots not readable" banner names what the snapper fix grants:
  read access to the snapshot directory listing and the snapshot info
  files, no snapshot creation, change or deletion (WP-060, WP-079).
- A tab change gives the keys back to the panel: a note or a new case
  typed on a tab you left is no longer sent by an Enter on another tab;
  the draft and an open sheet stay (WP-067).
- The Changelog cursor stays on its event when a new index adds rows
  above it, so Enter opens the drift sheet or row you chose (WP-067).
- On a bar with several monitors only one widget registers the
  `jax.seldon.panel` IPC target, so the shell no longer logs "another
  handler is registered"; the next widget takes the target over when the
  owner's monitor goes (WP-067).
- Every engine call that fails leaves one line in the shell journal
  (`journalctl --user -t omarchy-shell`): the command, its exit code and
  the first line of its error (WP-068).
- A capture that finds another seldon holding the lock (exit 4) is tried
  again after 30 s, up to three times; meanwhile the Changelog says
  "waiting for another seldon process" instead of showing an error
  (WP-068).
- *Create* in the new-case sheet and the drift sheet's action no longer
  do nothing while another case or drift action is pending: the sheet
  says "Another action is running — try again in a moment" (WP-068).
- The drift sheet keeps its first Enter and its notice when a new index
  arrives with the same item; only a change to the form disarms (WP-068).
- An engine older than the plugin's `engineMin` gets an "Engine too old"
  banner with the update command (WP-068).
- An engine error whose message has several lines leaves one journal
  line too: the warning keeps the message's first line (WP-078).
- *Create* in the new-decision sheet no longer does nothing while a
  decision sent from another monitor's panel is pending: it says
  "Another action is running — try again in a moment", like the
  new-case and drift sheets (WP-078).
- The Changelog's *Capture now* button captures at once while a locked
  capture waits for its retry, as the bar's right click and the `c` key
  already did; before, a click on *Capturing* did nothing (WP-078).
- The plugin README's States table lists the "Engine too old" banner
  and its fixes (WP-078).
- In the bar's centre section (once a centre anchor is set, the
  default) `jax.seldon.panel` belongs to the widget you can see, not to
  the zero-size placeholder the bar mounts for it, so
  `omarchy-shell jax.seldon.panel open` opens the panel under the pill;
  the target follows a bar reconfiguration (WP-078).
- The "Snapshots not readable" banner offers the read grant
  `sudo setfacl -m u:$USER:rx /.snapshots` instead of the `ALLOW_USERS`
  opt-in (WP-079, ADR-0026).

### Packaging and docs

- `scripts/validate-fixtures.py`, the reference `derive()`, clips the
  index texts exactly as the engine does (ADR-0025: 256 JSON bytes, the
  same marker and count); `--derive LOGBOOK` prints its derivation, and an
  engine test compares it with `seldon index` on long texts (WP-077).
- The uninstall guide (en, de) removes the hooks of a kept logbook with
  `seldon hook uninstall claude-code` and the global ones with
  `--settings`; the logbook guide (en, de) gains "Back up and restore the
  state directory"; the watcher README says the release binary and the
  AUR package already include `watch` (WP-056).
- CI: the workflow actions are pinned to commits and the build container
  to an image digest, each with its version as a comment; `cargo audit`
  gates the release build, so a dependency advisory stops a release
  unless `packaging/audit-ignore.txt` accepts it with a reason and an
  expiry; the weekly audit stays advisory (WP-072).
- `just check-perf` (not in `check`): SPEC-ENGINE §1's budgets at the
  stated scale on an optimised build — `status` at 10 011 ledger lines,
  304 cases and 365 journal files < 100 ms, `hook claude-code` at 10 000
  lines and just below the 1000-line rebuild threshold < 5 ms, and the
  ×150 index build (10 650 lines). `just bench` (CI) asserts ×10 and only
  prints ×150; a median over budget is measured once more. The panel
  harness's `work-live` step waits for each engine step's result
  instead of catching "Completing …" under load (WP-076).
- Getting started, configuration and troubleshooting (en, de) give the
  read grant as the snapper fix; troubleshooting explains the revert of
  the old `ALLOW_USERS` opt-in. The AUR package's snapper optdepend says
  it needs read access to `/.snapshots` (WP-079).
- Release provenance: the release workflow attests the engine tarball,
  the source tarball, `SHA256SUMS` and `install.sh` with GitHub artifact
  attestations (`actions/attest-build-provenance`, also in a dry run).
  `install.sh` verifies the engine tarball with `gh attestation verify`
  when the GitHub CLI is installed and logged in, accepting only the
  release workflow's attestation for the release's tag; a failed check
  installs nothing. Without `gh`, or for a release up to v0.1.1, one note
  says only the checksum was checked; the new `--require-verified`
  refuses instead, and `--skip-provenance` leaves a failing `gh` out on
  request. SECURITY.md gains "Verifying a release"; the READMEs,
  the install and update guides (en, de), packaging/README.md and the
  tag flow in docs/VERSIONING.md describe it (WP-080).

## [0.1.1] - 2026-10-02

### Engine

- Files the engine installs for you — the Claude Code hooks, the theme
  hook, the harness launcher — are recorded as Seldon's own writes
  (`owned.json`), so the capture after `seldon init` explains them
  instead of opening drift (WP-038).
- `seldon import omarchy-agent <vault>` imports the omarchy-agent kit's
  Obsidian vault into the logbook: cases, sessions, knowledge and
  deviation rows, with redaction and a dry run by default; `--apply`
  writes once, marked, and reports id collisions it renumbered (WP-043).
- `seldon plan start` warns when an R2 or R3 case starts without a
  snapshot (`warnings` in `--json`); R3 also asks for the human's explicit
  go per step. Advice only, never refused (ADR-0023, WP-050).
- The default `[drift] alwaysRed` list follows ADR-0023's R3 subjects: new
  `omarchy-settings`, `limine*`, `grub`, `mkinitcpio*`, `filesystem` and
  the login path `pam`, `sddm`, `uwsm`; `linux*` is narrowed to the
  kernels (`linux`, `-lts`, `-zen`, `-hardened`, `-rt`, `-rt-lts`,
  `-omarchy`), so firmware and header upgrades stay routine.
  `init` writes the list into `config.toml`, so an existing config keeps
  its old list; add the new globs by hand.
- `seldon decide` and `seldon status` fill the `decisions.index` table in
  the logbook's `DECISIONS.md` from `decisions/`; text outside the fence
  stays yours.
- A generated fence whose begin marker lost its end marker is now left
  alone with a warning (`dossier`, `decide`, `status`; an error in
  `import`) instead of getting a second fence that a later run would
  replace together with your text.
- `seldon doctor` prints `~`-shortened paths in the `logbook` row, like
  its header; the wizard's harness question says how to toggle and
  confirm.
- SPEC-ENGINE no longer lists `hook install generic`: there is nothing to
  install, other agents pipe into `seldon hook generic` themselves.
- `seldon completions bash|zsh|fish` prints a completion script and
  `seldon mangen` the man page seldon(1), both generated from the help
  (WP-049).
- `seldon hook uninstall claude-code` and `seldon init --remove-theme-hook`
  undo what `hook install` and `init --theme-hook` installed, and nothing
  else; the next capture explains the removal instead of opening drift.
- Every `--help` reviewed: one sentence per command, value names that say
  what they are (`<ACTOR>`, `<ZONE>`, …), a line for every argument,
  examples for `--since` and free text after `--`. `--snapshot` is
  offered by `plan start` only.
- Piping a command's output into a reader that stops early (`| head`)
  no longer crashes the engine.

### Plugin

- The panel is 460 px wide and tab labels no longer clip at the default
  font size (WP-039).
- *Update in terminal* on the "Index format mismatch" banner runs the
  GitHub installer while the AUR package does not exist, the same
  one-liner as *Install in terminal* (ADR-0024); it flips back to the
  AUR command when the package is live.
- The Prime Radiant mark replaces the `⟡` fallback glyph (WP-051). The
  pill shows the bar glyph before the counts (`2 · 3`): hand-hinted at the
  16 and 20 px boxes of scale 1.0 and 1.25, the vector at every other
  scale, its centre on the digits' centre (within 1 px, measured by the
  new `tests/plugin/bar-view.sh` in three themes), in the counts' colour.
- The panel header is the mark and "Seldon" with the designer's metrics.
  The status banner shows its state pictogram (engine missing, logbook not
  initialised, index missing, index stale), 48 px in the panel and 96 px
  in the Prime Radiant; the Today tab shows the day's state (crisis, open
  drift, active cases or all clear).
- The Timeline draws releases as diamonds, snapshots as dots, crises as
  the Prime Radiant spindle (no longer a second diamond) and case spans
  between brackets, with a legend in its title row.
- Every image follows the theme: the masks are tinted through their SVG
  root colour, no colour is written into QML. The images are copies under
  `plugin/assets/`; `preview.png` is re-rendered.

### Packaging and docs

- The AUR package installs the man page and the bash, zsh and fish
  completions; `install.sh` installs the man page and the completions of
  the shells you have under its prefix and removes them on `--uninstall`;
  the release's binary tarball carries both (WP-049).
- `assets/` holds the Prime Radiant files of record: the round-3 masks,
  the round-2 rasters and brand files, `DELIVERY.md`, `LICENSE` and a
  README listing every file and where the project uses it (WP-051). Both
  READMEs open with the hero image.
- New README for the project and the plugin repository (WP-046): what
  Seldon is and why, six features, a quick start from install to the
  plugin, a 60-second tour and a table of every document. The developer
  reading order and layout moved to `docs/DEVELOPMENT.md`. `docs-check`
  now also checks both READMEs, `docs/DEVELOPMENT.md` and `llms.txt`,
  including links into the public repositories and plugin links that
  must survive the subtree split.
- User guide in English and German (WP-045): `docs/user/en/` and
  `docs/user/de/` with thirteen pages each, from getting started to the
  glossary, a style sheet and a translation policy (English is the
  source; each German page names the commit it matches).
  `just docs-check`, part of `just check`, checks links, the page sets and
  their structure, and every `seldon` command in the guide against the
  engine's `--help`; the CLI reference is the engine's own help text.
- Repository hygiene (WP-048): CONTRIBUTING.md, SECURITY.md (GitHub
  private vulnerability reporting), CODE_OF_CONDUCT.md (Contributor
  Covenant 2.1), issue forms for bugs and features, a pull request
  template, docs/VERSIONING.md (SemVer, `contractVersion`, tag flow).
- Release notes come from the version's CHANGELOG.md section; the
  release workflow (dry run included) fails when it is missing
  (`packaging/release-notes.sh`).
- `cargo audit` runs weekly and on lock-file changes as a non-blocking
  advisory workflow (`audit.yml`); the plugin repository has a security
  policy too.

## [0.1.0] - 2026-10-02

First release. Engine `seldon` (Rust, static musl binary as the release
asset, AUR package against glibc per ADR-0022) and the Omarchy shell
plugin `jax.seldon` (published from `plugin/` as `jax-seldon-plugin`).

### Engine

- `seldon init` wizard: logbook layout, config, collectors, watched config
  paths, backfill with `--since`, pre-Seldon baseline, Obsidian vault,
  Claude Code / Omarchy-Agent harness hooks, theme hook opt-in, git
  autocommit; templates in English and German.
- Collectors: pacman (transactions, attribution), snapper (degraded until
  the user allows it, ADR-0011), omarchy, plugins, theme, config
  (manifest, redaction per SPEC-ENGINE §7). Idempotent captures.
- Ledger (append-only JSONL, contract v1), index.json for the plugin,
  generated STATUS.md and ledger views; `status`, `index`, `doctor`.
- Cases: `plan new|start|verify|done|drop|list|show`, journal, `log`,
  `event`, `decide`, `open`; agent attribution through hooks (ADR-0017,
  ADR-0019, ADR-0021); `agent start` with a config-driven launcher.
- Drift: `drift list|show|link|explain|dismiss` with transaction groups
  (ADR-0013), crises first, 200-item cap (ADR-0020).
- `rebuild` → outputs/REBUILD.md (seven sections a fresh install can
  follow); `dossier` → generated fences in system/*.md incl. the explicit
  package list with Omarchy-base vs user origin.
- `watch` (feature `watch`, off by default, ADR-0005) with a documented,
  never-enabled systemd user unit.
- Exit codes 0/1/2/3/4; every command supports `--json`.

### Plugin

- Bar pill (active cases · open drift), panel with Today, Changelog,
  Work, Decisions, System and Memory tabs, keyboard per SPEC-PLUGIN §5,
  QuickEntry, drift sheet (link/explain/dismiss), new case and decision
  sheets, Start agent.
- Prime Radiant overlay: heatmap, package series, drift bars, risk donut,
  timeline and The Plan for 30/90/365 days or all time; precomputed in
  the service, one paint per chart, hover read-outs; IPC targets
  `jax.seldon.panel` and `jax.seldon.service`.
- Degraded states with one-click fixes: engine missing, logbook not
  initialised, index missing or stale, contract mismatch, snapper.
- Theme tokens only; headless harnesses and an engine↔plugin e2e on a
  test host.

### Packaging and docs

- PKGBUILD, .SRCINFO, release workflow (tag → musl asset, GitHub
  release, AUR push and plugin subtree split when the secrets exist,
  `workflow_dispatch` dry run), packaging README.
- Specs (engine, plugin, logbook, contract), 22 ADRs, plugin README with
  security section, keybinding docs, preview image.

[Unreleased]: https://github.com/JohnAndrewsX/jax-seldon/compare/v0.1.3...HEAD
[0.1.3]: https://github.com/JohnAndrewsX/jax-seldon/releases/tag/v0.1.3
[0.1.2]: https://github.com/JohnAndrewsX/jax-seldon/releases/tag/v0.1.2
[0.1.1]: https://github.com/JohnAndrewsX/jax-seldon/releases/tag/v0.1.1
[0.1.0]: https://github.com/JohnAndrewsX/jax-seldon/releases/tag/v0.1.0
