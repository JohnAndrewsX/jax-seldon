# WP-110 HANDOVER

```
WP-110 HANDOVER
Done: ADR-0028 WP-B in the plugin: bar setting driftInBar (manifest enum crisis|all|none, default crisis, shell.json via setting()); pill D = crisis count by default; colour urgent on any crisis in every mode; neutral tooltip "Seldon — N active cases[, N crises], N changes without a case, last capture …"; red strip only for crises, new text; quiet "N changes without a case" line on the Changelog; open rows and drift sheet toned and labelled by crisis, not zone ("RESOLVE A CRISIS", "<zone> · crisis", "Crisis · no case" / "No case"); Today pictogram ignores attention; empty Ask agent slot first in the drift sheet; SPEC-PLUGIN §4/§5, plugin README, TESTING, CHANGELOG
Not done: the Ask agent button itself (WP-095, as the WP says); user guide pages (WP-C); schema description of crisis (WP-109)
Verified by: flock /tmp/seldon-check.lock just check → exit 0 (`check: ok`) on 1ab340c; omarchy plugin validate + qmllint before every commit; model.test.js 92 passed; bar-view 194, service-states 316, panel-view 827 passed, 0 failed; mutants 24/24 killed
Learned: memory/pitfalls.md, section "WP-110"
Decisions needed: none blocking; 8 taken (below), 3 open questions
Touched outside WP scope: memory/pitfalls.md; docs/TESTING.md (harness descriptions)
```

Branch `wp/110-quiet-surfaces`, worktree `wt/WP-110`, from `e4f6ce0`
(main). No push, no PR. Commits (oldest first):

- `df5652c` plugin: quiet surfaces — crisis count in the bar, driftInBar, labels keyed on crisis (WP-110)
- `2a50ab9` docs: SPEC-PLUGIN §4/§5, plugin README, TESTING and CHANGELOG for quiet surfaces (WP-110)
- `a967319` plugin: test that the attention count never goes negative (WP-110)
- `1ab340c` memory: pitfalls from WP-110
- this commit: handover

No contract change. The plugin reads only `summary` (`openDrift`,
`crisis`, `activeCases`) and `drift[]` (`crisis`, `zone`); no new engine
call, no new argv, no new file.

## What changed

**Setting (Omarchy first).** `driftInBar` lives where `wipLimit` and
`captureIntervalMin` live: `manifest.json` `barWidget.defaults` and
`barWidget.schema`, type `enum` with `options` — the same mechanism the
first-party `omarchy.agents` uses for `syncMode` (no private file; the
widget's `shell.json` entry hot-reloads; `omarchy bar set jax.seldon
driftInBar all`). `Model.driftInBarMode()` maps anything else to the
default. `BarWidget.qml` reads it with `setting()`, pushes it to the
service (`Service.setDriftInBar`, like `setCaptureInterval`) so the
service's IPC read-out shows the same pill; `Panel.qml` reads it for its
own `view().pill`.

**Model.js.**
- `counts()` adds `attention = max(0, openDrift − crisis)` (ADR-0028 §7).
- `pillText(c, mode)`: D = `crisis` / `drift` / nothing by mode.
- `pillTone` unchanged (urgent on any crisis, whatever the mode).
- `tooltipText`: the new neutral text; the crisis part only while > 0.
- `crisisText`: "N changes that can affect boot, login or the shell
  have no case" ("1 change … has no case").
- `attentionText` (new): "N changes without a case", "" at 0.
- `todayState`: crisis → urgent, else active → accent, else all clear.
- `driftTone(item)` (new): crisis → urgent, else accent. Used for open
  drift rows (`changelogRows`) and the sheet (`driftItemFor`); resolved
  and non-drift rows keep the event zone's tone.
- `driftItemFor`: zone fallback is the event's zone, no longer
  `crisis ? "red"` (zone is the ledger zone, §7).
- `rowStatus`: "Crisis · no case" / "No case" (was "Needs a reason" /
  "Unexplained").

**QML.** `ChangelogTab.qml`: `attentionLine` under the header (dim, wraps,
no action). `DriftSheet.qml`: `heading` and `zoneLabel` keyed on
`crisis`; `askAgentSlot` (empty `Item`, `visible: children.length > 0`)
as the first child of the form column, before the Link / Explain /
Dismiss group; `askSlotFirst` for the read-out. `Panel.qml` view():
`changelog.attention`, `drift.crisis|heading|zoneLabel|tone|askSlotFirst`.
Comments in `EventRow.qml`, `TodayTab.qml`, `BarWidget.qml`.

## Decisions (ADR silent; decided and carried on)

1. **Row and sheet colour by class, not zone.** §4b says attention rows are
   "marked as today" and the urgent colour is the crisis signal. With
   `drift[].zone` now the ledger zone, an attention `pacman -S htop` is red
   in the ledger; toning it urgent would put a red row in the panel for a
   quiet item. So open drift: crisis urgent, attention accent, whatever the
   zone; the zone text stays in the sheet. "Marked as today" is read as:
   bold, status note, *Resolve…*, as drift rows are marked today.
2. **Tooltip crisis part only while there is one** ("0 crises" reads as an
   alarm in a quiet bar). "Changes without a case" = attention
   (`openDrift − crisis`), so the two numbers are disjoint as in the ADR's
   example "1 crisis, 7 changes without a case".
3. **Changelog quiet line counts attention**, not all open drift, for the
   same reason; crises already have the strip. Its own line under the
   header, not appended to the header text.
4. **`none` hides the number, never the colour.** §4a: "colour … unchanged";
   a crisis still turns the glyph urgent with `none`.
5. **Row notes state, never ask:** "Crisis · no case" / "No case"
   (§3: a reason is never required). "RESOLVE DRIFT" stays the attention
   sheet's heading (the user opened it; the ADR names only the crisis one).
6. **Ask agent slot** is an empty, zero-height `Item` named
   `askAgentSlot` (alias on the sheet) as the first form child; WP-095 puts
   its button into it. No placeholder button, no disabled control.
7. **Today stats keep "open drift"** (the counts row: events today, 7 days,
   active, queued, open drift). The ADR names the pictogram only; see open
   question 1.
8. **Manifest label and description:** "Changes counted in the bar";
   options shown as the raw values `crisis` / `all` / `none` (the shell's
   enum renders options as given, like `Off` / `On` in omarchy.agents); the
   description explains each. The bar widget's description is now "Active
   cases and crises at a glance".

## Fixture rows for WP-109

The harness builds these from `index.sample.json` with `jq` (no fixture
change in this WP); WP-109's fixture should carry them so the plugin
tests can use them directly:

- **Crisis on a hook path, yellow ledger zone:** drift item and event
  `subject` `~/.config/omarchy/hooks/post-update.d/10-sync`, `zone`
  `"yellow"`, `crisis: true` (panel-view case 13b, model.test.js
  "zone is the ledger zone…").
- **Attention item red in the ledger:** a pacman `install` (`ollama`)
  with `zone: "red"`, `crisis: false` (same cases).
- **Attention alone:** `summary.crisis: 0` with open drift (case 13c).

**Cross-track note.** Many plugin expectations pin the current fixture's
counts (`summary.crisis 2`, `openDrift 4` → pill `2 · 2`, strip "2 changes
…", `driftTones` with `ollama urgent`, the drift-live click on the strip
opening the systemd unit). When WP-109's fixture lands (routine theme-set
and `-Syu` group leave `drift`, ollama becomes attention), these change
on main and need a follow-up update of `tests/plugin/*` — expected, as in
earlier fixture WPs.

## Tests

- `tests/plugin/model.test.js` (92 passed): `pillText` across all modes
  incl. unknown values; `driftInBarMode` and the manifest entry
  (enum, options, default, defaults) equal to the Model constants;
  tooltip incl. crisis > drift never negative; strip singular/plural and
  empty without a crisis; `attentionText`; rows toned by crisis with
  zones removed and swapped; `driftItemFor` yellow crisis / red attention,
  Explain pre-fills the ledger zone; `todayState` attention alone.
- `tests/plugin/bar-view.sh` (194 passed): sample pill `2 · 2` in three
  themes × three scales (glyph centre and tint unchanged); accent/default
  tone cases run with `driftInBar` `all` to keep digits; new: modes
  default/crisis/all/none/unknown (text, mode, urgent, tooltip, service
  read-out), `none` + crisis + no case = urgent glyph alone, attention
  alone = glyph alone in the bar foreground. Harness: `HARNESS_SETTINGS`
  (bar.qml and panel.qml), report gains `service.pill/driftInBar`.
- `tests/plugin/service-states.sh` (316 passed): pills and strip texts,
  plus `driftInBar` default and the tooltip.
- `tests/plugin/panel-view.sh` (827 passed): texts updated; drift-capped
  in `all` (`2 · 250`, "248 changes without a case"); new 13b (yellow
  crisis: strip, pictogram, pill, quiet line, tones, sheet labels, Explain
  zone, slot order, strip click) and 13c (attention alone: no strip, all
  clear / case active pictogram, no D, no "crisis" text).

## Mutants (24/24 killed)

Run on a copy of `plugin/ tests/plugin/ fixtures/ scripts/ assets/
schema/ engine/Cargo.toml` per mutant; an unmutated baseline passed in
each suite (node, panel, bar).

| Mutant | Killed by |
|---|---|
| driftTone always urgent / always accent / by zone again | model changelogRows |
| pill crisis mode counts drift; none counts crisis; mode not validated; default `all` | model pillText |
| Today drift-open pictogram back | model state pictograms |
| strip without crisis; strip singular verb | model crisisText |
| attention unclamped | model attentionText (added after it survived once) |
| attention line counts all drift | model attentionText |
| tooltip crisis always; tooltip counts all drift | model tooltipText |
| item zone red by crisis | model driftItemFor (ledger zone) |
| row note "Needs a reason" back | model changelogRows |
| sheet heading by zone; zone label ignores crisis; Ask slot after the actions | panel-view 13b / drift-sample |
| Changelog quiet line empty | panel-view 13b, 13c |
| Panel ignores driftInBar | panel-view drift-capped |
| Bar ignores driftInBar | bar-view accent/default |
| Bar does not push the mode to the service | bar-view mode-all service pill/mode |
| Bar `none` hides urgency | bar-view mode-none, none-crisis |

The "does not push" run also showed one `log has errors` on
`catppuccin-latte-out125` (a case the mutant does not affect; the
baseline and every other run were clean). Noted as a possible harness
flake under load; see "Check results".

## Check results

`flock /tmp/seldon-check.lock just check` on `1ab340c` (all code, tests
and docs of this WP; this handover adds only this file): **exit 0,
`check: ok`.** fmt-check, clippy (`-D warnings`, also `--features watch`)
clean; cargo test default and `--features watch` 1622 passed in total,
0 failed; check-packaging, install.test 209, deploy-test-host.test 190,
docs-check ok (429 links, 14 translated pages), schema-validate ok,
plugin-validate ok, qmllint ok (29 files, tokens ok); plugin-test:
model.test.js 92, real-home-guard.test 11, service-states 316,
panel-view 827, overlay-view 319, bar-view 194, all 0 failed. The
`catppuccin-latte-out125` log error seen in one mutant run did not
recur here. `CARGO_TARGET_DIR` unset (the worktree's own `engine/target`
on disk). No host change, no real `~/Seldon`, `~/.local/state/seldon` or
`~/.config` touched (the real-home guard passed in every harness).

## Open questions

1. Today's counts row still says "open drift" (crises + attention). Rename
   to "without a case" or split it? The ADR is silent; left unchanged.
2. Should `driftInBar` be offered in the Seldon panel too (e.g. a System tab
   line), or is the Omarchy settings UI / `omarchy bar set` enough? Left at
   the shell's own mechanism, per "Omarchy first".
3. Version bump to 0.1.4 (manifest + `PLUGIN_VERSION`) is left to the
   release WP; this WP stays at 0.1.3.
