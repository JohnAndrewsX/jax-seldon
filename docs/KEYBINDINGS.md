# KEYBINDINGS.md — Seldon keys, IPC and the suggested binding

The normative source is [SPEC-PLUGIN.md](SPEC-PLUGIN.md) §5.3 (the desk's
keys) and §8 (IPC, suggested binding). From 0.2.0 the plugin's one surface
is the desk (ADR-0034); this page is the key reference for it. The user
guide ([Daily use](user/en/03-daily-use.md)) and
[plugin/README.md → Keys](../plugin/README.md#keys) follow it. A change to
a key updates the spec, this page, the README table, the guide and the
checks in `tests/plugin/desk-view.sh` together.

## The desk

| Key | Does |
|---|---|
| `1`–`8` | Today, Changelog, Work, Decisions, System, Memory, Prime Radiant, Graph |
| `,` | Settings |
| `Alt+↓` / `Alt+↑` | the next / previous of the nine, wrapping |
| `/` | the sidebar search: filters the current list; Enter leaves the field and keeps the filter, Esc clears it and leaves |
| `↑`/`↓`, `k`/`j` | move in the list; `↑` at the top of the list goes on to the notices' buttons on a fresh press (a held `↑` stops at the top) |
| `Enter`, `Space` | on a narrow desk: show the selected row's detail; otherwise the detail's first action that launches nothing (Work: arm, then run — on an active case *To verification*; open drift: its form; Today's yesterday row: open or close; a button the keys reached: arm, then run). Enter never starts an agent: on *Agent sorts* it does nothing, `a` twice starts it |
| `Esc` | leave a section's form, then clear the search filter, then go back to the list (narrow desk), then close |
| `c` | capture now |
| `n` | Today's note field |
| `+` | Work's new case |
| `i` | Today's New case field, Work's intent field |
| `e` | open in the editor: Today the journal, Changelog this month's ledger, Work the case, Decisions the decision, System STATUS.md, Memory the logbook |
| `f` / `F` | Changelog: the next / previous chip (open, crisis, attention, routine, in case, all) |
| `↑` / `k` on the first row | Changelog: the agent's triage proposal (the row above the list); `↓` / `j` back to the list. On the proposal, Enter arms *Apply proposals*, a second Enter applies it (on a narrow desk the first Enter shows its detail); `↓` walks its crises before the list (`↑` back onto the proposal), Enter arms the one under the cursor, a second Enter applies that crisis alone, as its *Apply this crisis* button does. `↑` on the proposal (or on the first row without one): *Agent sorts N open changes*, `a` twice starts the agent (Enter does nothing there) |
| `↑` / `k` on the first row | Today, while Seldon is not set up: the setup card's row; `←`/`→` pick a button of the current step, Enter twice runs it (on a narrow desk the first Enter shows the card) |
| `↑` / `k` on the first row | Work: the header's buttons, *By agent* first; `←`/`→` *By agent*, *Import tasks…*, *New case*, Enter twice runs the one under the cursor as its click does (none starts an agent); `↓` / `j` back to the first row, `↑` on to the notices |
| `↑` / `k` at the top of a list | on a fresh press (a held `↑` stops at the top) the notices' buttons: `↑`/`↓` the notices, `←`/`→` (`h`/`l`) a notice's buttons, Enter twice runs one (*Restart shell*, *Install*, *Update rules*, …); `↓` on the last notice or Esc back to the list |
| `Enter` on Appearance | Settings: the controls; `↑`/`↓` width slider, presets, sidebar, `←`/`→` the value (the width as a preview), Enter twice saves it, Esc back to the groups. On Capture the same reaches *Offer again* |
| `a` | Work: hand the case to the agent (twice; the only key that starts one); while an agent works on the case, *Focus*: bring its window to the front (once) |
| `x` | Work: drop the case (twice; final) |
| `r` | Work: reopen a completed case (once) |
| `d` | Decisions: the new-decision form (Enter twice creates) |
| `←`/`→`, `h`/`l` | Prime Radiant: the previous / next period (30 d, 90 d, 365 d, All; wrapping) |
| `←`/`→` | Graph: the cut-off day one back / on |
| `Space`, `Enter`, `p` | Graph: play the growth from the first day (on from the cut when it is earlier), again: pause |
| `0`, `-`, `=` | Graph: fit the view, zoom out, zoom in |

Decisions (`4`): *Accept* on a proposed decision arms on the first press
or click (*Confirm accept*) and runs `seldon decide accept` on the second;
`e` opens the decision in the editor. The Prime Radiant (`7`) starts
on 90 d every time you enter it; its 0.1 keys `1`–`4` are the sections
now. The graph (`8`) takes the mouse: drag a node or the background,
the wheel zooms, a hover shows the card with *Open case*; Esc pauses the
replay, then lets a kept card go, then closes.

↑/↓ move the selection; the detail follows. A text field keeps every key
while it has the focus; Esc in it gives the keys back and keeps the text.
Writing actions need a second press (the action bar shows the hint); any
other key cancels. Every button the keys reach above (a notice's, the
setup card's, Work's header buttons, a Settings control) takes Enter twice and runs what its
click runs (*Agent sorts* takes `a` twice); any other key, a click on the desk, a summon
or another selection cancels. A held key counts as one press: its repeats never
confirm, write or start anything; only the keys that move (arrows, page
keys, `j`/`k`, `h`/`l`, `-`/`=`) repeat.
Tab and Shift-Tab do nothing: the desk is not a bar popup.

## The pill

| Click | Does |
|---|---|
| left | open or close the desk |
| middle | open the desk at the Prime Radiant |
| right | capture now |

## IPC and the suggested binding

| Command | Does |
|---|---|
| `omarchy-shell shell toggle jax.seldon` | open or close the desk |
| `omarchy-shell shell summon jax.seldon '{"section":"work"}'` | open at a section (`today`, `changelog`, `work`, `decisions`, `system`, `memory`, `radiant`, `graph`, `settings`); `'{"period":"30"}'` opens the Prime Radiant |
| `omarchy-shell shell call jax.seldon view ""` | what the desk shows, as JSON (while it is open) |
| `omarchy-shell shell call jax.seldon section <id>` | show a section |
| `omarchy-shell shell summon jax.seldon '{"section":"changelog","filter":"crisis"}'` | the Changelog at a chip (`open`, `crisis`, `attention`, `routine`, `case`, `all`); `"select":"<eventId>"` selects an event |
| `omarchy-shell shell call jax.seldon select <id>` | select an item of the current section (an event, a case, a decision id, …; the Changelog: `proposal`, the agent's triage proposal; the graph: keep that node's card); put the keys on a button, never pressing it: `notices`, `notice:<id>` (any section), `setup` (Today), `ask` (Changelog, *Agent sorts*), `header` (Work, *By agent*), `width`, `presets`, `sidebar`, `offer` (Settings) |
| `omarchy-shell shell call jax.seldon setPeriod 30` | show the Prime Radiant on a period (`30`, `90`, `365`, `all`); an unknown id changes nothing |
| `omarchy-shell shell call jax.seldon hover "heatmap 0.5,0.5"` | the read-out at that point of a Prime Radiant chart (fractions of its plot; `""` clears), while it is shown |
| `omarchy-shell jax.seldon.panel open\|close\|show\|hide\|toggle\|tab <name>\|resolve <eventId\|crisis\|proposal>\|filter <source>\|view\|pill` | the 0.1 panel's target, forwarded to the desk until a later release removes it (announced in the CHANGELOG) |
| `omarchy-shell jax.seldon.service status\|refresh\|capture` | the service |

The suggested Hyprland binding is documented, never installed by the
plugin (AGENTS.md §6: no edits under `~/.config` outside the plugin
folder). Super+Shift+L (L for logbook) is free in Omarchy's default and
config bindings (operator decision E81; Super+Shift+S is Omarchy's
Google Maps key):

```
o.bind("SUPER + SHIFT + L", "Seldon", "omarchy-shell shell toggle jax.seldon")
```
