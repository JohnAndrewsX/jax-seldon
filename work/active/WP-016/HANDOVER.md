WP-016 HANDOVER

Branch `wp/016-fixture-additions`, worktree `wt/WP-016`, on top of `main`
at `90c3dec`. Not pushed, no PR. Commits `main..HEAD`:
`7e1e2ad` drift index variants · `f07b5a4` hook payloads · `f00e7d2`
STATUS.md regenerated · `2d3c626` engine golden re-blessed (see *Touched
outside WP scope*) · `ff5b029` `-pre` payload check · `25b931e` memory ·
then this handover. `just check` exits 0 at HEAD.

## Done

- **Three index variants**, registered as overlays in `VARIANTS`
  (`scripts/validate-fixtures.py`), written with `--write-index`, and
  documented in the README variant table:

  | Variant | The one jq edit of the sample | Surface |
  |---|---|---|
  | `drift-explained-case` | `.events \|= map(if .id == "01M1MB2M1GWZYF485HTGVZ1KS3" then .case = "C-2026-002" else . end)` | btop row: `explained · C-2026-002: Kleines Monitoring-Tool, bewusst ohne Case.` (ADR-0021) |
  | `drift-capped` | `.summary.openDrift = 250` | "+246 more open drift items not listed here", pill `⟡ 2 · 250` (ADR-0020) |
  | `drift-members-capped` | `.events \|= map(select(.id != "01M3SXBRV0E702XKBM22HEV1B8"))` | the sheet shows "… and 1 more", then `drift show` lists all three members (CONTRACT.md rule 4) |

  - Each overlay `test`s the ids, subjects and values it relies on, so a
    sample change cannot silently move it onto another event.
  - Each file is byte-for-byte (`jq -S`) equal to its jq edit.
  - The sample itself is unchanged.
- **`fixtures/hooks/`**: five new payloads.
  - `claude-code-{mutating,non-mutating,secret}-pre.json`: the PostToolUse
    originals as `PreToolUse` without `tool_response`, with the same
    `tool_use_id`, so a Pre/Post pair is one event.
  - `claude-code-edit-watched.json`: an `Edit` of
    `/home/user/.config/hypr/monitors.conf`. The path is watched, so this
    gives a yellow `edit` event, with or without a case.
  - `claude-code-write-unwatched.json`: a `Write` of
    `/home/user/.config/zed/settings.json`. The path is not watched, so it
    gives a green `write` event with a case and nothing without one
    (ADR-0019).
  - README `## hooks/` rewritten as a table: per payload, the outcome
    without a case and with an active case, the common event fields, and
    how to run them (`HOME=/home/user`, `XDG_CONFIG_HOME` unset, scratch
    `SELDON_CONFIG` and `XDG_STATE_HOME`).
  - New validator check (3b): every `*-pre.json` must equal its
    PostToolUse sibling, with `hook_event_name: PreToolUse` and without
    `tool_response`. Negative control: a changed `command` in
    `claude-code-secret-pre.json` fails it. The file was restored
    afterwards.
- **`fixtures/logbook/STATUS.md` regenerated.**
  - Made with `seldon --logbook <copy> status --json` at
    `SELDON_NOW=2026-10-01T17:05:12+02:00`, in a scratch HOME.
  - Result: header line, `seldon:begin status` fence, English headings,
    German prose; 4 open drift items, 30/41 events. That fixes WP-007
    decision 3 (the old file said 3 drift, 27/35).
  - Byte-identical (`cmp`) to the `generated` text in
    `engine/tests/status.rs`.
  - A second `status` on a copy of the new fixture writes nothing
    (`files: []`, `diff -r` clean).
- `fixtures/README.md` updated: the path table, the variant table and
  `hooks/`.
- `memory/pitfalls.md` appended.

## Not done

- **The engine hook tests do not read the new fixtures yet.** `engine/`
  is not mine. What switching would take:
  - Bash Pre payloads: `payload("claude-code-mutating-pre.json", …)` works
    as it is.
  - `Edit`/`Write`: they carry `/home/user/…` paths. A temp-HOME test
    needs a one-line change in the `payload()` helper (`.replace("/home/user",
    home)` on the raw text) before `edit_and_write_on_watched_paths` and
    `green::recorded_with_a_case` can use them.
  - Today those tests still build their payloads with `tool_call()` and
    pass.
- **`tests/plugin/` unchanged.** No count changed: panel-view 419 and
  service-states 148, as before. The suites still build the capped and
  members-capped indexes with jq. They could point at the new variant
  files instead, but that is the plugin track's call.

## Verified by

```
$ bash scripts/validate-fixtures.sh
validate-fixtures: ok — 109 instances (109 incl. 8 expected failures), 71 ledger events
  traced to index.sample.json, 8 variants, 23 self-checks; backend builtin
$ just check                                   → exit 0
  cargo tests: 292 passed, 0 failed · validate-fixtures ok · plugin-validate: ok
  tokens: ok (314 references) · qmllint: ok (15 files)
  model.test.js: 47 passed · service-states: 148 passed, 0 failed · panel-view: 419 passed, 0 failed
$ find plugin -type l | wc -l                  → 0
```

Only the `builtin` schema backend is available on this host: there is no
python `jsonschema` and no `check-jsonschema`.

**Variant render checks.** I made a one-off copy of the
`tests/plugin/panel-view.sh` prelude in my scratch dir. It is the same
private offscreen Quickshell, with the shell's own `Commons/` and `Ui/`,
`env -i`, a temp HOME and the fake engine. Result: 16 passed, 0 failed,
and the real `~/.local/state/seldon` and `~/.config/seldon` were
untouched.

- `drift-explained-case`, after `tab:changelog;key:Down;key:Down*61`:
  - `.view.changelog.resolved` holds `btop: explained · C-2026-002:
    Kleines Monitoring-Tool, bewusst ohne Case.`;
  - the same text is on screen, along with `1.4.5-1 · system ·
    C-2026-002`;
  - `folded` is still 7; the log is clean.
  - Control: the sample's btop row reads `explained: Kleines …`, with no
    case.
- `drift-capped`:
  - `.view.changelog.more` and the on-screen text read "+246 more open
    drift items not listed here";
  - pill `⟡ 2 · 250`; the log is clean.
- `drift-members-capped`, live with the fake engine fed this variant and
  the noto event as `extra-events.json`:
  - `resolve:<firefox>` shows `firefox | libinput | … and 1 more`;
  - then 3 members, after the exact engine argv `… drift show
    01M3SXBQVR7AW8PJQC1YXDCQ14 --json`; the log is clean.
  - In dev mode the variant shows 61 rows and badge `firefox +2`.

**Hook payloads against the real engine** (debug build from this branch):

- **Setup:**
  - a scratch logbook from `seldon init --non-interactive`;
  - `HOME=/home/user`, which does not exist on this host, so nothing can
    be written there;
  - `XDG_CONFIG_HOME` unset;
  - `SELDON_CONFIG`, `XDG_STATE_HOME`, the pacman log and the theme file
    in scratch;
  - `SELDON_NOW=2026-10-01T10:11:20+02:00`.
- **Exit codes:** every run exited 0, with empty stdout and stderr.
- **Without a case**, each payload on a fresh logbook:
  - `mutating`(-pre): `yay` red, `yay -S --noconfirm zed`.
  - `non-mutating`(-pre): nothing.
  - `secret`(-pre): `git` yellow, `AWS_ACCESS_KEY_ID=‹redacted› git -C
    ~/.config/hypr push https://‹redacted›@github.com/example/dotfiles.git
    main --password ‹redacted›`.
  - `edit-watched`: `edit` yellow, `Edit ~/.config/hypr/monitors.conf`.
  - `write-unwatched`: nothing.
- **Pairs:** Pre then Post of the same `tool_use_id` gives one event for
  `mutating` and `secret`, and none for `non-mutating`.
- **With an active case** (`plan new` + `plan start` → C-2026-001):
  - `edit-watched`: yellow, with the case.
  - `write-unwatched`: `write` green, `Write ~/.config/zed/settings.json`,
    with the case.
  - `mutating-pre` and `secret-pre`: as above, plus the case.
  - `non-mutating-pre`: nothing.
  - The case's `events:` lists all four.
- **Nothing leaked:** `grep` of the ledgers finds no fake secret, no
  `tool_response` text and no Edit/Write content (`2560x1440`, `Tokyo
  Night`, `vim_mode`).

## Learned (in memory/pitfalls.md)

- The hook resolves `~` from `$HOME`, but checks "inside `~/.config`"
  against `$XDG_CONFIG_HOME`.
  - My first run had a scratch `XDG_CONFIG_HOME` with
    `HOME=/home/user`. It recorded nothing for the secret payload.
  - That was the environment, not the fixture: use `SELDON_CONFIG`
    instead.
- Edit/Write fixtures with absolute paths only reach `watchPaths` when
  `HOME` matches their prefix.
- Fixture logbook files feed engine goldens (`session-start.txt` quotes
  STATUS.md). Grep `engine/tests/golden/` too.
- Harness step numbers count steps, not key presses.

## Decisions needed

1. **The golden re-bless in `engine/`** (commit `2d3c626`, see below).
   The WP constraint allows "test expectations", but the brief says
   `engine/` is not mine. If the engine track should own it instead, drop
   `2d3c626` and they re-bless with `SELDON_BLESS=1 cargo test --test
   hooks session_start`. Without it, `just check` fails on main once the
   new STATUS.md lands.
2. **`drift-explained-case` uses C-2026-002, as the WP says.**
   Semantically, ADR-0021's `drift explain` creates a *new* completed case
   and does not reuse an old one. C-2026-002 is a real completed case the
   plugin can open, which is all the variant needs. The logbook is not
   touched, so C-2026-002's `events:` does not list btop. The README says
   so. If you want it truer to ADR-0021, it would need a new case in the
   logbook, which is a story change.
3. **`schema/external/claude-code-hook.schema.json` description is
   outdated.** It says "Only tool_input.command (Bash) is read by the
   engine", but the hook also reads `tool_input.file_path` (and `edits[]`)
   for Edit/Write/MultiEdit. That is a doc-only change to a third-party
   input schema, not the contract. I left it alone because the brief says
   "schema/: no changes expected". Say if it should go in a follow-up.

## Touched outside WP scope

- `engine/tests/golden/session-start.txt` (commit `2d3c626`): re-blessed,
  test expectation only.
  - The diff is the three Status lines the golden quotes from the fixture
    `STATUS.md`: `Stand: … · letztes Ereignis 17:00 …`, `Offene Drift:
    4`, `Ereignisse heute: 30 · letzte 7 Tage: 41`.
  - Those lines now agree with the index.
- No engine or plugin code. `schema/` and `tests/plugin/` are untouched.
  No contractVersion bump; existing ids are stable; the sample index is
  unchanged.
- No guard block was hit.
