---
language: {{language}}
machineId: {{machineId}}
---
# {{machineId}}

## Purpose
What this machine is for, in a few lines. Every agent reads this file.

## Must not happen here
- No change in the red zone (packages, services, `/etc`, boot loader) without a case.
- No edits inside the Omarchy package under `/usr/share/omarchy`; customise under `~/.config` only.

## Who works here
- Me (`human`)
- Agents, by the name they use in `--actor`, e.g. `agent:claude-code`
