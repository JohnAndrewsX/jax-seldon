```
WP-047 HANDOVER
Done:
- llms.txt at the repository root, llmstxt.org structure: H1 title,
  blockquote summary, two plain paragraphs (the two parts, how to call
  seldon, exit codes, raw-URL hint), then H2 sections whose items are all
  `- [name](url): notes`: Agent docs, Read-only commands, Writing commands
  (and when), Forbidden (never edit the ledger, never delete the logbook,
  never run the engine against another logbook, no change outside a case,
  no hidden drift or secrets, user-only commands), Specs, Human docs,
  Optional. Links are absolute GitHub blob URLs (anchors work there).
- Decision (WP asked to decide and state it): no llms.txt in the plugin
  split. The convention is one file per site root; the split is
  `git subtree split --prefix=plugin`, so a copy would have to live in
  plugin/ and would also land in every installed plugin folder. The
  plugin README gets a "For AI agents" section with absolute links to
  this repo's llms.txt and AGENT-GUIDE instead. Stated in the docs commit.
- docs/AGENT-GUIDE.md: where the rules live (the logbook's AGENTS.md wins),
  session start, plan before change (case lifecycle, propose and wait,
  name packages/paths so drift proposals work), zones and risk (R0–R3 as a
  rule of thumb, marked as not enforced), the commands (read-only table,
  writing table with "when", user-only list, exit codes), hooks and what
  they record (incl. the xargs/find -exec/python -c gap), drift and how to
  explain it, ending a session, the Phase 0 exit case as worked example
  (from the transcript, already redacted there), and a do-not list.
- engine/templates/{en,de}/AGENTS.md extended (template text only): new
  sections Session start, Commands, Ending a session; propose-and-wait,
  Result/Log, `plan done` only with the user's agreement, `--` for free
  text, what the hooks record, "otherwise ask the user" for drift, more
  Never items (move/delete logbook files, another logbook, shell strings
  from logbook text); link to the guide. English headings in both
  languages (ADR-0007). No CLAUDE.md anywhere.
- engine/tests/golden/init-skeleton.txt re-blessed (+3 headings only).
- New test setup::agents_md_carries_the_agent_rules_in_both_languages
  (engine/tests/init.rs): the ten `## ` sections in order and their key
  commands, per section, in en and de; link to docs/AGENT-GUIDE.md; init
  writes no CLAUDE.md. docs/TESTING.md init.rs row extended.
- README.md: "For AI agents" (three lines + links) appended at the very
  end, after the Author line; nothing else moved (WP-044/046 own the
  rest). plugin/README.md: the same pointer before "Project home".
- docs/SPEC-ENGINE.md §9: the one-line description of the generated
  AGENTS.md updated to the new sections (spec and template agree).

Not done:
- No live GitHub render (no push per instructions). Checked locally
  instead: balanced fences, every relative and github.com/…/blob/main link
  resolves to a file in the tree, every #anchor matches a heading slug,
  no raw `<…>` outside code spans (would be eaten as HTML), table pipes
  inside code spans escaped (`\|`, GFM).
- fixtures/logbook/AGENTS.md (an older hand-written German variant) left
  as is: no test ties it to the template, and fixture edits are outside
  this WP.

Verified by:
- `just check` → exit 0, "check: ok" (fmt, clippy, tests, watch feature,
  packaging, schema-validate, plugin-validate, qmllint 28 files,
  plugin-test incl. real-home guard, overlay-view 314 passed).
- `cargo test --test init` → 30 passed (new test included).
- Scratch init, HOME + all three XDG_* in one scratchpad dir,
  SELDON_TEST_GUARD set, `--config` in scratch, `--non-interactive
  --no-capture --no-git --language en|de` → exit 0 both; the logbook's
  AGENTS.md is byte-identical to the template with the ten sections
  (Session start … Never); no CLAUDE.md; ~/.config/seldon untouched.
- CLI checked against `seldon <cmd> --help` (same scratch env) for every
  command and flag the texts name (e.g. `hook install` takes only
  claude-code, `hook session-stop` defaults to agent:claude-code, `decide`
  has no --actor).

Learned: memory/pitfalls.md § WP-047 — the de template must not contain
  the string `logbook` (prose-language test), keep commands on one line in
  templates, template headings are pinned in two places.

Decisions needed:
- The texts now say an agent closes a case (`plan done`) only once the
  user agrees; before, the template let the agent close it after its own
  verification. This matches the Phase 0 run (agent → verification, human
  → done). Confirm, or I flip it back.
- Risk levels R0–R3 have no definition in the specs; the guide gives a
  rule of thumb (R0 nothing can break … R3 may not start). If the operator
  wants a normative scale, it belongs in SPEC-LOGBOOK §3 (an ADR?).

Touched outside WP scope: docs/SPEC-ENGINE.md §9 (one sentence, to keep
  the spec in step with the template); memory/pitfalls.md (append).
```
