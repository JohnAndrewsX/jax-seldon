# WP-193 — Handover

Branch `wp/193-highlights` (from `next` at `340954d9`), worktree `wt/WP-193`.
`release.yml` is unchanged, as the WP says.

## What was done

- **`packaging/release-notes.sh X.Y.Z`** — the section extraction is
  unchanged. Before 0.2.0 it prints the whole section as before. From
  0.2.0 (`major > 0 || minor >= 2`, so `0.10.0` counts) it prints:
  1. the standing paragraph, meaning whatever stands before
     `### Highlights`;
  2. `### Highlights` with its bullets (blank lines between bullets
     dropped);
  3. one line, `Every change in X.Y.Z: [CHANGELOG.md](<repo>/blob/vX.Y.Z/CHANGELOG.md#<anchor>)`.

  It exits 1, with a reason naming the version, when:
  - the section has no `### Highlights`;
  - `### Highlights` is not the section's first `###` part;
  - it has no bullets, or more than ten;
  - a line continues a bullet (wrapped or nested);
  - a bullet names `WP-<n>`;
  - a line is not a `- ` bullet (prose, `*`);
  - a bullet is empty;
  - the link reference `[X.Y.Z]: https://…/releases/tag/vX.Y.Z` is
    missing, or present but not `https` or pointing at another tag.

  `<repo>` comes from that link reference (VERSIONING.md already requires
  it before a tag), so the synthetic test changelogs do not claim the
  real repository. `<anchor>` is GitHub's slug of the heading: lower
  case, punctuation removed, spaces become `-` (`020---2026-10-14`, or
  `020` without a date).
- **`tests/release/release-notes.test.sh`** — 33 cases:
  - All old cases are kept. The synthetic sections they use were renamed
    to 0.1.x, because 1.x sections now need Highlights.
  - New: the real 0.1.4 section, compared whole with an independent
    oracle (`sed` cut between the headings, blank ends dropped).
  - New: the real `[Unreleased]` opens with `### Highlights`.
  - New Highlights bodies: with the standing paragraph and a date,
    without both, `0.10.0`, and the `0.2.0` boundary. A 0.1.x section
    that has a `### Highlights` with a wrapped bullet still prints whole.
  - New refusals, each one change against a passing ten-bullet base, each
    asserting its own message: missing, late, no bullet, eleven, wrapped,
    nested, WP number, prose, `*` bullet, empty bullet, no link
    reference, `http`, wrong tag.
- **`CHANGELOG.md`**:
  - The convention is in the header note.
  - `[Unreleased]` has an empty `### Highlights` for WP-126.
  - One Docs bullet for this change.
  - The standing paragraph for plugin 0.1.0 users is back at the top of
    `[Unreleased]`. VERSIONING.md says it stays there after a release,
    but it was not copied back when 0.1.4 was cut. It is copied verbatim
    from 0.1.4. The AUR package still does not exist (STATUS.md), so the
    paragraph still applies.
- **`docs/VERSIONING.md`** — a new part, "Highlights", under
  "CHANGELOG.md":
  - the rule (first part, 1–10 one-line `- ` bullets, user words, no WP
    numbers, Breaking items included);
  - `[Unreleased]` keeps the empty heading;
  - who writes the points: the release WP (WP-126 for 0.2.0, WP-188 for
    0.3.0), and the operator reads them with the tag question;
  - an example section and the body the script makes of it.

  The example was checked against the script: its first block, plus a
  link reference, gives exactly its second block. "Before a tag" items
  1–3 and tag-flow step 2 now say what the release body is and what
  `[Unreleased]` keeps. `packaging/README.md` (script row, `build` and
  `release` rows) and the TESTING.md packaging row follow.

Commits: `b854ecbc` (script, test, CHANGELOG), `b333997f` (docs).

## Decisions I took inside the WP (please confirm in review)

1. **WP numbers are refused, not only discouraged.** The WP lists three
   exit-1 conditions. "No WP numbers" is one more rule of the convention,
   and checking it costs one line. Remove it if the reviewer prefers the
   WP's literal list.
2. **Highlights must be the first `###` part**, not just present. This is
   how "opens with" is enforced. Anything before it counts as the
   standing paragraph and is printed as-is. The script cannot tell that
   paragraph from other prose, so the rule says nothing else goes there.
3. **The link reference is now required from 0.2.0.** VERSIONING.md
   already required it before a tag, so no valid release is newly
   refused. The repository URL is not hard-coded in the script.
4. **The standing paragraph was restored to `[Unreleased]`** (see above).
   WP-126 removes it again if the AUR package exists by then.

## Not done

- `release.yml` comments still say "the `## [X.Y.Z]` section … is the
  release body". This is still loosely true (the body is made from the
  section), and the WP keeps the file read-only. One line for WP-126 or
  WP-188 if wanted.
- The 0.2.0 points themselves (WP-126) and the 0.3.0 points (WP-188).

## How it was verified

| Check | Where | Result |
|---|---|---|
| `bash tests/release/release-notes.test.sh` | fixture | 33 ok, `release-notes: ok` |
| 14 mutants of `release-notes.sh` against that test, one per rule, plus the version gate, the 0.1.x path, the anchor and the link: each killed by the case it is about | fixture | 14/14 killed |
| `[Unreleased]` check: the real CHANGELOG with its `### Highlights` removed | fixture | killed |
| `release-notes.sh 0.1.0` … `0.1.4`: byte-identical (`cmp`) to the output of `next`'s script, captured before the change | fixture | 5/5 identical |
| VERSIONING.md example: script output equals the documented body | fixture | identical |
| all `tests/release/*.test.sh` (7 files) | fixture | all exit 0 |
| `SELDON_FULL_CHECK=1 just check`, run 1 | fixture, headless | **failed** in `test`: `doctor::an_empty_dot_git_is_degraded_with_its_fix` panicked at `engine/tests/doctor.rs:599` (`remove_dir_all(.git)`: `Directory not empty`). runtime-space, fmt-check and clippy were clean before it. `just` stopped there. |
| that doctor test alone, 10 times | fixture | 10/10 ok |
| `SELDON_FULL_CHECK=1 just test check-watch check-packaging check-install check-deploy check-guard check-runtime-dir schema-validate docs-check plugin-validate qmllint plugin-test` (run 2: everything after `clippy`) | fixture, headless | exit 0: engine tests ok (the doctor test included), `check-packaging: ok` (`release-notes: ok`), install 229/0, runtime-dir 45/0, guard mutants 85 killed, docs-check ok (477 links), plugin-validate ok, qmllint ok (51 files), plugin-test ok with the Quickshell harnesses run (not skipped) |
| shellcheck on the script and the test | not run (not installed locally; CI runs it in `check-packaging`) | — |

All runs used `TMPDIR` and a private 0700 `XDG_RUNTIME_DIR` on disk,
under the private gates folder. The cargo target was the worktree's
`engine/target`. No network. The real `~/Seldon`,
`~/.local/state/seldon` and `~/.config` were not touched.

## Open questions

- **The doctor flake** (not this WP's code; `engine/` is unchanged). The
  test removes the logbook's `.git` right after `init`. Under the load of
  the full parallel run, something still wrote into `.git` while it was
  being removed. My guess is a git process that `init`'s autocommit
  started; this is not verified. It passed in run 2 and 10/10 alone.
  Worth a small engine WP: wait for the child, or retry the removal in
  the test.
