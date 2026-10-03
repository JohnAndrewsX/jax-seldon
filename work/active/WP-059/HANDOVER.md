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
