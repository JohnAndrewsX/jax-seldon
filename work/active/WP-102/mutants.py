#!/usr/bin/env python3
"""WP-102 manual mutants: each change must make `--test import_task` fail."""
import os, subprocess, sys
WT = "/home/eandres/Work/johnandrewsx/jax-seldon/wt/WP-102"
F = "engine/src/commands/import/task.rs"
MUTANTS = [
    ("redaction off", "let text = scrubber.text(&shown, &text);", "let text = text.clone();"),
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
env = dict(os.environ, CARGO_TARGET_DIR=f"{WT}/engine/target")
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
