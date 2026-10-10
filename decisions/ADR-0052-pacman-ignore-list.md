# ADR-0052 — pacman's ignore list in the index: the IgnorePkg and IgnoreGroup names, an optional v2 field; a change is attention

**Status:** proposed (the operator accepts it)
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
(`Sources::etc_dir`) as pacman does (`pacman.conf(5)`, pacman's
`conf.c`/`ini.c`):

- `#` to the end of the line is a comment; lines are trimmed; empty lines
  are skipped.
- `[name]` starts a section. The section is **shared across includes**:
  a header in an included file changes it for the rest of the includer,
  as pacman's parser keeps one section state.
- `Include = <pattern>` is followed wherever it stands: the pattern is
  expanded like `glob(3)` (`*`, `?`, `[…]` in any component; a leading
  `.` is matched only explicitly; matches sorted by name) and every match
  is read in turn, at most 10 levels deep (pacman's limit). Only absolute
  paths under `/etc` are followed (under `SELDON_ETC_DIR`, or the test
  guard's `<guard>/etc`, they are read there); a relative path or one
  outside `/etc` is not read and makes the list *partial*.
- In the `[options]` section, the values of `IgnorePkg` and `IgnoreGroup`
  (exact key, as pacman matches it) are split on white space; repeated
  lines add up. Names keep pacman's order (the include chain's), without
  repeats.
- **Nothing else is kept.** Every other line, key and value — servers,
  repositories, signature levels, other options — is read past and
  dropped; nothing of it reaches a cursor, an event, the index or a log.

Bounds (WP-174's readers): a file is read only when it is a regular file
(`sys::open_regular`) of at most 1 MiB; at most 64 files per run; a name
is 1–128 characters of `[A-Za-z0-9@._+*?!^[]-]` (a package name or an
`fnmatch` pattern, as pacman allows in both lists); at most 256 names per
list. A file that cannot be read, a missing literal include, a file or
name past a bound and a name outside the set make the list **partial**:
the names read are kept, the rest is left out. A pattern that matches
nothing is no error.

If `/etc/pacman.conf` itself cannot be read, the run keeps the last list,
marked partial (none before the first read).

### 2. The cursor

`PacmanCursor` gains two optional members (absent in 0.1.x cursors, which
read unchanged):

- `ignore` `{packages, groups, partial?}`: the list as the last run read
  it — what the index shows;
- `ignoreKnown` `{packages, groups}`: the last **complete** list — what a
  change is measured against.

A run without a cursor, or with a cursor without `ignoreKnown` (the first
run of this engine, a lost state directory), takes the list as it is, **no
event** — a baseline, as every collector takes one. A partial read writes
no event and leaves `ignoreKnown` alone, so a file that could not be read
is never reported as names removed. Because `capture` saves cursors only
after the ledger write, a failed write never loses a change, and a second
capture writes nothing (idempotent).

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
partial?: true}`: the pacman cursor's `ignore` (§2), in pacman's order;
`partial` present only when true. Every index build drops a name the
logbook's redaction would change (and marks the list partial); the field
is absent while the pacman collector is disabled, when `cursors.json` is
another logbook's or holds no list, and while `[redaction] patterns` do
not compile (as ADR-0038 §2 withholds its texts). Names are user content
(CONTRACT.md rule 6): shown as plain text, never evaluated, never an
argument of any command.

### 6. The desk

The System section (5) gains a seventh tile, **Ignored by pacman**: the
big value is the number of names in both lists ("ignored"); the lead,
when there is one, is **"pacman's full upgrade skips them; `pacman -S`
still updates them."**, with none "pacman ignores nothing: no IgnorePkg or
IgnoreGroup in pacman.conf."; a partial list adds "Part of pacman's
configuration could not be read; the list may be incomplete." The rows
are IgnorePkg and IgnoreGroup with their names. No stripe: the list is
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
- `fixtures/`: the story's 09-27 mesa downgrade is followed by the human
  pinning the three packages (`IgnorePkg = mesa vulkan-radeon
  lib32-mesa`); the next capture records the change, which the human
  explains. `fixtures/state/pacman-cursor.json` is the pacman cursor the
  golden test and the reference derive read for `system.pacmanIgnore`.
- SPEC-ENGINE §2 (the cursor), §4 (the pacman collector), §5 (the row),
  §6 (the field); SPEC-LOGBOOK (the meta keys); SPEC-PLUGIN (the tile,
  the hint); CONTRACT.md rule 9; CHANGELOG.

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
