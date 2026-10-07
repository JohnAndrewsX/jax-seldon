# WP-127 — plan

Branch `wp/127-index-details` from `next` (109d02eb), merges into `next`.
Basis: ADR-0035 §6 (optional fields within contract 2 before 0.2.0 is
tagged), CONTRACT.md rule 9.

## Steps

1. **ADR-0038 (proposed):** four optional index fields under ADR-0035 §6,
   and the optional frontmatter key `source` of an imported case.
2. **Schema:** `drift[].rule` (rule id, ≤ 64), `cases[].intent`,
   `cases[].result` (≤ 256), `cases[].source` (`~/…`, ≤ 512, no control,
   bidi or format characters; also the frontmatter key),
   `decisions[].lead` (≤ 256). `contractVersion` stays 2.
3. **Engine:**
   - `DriftItem.rule` from the classification `drift show` reports
     (`attention-all` under `attention = "all"`).
   - `load`: the first paragraph of a case's `## Intent` and `## Result`
     and of a decision's `## Decision` (comments left out).
   - `build`: each text through the logbook's redaction, control
     characters other than line breaks and tabs as spaces, clipped as
     ADR-0025 with the marker `… (N more characters in the file)`; an
     invalid `[redaction] patterns` withholds the four fields.
   - `Case.source`: optional frontmatter key, written by `import task`;
     the index copies it when it is still a clean `~/` path of at most 512
     characters (after redaction), else drops it with a build warning.
4. **Fixtures:** C-2026-007 becomes an imported case (tag `imported`,
   `source`, provenance line); `index.sample.json`, the attention-all
   index and the variants regenerated; `validate-fixtures.py` derives the
   new fields.
5. **Plugin:** `driftRuleInfo` reads `rule` from the index item first (no
   `drift show` then); Work's case detail shows Intent and Result, the
   Decisions detail the lead, as plain text; *Open in editor* stays.
6. **Docs:** CONTRACT.md rule 9, SPEC-ENGINE §6 (index fields),
   SPEC-LOGBOOK §3 (`source`), SPEC-PLUGIN where the details are named,
   CHANGELOG.
7. **Tests:** engine (fields, absence, clipping, redaction, source
   checks, import writes `source`, rule equals `drift show`), plugin model
   tests (rule from the index, fields absent), desk harness (no `drift
   show` when the rule is in the index; the old path on an index without
   it; the details), mutants in a separate target.
8. `flock /tmp/seldon-check.lock just check`, HANDOVER.md.
