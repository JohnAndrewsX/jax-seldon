#!/usr/bin/env python3
"""WP-171 manual mutants: each one undoes one rule of the file-link check
(ADR-0049); the unit tests of `logbook::`, `sys::`, `ledger::` and
`commands::setup`, `--test linked_files`, `--test linked_folders`, the
layout tests of `--test doctor` or the link test of `--test rules` must
fail for every one. Run it from a copy of the tree: it rewrites the
source in place and puts it back. Names given as arguments run only the
mutants whose name contains one of them; `--check` only applies each
mutant.

The target dir is `MUTANTS_TARGET` (on disk, never under /tmp), else
`engine/target/mutants-wp171` of the copy; the tests' TMPDIR is
`MUTANTS_TMPDIR`, else `engine/target/tmp-mutants` of the copy.

A mutant that does not compile is reported as such, not as killed. The
writers have two layers on purpose (`checked_file` before the ledger, and
a write primitive that never follows a link); a mutant that removes the
second layer at one call site while the first still refuses is reported
as SURVIVED and named in the handover (only a race reaches that layer)."""
import os
import signal
import subprocess
import sys
from pathlib import Path

# the checkout this script lives in: work/active/WP-171/mutants.py
WT = str(Path(__file__).resolve().parents[3])
TARGET = os.environ.get("MUTANTS_TARGET", f"{WT}/engine/target/mutants-wp171")
TMPDIR = os.environ.get("MUTANTS_TMPDIR", f"{WT}/engine/target/tmp-mutants")
LOGBOOK = "engine/src/logbook/mod.rs"
LAYOUT = "engine/src/logbook/layout.rs"
SYS = "engine/src/sys.rs"
LEDGER = "engine/src/ledger.rs"
CASES = "engine/src/logbook/cases.rs"
JOURNAL = "engine/src/logbook/journal.rs"
PLAN = "engine/src/commands/plan.rs"
DECIDE = "engine/src/commands/decide.rs"
DRIFT = "engine/src/commands/drift.rs"
IMPORT = "engine/src/commands/import.rs"
CAPTURE = "engine/src/commands/capture.rs"
RULES = "engine/src/commands/rules.rs"
DOSSIER = "engine/src/commands/dossier.rs"
DOSSIER_FILES = "engine/src/dossier/mod.rs"
VIEWS = "engine/src/index/views.rs"
REBUILD = "engine/src/commands/rebuild.rs"
SETUP = "engine/src/commands/setup.rs"
DOCTOR = "engine/src/commands/doctor.rs"


def plain(a, b, count=1):
    def apply(src):
        assert src.count(a) == count, (a, src.count(a))
        return src.replace(a, b)
    return apply


def both(*steps):
    def apply(src):
        for step in steps:
            src = step(src)
        return src
    return apply


CHECK_MATCH = "    match std::fs::symlink_metadata(&path) {\n        Ok(m) if m.file_type().is_file() => {}"
REGULAR = "        Ok(m) if m.file_type().is_file() => Ok(Some(m)),"

MUTANTS = [
    # the helper
    ("checked_file: the file not checked", LOGBOOK, plain(CHECK_MATCH, "    match std::fs::symlink_metadata(&path) {\n        Ok(_) => {}\n        #[allow(unreachable_patterns)]\n        Ok(m) if m.file_type().is_file() => {}")),
    ("checked_file: links followed", LOGBOOK, plain(CHECK_MATCH, "    match std::fs::metadata(&path) {\n        Ok(m) if m.file_type().is_file() => {}")),
    ("checked_file: a link is a file", LOGBOOK, plain("        Ok(m) if m.file_type().is_file() => {}", "        Ok(m) if m.file_type().is_file() || m.file_type().is_symlink() => {}")),
    ("checked_file: anything but a link is a file", LOGBOOK, plain("        Ok(m) if m.file_type().is_file() => {}", "        Ok(m) if !m.file_type().is_symlink() => {}")),
    ("checked_file: link and non-file named the other way", LOGBOOK, plain("                relative.display(),\n                if m.file_type().is_symlink() {", "                relative.display(),\n                if !m.file_type().is_symlink() {")),
    ("checked_file: exit 2, not 1", LOGBOOK, both(
        plain("            return Err(Error::user(format!(\n                \"{} is {}, not a file of the logbook;", "            return Err(Error::from(anyhow::Error::msg(format!(\n                \"{} is {}, not a file of the logbook;"),
        plain("                    \"no regular file\"\n                }\n            )));", "                    \"no regular file\"\n                }\n            ))));"),
    )),
    ("checked_file: the absolute path shown", LOGBOOK, plain("                \"{} is {}, not a file of the logbook; make it a file and run the command again\",\n                relative.display(),", "                \"{} is {}, not a file of the logbook; make it a file and run the command again\",\n                path.display(),")),
    ("checked_file: a missing file refused", LOGBOOK, plain("        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}\n        Err(e) => return Err(anyhow::anyhow!(\"{}: {e}\", path.display()).into()),\n    }\n    Ok(path)", "        Err(e) if false && e.kind() == std::io::ErrorKind::NotFound => {}\n        Err(e) => return Err(anyhow::anyhow!(\"{}: {e}\", path.display()).into()),\n    }\n    Ok(path)")),
    ("checked_file: an unreadable file accepted", LOGBOOK, plain("        Err(e) => return Err(anyhow::anyhow!(\"{}: {e}\", path.display()).into()),\n    }\n    Ok(path)", "        Err(_) => {}\n    }\n    Ok(path)")),
    ("checked_file: the folder not checked", LOGBOOK, plain("    let dir = checked_dir(root, relative.parent().unwrap_or(Path::new(\"\")))?;", "    let dir = root.join(relative.parent().unwrap_or(Path::new(\"\")));")),
    ("checked_file: a path without a name taken", LOGBOOK, plain("    let Some(name) = relative.file_name() else {", "    let Some(name) = relative.file_name().or(Some(std::ffi::OsStr::new(\"x\"))) else {")),
    ("Logbook::checked_file: the folder only", LOGBOOK, plain("        checked_file(&self.root, relative)\n", "        if let Some(dir) = relative.parent() {\n            checked_dir(&self.root, dir)?;\n        }\n        Ok(self.root.join(relative))\n")),
    # the write primitives
    ("sys: a link is a regular file", SYS, plain(REGULAR, "        Ok(m) if m.file_type().is_file() || m.file_type().is_symlink() => Ok(Some(m)),")),
    ("sys: anything but a link is a regular file", SYS, plain(REGULAR, "        Ok(m) if !m.file_type().is_symlink() => Ok(Some(m)),")),
    ("sys: links followed by the check", SYS, plain("pub fn regular_or_missing(path: &Path) -> anyhow::Result<Option<std::fs::Metadata>> {\n    match std::fs::symlink_metadata(path) {", "pub fn regular_or_missing(path: &Path) -> anyhow::Result<Option<std::fs::Metadata>> {\n    match std::fs::metadata(path) {")),
    ("sys: link and non-file named the other way", SYS, plain("            \"{} is {}, not a file of the logbook; nothing written\",\n            path.display(),\n            if m.file_type().is_symlink() {", "            \"{} is {}, not a file of the logbook; nothing written\",\n            path.display(),\n            if !m.file_type().is_symlink() {")),
    ("sys: a missing file refused", SYS, plain("        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),\n        Err(e) => Err(e).with_context(|| format!(\"cannot read {}\", path.display())),", "        Err(e) if false && e.kind() == std::io::ErrorKind::NotFound => Ok(None),\n        Err(e) => Err(e).with_context(|| format!(\"cannot read {}\", path.display())),")),
    ("sys: an unreadable path taken as missing", SYS, plain("        Err(e) => Err(e).with_context(|| format!(\"cannot read {}\", path.display())),\n    }\n}", "        Err(_) => Ok(None),\n    }\n}")),
    ("sys: the mode not kept", SYS, plain(".map_or(NEW_FILE_MODE, |m| m.permissions().mode() & 0o777);", ".map_or(NEW_FILE_MODE, |_| NEW_FILE_MODE);")),
    ("sys: nofollow writes resolve the link", SYS, both(
        plain("    let mode = regular_or_missing(path)?.map_or(NEW_FILE_MODE, |m| m.permissions().mode() & 0o777);\n    write_atomic_at(path.to_path_buf(), bytes, Some(mode), sync)", "    let mode = regular_or_missing(path).ok().flatten().map_or(NEW_FILE_MODE, |m| m.permissions().mode() & 0o777);\n    write_atomic_with(path, bytes, Some(mode), sync)"),
    )),
    ("sys: the check skipped", SYS, plain("    let mode = regular_or_missing(path)?.map_or(", "    let mode = regular_or_missing(path).ok().flatten().map_or(")),
    # the ledger
    ("ledger: month files not checked", LEDGER, plain("            for month in by_month.keys() {\n                let file = Path::new(LEDGER_DIR).join(format!(\"{month}.jsonl\"));\n                crate::logbook::checked_file(root, &file)?;\n            }\n", "")),
    ("ledger: the wrong month file checked", LEDGER, plain("let file = Path::new(LEDGER_DIR).join(format!(\"{month}.jsonl\"));", "let file = Path::new(LEDGER_DIR).join(format!(\"{month}.json\"));")),
    ("ledger: append follows a link", LEDGER, plain("    crate::sys::regular_or_missing(path)?;\n", "")),
    # the commands: the check before the ledger
    ("decide accept: the file not checked", DECIDE, plain("    logbook.checked_file(&path)?;\n", "")),
    ("rules update: the file not checked", RULES, plain("    let path = logbook.checked_file(FILE)?;", "    let path = logbook.path(FILE);")),
    ("capture rules: the file not checked", CAPTURE, plain("            let written = logbook\n                .checked_file(rules)\n                .map_err(anyhow::Error::new)\n                .and_then(", "            let written = Ok::<_, anyhow::Error>(logbook.path(rules))\n                .and_then(")),
    ("dossier: changed files not checked", DOSSIER, plain("    for rel in files.changed() {\n        logbook.checked_file(&rel)?;\n    }\n", "")),
    ("plan step: the case file not prepared", PLAN, plain("    // the case file from and to, as the save will write it: a link there\n    // fails the step before the ledger (WP-171)\n    file.prepare(&logbook, |_| {})?;\n", "")),
    ("plan new: the area README not checked", PLAN, plain("        logbook.checked_file(format!(\"areas/{area}/README.md\"))?;", "        logbook.checked_dir(format!(\"areas/{area}\"))?;")),
    ("drift explain: the area README not checked", DRIFT, plain("        logbook.checked_file(format!(\"areas/{area}/README.md\"))?;", "        logbook.checked_dir(format!(\"areas/{area}\"))?;")),
    ("import: the marker read through a link", IMPORT, plain("    match std::fs::read_to_string(logbook.checked_file(&rel)?) {", "    match std::fs::read_to_string(logbook.path(&rel)) {")),
    ("views: STATUS.md not checked before its read", VIEWS, plain("    // checked before it is read: a link is not read through, a FIFO\n    // not opened (WP-171)\n    let path = logbook.checked_file(REL)?;\n    let existing = match", "    let path = logbook.path(REL);\n    let existing = match")),
    ("views: DECISIONS.md not checked before its read", VIEWS, plain("    // checked before it is read: a link is not read through, a FIFO\n    // not opened (WP-171)\n    let path = logbook.checked_file(REL)?;\n    let old = match", "    let path = logbook.path(REL);\n    let old = match")),
    ("setup: a link where a file goes taken", SETUP, plain("                let target = crate::logbook::checked_file(root, &to.join(&rel))?;", "                let folder = to.join(rel.parent().unwrap_or(Path::new(\"\")));\n                let target = crate::logbook::checked_dir(root, &folder)?.join(entry.file_name());")),
    # the second layer at single call sites (only a race reaches it)
    ("layer 2: journal day written with write_atomic", JOURNAL, plain("        sys::write_atomic_nofollow(&self.path, self.text.as_bytes())?;", "        sys::write_atomic(&self.path, self.text.as_bytes())?;")),
    ("layer 2: case saved with write_atomic", CASES, plain("            sys::write_atomic_nofollow(&self.path, text.as_bytes())?;", "            sys::write_atomic(&self.path, text.as_bytes())?;")),
    ("layer 2: views written with write_generated", VIEWS, plain("        Durable::No => sys::write_generated_nofollow(&path, text.as_bytes())?,", "        Durable::No => sys::write_generated(&path, text.as_bytes())?,")),
    ("layer 2: rebuild written with write_generated", REBUILD, plain("sys::write_generated_nofollow(&path", "sys::write_generated(&path")),
    ("layer 2: dossier written with write_atomic", DOSSIER_FILES, plain("sys::write_atomic_nofollow(&f.path", "sys::write_atomic(&f.path")),
    # doctor's layout row
    ("doctor: no layout row", DOCTOR, plain("            checks.push(check_layout(&logbook.root));\n", "")),
    ("doctor: degraded, not error", DOCTOR, plain("    Check::new(\n        \"layout\",\n        Status::Error,", "    Check::new(\n        \"layout\",\n        Status::Degraded,")),
    ("doctor: six shown", DOCTOR, plain("    const SHOWN: usize = 5;", "    const SHOWN: usize = 6;")),
    ("doctor: no count of the rest", DOCTOR, plain("    if found.len() > SHOWN {", "    if found.len() > SHOWN * 10 {")),
    ("doctor: names shown raw", DOCTOR, plain(".map(|(rel, what)| format!(\"{} ({})\", shown_name(rel), what.as_str()))", ".map(|(rel, what)| format!(\"{} ({})\", rel, what.as_str()))")),
    ("layout: years not looked into", LAYOUT, plain("    (\"journal\", Some(1)),", "    (\"journal\", Some(0)),")),
    ("layout: work looked into", LAYOUT, plain("    (\"work\", None),", "    (\"work\", Some(1)),")),
    ("layout: links in a folder not named", LAYOUT, plain("        if kind.is_symlink() {\n            found(&rel, Misplaced::Link);", "        if kind.is_symlink() {\n            continue;")),
    ("layout: non-files in a folder not named", LAYOUT, plain("        } else if !kind.is_file() {\n            found(&rel, Misplaced::NoRegularFile);\n        }\n    }\n}", "        }\n    }\n}")),
    ("layout: a linked folder part not named", LAYOUT, plain("                    note(&rel, what, &mut out);\n                    real = false;", "                    real = false;")),
    ("layout: a linked folder looked into", LAYOUT, plain("        if let (true, Some(depth)) = (real, depth) {", "        if let (_, Some(depth)) = (real, depth) {")),
    ("layout: a folder named twice", LAYOUT, plain("        if !out.iter().any(|(r, _)| *r == rel) {", "        if true {")),
    ("layout: root files not checked", LAYOUT, plain("const WRITTEN_ROOT_FILES: &[&str] = &[\"AGENTS.md\", \"STATUS.md\", \"DECISIONS.md\"];", "const WRITTEN_ROOT_FILES: &[&str] = &[];")),
    ("layout: a root link not named", LAYOUT, plain("            if kind.is_symlink() {\n                note(Path::new(file), Misplaced::Link, &mut out);", "            if kind.is_symlink() {\n                continue;")),
    ("layout: a folder in a file's place not named", LAYOUT, plain("            } else if !kind.is_file() {\n                note(Path::new(file), Misplaced::NoRegularFile, &mut out);", "            } else if false {\n                note(Path::new(file), Misplaced::NoRegularFile, &mut out);")),
]

# seconds one test run may take under a mutant
LIMIT = 900


def restore_on_term(signum, frame):
    # a SIGTERM must not leave a mutant in the source
    raise KeyboardInterrupt


signal.signal(signal.SIGTERM, restore_on_term)

check = "--check" in sys.argv
only = [a for a in sys.argv[1:] if a != "--check"]
os.makedirs(TMPDIR, exist_ok=True)
env = dict(os.environ, CARGO_TARGET_DIR=TARGET, TMPDIR=TMPDIR)
cargo = ["cargo", "test", "--manifest-path", "engine/Cargo.toml", "--locked", "--no-fail-fast"]
runs = [
    cargo + ["--lib", "--", "logbook::", "sys::tests::nofollow", "sys::tests::regular", "ledger::", "commands::setup", "--test-threads=4"],
    cargo + ["--test", "linked_files", "--", "--test-threads=4"],
    cargo + ["--test", "linked_folders", "--", "--test-threads=4"],
    cargo + ["--test", "doctor", "--", "layout", "linked_folders_and_files", "--test-threads=4"],
    cargo + ["--test", "rules", "--", "through_a_link", "--test-threads=4"],
]
results = []
for name, file, mutate in MUTANTS:
    if only and not any(o in name for o in only):
        continue
    path = os.path.join(WT, file)
    orig = open(path).read()
    try:
        mutated = mutate(orig)
    except (AssertionError, ValueError) as e:
        results.append((name, f"NOT APPLIED {e}"))
        print(f"{name}: {results[-1][1]}", flush=True)
        continue
    if mutated == orig:
        results.append((name, "NOT APPLIED (no change)"))
        continue
    if check:
        results.append((name, "killed (not run)"))
        continue
    try:
        open(path, "w").write(mutated)
        failed = []
        for cmd in runs:
            # its own process group: a test left blocked (a FIFO opened)
            # is killed with cargo after the limit, and counts as killed
            p = subprocess.Popen(cmd, cwd=WT, env=env, stdout=subprocess.PIPE,
                                 stderr=subprocess.PIPE, text=True, start_new_session=True)
            try:
                _, err = p.communicate(timeout=LIMIT)
            except subprocess.TimeoutExpired:
                os.killpg(p.pid, signal.SIGKILL)
                p.communicate()
                failed.append((cmd[7] if cmd[6] == "--test" else "lib") + " timeout")
                continue
            r = subprocess.CompletedProcess(cmd, p.returncode, "", err)
            if "error[E" in r.stderr or "error: could not compile" in r.stderr:
                failed = None
                break
            if r.returncode != 0:
                failed.append(cmd[7] if cmd[6] == "--test" else "lib")
    finally:
        open(path, "w").write(orig)
    if failed is None:
        results.append((name, "DOES NOT COMPILE"))
    else:
        results.append((name, f"killed ({', '.join(failed)})" if failed else "SURVIVED"))
    print(f"{name}: {results[-1][1]}", flush=True)

print()
killed = sum(r.startswith("killed") for _, r in results)
print(f"{killed}/{len(results)} killed")
for name, r in results:
    if not r.startswith("killed"):
        print(f"  {name}: {r}")
