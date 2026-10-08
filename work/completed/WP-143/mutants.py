#!/usr/bin/env python3
"""WP-143 manual mutants: each change must make the named tests fail.

Runs on a copy of the checkout (the engine includes schema/ and
fixtures/) in a temp dir with a target dir of its own, so
a mutated build never touches the checkout or another run's target.
Usage: python3 work/active/WP-143/mutants.py <copy-dir> <target-dir>
"""
import shutil, subprocess, sys
from pathlib import Path

WT = Path(__file__).resolve().parents[3]
COPY = Path(sys.argv[1])
TARGET = sys.argv[2]

PLAN = "src/commands/plan.rs"
DOCTOR = "src/commands/doctor.rs"
CASES = "src/logbook/cases.rs"
PLAN_TESTS = ["--lib", "commands::plan::tests"], ["--test", "plan", "plan::"]
DOCTOR_TESTS = ["--lib", "commands::doctor::tests"], ["--test", "doctor", "workpiece"]

RULE9 = "src/reconcile.rs"
RULE9_TESTS = (["--lib", "reconcile::tests::a_stop_condition_is_not_a_plan"],)
CASES_TESTS = (["--lib", "logbook::cases"],)

MUTANTS = [
    ("summary redaction off", PLAN, "clip(redactor.redact(&tail).trim(), CLOSING_TAIL_MAX)", "clip(tail.trim(), CLOSING_TAIL_MAX)", PLAN_TESTS),
    ("summary clip off", PLAN, "clip(redactor.redact(&tail).trim(), CLOSING_TAIL_MAX)", "redactor.redact(&tail).trim().to_string()", PLAN_TESTS),
    ("control characters kept", PLAN, "if c.is_control() || super::is_line_breaking(c) {", "if false {", PLAN_TESTS),
    ("line separators kept (round 2)", PLAN, "if c.is_control() || super::is_line_breaking(c) {", "if c.is_control() {", PLAN_TESTS),
    ("format characters kept (round 2, B1)", PLAN, ".filter(|c| !crate::import::is_direction_or_format(*c))", ".filter(|_| true)", PLAN_TESTS),
    ("result line off", PLAN, ".and_then(|r| r.lines().next()),", ".and_then(|_| None::<&str>),", PLAN_TESTS),
    ("list marker kept", PLAN, ".unwrap_or(l)\n                .trim()", ".map(|_| l)\n                .unwrap_or(l)\n                .trim()", PLAN_TESTS),
    ("drop reason off", PLAN, "reason.as_deref(), &redactor)", "None, &redactor)", PLAN_TESTS),
    ("shipped template not upgraded", CASES, "Ok(text) if shipped_template(name, &text) => built_in(),", "Ok(text) if false && shipped_template(name, &text) => built_in(),", PLAN_TESTS),
    ("edited template replaced", CASES, "Ok(text) if shipped_template(name, &text) => built_in(),", "Ok(text) if !text.is_empty() => built_in(),", PLAN_TESTS),
    ("de hash off (round 2, N2 B)", CASES, '"a0561f366ad32a90f903b1f079cd7934b71224b71769e5244c2f56c63c56cf4c"', '"0000000000000000000000000000000000000000000000000000000000000000"', CASES_TESTS),
    ("name not checked (round 2, N5)", CASES, "SHIPPED_TEMPLATES.contains(&(name, hash.as_str()))", "SHIPPED_TEMPLATES.iter().any(|(_, h)| *h == hash)", CASES_TESTS),
    ("stop if read by rule 9 (round 2, N1)", RULE9, "cases::without_stop_if(&cases::strip_comments(&body[r]))", "cases::strip_comments(&body[r])", RULE9_TESTS),
    ("stop if continuation kept (round 2, N1)", CASES, "            i += n;\n            continue;", "            continue;", CASES_TESTS),
    ("orphans not counted", DOCTOR, "let left = if !named.iter().any(|n| n == id) {", "let left = if false {", DOCTOR_TESTS),
    ("a case file that does not parse is absent (round 2, N2 C)", DOCTOR, "let left = if !named.iter().any(|n| n == id) {", "let left = if !files.iter().any(|f| f.case.id == id) {", DOCTOR_TESTS),
    ("open cases measured", DOCTOR, "            closed\n                .then(", "            true\n                .then(", DOCTOR_TESTS),
    ("threshold off", DOCTOR, ".filter(|size| size.bytes > WORKPIECE_LARGE)", ".filter(|_| true)", DOCTOR_TESTS),
    ("threshold inclusive (round 2, N2 H)", DOCTOR, ".filter(|size| size.bytes > WORKPIECE_LARGE)", ".filter(|size| size.bytes >= WORKPIECE_LARGE)", DOCTOR_TESTS),
    ("walk never stops early (round 2, N3)", DOCTOR, "if stop_above.is_some_and(|limit| bytes > limit) {", "if false {", DOCTOR_TESTS),
    ("an early stop is exact (round 2, N3)", DOCTOR, "exact: entries.peek().is_none() && stack.is_empty(),", "exact: true,", DOCTOR_TESTS),
    ("no entry cap (round 2, N3)", DOCTOR, "if seen > max_entries {", "if false {", DOCTOR_TESTS),
    ("inner symbolic links followed (round 2, N2 A)", DOCTOR, "let Ok(meta) = entry.metadata() else {", "let Ok(meta) = std::fs::metadata(entry.path()) else {", DOCTOR_TESTS),
    ("oldest is the newest", DOCTOR, "(k, o) > (key(id), name.as_str())", "(k, o) < (key(id), name.as_str())", DOCTOR_TESTS),
    ("oldest by name", DOCTOR, "(k, o) > (key(id), name.as_str())", "o > name.as_str()", DOCTOR_TESTS),
    ("symbolic links followed", DOCTOR, ".filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))", ".filter(|e| e.path().is_dir())", DOCTOR_TESTS),
    ("control characters shown", DOCTOR, "if crate::import::bad_path_char(c) || super::is_line_breaking(c) {", "if false {", DOCTOR_TESTS),
    ("format characters shown (round 2, B1)", DOCTOR, "if crate::import::bad_path_char(c) || super::is_line_breaking(c) {", "if c.is_control() || super::is_line_breaking(c) {", DOCTOR_TESTS),
    ("line separators shown (round 2, B1)", DOCTOR, "if crate::import::bad_path_char(c) || super::is_line_breaking(c) {", "if crate::import::bad_path_char(c) {", DOCTOR_TESTS),
    ("id suffix not checked", DOCTOR, "&& (name.len() == id.len() || name[id.len()..].starts_with('-')))", ")", DOCTOR_TESTS),
    ("short numbers accepted", DOCTOR, "&& digits >= 3", "&& digits >= 1", DOCTOR_TESTS),
]


def run(args):
    cmd = ["cargo", "test", "--locked", "--manifest-path", str(COPY / "engine" / "Cargo.toml"), *args]
    env = {**__import__("os").environ, "CARGO_TARGET_DIR": TARGET}
    return subprocess.run(cmd, env=env, capture_output=True, text=True).returncode


def main():
    if COPY.exists():
        shutil.rmtree(COPY)
    shutil.copytree(WT, COPY, ignore=shutil.ignore_patterns("target", ".git"))
    survived = []
    for name, rel, old, new, tests in MUTANTS:
        path = COPY / "engine" / rel
        original = path.read_text()
        assert original.count(old) == 1, f"{name}: pattern not unique in {rel}"
        path.write_text(original.replace(old, new))
        try:
            # a mutant that does not build proves nothing
            if run(["--no-run", "--lib", "--test", "plan", "--test", "doctor"]) != 0:
                print(f"NO BUILD {name}", flush=True)
                survived.append(name)
                continue
            caught = any(run(t) != 0 for t in tests)
        finally:
            path.write_text(original)
        print(f"{'caught  ' if caught else 'SURVIVED'} {name}", flush=True)
        if not caught:
            survived.append(name)
    print(f"{len(MUTANTS) - len(survived)}/{len(MUTANTS)} caught")
    sys.exit(1 if survived else 0)


main()
