# fixtures/

Test data for both tracks. Validate with `bash scripts/validate-fixtures.sh`
(every JSON fixture against its schema, plus: the sample index derives from
the sample logbook). Owner: Schema Keeper (WP-002, WP-014, WP-015).

| Path | What | Schema |
|---|---|---|
| `index.sample.json` | canonical index; the plugin develops against it | `schema/index.schema.json` |
| `index-variants/*.json` | states the sample does not show: `snapper-degraded` (ADR-0011), `not-initialised`, `index-stale`, `plugins-degraded`, `omarchy-git-checkout`, `drift-explained-case` (ADR-0021), `drift-capped` (ADR-0020), `drift-members-capped`; generated from the sample by an overlay (see below), never hand-edited | `schema/index.schema.json` |
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
| 09-01 | `seldon init`, C-2026-001 opened, verified (`seldon doctor` green) and closed | case lifecycle (`queued → active → verification → completed`, WP-015), note from `seldon log` |
| 09-03 | `pacman -S btop` without a case → drift → *explained* | red-zone drift, resolution |
| 09-05 | plugin `io.github.example.weather-plus` added → *explained* | plugin-add |
| 09-12/13 | C-2026-002 monitors: pre/post snapshots 108/109; Claude edits `monitors.conf` with the Edit tool (no Bash hook) → drift with proposal → *linked*; a note and its correction | `linked`, `correction`, `pairOf` |
| 09-15 | `omarchy update` 4.0.5 → 4.0.6 without a case → two drift items → *explained* | snapshot by `omarchy update`, release marker |
| 09-20/21 | theme `kanagawa` tried → *dismissed* | `dismissed` |
| 09-24 | plugin update → *explained* | plugin-update |
| 09-26…30 | cases 003–006 created; snapshot 111; snapshots 108/109 deleted | snapshot-delete |
| 09-30 | human runs a plain `pacman -Syu` without a case (firefox, libinput, noto-fonts upgraded) → stays open (WP-014) | **one yellow drift group** (`members: 3`, `txId`), ADR-0013 |
| 10-01 | C-2026-003: Claude runs `omarchy update` (keyring reinstall, -Syu, snapshot 112). C-2026-004: Claude installs zed via yay, writes `~/.config/zed/settings.json` via `tee` (no collector watches it: **green**, WP-015) and edits `bindings.conf` via `sed -i`. C-2026-008: human installs tailscale → proposal → *linked* → verification. Codex installs ollama + a user unit without a case (**two crises**). Snapshot 113. Theme `tokyo-night` (open drift, proposed for queued C-2026-005). Plugin `tyme` added → *explained*. For C-2026-008 (still in verification) the human turns on Tailscale MagicDNS inside `snapper create --command`: **pre/post pair 114/115** (WP-015). | everything the plugin renders |

Result: 71 ledger lines (9 resolutions), 62 index events (7 with
`resolutionDetail`; 1 with `zone: green`), 6 snapshots in `system.snapshots`
(1 pre/post pair), 4 open drift items — 3 single (2 crises) and 1 yellow group
of 3 —, 8 cases (3 queued, 2 active, 1 verification, 2 completed), 4 decisions
(1 proposed).

## How the index derives from the logbook

Normative rules: ADR-0012 and ADR-0013. `scripts/validate-fixtures.py`
implements them for the fixture check; `--write-index` regenerates the
logbook-derived parts of `index.sample.json` and every `index-variants/` file
after a logbook edit (the WP-007 golden test replaces it with real engine
output).

Rules the fixture check implements beyond the plain field copies:

- **Resolutions** are folded onto their target as `resolution`,
  `resolutionDetail` (the resolution's `detail`, when it has one; index only,
  never in a ledger line) and `case` when linked; the latest resolution wins
  (ADR-0012 §8, §11).
- **Proposals** (ADR-0012 §7; token rule ADR-0015 §4, which supersedes
  ADR-0012 §13): the event's subject must occur in an open case's `## Plan`
  section as a whole word, case-sensitive, where word characters are
  `[A-Za-z0-9._+-]`, except that a final `.` not followed by a word character
  is punctuation; the lowest case id wins. So `Install zed.` proposes `zed`,
  `zed.conf` does not, and `extra/zed` does (`/` is not a word character —
  which also makes a path like `~/.config/zed/settings.json` name `zed`).
- **Drift grouping** (ADR-0013 §1–§3): open, caseless, drift-eligible `pacman`
  events that share a `txId` form one item. Its `eventId`, `ts`, `subject`,
  `detail`, `actor` and `proposedCase` are the leader's (lowest-id explicit
  member, else lowest-id member); `txId` and `members` are present only when the
  item has two or more members. Other sources are never grouped.
- **Zone of a pacman item** is computed: yellow iff every member is *routine* —
  kind `upgrade` or `reinstall`, `explicit: false`, `meta.command` split on
  whitespace (as pacman logs it, unquoted) is `pacman` with the sync operation
  (`-S`/`--sync`), `-u`/`--sysupgrade` and no package word, and the subject
  matches none of the `alwaysRed` globs (default `linux*`, `systemd`, `glibc`,
  `hyprland`, `omarchy`, `quickshell`); else red. `crisis` iff red. Options
  that take an argument (`--overwrite`, `--config`, `-r`, `--ask`, …) consume
  it; an unknown option is assumed to take none, so its argument counts as a
  package and the item turns red. Other sources copy the event's zone.
- **`series.drift`** counts items, not lines (ADR-0013 §4): a caseless pacman
  transaction opens one item (week of its earliest line); the resolution lines
  of one group write (same `meta.txId`, `ts`, `actor`) count as one.
- **Self-checks.** Every run also derives the index from 18 in-memory mutations
  of the 09-30 group (a member explicit, `linux`/`linux-firmware`/`quickshell`
  as subject, an `install` member, commands naming a package or lacking `-u`,
  `--overwrite`/`-r` arguments, an unknown option, `yay`, a `--only`
  resolution, a fan-out resolution) and fails if the zone, crisis or member
  count is not what ADR-0013 says. Three more add an open caseless `zed`
  upgrade, replace every open case's Plan, and check the token rule end to
  end: `Install zed.` and `` `extra/zed` `` propose, `Edit zed.conf` does not.
- **Case lifecycle** (SPEC-LOGBOOK §3, the engine's `Transition::target`):
  every case's Log lines are walked through `created` → queued, `started`
  (queued → active), `verification` (active → verification), `completed`
  (verification → completed), `dropped: …` (any open status → dropped); other
  Log lines are free text. The walk must end in the frontmatter `status`, its
  steps must be the case's `case-*` ledger events (kind, minute, actor, in
  order), `created`/`started`/`closed` are the dates of their steps, and
  `started (snapshot N)` is `snapshotBefore`. A 22nd self-check removes
  C-2026-001's verification step (the fixture before WP-015) and requires the
  walk to reject `completed` from active.
- **Index times.** `generatedAt` is not before any event in `events`, and
  `state.lastCapture` is not before any collector event (the engine stamps
  both when it writes). This holds for the sample and every variant.

**Index variants** are overlays: `VARIANTS` in `scripts/validate-fixtures.py`
lists, per variant, RFC 6902 operations (`test`, `add`, `replace`, `remove`)
applied to `index.sample.json`. The check fails when a variant file differs
from sample + overlay, and when a file in `index-variants/` has no overlay. To
add a banner state, add an overlay and run `--write-index`.

| Variant | Overlay | Plugin state it drives |
|---|---|---|
| `snapper-degraded` | collector `snapper`: `ok: false` + the ADR-0011 message | degraded collector with a fix command |
| `not-initialised` | `state.status: notInitialised`, every section empty | "Run `seldon init`" |
| `index-stale` | `state.status: indexStale` only; `generatedAt` and `lastCapture` stay the sample's. The engine never writes `indexStale`; `plugin/Model.js` derives it, and this variant exercises its data-driven branch | stale banner from the data; with **`SELDON_NOW=2026-10-01T20:05:12+02:00`** (`STALE_NOW` in the script, also the plugin harness's clock) stale by the clock too — the check requires both times more than 2 h before it |
| `plugins-degraded` | collector `plugins`: `ok: false`, `message` `omarchy plugin list --json: timed out` (the engine's text for a shell IPC timeout) | a failing non-snapper collector |
| `omarchy-git-checkout` | `system.omarchy.repoHead: 3f9c2e1` (short hash, like `logbook.git.head`) | Omarchy run from a git checkout of `$OMARCHY_PATH` (SPEC-ENGINE §4) |
| `drift-explained-case` | btop's event (`01M1MB2M…`, `resolution: explained`) gets `case: C-2026-002`; jq: `.events \|= map(if .id == "01M1MB2M1GWZYF485HTGVZ1KS3" then .case = "C-2026-002" else . end)`. Index only: the logbook's explained lines stay caseless and C-2026-002's `events:` does not list btop | ADR-0021: the row reads `explained · C-2026-002: Kleines Monitoring-Tool, bewusst ohne Case.` and names the case |
| `drift-capped` | `summary.openDrift: 250`, `drift` unchanged (4 items); jq: `.summary.openDrift = 250` | ADR-0020: "+246 more open drift items not listed here" under the drift rows; pill `⟡ 2 · 250` |
| `drift-members-capped` | noto-fonts (`01M3SXBRV0E7…`) removed from `events`; the firefox group keeps `members: 3`; jq: `.events \|= map(select(.id != "01M3SXBRV0E702XKBM22HEV1B8"))` | CONTRACT.md rule 4: the drift sheet lists firefox and libinput plus "… and 1 more", then asks `seldon drift show <firefox> --json` for all three (the fallback) |

Not derivable from the logbook and therefore not checked beyond the index
times above: `generatedAt`, `engineVersion`, `logbook.path`, `logbook.git`,
`state` (engine state under `~/.local/state/seldon`). The golden test must
inject or normalise them; "today" is the date of `generatedAt`. The sample was
written at 2026-10-01 17:05:12 (`lastCapture` 17:05:00, the time of
`logs/snapper.json`), after the story's last event (17:00). A reader on that
day before 17:05 sees a future timestamp; the plugin counts it as fresh. Pin
the clock with `SELDON_NOW` (with `SELDON_INDEX`) for anything time-relative.

Dossier fences the index reads (`system/*.md`, format `- key: value` or a
Markdown table):

| Fence | Feeds |
|---|---|
| `omarchy.summary` (`version`, `theme`, `lastUpdate`) | `system.omarchy` |
| `packages.summary` (`explicit`, `total`, `aur`) | `system.packages` |
| `packages.history` (table `date, explicit, total`) | `series.packages` |
| `plugins.list` (table `id, enabled, firstParty, clonedFrom`) | `system.plugins` (rows, rows with `enabled = yes`) |
| `deviations.table` (table `path, reason, date, case`) | `system.deviations` (row count) |

## Engine assumptions baked into the fixtures (confirm in WP-004/005/006/008)

- **Event ids** are ULIDs whose time part equals `ts` and whose random part is
  a hash. Real ids carry capture time; nothing may rely on id time = `ts`.
- **txId** is `tx-<local time of "transaction started">` (opaque to the plugin).
- **Attribution** (ADR-0014 §1). A hook `agent command` event (with the active case) makes the
  collector events it caused carry the hook's actor and case: `omarchy update` →
  keyring reinstall, -Syu upgrades and the omarchy `update`; `yay -S zed` →
  zed + alsa-lib; `sed -i … bindings.conf` → the config change; `sudo pacman -S
  ollama` / `tee …ollama.service` → `agent:codex` without a case. Collector
  events without such a hook stay `actor: system` (tailscale, btop, 09-15 update,
  the 09-30 `-Syu`).
- **Zone of an event** (ADR-0014 §2): pacman, omarchy, and config under `~/.config/systemd`
  → red; other config, theme, plugins → yellow; snapper, notes, case events → none.
  Ledger events keep this zone; only the drift *item* of a pacman transaction
  gets the computed zone (ADR-0013 §3), so the 09-30 members are `red` in the
  ledger and in `index.events`, and their group is yellow in `index.drift`.
  **Green** has no row in ADR-0014 §2. The fixture assumes (WP-015, open
  decision) that a hook `command` whose target no collector watches is green:
  Claude's `tee ~/.config/zed/settings.json` on 10-01 (`~/.config/zed` is not in
  the default `watchPaths`).
- `seldon log` writes a journal entry **and** a `manual note` event (subject = case
  id or `journal`). Journal entries typed in an editor have no event.
- A full upgrade without a case yields **one drift item per transaction**
  (ADR-0013): the 09-30 `pacman -Syu` is one yellow group of three. The
  09-15 `omarchy update` predates that rule in the story but derives the same
  way (one member, `omarchy` is in `alwaysRed`, so red). On Omarchy a direct
  `pacman -Syu` only gets past `00-omarchy-update-guard.hook` with
  `OMARCHY_ALLOW_DIRECT_PACMAN=1`; the environment is not logged, so the log
  shows the plain command.
- **Group resolution** (ADR-0013 §4, not in the fixture): `seldon drift
  explain <member>` appends one `resolution` line per open member in one write,
  each with `meta.txId`; the check requires `meta.txId` to equal the target's
  `txId`.

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
    pacman events of `logbook/ledger/*.jsonl` (12 lines; ids and attribution aside) and
    nothing for the malformed lines. Complete lines end at byte 11159.
  - The 09-30 `pacman -Syu` block (WP-014) is a plain full upgrade: three
    `upgraded` lines, `explicit: false`, `meta.command` `pacman -Syu`. It is the
    input of the ADR-0013 routine class (one yellow group).
- `pacman-rotation/` — `pacman.log.1` (old inode, ends after the 09-15
  transaction, 7359 bytes; cursor at its end) and the new `pacman.log`, which
  starts by **repeating the 09-15 transaction** (copytruncate race) and continues
  with the 09-30 `-Syu` to 10-01. Expected: restart from 0 on the inode change, dedupe by
  `(ts, kind, subject, version)`, so the 09-15 upgrade is not emitted twice.
- `snapper-before.json` (2026-09-30 18:00, has pre/post 108/109) and `snapper.json`
  (2026-10-01 17:05, has pre/post 114/115; the collector links them through
  `type` and `pre-number` and ignores `userdata`). Diff: +111, −108,
  −109, +112, +113, +114, +115 — the snapper events of the ledger. `date` is
  local time without offset; snapshot 0 is `current`.
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
