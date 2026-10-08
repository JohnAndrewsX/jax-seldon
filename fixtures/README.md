# fixtures/

Test data for both tracks. Validate with `bash scripts/validate-fixtures.sh`
(every JSON fixture against its schema, plus: the sample index derives from
the sample logbook). Owner: Schema Keeper (WP-002, WP-014, WP-015).

| Path | What | Schema |
|---|---|---|
| `index.sample.json` | canonical index; the plugin develops against it | `schema/index.schema.json` |
| `index.attention-all.json` | the same logbook indexed with `[drift] attention = "all"` (ADR-0028 §5: the rollback, the drift rules before ADR-0028); derived by the script's legacy path, held to `seldon index` by the engine's golden test | `schema/index.schema.json` |
| `index-variants/*.json` | states the sample does not show: `snapper-degraded` (ADR-0026), `not-initialised`, `index-stale`, `plugins-degraded`, `omarchy-git-checkout`, `drift-explained-case` (ADR-0021), `drift-capped` (ADR-0020), `drift-members-capped`, `case-reopened` (ADR-0027); generated from the sample by an overlay (see below), never hand-edited | `schema/index.schema.json` |
| `invalid/<schema>.*.json` | must **fail** their schema (validator self-test; `index.contract-v3` doubles as the plugin's `contractMismatch` case) | `schema/<schema>.schema.json` |
| `proposals/<id>.json` | the agent's triage proposal the sample's `triage` points at (ADR-0034 §6, ADR-0035 §6): a link (tokyo-night → C-2026-005, evidence a Plan line and a journal entry), an explain (the `monitors.conf` removal) and a crisis item (the ollama unit); `logbook` is the sample's `/home/user/Seldon`. In the engine's state directory in real life; the plugin's dev mode reads it next to the index | `schema/proposal.schema.json` |
| `logbook/` | a complete small logbook (SPEC-LOGBOOK), the source of `index.sample.json` | ledger lines: `event.schema.json`; case frontmatter: `case.schema.json` |
| `logs/` | raw collector inputs (pacman, snapper, `omarchy plugin list/catalog`) | `schema/external/*.schema.json` |
| `logs/omarchy-packages/` | verbatim copies of Omarchy's `install/omarchy-base.packages` and `omarchy-other.packages` (dev host, 2026-10-01); the dossier's `SELDON_OMARCHY_PACKAGES` in tests (WP-036) | — |
| `hooks/` | Claude Code hook payloads | `schema/external/claude-code-hook.schema.json` |
| `vaults/omarchy-agent/` | a small synthetic vault in the omarchy-agent kit's layout (cases in every kit status, two journal months, knowledge topics, a dossier with deviations, inbox, templates, `.obsidian/`); invented German text, input of `seldon import omarchy-agent` (WP-043, `engine/tests/import.rs`). Fakes: `token=abc123geheim`, `https://user:geheim@example.org`, `/home/user` | — |

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
| 09-13 (WP-115) | during C-2026-002's verification the human adds the plugin `io.github.example.display-profiles`, which its Plan names, from Omarchy's menu (`actor: system`, no hook saw it); Claude closes the case without a capture; the next capture links it to the case (SPEC-ENGINE §5 rule 9, ADR-0029) | `linked` by `system` (`planned by C-2026-002; active at the time`), the case's Log line `linked after the fact: …` |
| 09-15 | `omarchy update` 4.0.5 → 4.0.6 without a case → two drift items → *explained* | snapshot by `omarchy update`, release marker |
| 09-20/21 | theme `kanagawa` tried → *dismissed* | `dismissed` |
| 09-22 (WP-113) | the human edits `weather-plus`'s forecast panel in place (no version change) → one `plugin-update` `files changed (sha256 … → …)` with the tree hashes → *explained* | plugin tree hashing (ADR-0028 WP-E): an in-place edit of a third-party plugin, attention until explained |
| 09-24 | plugin update (a pull of three commits) → *explained* | plugin-update with `meta.git` and `meta.commits` (WP-136) |
| 09-26…30 | cases 003–006 created; snapshot 111; snapshots 108/109 deleted | snapshot-delete |
| 09-25 | theme `catppuccin` tried and back to `kanagawa` | two **routine** theme switches (ADR-0028): history, no drift |
| 09-27 | human downgrades `mesa`, `vulkan-radeon`, `lib32-mesa` from the cache (`pacman -U …`) | **one attention group** (`members: 3`, `txId`): a named downgrade |
| 09-28 | `~/.config/hypr/monitors.conf` removed (the move to Lua) | `config-remove`: attention |
| 09-29 | plugin `weather-plus` disabled and enabled again; the shell rewrites `shell.json`; a hook `post-update.d/backup-dotfiles.sh` appears (no case, `system`) | toggles and `shell.json` **routine**; the hook a **crisis** in the yellow zone (ADR-0028 §2) |
| 09-30 | human runs a plain `pacman -Syu` without a case (firefox, libinput, noto-fonts upgraded) → stays without a resolution (WP-014) | a plain full upgrade: **routine** history since ADR-0028 (with `attention = "all"`: one yellow drift group, ADR-0013) |
| 10-01 | C-2026-003: Claude runs `omarchy update` (keyring reinstall, -Syu, snapshot 112). C-2026-004: Claude installs zed via yay, writes `~/.config/zed/settings.json` via `tee` (no collector watches it: **green**, WP-015) and edits `bindings.conf` via `sed -i`. C-2026-008: human installs tailscale → proposal → *linked* → verification. Codex installs ollama + a user unit without a case (the install quiet **attention**, the unit a **crisis**). Snapshot 113. Theme `tokyo-night` (open drift, proposed for queued C-2026-005). Plugin `tyme` added → *explained*. For C-2026-008 (still in verification) the human turns on Tailscale MagicDNS inside `snapper create --command`: **pre/post pair 114/115** (WP-015). | everything the plugin renders |
| 10-01 (WP-120) | The engine speaks contract 2 from the start of the day (ADR-0035): at 08:55 a capture finds `owned.json` unreadable and re-baselines the config collector (`state-loss`); at 09:00 the human raises C-2026-003 to R3 (`case-updated`, `meta.risk: R3`; its Log's `set risk R2 → R3`); every case line of the day carries `meta.risk`, the older ones do not (C-2026-003 was created on 09-26, so the harm guard reads its Log). The proposal of 17:02 (`proposals/`), the autocommit of the 17:00 note (`logbook.git.autocommit`, sample only), ADR-0003 naming C-2026-004 and C-2026-005, and the long note of 09-12 (the one clipped text, `meta.truncated`) complete the v2 surfaces | `case-updated`, `state-loss`, `meta.risk`, `meta.truncated`, `decisions[].cases`, `triage` |
| 10-01 (WP-127) | At 11:00 the human imports one item of `~/Notizen/aufgaben.md` (`seldon import task`): C-2026-007, tag `imported`, frontmatter `source`, its Intent opening with the `Imported from …` line; the Plan was written later by hand. Every case with Intent or Result text and every decision with a Decision section shows its first paragraph; every open drift item names its rule (ADR-0038) | `cases[].intent`/`result`/`source`, `decisions[].lead`, `drift[].rule` |
| 10-01 (WP-101) | C-2026-002 had been closed by Claude (`closed-by-agent`, ADR-0027 §5: its verify and done by `agent:claude-code`); C-2026-003 was raised to R3 before the `omarchy update` (Omarchy itself is R3, ADR-0027 §2c; as R2 the update would raise the R3 advisory). The reopen of C-2026-002 lives in the variant `case-reopened` (index only) | `closed-by-agent` marker |

Result: 87 ledger lines (11 resolutions), 76 index events (9 with
`resolutionDetail`; 1 with `zone: green`; 1 with `meta.truncated`), 6 snapshots in `system.snapshots`
(1 pre/post pair), 6 open drift items — 5 single (2 crises: the user unit
and the hook) and 1 attention group of 3 (the mesa downgrade) —, 6 routine
items (`drift --all`: the `-Syu` group, the two theme switches, the two
toggles, `shell.json`), 8 cases (3 queued, 2 active, 1 verification, 2
completed; C-2026-007 imported), 4 decisions (1 proposed; ADR-0003 names two
cases; each with a lead), 1 triage proposal (3 items, 1 crisis).

## How the index derives from the logbook

Normative rules: ADR-0012, ADR-0013 and ADR-0028 (classes). `scripts/validate-fixtures.py`
implements them for the fixture check; `--write-index` regenerates the
logbook-derived parts of `index.sample.json` and every `index-variants/` file
after a logbook edit (the WP-007 golden test replaces it with real engine
output).

Rules the fixture check implements beyond the plain field copies:

- **Resolutions** are folded onto their target as `resolution`,
  `resolutionDetail` (the resolution's `detail`, when it has one; index only,
  never in a ledger line) and `case` when linked; the latest resolution wins
  (ADR-0012 §8, §11).
- **The desk's details** (ADR-0038): `drift[].rule` is the class's rule (`attention-all` under
  `attention = "all"`); `cases[].intent`/`result` and `decisions[].lead` are the first paragraph
  of `## Intent`/`## Result`/`## Decision` (comments left out; an imported case's provenance line
  gives way to the next paragraph), control characters as spaces, clipped with `in the file`; the
  script does not redact (no fixture text holds a secret; the engine's golden test redacts and
  must agree); `cases[].source` is the frontmatter's `source` while it has its shape.
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
- **Class** (ADR-0028 §2; `Classifier` in the script, `engine/src/index/class.rs`
  in the engine): every linkable event is routine, attention or crisis; a
  group takes the highest class of its members; routine items are not drift
  unless an open case's Plan names them (then attention with `proposedCase`);
  `crisis` iff the class is crisis; `zone` is the leader's ledger zone.
- **Under `attention = "all"`** (`index.attention-all.json`) the zone of a
  pacman item is computed: yellow iff every member is *routine* —
  kind `upgrade` or `reinstall`, `explicit: false`, `meta.command` split on
  whitespace (as pacman logs it, unquoted) is `pacman` with the sync operation
  (`-S`/`--sync`), `-u`/`--sysupgrade` and no package word, and the subject
  matches none of the `alwaysRed` globs (default in SPEC-ENGINE §2: the
  kernels `linux`, `linux-lts`, `linux-zen`, `linux-hardened`, `linux-rt`,
  `linux-rt-lts`, `linux-omarchy`; `systemd`, `glibc`, `hyprland`, `omarchy`,
  `omarchy-settings`, `quickshell`, `limine*`, `grub`, `mkinitcpio*`,
  `filesystem`, `pam`, `sddm`, `uwsm`); else red. `crisis` iff red. Options
  that take an argument (`--overwrite`, `--config`, `-r`, `--ask`, …) consume
  it; an unknown option is assumed to take none, so its argument counts as a
  package and the item turns red. Other sources copy the event's zone.
- **`series.drift`** counts items, not lines (ADR-0013 §4): a caseless pacman
  transaction opens one item (week of its earliest line); the resolution lines
  of one group write (same `meta.txId`, `ts`, `actor`) count as one. ADR-0028
  §5: a routine group opens nothing, and a resolution counts only when its
  target opened an item.
- **Self-checks.** Every run also derives the index from 20 in-memory mutations
  of the 09-30 group under `attention = "all"` (a member explicit,
  `linux`/`linux-firmware`/`quickshell` as subject, an `install` member,
  commands naming a package or lacking `-u`, `--overwrite`/`-r` arguments, an
  unknown option, `yay`, a `--only` resolution, a fan-out resolution) and
  fails if the zone, crisis or member count is not what ADR-0013 says, and
  from 18 more under the default rules (`-Syyuu`, `-Su`, bare `yay`, Omarchy's
  update line, a kernel in the upgrade, `:: Replace`, downgrades, a named
  upgrade, a named kernel install, the keyring, targets from stdin, `-U`
  outside a cache, a keyring removal) against ADR-0028 §2. Three
  more add an open caseless `zed` install, replace every open case's Plan,
  and check the token rule end to end: `Install zed.` and `` `extra/zed` ``
  propose, `Edit zed.conf` does not.
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
  Contract 2 (ADR-0035 §1; `CONTRACT_2_FROM`, the start of 10-01 in the
  story): from then on every `set …` Log line is a `case-updated` line with
  its words as `detail`, and every `case-created|started|updated` line
  carries the risk the Log has at that step (`meta.risk`); earlier lines
  carry none. Three self-checks: without the `case-updated` line, with a
  wrong `meta.risk` on a start, and the whole logbook without any
  `meta.risk` (a contract-1 ledger), which must derive the same cases and
  drift.
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
| `snapper-degraded` | collector `snapper`: `ok: false` + the `NO_PERMISSIONS` message (ADR-0026) | degraded collector with a fix command |
| `not-initialised` | `state.status: notInitialised`, every section empty | "Run `seldon init`" |
| `index-stale` | `state.status: indexStale` only; `generatedAt` and `lastCapture` stay the sample's. The engine never writes `indexStale`; `plugin/Model.js` derives it, and this variant exercises its data-driven branch | stale banner from the data; with **`SELDON_NOW=2026-10-01T20:05:12+02:00`** (`STALE_NOW` in the script, also the plugin harness's clock) stale by the clock too — the check requires both times more than 2 h before it |
| `plugins-degraded` | collector `plugins`: `ok: false`, `message` `omarchy plugin list --json: timed out` (the engine's text for a shell IPC timeout) | a failing non-snapper collector |
| `omarchy-git-checkout` | `system.omarchy.repoHead: 3f9c2e1` (short hash, like `logbook.git.head`) | Omarchy run from a git checkout of `$OMARCHY_PATH` (SPEC-ENGINE §4) |
| `drift-explained-case` | btop's event (`01M1MB2M…`, `resolution: explained`) gets `case: C-2026-002`; jq: `.events \|= map(if .id == "01M1MB2M1GWZYF485HTGVZ1KS3" then .case = "C-2026-002" else . end)`. Index only: the logbook's explained lines stay caseless and C-2026-002's `events:` does not list btop | ADR-0021: the row reads `explained · C-2026-002: Kleines Monitoring-Tool, bewusst ohne Case.` and names the case |
| `drift-capped` | `summary.openDrift: 250`, `drift` unchanged (6 items); jq: `.summary.openDrift = 250` | ADR-0020: "+244 more changes without a case not listed here" under the drift rows; pill `2 · 250` |
| `drift-members-capped` | lib32-mesa (`01M3H6M8184N…`) removed from `events`; the mesa group keeps `members: 3`; jq: `.events \|= map(select(.id != "01M3H6M8184NVTFDTEGPD71P5H"))` | CONTRACT.md rule 4: the drift sheet lists mesa and vulkan-radeon plus "… and 1 more", then asks `seldon drift show <mesa> --json` for all three (the fallback) |
| `case-reopened` | a third active case C-2026-009 `Reopen: Hyprland-Monitorlayout für Dual-WQHD` with `tags: [reopens:C-2026-002]` (what `seldon plan reopen C-2026-002` makes), `summary.activeCases: 3`. Index only: in the logbook it would move every list the plugin harness walks | ADR-0027 §5: the reopen of an agent-closed case; WIP at the limit |

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
    pacman events of `logbook/ledger/*.jsonl` (15 lines; ids and attribution aside) and
    nothing for the malformed lines. Complete lines end at byte 11832.
  - The 09-30 `pacman -Syu` block (WP-014) is a plain full upgrade: three
    `upgraded` lines, `explicit: false`, `meta.command` `pacman -Syu`: routine
    `sysupgrade` (ADR-0028; under `attention = "all"` the ADR-0013 yellow
    group). The 09-27 `pacman -U` block downgrades three packages from the
    cache, each named (`explicit: true`): one attention group.
- `pacman-rotation/` — `pacman.log.1` (old inode, ends after the 09-15
  transaction, 7359 bytes; cursor at its end) and the new `pacman.log`, which
  starts by **repeating the 09-15 transaction** (copytruncate race) and continues
  with the 09-27 downgrade and the 09-30 `-Syu` to 10-01. Expected: restart from 0 on the inode change, dedupe by
  `(ts, kind, subject, version)`, so the 09-15 upgrade is not emitted twice.
- `snapper-before.json` (2026-09-30 18:00, has pre/post 108/109) and `snapper.json`
  (2026-10-01 17:05, has pre/post 114/115; the collector links them through
  `type` and `pre-number` and ignores `userdata`). Diff: +111, −108,
  −109, +112, +113, +114, +115 — the snapper events of the ledger. `date` is
  local time without offset; snapshot 0 is `current`.
  Shape from snapper upstream, checked against snapper 0.13.1 on the dev host (WP-082).
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

Claude Code hook payloads for `seldon hook claude-code` (stdin). The Bash
payloads come in pairs: the `PostToolUse` original (with `tool_response`) and
its `-pre` variant (`PreToolUse`, the same `tool_use_id`, no `tool_response`).
The engine records on `PreToolUse` (ADR-0017 §1); a `PostToolUse` whose
`tool_use_id` is already in the ledger writes nothing, a `PostToolUse` alone is
recorded. `validate-fixtures` fails when a `-pre` payload is not its sibling
with `hook_event_name: PreToolUse` and without `tool_response`. `Edit`/`Write` are `PreToolUse` only and carry an absolute
`file_path` under the fixture user's home `/home/user`.

Every recorded event: `source: agent`, `kind: command`, `actor:
agent:claude-code`, `ts` = the hook's clock, `meta.toolUseId` = the payload's
`tool_use_id`, `meta.sessionId` = its `session_id`, `case` = the active case
(`.seldon/active-case`) when one is set, else none.

| File | Hook | Without a case | With an active case |
|---|---|---|---|
| `claude-code-mutating.json` | Post, Bash | one event, subject `yay`, zone `red`, `meta.command` = `yay -S --noconfirm zed` | the same with `case` |
| `claude-code-mutating-pre.json` | Pre, Bash | the same; after it the Post payload adds nothing | the same with `case` |
| `claude-code-non-mutating.json` | Post, Bash | nothing (`pacman -Qi`, `git status` are queries) | nothing |
| `claude-code-non-mutating-pre.json` | Pre, Bash | nothing | nothing |
| `claude-code-secret.json` | Post, Bash | one event, subject `git`, zone `yellow` (git inside `~/.config/hypr`, watched); `meta.command` = `AWS_ACCESS_KEY_ID=‹redacted› git -C ~/.config/hypr push https://‹redacted›@github.com/example/dotfiles.git main --password ‹redacted›`, none of `AKIAIOSFODNN7EXAMPLE`, `ghp_EXAMPLE…`, `user:`, `hunter2`; nothing from `tool_response` (it holds an `sk-` token) | the same with `case` |
| `claude-code-secret-pre.json` | Pre, Bash | the same; after it the Post payload adds nothing | the same with `case` |
| `claude-code-edit-watched.json` | Pre, Edit `~/.config/hypr/monitors.conf` (watched) | one event, subject `edit`, zone `yellow` (config under `watchPaths`, ADR-0014 §4), `meta.command` = `Edit ~/.config/hypr/monitors.conf`; neither `old_string` nor `new_string` is recorded | the same with `case` |
| `claude-code-write-unwatched.json` | Pre, Write `~/.config/zed/settings.json` (not in `watchPaths`) | nothing (ADR-0019 §1: untracked effects are recorded only with a case) | one event, subject `write`, zone `green` (ADR-0019 §2), `meta.command` = `Write ~/.config/zed/settings.json`; `content` is not recorded |

In every case: no stdout, exit 0.

**Running them.** `~` in the Bash commands and the `/home/user/…` paths of
`Edit`/`Write` resolve against `$HOME`, and the git rule compares with
`$XDG_CONFIG_HOME` (default `$HOME/.config`). To reproduce the table, run with
`HOME=/home/user`, `XDG_CONFIG_HOME` unset, and the config, state and logbook in
a scratch dir: `SELDON_CONFIG=<scratch>/config.toml`,
`XDG_STATE_HOME=<scratch>/state` (WP-016 did this; `/home/user` does not exist
on the dev host, so nothing can be written there). A test with its own temp
`HOME` replaces the `/home/user` prefix in the payload with that home first;
otherwise the `Edit` path is outside `watchPaths` and records nothing without a
case.
