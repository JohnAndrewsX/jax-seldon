```
WP-059 HANDOVER
```

**Done**

- New module `engine/src/rebuild/shell_arg.rs` (declared in
  `rebuild/mod.rs`): `is_package_name` (makepkg charset), `is_unit_name`
  (systemd unit-name rules), `is_theme_slug`, `is_plugin_id` (the rule of
  `omarchy plugin validate`), `is_https_url` (host, optional port, plain
  path; no user info, query or fragment). None accepts a leading `-`.
  `quote` makes a checked value one shell word: unchanged when plain,
  else single-quoted.
- Every command sink in `rebuild/render.rs` checks its value first: the
  "Before the logbook" `sh` block, the per-package lines (repository,
  AUR, unknown), the two "all at once" lines, plugin clone (and the
  derived disable), plugin add URL, plugin enable id, the first-party
  "disable each" list, theme, user units (with and without a unit file),
  system units. An item whose value fails is rendered as
  ``- `value` — not reproduced: invalid name`` (German logbook: "nicht
  nachgebaut: ungültiger Name") with its case/event suffix and no
  command; `seldon rebuild` prints a warning for it, also in `--json`
  `warnings`.
- `dossier::parse_explicit` uses `is_package_name`: `Explicit` has a new
  `valid_name` flag (it replaces the whitespace-only check); `collect`
  puts failing names into `Before.invalid`.
- `render::code` picks a backtick fence one longer than the longest run
  in the text.
- Docs: SPEC-ENGINE §3 `seldon rebuild` lines; user guide page 08 in
  English and German (source marker moved to `170db59`); CHANGELOG
  `[Unreleased]` Engine line.

**Golden diff:** none. `engine/tests/golden/REBUILD.md` is unchanged —
every fixture name is valid and plain, so quoting changes no byte.

**Not done**

- Checking names when `seldon event` writes them (second guard, out of
  scope per the WP; the dossier fences are user-editable anyway).
- URLs with a query or fragment (`?`, `&`, `#`) are refused; such a
  plugin is listed as not reproduced.

**Verified by**

- `just check` on `68d4978` → exit 0, ending in `check: ok`. Cargo
  tests: 873 passed, 0 failed (54 suites, with and without `watch`).
  Clippy `-D warnings` and fmt clean. `docs-check: ok (380 links, 14
  translated pages, 40 commands, 425 command lines)`, with no
  source-commit warning. Plugin harnesses all passed. shellcheck is not
  installed on this host, so the script check ran `bash -n` only.
- `cargo test --test rebuild`: 9 passed, among them the new
  - `names_that_fail_their_check_never_reach_a_command` — per sink, eight
    made-up values with `;`, `$(`, a backtick, `${IFS}`, `|`, `&&`, a
    leading `-` and a quote. Asserts that no command (each `sh` block line,
    each code span starting with `omarchy `, `systemctl ` or `sudo `)
    holds the value, that each item is listed as "not reproduced: invalid
    name", and that `--json` `warnings` names it. It also checks that
    valid names render as before and that a URL with `~` is single-quoted.
    Plugins go in through `seldon event`, packages and unit files through
    ledger lines, and fences through the dossier files. A `|` cannot be
    part of a table cell, so for the table-fed sinks (services, plugin
    rows) the test asserts only that no command holds those values.
  - `a_theme_name_that_fails_its_check_is_not_set` — the same eight
    values as successive `theme-set` events.
- Unit tests: `rebuild::shell_arg::tests::*` (accept and refuse tables for
  each predicate and `quote`), `dossier::tests` (a `-x` / `a b` name is
  kept and marked), `render::tests::a_code_span_outlasts_the_backticks_inside`.
- Mutants: dropping the check in any one sink (11 mutants: each sink
  above, plus `quote` as identity) makes one of the two new tests fail;
  without the change, the two new tests fail on the old engine.
- Manual run (TESTING.md "Manual runs": scratch HOME/XDG,
  `SELDON_TEST_GUARD`, copy of `fixtures/logbook/`): rejected items show
  as not reproduced with warnings, a `~` URL and a `\x2d` unit name come
  out single-quoted, everything else unchanged.

**Learned:** `memory/pitfalls.md` (a mutant's binary outlives
`git checkout`).

**Decisions needed:** none.

**Touched outside WP scope:** `engine/src/commands/rebuild.rs` (+3
lines). It merges the render warnings into the human output and
`--json` `warnings`, which the WP's "warning in `--json`" needs. No other
active or queued WP names the file.

**Open questions**

- A rejected value is shown verbatim in a code span, so the user knows
  what to check. It is not presented as a command, and section 7 already
  shows subjects the same way. The reviewer may prefer an escaped form.
  (Answered in the fix round: control characters are escaped, the value
  stays visible.)

## Fix round 1 (review: SEND BACK, B1 + N1 + N2)

Commits `f62ede2` (engine + test), `0141b01` (SPEC).

**Done**

- **B1:** `render::code` now writes control characters (C0, DEL, C1,
  including `\n`, `\r`, `\t`) and U+2028/U+2029 as visible escapes
  (`\n`, `\r`, `\t`, `\u{1b}`). Other text keeps its bytes, for example
  `home-a\x2db.mount` and `ä`. This holds for the whole document,
  section 7 included. The golden `REBUILD.md` is still unchanged.
- **N1:** the "Before the logbook" block and the two "all at once" lines
  now pass their names through `shell_arg::quote`, like every other
  command. For valid names this changes no byte.
- **N2:** SPEC-ENGINE §3 now says the URL check refuses user info, query
  and fragment. It also names the code-span escapes.

**Verified by**

- New test `a_value_with_line_breaks_stays_inside_its_code_span`
  (`engine/tests/rebuild.rs`). Scratch ledger lines carry a made-up value
  that spans lines and holds its own `sh` fence, in these places:
  - a package subject (§2, the not-reproduced line);
  - plugin ids (§4: a line with a command and a not-reproduced line);
  - a theme (§5, not reproduced);
  - a unit file path (§6);
  - open and dismissed items (§7).

  The test asserts exactly one `sh` fence opener (the "Before the
  logbook" block) and exactly two fence lines in the whole document. It
  also checks that each value appears escaped in its section.
- Mutant: with the previous `code()` (no escaping) the test fails,
  finding 12 `sh` openers instead of 1.
- New unit test
  `render::tests::a_code_span_shows_control_characters_as_escapes`.
- `cargo fmt --check` and `cargo clippy --all-targets -D warnings` are
  clean. `cargo test --test rebuild` passed 10/10, `--test dossier`
  12/12, and the lib tests for `dossier`/`rebuild` 27/27.

**Not done / open questions**

- Three values that section 1 and the line suffixes print outside a
  code span are not escaped by this change:
  - the Omarchy version (`meta.to` of the last update, or
    `omarchy.summary`);
  - the agent actor;
  - the `[[case]]` id.

  The engine checks actor and case when it writes an event, but not
  when it loads a ledger line that was edited by hand. The version is
  never checked. A line break in one of them would still break the
  document's layout. A follow-up could route them through
  `code`/`visible` or `one_line`. This is left to the reviewer, because
  B1's decision named `code()`.
- `just check` was not re-run for this round. The rebuild and dossier
  suites, fmt and clippy were run as the review asked.
