# schema/external/

Schemas for **third-party input formats** the engine reads: snapper, `omarchy plugin
list --json`, `omarchy plugin catalog`, Claude Code hook payloads. They pin what
was observed on the dev host (Omarchy 4.0.4) so fixture drift is noticed. They are
**not** part of the engine ⇄ plugin contract and are not covered by
`contractVersion`; when the upstream format changes, update the schema and the
fixtures in `fixtures/logs/` or `fixtures/hooks/` (ADR-0012).
