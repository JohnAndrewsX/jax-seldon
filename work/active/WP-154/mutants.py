#!/usr/bin/env python3
"""WP-154 manual mutants: each change must make a test fail
(`--lib` and `--test git`, `--test collectors_user plugin`)."""
import os, subprocess, sys
from pathlib import Path
# the checkout this script lives in: work/active/WP-154/mutants.py
WT = str(Path(__file__).resolve().parents[3])
GIT = "engine/src/logbook/git.rs"
SYS = "engine/src/sys.rs"
PLUGINS = "engine/src/collectors/plugins.rs"
INDEX = "engine/src/index/mod.rs"
# a target dir of its own: a mutated build must never reach another run
TARGET = f"{WT}/engine/target/mutants"
# (name, [(old, new), ...], file); every old must match exactly once
MUTANTS = [
    # rule 1: caps
    ("R1 a stdout over the cap is an answer", [("    if out.cut {\n        return Run::Cut;\n    }\n", "")], SYS),
    ("R1 the drain keeps everything", [("let keep = n.min(cap - buf.len());", "let keep = n;")], SYS),
    ("R1 git calls uncapped", [("sys::run_command_in_engine_group(command(root, args), TIMEOUT, sys::OUTPUT_MAX)",
                                "sys::run_command_in_engine_group(command(root, args), TIMEOUT, sys::WHOLE_OUTPUT)"),
                               ("            timeout,\n            sys::OUTPUT_MAX,\n",
                                "            timeout,\n            sys::WHOLE_OUTPUT,\n")], GIT),
    ("R1 OUTPUT_MAX 100 MiB", [("pub const OUTPUT_MAX: usize = 1024 * 1024;", "pub const OUTPUT_MAX: usize = 100 * 1024 * 1024;")], SYS),
    ("R1 a cut status is clean", [("        Run::Cut => Ok(false),\n", "        Run::Cut => Ok(true),\n")], GIT),
    ("R1 the plugins' cap lost", [("sys::run_command(self.command(dir, args), self.timeout, GIT_OUTPUT_MAX)",
                                   "sys::run_command(self.command(dir, args), self.timeout, sys::WHOLE_OUTPUT)")], PLUGINS),
    # rule 2: no network for queries
    ("R2 query rules off", [("    cmd.envs(QUERY_ENV);\n", ""),
                            ("    if option {\n        argv.push(NO_LAZY_FETCH);\n    }\n", "    let _ = option;\n")], GIT),
    ("R2 only QUERY_ENV off", [("    cmd.envs(QUERY_ENV);\n", "")], GIT),
    ("R2 only the option off", [("    if option {\n        argv.push(NO_LAZY_FETCH);\n    }\n", "    let _ = option;\n")], GIT),
    ("R2 GIT_ALLOW_PROTOCOL off", [('("GIT_ALLOW_PROTOCOL", "none")', '("GIT_UNUSED", "none")')], GIT),
    ("R2 GIT_NO_LAZY_FETCH off", [('("GIT_NO_LAZY_FETCH", "1")', '("GIT_UNUSED", "1")')], GIT),
    ("R2 commits get the query rules", [("    sys::run_command_in_engine_group(command(root, args), TIMEOUT, sys::OUTPUT_MAX)",
                                         "    sys::run_command_in_engine_group(query_command(root, args, true), TIMEOUT, sys::OUTPUT_MAX)")], GIT),
    ("R2 status with transport", [('    status_is_empty(query_here(root, &["status", "--porcelain"])).map(|empty| !empty)',
                                   '    status_is_empty(run(Some(root), &["status", "--porcelain"])).map(|empty| !empty)')], GIT),
    ("R2 index status with transport", [('git::status_is_empty(git::query(root, &["status", "--porcelain"], timeout))',
                                         'git::status_is_empty(crate::sys::run_command_in_engine_group({ let mut c = std::process::Command::new("git"); c.args(["status", "--porcelain"]).current_dir(root); c }, timeout, crate::sys::OUTPUT_MAX))')], INDEX),
    ("R2 old git not retried", [("    if option && refuses_no_lazy_fetch(&answer) {", "    if false && refuses_no_lazy_fetch(&answer) {")], GIT),
    ("R2 refusal not remembered", [("        NO_LAZY_FETCH_REFUSED.store(true, Ordering::Relaxed);\n", "")], GIT),
    ("R2 any 129 is the refusal", [("if stderr.contains(NO_LAZY_FETCH))", "if !stderr.is_empty())")], GIT),
    ("R2 any exit code is the refusal", [("Run::Exited { code: Some(129), stderr, .. }", "Run::Exited { stderr, .. }")], GIT),
]
env = dict(os.environ, CARGO_TARGET_DIR=TARGET)
results = []
# optional arguments: run only the mutants whose name starts with one of them
only = sys.argv[1:]
for name, pairs, rel in MUTANTS:
    if only and not any(name.startswith(o) for o in only):
        continue
    path = os.path.join(WT, rel)
    orig = open(path).read()
    bad = [a for a, _ in pairs if orig.count(a) != 1]
    if bad:
        results.append((name, f"PATTERN COUNT {[orig.count(a) for a in bad]}")); continue
    mutated = orig
    for a, b in pairs:
        mutated = mutated.replace(a, b)
    try:
        open(path, "w").write(mutated)
        runs = [["--lib", "--test", "git"], ["--test", "collectors_user", "plugin"]]
        out, code = "", 0
        for args in runs:
            r = subprocess.run(["cargo", "test", "--manifest-path", "engine/Cargo.toml",
                                "--no-fail-fast", *args],
                               cwd=WT, env=env, capture_output=True, text=True)
            out += r.stdout + r.stderr
            code |= r.returncode
            if "error[E" in r.stderr:
                break
    finally:
        open(path, "w").write(orig)
    failed = [l.split()[1] for l in out.splitlines() if l.startswith("test ") and l.endswith("FAILED")]
    results.append((name, ("killed by " + ", ".join(failed)) if code != 0 and failed else
                    ("SURVIVED" if code == 0 else "build error:\n" + out[-800:])))
for n, r in results:
    print(f"{n}: {r}")
