#!/usr/bin/env python3
"""WP-168 manual mutants: each one undoes one rule of the linked-folder
check; the unit tests of `logbook::` / `commands::setup` or
`--test linked_folders` must fail for every one. Run it from a copy of
the tree: it rewrites the source in place and puts it back. Names given as
arguments run only the mutants whose name contains one of them; `--check`
only applies each mutant.

A mutant that does not compile is reported as such, not as killed. The
writers have two layers on purpose (a check before the ledger and one in
the write primitive); a mutant that removes one layer while the other
still refuses is reported as SURVIVED and named in the handover."""
import os
import subprocess
import sys
from pathlib import Path

# the checkout this script lives in: work/active/WP-168/mutants.py
WT = str(Path(__file__).resolve().parents[3])
# a target dir of its own, on disk: a mutated build never reaches another run
TARGET = f"{WT}/engine/target/mutants-wp168"
LOGBOOK = "engine/src/logbook/mod.rs"
CASES = "engine/src/logbook/cases.rs"
JOURNAL = "engine/src/logbook/journal.rs"
LEDGER = "engine/src/ledger.rs"
COMMANDS = "engine/src/commands/mod.rs"
PLAN = "engine/src/commands/plan.rs"
DECIDE = "engine/src/commands/decide.rs"
DRIFT = "engine/src/commands/drift.rs"
IMPORT = "engine/src/commands/import.rs"
TASK = "engine/src/commands/import/task.rs"
LOG = "engine/src/commands/log.rs"
EVENT = "engine/src/commands/event.rs"
DOSSIER = "engine/src/commands/dossier.rs"
VIEWS = "engine/src/index/views.rs"
REBUILD = "engine/src/commands/rebuild.rs"
RULES = "engine/src/commands/rules.rs"
SKILLS = "engine/src/commands/skills.rs"
SETUP = "engine/src/commands/setup.rs"


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


MUTANTS = [
    # the helper
    ("helper: any part accepted", LOGBOOK, plain(".all(|c| matches!(c, Component::Normal(_)))", ".all(|_| true)")),
    ("helper: one plain part enough", LOGBOOK, plain(".all(|c| matches!(c, Component::Normal(_)))", ".any(|c| matches!(c, Component::Normal(_)))")),
    ("helper: links followed", LOGBOOK, plain("match std::fs::symlink_metadata(&dir) {", "match std::fs::metadata(&dir) {")),
    ("helper: a link is a folder", LOGBOOK, plain("Ok(m) if m.file_type().is_dir() => {}", "Ok(m) if m.file_type().is_dir() || m.file_type().is_symlink() => {}")),
    ("helper: a file is a folder", LOGBOOK, plain("Ok(m) if m.file_type().is_dir() => {}", "Ok(m) if !m.file_type().is_symlink() => {}")),
    ("helper: nothing refused", LOGBOOK, plain("Ok(m) if m.file_type().is_dir() => {}", "Ok(_) => {}\n            #[allow(unreachable_patterns)]\n            Ok(m) if m.file_type().is_dir() => {}")),
    ("helper: link and file named the other way", LOGBOOK, plain("                    if m.file_type().is_symlink() {\n                        \"a symbolic link\"", "                    if !m.file_type().is_symlink() {\n                        \"a symbolic link\"")),
    ("helper: a missing part refused", LOGBOOK, plain("Err(e) if e.kind() == std::io::ErrorKind::NotFound => break,", "Err(e) if false && e.kind() == std::io::ErrorKind::NotFound => break,")),
    ("helper: an unreadable part accepted", LOGBOOK, plain("Err(e) => return Err(anyhow::anyhow!(\"{}: {e}\", dir.display()).into()),", "Err(_) => break,")),
    ("helper: exit 2, not 1", LOGBOOK, both(
        plain("                return Err(Error::user(format!(\n                    \"{} is {}, not a folder of the logbook;", "                return Err(Error::from(anyhow::Error::msg(format!(\n                    \"{} is {}, not a folder of the logbook;"),
        plain("                        \"no directory\"\n                    }\n                )));", "                        \"no directory\"\n                    }\n                ))));"),
    )),
    ("helper: the absolute path shown", LOGBOOK, plain("        shown.push(name);\n", "        shown = dir.clone();\n")),
    ("helper: the returned path is the root", LOGBOOK, plain("    Ok(root.join(relative))\n}", "    Ok(root.to_path_buf())\n}")),
    ("checked_file: folder not checked", LOGBOOK, plain("        if let Some(dir) = relative.parent() {\n            checked_dir(&self.root, dir)?;\n        }\n", "")),
    ("checked_file: the file checked, not its folder", LOGBOOK, plain("if let Some(dir) = relative.parent() {", "if let Some(dir) = Some(relative) {")),
    ("checked_file: a path outside taken", LOGBOOK, plain("            path.strip_prefix(&self.root).map_err(|_| {\n                anyhow::anyhow!(\n                    \"{} is not in the logbook {}\",\n                    path.display(),\n                    self.root.display()\n                )\n            })?", "            path.strip_prefix(&self.root).unwrap_or(path)")),
    ("open: a .seldon file is an engine error", LOGBOOK, plain("std::io::ErrorKind::NotFound | std::io::ErrorKind::NotADirectory", "std::io::ErrorKind::NotFound")),
    # the primitives
    ("ledger: append not checked", LEDGER, plain("        if let Some(root) = &self.root {\n            crate::logbook::checked_dir(root, Path::new(LEDGER_DIR))?;\n        }\n", "")),
    ("ledger: new without the root", LEDGER, plain("            root: Some(logbook.root.clone()),", "            root: None,")),
    ("write_new: not checked", COMMANDS, plain("    let path = &logbook.checked_file(path)?;\n", "")),
    ("case save: its folder not checked", CASES, plain("        logbook.checked_file(&self.path)?;\n        if !self.path.is_file() {", "        if !self.path.is_file() {")),
    ("case save: the target folder not checked", CASES, plain("logbook.checked_file(Path::new(\"work\").join(self.case.status.folder()).join(name))?;", "logbook.path(Path::new(\"work\").join(self.case.status.folder()).join(name));")),
    ("case folders: completed not checked", CASES, plain("        CaseStatus::Active,\n        CaseStatus::Completed,\n    ] {", "        CaseStatus::Active,\n    ] {")),
    ("case folders: active not checked", CASES, plain("        CaseStatus::Active,\n        CaseStatus::Completed,\n    ] {", "        CaseStatus::Completed,\n    ] {")),
    ("active case: set not checked", CASES, plain("        &logbook.checked_file(ACTIVE_CASE_FILE)?,", "        &logbook.path(ACTIVE_CASE_FILE),")),
    ("active case: clear not checked", CASES, plain("    let path = logbook.checked_file(ACTIVE_CASE_FILE)?;", "    let path = logbook.path(ACTIVE_CASE_FILE);")),
    ("area: not checked", CASES, plain("    let path = logbook.checked_file(&rel)?;\n    if path.is_file() {", "    let path = logbook.path(&rel);\n    if path.is_file() {")),
    ("journal: prepare not checked", JOURNAL, plain("    let path = logbook.checked_file(&rel)?;\n    let entry = JournalEntry {", "    let path = logbook.path(&rel);\n    let entry = JournalEntry {")),
    ("journal: ensure_day not checked", JOURNAL, plain("    logbook.checked_file(&rel)?;\n    let record = Journal {", "    let record = Journal {")),
    ("views: not checked", VIEWS, plain("    let path = logbook.checked_file(rel)?;", "    let path = logbook.path(rel);")),
    # the commands
    ("plan new: case folders not checked", PLAN, plain("    cases::checked_folders(logbook)?;\n    if let Some(area) = spec.area.as_deref() {", "    if let Some(area) = spec.area.as_deref() {")),
    ("plan new: area not checked first", PLAN, plain("    if let Some(area) = spec.area.as_deref() {\n        logbook.checked_dir(format!(\"areas/{area}\"))?;\n    }\n    if spec.start && spec.point {", "    if spec.start && spec.point {")),
    ("plan step: case folders not checked", PLAN, plain("    let lock = ctx.lock()?;\n    cases::checked_folders(&logbook)?;\n    let mut file = cases::find(&logbook, &args.id)?;\n    let from = file.case.status;", "    let lock = ctx.lock()?;\n    let mut file = cases::find(&logbook, &args.id)?;\n    let from = file.case.status;")),
    ("plan step: its folders not checked before the ledger", PLAN, plain("    logbook.checked_file(&file.path)?;\n    logbook.checked_dir(format!(\"work/{}\", to.folder()))?;\n", "")),
    ("plan step: .seldon not checked before the ledger", PLAN, plain("    if transition != Transition::Verify {\n        logbook.checked_file(crate::logbook::ACTIVE_CASE_FILE)?;\n    }\n", "")),
    ("plan set: case folders not checked", PLAN, plain("    cases::checked_folders(&logbook)?;\n    let mut file = cases::find(&logbook, &args.id)?;\n    open_only(&file, \"set\")?;", "    let mut file = cases::find(&logbook, &args.id)?;\n    open_only(&file, \"set\")?;")),
    ("decide: new not checked first", DECIDE, plain("    // before the next id is read from it (WP-168)\n    logbook.checked_dir(\"decisions\")?;\n", "")),
    ("decide: accept not checked", DECIDE, plain("    // rewrite out of the logbook (WP-168)\n    logbook.checked_dir(\"decisions\")?;\n", "    // rewrite out of the logbook (WP-168)\n")),
    ("drift: ledger not checked first", DRIFT, plain("    logbook.checked_dir(crate::ledger::LEDGER_DIR)?;\n", "")),
    ("drift: case folders not checked first", DRIFT, plain("        cases::checked_folders(&logbook)?;\n    }\n    if let Action::Explain", "    }\n    if let Action::Explain")),
    ("drift: area not checked first", DRIFT, plain("        logbook.checked_dir(format!(\"areas/{area}\"))?;\n    }\n    let built", "    }\n    let built")),
    ("drift: explain not checked before the ledger", DRIFT, plain("        logbook.checked_file(&file.path)?;\n        if let Some(area) = explain.area.as_deref() {", "        if let Some(area) = explain.area.as_deref() {")),
    ("log: case folders not checked", LOG, plain("    if args.case_id.is_some() {\n        cases::checked_folders(&logbook)?;\n    }\n", "")),
    ("event: case folders not checked", EVENT, plain("    if args.case_id.is_some() {\n        cases::checked_folders(&logbook)?;\n    }\n", "")),
    ("import: folders not checked first", IMPORT, plain("        for rel in IMPORT_FOLDERS {\n            logbook.checked_dir(rel)?;\n        }\n", "")),
    ("import: memory not among the folders", IMPORT, plain("    \"journal\",\n    \"memory\",\n    \"system\",\n", "    \"journal\",\n    \"system\",\n")),
    ("import: plan paths not checked", IMPORT, plain("    check_folders(&logbook, &plan)?;\n", "")),
    ("import: report not checked", IMPORT, plain("    let path = logbook.checked_file(report_path(SOURCE))?;", "    let path = logbook.path(report_path(SOURCE));")),
    ("import task: marker not checked", TASK, plain("        logbook.checked_file(&marker_rel)?;\n", "")),
    ("dossier: system not checked", DOSSIER, plain("Files::read(&logbook.checked_dir(\"system\")?,", "Files::read(&logbook.path(\"system\"),")),
    ("rebuild: outputs not checked", REBUILD, plain("    let path = logbook.checked_file(REL_PATH)?;", "    let path = logbook.path(REL_PATH);")),
    ("rules: archive not checked", RULES, plain("    let dir = logbook.checked_dir(\"archive\")?;", "    let dir = logbook.path(\"archive\");")),
    ("skills: archive not checked", SKILLS, plain("        let parent = crate::logbook::checked_dir(&self.root, Path::new(\"archive\"))?;", "        let parent = self.root.join(\"archive\");")),
    ("setup: kit folders not checked", SETUP, plain("let target = crate::logbook::checked_dir(root, &folder)?.join(entry.file_name());", "let target = root.join(&folder).join(entry.file_name());")),
]

check = "--check" in sys.argv
only = [a for a in sys.argv[1:] if a != "--check"]
env = dict(os.environ, CARGO_TARGET_DIR=TARGET)
cargo = ["cargo", "test", "--manifest-path", "engine/Cargo.toml", "--locked", "--no-fail-fast"]
runs = [
    cargo + ["--lib", "--", "logbook::", "commands::setup", "commands::skills", "commands::tests::write_new", "--test-threads=4"],
    cargo + ["--test", "linked_folders", "--", "--test-threads=4"],
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
            r = subprocess.run(cmd, cwd=WT, env=env, capture_output=True, text=True)
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
