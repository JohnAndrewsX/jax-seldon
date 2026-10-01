```
WP-035 HANDOVER
Done: seldon dossier [--section …] [--json] writes the eight generated fences of system/*.md (seven in use + new packages.explicit) from read-only queries and the ledger; rebuild §2 "Before the logbook"; shared views::merge_fence; init runs the dossier once after the first capture; tests, golden, docs
Not done: nothing in scope; follow-ups under "Decisions needed"
Verified by: just check (exit 0, again after review round 1), cargo test --test dossier (9 tests after review round 1), --test rebuild (7), --test init (29), lib unit tests (109); real-host read-only run on the dev host (scratch logbook copy, XDG redirected, real queries)
Learned: memory/rust-notes.md + memory/pitfalls.md, section "WP-035"
Decisions needed: none open (review round 1 decided all six; see the end)
Touched outside WP scope: collectors/{plugins,omarchy}.rs (two readers made public and Ctx-free), lib.rs / commands/mod.rs / main.rs (one line, one line, one variant + one arm), tests/common/mod.rs (query_shims, hardware_root), tests/init.rs (expects the dossier commit, pins SELDON_HARDWARE_ROOT)
```

Branch `wp/035-dossier`, worktree `wt/WP-035`. Main has not moved since the
branch point (`338ff3c`, checked before the handover). No PR, no push.
WP-034's files (watch.rs, watch/, systemd/, tests/watch.rs, Cargo.toml,
justfile) are untouched.

## What was done

- **`engine/src/dossier/query.rs`**: the host reads. Only these run:
  `pacman -Qqe`, `-Qqm`, `-Q`;
  `systemctl --system|--user list-unit-files --state=enabled --no-legend --no-pager`;
  `omarchy plugin list --json` (the plugins collector's `list`);
  `omarchy-version`, falling back to `pacman -Q omarchy` (the omarchy
  collector's `current_version`); the theme file; and the files
  `proc/cpuinfo`, `proc/meminfo`, `proc/mounts` and
  `sys/class/dmi/id/product_name` (no `lscpu`). Every call is a fixed argv
  through `sys::run`. The overrides are `SELDON_PACMAN`, `SELDON_SYSTEMCTL`
  (new), `SELDON_OMARCHY`, `SELDON_OMARCHY_VERSION`, `SELDON_THEME_FILE`
  and `SELDON_HARDWARE_ROOT` (new, default `/`). Exit 1 with no output at
  all counts as empty: `-Qqm` does that on a host without foreign packages.
- **`engine/src/dossier/mod.rs`**:
  - `Files` reads `system/*.md` and replaces fence *bodies* only, through
    `views::replace_fence`. When no file has a fence, it is appended to
    its default file under a `##` heading in the logbook language;
    `packages.explicit` on the fixture gets `## Explizite Pakete`.
  - `facts(&Built)` reads, from the folded events: the latest pacman
    `install` per package (a later `remove` clears it), the newest Omarchy
    `update`, the newest theme that was not dismissed, the cased config
    paths, and the unit cases.
  - The renderers, one per fence.
  - `parse_explicit`, which rebuild uses.
- **`engine/src/commands/dossier.rs`**:
  - The run: lock (exit 4), open the logbook (exit 3), derive the index,
    render the selected fences, then `Files::write`. The write is atomic
    and touches only changed files.
  - A change is committed as `seldon: dossier`, then the index is rebuilt
    (it reads these fences). Nothing is written to the ledger.
  - Each section's queries run lazily, once per run.
  - A failed query marks its fences `skipped` and keeps them, with a
    warning.
  - A `plugins.list` that comes back empty, when the table was not empty,
    is treated like the collector treats it: degraded, fence kept.
  - `--json`: `{files, sections: {<fence>: written|unchanged|skipped},
    counts: {explicit, preLogbook, total, aur, units, plugins}, git,
    warnings}`.
  - Human output: `Wrote system/… (N fence(s) changed)` or
    `Nothing changed (8 fence(s) checked)`, then a `Packages:` line.
- **Fence contents**:
  - `packages.summary`: explicit / total / aur, where aur counts every
    foreign package (`-Qqm`).
  - `packages.history`: the old lines verbatim. A row for today is added
    when the counts differ from the last row; if the last row is already
    today's, it is replaced.
  - `packages.explicit` (new): `- <name> · repo|aur · since <date>
    [[C-…]]`, or `· pre-logbook`, sorted. The origin is `aur` when the
    package is foreign. Versions are left out, so an upgrade does not
    rewrite the fence.
  - `services.enabled`: system units, then user units, each sorted by
    name. The case comes from the ledger (a user unit file's cased config
    event, or an agent's cased `systemctl [--user] enable <unit>`, parsed
    with `pkgcmd`). Without one, the old row's case is kept, else `—`.
  - `omarchy.summary`: version, theme (the theme file, else the ledger),
    and `lastUpdate` (the ledger). A key without a new value keeps its old
    value.
  - `hardware.summary`: `cpu`, `memory` (`MemTotal` rounded to whole GiB),
    `machine`, `rootfs`. Since review round 1, other lines of the old body
    (hand-written `gpu`, `displays`, `disk`) are kept in place.
  - `plugins.list`: one row per plugin, sorted by id, with `clonedFrom`.
  - `deviations.table`: the old lines byte for byte, plus one row
    `| path |  | date | [[case]] |` for each cased config path without a
    row. A path is left out when it is under `~/.config/systemd/`
    (units), dismissed, or added and removed again.
- **`seldon rebuild`** (`rebuild/{mod,render}.rs`): `Rebuild.before` comes
  from the `pre-logbook` lines of `packages.explicit`. §2 gains
  `### Before the logbook` with a one-line intro (counts per origin; a
  fresh Omarchy install already has many of them, and `omarchy pkg
  add|aur add` skip what is installed, checked read-only in
  `omarchy-pkg-add`). Then comes one `sh` block: `omarchy pkg add …` and
  `omarchy pkg aur add …`, wrapped with ` \` at 76 columns. The block lines
  do not start with ``- `omarchy pkg``, so the WP-032 trace helper still
  sees only the ledger's packages. Without the fence the old sentence
  ("Pakete von vor dem Logbuch fehlen hier …") stays.
- **`index/views.rs`** (WP-032 review item 6): `replace_fence(text, name,
  content)` and `merge_fence(existing, name, content)`. Their two callers
  are `merge_status` and `rebuild::merge`, now one line each. See decision
  3 for the one behaviour change.
- **`init`**: after the first capture (and its commit and index rebuild),
  `seldon dossier` runs once with all sections. It makes its own commit,
  `seldon: dossier`. `init --json` gains `dossier: {ran:true, files,
  sections, counts, git, warnings} | {ran:false, reason|error}`; the human
  output gains `Dossier: …`. With `--no-capture`, or when the capture
  failed, the dossier does not run, and `seldon dossier` is added to the
  next steps.
- **Tests**: `engine/tests/dossier.rs` (7), the golden
  `engine/tests/golden/dossier.md`, the shim inputs in `fixtures/logs/`
  (`pacman-Qqe.txt`, `pacman-Qqm.txt`, `pacman-Q.txt`,
  `systemctl-system.txt`, `systemctl-user.txt`, `hardware/proc/*`,
  `hardware/sys/class/dmi/id/product_name`), and `Env::query_shims` in
  `tests/common/mod.rs`. The rebuild golden test runs `dossier --section
  packages` first and checks the new group with `before_trace`. Unit tests
  are in `dossier/`, `dossier/query.rs` and `rebuild/render.rs`.

## The init/capture choice

`init` runs the dossier, once, after the first capture. `capture` and
`status` never run it; a test proves that neither touches `system/*.md` nor
asks the package or unit queries. The reasons:

- **The dossier is a snapshot, not an event stream.** A refresh on every
  15-minute capture would commit `packages.history` and `services.enabled`
  churn into the logbook for no reader.
- **ADR-0005 stays as it is.**
- **The user, or a later `seldon watch` / plugin action, refreshes on
  purpose** with `seldon dossier`.

The dossier commit comes after the first-capture commit, so
`init` with a capture now ends with three commits (`init logbook`, `first
capture…`, `dossier`) and a clean tree. The index names the last head.

## How it was verified

- `just check` → exit 0 (fmt, clippy `-D warnings`, all engine tests,
  schema-validate, plugin-validate, qmllint, plugin tests).
- `cargo test --test dossier` (7 tests; all go through `common::Env` with
  `SELDON_TEST_GUARD`, and every query is a shim):
  - **The fixture copy matches the golden.** All eight fences are present.
    Outside the fences each file is byte-identical: only `packages.md`
    grew, by the appended `packages.explicit` block. The shim log holds
    exactly the six read-only queries, once each. `seldon: dossier` is
    committed once and the tree is clean.
  - **A second run a day later changes nothing.** `files: []`, every
    fence `unchanged`, the human output starts with "Nothing changed (8
    fence(s) checked)", and nothing is committed.
  - **Emptied fences are all filled.** Four packages are marked `since`
    (btop; ollama; tailscale `[[C-2026-008]]`; zed `[[C-2026-004]]`) and
    eleven `pre-logbook`. The cased deviation rows are monitors.conf
    (C-2026-002) and bindings.conf (C-2026-004). The hardware comes from
    the fixture files.
  - **Without the programs on PATH**, the packages, services and plugins
    fences are skipped with three warnings and stay byte-identical.
  - **`--section`** writes only its fences: `services` alone; then
    `packages,plugins` plus `--section hardware`; an unknown value exits 1.
  - **A cased change through the CLI.** A cased `seldon event config
    config-change` adds one deviations row and leaves the old file byte
    for byte otherwise. A cased agent `systemctl --user enable --now
    pipewire.socket` puts the case on that unit. tailscaled keeps its old
    row's case.
  - **`capture --all` and `status` leave the dossier alone.**
  - Exit 3 without a logbook, exit 4 with the lock held.
- `cargo test --test rebuild`: 7 tests, including the WP-032 trace test
  (unchanged, still 4 package lines) and the new `before_trace` (11
  packages: 9 repo, 2 AUR; each under its own command, none extra).
- **Real-host read-only run on this dev host.**
  - Setup: a scratch copy of the fixture logbook. `HOME` and all three
    `XDG_*` variables point into the scratchpad, with `SELDON_TEST_GUARD`
    set and `--no-commit`. The real PATH is used, so the queries were real.
  - Counts: **169 explicit packages, 167 of them from before the logbook,
    966 packages in total, 0 foreign/AUR, 40 enabled units (system +
    user), 38 plugins**, no warnings.
  - The second run printed "Nothing changed".
  - `seldon rebuild` on the same copy lists the 167 `pre-logbook`
    packages in the "Before the logbook" block (29 lines, at most 76
    columns).
  - Nothing under the real `~/.config` or `~/.local/state` was touched:
    `find … -newer <marker>` is empty for every seldon path, and
    `~/.local/state/seldon` does not exist.
  - `~/.config/seldon/config.toml` exists but dates from 17:10, before this
    run; another session wrote it earlier, as already noted in
    memory/pitfalls.md for WP-022/024.

## SPEC wording (already in this branch)

- SPEC-ENGINE §3 replaces the `seldon dossier` line:

  ```
  seldon dossier [--section packages|services|omarchy|hardware|plugins|deviations|all] [--json]
                                                 # WP-035: rewrites only the bodies of the selected generated
                                                 # fences of system/*.md (comma list or repeated; default all):
                                                 # packages.summary (explicit/total/aur), packages.history (a row
                                                 # for today when the counts changed; today's row is replaced),
                                                 # packages.explicit (sorted, `- <name> · repo|aur · since <date>
                                                 # [[C-…]]` from the ledger's latest install, else `pre-logbook`),
                                                 # services.enabled (system then user units; case from the ledger:
                                                 # a user unit file's config event or an agent's cased `systemctl
                                                 # [--user] enable`, else the old row's, else —), omarchy.summary
                                                 # (version, theme, lastUpdate), hardware.summary (cpu, memory,
                                                 # machine, rootfs from /proc and /sys files only), plugins.list,
                                                 # deviations.table (old lines kept byte for byte, a row added per
                                                 # cased config path without one, reason left empty). Text outside
                                                 # the fences is never changed; a missing fence is appended to its
                                                 # default file under a heading in the logbook language. A failed
                                                 # query skips its fences (warning, fence kept). Files written
                                                 # atomically and only on change, autocommit `seldon: dossier`,
                                                 # index rebuilt; no ledger write. `init` runs it once after the
                                                 # first capture; `capture` and `status` never do. --json →
                                                 # {files, sections: {<fence>: written|unchanged|skipped}, counts:
                                                 # {explicit, preLogbook, total, aur, units, plugins}, git, warnings}
  ```

- The `rebuild` line in §3 gains "Before the logbook" in section 2.
- The `init --json` shape gains the `dossier` key.
- §4 gains one sentence: "`seldon dossier` (§3) is not a collector and
  writes no events; it queries pacman read-only (`-Qqe`, `-Qqm`, `-Q`)
  and systemd read-only (`list-unit-files --state=enabled`); no package
  manager or `systemctl` is ever invoked with a mutating verb."
- §9 puts `seldon dossier` into the init order.
- SPEC-LOGBOOK §3 "Dossier" gains the table of all eight fences (file,
  content format). It also says that a missing fence is appended under a
  heading, that existing deviation rows are never changed, and that
  rebuild lists the `pre-logbook` packages.
- `docs/TESTING.md` gains a row for `tests/dossier.rs`, an updated row for
  `tests/rebuild.rs`, and the manual and real-host block. CONTRACT.md is
  unchanged: no schema field changed, and the plugin does not read
  `packages.explicit`.

## Decisions needed (none blocks the merge)

1. **`--section hardware` is new.** The WP's list (`packages | services |
   omarchy | plugins | deviations | all`) gave `hardware.summary` no
   section. I added `hardware` instead of folding it into `omarchy`.
   Accept, or fold it in?
2. **Existing deviation rows are never changed.** A later cased event on
   a path that already has a row does not fill that row's empty case or
   move its date; the WP said "existing rows … are kept". `rebuild`
   still shows the event's case when the row has none. Fill empty case
   cells, or keep this?
3. **Unifying `merge_fence` changed one edge case of `rebuild::merge`.**
   A REBUILD.md that starts with the generated header but has no
   `rebuild` fence is now replaced, as STATUS.md always was. Before, its
   text was kept below the fence. A file with the "do not edit" header is
   the engine's, and no test or fixture had that shape. Accept?
4. **"Before the logbook" lists every pre-logbook explicit package**
   (167 on the dev host), including those Omarchy's installer brings. The
   commands skip installed packages, so this is safe but long.
   `$OMARCHY_PATH/install/omarchy-base.packages` exists on the host, so a
   follow-up could split "comes with Omarchy" from "your own additions"
   (a read-only file read; Omarchy's list can differ between versions).
   Queue a WP?
5. **The init templates get no `packages.explicit` fence.** The templates
   are outside this WP's paths, and changing them moves the init-skeleton
   golden. A new logbook's first dossier run appends the fence under
   `## Explicit packages` / `## Explizite Pakete`. Likewise
   `fixtures/logbook/system/packages.md` is unchanged, so
   `index.sample.json` does not move. The rebuild golden gets the fence by
   running the dossier with shims. Should the templates and the fixture
   carry the fence (Schema Keeper / a small follow-up)?
6. **The hardware fence holds four keys.** They are `cpu`, `memory`
   (MemTotal, so 64 GB shows as `63 GiB`), `machine` and `rootfs`. The
   fixture's hand-made `gpu`, `displays` and `disk` lines would disappear
   on the first real run, because the files cannot provide them without
   `lspci`, DRM parsing or root. Fine for v1?

Smaller choices, no decision needed:
- `--json` carries `counts` and `git` besides the WP's `{files, sections,
  warnings}`.
- A dismissed install (ollama after `drift dismiss`) is still `since …` in
  `packages.explicit`, because the ledger knows it. `rebuild` lists it
  neither in §2 nor under "Before the logbook".

## Guard note

A read-only `grep -n -i "…\|systemctl\|list-unit…" memory/host.md` was
blocked as a "service or boot command" (the word was only in the search
pattern). Nothing ran. I did not reword the command and read the file with
the Read tool. No other block. I typed no package-manager or `systemctl`
command myself; the real-host run invoked only the engine.

## Review round 1 (APPROVE, no blocking items; fixed in one commit)

Decisions taken by the reviewer:
- 1 `--section hardware` accepted.
- 2 existing deviation rows stay untouched (filling empty cells → follow-up
  WP).
- 3 the `merge_fence` edge case accepted.
- 4 splitting Omarchy's base packages from the user's additions →
  follow-up WP.
- 5 templates and fixture carrying `packages.explicit` → follow-up WP.
- 6 fixed (below).

Fixes, commit `engine: dossier review fixes (WP-035)`:

- **(6) Hand-written hardware lines stay.** `hardware_summary(hw, body)`
  goes through the same `key_values` as `omarchy.summary`. `key_values`
  now owns only its own keys: each engine key's line is refreshed in
  place, and every other old line is kept verbatim. Keys the body lacks
  are appended, and a stale duplicate of an engine key is dropped.
  - On the fixture, `gpu`, `displays` and `disk` survive. That adds three
    lines to the golden `tests/golden/dossier.md`; the integration test
    asserts the whole body.
  - Unit test `hardware_keeps_hand_written_lines`, idempotent.
  - `omarchy.summary` uses the same function, so a hand-written key there
    is kept too.
- **(a) One lookup for "has the fence".** `views::fence_body` finds a
  body exactly as `replace_fence` does (`replace_fence` is built on it).
  `Files::body` uses it, and `Files::set` decides with
  `replace_fence(...)`. So an unclosed `<!-- seldon:begin notes -->` above
  a fence can no longer hide it and make every run append it again.
  - Unit test `a_damaged_marker_elsewhere_does_not_hide_the_fence`, and
    integration test `a_damaged_marker_above_a_fence_never_appends_it_again`:
    the second run gives `files: []`, and each fence appears once.
  - Negative control: with the old logic put back, the integration test
    fails.
  - Note: the index loader (`load::fences`, a sequential scan) still
    misreads such a damaged file. That is outside this WP and unchanged.
- **(b) Redaction.** The command builds
  `Redactor::with_patterns(&config.redaction.patterns)` before taking the
  lock, so an invalid pattern exits 1 and nothing is written. These host
  strings go through it before rendering:
  - package names;
  - unit names;
  - plugin ids and `clonedFrom`;
  - the four hardware values;
  - the Omarchy version and theme;
  - the paths of new deviation rows.

  Old user lines are not rewritten. The integration test
  `host_strings_go_through_the_users_redaction_patterns` uses patterns
  `MS-7D\d+`, `pipewire` and `weather-plus`: the result is `machine:
  ‹redacted›` and `| ‹redacted›.socket | user | — |`, with no
  weather-plus left. An invalid pattern gives exit 1 with the files
  unchanged.
- **(c)** `enabled_units` drops a unit name containing `|`.
- **(d)** `enabled_by` ignores a path argument (`systemctl enable
  /etc/…/x.service`); this has a unit-test assertion.

Verified:
- `cargo test --test dossier --test rebuild --test init` → 9 / 7 / 29
  passed.
- `cargo clippy --all-targets -- -D warnings` clean, `cargo fmt --check`
  clean.
- `just check` → exit 0.
- The real-host read-only run was repeated on a fresh scratch copy (XDG
  redirected, `--no-commit`). The counts are unchanged (169 explicit, 167
  pre-logbook, 966 total, 0 AUR, 40 units, 38 plugins, no warnings). The
  three hand-written hardware lines were kept. The second run printed
  "Nothing changed". Nothing under the real `~/.config` or
  `~/.local/state` was written.

No package-manager or service-manager command was typed by me in this
round. The guard blocked nothing.
