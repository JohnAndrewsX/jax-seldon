```
WP-093 HANDOVER
```

Branch `wp/093-redact-email`, base `f07d5d1` (main with WP-087 and
WP-089 merged). Commits: `8219e5b` (rule, rows, tests), `ed88882`,
`1e0ac67` (rows that killed surviving mutants), `a9f47f4` (timing
rows), `455b992` (SPEC-ENGINE §7, module doc), `4b5a87b` / `21defff`
(guide 06 en/de), `fb4bf2c` (CHANGELOG), `164db42` (skipPaths example
tested), `3800c9d` (pitfalls), plus this file.

## Done

1. **Rule `email`** (`redact::BUILTIN`, last of 24, trigger `@`). It
   masks the local part and **keeps the domain**:
   `me@example.com` → `‹redacted›@example.com`.
   - **Why keep the domain.** The WP asks that a subject stay
     recognisable enough to find the file. A web app's desktop entry
     `Mail (me@example.com).desktop` becomes
     `Mail (‹redacted›@example.com).desktop`. If both parts were masked,
     `bob.smith+web@example.org.desktop` would become
     `‹redacted›.desktop`, which no user can map back to a file. The
     domain also tells a work account from a private one. This matches
     `url-userinfo`, which keeps the host. Two files whose names differ
     only in the local part then share a subject; see Decisions 2.
   - **Address.** The local part is ASCII letters, digits and `._%+-`,
     plus any character beyond ASCII except the marker's quotes
     U+2039/U+203A. Then `@`, then a domain of at least two labels;
     the last label is letters only and at least two long (`example.de`,
     `müller.example`).
2. **Not masked, as the WP asks** (CLEAR rows for each):
   - `user@host` without a dot (`ssh me@localhost`);
   - versions and npm scopes (`@scope/pkg`, `@scope/other@1.2.3`,
     `left-pad@1.3.0`, `react@18.2.0-rc.1`, `typescript@5.4.10`,
     `typescript@latest`);
   - SSH remotes and `host:path` (`git@github.com:example/x.git`,
     `rsync … me@host.example:/srv/www`);
   - also: systemd units (`getty@tty1.service`, `wg-quick@wg0.service`,
     `app@x.timer`, `.socket`, `user@1000.slice`, …), `2560x1440@144`,
     `alpine@sha256:…`.
3. **`Rule::unless`** (new field). The `regex` crate has no look-around,
   so the pattern matches the context as a named group (`url`, `port`,
   `unit`), and the rule leaves a match in which one of them takes part.
   Leftmost-first matching makes a match that starts at the context
   win, so the address inside it is never found on its own.
   - `url` reads a URL's userinfo with the same grammar as
     `url-userinfo`, including a password that holds `/` and `@`. An
     `@` up to which `url-userinfo` masks is therefore not an address,
     and the import report counts the line once.
   - `port` is a `:` after the domain followed by a character other
     than white space. `Reply to ivan@example.com: thanks` is still
     masked.
   - `unit` is a last label that is a systemd unit type, followed by a
     word end. `judy@example.services` (a real TLD) is masked.
4. **Tests** (`engine/tests/redaction.rs`):
   - 13 TABLE rows for `email`: the desktop-entry subject, `+` and `.`
     in the local part, `git config user.email`, a German sentence with
     a `.de` domain, `mailto:`, two addresses separated by a comma,
     `jürgen@müller.example`, an address in a URL query, `: thanks`,
     `.services`, an address after an SSH remote, an address after a
     unit, and `ssh -p 2222 me@host.example`;
   - one `url-userinfo` row with `/` and `@` in the password;
   - 11 CLEAR rows;
   - `email_rule_is_disjoint_and_stable`: `email` matches only its own
     rows and no row of any other rule. A masked text matches `email`
     no more, a second pass changes nothing, and an address next to a
     URL with userinfo is masked as well;
   - `commands::a_desktop_entry_named_after_an_address_is_masked`, end
     to end through `seldon capture --source config`. Two entries,
     `alice.webapp@` and `bob.webapp@`, have the same masked name. The
     test covers add, change and remove, and a second capture writes
     0 events each time. The ledger, the index and every logbook file
     hold no local part. The manifest keeps the real names. Guide 06's
     `"*@*.desktop"` matches both entries and not `Zoom.desktop`.
   - Timing rows "addresses" and "at signs" (16/64/128 KB, budget
     20 ms at 128 KB).
5. **Changed existing rows** (look at these):
   - The three `sshpass` TABLE rows and the CLEAR row `ssh -p 2222
     me@host.example` now use `me@host`, because a dotted SSH login now
     reads as an address.
   - What those rows test (the `-p` mask, the port that stays) is
     unchanged. A new `email` row shows `ssh -p 2222 ‹redacted›@host.example`.
6. **Docs.**
   - SPEC-ENGINE §7: one rule-list entry and one paragraph.
   - Module doc of `redact.rs`.
   - Guide 06 en/de: a bullet in the rule list, plus the paragraph the
     WP asks for. It says that file names under watched paths become
     subjects and how `"*@*.desktop"` in `skipPaths` keeps such a file
     out. The German source line is updated.
   - CHANGELOG `[Unreleased] / Engine`.

## Not done

- **No look at hook.rs, the index rebuild, capture.rs, doctor.rs or
  config.rs.** None were changed.
- **Known over-masking, accepted and documented:**
  - `ssh me@host.example` (it cannot be told from an address);
  - HiDPI asset names such as `icon@2x.png`, where `png` reads as a
    top-level domain (no row);
  - a non-ASCII punctuation mark glued to the local part, such as
    `„me@…` or `Kontakt—me@…`: it is masked with it.
- **Not masked:**
  - an address directly followed by `:` and text (`me@example.com:x`);
  - a domain whose last label is a systemd unit type;
  - an address in an IDN or punycode TLD whose last label holds
    digits (`xn--p1ai`). It is masked up to `xn`, so the local part is
    still masked.
- **Ledger lines written before 0.1.4** keep their unmasked subjects,
  as SPEC §7 says for every new rule.

## Verified by

- `cargo fmt --check` and `cargo clippy --all-targets -- -D warnings`
  are clean.
- `cargo test --locked --no-fail-fast`: 33 binaries, 0 failed.
  `redaction`: 21 passed, 1 ignored.
- `scripts/docs-check.sh`: ok.
- **`flock /tmp/seldon-check.lock just check` at `3800c9d`: exit 0**
  (16:57–17:08). Results: service-states 314/0, panel-view 782/0,
  overlay-view 319/0, bar-view 143/0, install 209/0, real-home-guard
  11/0, qmllint 29 files, docs-check ok, `check: ok`. The commits after
  `3800c9d` add only this file.

### Mutants (each run of `--test redaction --no-fail-fast` on the final state `3800c9d`; source restored with `git checkout HEAD` + `touch`; baseline green afterwards, 21 passed; `git status` clean)

| # | Mutant | Caught by |
|---|---|---|
| E1 | `unless` without `url` | `email_rule_is_disjoint_and_stable` |
| E2 | `unless` without `port` | `…disjoint…` (email and WP-084), `every_builtin_pattern`, `harmless_text_stays` |
| E3 | `unless` without `unit` | `every_builtin_pattern`, `harmless_text_stays` |
| E4 | `applies` ignores `unless` | `…disjoint…` (both), `every_builtin_pattern`, `harmless_text_stays` |
| E5 | domain needs no dot | same four |
| E6 | top-level domain may hold digits | `harmless_text_stays` (`typescript@5.4.10`, row `1e0ac67`; survived before it) |
| E7 | local part ASCII only | `every_builtin_pattern` (`jürgen`) |
| E8 | domain labels ASCII only | `email_rule_…`, `every_builtin_pattern` |
| E9 | the marker's quotes count as address characters | `email_rule_…` (a masked text matches no more) |
| E10 | the domain is masked too | `a_desktop_entry_…`, `email_rule_…`, `every_builtin_pattern`, `user_patterns` |
| E11 | trigger `mailto` instead of `@` | `a_desktop_entry_…`, `email_rule_…`, `every_builtin_pattern`, `every_row_holds_a_trigger_of_its_rule` |
| E12 | URL skip without the `user:password` branch | `email_rule_…` (`https://bob:fake/pw1@…`) |
| E13 | URL skip: password without `@` | `email_rule_…` (row `1e0ac67`, `fake/pw@5`; survived before it) |
| E15 | unit without its word end | `email_rule_…`, `every_builtin_pattern` (`.services`) |
| E16 | `port` = a bare `:` | `email_rule_…`, `every_builtin_pattern` (`: thanks`) |
| E19 | local part without `+` | `every_builtin_pattern` |
| E20 | local part without `.` | `a_desktop_entry_…`, `every_builtin_pattern` |
| E21 | top-level domain of 3 or more letters | `email_rule_…`, `every_builtin_pattern` (`.de`) |
| E14 | URL skip without `(?-u:\b)` before the scheme | **survives** |

E14 is equivalent except when a `_` stands directly before a scheme
(`x_https://u:p@h.example`). `url-userinfo` has the same boundary and
does not mask there. With the boundary, `email` masks `p` as a local
part; without it, nothing is masked. So the boundary is the safer
reading, but that case has no row, because it would enshrine a
`url-userinfo` gap.

### Timings (bench profile, median of 21)

Interleaved A/B: bench builds of `main`'s `redact.rs` and this branch's,
for both the `redaction` test binary and `seldon`. Each was swapped into
`target/release/seldon` in turn and run under the check lock, 3 rounds,
alternating order (WP-087 method).

Rounds 1 and 3 ran `main` first, round 2 ran the branch first. In round
2, `main`'s redaction run went about ×1.8 slower on every row (load).

| Row (budget) | main r1 / r2 / r3 | WP-093 r1 / r2 / r3 |
|---|---|---|
| url line, 64 KB (2 ms; holds an `@`) | 0.585 / 0.757 / 0.551 ms | 0.629 / 0.631 / 0.642 ms |
| german note, 64 KB (2 ms) | 0.560 / 0.719 / 0.530 ms | 0.549 / 0.516 / 0.550 ms |
| quoted line, 64 KB (2 ms) | 0.572 / 0.750 / 0.540 ms | 0.541 / 0.553 / 0.559 ms |
| apostrophes, 64 KB (2 ms) | 0.550 / 0.742 / 0.541 ms | 0.540 / 0.518 / 0.532 ms |
| two option kinds, 128 KB (20 ms) | 8.41 / 15.5 / 8.37 ms | 8.63 / 8.20 / 8.45 ms |
| five option kinds, 128 KB (–) | 13.3 / 25.2 / 13.4 ms | 13.7 / 13.1 / 13.2 ms |
| password and token, 128 KB (10 ms) | 4.05 / 7.66 / 3.70 ms | 4.06 / 3.93 / 4.00 ms |
| addresses, 16 / 64 / 128 KB (20 ms at 128) ¹ | 0.05 / 0.22 / 0.52 ms (r1) | 0.59 / 2.51 / 4.82 ms (r1); 128 KB: 4.73, 4.72 |
| at signs, 16 / 64 / 128 KB (20 ms at 128) ¹ | 0.16 / 0.61 / 1.23 ms (r1) | 0.38 / 1.52 / 3.09 ms (r1); 128 KB: 3.02, 3.01 |
| hook, recorded, 10 000 lines (5 ms) | 1.81 / 1.77 / 1.83 ms | 1.81 / 1.75 / 1.75 ms |
| hook, curl line with a marker, 10 000 lines (5 ms) | 2.70 / 2.66 / 2.59 ms | 2.89 / 2.83 / 2.88 ms |
| hook, recorded, 900 lines (5 ms) | 4.01 / 3.90 / 3.48 ms | 3.71 / **9.18, 12.7** ² / 3.61 ms |
| hook, curl line with a marker, 900 lines (5 ms) | **5.05**, 4.99 / 4.71 / 4.56 ms | 4.86 / – ² / 4.88 ms |

The 16 KB rows were measured separately, alternating
`main, wp, wp, main, main, wp`, each under the lock (after the check):

| Row (1 ms) | main | WP-093 |
|---|---|---|
| url line, 16 KB (holds an `@`) | 0.132 / 0.132 / 0.132 ms | 0.153 / 0.150 / 0.147 ms |
| german note, 16 KB | 0.130 / 0.125 / 0.130 ms | 0.130 / 0.128 / 0.125 ms |

The addresses row is linear: ×2 per doubling from 64 to 128 KB,
×4.3 from 16 to 64 KB (about 0.5 µs per address: 9 000 addresses at 128 KB).

¹ `main` has no `email` rule, so these rows mask nothing there.
² A load spike. That payload is `yay -S --noconfirm zed`, with no
`@`: `email` is not triggered, and the code path is identical to
`main`'s. The test stopped there, so the curl case of that round did
not run. `main` itself needed its retry in round 1 (5.05 ms).

**What it costs.** Any line with an `@` compiles `email` once per
process: compile plus first match takes 183 µs (`url-userinfo`:
99 µs; throwaway test, median of 101). Of that, 46 µs is the URL skip
group. The hook's curl line holds an `@` (in its URL), so it pays
about +0.2 ms. That shows at 10 000 lines (+0.2 ms) and leaves about
0.1 ms of headroom to 5 ms at 900 lines (4.86 / 4.88 ms). An
ASCII-whitespace class in the skip group saves only 20 µs, and the
skip would then no longer read URLs as `url-userinfo` does. Not done.

A line without an `@` costs nothing (german note, quoted line,
apostrophes, option rows: unchanged). A line with one pays the search
once more: url line +0.02 ms at 16 KB and +0.05 to +0.09 ms at 64 KB.

## Learned

Appended to `memory/pitfalls.md`:

- emulating look-behind by matching the context, then keeping the
  match;
- a dotted `user@host` in another rule's row now matches `email`;
- a shell chain split after a heredoc does not short-circuit. It caused
  a commit of the old state, which was amended before the next commit.

## Decisions needed

1. **Domain kept** (above). The privacy stage should confirm it.
   Against it: a personal vanity domain still identifies its owner.
   Such a user can add a pattern or a `skipPaths` entry, and guide 06
   says how.
2. **Same masked name, different files.**
   - `config.rs` `replay` maps redacted subject → path in a
     `BTreeMap`. Two files whose names differ only in the local part
     map to one entry, the last key.
   - This only matters after a failed cursor save, and it is not new:
     any redaction in a file name has it. The rule makes it more
     likely.
   - The normal path is covered by the end-to-end test: add, change
     and remove next to a twin.
   - Not fixed: `config.rs` is outside my scope. A follow-up could map
     a subject to every path with that subject and choose by `hashFrom`.
3. **Hook headroom at 900 lines** is down to about 0.1 ms on a line
   with an `@`. The lever named in WP-084 (narrower triggers, its open
   question 1) still applies. Alternatives: drop the URL skip group and
   accept that `url-userinfo` and `email` both count a line like
   `https://u:p@h.example` in the import report (−46 µs), or compile
   the rule lazily per `@` with a dot after it (no gain for that line).
4. **Existing ledgers.** Subjects recorded before the upgrade stay
   unmasked, so after the upgrade a desktop entry's events carry two
   spellings. The index rebuild (WP-092) and drift pairing by subject
   may show them as two files. I did not look at the index side.

## Touched outside WP scope

- `memory/pitfalls.md` (append).
- No guard-hook blocks. Nothing outside the repository.
- Scratch scripts and binaries stayed in the session scratchpad.
  `engine/target/release/seldon` was swapped during the A/B run, and
  the bench build of this branch is back in place.
- Two throwaway test files, created and deleted in the same command,
  never committed.

## Round 2 (review: stage 1 SEND BACK)

Commits:

- `74a0a96`: B1, B2, the N1 rows and the personal-domain pattern test.
- `7744683`: the domain class and a full-width colon row.
- `fea31a4`: a row for an address before `host:path`.
- `22dd08f`: a timing row.
- `5ae9b90`: SPEC-ENGINE §7.
- `d9ea49f` / `967eaf5`: guide 06 en/de.
- `f58247b`: CHANGELOG.
- This section.

### B1: text glued to an address stays

- **Local part (`LOCAL_NON_ASCII`).** It is ASCII letters, digits and
  `._%+-`, plus `\x{A1}-\x{1FFF}\x{2070}-\x{2FFF}\x{A000}-\x{F8FF}
  \x{FB00}-\x{FEFF}\x{10000}-\x{10FFFF}`, the reviewer's ranges. Left
  out are C1 and NBSP, general punctuation (U+2000–U+206F, the marker
  quotes included), CJK (U+3000–U+9FFF, U+F900–U+FAFF) and full-width
  forms (U+FF00–U+FFEF).
  - Rows: a Japanese sentence glued to an address, `（連絡先：` with a
    full-width colon right before the address, `—` and `„…“`. In each,
    only the address is masked.
  - `jürgen@` keeps working.
  - A local part written in CJK is not matched any more (SPEC lists it
    under "not recognised").
- **Domain (`NON_ASCII`).** It is now any character beyond ASCII,
  `\x{80}-\x{10FFFF}`. The marker-quote exclusion in the domain was dead
  code: the domain is copied unchanged, and the round-2 mutant E9 that
  allowed the quotes survived every test. Allowing them only adds
  matches; for example `me@‹redacted›.example` now masks `me`.
  - IDN top-level domains (`.испытание`, `.テスト`) stay matched.
  - A domain followed by CJK text keeps that text inside the kept
    domain. It is not masked.
- My round-1 "Not done" note ("a non-ASCII punctuation mark glued to
  the local part") understated this: a whole CJK sentence was lost.
  That is fixed now.

### B2: the `:` takes nothing from what follows

- **The group holds only the `:`.** `port` is now `(?P<port>:)?` and
  moved from `unless` to a new field, `unless_followed`.
- **`Rule::kept` decides from what follows the match.** The `:` keeps
  the match only when two things hold:
  - the next character is not white space and not the end of the text;
  - the rule's next match does not start right there. The list from
    `Rule::matches` already has it, so no second regex and no extra
    search are needed.
- **Callers.** `applies` gets the text and the next match.
  `replace` and `matching_rules` pass them.
- **The `:` is restored.** The replacement is now
  `‹redacted›@${domain}${port}`, so a masked address keeps its `:`.
- **How I read "both addresses masked fully".** In
  `a@b.co:c@d.example` and `me@example.org:me2@example.net` *both*
  addresses are masked:
  - `‹redacted›@b.co:‹redacted›@d.example`;
  - `‹redacted›@example.org:‹redacted›@example.net`.
  The two fixes the packet suggests would have kept the first address
  of each line, since a non-space follows it. Masking both covers either
  reading, and a `host:path` never starts with an address.
- **Rows:**
  - both lines above;
  - `an rita@example.org:` (a `:` at the end is masked);
  - `a@b.co:git@h.example:o/r`: the first address is masked and the
    `host:path` after it stays. This row kills K6.
  - The old rows `Reply to ivan@example.com: thanks` and
    `git@github.com:…` / `rsync … :/srv` are unchanged.

### N1, N2, guide

- **N1.**
  - TABLE row `olga@пример.испытание` (kills R2).
  - `harmless_text_stays` loops over all 11 unit types,
    `systemctl status app@x.<unit>` (kills R3).
- **N2.**
  - SPEC §7 gains a "not recognised" line: `me%40example.org`,
    `"me"@example.org`, a CJK local part.
  - SPEC §7 also describes the new local-part ranges and the `:` rule.
  - CHANGELOG: "in the logbook and the index".
- **Guide 06 en/de.** One sentence on a personal domain:
  `"@smith\\.example\\b"` turns `jo@smith.example` into
  `‹redacted›‹redacted›`. `user_patterns` asserts exactly that, next to
  an `@example.org` address that keeps its domain.
- **Decisions from the brief.** Taken as given, nothing to change:
  - the domain stays;
  - a Mastodon handle stays masked;
  - the replay collision becomes a follow-up WP;
  - old subjects are documented only;
  - merge after WP-092.

### Mutants (round 2; each run of `--test redaction --no-fail-fast`, the whole list at `7744683`, K1, K6 and K7 again at `fea31a4` after the row that killed K6; source restored with `git checkout HEAD` + `touch`; baseline green afterwards, 21 passed; `git status` clean)

Round-1 mutants are adapted to the new code (E9 is gone with the dead
exclusion):

| # | Mutant | Caught by |
|---|---|---|
| E1 | `unless` without `url` | `email_rule_is_disjoint_and_stable` |
| E2 | no `:` context (`unless_followed` empty) | `email_rule_…`, `every_builtin_pattern`, `harmless_text_stays`, `proxy_json_…` |
| E3 | `unless` without `unit` | `every_builtin_pattern`, `harmless_text_stays` |
| E4 | `applies` ignores `kept()` | `email_rule_…`, `every_builtin_pattern`, `harmless_text_stays`, `proxy_json_…` |
| E5 | domain needs no dot | same four |
| E6 | top-level domain may hold digits | `harmless_text_stays` |
| E7 | local part ASCII only | `every_builtin_pattern` |
| E8 | domain labels ASCII only | `email_rule_…`, `every_builtin_pattern` |
| E10 | domain masked too | `a_desktop_entry_…`, `email_rule_…`, `every_builtin_pattern`, `user_patterns` |
| E11 | trigger `mailto` | `a_desktop_entry_…`, `email_rule_…`, `every_builtin_pattern`, `every_row_holds_a_trigger_…`, `user_patterns` |
| E12 | URL skip without the `user:password` branch | `email_rule_…` |
| E13 | URL skip: password without `@` | `email_rule_…` |
| E15 | unit without its word end | `email_rule_…`, `every_builtin_pattern` |
| E19 / E20 | local part without `+` / `.` | `every_builtin_pattern` (E20 also `a_desktop_entry_…`) |
| E21 | top-level domain of 3 or more | `email_rule_…`, `every_builtin_pattern` |
| E14 | URL skip without `(?-u:\b)` | **survives**, as in round 1 (equivalent except after `_`) |

The reviewer's mutants and the new ones:

| # | Mutant | Caught by |
|---|---|---|
| R1 | `port` = `:\S+` | `email_rule_…`, `every_builtin_pattern`, `harmless_text_stays`, `masking_twice…`, `proxy_json_…` |
| R1b | `port` = `:\S` (round 1) | `email_rule_…`, `every_builtin_pattern` |
| R2 | top-level domain ASCII only | `email_rule_…`, `every_builtin_pattern` (`.испытание`) |
| R3 | unit list without `device` | `harmless_text_stays` (unit loop) |
| B1 | local part takes all non-ASCII (round 1) | `email_rule_…`, `every_builtin_pattern` |
| B1b | local part takes general punctuation | `every_builtin_pattern` (`—`, `„`) |
| B1c | local part takes CJK | `every_builtin_pattern` |
| B1d | local part takes full-width forms | `every_builtin_pattern` (`：`, row `7744683`; it survived the first row, in which hiragana stood before the address) |
| K1 | a following match does not end the `:` context | `email_rule_…`, `every_builtin_pattern` |
| K2 | white space after the `:` keeps it too | `email_rule_…`, `every_builtin_pattern` |
| K3 | the end of the text keeps it | `email_rule_…`, `every_builtin_pattern` |
| K4 | the replacement drops the `:` | `every_builtin_pattern` |
| K5 | `port` back in `unless` (kept whatever follows) | `email_rule_…`, `every_builtin_pattern` |
| K6 | `matching_rules` without the next match | `email_rule_…`, `every_builtin_pattern` (row `fea31a4`; it survived before) |
| K7 | `replace` without the next match | `email_rule_…`, `every_builtin_pattern` |

**Pre-fix run.** The new rows against round 1's `redact.rs` make
`every_builtin_pattern` and `email_rule_is_disjoint_and_stable` fail.
The first was the CJK row: `…改行なしnana@example.com` →
`‹redacted›@example.com`.

### Timings (round 2; bench profile, median of 21)

**Method.** As in round 1: interleaved A/B of `main`'s `redact.rs`
(unchanged since the base) against `22dd08f`. Each run was under the
check lock, 3 rounds, in the order main·wp | wp·main | main·wp.
Between runs the lock passed to other sessions' checks. Round 1's
`main` started at load 14, every other run at 0.3–0.7. No run failed
and no budget needed a retry.

| Row (budget) | main r1 / r2 / r3 | WP-093 r1 / r2 / r3 |
|---|---|---|
| url line, 16 KB (1 ms; holds an `@`) | 0.133 / 0.129 / 0.132 ms | 0.150 / 0.149 / 0.141 ms |
| url line, 64 KB (2 ms) | 0.580 / 0.561 / 0.584 ms | 0.638 / 0.627 / 0.615 ms |
| german note, 64 KB (2 ms) | 0.563 / 0.509 / 0.568 ms | 0.562 / 0.524 / 0.536 ms |
| quoted line, 64 KB (2 ms) | 0.531 / 0.530 / 0.551 ms | 0.564 / 0.531 / 0.517 ms |
| two option kinds, 128 KB (20 ms) | 7.68 / 7.93 / 8.35 ms | 8.62 / 8.20 / 7.98 ms |
| password and token, 128 KB (10 ms) | 3.71 / 3.76 / 3.77 ms | 3.93 / 3.80 / 3.78 ms |
| addresses, 16 / 64 / 128 KB (20 ms at 128) | 0.05 / 0.21 / 0.43 ms ¹ | 0.60 / 2.44 / 4.93; 4.69, 4.77 ms |
| at signs, 16 / 64 / 128 KB (20 ms at 128) | 0.13 / 0.52 / 1.06 ms ¹ | 0.38 / 1.50 / 2.99; 3.07, 2.92 ms |
| address colons, 16 / 64 / 128 KB (20 ms at 128, new) | 0.05 / 0.20 / 0.42 ms ¹ | 0.66 / 2.63 / 5.45; 5.45, 5.51 ms |
| hook, recorded, 10 000 lines (5 ms) | 1.83 / 1.82 / 1.77 ms | 1.83 / 1.84 / 1.75 ms |
| hook, curl line with a marker, 10 000 lines (5 ms) | 2.78 / 2.83 / 2.69 ms | 2.91 / 3.01 / 2.85 ms |
| hook, recorded, 900 lines (5 ms) | 3.69 / 3.78 / 3.57 ms | 3.63 / 3.73 / 3.51 ms |
| hook, curl line with a marker, 900 lines (5 ms) | 4.65 / 4.86 / 4.54 ms | 4.85 / 4.96 / 4.77 ms |

¹ `main` has no `email` rule, so these rows mask nothing there.

**Reading.**
- **Lines without an `@`:** unchanged, within noise.
- **The hook's curl line** holds an `@` and pays the compile once:
  - +0.12 to +0.18 ms at 10 000 lines;
  - +0.10 to +0.23 ms at 900 lines;
  - headroom to 5 ms is 0.04–0.23 ms. That matches round 1 and the
    reviewer's N3. B1 and B2 did not change the cost; the reviewer
    measured the class itself at the same compile cost.
- **"address colons"** is linear: ×2.0 per doubling from 64 to 128 KB.
  Per address it costs what the "addresses" row costs, about 0.51 µs:
  - about 10 600 addresses at 128 KB in 5.45 ms;
  - against about 9 400 in 4.69–4.93 ms.
  So the `:` check (one look at the next character and at the next
  match) adds nothing measurable.
- **WP-092 order.** The orchestrator's decision stands: WP-093 merges
  after WP-092.

### Verified (round 2)

- `cargo fmt --check` and `cargo clippy --all-targets -- -D warnings`
  are clean.
- `redaction`: 21 passed, 1 ignored. The bench run passes every budget.
- `scripts/docs-check.sh`: ok. The German source line points to
  `d9ea49f`.
- **`flock /tmp/seldon-check.lock just check` at `22dd08f`: exit 0**
  (19:45–19:53). Results: service-states 314/0, panel-view 782/0,
  overlay-view 319/0, bar-view 143/0, install 209/0, real-home-guard
  11/0, qmllint 29 files, docs-check ok, `check: ok`. The commit after
  `22dd08f` adds only this section.

### Touched outside scope (round 2)

- None. No guard-hook blocks.
- Scratch scripts, binaries and the two throwaway timing tests of
  round 1 stayed in the session scratchpad or were deleted unseen by
  git.
- `engine/target/release/seldon` was swapped during the A/B run; the
  branch's bench build is back in place.
