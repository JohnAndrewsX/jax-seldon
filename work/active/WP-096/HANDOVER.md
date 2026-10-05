```
WP-096 HANDOVER
Done: `seldon agent start` launches with SELDON_ACTOR=agent:<launcher name> (normalised; refused when nothing is left) and SELDON_ATTENDED=1, replacing the caller's values; `--json` reports `actor`. plan (new and every step), log, drift (link/explain/dismiss) and event take SELDON_ACTOR when --actor is absent; --actor wins and the variable is then not read; a refused value is exit 1 naming the variable and the allowed form, before anything is written; empty = unset. hook generic takes it when the payload has no "actor" (the payload wins). SPEC-ENGINE §3/§8, guides 04/05 en/de (re-stamped), CHANGELOG.
Not done: decide (it takes no --actor and writes no actor anywhere; nothing to default). Live check on the test host (not asked by the WP; the e2e test covers the chain with a stub launcher).
Verified by: flock /tmp/seldon-check.lock just check → exit 0, `check: ok` (run once, after the last mutant and the last code commit); cargo test --no-fail-fast: 0 failures; fmt + clippy -D warnings clean; docs-check ok; 26 mutants, all killed by the test meant for them (list below).
Learned: memory/pitfalls.md, section "WP-096"
Decisions needed: 4, none blocking (below)
Touched outside WP scope: engine/src/commands/hook.rs (the brief lists hook.rs as WP-092's, the WP lists `hook generic` as an output: ~20 lines in `generic()`, `GenericPayload`, the module doc and the Generic help line); engine/tests/hooks.rs (one helper field + one test); memory/pitfalls.md
```

Branch `wp/096-default-actor`, worktree `wt/WP-096`, from `bc296ba`. No
push, no PR. Commits (oldest first):

- `68eef58` engine: SELDON_ACTOR is the actor when --actor is absent (WP-096)
- `a53a6b0` engine: agent start sets SELDON_ACTOR and SELDON_ATTENDED (WP-096)
- `e3483a4` docs: SELDON_ACTOR and SELDON_ATTENDED in SPEC-ENGINE, guides 04/05, CHANGELOG (WP-096)
- `266277a` docs: de guides 04/05 for SELDON_ACTOR and SELDON_ATTENDED, re-stamped (WP-096)
- this commit: handover and pitfalls

No contract change: `agent:<slug>` is already in `event.schema.json`
(`$defs.actor`), and nothing new reaches the index.

## Design

**One resolver** (`commands/event.rs`): `actor_or_env(flag, parse,
default)` and `env_actor(parse)`. `--actor` (clap already checked it) wins
and the variable is not read at all, so a broken `SELDON_ACTOR` never
blocks an explicit call. Otherwise the variable goes through the same
parser as the flag (`parse_person` for plan/log/drift/hook generic,
`parse_actor` for event, which also allows `system`). Refusal message:
`SELDON_ACTOR (the actor when none is named): `<value>` is not an actor
(human, system, agent:<name> with a lowercase name)`, exit 1; non-UTF-8 has
its own message. An empty value counts as unset, like `SELDON_CONFIG` and
`SELDON_NOW` in the engine. Every command resolves the actor first, before
it opens the logbook (as clap did for the flag), so a refused value writes
nothing.

**`event` takes the variable after the ledger attribution.** The theme
hook (`seldon-theme-set.sh`) calls `seldon event theme theme-set` without
`--actor`, and it runs inside the agent's environment when the agent runs
`omarchy theme set`. Read first, the variable would have made that event
`agent:<launcher>` without a case and skipped the attribution that gives it
the agent command's actor *and case*. So: actor starts at `--actor` or
`system`; the existing attribution runs as before; the variable replaces
`system` only if the actor is still `system` afterwards. An explicit
`--actor system` is never replaced. `--case` with no `--actor` gets the
variable's actor.

**`hook generic`**: `"actor"` became optional. Payload wins; without it,
`SELDON_ACTOR`; with neither, or a value that is not human/agent, the hook
records nothing and says so on stderr (exit 0, as every agent hook).
`hook claude-code` and `hook session-stop` do **not** read the variable:
their actor is the harness's (`agent:claude-code`, or session-stop's
`--actor`), and their events must keep matching each other.

**Launcher actor** (`Launcher::actor`): `agent:` + the launcher name,
ASCII-lowercased, each run of other characters one `-`, none at either
end: `default` → `agent:default`, `omarchy` → `agent:omarchy`, `Claude
Code` → `agent:claude-code`, `Agent/Ünï` → `agent:agent-n`. A name with no
ASCII letter or digit (`___`, `ÄÖÜ`) is exit 1 **before** the lock and the
active case change. `launch()` takes the actor as a parameter, so
WP-101's `--new` and WP-095's `ask` cannot start an agent without it (the
compiler forces it). `SELDON_ATTENDED` is set to `1` and read nowhere in
the engine (grep: only `agent.rs` sets it).

## Tests (new)

- `engine/tests/actor_env.rs` (6): log (env, flag wins, unset, empty,
  one-line rule for an env agent), refusals for log/plan new/plan start/
  event with 4 bad values and nothing written, flag with a bad variable,
  every plan step + Log lines + `agents`, drift link/dismiss/explain,
  event (manual with `--case`, explicit `--actor system`, the theme-set
  attribution first, the variable as fallback).
- `engine/tests/hooks.rs` `generic::the_actor_defaults_to_seldon_actor`.
- `engine/tests/agent.rs`: `the_launched_agent_gets_its_actor_and_the_attended_marker`
  (stub launcher records both variables; caller's `SELDON_ACTOR=agent:caller`
  and `SELDON_ATTENDED=0` are replaced; named launcher `Claude Code`;
  `--launcher omarchy`; `___` refused, nothing launched, active case
  unchanged) and `the_launched_agents_writes_are_recorded_as_the_agent`
  (end to end: the stub launcher runs `seldon log`, `plan verify`, `plan
  done` without `--actor` after `agent start` returned → `note`,
  `case-verified`, `case-completed` all `agent:default`).
- Unit: `event::tests::the_actor_variable`, `agent::tests::the_actor_is_the_launcher_name_as_a_slug`.

## Mutants (26, all killed, none fails to compile)

Applied one at a time to the committed code, restored with `git checkout`
plus a touch, `cargo build` after the loop.

| # | Mutant | Killed by |
|---|---|---|
| M1 | variable never read | actor_env (5 tests) |
| M2 | empty value not unset | actor_env::log_takes_the_variable_without_actor |
| M3 | variable not checked | actor_env::a_refused_variable…, drift_resolutions… |
| M4 | variable wins over the flag | actor_env (4 tests) |
| M5 | event: variable before the attribution | actor_env::event_takes_the_variable_after_the_ledger_attribution |
| M6 | event: variable never applied | same |
| M7 | event: variable only without `--case` | same |
| M8 | event: variable read although `--actor` is given | actor_env::a_refused_variable… |
| M9 | log: one-line rule skips an env agent | actor_env::log_takes… |
| M10 | log ignores the variable | actor_env::log_takes…, a_refused… |
| M11 | plan new ignores it | actor_env::plan_steps…, a_refused… |
| M12 | plan steps ignore it | actor_env::plan_steps…, a_refused… |
| M13 | drift ignores it | actor_env::drift_resolutions…, drift_explain… |
| M14 | hook generic ignores it | hooks::generic::the_actor_defaults_to_seldon_actor |
| M15 | hook generic: variable wins over the payload | same |
| M16 | hook generic: variable not checked | same |
| M17 | launch without SELDON_ACTOR | agent::the_launched_agent_gets…, the_launched_agents_writes… |
| M18 | launch without SELDON_ATTENDED | agent::the_launched_agent_gets… |
| M19 | SELDON_ATTENDED=true instead of 1 | same |
| M20 | actor: no lowercasing | unit the_actor_is_the_launcher_name_as_a_slug |
| M21 | actor: runs not folded | same |
| M22 | actor: trailing `-` kept | same |
| M23 | actor: empty name not refused | agent::the_launched_agent_gets… |
| M24 | actor checked after the active case is set | same (active case unchanged) |
| M25 | `--json` without `actor` | same |
| M26 | caller's SELDON_ACTOR passed on | same |

## Decisions needed (none blocking)

1. **`agent:default` / `agent:omarchy` for the built-in launchers.** The
   default launcher is `omarchy agent prompt`; which agent that starts is
   Omarchy's choice (`omarchy-default-agent`), so the engine records the
   launcher's name, as the WP says. Alternative: read the default agent
   at launch (one more program run, coupled to Omarchy internals). The v2
   rules still tell agents to pass `--actor agent:<name>`, so the
   variable is only the fallback. Recommendation: keep.
2. **A launcher named `claude` gives `agent:claude`, while Claude Code's
   hook records `agent:claude-code`.** Both are correct per their source;
   a user who wants one name names the launcher `claude-code`. Guide 06
   could say so (not changed: not in scope).
3. **hook.rs is listed as WP-092's in the brief** but `hook generic` is an
   output of this WP. The change is local to `generic()` and
   `GenericPayload` (+ module doc and help line). Checked: branch
   `wp/092-hook-budget` (as of this handover) does not touch
   `engine/src/commands/hook.rs`, so no conflict there today.
4. **Ships with WP-100; the docs overlap.** WP-100's `AGENTS.md` says
   `seldon agent start` sets `SELDON_ATTENDED=1`; this branch makes that
   true. Checked: `wp/100-agent-rules-v2` also changes
   `docs/SPEC-ENGINE.md`, `docs/user/{en,de}/04-working-with-agents.md`,
   `docs/user/{en,de}/05-cli-reference.md` and `memory/pitfalls.md`.
   Whichever merges second: resolve the text conflicts, run `bash
   scripts/docs-check.sh --write` (both branches change help blocks), commit
   the en side, then re-stamp the two de pages with that commit (merge, do
   not rebase; pitfall WP-058).

## Round 2

Review 1 (Opus): APPROVE with F1 to F3; the four decisions accepted as
written. Stage 2 (Fable): APPROVE after round 2, with one sentence for
guide 04 (A1 below). Commits (oldest first):

- `9cd6be8` engine: test a refused SELDON_ACTOR on drift resolutions (WP-096)
- `a56a288` docs: name the commands that take SELDON_ACTOR; hook generic payload in SPEC (WP-096)
- `385ac64` docs: de guide 04 names the commands that take SELDON_ACTOR, re-stamped (WP-096)
- `effa0bc` docs: a terminal server does not pass SELDON_ACTOR and SELDON_ATTENDED on (WP-096)
- `a4bcd20` docs: de guide 04, a terminal server does not pass the variables on, re-stamped (WP-096)
- this commit: round 2 notes

- **F1** `drift dismiss <THEME> --only -- x` is now in the args loop of
  `actor_env::a_refused_variable_is_exit_1_and_writes_nothing`. The actor
  is resolved before the event is looked up, so the fresh logbook needs
  no drift. The reviewer's mutant O3 (`parse_actor` instead of
  `parse_person` in `drift::resolve`) was run against the committed code.
  `system` then passes the parser and the call fails on the unknown event
  without naming `SELDON_ACTOR`. Result: killed by that test. Restored
  with `git checkout` + touch + `cargo build`.
- **F2** Guide 04 en now says that `seldon log`, `plan`, `drift`, `event`
  and `hook generic` record the name when `--actor` (for `hook generic`,
  `"actor"`) is missing. It adds that `event` first takes the agent
  command it finds in the ledger, with that command's case. de is
  translated and re-stamped to `a56a288`.
- **F3** SPEC-ENGINE §8: `hook generic` takes
  `{"command","actor"?,"cwd","case"?,"startedAt"?}`.
- **A1 (stage 2)** Guide 04 en, after "Seldon itself never reads it.":
  "Both variables reach the agent only when the launcher starts the
  terminal; a terminal server (footclient, kitty --single-instance, a
  wezterm mux) reuses its own environment, and then only --actor names
  the agent." The wording is as given. Only `footclient`, `kitty
  --single-instance` and `--actor` are in backticks, as in the rest of
  the guide. de is translated and re-stamped to `effa0bc`.
- **Live check (orchestrator):** on the test host (Alacritty) and on the
  dev host (foot without server), `SELDON_ACTOR` and `SELDON_ATTENDED`
  reach the agent process through the `omarchy-launch-tui` → uwsm →
  terminal chain. This answers review question 3 for those two
  terminals. Terminal servers were not run live; A1 documents the
  limit.
- **Verified by:** `cargo test --test actor_env` 6/6; fmt and
  `clippy --all-targets -D warnings` clean; docs-check ok;
  `flock /tmp/seldon-check.lock just check` run once on `a4bcd20`, after
  the last code and docs commit: exit 0, `check: ok`, 0 failures. A first run, started before A1
  arrived, was stopped while it waited for the lock, so it never ran.
