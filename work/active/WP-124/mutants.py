#!/usr/bin/env python3
"""WP-124 mutants: each patch must make its test fail. Restores every file.

Run from anywhere: python3 work/active/WP-124/mutants.py [M1 M15 …]
The repository root comes from this file's place; the build goes to
engine/target/mutants (its own CARGO_TARGET_DIR, so a parallel `just check`
is not disturbed).
"""
import os
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
A = "engine/src/commands/agent.rs"
T = "engine/src/commands/triage.rs"
D = "engine/src/commands/drift.rs"
C = "engine/src/commands/mod.rs"
I = "engine/src/index/triage.rs"
TRIAGE = ["--test", "triage"]
LIB = ["--lib"]

# (name, file, old, new, cargo test selection, test name filter)
M = [
    ("M1 subject in the drift prompt", A,
     "            (Ask::Drift(id), None)\n",
     "            (Ask::Drift(format!(\"{id} {}\", event.event.subject)), None)\n",
     TRIAGE, "an_ask_prompt_holds_no_logbook_text"),
    ("M2 case title in the case prompt", A,
     "            cases::find(&logbook, &id)?;\n            (Ask::Case(id), None)",
     "            let t = cases::find(&logbook, &id)?.case.title;\n            (Ask::Case(format!(\"{id} {t}\")), None)",
     TRIAGE, "an_ask_prompt_holds_no_logbook_text"),
    ("M3 SELDON_CASE inherited", A,
     "        None => cmd.env_remove(CASE_ENV),",
     "        None => &mut cmd,",
     TRIAGE, "ask_launches_with_ids_only_and_no_case"),
    ("M4 crisis by the file's flag only", T,
     "if (sel.crisis() || item.crisis) && !by_name",
     "if item.crisis && !by_name",
     TRIAGE, "the_engine_decides_a_crisis_not_the_file"),
    ("M5 crisis by the engine only", T,
     "if (sel.crisis() || item.crisis) && !by_name",
     "if sel.crisis() && !by_name",
     TRIAGE, "the_engine_decides_a_crisis_not_the_file"),
    ("M6 the file's text in the detail", T,
     "            Ok(text) => resolved.push((r.kind, r.reference.clone(), text)),",
     "            Ok(text) => resolved.push((r.kind, r.reference.clone(), r.text.clone().unwrap_or(text))),",
     TRIAGE, "apply_reads_the_evidence_again_never_the_file_s_text"),
    ("M7 apply trusts the file's evidence", T,
     "            Ok(text) => resolved.push((r.kind, r.reference.clone(), text)),\n            Err(why) => {",
     "            Ok(text) => resolved.push((r.kind, r.reference.clone(), text)),\n            Err(_) => resolved.push((r.kind, r.reference.clone(), r.text.clone().unwrap_or_default())),\n            #[allow(unreachable_patterns)]\n            Err(why) => {",
     TRIAGE, "apply_reads_the_evidence_again_never_the_file_s_text"),
    ("M8 self-evidence allowed", T,
     "        if members.iter().any(|m| m.id == id) {",
     "        if members.iter().any(|m| m.id == id) && false {",
     TRIAGE, "a_proposal_is_refused_whole_and_names_the_item"),
    ("M9 agent may apply", T,
     "    if is_agent(&actor) {\n        return Err(Error::user(format!(\n            \"{actor} may propose",
     "    if is_agent(&actor) && false {\n        return Err(Error::user(format!(\n            \"{actor} may propose",
     TRIAGE, "apply_and_discard_are_the_user_s_and_check_the_file"),
    ("M10 applied rewritten on every run", T,
     "let first = named.is_empty() && proposal.applied.is_none();",
     "let first = named.is_empty();",
     TRIAGE, "apply_resolves_as_the_user_holds_crises_back_and_is_idempotent"),
    ("M11 human may propose", T,
     "    if !is_agent(&actor) {\n        return Err(Error::user(format!(\n            \"a proposal is an agent's",
     "    if !is_agent(&actor) && false {\n        return Err(Error::user(format!(\n            \"a proposal is an agent's",
     TRIAGE, "a_proposal_is_refused_whole_and_names_the_item"),
    ("M12 routine items proposable", T,
     "let classified = crate::reconcile::item_of(built, e).filter(|_| open);",
     "let classified = crate::reconcile::item_of(built, e);",
     TRIAGE, "a_proposal_is_refused_whole_and_names_the_item"),
    ("M13 member not stored as leader", T,
     "let leader = classified.item.event_id.clone();",
     "let leader = e.id.to_string();",
     TRIAGE, "a_proposal_is_stored_with_the_engine_s_text_and_crisis"),
    ("M14 an unapplied proposal kept", T,
     "        if let Err(e) = std::fs::remove_file(dir.join(format!(\"{id}.json\"))) {",
     "        if let Err(e) = Ok::<(), String>(()) {",
     TRIAGE, "a_new_proposal_replaces_the_unapplied_one_and_says_so"),
    # round 2
    # the leader check and the member filter cover each other (an open
    # leader's item has only open members, and a closed leader's item none):
    # each alone is an equivalent mutant, so M15 removes both
    ("M15 B1 apply on what is no longer open", T,
     "    if !built.open_drift.contains(&sel.event.event.id) {\n        return Outcome::Skipped(closed_reason(built, sel.event));\n    }\n    // the change itself is no evidence: every event of it counts, also a\n    // member the open-only filter drops or the engine resolved (N8)\n    let linkable = the_change(built, &sel.event.event);\n    sel.members.retain(|m| built.open_drift.contains(&m.id));",
     "    let linkable = the_change(built, &sel.event.event);",
     TRIAGE, "apply_leaves_a_change_that_is_no_longer_open_alone"),
    ("M16 B2 self-citation allowed", T,
     "        if found.authors.iter().any(|a| a == self.proposer) {",
     "        if found.authors.iter().any(|a| a == self.proposer) && false {",
     TRIAGE, "an_agent_cannot_cite_its_own_words"),
    ("M17 B2 no author in the text", T,
     "        Ok(clip(&format!(\"by {} · {words}\", found.label), TEXT_MAX))",
     "        Ok(clip(&words, TEXT_MAX))",
     TRIAGE, "an_agent_cannot_cite_its_own_words"),
    ("M18 B2 Plan authors without the case's agents", T,
     "        for a in workers {\n            if !authors.contains(a) {",
     "        for a in workers.iter().take(0) {\n            if !authors.contains(a) {",
     TRIAGE, "an_agent_cannot_cite_its_own_words"),
    ("M19 N3 evidence text not redacted", T,
     "        let words = one_line_text(&self.redactor.redact(&found.words));",
     "        let words = one_line_text(&found.words);",
     TRIAGE, "evidence_and_titles_are_redacted"),
    ("M20 N3 title not redacted at apply", T,
     "    }\n    .redacted(redactor);",
     "    };",
     TRIAGE, "evidence_and_titles_are_redacted"),
    ("M21 N3 the proposal write follows a link", T,
     "    sys::write_atomic_replace(path, text.as_bytes(), sys::NEW_FILE_MODE)?;",
     "    sys::write_atomic_mode(path, text.as_bytes(), sys::NEW_FILE_MODE)?;",
     LIB, "a_proposal_is_written_over_a_link_not_through_it"),
    ("M22 N3 a foreign seldon/ folder as the guide", A,
     "        if matches!(skills::state(&folder), State::Missing | State::Foreign) {",
     "        if matches!(skills::state(&folder), State::Missing) {",
     TRIAGE, "a_foreign_seldon_folder_is_no_guide"),
    ("M23 N2 any path in the prompt", A,
     "    prompt_path(\"the logbook's path\", &logbook.root)?;",
     "",
     TRIAGE, "ask_refuses_a_path_a_prompt_must_not_carry"),
    ("M24 N4 separators pass one_line", C,
     "    if text.contains(['\\n', '\\r']) || text.chars().any(is_line_breaking) {",
     "    if text.contains(['\\n', '\\r']) {",
     TRIAGE, "separators_and_bidi_controls_are_refused"),
    ("M25 N5 a linked proposals folder accepted", T,
     "        Ok(m) if m.file_type().is_dir() => Ok(dir),",
     "        Ok(_) => Ok(dir),\n        #[allow(unreachable_patterns)]\n        Ok(m) if m.file_type().is_dir() => Ok(dir),",
     TRIAGE, "a_linked_proposals_folder_is_refused"),
    ("M26 N5 the index reads through a linked folder", I,
     "    if std::fs::symlink_metadata(&dir).is_ok_and(|m| !m.file_type().is_dir()) {",
     "    if false {",
     TRIAGE, "a_linked_proposals_folder_is_refused"),
    ("M27 N6 no commit after a case-file failure", D,
     "    let case_error = after().err();",
     "    after()?;\n    let case_error = None;",
     TRIAGE, "a_write_that_fails_after_its_ledger_line_is_committed_and_reported"),
    # round 3
    ("M28 B4 no proposed-by tag", D,
     "                tags.push(format!(\"{}{agent}\", super::triage::PROPOSED_BY));",
     "                let _ = agent;",
     TRIAGE, "an_applied_explanation_keeps_its_proposer_s_name"),
    ("M29 B4 the ledger forgets the proposer", T,
     "                .and_then(|d| d.strip_prefix(\"proposed by \"))",
     "                .and_then(|d| d.strip_prefix(\"never \"))",
     TRIAGE, "an_applied_explanation_keeps_its_proposer_s_name"),
    ("M30 B4 a case line by its own actor", T,
     "        if e.kind.as_str().starts_with(\"case-\") {",
     "        if false {",
     TRIAGE, "an_applied_explanation_keeps_its_proposer_s_name"),
    ("M31 N8 the transaction is not the change", T,
     "    if let Some(tx) = e.tx_id.as_deref() {",
     "    if let Some(tx) = None::<&str> {",
     TRIAGE, "a_dropped_member_is_still_no_evidence"),
    ("M32 N9 no worked-by label", T,
     "            label = format!(\"{label} (worked by {})\", workers.join(\", \"));",
     "",
     TRIAGE, "a_plan_line_names_who_worked_the_case"),
    ("M33 N10 two crises in one run", T,
     "        if crises.len() > 1 {",
     "        if crises.len() > 9 {",
     TRIAGE, "two_crises_in_one_run_are_refused"),
    ("M34 N11 markedApplied means done", T,
     "            \"markedApplied\": first,",
     "            \"markedApplied\": !done.is_empty(),",
     TRIAGE, "applied_marks_the_run_not_the_items"),
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
            print(f"{name}: {'killed' if killed else 'SURVIVED'}")
            if not killed:
                survivors.append(name)
        finally:
            open(path, "w").write(orig)
    print("survivors:", survivors)


if __name__ == "__main__":
    main()
