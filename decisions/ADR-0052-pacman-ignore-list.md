# ADR-0052 — pacman's ignore list in the index: the IgnorePkg and IgnoreGroup names, an optional v2 field; a change is attention

**Status:** accepted (operator decision 2026-10-10, E77)
**Date:** 2026-10-10

> Adds one **optional** field to contract 2 under ADR-0035 §6, before
> 0.2.0 is tagged: nothing required is added, nothing is removed or
> changes meaning, `contractVersion` stays 2 and `contractReadableFrom`
> stays 2 (ADR-0051). **Amends ADR-0028 §2 by one row**, as ADR-0042 did
> for two. Operator decision 2026-10-08 (S9: names only, AGENTS.md §6);
> the wording from the prototype 0.3 debate (SYNTHESIS §4 item 4, E48–E63).
> Work package: WP-165.

## Context

1. **A package pacman ignores is invisible.** `IgnorePkg` and
   `IgnoreGroup` in `/etc/pacman.conf` (and the files it includes) make
   pacman's full upgrade (`-Syu`, `omarchy update`) skip those packages.
   Users pin a kernel or a driver after a bad update and forget it; an
   agent asked "why is mesa old?" has no way to know. Seldon records every
   transaction but not the one setting that keeps transactions from
   happening.
2. **Omarchy rewrites the file.** `omarchy refresh pacman` copies the
   channel's template (`$OMARCHY_PATH/default/pacman/pacman-<channel>.conf`,
   which names no `IgnorePkg`) over `/etc/pacman.conf`; Omarchy's own hook
   sample (`pre-refresh-pacman.d/add-custom-repo.sample`) names "extra
   IgnorePkg lines" as a thing users must re-add by hook. A list that
   vanishes silently is exactly a change worth a reason.
3. **The zone.** AGENTS.md §6 (S9) allows the collectors to read the
   `IgnorePkg` and `IgnoreGroup` values of `/etc/pacman.conf` and its
   includes — names only, no other key, no other value.
4. **The window.** A field the 0.2.0 plugin reads must ship in 0.2.0
   (ADR-0051 Context 4); otherwise it waits for contract 3 (WP-184). With
   ADR-0051 that costs users nothing, so this is "should", not "must".

## Decision

### 1. Reading the names

The **pacman collector** reads them on every run that reads `pacman.log`
(a degraded run reads nothing, as today). It reads `/etc/pacman.conf`
(`Sources::etc_dir`) and its includes as pacman does. What
`pacman.conf(5)` documents is followed as written: keys in CamelCase
(exact match), `Include = <path>` expanded by glob(7) rules, `IgnorePkg =
package ...` and `IgnoreGroup = group ...` with the names separated by
**spaces** (a tab or a comma is part of a name, as in pacman's
`setrepeatingoption`). Where the page is silent or narrower, pacman's
parser (`ini.c`, `conf.c`) is followed:

- `#` cuts the rest of a line. The page says comments begin a line;
  pacman's parser also drops end-of-line comments, so `IgnorePkg = linux
  # the kernel` holds `linux` only. Lines are trimmed, empty lines
  skipped; `[]` is refused by pacman and makes the read incomplete.
- `[name]` starts a section. The section is **shared across includes**:
  a header in an included file changes it for the rest of the includer,
  as pacman's parser keeps one section state.
- `Include` is followed wherever it stands: matches sorted by name, a
  leading `.` matched only explicitly, every match read in turn, at most
  **10 levels below `pacman.conf`** (the page names no depth; this is
  the recursion limit as the implementer read pacman's `conf.c`,
  unverified against a running pacman). An include is read wherever it
  lies, as pacman reads it: a path under `/etc` below `Sources::etc_dir`,
  any other absolute path below that directory's parent — `/` on the
  host, the guard in tests, so no test reads the host. A relative path
  (pacman resolves it against its working directory, which Seldon cannot
  know) is not read. An include that lies under a home directory (`/home/*`,
  `/root`), named by `pacman.conf` or reached through a symbolic link in
  `/etc`, is read like any other: root made it part of pacman's
  configuration, pacman reads it, and only the two lists' names leave it
  (AGENTS.md §6, S9). The §6 rule for files under `~/.config` (E41: paths
  and modification times only) governs the recent-config scan, not a
  file pacman includes.
- In the `[options]` section the values of `IgnorePkg` and `IgnoreGroup`
  add up over repeated lines. Names keep pacman's order (the include
  chain's), without repeats.
- **Nothing else is kept.** Every other line, key and value — servers,
  repositories, signature levels, other options — is read past and
  dropped; nothing of it reaches a cursor, an event, the index or a log.

**Bounds** (WP-174): a file is read only when it is a regular file
(`sys::open_regular`) of at most 1 MiB. One read has one budget, counted,
never timed: 64 files (`pacman.conf` included), 64 `Include` lines
followed, 16 384 directory entries looked at by all its globs together;
at most 256 names per list; a name longer than 512 bytes is kept in the
cursor as `sha256:<hex>` of its bytes.

**Partial means incomplete.** Only a read that could not see everything
is *partial*: a file that cannot be read (missing, not regular, too
large, no permission), a relative include, the depth, `[]`, the budget,
names past 256. A partial read keeps what it read and records no change
(§2). A name Seldon will not show is **not** an incomplete read (§5).

If `/etc/pacman.conf` itself cannot be read, the run keeps the last list,
marked partial (none before the first read).

### 2. The cursor

`PacmanCursor` gains three optional members (absent in 0.1.x cursors,
which read unchanged):

- `ignore` `{packages, groups, partial?}`: the list as the last run read
  it, the raw names — what the index shows (§5);
- `ignoreKnown` `{packages, groups}`: the last **complete** list — what a
  change is measured against;
- `ignoreAt`: when `ignoreKnown` was read (RFC 3339, the capture time).

The raw names are engine-private state in the user's state directory,
as the orchestrator accepted (review round 1, Q3).

A run without a cursor, or with a cursor without `ignoreKnown` (the first
run of this engine, a lost state directory), takes the list as it is, **no
event** — a baseline, as every collector takes one. A partial read writes
no event and leaves `ignoreKnown` and `ignoreAt` alone, so a file that
could not be read is never reported as names removed. Every complete read
sets both. Names are compared as sets: another order, across lines and
files, is no change. Because `capture` saves cursors only after the
ledger write, a failed write never loses a change, and a second capture
writes nothing (idempotent).

A change can already be in the ledger although the cursor does not know
it: the cursor save after the write failed, or an older state directory
was restored. Then the newest ignore-list note **at or after `ignoreAt`**
records the new lists exactly as the ledger wrote them (through the
logbook's redaction), and the change is not written again. A note from
before `ignoreAt` never counts: after a state loss the known list is a
new baseline, and a change after it is a change, whatever older notes
say (review round 1, F1). The ledger is read for this only when the list
changed, and only since `ignoreAt`.

### 3. The event

When a complete read differs from `ignoreKnown`, the run writes one
event:

- `source: pacman`, `kind: note`, `subject: /etc/pacman.conf`, `ts` the
  capture time, `actor: system`, no `txId`, no `case`;
- `detail` what changed, e.g. `IgnorePkg: added mesa, vulkan-radeon;
  removed linux. IgnoreGroup: added gnome.`;
- `meta.ignorePkg` and `meta.ignoreGroup`: the **new** lists, names
  separated by one space as pacman.conf writes them (`""` for an empty
  one); both always present on this event, and only on it.
- A name Seldon does not show by its shape (§5) is written as `(hidden)`
  in `detail` and `meta`; it still makes a change. The ledger's
  redaction applies as to every line.

`seldon event` refuses `--meta ignorePkg=…` and `--meta ignoreGroup=…`
(exit 1): the collector writes them. A line with them that is not a pacman
`note` is read like any other line, and only the collector's list (§2)
feeds the index field (§5).

### 4. ADR-0028 §2: one row

| Event | Class | Rule / resolution |
|---|---|---|
| pacman `note` with `meta.ignorePkg` (the ignore list changed, §3) | **attention** | `ignore-list`: reason test — the full upgrade now skips (or no longer skips) these packages; nothing routine changes that setting, and a rebuild from the logbook would not know it |

It precedes ADR-0042's rows (a file pacman left has no `meta.ignorePkg`).
It is no crisis: the harm test fails — a held package keeps working, and
pacman says so on every upgrade (`warning: <pkg>: ignoring package
upgrade`). ADR-0028's three tests, its other rows and §3–§7 stand.

### 5. The index field

`system.pacmanIgnore` (optional) = `{packages: [name], groups: [name],
hidden?: n, partial?: true}`: the pacman cursor's `ignore` (§2), in
pacman's order. A name is shown when it is 1–128 characters of
`[A-Za-z0-9@._+*?!^[]-]` (a package or group name, or an `fnmatch`
pattern of one) and the logbook's redaction leaves it unchanged, on
every index build; the others are counted in `hidden` (present only when
not 0), never shown. `partial` (present only when true) is the read's
(§1). The field
is absent while the pacman collector is disabled, when `cursors.json` is
another logbook's or holds no list, and while `[redaction] patterns` do
not compile (as ADR-0038 §2 withholds its texts). Names are user content
(CONTRACT.md rule 6): shown as plain text, never evaluated, never an
argument of any command.

### 6. The desk

The System section (5) gains a seventh tile, **Ignored by pacman**: the
big value is the number of names in both lists, hidden ones included
("ignored"); the lead,
when there is one, is **"pacman's full upgrade skips them; `pacman -S`
still updates them."**, with none "pacman ignores nothing: no IgnorePkg or
IgnoreGroup in pacman.conf."; a partial list adds "Part of pacman's
configuration could not be read; the list may be incomplete." The rows
are IgnorePkg and IgnoreGroup with their names, and with hidden names
"Not shown · N names (not a plain package or group name, or masked by
your redaction)". No stripe: the list is
a fact, the change is the drift.

The event's detail (Changelog, Today) gets the row **Hint**: "pacman's
full upgrade skips the packages in IgnorePkg and IgnoreGroup; `pacman -S`
still updates them. The list is in System." — text only (AGENTS.md §8).

### 7. Contract

`contractVersion` stays **2**, `contractReadableFrom` stays **2**.
`index.schema.json` gains the optional `system.pacmanIgnore`;
`event.schema.json` names `ignorePkg` and `ignoreGroup` among the
conventional `meta` keys (descriptions only: they are scalar strings, as
every `meta` value); `ignore-list` joins the open set of `drift[].rule`.
CONTRACT.md rule 9 lists the field. An index of an earlier contract-2
build lacks it and is valid: the desk shows "—" on the tile. No released
plugin reads contract 2 yet, so no reader can misread it.

## Consequences

- Every capture reads `/etc/pacman.conf` and its includes (on Omarchy the
  mirrorlist, a few KiB): well under a millisecond.
- The first capture of this engine shows the list without an event; from
  then on, an edit of IgnorePkg — or `omarchy refresh pacman` wiping it —
  is one attention item until explained.
- `fixtures/`: `fixtures/state/pacman-cursor.json` is the pacman cursor
  of the sample's 17:05 capture (`IgnorePkg = zoom slack-desktop`,
  pinned before the logbook began, so no ledger line); the golden test
  and the reference derive read it for `system.pacmanIgnore`. The change
  is the index variant `pacman-ignore-changed` (after the 09-27 mesa
  downgrade the human pins the three packages: one open attention item
  `ignore-list`), index only like `boot-config`: in the sample's ledger
  it would move every list the plugin harness walks.
- SPEC-ENGINE §2 (the cursor), §4 (the pacman collector), §5 (the row),
  §6 (the field); `event.schema.json` (the meta keys); SPEC-PLUGIN (the
  tile, the hint); CONTRACT.md rule 9; CHANGELOG.

## Alternatives considered

- *Hash `/etc/pacman.conf` with the config collector* (WP-114): says that
  the file changed, not what pacman skips; it is WP-114's, and complements
  this.
- *A collector of its own:* a new name in `state.collectors`, in
  `[collectors]` and in every caller that lists them, for a few names the
  pacman collector can carry in its cursor.
- *A new event kind* (`ignore-change`): a closed enum value is not an
  optional field (ADR-0035 §6); a pacman `note` with two `meta` keys says
  the same.
- *Store the list in the ledger only* (the last event's `meta`): the
  baseline writes no event, so the first list would be unknown.
- *Crisis:* fails the harm test (§4); the bar's signal stays rare
  (ADR-0028 §1).
- *Show patterns resolved to installed packages:* needs the package
  database and changes with every install; the names as written are what
  the user can find and edit.
- *Refuse an include under a home directory:* would make such a
  configuration permanently partial (F6) for a case pacman itself
  handles; kept as the fallback if the operator does not want these files
  read.
- *An unshown name or an include outside `/etc` makes the list partial*
  (the first draft): one typo (`IgnorePkg = linux,nvidia-utils`) would
  silence every later change. Partial is kept for reads that are really
  incomplete (review round 1, F6).
- *The ledger's newest note decides alone whether a change is new* (the
  first draft): after a state loss an older note can name the new list,
  and a real change goes unrecorded (review round 1, F1). Bounded by
  `ignoreAt` instead.
