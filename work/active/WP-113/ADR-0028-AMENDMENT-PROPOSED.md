# ADR-0028 — proposed amendment (WP-113)

**Status:** proposed by the WP-113 Engine Dev, 2026-10-07. Not accepted;
nothing below is implemented as a classification change. ADR-0028 is
accepted and immutable: if the orchestrator takes these items, they go
into an amending ADR (number assigned by the orchestrator) that names
the rows it changes.

Items A and B change rows of ADR-0028 §2 and wait for that decision.
Item C is recorded for completeness: the operator approved it on
2026-10-06 (0.2.0 form of WP-113) and WP-113 implements it.

## A — narrow `system-link` (WP-109 stage 2) — *open, not implemented*

**Problem.** Row `config-* with meta.matches = "system-link"` makes any
symlink under a watched path whose resolved target lies under `/usr/`
routine. Hooks run `bash <target>` and `uwsm/env` is sourced, so a link
to *any* root-owned script under `/usr/` (an interpreter, a tool that
takes its arguments from the environment) runs at hook or login time and
is silenced by the mark. Root-owned content is not user persistence, but
a user-chosen link *to* it is.

**Proposed row text.** `config-*` with `meta.matches = "system-link"`:
the subject is a symlink under `~/.config/systemd/user/**` whose
resolved target lies under `/usr/lib/systemd/user/`, or under
`~/.config/autostart/**` whose resolved target lies under
`/usr/share/applications/` → **routine** `system-link`. No mark anywhere
else. (Open question: `/etc/xdg/autostart/` is the XDG system autostart
location; today's mark never covers it because it is not under `/usr/`,
and this proposal keeps that.)

**What changes.** The capture writes the mark only for those pairs
(`collectors/config.rs` `evidence()`). The classifier is unchanged (it
reads the mark). Old events keep the mark they have and classify as
before (§5: marks are capture-time facts; nothing is rewritten). A link
elsewhere (hooks, `environment.d`, `uwsm`, `.profile`) into `/usr/` then
falls through to `alwaysRedPaths` → crisis, unless its content is
Omarchy's shipped copy (`omarchy-default` by content still applies: a
link to `$OMARCHY_PATH/config/uwsm/env` is routine by that row).

**Evidence checked (dev host, read-only).** Omarchy's migrations
`1785095882.sh`, `1785167800.sh`, `1786539345.sh` link units into
`~/.config/systemd/user/…wants/` with targets under
`/usr/lib/systemd/user/` — covered. **Not checked:** what the
`--user enable` of a packaged unit and Omarchy's install scripts link to
(a read-only `grep` for it was blocked by the repository's guard hook
because the pattern named the service manager; reported, not routed
around). The autostart prefix needs the same check.
WP-D's measured follow-up on the test host is the place to confirm.

**Test sketch.** `drift_classes.rs`: a link `hooks/post-update.d/x ->
/usr/bin/env` → crisis; `systemd/user/default.target.wants/u.service ->
/usr/lib/systemd/user/u.service` → routine `system-link`; `autostart/
a.desktop -> /usr/share/applications/a.desktop` → routine; `uwsm/env ->
/usr/share/doc/…` → crisis.

## B — a toggle turned off is routine — *open; evidence already captured*

**Problem.** `omarchy-hyprland-toggle <flag> off` deletes
`~/.local/state/omarchy/toggles/hypr/<flag>.lua`. With the toggles
directory watched (WP-113, operator-approved), that is a `config-remove`,
which the row "`config-remove` of any non-routine path → attention"
makes quiet attention. ADR-0028 §1 calls a toggle routine ("reversible
in one step from the desktop … a toggle"); turning one on is routine by
`omarchy-default` evidence, turning it off is not. Every *off* of a
gaps or aspect-ratio toggle would be one quiet drift item.

**Proposed row text** (added after the `omarchy-default` row):
`config-remove` with `meta.matches = "omarchy-default"` under
`~/.local/state/omarchy/toggles/**`: at capture the removed content
(`hashFrom`) equals Omarchy's shipped flag
`$OMARCHY_PATH/default/<app>/toggles/<rel>` (trusted tree, as the
`omarchy-default` row) → **routine** `omarchy-default`.

Deliberately limited to the toggles directory: elsewhere, removing a
copy of an Omarchy default can break a `require` in `hyprland.lua`, and
a removal is the reason test's business.

**What changes.** Nothing at capture: WP-113 already writes the mark on
such removals (append-only: a mark cannot be added to old lines later).
The classifier (`index/class.rs` `config()`, and the port in
`scripts/validate-fixtures.py`) reads the mark on a removal for this
directory. A three-line change plus a table-test row.

## C — `~/.ssh/authorized_keys` in the default `alwaysRedPaths` — *operator-approved, implemented*

The `alwaysRedPaths` row's default list gains `~/.ssh/authorized_keys`
(operator, 2026-10-06, WP-113 0.2.0 form: "only when `config.toml` opts
in (its change is a crisis under ADR-0028 §3 when no case covers it)").
It is no default watch path: the opt-in is adding it to `watchPaths`,
and until then the glob matches nothing. Recorded here because the row
names the default list; the class of the row is unchanged.
