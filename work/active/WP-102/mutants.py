#!/usr/bin/env python3
"""WP-102 manual mutants: each change must make `--test import_task` fail."""
import os, subprocess, sys
WT = "/home/eandres/Work/johnandrewsx/jax-seldon/wt/WP-102"
F = "engine/src/commands/import/task.rs"
# a target dir of its own: a mutated binary must never reach another run
TARGET = f"{WT}/engine/target/mutants"
MUTANTS = [
    ("redaction off", "let text = scrubber.text(&shown, &whole);", "let text = text.clone();"),
    ("whole-text redaction off (lines only)", "let whole = redactor.redact_keeping_lines(&text);", "let whole = text.clone();"),
    ("session agent check off (B2)", ".or(session.as_ref().filter(|s| is_agent(s)))", ".or(None)"),
    ("resolved-path character check off (N1)", "if real.to_string_lossy().chars().any(bad_path_char) {", "if false {"),
    ("pending entry off: marker only after the case (N2, R1)", "if let Err(e) = write_marker(&logbook.path(&marker_rel), &marker) {\n            marker.items.pop();", "if let Err(e) = Ok::<(), Error>(()) {\n            marker.items.pop();"),
    ("settle trusts any pending entry (N2)", "        e.pending = false;\n        made\n", "        e.pending = false;\n        let _ = made;\n        true\n"),
    ("same-path dedupe off (R5)", "if !seen.insert(path.clone()) {", "if false && !seen.insert(path.clone()) {"),
    ("replaces even when the old text is still there (R7)", "&& !tasks.iter().any(|o| o.file == e.file && o.hash == e.hash)", ""),
    ("path redaction off (R8)", "let shown = redactor.redact(&format!(\"~/{}\", rest.to_string_lossy()));", "let shown = format!(\"~/{}\", rest.to_string_lossy());"),
    ("provenance line off (N4)", "provenance(&task.source()),", "String::new(),"),
    ("idempotency off", ".find(|e| e.file == t.file && e.hash == t.hash)", ".find(|e| e.file == t.file && e.hash == \"x\")"),
    ("logbook refusal off", "if real.starts_with(&root) {", "if false && real.starts_with(&root) {"),
    ("home refusal off", "if real == home || !real.starts_with(&home) {", "if false {"),
    ("escape off", "cases::escape_lines(&task.intent)", "task.intent.clone()"),
    ("done skip off", "if t.done && !include_done {", "if false {"),
    ("dry run applies", "let lock = if args.dry_run {", "let lock = if false {"),
    ("limit off", "if planned > MAX_CASES {", "if planned > MAX_CASES * 10 {"),
    ("changed-item link off", "&& e.line == t.line", "&& false"),
    ("duplicate check off", "if !taken.insert((&t.file, &t.hash)) {", "if false && !taken.insert((&t.file, &t.hash)) {"),
    ("extension check off", ".is_some_and(|e| e.eq_ignore_ascii_case(\"md\"))", ".is_some_and(|_| true)"),
    ("size cap off", "if bytes.len() as u64 > MAX_FILE_BYTES {", "if false {"),
]
env = dict(os.environ, CARGO_TARGET_DIR=TARGET)
path = os.path.join(WT, F)
orig = open(path).read()
results = []
try:
    for name, a, b in MUTANTS:
        if orig.count(a) != 1:
            results.append((name, f"PATTERN COUNT {orig.count(a)}")); continue
        open(path, "w").write(orig.replace(a, b))
        r = subprocess.run(["cargo", "test", "--manifest-path", "engine/Cargo.toml", "--test", "import_task"],
                           cwd=WT, env=env, capture_output=True, text=True)
        failed = [l.split()[1] for l in r.stdout.splitlines() if l.startswith("test ") and l.endswith("FAILED")]
        results.append((name, ("killed by " + ", ".join(failed)) if r.returncode != 0 and failed else
                        ("SURVIVED" if r.returncode == 0 else "build error:\n" + r.stderr[-800:])))
finally:
    open(path, "w").write(orig)
for n, r in results:
    print(f"{n}: {r}")
