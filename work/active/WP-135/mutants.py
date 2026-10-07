#!/usr/bin/env python3
"""WP-135 mutants: each patch must make its test fail. Restores every file.

Run from anywhere: python3 work/active/WP-135/mutants.py [M1 M5 …]
The repository root comes from this file's place; the build goes to
engine/target/mutants (its own CARGO_TARGET_DIR, so a parallel `just check`
is not disturbed).
"""
import os
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
D = "engine/src/commands/decide.rs"
T = ["--test", "decide_accept"]

# (name, file, old, new, cargo test selection, test name filter)
M = [
    ("M1 an agent actor accepts", D,
     "    if is_agent(&actor) {\n",
     "    if is_agent(&actor) && false {\n",
     T, "an_agent_never_accepts"),
    ("M2 --actor human in an agent's session accepts", D,
     "    if let Some(agent) = session {\n",
     "    if let Some(agent) = session.filter(|_| false) {\n",
     T, "an_agent_never_accepts"),
    ("M3 the refusal after the logbook is opened", D,
     "    let actor = user_actor(args.actor, &id)?;\n    let (config, logbook) = ctx.open_logbook()?;\n",
     "    let (config, logbook) = ctx.open_logbook()?;\n    let actor = user_actor(args.actor, &id)?;\n",
     T, "an_agent_never_accepts"),
    ("M4 a superseded decision is accepted", D,
     "    match decision.status {\n        DecisionStatus::Proposed => {}",
     "    match if decision.status == DecisionStatus::Superseded { DecisionStatus::Proposed } else { decision.status } {\n        DecisionStatus::Proposed => {}",
     T, "refuses_a_decision_that_is_not_proposed"),
    ("M5 an accepted decision is accepted again", D,
     "    match decision.status {\n        DecisionStatus::Proposed => {}",
     "    match if decision.status == DecisionStatus::Accepted { DecisionStatus::Proposed } else { decision.status } {\n        DecisionStatus::Proposed => {}",
     T, "a_second_accept_changes_nothing"),
    ("M6 the date stays", D,
     "    decision.date = ctx.now.date_naive();\n",
     "",
     T, "accepts_a_proposed_decision"),
    ("M7 no ledger line", D,
     "    let event = emit_one(&lock, &config, &logbook, event)?;\n",
     "",
     T, "accepts_a_proposed_decision"),
    ("M8 the ledger line as the session's actor", D,
     "        .actor(&actor)\n        .detail(format!(\"accepted: {}\", decision.title));",
     "        .actor(\"system\")\n        .detail(format!(\"accepted: {}\", decision.title));",
     T, "accepts_a_proposed_decision"),
    ("M9 another decision's file", D,
     "    if decision.id != id {\n",
     "    if decision.id != id && false {\n",
     T, "refuses_a_decision_that_is_not_proposed"),
    ("M10 DECISIONS.md not filled", D,
     "    let warnings = fill_index(&logbook);\n    let commit = autocommit(ctx, &config, &logbook, &format!(\"{id} accepted\"));",
     "    let warnings = Vec::new();\n    let commit = autocommit(ctx, &config, &logbook, &format!(\"{id} accepted\"));",
     T, "accepts_a_proposed_decision"),
    ("M11 no commit", D,
     "    let commit = autocommit(ctx, &config, &logbook, &format!(\"{id} accepted\"));",
     "    let commit = super::Commit::Skipped(\"mutant\");",
     T, "accepts_a_proposed_decision"),
    ("M12 no index rebuild", D,
     "    crate::index::rebuild_if_initialised(ctx);\n    drop(lock);\n    Ok(accept_output(",
     "    drop(lock);\n    Ok(accept_output(",
     T, "accepts_a_proposed_decision"),
    ("M13 the file not written", D,
     "    crate::sys::write_atomic(&path, doc.render().as_bytes())?;\n",
     "",
     T, "accepts_a_proposed_decision"),
]


def main():
    only = sys.argv[1:]
    os.chdir(ROOT)
    env = dict(os.environ, CARGO_TARGET_DIR=str(ROOT / "engine" / "target" / "mutants"))
    survivors = []
    for name, path, old, new, selection, test in M:
        if only and name.split()[0] not in only:
            continue
        orig = open(path).read()
        if orig.count(old) != 1:
            print(f"{name}: pattern found {orig.count(old)} times")
            survivors.append(name)
            continue
        try:
            open(path, "w").write(orig.replace(old, new))
            r = subprocess.run(
                ["cargo", "test", "--manifest-path", "engine/Cargo.toml", *selection, test],
                capture_output=True, text=True, env=env,
            )
            killed = r.returncode != 0 and "test result: FAILED" in r.stdout
            if r.returncode != 0 and not killed:
                print(f"{name}: did not compile or other error:\n{r.stderr[-1500:]}")
            print(f"{name}: {'killed' if killed else 'SURVIVED'}", flush=True)
            if not killed:
                survivors.append(name)
        finally:
            open(path, "w").write(orig)
    print("survivors:", survivors)


if __name__ == "__main__":
    main()
