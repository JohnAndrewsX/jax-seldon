//! The pacman, snapper and omarchy collectors against `fixtures/logs/`
//! (WP-004; SPEC-ENGINE §4, ADR-0011, ADR-0013 §5, ADR-0014 §1).
//!
//! The golden test replays the fixture story and requires exactly the
//! pacman, snapper and omarchy lines of `fixtures/logbook/ledger/*.jsonl`,
//! ids aside (fixtures/README.md "Event ids"). Capture times are the
//! fixture's: the bench runs each collector at the `now` the fixture's
//! capture-time events carry (omarchy `update`, `snapshot-delete`).

mod support;

use std::io::Write as _;
use std::os::unix::fs::MetadataExt as _;

use seldon::collectors::omarchy::Omarchy;
use seldon::collectors::pacman::{self, Pacman, PacmanCursor};
use seldon::collectors::snapper::{NO_PERMISSIONS, Snapper};
use seldon::collectors::to_cursor;
use seldon::commands::doctor::SNAPPER_FIX;
use seldon::model::event::{Event, Kind, Source};
use support::{
    Bench, assert_golden, assert_schema_valid, cest, fixture, fixture_events, normalised,
    normalised_sorted, story, subjects,
};

mod collectors {
    use super::*;

    #[test]
    fn fixture_story_reproduces_the_ledger_fixture() {
        let b = story();
        for source in [Source::Pacman, Source::Snapper, Source::Omarchy] {
            let got = b.ledger_events(source);
            got.iter().for_each(assert_schema_valid);
            assert_eq!(
                normalised_sorted(&got),
                normalised_sorted(&fixture_events(source)),
                "{source} events differ from fixtures/logbook/ledger"
            );
            let golden: Vec<String> = got.iter().map(normalised).collect();
            assert_golden(&format!("{source}.jsonl"), &golden);
        }
    }

    #[test]
    fn pacman_offsets_match_the_fixture_readme() {
        let log = std::fs::read(fixture("logs/pacman.log")).unwrap();
        let parsed = pacman::parse(&log, 0, false, cest());
        assert_eq!(parsed.resume, 11159, "complete lines end at byte 11159");
        // the unterminated last line is never read, even with pacman idle
        assert!(
            parsed
                .txs
                .iter()
                .all(|t| t.lines.iter().all(|l| !l.name.starts_with("nv")))
        );

        // from init's baseline cursor (6129): exactly the ledger's events
        let mut b = Bench::new("offset");
        b.seed(&[Source::Agent]);
        let inode = std::fs::metadata(fixture("logs/pacman.log")).unwrap().ino();
        b.cursors.insert(
            "pacman",
            to_cursor(&PacmanCursor {
                inode,
                offset: 6129,
            }),
        );
        b.baseline = support::ts("2100-01-01T00:00:00+00:00"); // ignored with a cursor
        let out = b.run(&Pacman, "2026-10-01T17:05:00+02:00");
        assert_eq!(
            normalised_sorted(&out.events),
            normalised_sorted(&fixture_events(Source::Pacman))
        );
        assert_eq!(
            out.cursor,
            Some(to_cursor(&PacmanCursor {
                inode,
                offset: 11159
            }))
        );
    }

    #[test]
    fn pacman_rotation_restarts_and_dedupes() {
        let mut b = Bench::new("rotation");
        b.seed(&[Source::Agent]);
        let log = b.scratch.path("pacman.log");
        b.sources.pacman_log = log.clone();
        std::fs::copy(fixture("logs/pacman-rotation/pacman.log.1"), &log).unwrap();
        let first = b.run(&Pacman, "2026-09-20T12:00:00+02:00");
        assert_eq!(subjects(&first.events), ["btop", "omarchy"]);
        assert_eq!(
            first.cursor.as_ref().unwrap()["offset"],
            7359,
            "cursor at the end of the old file"
        );

        // logrotate: the old file keeps its inode as .1, a new file starts
        // by repeating the 09-15 transaction (copytruncate race)
        std::fs::rename(&log, b.scratch.path("pacman.log.1")).unwrap();
        std::fs::copy(fixture("logs/pacman-rotation/pacman.log"), &log).unwrap();
        let second = b.run(&Pacman, "2026-10-01T17:05:00+02:00");
        assert!(
            !second
                .events
                .iter()
                .any(|e| e.subject == "omarchy" && e.meta.to.as_deref() == Some("4.0.6-1")),
            "the repeated 09-15 upgrade is not emitted twice"
        );
        assert_eq!(second.events.len(), 10);
        assert_eq!(
            normalised_sorted(&b.ledger_events(Source::Pacman)),
            normalised_sorted(&fixture_events(Source::Pacman))
        );
        assert_eq!(b.run(&Pacman, "2026-10-01T17:06:00+02:00").events.len(), 0);
    }

    #[test]
    fn pacman_rotation_reads_the_rest_of_the_old_file_first() {
        let mut b = Bench::new("rotation-tail");
        let log = b.scratch.path("pacman.log");
        b.sources.pacman_log = log.clone();
        let old = std::fs::read(fixture("logs/pacman-rotation/pacman.log.1")).unwrap();
        // the first capture sees the old file up to the btop transaction
        let cut = find(&old, b"[2026-09-03T21:30:00");
        std::fs::write(&log, &old[..cut]).unwrap();
        assert_eq!(
            subjects(&b.run(&Pacman, "2026-09-04T00:00:00+02:00").events),
            ["btop"]
        );
        // pacman appends the 09-15 transaction, then the log is rotated
        std::fs::OpenOptions::new()
            .append(true)
            .open(&log)
            .unwrap()
            .write_all(&old[cut..])
            .unwrap();
        std::fs::rename(&log, b.scratch.path("pacman.log.1")).unwrap();
        std::fs::copy(fixture("logs/pacman-rotation/pacman.log"), &log).unwrap();
        let out = b.run(&Pacman, "2026-10-01T17:05:00+02:00");
        let omarchy: Vec<_> = out
            .events
            .iter()
            .filter(|e| e.subject == "omarchy")
            .collect();
        assert_eq!(omarchy.len(), 2, "4.0.6 once (from the old file) and 4.0.7");
        assert_eq!(out.events.len(), 11);
    }

    #[test]
    fn pacman_holds_a_running_transaction_back() {
        let mut b = Bench::new("buffer");
        let log = b.scratch.path("pacman.log");
        b.sources.pacman_log = log.clone();
        let head = "[2026-10-01T10:00:00+0200] [PACMAN] Running 'pacman -S zed'\n";
        let open = "[2026-10-01T10:00:01+0200] [ALPM] transaction started\n\
                    [2026-10-01T10:00:02+0200] [ALPM] installed alsa-lib (1.2.14-1)\n";
        std::fs::write(&log, format!("{head}{open}")).unwrap();
        std::fs::write(&b.sources.pacman_db_lock, "").unwrap();
        let out = b.run(&Pacman, "2026-10-01T10:00:03+02:00");
        assert!(out.events.is_empty(), "pacman still holds db.lck");
        assert_eq!(
            out.cursor.as_ref().unwrap()["offset"],
            0,
            "cursor stays before the Running line"
        );

        let rest = "[2026-10-01T10:00:03+0200] [ALPM] installed zed (0.198.4-1)\n\
                    [2026-10-01T10:00:03+0200] [ALPM] transaction completed\n";
        std::fs::OpenOptions::new()
            .append(true)
            .open(&log)
            .unwrap()
            .write_all(rest.as_bytes())
            .unwrap();
        let out = b.run(&Pacman, "2026-10-01T10:00:04+02:00");
        assert_eq!(subjects(&out.events), ["alsa-lib", "zed"]);
        assert_eq!(out.events[1].meta.command.as_deref(), Some("pacman -S zed"));
        assert_eq!(out.events[0].explicit, Some(false));
        assert_eq!(out.events[1].explicit, Some(true));

        // an interrupted transaction (no completion line, lock gone) is emitted
        let crash = "[2026-10-01T11:00:00+0200] [ALPM] transaction started\n\
                     [2026-10-01T11:00:01+0200] [ALPM] removed btop (1.4.5-1)\n";
        std::fs::OpenOptions::new()
            .append(true)
            .open(&log)
            .unwrap()
            .write_all(crash.as_bytes())
            .unwrap();
        assert!(
            b.run(&Pacman, "2026-10-01T11:00:02+02:00")
                .events
                .is_empty()
        );
        std::fs::remove_file(&b.sources.pacman_db_lock).unwrap();
        let out = b.run(&Pacman, "2026-10-01T11:00:03+02:00");
        assert_eq!(subjects(&out.events), ["btop"]);
        assert_eq!(out.events[0].explicit, None, "no command line, no explicit");
    }

    #[test]
    fn attribution_needs_a_command_that_names_the_package() {
        let mut b = Bench::new("attribution");
        let log = b.scratch.path("pacman.log");
        b.sources.pacman_log = log.clone();
        let hook = |ts: &str, actor: &str, case: Option<&str>, command: &str| {
            let mut e = Event::new(support::ts(ts), Source::Agent, Kind::Command, "x")
                .actor(actor)
                .case(case.map(String::from));
            e.meta.command = Some(command.into());
            e
        };
        let lock = &b.lock;
        b.ledger
            .append(
                lock,
                vec![
                    // names another package: proximity alone is no proof
                    hook(
                        "2026-10-01T09:59:00+02:00",
                        "agent:codex",
                        None,
                        "sudo pacman -S btop",
                    ),
                    // names zed, 11 minutes before the second transaction
                    hook(
                        "2026-10-01T10:49:00+02:00",
                        "agent:claude-code",
                        Some("C-2026-004"),
                        "yay -S zed",
                    ),
                ],
            )
            .unwrap();
        std::fs::write(
            &log,
            "[2026-10-01T10:00:00+0200] [PACMAN] Running 'pacman -S zed'\n\
             [2026-10-01T10:00:01+0200] [ALPM] transaction started\n\
             [2026-10-01T10:00:02+0200] [ALPM] installed alsa-lib (1.2.14-1)\n\
             [2026-10-01T10:00:03+0200] [ALPM] installed zed (0.198.4-1)\n\
             [2026-10-01T10:00:03+0200] [ALPM] transaction completed\n\
             [2026-10-01T11:00:00+0200] [PACMAN] Running 'pacman -S zed'\n\
             [2026-10-01T11:00:01+0200] [ALPM] transaction started\n\
             [2026-10-01T11:00:02+0200] [ALPM] reinstalled zed (0.198.4-1)\n\
             [2026-10-01T11:00:03+0200] [ALPM] transaction completed\n",
        )
        .unwrap();
        b.baseline = support::ts("2026-10-01T00:00:00+02:00");
        let out = b.run(&Pacman, "2026-10-01T12:00:00+02:00");
        let actors: Vec<_> = out
            .events
            .iter()
            .map(|e| (e.subject.as_str(), e.actor.as_str(), e.case.as_deref()))
            .collect();
        assert_eq!(
            actors,
            [
                ("alsa-lib", "system", None),
                ("zed", "system", None),
                ("zed", "system", None),
            ],
            "btop's command does not cover zed; zed's command is 11 min early"
        );

        // the same command inside the window attributes zed and its dependency
        let mut b2 = Bench::new("attribution-ok");
        b2.sources.pacman_log = log.clone();
        b2.baseline = b.baseline;
        let lock = &b2.lock;
        b2.ledger
            .append(
                lock,
                vec![hook(
                    "2026-10-01T09:51:00+02:00",
                    "agent:claude-code",
                    Some("C-2026-004"),
                    "yay -S --noconfirm zed",
                )],
            )
            .unwrap();
        let out = b2.run(&Pacman, "2026-10-01T12:00:00+02:00");
        let actors: Vec<_> = out
            .events
            .iter()
            .map(|e| (e.subject.as_str(), e.actor.as_str(), e.case.as_deref()))
            .collect();
        assert_eq!(
            actors,
            [
                ("alsa-lib", "agent:claude-code", Some("C-2026-004")),
                ("zed", "agent:claude-code", Some("C-2026-004")),
                ("zed", "system", None),
            ]
        );
        drop(b);
    }

    #[test]
    fn a_full_upgrade_command_does_not_cover_a_separate_install() {
        // ADR-0017 §3: an agent's `omarchy update` at 12:00 causes the keyring
        // reinstall and the full upgrade, not a human's install at 12:05
        let mut b = Bench::new("full-upgrade-scope");
        append(
            &b,
            vec![hook_event(
                "2026-10-01T12:00:00+02:00",
                "agent:claude-code",
                Some("C-2026-003"),
                "omarchy update",
            )],
        );
        let log = b.scratch.path("pkg.log");
        b.sources.pacman_log = log.clone();
        std::fs::write(
            &log,
            "[2026-10-01T12:00:05+0200] [PACMAN] Running 'pacman -Sy --noconfirm archlinux-keyring'\n\
             [2026-10-01T12:00:06+0200] [ALPM] transaction started\n\
             [2026-10-01T12:00:06+0200] [ALPM] reinstalled archlinux-keyring (20260902-1)\n\
             [2026-10-01T12:00:06+0200] [ALPM] transaction completed\n\
             [2026-10-01T12:00:11+0200] [PACMAN] Running 'pacman -Syu --noconfirm --overwrite /usr/share/omarchy/*'\n\
             [2026-10-01T12:03:00+0200] [ALPM] transaction started\n\
             [2026-10-01T12:03:00+0200] [ALPM] upgraded hyprland (0.52.1-1 -> 0.53.0-1)\n\
             [2026-10-01T12:03:01+0200] [ALPM] transaction completed\n\
             [2026-10-01T12:05:00+0200] [PACMAN] Running 'pacman -S tailscale'\n\
             [2026-10-01T12:05:01+0200] [ALPM] transaction started\n\
             [2026-10-01T12:05:01+0200] [ALPM] installed tailscale (1.102.3-1)\n\
             [2026-10-01T12:05:01+0200] [ALPM] transaction completed\n",
        )
        .unwrap();
        b.baseline = support::ts("2026-10-01T00:00:00+02:00");
        let out = b.run(&Pacman, "2026-10-01T12:10:00+02:00");
        assert_eq!(
            attribution(&out.events),
            [
                ("archlinux-keyring", "agent:claude-code", Some("C-2026-003")),
                ("hyprland", "agent:claude-code", Some("C-2026-003")),
                ("tailscale", "system", None),
            ]
        );
    }

    #[test]
    fn a_naming_command_does_not_reach_a_later_plain_upgrade() {
        // reviewer's case: the agent installs zed at 12:00 in its own
        // transaction; at 12:05 a human's plain -Syu upgrades zed and
        // firefox. The agent's command names zed, but the -Syu does not, so
        // the upgrade stays the human's (system), with no case.
        let mut b = Bench::new("naming-scope");
        append(
            &b,
            vec![hook_event(
                "2026-10-01T12:00:00+02:00",
                "agent:claude-code",
                Some("C-2026-004"),
                "yay -S zed",
            )],
        );
        let log = b.scratch.path("pkg.log");
        b.sources.pacman_log = log.clone();
        std::fs::write(
            &log,
            "[2026-10-01T12:00:10+0200] [PACMAN] Running 'pacman -S --needed --noconfirm --config /etc/pacman.conf -- extra/zed'\n\
             [2026-10-01T12:00:11+0200] [ALPM] transaction started\n\
             [2026-10-01T12:00:11+0200] [ALPM] installed zed (0.198.4-1)\n\
             [2026-10-01T12:00:11+0200] [ALPM] transaction completed\n\
             [2026-10-01T12:05:00+0200] [PACMAN] Running 'pacman -Syu'\n\
             [2026-10-01T12:05:30+0200] [ALPM] transaction started\n\
             [2026-10-01T12:05:30+0200] [ALPM] upgraded zed (0.198.4-1 -> 0.199.0-1)\n\
             [2026-10-01T12:05:31+0200] [ALPM] upgraded firefox (143.0.1-1 -> 143.0.2-1)\n\
             [2026-10-01T12:05:31+0200] [ALPM] transaction completed\n",
        )
        .unwrap();
        b.baseline = support::ts("2026-10-01T00:00:00+02:00");
        let out = b.run(&Pacman, "2026-10-01T12:10:00+02:00");
        assert_eq!(
            attribution(&out.events),
            [
                ("zed", "agent:claude-code", Some("C-2026-004")),
                ("zed", "system", None),
                ("firefox", "system", None),
            ]
        );
        assert_eq!(out.events[1].explicit, Some(false));
    }

    #[test]
    fn queries_and_late_commands_never_attribute() {
        let mut b = Bench::new("queries");
        append(
            &b,
            vec![
                // ADR-0017 §5: a search names zed but changes nothing
                hook_event(
                    "2026-10-01T10:00:00+02:00",
                    "agent:codex",
                    None,
                    "pacman -Ss zed",
                ),
                // ADR-0017 §2: started after the transaction began
                hook_event(
                    "2026-10-01T11:00:30+02:00",
                    "agent:codex",
                    None,
                    "sudo pacman -S btop",
                ),
            ],
        );
        let log = b.scratch.path("pkg.log");
        b.sources.pacman_log = log.clone();
        std::fs::write(
            &log,
            "[2026-10-01T10:01:00+0200] [PACMAN] Running 'pacman -S zed'\n\
             [2026-10-01T10:01:01+0200] [ALPM] transaction started\n\
             [2026-10-01T10:01:01+0200] [ALPM] installed zed (0.198.4-1)\n\
             [2026-10-01T10:01:01+0200] [ALPM] transaction completed\n\
             [2026-10-01T11:00:00+0200] [PACMAN] Running 'pacman -S btop'\n\
             [2026-10-01T11:00:31+0200] [ALPM] transaction started\n\
             [2026-10-01T11:00:31+0200] [ALPM] installed btop (1.4.5-1)\n\
             [2026-10-01T11:00:31+0200] [ALPM] transaction completed\n",
        )
        .unwrap();
        b.baseline = support::ts("2026-10-01T00:00:00+02:00");
        let out = b.run(&Pacman, "2026-10-01T12:00:00+02:00");
        assert_eq!(
            attribution(&out.events),
            [("zed", "system", None), ("btop", "system", None)]
        );
    }

    #[test]
    fn a_running_line_during_the_download_is_read_again() {
        let mut b = Bench::new("download");
        let log = b.scratch.path("pkg.log");
        b.sources.pacman_log = log.clone();
        b.baseline = support::ts("2026-10-01T00:00:00+02:00");
        std::fs::write(
            &log,
            "[2026-10-01T09:10:44+0200] [PACMAN] Running 'pacman -Syu --noconfirm --overwrite /usr/share/omarchy/*'\n\
             [2026-10-01T09:10:44+0200] [PACMAN] synchronizing package lists\n",
        )
        .unwrap();
        std::fs::write(&b.sources.pacman_db_lock, "").unwrap();
        let out = b.run(&Pacman, "2026-10-01T09:11:00+02:00");
        assert!(out.events.is_empty());
        assert_eq!(out.cursor.as_ref().unwrap()["offset"], 0);
        std::fs::OpenOptions::new()
            .append(true)
            .open(&log)
            .unwrap()
            .write_all(
                b"[2026-10-01T09:13:58+0200] [ALPM] transaction started\n\
                  [2026-10-01T09:13:58+0200] [ALPM] upgraded hyprland (0.52.1-1 -> 0.53.0-1)\n\
                  [2026-10-01T09:13:59+0200] [ALPM] transaction completed\n",
            )
            .unwrap();
        std::fs::remove_file(&b.sources.pacman_db_lock).unwrap();
        let out = b.run(&Pacman, "2026-10-01T09:14:00+02:00");
        assert_eq!(out.events.len(), 1);
        assert_eq!(out.events[0].explicit, Some(false));
        assert_eq!(
            out.events[0].meta.command.as_deref(),
            Some("pacman -Syu --noconfirm --overwrite /usr/share/omarchy/*")
        );
    }

    #[test]
    fn snapper_degrades_without_permissions() {
        let mut b = Bench::new("snapper-perm");
        b.sources.snapper = b.scratch.stub(
            "snapper",
            &format!(
                "cat '{}' >&2; exit 1",
                fixture("logs/snapper-no-permissions.stderr").display()
            ),
        );
        let out = b.run(&Snapper, "2026-10-01T17:05:00+02:00");
        assert!(!out.ok);
        assert!(out.events.is_empty());
        assert_eq!(out.cursor, None, "the cursor is kept");
        assert_eq!(out.message.as_deref(), Some(NO_PERMISSIONS));
        assert_eq!(out.fix.as_deref(), Some(SNAPPER_FIX));
        // the index variant shows the same message (fixtures/index-variants)
        let variant =
            std::fs::read_to_string(fixture("index-variants/snapper-degraded.json")).unwrap();
        assert!(variant.contains(NO_PERMISSIONS));

        b.sources.snapper = b.scratch.path("bin/missing").to_string_lossy().into_owned();
        let out = b.run(&Snapper, "2026-10-01T17:05:00+02:00");
        assert_eq!(
            (out.ok, out.message.as_deref()),
            (false, Some("snapper is not installed"))
        );
        b.sources.snapper = b.scratch.stub("snapper", "echo not json");
        assert!(!b.run(&Snapper, "2026-10-01T17:05:00+02:00").ok);
    }

    #[test]
    fn omarchy_falls_back_to_the_package_query() {
        let mut b = Bench::new("omarchy");
        b.sources.omarchy_version = b.scratch.path("bin/missing").to_string_lossy().into_owned();
        b.sources.pacman = b.scratch.stub("pkgquery", "echo 'omarchy 4.0.4-1'");
        let out = b.run(&Omarchy, "2026-10-01T10:00:00+02:00");
        assert!(out.ok);
        assert_eq!(out.cursor.unwrap()["version"], "4.0.4-1");
        b.sources.pacman = b.scratch.stub("pkgquery", "exit 1");
        let out = b.run(&Omarchy, "2026-10-01T10:00:00+02:00");
        assert!(!out.ok);
        assert!(out.events.is_empty());
    }

    #[test]
    fn omarchy_update_after_an_agent_full_upgrade() {
        // no package event for omarchy (log not captured), but an agent ran
        // `omarchy update` 5 minutes ago: the update is attributed
        let mut b = Bench::new("omarchy-hook");
        let lock = &b.lock;
        let mut e = Event::new(
            support::ts("2026-10-01T09:55:00+02:00"),
            Source::Agent,
            Kind::Command,
            "omarchy",
        )
        .actor("agent:claude-code")
        .case(Some("C-2026-003".into()));
        e.meta.command = Some("omarchy update".into());
        b.ledger.append(lock, vec![e]).unwrap();
        b.sources.omarchy_version = b.scratch.stub("omarchy-version", "echo 4.0.6-1");
        b.run(&Omarchy, "2026-10-01T09:00:00+02:00");
        b.sources.omarchy_version = b.scratch.stub("omarchy-version", "echo 4.0.7-1");
        let out = b.run(&Omarchy, "2026-10-01T10:00:00+02:00");
        assert_eq!(out.events.len(), 1);
        assert_eq!(out.events[0].actor, "agent:claude-code");
        assert_eq!(out.events[0].case.as_deref(), Some("C-2026-003"));
        assert_eq!(out.events[0].detail.as_deref(), Some("4.0.6-1 → 4.0.7-1"));
        assert_schema_valid(&out.events[0]);
    }

    #[test]
    fn registry_order_and_names() {
        let names: Vec<_> = seldon::collectors::REGISTRY
            .iter()
            .map(|c| c.name())
            .collect();
        assert_eq!(names, seldon::config::Collectors::NAMES);
        assert!(seldon::collectors::find("pacman").is_some());
        assert!(seldon::collectors::find("nope").is_none());
    }
}

/// A hook `command` event as WP-009 writes it (ts = the command's start).
fn hook_event(ts: &str, actor: &str, case: Option<&str>, command: &str) -> Event {
    let mut e = Event::new(support::ts(ts), Source::Agent, Kind::Command, "x")
        .actor(actor)
        .case(case.map(String::from));
    e.meta.command = Some(command.into());
    e
}

fn append(b: &Bench, events: Vec<Event>) {
    let lock = &b.lock;
    b.ledger.append(lock, events).unwrap();
}

fn attribution(events: &[Event]) -> Vec<(&str, &str, Option<&str>)> {
    events
        .iter()
        .map(|e| (e.subject.as_str(), e.actor.as_str(), e.case.as_deref()))
        .collect()
}

fn find(hay: &[u8], needle: &[u8]) -> usize {
    hay.windows(needle.len())
        .position(|w| w == needle)
        .expect("needle in fixture")
}
