//! The pacman, snapper and omarchy collectors against `fixtures/logs/`
//! (WP-004; SPEC-ENGINE §4, ADR-0026, ADR-0013 §5, ADR-0014 §1).
//!
//! The golden test replays the fixture story and requires exactly the
//! pacman, snapper and omarchy lines of `fixtures/logbook/ledger/*.jsonl`,
//! ids aside (fixtures/README.md "Event ids"). Capture times are the
//! fixture's: the bench runs each collector at the `now` the fixture's
//! capture-time events carry (omarchy `update`, `snapshot-delete`).

mod common;
mod support;

use std::io::Write as _;
use std::os::unix::fs::MetadataExt as _;

use seldon::collectors::omarchy::Omarchy;
use seldon::collectors::pacman::{self, Pacman, PacmanCursor};
use seldon::collectors::snapper::{NO_PERMISSIONS, Snapper};
use seldon::collectors::{Lost, to_cursor};
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
        assert_eq!(parsed.resume, 13254, "complete lines end at byte 13254");
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
                offset: 13254
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
        assert_eq!(second.events.len(), 19);
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
        assert_eq!(out.events.len(), 20);
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
        // no snapshot directory either: nothing to read instead
        b.sources.snapshots = b.scratch.path("no-snapshots");
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

    /// A snapper stub that answers like snapper without permission.
    fn no_permission(b: &Bench) -> String {
        b.scratch.stub(
            "snapper-denied",
            &format!(
                "cat '{}' >&2; exit 1",
                fixture("logs/snapper-no-permissions.stderr").display()
            ),
        )
    }

    /// `fixtures/logs/snapshots` copied into the bench's scratch dir.
    fn snapshots_copy(b: &Bench) -> std::path::PathBuf {
        let to = b.scratch.path("snapshots");
        for entry in std::fs::read_dir(fixture("logs/snapshots")).unwrap() {
            let entry = entry.unwrap();
            let dir = to.join(entry.file_name());
            std::fs::create_dir_all(&dir).unwrap();
            std::fs::copy(entry.path().join("info.xml"), dir.join("info.xml")).unwrap();
        }
        to
    }

    fn kinds(events: &[Event]) -> Vec<String> {
        events
            .iter()
            .map(|e| format!("{} {}", e.kind, e.subject))
            .collect()
    }

    /// Without permission to list, the info files give the same events as
    /// the list, and switching between the two adds none.
    #[test]
    fn snapper_reads_the_info_files_when_listing_is_not_permitted() {
        // the list: snapper-before.json, then snapper.json
        let mut list = Bench::new("snapper-list");
        list.sources.snapper = list
            .scratch
            .stub_cat("snapper", &fixture("logs/snapper-before.json"));
        list.run(&Snapper, "2026-09-30T18:00:00+02:00");
        list.sources.snapper = list
            .scratch
            .stub_cat("snapper", &fixture("logs/snapper.json"));
        let want = list.run(&Snapper, "2026-09-30T19:05:00+02:00");
        assert_eq!(want.events.len(), 7);

        // the same cursor, then the info files of snapper.json's state
        let mut b = Bench::new("snapper-info");
        b.sources.snapper = b
            .scratch
            .stub_cat("snapper", &fixture("logs/snapper-before.json"));
        b.run(&Snapper, "2026-09-30T18:00:00+02:00");
        b.sources.snapper = no_permission(&b);
        b.sources.snapshots = fixture("logs/snapshots");
        let got = b.run(&Snapper, "2026-09-30T19:05:00+02:00");
        assert!(got.ok, "{:?}", got.message);
        assert_eq!(got.fix, None);
        assert_eq!(got.baseline, None, "continued from its cursor");
        assert_eq!(
            normalised_sorted(&got.events),
            normalised_sorted(&want.events)
        );
        assert_eq!(got.cursor, want.cursor, "the same cursor");
        let message = got.message.unwrap();
        assert!(
            message.starts_with(
                "snapper list is not permitted; 10 snapshots read from the info files in "
            ),
            "{message}"
        );

        // a second capture, then list ↔ info files: nothing new
        assert_eq!(
            kinds(&b.run(&Snapper, "2026-09-30T19:10:00+02:00").events),
            [""; 0]
        );
        b.sources.snapper = b.scratch.stub_cat("snapper", &fixture("logs/snapper.json"));
        let out = b.run(&Snapper, "2026-09-30T19:15:00+02:00");
        assert!(out.ok);
        assert_eq!(out.message, None);
        assert_eq!(kinds(&out.events), [""; 0]);
        b.sources.snapper = no_permission(&b);
        assert_eq!(
            kinds(&b.run(&Snapper, "2026-09-30T19:20:00+02:00").events),
            [""; 0]
        );

        // a first run from the info files takes the same baseline as the list
        let mut fresh_list = Bench::new("snapper-fresh-list");
        fresh_list.baseline = support::ts("2026-09-30T00:00:00+02:00");
        let want = fresh_list.run(&Snapper, "2026-10-01T17:05:00+02:00");
        let mut fresh = Bench::new("snapper-fresh-info");
        fresh.baseline = support::ts("2026-09-30T00:00:00+02:00");
        fresh.sources.snapper = no_permission(&fresh);
        fresh.sources.snapshots = fixture("logs/snapshots");
        let got = fresh.run(&Snapper, "2026-10-01T17:05:00+02:00");
        assert_eq!(got.baseline, Some(Lost::Cursor), "WP-081");
        assert_eq!(
            kinds(&got.events),
            [
                "snapshot 111",
                "snapshot 112",
                "snapshot 113",
                "snapshot 114",
                "snapshot 115"
            ]
        );
        assert_eq!(
            normalised_sorted(&got.events),
            normalised_sorted(&want.events)
        );
    }

    /// An info file that cannot be read is skipped and named; its snapshot
    /// is neither new nor deleted. Other entries are ignored.
    #[test]
    fn snapper_info_files_skip_what_cannot_be_read() {
        use std::os::unix::fs::PermissionsExt as _;
        let mut b = Bench::new("snapper-skip");
        b.run(&Snapper, "2026-10-01T17:05:00+02:00"); // the list's cursor
        let dir = snapshots_copy(&b);
        std::fs::create_dir_all(dir.join("lost+found")).unwrap();
        std::fs::write(dir.join("README"), "not a snapshot").unwrap();
        // 112: no <num>; 113: not readable (where permissions apply)
        std::fs::write(
            dir.join("112/info.xml"),
            "<?xml version=\"1.0\"?>\n<snapshot>\n  <type>single</type>\n</snapshot>\n",
        )
        .unwrap();
        let unreadable = dir.join("113/info.xml");
        std::fs::set_permissions(&unreadable, std::fs::Permissions::from_mode(0o000)).unwrap();
        let denied = std::fs::read(&unreadable).is_err();
        b.sources.snapper = no_permission(&b);
        b.sources.snapshots = dir.clone();
        let out = b.run(&Snapper, "2026-10-01T17:10:00+02:00");
        assert!(out.ok, "{:?}", out.message);
        assert_eq!(
            kinds(&out.events),
            [""; 0],
            "no snapshot-delete for 112 or 113"
        );
        let message = out.message.unwrap();
        assert!(
            message.contains("; skipped 112/info.xml: no <num>"),
            "{message}"
        );
        if denied {
            assert!(
                message.contains("; skipped 113/info.xml: permission denied"),
                "{message}"
            );
        }
        let known = &out.cursor.unwrap()["known"];
        assert_eq!(known["112"]["type"], "single", "kept in the cursor");
        assert_eq!(known["113"]["description"], "pre: ollama");

        // readable again: still nothing new
        std::fs::set_permissions(&unreadable, std::fs::Permissions::from_mode(0o644)).unwrap();
        std::fs::copy(
            fixture("logs/snapshots/112/info.xml"),
            dir.join("112/info.xml"),
        )
        .unwrap();
        let out = b.run(&Snapper, "2026-10-01T17:15:00+02:00");
        assert_eq!(kinds(&out.events), [""; 0]);
        assert!(!out.message.unwrap().contains("skipped"));

        // every numbered info file unreadable: degraded, as without the files
        for entry in std::fs::read_dir(&dir).unwrap() {
            let info = entry.unwrap().path().join("info.xml");
            if info.exists() {
                std::fs::write(&info, "<snapshot></snapshot>").unwrap();
            }
        }
        let out = b.run(&Snapper, "2026-10-01T17:20:00+02:00");
        assert!(!out.ok);
        assert_eq!(out.message.as_deref(), Some(NO_PERMISSIONS));
        assert_eq!(out.fix.as_deref(), Some(SNAPPER_FIX));
        assert_eq!(out.cursor, None, "the cursor is kept");

        // an empty, readable directory (e.g. after booting into a
        // snapshot): degraded, no snapshot-delete, the cursor kept
        let before = b.cursors["snapper"].clone();
        let empty = b.scratch.path("empty-snapshots");
        std::fs::create_dir_all(&empty).unwrap();
        b.sources.snapshots = empty;
        let out = b.run(&Snapper, "2026-10-01T17:22:00+02:00");
        assert_eq!(kinds(&out.events), [""; 0], "no snapshot-delete");
        assert!(!out.ok);
        assert_eq!(out.message.as_deref(), Some(NO_PERMISSIONS));
        assert_eq!(out.fix.as_deref(), Some(SNAPPER_FIX));
        assert_eq!(out.cursor, None, "the cursor is kept");
        assert_eq!(b.cursors["snapper"], before);

        // the directory itself not readable: degraded
        b.sources.snapshots = fixture("logs/snapshots");
        let locked = b.scratch.path("locked");
        std::fs::create_dir_all(&locked).unwrap();
        std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o000)).unwrap();
        if std::fs::read_dir(&locked).is_err() {
            b.sources.snapshots = locked.clone();
            let out = b.run(&Snapper, "2026-10-01T17:25:00+02:00");
            assert_eq!(out.message.as_deref(), Some(NO_PERMISSIONS));
        }
        std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o755)).unwrap();
    }

    /// `snapper --jsonout list` output of the `root` config:
    /// `(number, date, description)`, all `single`.
    fn snapper_list(snapshots: &[(u64, &str, &str)]) -> String {
        let list: Vec<serde_json::Value> = snapshots
            .iter()
            .map(|(n, date, description)| {
                serde_json::json!({
                    "number": n, "type": "single", "date": date,
                    "cleanup": "number", "description": description,
                })
            })
            .collect();
        serde_json::json!({ "root": list }).to_string()
    }

    /// snapper reuses the number after the highest one: a known number
    /// with another date is a deletion and a new snapshot (WP-073).
    #[test]
    fn snapper_sees_a_reused_number() {
        let mut b = Bench::new("snapper-reuse");
        let list = b.scratch.path("list.json");
        b.sources.snapper = b.scratch.stub_cat("snapper", &list);
        let set = |snapshots: &[(u64, &str, &str)]| {
            std::fs::write(&list, snapper_list(snapshots)).unwrap();
        };
        let four = (4, "2026-10-01 08:00:00", "timeline");
        set(&[four]);
        b.run(&Snapper, "2026-10-01T08:30:00+02:00");
        set(&[four, (5, "2026-10-01 08:58:00", "pre: pacman -S foo")]);
        assert_eq!(
            kinds(&b.run(&Snapper, "2026-10-01T09:00:00+02:00").events),
            ["snapshot 5"]
        );
        let before = b.cursors.clone();

        // 5 deleted, a new 5 made before the next capture
        set(&[four, (5, "2026-10-01 09:05:00", "pre: pacman -S bar")]);
        let out = b.run(&Snapper, "2026-10-01T09:15:00+02:00");
        assert_eq!(kinds(&out.events), ["snapshot-delete 5", "snapshot 5"]);
        let (gone, made) = (&out.events[0], &out.events[1]);
        assert_eq!(gone.detail.as_deref(), Some("pre: pacman -S foo"));
        assert_eq!(made.detail.as_deref(), Some("pre: pacman -S bar"));
        // both at the new snapshot's date, the deletion first: the ledger's
        // last word on 5 is the snapshot that exists
        assert_eq!(gone.ts, support::ts("2026-10-01T09:05:00+02:00"));
        assert_eq!(made.ts, gone.ts);
        let ledger = b.ledger_events(Source::Snapper);
        assert_eq!(
            kinds(&ledger[ledger.len() - 2..]),
            ["snapshot-delete 5", "snapshot 5"]
        );
        assert!(
            b.run(&Snapper, "2026-10-01T09:25:00+02:00")
                .events
                .is_empty()
        );
        // a crash before the cursor save: nothing twice
        b.cursors = before;
        assert!(
            b.run(&Snapper, "2026-10-01T09:30:00+02:00")
                .events
                .is_empty()
        );
        assert_eq!(b.ledger_events(Source::Snapper).len(), 4);

        // a cursor from before WP-073 has no dates: they are filled in
        // without events, and a reuse after that is seen
        let mut old = b.cursors["snapper"].clone();
        for known in old["known"].as_object_mut().unwrap().values_mut() {
            assert!(known.as_object_mut().unwrap().remove("date").is_some());
        }
        b.cursors.insert("snapper", old);
        set(&[four, (5, "2026-10-01 09:40:00", "pre: pacman -S baz")]);
        assert!(
            b.run(&Snapper, "2026-10-01T09:45:00+02:00")
                .events
                .is_empty(),
            "an old cursor cannot tell"
        );
        assert!(b.cursors["snapper"]["known"]["5"]["date"].is_string());
        set(&[four, (5, "2026-10-01 09:50:00", "pre: pacman -S qux")]);
        assert_eq!(
            kinds(&b.run(&Snapper, "2026-10-01T09:55:00+02:00").events),
            ["snapshot-delete 5", "snapshot 5"]
        );
    }

    /// Central European Time as a POSIX `TZ` rule (no tzdata needed):
    /// summer time ends on 2026-10-25 at 03:00 CEST, when the clocks go
    /// back to 02:00 CET, so 02:00–02:59 comes twice; it begins on
    /// 2027-03-28 at 02:00 CET, when the clocks jump to 03:00 CEST, so
    /// 02:00–02:59 does not exist.
    const CET: &str = "CET-1CEST,M3.5.0,M10.5.0/3";

    /// `seldon capture --source snapper` through the binary in `TZ=CET`.
    /// The snapper stub prints `list.json`, or answers like snapper
    /// without permission while `denied` exists; the info files are in
    /// `<guard>/.snapshots`.
    struct Zoned {
        env: common::Env,
        logbook: std::path::PathBuf,
    }

    impl Zoned {
        fn new() -> Self {
            let env = common::Env::new(common::Snapper::Missing);
            let logbook = env.init_logbook();
            let tmp = env.tmp.path();
            env.stub(
                "snapper",
                &format!(
                    "if [ -e '{}' ]; then echo 'No permissions.' >&2; exit 1; fi\n/bin/cat '{}'",
                    tmp.join("denied").display(),
                    tmp.join("list.json").display()
                ),
            );
            let z = Zoned { env, logbook };
            // created before the snapshots, whatever today is: the baseline
            // of a capture that lost its state
            z.created("2026-10-20T11:00:00+02:00");
            z.list(&[]);
            // the first capture: an empty cursor, so what follows is news
            assert_eq!(z.capture("2026-10-20T12:00:00+02:00"), [""; 0]);
            z
        }

        /// Sets the logbook's `created`.
        fn created(&self, at: &str) {
            let meta = self.logbook.join(".seldon/logbook.toml");
            let text = common::read(&meta);
            let line = text.lines().find(|l| l.starts_with("created = ")).unwrap();
            let text = text.replace(line, &format!("created = {at}"));
            std::fs::write(&meta, text).unwrap();
        }

        /// The list: `(number, local date)`, all `single` timeline snapshots.
        fn list(&self, snapshots: &[(u64, &str)]) {
            let snapshots: Vec<(u64, &str, &str)> = snapshots
                .iter()
                .map(|(n, d)| (*n, *d, "timeline"))
                .collect();
            std::fs::write(
                self.env.tmp.path().join("list.json"),
                snapper_list(&snapshots),
            )
            .unwrap();
        }

        /// The info files, replacing the ones before: `(number, UTC date)`.
        fn info(&self, snapshots: &[(u64, &str)]) {
            let dir = self.env.tmp.path().join(".snapshots");
            let _ = std::fs::remove_dir_all(&dir);
            for (n, date) in snapshots {
                std::fs::create_dir_all(dir.join(n.to_string())).unwrap();
                std::fs::write(
                    dir.join(n.to_string()).join("info.xml"),
                    format!(
                        "<?xml version=\"1.0\"?>\n<snapshot>\n  <type>single</type>\n  \
                         <num>{n}</num>\n  <date>{date}</date>\n  \
                         <description>timeline</description>\n  \
                         <cleanup>timeline</cleanup>\n</snapshot>\n"
                    ),
                )
                .unwrap();
            }
        }

        /// Whether snapper refuses to list (the collector reads the info
        /// files then).
        fn deny(&self, denied: bool) {
            let flag = self.env.tmp.path().join("denied");
            if denied {
                std::fs::write(flag, "").unwrap();
            } else {
                let _ = std::fs::remove_file(flag);
            }
        }

        /// Captures at `now`; the snapper events it wrote as `kind subject
        /// ts`.
        fn capture(&self, now: &str) -> Vec<String> {
            let ids = |ledger: &[serde_json::Value]| -> std::collections::HashSet<String> {
                ledger.iter().map(|e| e["id"].to_string()).collect()
            };
            let before = ids(&common::ledger(&self.logbook));
            let out = self
                .env
                .command(&["capture", "--source", "snapper", "--json"])
                .env("TZ", CET)
                .env("SELDON_NOW", now)
                .output()
                .unwrap();
            assert_eq!(out.status.code(), Some(0), "{}", common::stderr(&out));
            let snapper = &common::json(&out)["collectors"]
                .as_array()
                .unwrap()
                .iter()
                .find(|c| c["name"] == "snapper")
                .cloned()
                .unwrap();
            assert_eq!(snapper["ok"], true, "{snapper}");
            common::ledger(&self.logbook)
                .into_iter()
                .filter(|e| e["source"] == "snapper" && !before.contains(&e["id"].to_string()))
                .map(|e| {
                    format!(
                        "{} {} {}",
                        e["kind"].as_str().unwrap(),
                        e["subject"].as_str().unwrap(),
                        e["ts"].as_str().unwrap()
                    )
                })
                .collect()
        }

        fn cursors_file(&self) -> std::path::PathBuf {
            self.env.home.join(".local/state/seldon/cursors.json")
        }

        /// The date the cursor keeps for snapshot `n`.
        fn date(&self, n: u64) -> serde_json::Value {
            let cursors: serde_json::Value =
                serde_json::from_str(&common::read(&self.cursors_file())).unwrap();
            cursors["collectors"]["snapper"]["cursor"]["known"][n.to_string()]["date"].clone()
        }

        /// Sets the date the cursor keeps for snapshot `n`.
        fn set_date(&self, n: u64, date: &str) {
            let mut cursors: serde_json::Value =
                serde_json::from_str(&common::read(&self.cursors_file())).unwrap();
            cursors["collectors"]["snapper"]["cursor"]["known"][n.to_string()]["date"] =
                date.into();
            std::fs::write(self.cursors_file(), cursors.to_string()).unwrap();
        }
    }

    /// A snapshot made in the repeated hour, listed first: the list cannot
    /// tell which 02:30 it is and takes the earlier. The info files then
    /// give the instant: the same snapshot, no `snapshot-delete` plus
    /// `snapshot`, and the cursor keeps the info file's date from then on,
    /// also when listing again.
    #[test]
    fn snapper_keeps_one_date_from_the_list_to_the_info_files() {
        let z = Zoned::new();
        // 11 is made at 02:30 after the clocks went back (01:30 UTC)
        z.list(&[(10, "2026-10-25 01:30:00"), (11, "2026-10-25 02:30:00")]);
        assert_eq!(
            z.capture("2026-10-25T04:00:00+01:00"),
            [
                "snapshot 10 2026-10-25T01:30:00+02:00",
                "snapshot 11 2026-10-25T02:30:00+02:00"
            ],
            "no info file: the earlier of the two"
        );

        z.deny(true);
        z.info(&[(10, "2026-10-24 23:30:00"), (11, "2026-10-25 01:30:00")]);
        assert_eq!(z.capture("2026-10-25T05:00:00+01:00"), [""; 0]);
        assert_eq!(z.date(11), "2026-10-25T02:30:00+01:00", "the info file's");

        // listing again, without the info files: the cursor's date
        z.deny(false);
        z.info(&[]);
        assert_eq!(z.capture("2026-10-25T06:00:00+01:00"), [""; 0]);
        assert_eq!(z.date(11), "2026-10-25T02:30:00+01:00");
        // and with them
        z.info(&[(10, "2026-10-24 23:30:00"), (11, "2026-10-25 01:30:00")]);
        assert_eq!(z.capture("2026-10-25T07:00:00+01:00"), [""; 0]);
        assert_eq!(z.date(11), "2026-10-25T02:30:00+01:00");

        // a real deletion and a reuse of the number are still seen, also
        // across the switch: 10 deleted, 11 deleted and made again
        z.deny(true);
        z.info(&[(11, "2026-10-25 08:00:00")]);
        assert_eq!(
            z.capture("2026-10-25T09:30:00+01:00"),
            [
                "snapshot-delete 11 2026-10-25T09:00:00+01:00",
                "snapshot 11 2026-10-25T09:00:00+01:00",
                "snapshot-delete 10 2026-10-25T09:30:00+01:00"
            ]
        );
    }

    /// Read first from the info files, then from the list: the list's
    /// local time is resolved to the instant the info files gave, through
    /// the info file while it can be read, else through the cursor.
    #[test]
    fn snapper_keeps_one_date_from_the_info_files_to_the_list() {
        let z = Zoned::new();
        // 11 at 02:30 before the clocks went back, 12 at 02:15 after
        let info = [
            (10, "2026-10-24 23:30:00"),
            (11, "2026-10-25 00:30:00"),
            (12, "2026-10-25 01:15:00"),
        ];
        let list = [
            (10, "2026-10-25 01:30:00"),
            (11, "2026-10-25 02:30:00"),
            (12, "2026-10-25 02:15:00"),
        ];
        z.deny(true);
        z.info(&info);
        assert_eq!(
            z.capture("2026-10-25T04:00:00+01:00"),
            [
                "snapshot 10 2026-10-25T01:30:00+02:00",
                "snapshot 11 2026-10-25T02:30:00+02:00",
                "snapshot 12 2026-10-25T02:15:00+01:00"
            ]
        );
        let dates = |z: &Zoned| [10, 11, 12].map(|n| z.date(n));
        let want = dates(&z);

        z.deny(false);
        z.list(&list);
        assert_eq!(z.capture("2026-10-25T05:00:00+01:00"), [""; 0]);
        assert_eq!(dates(&z), want);
        z.info(&[]);
        assert_eq!(z.capture("2026-10-25T06:00:00+01:00"), [""; 0]);
        assert_eq!(dates(&z), want);
    }

    /// A new snapshot in the repeated hour, listed: its info file decides
    /// when it can be read, else the number order where it can.
    #[test]
    fn snapper_dates_a_listed_snapshot_in_the_repeated_hour() {
        let z = Zoned::new();
        z.list(&[(10, "2026-10-25 01:30:00"), (11, "2026-10-25 02:30:00")]);
        z.info(&[(11, "2026-10-25 01:30:00")]); // after the clocks went back
        assert_eq!(
            z.capture("2026-10-25T04:00:00+01:00"),
            [
                "snapshot 10 2026-10-25T01:30:00+02:00",
                "snapshot 11 2026-10-25T02:30:00+01:00"
            ]
        );

        // no info files: 11 at 02:40 can be either (the earlier); 12 at
        // 02:20 is made after 11, so after the clocks went back
        let z = Zoned::new();
        z.list(&[
            (10, "2026-10-25 01:30:00"),
            (11, "2026-10-25 02:40:00"),
            (12, "2026-10-25 02:20:00"),
        ]);
        assert_eq!(
            z.capture("2026-10-25T04:00:00+01:00"),
            [
                "snapshot 10 2026-10-25T01:30:00+02:00",
                "snapshot 11 2026-10-25T02:40:00+02:00",
                "snapshot 12 2026-10-25T02:20:00+01:00"
            ]
        );
    }

    /// Before WP-082 the list's 02:30 in the repeated hour was always the
    /// instant with the smaller offset, the later one (chrono's
    /// `earliest()`). Such a cursor entry moves to the info file's instant
    /// without an event.
    #[test]
    fn snapper_moves_an_older_cursor_to_the_info_file_date() {
        let z = Zoned::new();
        z.list(&[(10, "2026-10-25 01:30:00"), (11, "2026-10-25 02:30:00")]);
        assert_eq!(z.capture("2026-10-25T04:00:00+01:00").len(), 2);
        // the ledger and the cursor as a capture before WP-082 wrote them
        let month = z.logbook.join("ledger/2026-10.jsonl");
        let ledger = common::read(&month);
        let (earlier, later) = ("2026-10-25T02:30:00+02:00", "2026-10-25T02:30:00+01:00");
        assert_eq!(ledger.matches(earlier).count(), 1);
        std::fs::write(&month, ledger.replace(earlier, later)).unwrap();
        z.set_date(11, later);
        z.deny(true);
        z.info(&[(10, "2026-10-24 23:30:00"), (11, "2026-10-25 00:30:00")]);
        assert_eq!(z.capture("2026-10-25T05:00:00+01:00"), [""; 0]);
        assert_eq!(z.date(11), earlier);
    }

    /// A snapshot older than the baseline is known without an event, so
    /// the ledger cannot tell its two instants apart: the cursor rule alone
    /// keeps the switch to the info files free of a false pair.
    #[test]
    fn snapper_keeps_one_date_for_a_snapshot_known_without_an_event() {
        let z = Zoned::new();
        std::fs::remove_dir_all(z.env.home.join(".local/state/seldon")).unwrap();
        z.created("2026-10-26T00:00:00+01:00");
        z.list(&[(10, "2026-10-25 01:30:00"), (11, "2026-10-25 02:30:00")]);
        assert_eq!(z.capture("2026-10-26T01:00:00+01:00"), [""; 0], "history");
        assert_eq!(z.date(11), "2026-10-25T02:30:00+02:00");
        z.deny(true);
        z.info(&[(10, "2026-10-24 23:30:00"), (11, "2026-10-25 01:30:00")]);
        assert_eq!(z.capture("2026-10-26T02:00:00+01:00"), [""; 0]);
        assert_eq!(z.date(11), "2026-10-25T02:30:00+01:00");
    }

    /// A listed time that does not exist (the hour skipped when summer time
    /// begins) names no instant: the snapshot is remembered without a date
    /// and without an event, and gets its date from the info files later,
    /// again without an event.
    #[test]
    fn snapper_remembers_a_listed_time_in_the_dst_gap() {
        let z = Zoned::new();
        // 22 at 02:00:00, the start of the skipped hour (chrono reads it as
        // 01:00 UTC, which is 03:00 on the wall)
        z.list(&[
            (20, "2027-03-28 01:30:00"),
            (21, "2027-03-28 02:30:00"),
            (22, "2027-03-28 02:00:00"),
        ]);
        assert_eq!(
            z.capture("2027-03-28T04:00:00+02:00"),
            ["snapshot 20 2027-03-28T01:30:00+01:00"]
        );
        assert_eq!(z.date(21), serde_json::Value::Null, "known, no date");
        assert_eq!(z.date(22), serde_json::Value::Null, "known, no date");
        z.deny(true);
        z.info(&[
            (20, "2027-03-28 00:30:00"),
            (21, "2027-03-28 01:30:00"),
            (22, "2027-03-28 01:45:00"),
        ]);
        assert_eq!(z.capture("2027-03-28T05:00:00+02:00"), [""; 0]);
        assert_eq!(z.date(21), "2027-03-28T03:30:00+02:00");
        assert_eq!(z.date(22), "2027-03-28T03:45:00+02:00");
    }

    /// The ends of the repeated hour: 02:00:00 is two instants, 03:00:00
    /// only one (chrono also offers 03:00+02:00, which is 02:00 CET on the
    /// wall). Neither gives a false pair when the info files follow.
    #[test]
    fn snapper_keeps_one_date_at_the_ends_of_the_repeated_hour() {
        let z = Zoned::new();
        // 11 at 02:00:00 after the clocks went back, 12 at 03:00:00
        z.list(&[
            (10, "2026-10-25 01:30:00"),
            (11, "2026-10-25 02:00:00"),
            (12, "2026-10-25 03:00:00"),
        ]);
        assert_eq!(
            z.capture("2026-10-25T04:00:00+01:00"),
            [
                "snapshot 10 2026-10-25T01:30:00+02:00",
                "snapshot 11 2026-10-25T02:00:00+02:00",
                "snapshot 12 2026-10-25T03:00:00+01:00"
            ],
            "11 can be either (the earlier), 12 only one"
        );
        z.deny(true);
        z.info(&[
            (10, "2026-10-24 23:30:00"),
            (11, "2026-10-25 01:00:00"),
            (12, "2026-10-25 02:00:00"),
        ]);
        assert_eq!(z.capture("2026-10-25T05:00:00+01:00"), [""; 0]);
        assert_eq!(z.date(11), "2026-10-25T02:00:00+01:00");
        assert_eq!(z.date(12), "2026-10-25T03:00:00+01:00");
    }

    /// The ledger dedupe knows both instants of a time in the repeated
    /// hour: a snapshot the list recorded is not recorded again from the
    /// info files when the cursor save failed in between, or when the
    /// state directory was lost (the baseline is then the logbook's
    /// `created`, before the snapshot).
    #[test]
    fn snapper_dedupes_the_other_instant_after_a_lost_cursor() {
        let z = Zoned::new();
        let state = z.env.home.join(".local/state/seldon");
        z.list(&[(10, "2026-10-25 01:30:00")]);
        assert_eq!(z.capture("2026-10-25T02:00:00+02:00").len(), 1);
        // 11 at 02:30 after the clocks went back; listed as the earlier
        z.list(&[(10, "2026-10-25 01:30:00"), (11, "2026-10-25 02:30:00")]);
        let info = [(10, "2026-10-24 23:30:00"), (11, "2026-10-25 01:30:00")];

        // the cursor save fails: the cursor stays as before the capture, so
        // 11 alone is new to the next one, an hour from the ledger's 11
        let cursors = common::read(&z.cursors_file());
        assert_eq!(z.capture("2026-10-25T04:00:00+01:00").len(), 1);
        std::fs::write(z.cursors_file(), cursors).unwrap();
        z.deny(true);
        z.info(&info);
        assert_eq!(z.capture("2026-10-25T05:00:00+01:00"), [""; 0]);

        // the state directory lost, then listed again: nothing new
        std::fs::remove_dir_all(&state).unwrap();
        z.deny(false);
        z.info(&[]);
        assert_eq!(z.capture("2026-10-25T06:00:00+01:00"), [""; 0]);
        // lost again, then the info files
        std::fs::remove_dir_all(&state).unwrap();
        z.deny(true);
        z.info(&info);
        assert_eq!(z.capture("2026-10-25T07:00:00+01:00"), [""; 0]);
        let ledger: Vec<_> = common::ledger(&z.logbook)
            .into_iter()
            .filter(|e| e["source"] == "snapper")
            .collect();
        assert_eq!(ledger.len(), 2, "10 and 11 once each");

        // a reused number: the deletion and the new snapshot, recorded from
        // the list, are not recorded again from the info files
        let z = Zoned::new();
        z.list(&[(11, "2026-10-24 12:00:00")]);
        assert_eq!(z.capture("2026-10-24T13:00:00+02:00").len(), 1);
        let cursors = common::read(&z.cursors_file());
        z.list(&[(11, "2026-10-25 02:30:00")]);
        assert_eq!(
            z.capture("2026-10-25T04:00:00+01:00"),
            [
                "snapshot-delete 11 2026-10-25T02:30:00+02:00",
                "snapshot 11 2026-10-25T02:30:00+02:00"
            ]
        );
        std::fs::write(z.cursors_file(), cursors).unwrap();
        z.deny(true);
        z.info(&[(11, "2026-10-25 01:30:00")]);
        assert_eq!(z.capture("2026-10-25T05:00:00+01:00"), [""; 0]);
    }

    /// `seldon capture` runs on the invocation's clock: `SELDON_NOW` sets
    /// the collectors' `checked` and `lastRun` and the time of a
    /// capture-time event (WP-073).
    #[test]
    fn capture_runs_on_seldon_now() {
        let env = common::Env::new(common::Snapper::Missing);
        let lb = env.init_logbook();
        let hypr = env.home.join(".config/hypr");
        std::fs::create_dir_all(&hypr).unwrap();
        std::fs::write(hypr.join("a.conf"), "a\n").unwrap();
        std::fs::write(hypr.join("b.conf"), "b\n").unwrap();
        let capture = |now: &str| {
            let out = env.at(now, &["capture", "--source", "config", "--json"]);
            assert_eq!(out.status.code(), Some(0), "{}", common::stderr(&out));
            common::json(&out)
        };
        let t1 = "2026-12-24T09:00:00+01:00";
        let t2 = "2026-12-24T10:00:00+01:00";
        assert_eq!(capture(t1)["written"], 0, "baseline");
        std::fs::remove_file(hypr.join("b.conf")).unwrap();
        let out = capture(t2);
        assert_eq!(out["written"], 1);
        assert_eq!(out["files"], serde_json::json!(["ledger/2026-12.jsonl"]));
        let ledger = common::ledger(&lb);
        let removed = ledger
            .iter()
            .find(|e| e["kind"] == "config-remove")
            .unwrap();
        assert_eq!(removed["subject"], "~/.config/hypr/b.conf");
        let at = |v: &serde_json::Value| {
            chrono::DateTime::parse_from_rfc3339(v.as_str().unwrap()).unwrap()
        };
        assert_eq!(at(&removed["ts"]), support::ts(t2));
        let cursors: serde_json::Value = serde_json::from_str(&common::read(
            &env.home.join(".local/state/seldon/cursors.json"),
        ))
        .unwrap();
        let config = &cursors["collectors"]["config"];
        assert_eq!(at(&config["lastRun"]), support::ts(t2));
        assert_eq!(at(&config["cursor"]["checked"]), support::ts(t2));
        // the marker: the removal is the one config event at the check (WP-107)
        assert_eq!(config["cursor"]["atCheck"], 1);
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
