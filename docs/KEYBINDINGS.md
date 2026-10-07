# KEYBINDINGS.md — Seldon keys, IPC and the suggested binding

The normative source is [SPEC-PLUGIN.md](SPEC-PLUGIN.md) §5.3 (the desk's
keys) and §8 (IPC, suggested binding). From 0.2.0 the plugin's one surface
is the desk (ADR-0034); this page is the key reference for it. The user
guide and the plugin README follow it in WP-126; until then
[plugin/README.md → Keys](../plugin/README.md#keys) still describes the 0.1
panel. A change to a key updates the spec, this page, the README table and
the checks in `tests/plugin/desk-view.sh` together.

## The desk

| Key | Does |
|---|---|
| `1`–`8` | Today, Changelog, Work, Decisions, System, Memory, Prime Radiant, Graph |
| `,` | Settings |
| `Alt+↓` / `Alt+↑` | the next / previous of the nine, wrapping |
| `/` | the sidebar search: filters the current list; Enter leaves the field and keeps the filter, Esc clears it and leaves |
| `↑`/`↓`, `k`/`j` | move in the list |
| `Enter`, `Space` | select the row under the cursor (on a narrow desk: show its detail) |
| `Esc` | leave a section's form, then clear the search filter, then go back to the list (narrow desk), then close |
| `c` | capture now |
| `n` | Today's note field |
| `+` | Work's new case |
| `i`, `e`, `a`, `r`, `f`/`F`, `x`, `d` | the section's own keys (intent field, open in editor, hand to agent, reopen, filter, drop, new decision; ADR-0034 §2) |
| `←`/`→`, `h`/`l` | Prime Radiant: the previous / next period (30 d, 90 d, 365 d, All; wrapping) |

Decisions (`4`): `e` opens the selected decision in the editor (*Accept*
on a proposed one does the same: set `status: accepted` there), `d` the
new-decision form (Enter twice creates). System (`5`) and Memory (`6`):
`e` opens STATUS.md and the logbook. The Prime Radiant (`7`) starts on
90 d every time you enter it; its 0.1 keys `1`–`4` are the sections now.

A text field keeps every key while it has the focus. Writing actions need
a second press (the action bar shows the hint); any other key cancels.
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
| `omarchy-shell shell call jax.seldon select <id>` | select an item of the current section (a decision id, …) |
| `omarchy-shell shell call jax.seldon setPeriod 30` | show the Prime Radiant on a period (`30`, `90`, `365`, `all`) |
| `omarchy-shell shell call jax.seldon hover "heatmap 0.5,0.5"` | the read-out at that point of a Prime Radiant chart (fractions of its plot; `""` clears), while it is shown |
| `omarchy-shell jax.seldon.panel open\|close\|toggle\|tab <name>\|view\|pill` | the 0.1 panel's target, forwarded to the desk until 0.3.0 |
| `omarchy-shell jax.seldon.service status\|refresh\|capture` | the service |

The suggested Hyprland binding is documented, never installed by the
plugin (AGENTS.md §6: no edits under `~/.config` outside the plugin
folder):

```
o.bind("SUPER + SHIFT + S", "Seldon", "omarchy-shell shell toggle jax.seldon")
```
