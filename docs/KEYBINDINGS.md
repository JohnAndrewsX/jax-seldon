# KEYBINDINGS.md — Seldon keys, IPC and the suggested binding

The user-facing key reference lives in the plugin README, because only
`plugin/` ships in the plugin repository (ADR-0009) and users read it
there. This page points to it, so the keys are written down once.

| What | Where |
|---|---|
| Panel keys (`1`–`6`, ←/→ `h`/`l`, ↑/↓ `k`/`j`, Tab / Shift-Tab, Enter / Space, `x`, `a`, `f`/`F`, `c`, `n`, `+`, `d`, `e`, Esc), two-press arming, keys in fields and sheets | [plugin/README.md → Keys](../plugin/README.md#keys) |
| Prime Radiant keys (`1`–`4`, ←/→ `h`/`l`, Esc) | [plugin/README.md → Keys](../plugin/README.md#keys) |
| Pill clicks (left, middle, right) | [plugin/README.md → The pill](../plugin/README.md#the-pill) |
| IPC targets (`shell toggle\|summon\|call jax.seldon`, `jax.seldon.panel`, `jax.seldon.service`) and the suggested binding | [plugin/README.md → Configure](../plugin/README.md#configure) |

The normative source is [SPEC-PLUGIN.md](SPEC-PLUGIN.md) §5 (panel keys)
and §8 (IPC, suggested binding); the README follows it. A change to a key
updates the spec, the README table and the harness checks in
`tests/plugin/panel-view.sh` / `overlay-view.sh` together.

The suggested Hyprland binding is documented, never installed by the
plugin (AGENTS.md §6: no edits under `~/.config` outside the plugin
folder):

```
o.bind("SUPER + SHIFT + S", "Seldon", "omarchy-shell shell toggle jax.seldon")
```
