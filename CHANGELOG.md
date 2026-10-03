# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Engine

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
  snapper fix grants besides listing (WP-060).
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

### Plugin

- The "Snapshots not readable" banner has a third action, *Check again*
  (WP-054), which runs a capture (the same call as *Capture now*), so the
  banner clears right after the snapper fix instead of at the next
  automatic capture; after *Run in terminal* it says "When the command
  has finished, press Check again" (fixes #2).
- The "Snapshots not readable" banner names what the snapper fix grants:
  its `ALLOW_USERS` entry also lets your user create, change and delete
  root snapshots without a password (WP-060).
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

### Packaging and docs

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

[Unreleased]: https://github.com/JohnAndrewsX/jax-seldon/compare/v0.1.1...HEAD
[0.1.1]: https://github.com/JohnAndrewsX/jax-seldon/releases/tag/v0.1.1
[0.1.0]: https://github.com/JohnAndrewsX/jax-seldon/releases/tag/v0.1.0
