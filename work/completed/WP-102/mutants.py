#!/usr/bin/env python3
"""WP-102 manual mutants: each change must make `--test import_task` fail."""
import os, subprocess, sys
from pathlib import Path
# the checkout this script lives in: work/active/WP-102/mutants.py
WT = str(Path(__file__).resolve().parents[3])
F = "engine/src/commands/import/task.rs"
# a target dir of its own: a mutated binary must never reach another run
TARGET = f"{WT}/engine/target/mutants"
MUTANTS = [
    ("redaction off", "    let text = scrubber.text(&shown, &text);", "    let text = text.clone();"),
    ("whole-text redaction off (lines only)", "        let whole = self.redactor.redact_keeping_lines(text);", "        let whole = text.to_string();", "engine/src/import/mod.rs"),
    ("session agent check off (B2)", ".or(session.as_ref().filter(|s| is_agent(s)))", ".or(None)"),
    ("resolved-path character check off (N1)", "if real.to_string_lossy().chars().any(bad_path_char) {", "if false {"),
    ("pending entry off: marker only after the case (N2, R1)", "if let Err(e) = write_marker(&logbook.path(&marker_rel), &marker) {\n            marker.items.pop();", "if let Err(e) = Ok::<(), Error>(()) {\n            marker.items.pop();"),
    ("settle trusts any pending entry (N2)", "        e.pending = false;\n        made\n", "        e.pending = false;\n        let _ = made;\n        true\n"),
    ("same-path dedupe off (R5)", "if !seen.insert(path.clone()) {", "if false && !seen.insert(path.clone()) {"),
    ("replaces even when the old text is still there (R7)", "&& !tasks.iter().any(|o| o.file == e.file && o.hash == e.hash)", ""),
    ("path redaction off (R8)", "let shown = redactor.redact(&format!(\"~/{}\", rest.to_string_lossy()));", "let shown = format!(\"~/{}\", rest.to_string_lossy());"),
    ("provenance line off (N4)", "provenance(&self.source()),", "String::new(),"),
    ("idempotency off", ".find(|e| e.file == t.file && e.hash == t.hash)", ".find(|e| e.file == t.file && e.hash == \"x\")"),
    ("logbook refusal off", "if real.starts_with(&root) {", "if false && real.starts_with(&root) {"),
    ("home refusal off", "if real == home || !real.starts_with(&home) {", "if false {"),
    ("escape off", "cases::escape_lines(&self.intent)", "self.intent.clone()"),
    ("done skip off", "if t.done && !include_done {", "if false {"),
    ("dry run applies", "let lock = if args.dry_run {", "let lock = if false {"),
    ("limit off", "if planned > MAX_CASES {", "if planned > MAX_CASES * 10 {"),
    ("changed-item link off", "&& e.line == t.line", "&& false"),
    ("duplicate check off", "if !taken.insert((&t.file, &t.hash)) {", "if false && !taken.insert((&t.file, &t.hash)) {"),
    ("extension check off", ".is_some_and(|e| e.eq_ignore_ascii_case(\"md\"))", ".is_some_and(|_| true)"),
    ("size cap off", "if bytes.len() as u64 > MAX_FILE_BYTES {", "if false {"),
    # round 3
    ("CRLF normalisation off", "let text = text.replace(\"\\r\\n\", \"\\n\");", "let text = text.clone();"),
    ("format characters allowed in paths", "            | '\\u{200B}'..='\\u{200F}'\n", "            | '\\u{200E}'..='\\u{200F}'\n", "engine/src/import/mod.rs", ["--lib", "import::"]),
    ("plan start of an imported case by an agent allowed", "        refuse_agent_start_of_imported(&file, &actor)?;\n", "\n", "engine/src/commands/plan.rs"),
    ("session part of the imported-start refusal off", "        .or(session.as_deref().filter(|s| is_agent(s)));", "        .or(None::<&str>);", "engine/src/commands/plan.rs"),
    # 102b round 2 (a fifth element: the cargo test target, default --test import_task)
    ("E1 plan show intent not redacted", "    (redactor.redact(&marked), hidden)", "    (marked, hidden)", "engine/src/index/build.rs"),
    ("E3 no character-boundary walk", "    while !text.is_char_boundary(end) {\n        end -= 1;\n    }\n", "", "engine/src/commands/plan.rs"),
    ("E4 unredacted text when the patterns fail", "    let Ok(redactor) = Redactor::for_config(config) else {\n        return Value::Null;\n    };", "    let redactor = Redactor::for_config(config).unwrap_or_else(|_| Redactor::with_patterns(&[]).unwrap());", "engine/src/commands/plan.rs"),
    ("B2 import keeps invisible characters", "        .filter(|c| !is_direction_or_format(*c))\n", ""),
    ("B2 plan show drops instead of marking", "            marked.push_str(&format!(\"‹U+{:04X}›\", c as u32));", "", "engine/src/index/build.rs"),
    ("B1 too-long off", "            if t.full_intent().len() > SHOW_INTENT_MAX {", "            if false {"),
    ("N3 separators allowed in paths", " || matches!(c, '\\u{2028}' | '\\u{2029}')", "", "engine/src/import/mod.rs", ["--lib", "import::"]),
    ("agent start on a queued imported case: plain hint", "            super::plan::refuse_agent_start_of_imported(&file, actor)?;\n", "\n", "engine/src/commands/agent.rs"),
]
env = dict(os.environ, CARGO_TARGET_DIR=TARGET)
results = []
for mutant in MUTANTS:
    name, a, b = mutant[:3]
    path = os.path.join(WT, mutant[3] if len(mutant) > 3 else F)
    orig = open(path).read()
    if orig.count(a) != 1:
        results.append((name, f"PATTERN COUNT {orig.count(a)}")); continue
    try:
        open(path, "w").write(orig.replace(a, b))
        target = mutant[4] if len(mutant) > 4 else ["--test", "import_task"]
        r = subprocess.run(["cargo", "test", "--manifest-path", "engine/Cargo.toml"] + target,
                           cwd=WT, env=env, capture_output=True, text=True)
    finally:
        open(path, "w").write(orig)
    failed = [l.split()[1] for l in r.stdout.splitlines() if l.startswith("test ") and l.endswith("FAILED")]
    results.append((name, ("killed by " + ", ".join(failed)) if r.returncode != 0 and failed else
                    ("SURVIVED" if r.returncode == 0 else "build error:\n" + r.stderr[-800:])))
for n, r in results:
    print(f"{n}: {r}")
