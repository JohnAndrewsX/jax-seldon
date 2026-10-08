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

MUTANTS = [
    ("summary redaction off", PLAN, "clip(redactor.redact(&tail).trim(), CLOSING_TAIL_MAX)", "clip(tail.trim(), CLOSING_TAIL_MAX)", PLAN_TESTS),
    ("summary clip off", PLAN, "clip(redactor.redact(&tail).trim(), CLOSING_TAIL_MAX)", "redactor.redact(&tail).trim().to_string()", PLAN_TESTS),
    ("control characters kept", PLAN, "if c.is_control() { ' ' } else { c }", "c", PLAN_TESTS),
    ("result line off", PLAN, ".and_then(|r| r.lines().next()),", ".and_then(|_| None::<&str>),", PLAN_TESTS),
    ("list marker kept", PLAN, ".unwrap_or(l)\n                .trim()", ".map(|_| l)\n                .unwrap_or(l)\n                .trim()", PLAN_TESTS),
    ("drop reason off", PLAN, "reason.as_deref(), &redactor)", "None, &redactor)", PLAN_TESTS),
    ("shipped template not upgraded", CASES, "Ok(text) if SHIPPED_TEMPLATES.contains", "Ok(text) if false && SHIPPED_TEMPLATES.contains", PLAN_TESTS),
    ("edited template replaced", CASES, "Ok(text) if SHIPPED_TEMPLATES.contains(&sys::sha256_hex(text.as_bytes()).as_str())", "Ok(text) if !text.is_empty()", PLAN_TESTS),
    ("orphans not counted", DOCTOR, "let left = if !named.iter().any(|n| n == id) {", "let left = if false {", DOCTOR_TESTS),
    ("open cases measured", DOCTOR, "            closed\n                .then(", "            true\n                .then(", DOCTOR_TESTS),
    ("threshold off", DOCTOR, ".filter(|size| *size > WORKPIECE_LARGE)", ".filter(|_| true)", DOCTOR_TESTS),
    ("oldest is the newest", DOCTOR, "(k, o) > (key(id), name.as_str())", "(k, o) < (key(id), name.as_str())", DOCTOR_TESTS),
    ("oldest by name", DOCTOR, "(k, o) > (key(id), name.as_str())", "o > name.as_str()", DOCTOR_TESTS),
    ("symbolic links followed", DOCTOR, ".filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))", ".filter(|e| e.path().is_dir())", DOCTOR_TESTS),
    ("control characters shown", DOCTOR, "if c.is_control() { '?' } else { c }", "c", DOCTOR_TESTS),
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
