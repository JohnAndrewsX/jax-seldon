# WP-129 — plan

Branch `wp/129-privileged-commands` from `next` (c1ee5d27), merged into
`next`.

## What the code does today

- `pkgcmd::unwrap_command` reads past `sudo`, `doas`, `pkexec`, `run0`
  (and `env`, `nice`, `timeout`, …) and returns an empty argv for a probe
  (`sudo -l`, `pkexec --version`, `command -v …`). It does not say which
  wrapper it read past.
- `pkgcmd::simple_commands` opens `sh|bash|zsh|dash -c '…'` and `eval`;
  the opened commands lose the wrapper of the shell that ran them
  (`pkexec sh -c 'lpadmin …'` reads as a bare `lpadmin`).
- `hook::classify` picks one mutation per line from the classes of
  SPEC-ENGINE §8; `pkexec lpadmin …` matches none, so nothing is
  recorded.

## Steps

1. `pkgcmd`: `PRIVILEGE_WRAPPERS`; `Unwrapped::privilege` (the first
   privilege wrapper read past, none for a probe); `Segment::privileged_by`
   (inherited from the `sh -c`/`eval` the command was opened from) and
   `Segment::privilege()`. Unit tests.
2. `hook`: `Mutation::wrapper`; `mutations(line)` = the line's class
   record (as before) plus at most one privileged record: the first
   segment that has a privilege, runs a program that is not a probe, and
   whose own class records nothing by itself (none, or an ADR-0019 green
   record that needs a case; a snapshot command stays its own class).
   The privileged segment's green record is superseded, not doubled.
   Record: `agent/command`, subject = program basename, zone red,
   recorded without a case too, `detail` and `meta.command` = the
   redacted, clipped line, `meta.wrapper`.
3. `case_notes`: a privileged record is never a snapshot command (a
   `sudo snapper delete` must not own a snapshot).
4. Tests: unit (`hook.rs`), integration (`tests/hooks.rs`): the WP's list
   plus nested wrappers, `-u`, variables, skipPaths, two classes in one
   line, replay.
5. ADR-0039 (proposed): the record class and the ADR-0028 row
   *attention*; the row needs `drift[].source` to admit `agent` — a
   contract question (see Decisions), so the engine does not make the
   record drift yet.
6. SPEC-ENGINE §8, guide 04 en/de, CHANGELOG.
7. `just check`, `just check-perf` (hook budget).

## Decisions (WP leaves open)

See HANDOVER.md §Decisions; summarised here as they are taken.

- Kind stays `command` (the SPEC convention for every hook record; a new
  kind is a schema enum change). The class is marked by `meta.wrapper`,
  which only privileged records carry.
- Zone red: the command acts on the system as root (ADR-0014: red is where
  packages and system services act); it also makes the record count as the
  case's first red change for `plan snapshot`'s order check.
- Probes besides the wrappers' own: `true`, `false`, `:`, `id`,
  `whoami`, `test`, `[` (`sudo -n true` tests the credential cache).
- One privileged record per line (the first privileged program is the
  subject; the detail holds the whole line).
