# ADR-0019 — Green zone for agent commands no collector tracks

**Status:** accepted (clarifies ADR-0014 §2)
**Date:** 2026-10-01

## Context
ADR-0014 §2 assigns red and yellow to collector sources and "none" to
snapper, notes, case events and resolutions, and says a hook `command`
event "takes the zone of what the command would produce". It never
produces `green`, although the schema and the plugin colour legend have
it. WP-015 needed a green event for the fixture and asked for the
producer rule; WP-009 implements hook zones now.

## Decision
1. **Recording.** The hook records a mutating agent command whose effect
   no collector tracks (not a package, plugin, theme, service or update
   command per the shared parser; every path it names outside
   `watchPaths`; or `Edit`/`Write` on such a path) **only while a case is
   set** (`.seldon/active-case`, or `--case` on the generic hook). Without
   a case such commands are noise and are not recorded; SPEC-ENGINE §8 and
   ADR-0014 §4 are amended accordingly (today they record only commands
   targeting a watched path).
2. **Zone.** Such an event is **green**. Examples: `tee
   ~/.config/zed/settings.json` with `~/.config/zed` unwatched, `npm
   install` inside a project, `git push`. Green events are never drift
   (ADR-0012 §6) and never crisis; they exist so a case's trace shows what
   the agent did between the tracked changes.
3. A hook command that *would* produce a tracked change keeps the zone of
   that change (red for pacman, yellow for config under `watchPaths`, and
   so on), as ADR-0014 §2 says.

## Consequences
WP-009 (or its follow-up) implements the rule in the hook classifier;
`zone_for` gains the green case for `source: agent`. The fixture's 10-01
`tee` event (WP-015, recorded under C-2026-004) is the example. The
plugin's green stripe now has a producer.
