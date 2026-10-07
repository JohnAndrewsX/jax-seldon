#!/usr/bin/env python3
"""WP-127 manual mutants: each change must make a test fail.

Engine mutants run `cargo test --lib --test index --test redaction --test
import_task --test drift` in a target dir of their own (one that only slows
the build, N5, runs the timing test of `just check-perf` instead); plugin mutants run
`node tests/plugin/model.test.js`. Every file is restored after its run. Arguments, if any, keep only the
mutants whose name contains one of them.
"""
import os, subprocess, sys
from pathlib import Path

# the checkout this script lives in: work/active/WP-127/mutants.py
WT = str(Path(__file__).resolve().parents[3])
# a target dir of its own: a mutated binary must never reach another run
TARGET = f"{WT}/engine/target/mutants-wp127"
BUILD = "engine/src/index/build.rs"
LOAD = "engine/src/index/load.rs"
MODEL = "plugin/Model.js"
PERF = ["cargo", "test", "--manifest-path", "engine/Cargo.toml", "--locked", "--profile", "bench",
        "--test", "index", "--", "--ignored", "the_first_paragraphs_of_a_large_intent"]
ENGINE = [
    ("drift rule left out", BUILD, "rule: Some(rule.to_string()),", "rule: None,"),
    ("texts not redacted", BUILD, "let redacted = redactor.redact(&plain);", "let redacted = plain.clone();"),
    ("source not redacted again", BUILD, "let source = redactor?.redact(case.source.as_deref()?);",
     "let _ = redactor?;\n    let source = case.source.clone()?;"),
    ("source shape not checked", BUILD, "if is_case_source(&source) {", "if !source.is_empty() {"),
    ("texts not clipped", BUILD, "let shown = clip_with(&redacted, IN_THE_FILE).into_owned();", "let shown = redacted.clone();"),
    ("ledger marker for file texts", BUILD, "clip_with(&redacted, IN_THE_FILE)", "clip(&redacted)"),
    ("control characters kept", BUILD, "if c.is_control() && c != '\\n' && c != '\\t' {", "if false {"),
    ("invalid patterns fall back to the built-in rules", "engine/src/index/mod.rs",
     "redactor: crate::redact::Redactor::for_config(config).ok(),",
     "redactor: Some(crate::redact::Redactor::for_config(config).unwrap_or_else(|_| crate::redact::Redactor::builtin())),"),
    ("provenance line kept as the intent", LOAD, "if imported && is_provenance(&first) {", "if false {"),
    ("provenance skipped without the tag", LOAD,
     "let imported = case.tags.iter().any(|t| t == TAG_IMPORTED);", "let imported = !case.tags.contains(&String::new());"),
    ("lead from the wrong section", LOAD, 'cases::first_paragraph(&doc.body, "Decision")', 'cases::first_paragraph(&doc.body, "Context")'),
    ("result from the intent", LOAD, 'result: cases::first_paragraph(&doc.body, "Result"),', 'result: cases::first_paragraph(&doc.body, "Intent"),'),
    ("heading lines are paragraph text", "engine/src/logbook/cases.rs",
     "if line.trim().is_empty() || is_heading(line.trim_start()) {", "if line.trim().is_empty() {"),
    ("comments are paragraph text", "engine/src/logbook/cases.rs",
     'match rest[..nl].find("<!--") {', "match None::<usize> {"),
    ("import writes no source", "engine/src/commands/import/task.rs",
     "source: Some(case_source(&task.source())),", "source: None,"),
    ("long source not shortened", "engine/src/import/mod.rs", "if source.len() <= SOURCE_MAX {", "if !source.is_empty() {"),
    ("format characters pass in a source", "engine/src/import/mod.rs",
     "&& !source.chars().any(bad_path_char)", "&& !source.chars().any(char::is_control)"),
    ("every case file gets a source key", "engine/src/model/case.rs",
     'if let Some(source) = &self.source {\n            values.push(("source", FmValue::text(source)));\n        }',
     'values.push(("source", FmValue::opt_str(self.source.as_ref())));'),
    # round 2
    ("clip before redaction (B1)", BUILD,
     "    let redacted = redactor.redact(&plain);\n    let shown = clip_with(&redacted, IN_THE_FILE).into_owned();",
     "    let clipped = clip_with(&plain, IN_THE_FILE).into_owned();\n    let shown = redactor.redact(&clipped);"),
    ("plan show prints any source (R2)", "engine/src/commands/plan.rs",
     ".filter(|s| crate::import::is_case_source(s))", ""),
    ("a blank text is a text (R3)", BUILD, "(!shown.trim().is_empty()).then_some(shown)", "(!shown.is_empty()).then_some(shown)"),
    ("direction and format characters kept (N4)", BUILD, ".filter(|c| !is_direction_or_format(*c))", ".filter(|_| true)"),
    ("source cap in characters, not bytes", "engine/src/import/mod.rs",
     "source.starts_with(\"~/\") && source.len() <= SOURCE_MAX", "source.starts_with(\"~/\") && source.chars().count() <= SOURCE_MAX"),
    ("a shortened source may split a character", "engine/src/import/mod.rs",
     "while !source.is_char_boundary(start) {", "while false && !source.is_char_boundary(start) {"),
    # equivalent output, slower: only the timing of `just check-perf` sees it
    ("paragraphs read past the ones needed (N5)", "engine/src/logbook/cases.rs",
     "while pos < text.len() && out.len() < max {", "while pos < text.len() {", PERF),
    ("a non-string source makes the case unreadable", "engine/src/model/case.rs",
     '#[serde(default, deserialize_with = "string_only")]\n    pub source', "#[serde(default)]\n    pub source"),
]
PLUGIN = [
    ("index rule not read", MODEL,
     'if (isObject(d) && d.eventId === eventId && typeof d.rule === "string" && RULE_ID.test(d.rule))',
     "if (false)"),
    ("rule shape not checked", MODEL,
     'if (isObject(d) && d.eventId === eventId && typeof d.rule === "string" && RULE_ID.test(d.rule))',
     'if (isObject(d) && d.eventId === eventId && d.rule)'),
    ("intent not in the case detail", MODEL, "    intent: str(raw.intent),\n", '    intent: "",\n'),
    ("source row not shown", MODEL,
     'if (str(raw.source) !== "") kv.push(["Imported from", str(raw.source)])', ""),
    ("lead not in the decision detail", MODEL, "    text: str(row.lead),\n", '    text: "",\n'),
    ("a non-string intent shown", MODEL, "    intent: str(raw.intent),\n", "    intent: raw.intent === undefined ? \"\" : raw.intent,\n"),
]
CARGO = ["cargo", "test", "--manifest-path", "engine/Cargo.toml", "--locked", "--lib",
         "--test", "index", "--test", "redaction", "--test", "import_task", "--test", "drift"]
env = dict(os.environ, CARGO_TARGET_DIR=TARGET, TZ="UTC")
results = []
for kind, mutants, cmd in (("engine", ENGINE, CARGO), ("plugin", PLUGIN, ["node", "tests/plugin/model.test.js"])):
    for name, rel, a, b, *own in mutants:
        if sys.argv[1:] and not any(w in name for w in sys.argv[1:]):
            continue
        path = os.path.join(WT, rel)
        orig = open(path, encoding="utf-8").read()
        if orig.count(a) != 1:
            results.append((kind, name, f"PATTERN COUNT {orig.count(a)}"))
            continue
        try:
            open(path, "w", encoding="utf-8").write(orig.replace(a, b))
            r = subprocess.run(own[0] if own else cmd, cwd=WT, env=env, capture_output=True, text=True)
        finally:
            open(path, "w", encoding="utf-8").write(orig)
        out = r.stdout + r.stderr
        failed = [l.split()[1] for l in out.splitlines() if l.startswith("test ") and l.endswith("FAILED")]
        failed += [l[5:].strip() for l in out.splitlines() if l.startswith("FAIL ")]
        verdict = ("killed by " + ", ".join(failed)) if r.returncode != 0 and failed else \
            "SURVIVED" if r.returncode == 0 else "build error:\n" + out[-800:]
        results.append((kind, name, verdict))
        print(f"{kind}: {name}: {verdict}", flush=True)
survived = [n for _, n, v in results if not v.startswith("killed")]
print(f"\n{len(results) - len(survived)} of {len(results)} killed" + (f"; not killed: {survived}" if survived else ""))
