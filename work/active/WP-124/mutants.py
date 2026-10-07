#!/usr/bin/env python3
"""WP-124 mutants: each patch must make its test fail. Restores every file."""
import subprocess, sys, os
ROOT = "/home/eandres/Work/johnandrewsx/jax-seldon/wt/WP-124"
A = "engine/src/commands/agent.rs"
T = "engine/src/commands/triage.rs"
M = [
 ("M1 subject in the drift prompt", A,
  "            (Ask::Drift(id), None)\n",
  "            (Ask::Drift(format!(\"{id} {}\", event.event.subject)), None)\n",
  "an_ask_prompt_holds_no_logbook_text"),
 ("M2 case title in the case prompt", A,
  "            cases::find(&logbook, &id)?;\n            (Ask::Case(id), None)",
  "            let t = cases::find(&logbook, &id)?.case.title;\n            (Ask::Case(format!(\"{id} {t}\")), None)",
  "an_ask_prompt_holds_no_logbook_text"),
 ("M3 SELDON_CASE inherited", A,
  "        None => cmd.env_remove(CASE_ENV),",
  "        None => &mut cmd,",
  "ask_launches_with_ids_only_and_no_case"),
 ("M4 crisis by the file's flag only", T,
  "if (sel.crisis() || item.crisis) && !by_name",
  "if item.crisis && !by_name",
  "the_engine_decides_a_crisis_not_the_file"),
 ("M5 crisis by the engine only", T,
  "if (sel.crisis() || item.crisis) && !by_name",
  "if sel.crisis() && !by_name",
  "the_engine_decides_a_crisis_not_the_file"),
 ("M6 the file's text in the detail", T,
  "            Ok(text) => resolved.push((r.kind, r.reference.clone(), text)),",
  "            Ok(text) => resolved.push((r.kind, r.reference.clone(), r.text.clone().unwrap_or(text))),",
  "apply_reads_the_evidence_again_never_the_file_s_text"),
 ("M7 apply trusts the file's evidence", T,
  "            Err(why) => {\n                return Outcome::Refused(format!(\n                    \"evidence {} `{}` no longer resolves ({why})\",",
  "            Err(_) => resolved.push((r.kind, r.reference.clone(), r.text.clone().unwrap_or_default())),\n            Err(why) => {\n                return Outcome::Refused(format!(\n                    \"evidence {} `{}` no longer resolves ({why})\",",
  "apply_reads_the_evidence_again_never_the_file_s_text"),
 ("M8 self-evidence allowed", T,
  "        if members.iter().any(|m| m.id == id) {",
  "        if members.iter().any(|m| m.id == id) && false {",
  "a_proposal_is_refused_whole_and_names_the_item"),
 ("M9 agent may apply", T,
  "    if is_agent(&actor) {\n        return Err(Error::user(format!(\n            \"{actor} may propose",
  "    if is_agent(&actor) && false {\n        return Err(Error::user(format!(\n            \"{actor} may propose",
  "apply_and_discard_are_the_user_s_and_check_the_file"),
 ("M10 applied rewritten on every run", T,
  "let first = named.is_empty() && proposal.applied.is_none();",
  "let first = named.is_empty();",
  "apply_resolves_as_the_user_holds_crises_back_and_is_idempotent"),
 ("M11 human may propose", T,
  "    if !is_agent(&actor) {\n        return Err(Error::user(format!(\n            \"a proposal is an agent's",
  "    if !is_agent(&actor) && false {\n        return Err(Error::user(format!(\n            \"a proposal is an agent's",
  "a_proposal_is_refused_whole_and_names_the_item"),
 ("M12 routine items proposable", T,
  "let classified = crate::reconcile::item_of(built, e).filter(|_| open);",
  "let classified = crate::reconcile::item_of(built, e);",
  "a_proposal_is_refused_whole_and_names_the_item"),
 ("M13 member not stored as leader", T,
  "let leader = classified.item.event_id.clone();",
  "let leader = e.id.to_string();",
  "a_proposal_is_stored_with_the_engine_s_text_and_crisis"),
 ("M14 an unapplied proposal kept", T,
  "        if let Err(e) = std::fs::remove_file(dir.join(format!(\"{id}.json\"))) {",
  "        if let Err(e) = Ok::<(), String>(()) {",
  "a_new_proposal_replaces_the_unapplied_one_and_says_so"),
]
os.chdir(ROOT)
env = dict(os.environ)
failed_to_kill = []
for name, path, old, new, test in M:
    orig = open(path).read()
    if orig.count(old) != 1:
        print(f"{name}: pattern found {orig.count(old)} times"); failed_to_kill.append(name); continue
    try:
        open(path, "w").write(orig.replace(old, new))
        r = subprocess.run(["cargo", "test", "--manifest-path", "engine/Cargo.toml", "--test", "triage", test],
                           capture_output=True, text=True, env=env)
        killed = r.returncode != 0 and "test result: FAILED" in r.stdout
        if r.returncode != 0 and not killed:
            print(f"{name}: did not compile or other error:\n{r.stderr[-1500:]}")
        print(f"{name}: {'killed' if killed else 'SURVIVED'}")
        if not killed: failed_to_kill.append(name)
    finally:
        open(path, "w").write(orig)
print("survivors:", failed_to_kill)
