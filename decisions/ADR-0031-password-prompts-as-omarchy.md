# ADR-0031 — Password prompts follow Omarchy: as few as the route allows

**Status:** accepted (operator decision 2026-10-06)
**Date:** 2026-10-06

> Amends [ADR-0027](ADR-0027-act-then-account.md) §1 (the target "at most
> one password prompt"). Everything else in ADR-0027 stands.

## Context

ADR-0027 §1 set the target for a case the user starts: one click, one
sentence, at most one password prompt, nothing at the end. The live test
of 2026-10-06 and the reviews of WP-094 and WP-111 showed what the
official route costs on Omarchy: an agent's tool calls have no terminal,
so a privileged step goes through `pkexec`, as Omarchy's own agent skill
says, and `pkexec` asks for the password on every command. An R2 install
(a snapshot for each snapper config, then the package) costs three to four
prompts.

Keeping "at most one" would need either a polkit rule that remembers the
authorisation (a host change a fresh Omarchy does not have, installed by
Seldon) or a Seldon root helper that runs several privileged steps under
one prompt (in effect a root shell for agent commands). Both leave
Omarchy's default and widen what runs as root.

## Decision

1. **Omarchy's way.** A privileged step asks for the password the way
   Omarchy does: `sudo` where the user's terminal shows the prompt,
   `pkexec` where it cannot, one program per `pkexec`. Seldon installs no
   polkit rule and no root helper, and never bundles privileged commands
   in `pkexec sh -c`.
2. **The target is "as few password prompts as the route allows".** It
   replaces ADR-0027 §1's "at most one". The live check counts prompts
   per case as before and reports them; more prompts than the route
   needs is a finding.
3. **Fewer prompts by design:** before an R2 or R3 step the agent takes
   the snapshot of the `root` snapper config only (where packages and
   system files change); other configs only when the case changes their
   files. A typical R2 install then costs two prompts (snapshot, package).

## Consequences

- The rules block (v4, WP-116), the skill and the guides say "as few
  password prompts as the route allows" and "snapshot `root`".
- Seldon stays on a fresh Omarchy exactly as Omarchy is: no privilege
  changes, nothing extra to trust.
- A user who wants fewer prompts can change polkit or sudo on their own
  machine; Seldon neither does it nor depends on it.
