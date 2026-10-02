```
WP-050 HANDOVER
Done:
- (1) `seldon plan start` warns on an R2 or R3 case with no snapshot
  (no `--snapshot` and no `snapshotBefore` in the case) and never
  refuses (9e378c5). Human: a `warning: …` line after the transition;
  `--json`: `warnings: [...]`, present on every plan step and empty
  unless `plan start` warned. Texts:
    R2: "C-… is risk R2 and was started without --snapshot: its rollback
        needs a snapshot or backup; take one before the first change and
        note it in the case (ADR-0023)"
    R3: "C-… is risk R3 and was started without --snapshot: a snapshot is
        mandatory before the first change, and the work needs the human's
        explicit go per step, never unattended (ADR-0023)"
  `--snapshot` help: "Snapper snapshot taken before the work (`plan start`
  only; an R2 or R3 case started without one gets a warning, ADR-0023)".
  SPEC wording (SPEC-ENGINE §3 synopsis, now also listing `--actor`):
    seldon plan start|verify|done|drop <ID> [--snapshot N] [--reason TEXT] [--actor A]
    # --snapshot: `plan start` only. Starting an R2 or R3 case with no snapshot
    # (no --snapshot, no snapshotBefore) prints a warning and never refuses
    # (ADR-0023); R3's also asks for the human's explicit go per step (WP-050)
  and in "JSON shapes": plan steps return … `journal` (WP-006) and
  `warnings` (a list of strings, empty unless `plan start` warned; WP-050).
- (2) `[drift] alwaysRed` default reviewed against ADR-0023's R3 subjects
  (272e65c, fix round eff22f5). Final default:
    linux, linux-lts, linux-zen, linux-hardened, linux-rt, linux-rt-lts,
    linux-omarchy, systemd, glibc, hyprland, omarchy, omarchy-settings,
    quickshell, limine*, grub, mkinitcpio*, filesystem, pam, sddm, uwsm
  Mapping: kernel → the kernel packages only (fix round; see below); systemd; glibc; Hyprland → hyprland; Omarchy
  itself → omarchy + omarchy-settings (Omarchy 4 ships its /etc layer
  there: mkinitcpio, limine, sddm, modprobe confs); boot loader →
  limine* (Omarchy's), grub, and mkinitcpio* (initramfs); /etc →
  omarchy-settings and filesystem (base /etc files). quickshell kept
  (ADR-0013: the shell); login → pam, sddm, uwsm (fix round). Deliberately not added: systemd-libs,
  hyprutils/hyprlang (move in the same transaction as their leader),
  omarchy-nvim/omarchy-keyring (not boot/login/shell). The globs only
  see package names, so `/etc` itself cannot be a glob.
  SPEC wording (§2 config row, after the fix round): "`[drift] alwaysRed`
  (ADR-0013; package globs, default `linux`, `linux-lts`, `linux-zen`,
  `linux-hardened`, `linux-rt`, `linux-rt-lts`, `linux-omarchy`, `systemd`,
  `glibc`, `hyprland`, `omarchy`, `omarchy-settings`, `quickshell`,
  `limine*`, `grub`, `mkinitcpio*`, `filesystem`, `pam`, `sddm`, `uwsm` —
  the R3 subjects of ADR-0023 as packages: the kernels only (firmware and
  headers are not R3; another kernel package is added by hand), the login
  path `pam`/`sddm`/`uwsm`, `/etc` through `omarchy-settings` and
  `filesystem`; WP-050. `init` writes the list into the file, so an
  existing config keeps its own)". §5 rule 5 now says
  "default in §2". Unit test `always_red_globs` covers each new glob and
  the near misses (omarchy-nvim, hyprutils, grub-customizer).
- (3) docs/CONCEPT.md "Agents and the logbook": the hook table names
  SessionStart, PreToolUse (Bash, Edit, Write, MultiEdit; start time per
  ADR-0017) and SessionEnd (why not Stop); the generic form gains `cwd`
  and "before the command runs" (5766b30).
- (4) `hook install generic`: DROPPED from the SPEC-ENGINE §3 synopsis,
  not implemented (6ea26cc). Why: SPEC §8 documents no generic hook
  script, only the stdin entry point `seldon hook generic`, which other
  agents call themselves ("Other agents call `hook generic` themselves").
  There is no generic harness settings file to merge into, so `install
  generic` would have to invent a script and a location nobody reads.
  The binary already accepts only `claude-code`. SPEC wording:
    seldon hook install claude-code [--settings PATH]
    # WP-050: `generic` dropped from the synopsis: it has no settings file
    # to merge into; other agents pipe into `hook generic` themselves (§8)
- (5) Wizard: "Agent harnesses (space toggles, enter confirms)" (fa375ea).
- (6) `seldon doctor`: the `logbook` row's summary, its "not initialised
  at …" message and the `seldon init --path …` fix use the header's
  `~`-shortened path (21b3cac). JSON `logbook` stays absolute; `init
  --path ~/x` expands `~` itself, so the fix works without a shell too.
  The git row's `git -C <path> init` fix is unchanged (absolute).
- (7) `decisions.index` (89d1edf): `seldon decide` and `seldon status`
  fill the fence of the logbook's DECISIONS.md from decisions/*.md
  frontmatter. Row `| [[ADR-NNNN]] | title | status | date |`, newest id
  first (the format of fixtures/logbook/DECISIONS.md), `|` in a title
  escaped as `\|`. The table head inside the fence is kept when it has
  one (a translated head stays; the fixture is byte-stable), else
  `| ID | Title | Status | Date |`. Fence mechanics as in the dossier
  (`views::replace_fence`): bytes outside the fence never change; a
  missing fence is appended under `## Index`; a missing file is created;
  written atomically and only on change. One deviation from the dossier,
  on purpose: a begin marker without its end leaves the file alone with
  a warning (see Decisions needed). `decide` fills it before its
  autocommit (same commit as the ADR; a fill failure is a warning in
  `warnings`, exit 0); `status` lists `DECISIONS.md` in `files`. Shared
  helpers `index::load::decisions` and `index::build::decision_rows`.
  Template sentence (en): "Seldon fills the table between the fences from
  `decisions/` on every `seldon decide` and `seldon status`; your own text
  goes outside it." (de): "Die Tabelle zwischen den Zäunen füllt Seldon
  bei jedem `seldon decide` und `seldon status` aus `decisions/`; eigener
  Text gehört außerhalb davon." SPEC wording (§3, under decide/status):
    # decide and status (WP-050) fill the `decisions.index` fence of the logbook's
    # DECISIONS.md from decisions/*.md frontmatter: `| [[id]] | title | status |
    # date |`, newest id first, `|` in a title escaped; the table head inside the
    # fence is kept when it has one (a translated head stays), else `| ID | Title |
    # Status | Date |`. Text outside the fence is never changed; a missing fence is
    # appended under `## Index`, a missing file created; a begin marker without its
    # end leaves the file alone (warning). Written only on change; decide commits
    # it with the new ADR, status lists it in `files`.
  and `decide --json` → {decision, editor, git, warnings}.
  Init golden: unchanged (it pins headings, fence and table head only,
  none of which changed); init test now checks the DECISIONS.md sentence
  names `seldon decide` and `seldon status` in en and de.
- (8) CHANGELOG [Unreleased] → new "### Engine" block (b44c6f1), incl.
  the note that an existing config.toml keeps its old alwaysRed list.

Fix round (review: APPROVE; orchestrator decisions applied):
- (1) eff22f5 — red list: `pam`, `sddm`, `uwsm` added; `linux*` replaced
  by the kernel packages `linux`, `linux-lts`, `linux-zen`,
  `linux-hardened`, `linux-omarchy` as asked, plus `linux-rt` and
  `linux-rt-lts` (the two remaining official Arch kernels; drop them if
  unwanted). Globs have no negation, so "kernels only" is an explicit
  list: `linux-firmware*`, `linux-api-headers` and every `*-headers` stay
  routine; an AUR kernel (e.g. linux-cachyos) must be added by hand (SPEC
  says so). Changed together: config.rs, the validator's `ALWAYS_RED`
  and its self-checks (linux-firmware now yellow; new: limine-snapper-sync
  glob red, sddm red; 23 → 25 self-checks), engine/tests/index.rs (same
  three cases), `always_red_globs` (pam/sddm/uwsm/linux-lts/linux-omarchy
  red; linux-firmware, linux-firmware-amdgpu, linux-api-headers,
  linux-headers, linux-omarchy-headers, pambase, sddm-kcm not),
  fixtures/README, SPEC-ENGINE §2, CHANGELOG. No fixture event changes
  zone (no linux-* subject in the fixture ledger).
- (2) 4ee08dd — `decide --json` `warnings` now carries the loader's
  warnings: an invalid ADR file is named ("decisions/ADR-0001-broken.md:
  invalid decision: …; skipped") and left out of the table; the new
  decision is still created. Test
  status::decide_names_a_skipped_decision_in_its_warnings (JSON and the
  human `warning:` line).
- (3) 4ee08dd — `dossier::Files::set` returns `Result<bool, String>`: a
  file with a damaged fence of that name is left alone and the fence
  skipped, `Err` = "system/<file>: the <fence> fence has no end marker of
  its own; fence kept". "Damaged" (`views::fence_damaged`, shared with
  `write_decisions_index`): the begin marker is there but `fence_body`
  finds no end, or the body it finds contains another begin marker (the
  end belongs to the next fence — the same data-loss path one step
  removed). `seldon dossier` reports the fence as `skipped` with the
  warning. `import omarchy-agent` (the other caller) records it as an
  error in `skipped` (path system/deviations.md) with no deviation rows,
  so `--apply` is blocked until the user repairs the fence — the import
  never writes past a damaged fence. Tests:
  dossier::tests::a_fence_without_its_end_is_skipped_not_appended (no
  end; borrowed end; the intact fence in the same file still written),
  views::tests::a_fence_is_damaged_without_an_end_of_its_own,
  tests/dossier.rs a_fence_without_its_end_is_skipped_with_a_warning,
  tests/import.rs a_deviations_fence_without_its_end_blocks_the_rows.
  SPEC-ENGINE §3 dossier: "A failed query skips its fences (warning,
  fence kept); so does a fence whose begin marker has no end marker of
  its own (WP-050: no second fence is appended; `import` reports it as an
  error)." The decisions comment now reads "a begin marker without an end
  marker of its own leaves the file alone (warning)".
- (4) SPEC-ENGINE §3: `seldon decide "<title>" [--case ID] [--no-edit]`.
- (5) llms.txt: "R2 and R3 cases with `--snapshot N` (without one it
  warns, never refuses)"; docs/AGENT-GUIDE.md §3 step 3: "For an `R2` or
  `R3` case take a snapper snapshot first and pass it: `--snapshot <N>`.
  Without one, `plan start` prints a warning (and `warnings` in `--json`)
  but never refuses (ADR-0023)."; docs/seldon-concept.html hooks:
  PreToolUse (Bash and Edit/Write/MultiEdit, classified before they run)
  and SessionEnd (once per session, not after every reply), German as the
  page is.
- Verified: `cargo clippy --all-targets -D warnings` clean; suites
  `--test dossier` 12, `--test import` 7, `--test status` 9, `--test
  index` 17, lib 131 all pass; `python3 scripts/validate-fixtures.py` ok
  (25 self-checks); `just check` exit 0 ("check: ok"). No scratch
  logbook was needed this round; scratchpad emptied.
- Left as is (not asked): AGENT-GUIDE §4 zone table ("red … and a
  snapshot first") and its do-not list still pair the snapshot with the
  red zone as a zone rule; not contradictory to ADR-0023 but softer.

Not done:
- Existing installs keep their old alwaysRed list (init writes the
  default into config.toml). No migration; CHANGELOG says to add the
  globs by hand.
- Plugin: `Model.planResult` ignores `warnings`, so a start from the
  panel shows no R2/R3 advice. Plugin follow-up if wanted (outside this
  engine WP).

Verified by:
- `just check` → exit 0, "check: ok" (fmt, clippy -D warnings, all
  engine tests, watch feature, packaging, install, schema-validate,
  plugin-validate, qmllint, plugin-test incl. real-home guard,
  overlay-view 314 passed).
- New tests: plan::start_warns_on_r2_and_r3_without_a_snapshot (R0/R1
  none; R2 text without "explicit go"; R3 with "the human's explicit go
  per step"; case still goes active; `--snapshot` and a hand-set
  snapshotBefore silence it; human `warning:` line; verify → `[]`),
  doctor::logbook_row_shows_paths_like_the_header,
  status::decide_and_status_fill_the_decisions_index (user text above and
  below kept, two decides, pipe escaped, in the ADR's own commit, status
  after a hand edit, translated head kept, second run `files: []` and same
  bytes, unterminated fence → warning and untouched, missing fence
  appended, missing file created),
  status::the_fixture_decisions_index_is_stable,
  views::decisions_index_keeps_the_head_and_writes_every_row,
  drift::always_red_globs extended.
- `seldon plan start --help` shows the `--snapshot` line quoted above;
  the SPEC synopsis now lists the same four options.
- Manual scratch run (HOME and all three XDG_* in one scratchpad dir,
  SELDON_TEST_GUARD set; doctor's config row named the scratch file
  before anything ran): init --path ~/Seldon; two `decide --no-edit`
  (the second title with a `|`) → both rows in DECISIONS.md, newest
  first; ADR-0001 set to accepted by hand; `status --json` → files
  [STATUS.md, DECISIONS.md], row shows accepted; second `status --json`
  → files [], git "nothing changed". `plan new --risk R3` + `plan start`
  → exit 0 with the R3 warning. `doctor` → "logbook  ~/Seldon · machine
  …"; `doctor --path ~/Nope` → "not initialised at ~/Nope", fix
  "seldon init --path ~/Nope". Scratch deleted afterwards; no scratch
  left under /tmp.
- The operator's ~/Seldon was never touched.

Learned: memory/rust-notes.md and memory/pitfalls.md, section
"2026-10-02 · WP-050" (table head kept in generated tables; one loader
for index and decide; a write after the record exists is a warning; a
changed config default does not reach existing installs; an unterminated
fence must stop a fence writer; alwaysRed sees package names only).

Decisions needed (first round; resolved by the review: dossier fix done
in the fix round, /etc events stay yellow — deferred ADR, login
packages added, panel warnings → plugin v0.1.1, stale docs fixed):
- /etc config events: alwaysRed cannot express `/etc` (package names
  only). If a user watches `/etc/...`, its config events are yellow
  (`zone_for`, ADR-0014 §2 makes only `~/.config/systemd/` red). Making
  `/etc` red would change ADR-0014 → needs an ADR; flagged, not done.
- Login candidates for R3 not added: `sddm`, `uwsm`, `pam`. ADR-0023's
  R3 says "boot, login or the shell", the WP's list does not name them.
  Operator's call.
- Stale lines outside my allowed files: docs/seldon-concept.html (German
  concept page) still names PostToolUse/Stop; llms.txt says "red cases
  with `--snapshot N`" (ADR-0023 ties the snapshot to risk R2/R3, not
  to the zone); docs/AGENT-GUIDE.md §3 step 3 says the same and could
  mention the warning. Left for the docs owner (WP-045 track).

Touched outside WP scope (fix round, as instructed): llms.txt,
docs/AGENT-GUIDE.md, docs/seldon-concept.html, engine/src/import/
omarchy_agent.rs (the second Files::set caller), engine/tests/{index,
import,dossier}.rs.
Touched outside WP scope (first round):
- scripts/validate-fixtures.py: its `ALWAYS_RED` mirror of the default
  updated with the new list (the fixture validator must agree with the
  engine; no fixture changed, `validate-fixtures: ok`).
- fixtures/README.md: the one line listing the alwaysRed default.
- engine/tests/{plan,doctor,status,init}.rs (tests for the above).
```
