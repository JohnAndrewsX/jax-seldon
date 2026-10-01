# fixtures/

Test data for both tracks. Validate with `bash scripts/validate-fixtures.sh`
(every JSON fixture against its schema, plus: the sample index derives from
the sample logbook). Owner: Schema Keeper (WP-002).

| Path | What | Schema |
|---|---|---|
| `index.sample.json` | canonical index; the plugin develops against it | `schema/index.schema.json` |
| `index-variants/*.json` | banner states: `snapper-degraded` (ADR-0011), `not-initialised` | `schema/index.schema.json` |
| `invalid/<schema>.*.json` | must **fail** their schema (validator self-test; `index.contract-v2` doubles as the plugin's `contractMismatch` case) | `schema/<schema>.schema.json` |
| `logbook/` | a complete small logbook (SPEC-LOGBOOK), the source of `index.sample.json` | ledger lines: `event.schema.json`; case frontmatter: `case.schema.json` |
| `logs/` | raw collector inputs (pacman, snapper, `omarchy plugin list/catalog`) | `schema/external/*.schema.json` |
| `hooks/` | Claude Code hook payloads | `schema/external/claude-code-hook.schema.json` |

Fixture prose is German (user content); keys, enums, paths and file names are English.
No real hostnames, users or tokens: the user is `user`, the machine `workstation-7f3a`,
every secret is a documented fake (`AKIAIOSFODNN7EXAMPLE`, `ghp_EXAMPLE…`, `sk-EXAMPLE…`).

## The story (2026-09-01 → 2026-10-01, machine `workstation-7f3a`)

| Date | What happens | Shows |
|---|---|---|
| 09-01 | `seldon init`, C-2026-001 opened and closed | case lifecycle, note from `seldon log` |
| 09-03 | `pacman -S btop` without a case → drift → *explained* | red-zone drift, resolution |
| 09-05 | plugin `io.github.example.weather-plus` added → *explained* | plugin-add |
| 09-12/13 | C-2026-002 monitors: pre/post snapshots 108/109; Claude edits `monitors.conf` with the Edit tool (no Bash hook) → drift with proposal → *linked*; a note and its correction | `linked`, `correction`, `pairOf` |
| 09-15 | `omarchy update` 4.0.5 → 4.0.6 without a case → two drift items → *explained* | snapshot by `omarchy update`, release marker |
| 09-20/21 | theme `kanagawa` tried → *dismissed* | `dismissed` |
| 09-24 | plugin update → *explained* | plugin-update |
| 09-26…30 | cases 003–006 created; snapshot 111; snapshots 108/109 deleted | snapshot-delete |
| 10-01 | C-2026-003: Claude runs `omarchy update` (keyring reinstall, -Syu, snapshot 112). C-2026-004: Claude installs zed via yay and edits `bindings.conf` via `sed -i`. C-2026-008: human installs tailscale → proposal → *linked* → verification. Codex installs ollama + a user unit without a case (**two crises**). Snapshot 113. Theme `tokyo-night` (open drift, proposed for queued C-2026-005). Plugin `tyme` added → *explained*. | everything the plugin renders |

Result: 64 ledger lines (9 resolutions), 55 index events, 3 open drift (2 crises),
8 cases (3 queued, 2 active, 1 verification, 2 completed), 4 decisions (1 proposed).

## How the index derives from the logbook

Normative rules: ADR-0012. `scripts/validate-fixtures.py` implements them for
the fixture check; `--write-index` regenerates the logbook-derived parts of
`index.sample.json` after a logbook edit (the WP-007 golden test replaces it
with real engine output).

Not derivable from the logbook and therefore not checked: `generatedAt`,
`engineVersion`, `logbook.path`, `logbook.git`, `state` (engine state under
`~/.local/state/seldon`). The golden test must inject or normalise them;
"today" is the date of `generatedAt`.

Dossier fences the index reads (`system/*.md`, format `- key: value` or a
Markdown table):

| Fence | Feeds |
|---|---|
| `omarchy.summary` (`version`, `theme`, `lastUpdate`) | `system.omarchy` |
| `packages.summary` (`explicit`, `total`, `aur`) | `system.packages` |
| `packages.history` (table `date, explicit, total`) | `series.packages` |
| `plugins.list` (table `id, enabled, firstParty, clonedFrom`) | `system.plugins` (rows, rows with `enabled = yes`) |
| `deviations.table` (table `path, reason, date, case`) | `system.deviations` (row count) |

## Engine assumptions baked into the fixtures (confirm in WP-004/005/006)

- **Event ids** are ULIDs whose time part equals `ts` and whose random part is
  a hash. Real ids carry capture time; nothing may rely on id time = `ts`.
- **txId** is `tx-<local time of "transaction started">` (opaque to the plugin).
- **Attribution.** A hook `agent command` event (with the active case) makes the
  collector events it caused carry the hook's actor and case: `omarchy update` →
  keyring reinstall, -Syu upgrades and the omarchy `update`; `yay -S zed` →
  zed + alsa-lib; `sed -i … bindings.conf` → the config change; `sudo pacman -S
  ollama` / `tee …ollama.service` → `agent:codex` without a case. Collector
  events without such a hook stay `actor: system` (tailscale, btop, 09-15 update).
- **Zone of an event**: pacman, omarchy, and config under `~/.config/systemd`
  → red; other config, theme, plugins → yellow; snapper, notes, case events → none.
- `seldon log` writes a journal entry **and** a `manual note` event (subject = case
  id or `journal`). Journal entries typed in an editor have no event.
- A full `omarchy update` without a case yields one drift item per upgraded
  package (here only one package to stay small) — see the handover.

## logs/

- `pacman.log` — install-time `archinstall` lines (`pacman -b /mnt//var/lib/pacman/
  -r /mnt …`, `+0000`), then local time (`+0200`); transaction blocks with hooks
  and scriptlets (one with ANSI colour codes); `remove` with dependency removal,
  `--ask 4` (argument!), `-U` downgrade with epoch, reinstall, yay's
  `-S … -- extra/zed` + `-D --asexplicit` (no transaction), malformed lines
  (empty, no timestamp, impossible date, truncated, missing parens, unknown tag,
  an `upgraded` line without version inside a transaction) and an
  **unterminated last line** (an interrupted write; the cursor must stop before it).
  - Baseline cursor of `seldon init`: byte offset **6129** (first line after it is
    the 09-03 btop transaction). From there the parser must produce exactly the
    pacman events of `logbook/ledger/*.jsonl` (ids and attribution aside) and
    nothing for the malformed lines. Complete lines end at byte 10408.
- `pacman-rotation/` — `pacman.log.1` (old inode, ends after the 09-15
  transaction, 7359 bytes; cursor at its end) and the new `pacman.log`, which
  starts by **repeating the 09-15 transaction** (copytruncate race) and continues
  to 10-01. Expected: restart from 0 on the inode change, dedupe by
  `(ts, kind, subject, version)`, so the 09-15 upgrade is not emitted twice.
- `snapper-before.json` (2026-09-30 18:00, has pre/post 108/109) and `snapper.json`
  (2026-10-01 17:05). Diff: +111, −108, −109, +112, +113 — the snapper events of
  the ledger. `date` is local time without offset; snapshot 0 is `current`.
  Shape from snapper upstream; **not verified on the dev host** (ADR-0011).
- `snapper-no-permissions.stderr` — what snapper prints unprivileged: exit code 1,
  this on stderr, nothing on stdout, also with `--jsonout`.
- `plugin-list-before.json` / `plugin-list-after.json` — real field set and
  formatting (one line) of `omarchy plugin list --json` on Omarchy 4.0.4 plus two
  third-party plugins and a clone (`user.clock`, `clonedFrom: omarchy.clock`).
  Diff: one `plugin-add io.github.example.tyme` (disabled). No `version` field.
- `plugin-catalog.json` — `omarchy plugin catalog` (real first-party entries plus
  the three non-first-party ones). It has **no `version` either**; read the
  manifest at `manifestPath` for `plugin-update` detection.

## hooks/

All three are `PostToolUse` payloads for the Bash tool.

| File | Expected from `seldon hook claude-code` |
|---|---|
| `claude-code-mutating.json` | one `agent command` event, subject `yay`, `meta.command` = `yay -S --noconfirm zed`, actor `agent:claude-code`, case from `.seldon/active-case` |
| `claude-code-non-mutating.json` | no event (`pacman -Qi`, `git status` are queries) |
| `claude-code-secret.json` | one `agent command` event, subject `git` (git inside `~/.config`); `meta.command` contains `‹redacted›` and none of `AKIAIOSFODNN7EXAMPLE`, `ghp_EXAMPLE…`, `user:`, `hunter2`; nothing from `tool_response` (it holds an `sk-` token) is recorded |

In every case: no stdout, exit 0.
