//! `seldon doctor` in a throw-away home, and read-only on the fixture logbook.

mod common;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use common::{Env, Snapper, json, stderr, stdout};

fn check<'a>(report: &'a serde_json::Value, name: &str) -> &'a serde_json::Value {
    report["checks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["name"] == name)
        .unwrap_or_else(|| panic!("no check {name} in {report}"))
}

fn init(env: &Env) -> PathBuf {
    let root = env.tmp.path().join("logbook");
    let out = env.seldon(&[
        "init",
        "--non-interactive",
        "--no-capture",
        "--path",
        root.to_str().unwrap(),
    ]);
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    root
}

/// Every file under `dir` with its bytes.
fn snapshot(dir: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    let mut out = BTreeMap::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for entry in std::fs::read_dir(&d).unwrap() {
            let p = entry.unwrap().path();
            if p.is_dir() {
                stack.push(p);
            } else {
                out.insert(p.clone(), std::fs::read(&p).unwrap());
            }
        }
    }
    out
}

mod doctor {
    use super::*;

    #[test]
    fn green_after_init_with_snapper_degraded() {
        let env = Env::new(Snapper::NoPermissions);
        let root = init(&env);
        let out = env.seldon(&["doctor", "--path", root.to_str().unwrap(), "--json"]);
        assert_eq!(out.status.code(), Some(0), "{}", stdout(&out));
        let v = json(&out);
        assert_eq!(v["ok"], true);
        for name in ["engine", "config", "logbook", "omarchy"] {
            assert_eq!(check(&v, name)["status"], "ok", "{name}: {v}");
        }
        assert_eq!(check(&v, "omarchy")["message"], "Omarchy 4.0.4-1");
        let snapper = check(&v, "snapper");
        assert_eq!(snapper["status"], "degraded");
        // the read grant of ADR-0026, with what it grants
        assert_eq!(snapper["fix"], "sudo setfacl -m u:$USER:rx /.snapshots");
        assert_eq!(
            snapper["message"],
            format!(
                "No permissions. Snapshots are not recorded until you grant your user read access to the snapshot directory once (ADR-0026). {}",
                seldon::commands::doctor::SNAPPER_FIX_GRANTS
            )
        );
        assert_eq!(
            seldon::commands::doctor::SNAPPER_FIX_GRANTS,
            "The fix grants your user read access to the snapshot directory listing and the snapshot info files (files inside a snapshot keep their own permissions), nothing else: no snapshot creation, change or deletion."
        );
        if env.has_git {
            assert_eq!(check(&v, "git")["status"], "ok", "{v}");
        }
    }

    #[test]
    fn human_output_lists_every_check() {
        let env = Env::new(Snapper::NoPermissions);
        let root = init(&env);
        let out = env.seldon(&["--logbook", root.to_str().unwrap(), "doctor"]);
        assert_eq!(out.status.code(), Some(0));
        let text = stdout(&out);
        for name in ["engine", "config", "logbook", "omarchy", "snapper", "git"] {
            assert!(text.contains(name), "{name} missing in:\n{text}");
        }
        assert!(text.contains("fix: sudo setfacl -m u:$USER:rx /.snapshots"));
        assert!(
            text.contains(seldon::commands::doctor::SNAPPER_FIX_GRANTS),
            "{text}"
        );
        assert!(text.trim_end().ends_with("doctor: ok"));
    }

    /// Issue #1: snapper's permission error is recognised in a German
    /// locale. The stub answers in German unless `LC_ALL=C`, like snapper's
    /// own catalog; the collector state, `init`'s hint and `doctor`'s fix
    /// line must all appear.
    #[test]
    fn snapper_permission_error_is_recognised_in_any_locale() {
        let env = Env::new(Snapper::NoPermissionsLocalized);
        let german = |args: &[&str]| {
            env.command(args)
                .env("LANG", "de_DE.UTF-8")
                .env("LANGUAGE", "de")
                .output()
                .expect("run seldon")
        };
        let root = env.tmp.path().join("logbook");
        let out = german(&[
            "init",
            "--non-interactive",
            "--path",
            root.to_str().unwrap(),
        ]);
        assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
        let text = stdout(&out);
        assert!(
            text.contains("\nSnapshots   not readable yet; optional, Seldon works without them\n"),
            "{text}"
        );
        assert!(
            text.ends_with(&format!(
                "{}\n  {}\n",
                seldon::commands::init::SNAPPER_OPTIONAL,
                seldon::commands::doctor::SNAPPER_FIX
            )),
            "{text}"
        );
        assert!(!text.contains("Keine"), "{text}");

        let cursors: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(env.home.join(".local/state/seldon/cursors.json")).unwrap(),
        )
        .unwrap();
        let state = &cursors["collectors"]["snapper"];
        assert_eq!(state["ok"], false, "{state}");
        assert_eq!(
            state["message"],
            seldon::collectors::snapper::NO_PERMISSIONS,
            "{state}"
        );
        assert_eq!(state["fix"], seldon::commands::doctor::SNAPPER_FIX);

        let out = german(&["doctor", "--path", root.to_str().unwrap()]);
        assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
        let text = stdout(&out);
        assert!(
            text.contains(&format!("fix: {}", seldon::commands::doctor::SNAPPER_FIX)),
            "{text}"
        );
        assert!(!text.contains("Keine"), "{text}");
    }

    /// WP-050: the `logbook` row shortens the home to `~` like the header.
    #[test]
    fn logbook_row_shows_paths_like_the_header() {
        let env = Env::new(Snapper::Allowed);
        let root = env.home.join("Seldon");
        let out = env.seldon(&[
            "init",
            "--non-interactive",
            "--no-capture",
            "--path",
            root.to_str().unwrap(),
        ]);
        assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
        let text = stdout(&env.seldon(&["doctor"]));
        assert!(text.starts_with("seldon doctor · ~/Seldon\n"), "{text}");
        assert!(text.contains(" logbook  ~/Seldon · machine "), "{text}");
        assert!(!text.contains(env.home.to_str().unwrap()), "{text}");

        let missing = env.home.join("Elsewhere");
        let v = json(&env.seldon(&["doctor", "--path", missing.to_str().unwrap(), "--json"]));
        assert_eq!(
            v["logbook"],
            missing.to_str().unwrap(),
            "JSON keeps it absolute"
        );
        let logbook = check(&v, "logbook");
        assert!(
            logbook["message"]
                .as_str()
                .unwrap()
                .starts_with("not initialised at ~/Elsewhere (path from "),
            "{logbook}"
        );
        assert_eq!(logbook["fix"], "seldon init --path ~/Elsewhere");
    }

    #[test]
    fn logbook_from_config_after_init() {
        let env = Env::new(Snapper::Allowed);
        let root = init(&env);
        let v = json(&env.seldon(&["doctor", "--json"]));
        assert_eq!(v["logbook"], root.to_str().unwrap());
        assert_eq!(v["ok"], true);
        assert_eq!(check(&v, "snapper")["status"], "ok");
        assert_eq!(
            check(&v, "snapper")["message"],
            format!(
                "2 snapshots (config root). {}",
                seldon::commands::doctor::SNAPPER_LIST_GRANTS
            )
        );
    }

    /// ADR-0026: a user still listed in `ALLOW_USERS` (the old opt-in of
    /// ADR-0011) gets the revert, then the read grant, as the fix of an
    /// `ok` row; a user who is not listed, a failing `get-config` and a
    /// missing snapper get none. The stub answers `get-config` only in the
    /// C locale and logs every call, so doctor runs nothing but the two
    /// read-only queries.
    #[test]
    fn snapper_revert_hint_only_for_a_listed_user() {
        let env = Env::new(Snapper::Allowed);
        let root = init(&env);
        let calls = env.tmp.path().join("snapper-calls.log");
        let stub = |config: &str| {
            env.stub(
                "snapper",
                &format!(
                    "printf '%s\\n' \"$*\" >> '{}'\n\
                     case \"$*\" in\n\
                     '--jsonout list') echo '{{\"root\":[{{\"number\":0}},{{\"number\":1}}]}}' ;;\n\
                     '--jsonout -c root get-config') {config} ;;\n\
                     *) exit 64 ;;\n\
                     esac",
                    calls.display()
                ),
            )
        };
        let listed = "if [ \"$LC_ALL\" = C ]; then \
                      echo '{\"ALLOW_USERS\": \"alice  bob\", \"SYNC_ACL\": \"yes\"}'; \
                      else echo 'Keine Berechtigungen.' >&2; exit 1; fi";
        let doctor = |vars: &[(&str, &str)]| {
            let mut cmd = env.command(&["doctor", "--path", root.to_str().unwrap(), "--json"]);
            for (k, v) in vars {
                cmd.env(k, v);
            }
            let out = cmd.output().unwrap();
            assert_eq!(out.status.code(), Some(0), "{}", stdout(&out));
            check(&json(&out), "snapper").clone()
        };
        let plain = format!(
            "1 snapshots (config root). {}",
            seldon::commands::doctor::SNAPPER_LIST_GRANTS
        );
        let revert = "sudo snapper -c root set-config ALLOW_USERS=\"\" SYNC_ACL=no && sudo setfacl -m u:$USER:rx /.snapshots";

        stub(listed);
        for vars in [
            &[("USER", "alice")][..],
            &[("USER", "bob"), ("LANG", "de_DE.UTF-8"), ("LANGUAGE", "de")],
            &[("LOGNAME", "alice")],
        ] {
            let got = doctor(vars);
            assert_eq!(got["fix"], revert, "{vars:?}: {got}");
            assert_eq!(got["status"], "ok", "{got}");
            assert_eq!(
                got["message"],
                format!(
                    "{plain} Your user is in ALLOW_USERS of the root snapper config, the opt-in that ADR-0026 replaces by a read grant: revert it (this empties ALLOW_USERS and turns SYNC_ACL off), then grant read access. {}",
                    seldon::commands::doctor::SNAPPER_FIX_GRANTS
                )
            );
        }
        let text = stdout(
            &env.command(&["doctor", "--path", root.to_str().unwrap()])
                .env("USER", "alice")
                .output()
                .unwrap(),
        );
        assert!(text.contains(&format!("fix: {revert}\n")), "{text}");

        // not listed (a name is matched whole; `USER` before `LOGNAME`), or
        // no user known
        for vars in [
            &[("USER", "carol")][..],
            &[("USER", "alic")],
            &[("USER", "carol"), ("LOGNAME", "alice")],
            &[],
        ] {
            let got = doctor(vars);
            assert_eq!(got.get("fix"), None, "{vars:?}: {got}");
            assert_eq!(got["message"], plain, "{vars:?}");
        }
        // get-config refused (a user listing works for by ALLOW_GROUPS)
        stub("echo 'No permissions.' >&2; exit 1");
        let got = doctor(&[("USER", "alice")]);
        assert_eq!(got.get("fix"), None, "{got}");
        assert_eq!(got["message"], plain);

        let log = std::fs::read_to_string(&calls).unwrap();
        assert!(
            log.lines()
                .all(|l| l == "--jsonout list" || l == "--jsonout -c root get-config"),
            "{log}"
        );
        assert!(log.contains("get-config"), "{log}");

        // listing refused: degraded with the read grant, and get-config is
        // never asked (it runs only after `snapper list` succeeded)
        std::fs::remove_file(&calls).unwrap();
        env.stub(
            "snapper",
            &format!(
                "printf '%s\\n' \"$*\" >> '{}'\n\
                 case \"$*\" in\n\
                 '--jsonout -c root get-config') {listed} ;;\n\
                 *) echo 'No permissions.' >&2; exit 1 ;;\n\
                 esac",
                calls.display()
            ),
        );
        let got = doctor(&[("USER", "alice")]);
        assert_eq!(got["status"], "degraded", "{got}");
        assert_eq!(got["fix"], seldon::commands::doctor::SNAPPER_FIX, "{got}");
        let log = std::fs::read_to_string(&calls).unwrap();
        assert_eq!(log, "--jsonout list\n", "{log}");

        // no snapper: no revert either
        std::fs::remove_file(env.tmp.path().join("bin/snapper")).unwrap();
        let got = doctor(&[("USER", "alice")]);
        assert_eq!(got["status"], "degraded", "{got}");
        assert_eq!(got.get("fix"), None, "{got}");
    }

    /// Without permission to list, readable info files make snapper `ok`
    /// without a fix. Under the test guard the default snapshot directory
    /// is `<guard>/.snapshots`, never the host's.
    #[test]
    fn snapper_info_files_are_enough() {
        let env = Env::new(Snapper::NoPermissions);
        let root = init(&env);
        let snapper = |extra: &[(&str, &Path)]| {
            let mut cmd = env.command(&["doctor", "--path", root.to_str().unwrap(), "--json"]);
            for (k, v) in extra {
                cmd.env(k, v);
            }
            let v = json(&cmd.output().unwrap());
            check(&v, "snapper").clone()
        };
        let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("../fixtures/logs/snapshots");
        let got = snapper(&[("SELDON_SNAPSHOTS_DIR", &fixture)]);
        assert_eq!(got["status"], "ok", "{got}");
        assert_eq!(got.get("fix"), None, "{got}");
        assert_eq!(
            got["message"],
            format!(
                "snapper list is not permitted; 10 snapshots read from the info files in {}",
                fixture.display()
            )
        );

        // the guard's default: missing or empty → degraded with the fix;
        // with a snapshot → ok
        assert_eq!(snapper(&[])["status"], "degraded");
        std::fs::create_dir_all(env.tmp.path().join(".snapshots")).unwrap();
        let empty = snapper(&[]);
        assert_eq!(empty["status"], "degraded", "{empty}");
        assert_eq!(empty["fix"], seldon::commands::doctor::SNAPPER_FIX);
        let guarded = env.tmp.path().join(".snapshots/7");
        std::fs::create_dir_all(&guarded).unwrap();
        std::fs::write(
            guarded.join("info.xml"),
            "<snapshot><type>single</type><num>7</num></snapshot>",
        )
        .unwrap();
        let got = snapper(&[]);
        assert_eq!(got["status"], "ok", "{got}");
        assert!(
            got["message"]
                .as_str()
                .unwrap()
                .contains("; 1 snapshot read from the info files"),
            "{got}"
        );
    }

    #[test]
    fn not_initialised_exits_3() {
        let env = Env::new(Snapper::Missing);
        let out = env.seldon(&["doctor", "--json"]);
        assert_eq!(out.status.code(), Some(3));
        let v = json(&out);
        assert_eq!(v["ok"], false);
        assert_eq!(check(&v, "logbook")["status"], "error");
        assert_eq!(check(&v, "config")["status"], "degraded");
        assert_eq!(check(&v, "snapper")["status"], "degraded");
        assert!(
            check(&v, "logbook")["fix"]
                .as_str()
                .unwrap()
                .starts_with("seldon init --path ")
        );
    }

    #[test]
    fn invalid_frontmatter_is_an_error() {
        let env = Env::new(Snapper::Allowed);
        let root = init(&env);
        std::fs::write(
            root.join("work/queued/C-2026-001-broken.md"),
            "---\nid: C-2026-001\ntype: case\ntitle: \"x\"\nstatus: nonsense\nzone: red\nrisk: R1\ncreated: 2026-10-01\n---\n",
        )
        .unwrap();
        let out = env.seldon(&["doctor", "--json"]);
        assert_eq!(out.status.code(), Some(1));
        let logbook = json(&out)["checks"][2].clone();
        assert_eq!(logbook["status"], "error");
        assert!(
            logbook["message"]
                .as_str()
                .unwrap()
                .contains("work/queued/C-2026-001-broken.md"),
            "{logbook}"
        );
    }

    #[test]
    fn case_in_the_wrong_folder_is_degraded() {
        let env = Env::new(Snapper::Allowed);
        let root = init(&env);
        std::fs::write(
            root.join("work/queued/C-2026-001-early.md"),
            "---\nid: C-2026-001\ntype: case\ntitle: \"x\"\nstatus: active\nzone: red\nrisk: R1\ncreated: 2026-10-01\n---\n",
        )
        .unwrap();
        let v = json(&env.seldon(&["doctor", "--json"]));
        assert_eq!(v["ok"], true);
        let logbook = check(&v, "logbook");
        assert_eq!(logbook["status"], "degraded");
        assert!(
            logbook["message"]
                .as_str()
                .unwrap()
                .contains("status active")
        );
    }

    #[test]
    fn fixture_logbook_is_valid_and_untouched() {
        let env = Env::new(Snapper::NoPermissions);
        let fixture = common::fixture_logbook();
        let before = snapshot(&fixture);
        let out = env.seldon(&["doctor", "--path", fixture.to_str().unwrap(), "--json"]);
        let v = json(&out);
        assert_eq!(out.status.code(), Some(0), "{v}");
        let logbook = check(&v, "logbook");
        assert_eq!(logbook["status"], "ok", "{logbook}");
        let message = logbook["message"].as_str().unwrap();
        assert!(
            message.contains("8 cases, 4 decisions, 10 journal days"),
            "{message}"
        );
        assert!(message.contains("workstation-7f3a"));
        assert_eq!(
            before,
            snapshot(&fixture),
            "doctor wrote into fixtures/logbook"
        );
    }

    /// WP-061: what keeps every autocommit from committing is degraded,
    /// with the fix; `log` itself only warns (`tests/git.rs`).
    fn git_check(env: &Env, root: &Path) -> serde_json::Value {
        let out = env.seldon(&["doctor", "--path", root.to_str().unwrap(), "--json"]);
        let v = json(&out);
        assert_eq!(out.status.code(), Some(0), "{v}");
        assert_eq!(v["ok"], true, "degraded is not an error: {v}");
        check(&v, "git").clone()
    }

    #[test]
    fn a_stale_index_lock_is_degraded_with_its_fix() {
        let env = Env::new(Snapper::NoPermissions);
        if !env.has_git {
            return;
        }
        let root = init(&env);
        let lock = root.join(".git/index.lock");
        std::fs::write(&lock, "").unwrap();
        let git = git_check(&env, &root);
        assert_eq!(git["status"], "degraded", "{git}");
        let message = git["message"].as_str().unwrap();
        assert!(message.contains(".git/index.lock exists"), "{message}");
        assert!(message.contains("every autocommit fails"), "{message}");
        assert_eq!(git["fix"], format!("rm {}", lock.display()), "{git}");

        // gone: green again
        std::fs::remove_file(&lock).unwrap();
        let git = git_check(&env, &root);
        assert_eq!(git["status"], "ok", "{git}");
        assert!(git.get("fix").is_none(), "{git}");
    }

    #[test]
    fn a_detached_head_is_degraded_with_its_fix() {
        let env = Env::new(Snapper::NoPermissions);
        if !env.has_git {
            return;
        }
        let root = init(&env);
        let branch = stdout(&env.git(&root, &["branch", "--show-current"]))
            .trim()
            .to_string();
        assert!(!branch.is_empty());
        let out = env.git(&root, &["checkout", "-q", "--detach"]);
        assert!(out.status.success(), "{}", stderr(&out));
        let git = git_check(&env, &root);
        assert_eq!(git["status"], "degraded", "{git}");
        let message = git["message"].as_str().unwrap();
        assert!(message.contains("HEAD is detached"), "{message}");
        assert_eq!(
            git["fix"],
            format!("git -C {} switch {branch}", root.display()),
            "{git}"
        );

        // with autocommit off a detached HEAD is the user's business
        let config = env.config_file();
        let text = std::fs::read_to_string(&config).unwrap();
        assert!(text.contains("autocommit = true"), "{text}");
        std::fs::write(
            &config,
            text.replace("autocommit = true", "autocommit = false"),
        )
        .unwrap();
        let git = git_check(&env, &root);
        assert_eq!(git["status"], "ok", "{git}");
    }

    #[test]
    fn a_read_only_dot_git_is_degraded() {
        use std::os::unix::fs::PermissionsExt as _;
        let env = Env::new(Snapper::NoPermissions);
        if !env.has_git {
            return;
        }
        let root = init(&env);
        let dot_git = root.join(".git");
        std::fs::set_permissions(&dot_git, std::fs::Permissions::from_mode(0o555)).unwrap();
        let git = git_check(&env, &root);
        std::fs::set_permissions(&dot_git, std::fs::Permissions::from_mode(0o755)).unwrap();
        assert_eq!(git["status"], "degraded", "{git}");
        let message = git["message"].as_str().unwrap();
        assert!(message.contains(".git is read-only"), "{message}");
        assert_eq!(
            git["fix"],
            format!("chmod u+w {}", dot_git.display()),
            "{git}"
        );
    }

    #[test]
    fn an_identity_git_cannot_resolve_is_degraded() {
        let env = Env::new(Snapper::NoPermissions);
        if !env.has_git {
            return;
        }
        let root = init(&env);
        // an email but an empty name: the autocommit uses it as it is
        for (key, value) in [("user.email", "someone@example.invalid"), ("user.name", "")] {
            let out = env.git(&root, &["config", key, value]);
            assert!(out.status.success(), "{}", stderr(&out));
        }
        let git = git_check(&env, &root);
        assert_eq!(git["status"], "degraded", "{git}");
        let message = git["message"].as_str().unwrap();
        assert!(
            message.contains("git cannot name the committer"),
            "{message}"
        );
        assert!(message.contains("empty ident name"), "{message}");
        assert_eq!(
            git["fix"],
            format!("git -C {} config user.name \"Your Name\"", root.display()),
            "{git}"
        );
    }

    #[test]
    fn an_empty_dot_git_is_degraded_with_its_fix() {
        let env = Env::new(Snapper::NoPermissions);
        if !env.has_git {
            return;
        }
        let root = init(&env);
        // inside another repository: git must not answer for that one
        env.git(env.tmp.path(), &["init", "-q"]);
        std::fs::remove_dir_all(root.join(".git")).unwrap();
        std::fs::create_dir(root.join(".git")).unwrap();
        let git = git_check(&env, &root);
        assert_eq!(git["status"], "degraded", "{git}");
        let message = git["message"].as_str().unwrap();
        assert!(
            message.contains("the logbook's .git is not a usable repository"),
            "{message}"
        );
        assert_eq!(
            git["fix"],
            format!("git -C {} init", root.display()),
            "{git}"
        );
    }

    /// doctor only reads: no lock, no index refresh, `.git` byte-identical
    /// even when a tracked file's stat data is stale.
    #[test]
    fn doctor_leaves_dot_git_untouched() {
        let env = Env::new(Snapper::NoPermissions);
        if !env.has_git {
            return;
        }
        let root = init(&env);
        let project = root.join("PROJECT.md");
        let file = std::fs::File::options().write(true).open(&project).unwrap();
        file.set_modified(std::time::SystemTime::now() + std::time::Duration::from_secs(3600))
            .unwrap();
        drop(file);
        let before = snapshot(&root.join(".git"));
        let git = git_check(&env, &root);
        assert_eq!(git["status"], "ok", "{git}");
        assert!(
            before == snapshot(&root.join(".git")),
            "doctor wrote into the logbook's .git"
        );
    }

    /// WP-070: `doctor --json` with its exit code.
    fn doctor(env: &Env, extra: &[&str]) -> (Option<i32>, serde_json::Value) {
        let mut args = vec!["doctor", "--json"];
        args.extend_from_slice(extra);
        let out = env.seldon(&args);
        (out.status.code(), json(&out))
    }

    /// F-540: with a config.toml that does not parse, the logbook path it
    /// names is not known. doctor does not guess the default (exit 3, an
    /// `init` fix that fails on the same config) but exits 1 with the
    /// config error first and the logbook "not checked".
    #[test]
    fn an_invalid_config_is_not_reported_as_a_missing_logbook() {
        let env = Env::new(Snapper::NoPermissions);
        let root = init(&env);
        let config = env.config_file();
        let text = std::fs::read_to_string(&config).unwrap();
        assert!(text.contains("language = \"en\""), "{text}");
        std::fs::write(
            &config,
            text.replace("language = \"en\"", "language = \"fr\""),
        )
        .unwrap();

        let (code, v) = doctor(&env, &[]);
        assert_eq!(code, Some(1), "{v}");
        assert_eq!(v["ok"], false);
        assert_eq!(v["logbook"], serde_json::Value::Null, "{v}");
        let checks = v["checks"].as_array().unwrap();
        let names: Vec<&str> = checks.iter().map(|c| c["name"].as_str().unwrap()).collect();
        assert_eq!(names[..3], ["engine", "config", "logbook"], "{v}");
        assert_eq!(checks[1]["status"], "error");
        assert!(
            checks[1]["message"]
                .as_str()
                .unwrap()
                .contains("unknown variant `fr`"),
            "{v}"
        );
        assert_eq!(checks[2]["status"], "error");
        assert_eq!(
            checks[2]["message"],
            "not checked: config.toml is invalid, so the logbook path is not known"
        );
        assert!(checks[2].get("fix").is_none(), "{v}");
        // review N2: the config row says how to get out
        assert_eq!(
            checks[1]["fix"],
            "correct ~/.config/seldon/config.toml (the message names the key), or move it away and run seldon init"
        );
        let text = v.to_string();
        assert!(!text.contains("not initialised"), "{text}");
        assert!(!text.contains("seldon init --path"), "{text}");

        let out = env.seldon(&["doctor"]);
        assert_eq!(out.status.code(), Some(1));
        assert!(
            stdout(&out)
                .starts_with("seldon doctor · logbook not known (config.toml is invalid)\n"),
            "{}",
            stdout(&out)
        );

        // a path given on the command line is still checked; the exit code
        // stays the config error's
        let (code, v) = doctor(&env, &["--path", root.to_str().unwrap()]);
        assert_eq!(code, Some(1), "{v}");
        assert_eq!(check(&v, "logbook")["status"], "ok", "{v}");
        let missing = env.tmp.path().join("nowhere");
        let (code, v) = doctor(&env, &["--path", missing.to_str().unwrap()]);
        assert_eq!(code, Some(1), "the config error wins over exit 3: {v}");
        // review N3: `seldon init` stops at the same config error
        let fix = check(&v, "logbook")["fix"].as_str().unwrap().to_string();
        assert!(
            fix.starts_with("after fixing config.toml: seldon init --path "),
            "{fix}"
        );
    }

    /// Review N2: a config.toml that cannot be read is a config row with a
    /// chmod fix and exit 1 (was a bare exit 2), the logbook not checked.
    #[test]
    fn an_unreadable_config_is_a_config_error() {
        use std::os::unix::fs::PermissionsExt as _;
        let env = Env::new(Snapper::NoPermissions);
        init(&env);
        let config = env.config_file();
        std::fs::set_permissions(&config, std::fs::Permissions::from_mode(0o000)).unwrap();
        if std::fs::read(&config).is_ok() {
            // root reads it anyway
            return;
        }
        let (code, v) = doctor(&env, &[]);
        std::fs::set_permissions(&config, std::fs::Permissions::from_mode(0o644)).unwrap();
        assert_eq!(code, Some(1), "{v}");
        let c = check(&v, "config");
        assert_eq!(c["status"], "error", "{c}");
        assert!(
            c["message"].as_str().unwrap().contains("cannot read"),
            "{c}"
        );
        assert_eq!(c["fix"], format!("chmod u+r {}", config.display()));
        assert_eq!(
            check(&v, "logbook")["message"],
            "not checked: config.toml cannot be read, so the logbook path is not known"
        );
        assert_eq!(v["logbook"], serde_json::Value::Null, "{v}");
    }

    /// Review B1: a collector whose last capture failed (`cursors.json`,
    /// this logbook's) is degraded in doctor with its message and fix; a
    /// disabled one and another logbook's cursors are not reported.
    #[test]
    fn a_failing_collector_is_reported_with_its_fix() {
        let env = Env::new(Snapper::Allowed);
        let root = init(&env);
        let (_, v) = doctor(&env, &[]);
        assert_eq!(
            check(&v, "collectors")["message"],
            "no capture for this logbook yet",
            "{v}"
        );
        let state = env.home.join(".local/state/seldon");
        std::fs::create_dir_all(&state).unwrap();
        let cursors = |logbook: &Path| {
            serde_json::json!({
                "logbook": logbook,
                "collectors": {
                    "pacman": {"ok": true, "lastRun": "2026-10-03T09:00:00+02:00"},
                    "plugins": {"ok": false, "message": "omarchy plugin list failed: exit 2",
                                "lastRun": "2026-10-03T09:00:00+02:00"},
                    "snapper": {"ok": false, "message": "No permissions.",
                                "fix": "sudo setfacl -m u:$USER:rx /.snapshots",
                                "lastRun": "2026-10-03T09:00:00+02:00"},
                    "theme": {"ok": false, "message": "theme file missing",
                              "lastRun": "2026-10-03T09:00:00+02:00"}
                }
            })
            .to_string()
        };
        let canonical = std::fs::canonicalize(&root).unwrap();
        std::fs::write(state.join("cursors.json"), cursors(&canonical)).unwrap();
        let config = env.config_file();
        let text = std::fs::read_to_string(&config).unwrap();
        assert!(text.contains("theme = true"), "{text}");
        std::fs::write(&config, text.replace("theme = true", "theme = false")).unwrap();

        let (code, v) = doctor(&env, &[]);
        assert_eq!(code, Some(0), "degraded is not an error: {v}");
        let c = check(&v, "collectors");
        assert_eq!(c["status"], "degraded", "{c}");
        assert_eq!(
            c["message"],
            "last capture failed: plugins: omarchy plugin list failed: exit 2; snapper: No permissions."
        );
        assert_eq!(c["fix"], "sudo setfacl -m u:$USER:rx /.snapshots");

        // another logbook's cursors are not this one's state
        std::fs::write(
            state.join("cursors.json"),
            cursors(&env.tmp.path().join("other")),
        )
        .unwrap();
        let (_, v) = doctor(&env, &[]);
        assert_eq!(check(&v, "collectors")["status"], "ok", "{v}");
    }

    /// Review Q3: the omarchy probe runs `SELDON_OMARCHY_VERSION` like the
    /// collector.
    #[test]
    fn the_omarchy_probe_honours_seldon_omarchy_version() {
        let env = Env::new(Snapper::NoPermissions);
        let root = init(&env);
        env.stub("omarchy-version-other", "echo 9.9.9-1");
        let out = env
            .command(&["doctor", "--path", root.to_str().unwrap(), "--json"])
            .env("SELDON_OMARCHY_VERSION", "omarchy-version-other")
            .output()
            .unwrap();
        let v = json(&out);
        assert_eq!(check(&v, "omarchy")["message"], "Omarchy 9.9.9-1", "{v}");
    }

    /// F-541: a `[redaction] patterns` entry that does not compile makes
    /// every writing command refuse; doctor says so, with the fix.
    #[test]
    fn an_invalid_redaction_pattern_is_a_config_error() {
        let env = Env::new(Snapper::NoPermissions);
        init(&env);
        let config = env.config_file();
        let text = std::fs::read_to_string(&config).unwrap();
        assert!(text.contains("\npatterns = []"), "{text}");
        std::fs::write(
            &config,
            text.replace("\npatterns = []", "\npatterns = [\"(unclosed\"]"),
        )
        .unwrap();
        let (code, v) = doctor(&env, &[]);
        assert_eq!(code, Some(1), "{v}");
        let c = check(&v, "config");
        assert_eq!(c["status"], "error", "{c}");
        let message = c["message"].as_str().unwrap();
        assert!(message.contains("invalid regex `(unclosed`"), "{message}");
        assert!(!message.contains('\n'), "one line: {message}");
        assert_eq!(
            c["fix"],
            "fix or remove that pattern under [redaction] patterns in ~/.config/seldon/config.toml"
        );
        // the logbook is still found through the config
        assert_eq!(check(&v, "logbook")["status"], "ok", "{v}");
    }

    /// F-541: each collector state file is loaded strictly; a corrupt one
    /// is an error that names the file, what it breaks, and the fix.
    #[test]
    fn a_corrupt_state_file_is_an_error_with_its_fix() {
        let env = Env::new(Snapper::NoPermissions);
        let root = env.tmp.path().join("logbook");
        let out = env.seldon(&[
            "init",
            "--non-interactive",
            "--path",
            root.to_str().unwrap(),
        ]);
        assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
        let state = env.home.join(".local/state/seldon");
        // init's capture wrote the cursors and the config manifest
        let (code, v) = doctor(&env, &[]);
        assert_eq!(code, Some(0), "{v}");
        let ok = check(&v, "state");
        assert_eq!(ok["status"], "ok", "{ok}");
        assert_eq!(
            ok["message"],
            "~/.local/state/seldon: cursors.json, manifest.json readable"
        );

        for (name, says) in [
            ("cursors.json", "every capture fails"),
            ("manifest.json", "new config baseline"),
            ("owned.json", "reported as drift"),
        ] {
            let file = state.join(name);
            let kept = std::fs::read(&file).ok();
            std::fs::write(&file, "{\"logbook\": ").unwrap();
            let (code, v) = doctor(&env, &[]);
            assert_eq!(code, Some(1), "{name}: {v}");
            let c = check(&v, "state");
            assert_eq!(c["status"], "error", "{name}: {c}");
            let message = c["message"].as_str().unwrap();
            assert!(
                message.starts_with(&format!("~/.local/state/seldon/{name} is corrupt (")),
                "{message}"
            );
            assert!(message.contains(says), "{message}");
            assert_eq!(
                c["fix"],
                format!("mv {0} {0}.bad", file.display()),
                "{name}: {c}"
            );
            match kept {
                Some(bytes) => std::fs::write(&file, bytes).unwrap(),
                None => std::fs::remove_file(&file).unwrap(),
            }
        }
        let (code, v) = doctor(&env, &[]);
        assert_eq!(code, Some(0), "{v}");
    }

    /// WP-081: a corrupt manifest is an error that says the capture will
    /// record a state reset; after that capture a degraded `state` row names
    /// the reset with the restore hint, until the next capture.
    #[test]
    fn a_state_reset_is_shown_until_the_next_capture() {
        let env = Env::new(Snapper::Missing);
        init(&env);
        let capture = |now: &str| {
            let out = env.at(now, &["capture", "--source", "config", "--json"]);
            assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
            json(&out)
        };
        let states = |v: &serde_json::Value| -> Vec<serde_json::Value> {
            let checks = v["checks"].as_array().unwrap();
            checks
                .iter()
                .filter(|c| c["name"] == "state")
                .cloned()
                .collect()
        };
        let conf = env.home.join(".config/hypr/hyprland.conf");
        std::fs::create_dir_all(conf.parent().unwrap()).unwrap();
        std::fs::write(&conf, "a = 1\n").unwrap();
        capture("2026-10-04T10:00:00+02:00");
        std::fs::write(&conf, "a = 2\n").unwrap();
        assert_eq!(capture("2026-10-04T10:05:00+02:00")["written"], 1);
        let (_, v) = doctor(&env, &[]);
        let rows = states(&v);
        assert_eq!(rows.len(), 1, "{v}");
        assert_eq!(rows[0]["status"], "ok");

        let manifest = env.home.join(".local/state/seldon/manifest.json");
        std::fs::write(&manifest, "{").unwrap();
        let (code, v) = doctor(&env, &[]);
        assert_eq!(code, Some(1), "{v}");
        let rows = states(&v);
        assert_eq!(rows.len(), 1, "{v}");
        assert_eq!(rows[0]["status"], "error");
        let message = rows[0]["message"].as_str().unwrap();
        assert!(
            message.contains("new config baseline, recorded as a state reset"),
            "{message}"
        );

        let out = capture("2026-10-04T10:10:00+02:00");
        assert_eq!(out["written"], 1, "the note: {out}");
        let (code, v) = doctor(&env, &[]);
        assert_eq!(code, Some(0), "degraded is no error: {v}");
        let rows = states(&v);
        assert_eq!(rows.len(), 2, "{v}");
        assert_eq!(rows[0]["status"], "degraded");
        assert_eq!(
            rows[0]["message"],
            "the last capture recorded a state reset: config took a new baseline (manifest missing or unreadable in ~/.local/state/seldon), so changes made in between may be missing"
        );
        let fix = rows[0]["fix"].as_str().unwrap();
        assert!(
            fix.starts_with("restore a backup of ~/.local/state/seldon and run seldon capture"),
            "{fix}"
        );
        assert_eq!(rows[1]["status"], "ok", "the manifest is valid again");
        let human = stdout(&env.seldon(&["doctor"]));
        assert!(
            human.contains("degraded  state    the last capture recorded a state reset"),
            "{human}"
        );

        assert_eq!(capture("2026-10-04T10:15:00+02:00")["written"], 0);
        let (_, v) = doctor(&env, &[]);
        let rows = states(&v);
        assert_eq!(rows.len(), 1, "{v}");
        assert_eq!(rows[0]["status"], "ok");

        // review F3 (R3): the cursors of another logbook, last run before
        // this logbook's reset, do not make it "the last capture" here
        let root = env.tmp.path().join("logbook");
        let other = env.tmp.path().join("other");
        let out = env.seldon(&[
            "init",
            "--non-interactive",
            "--no-capture",
            "--path",
            other.to_str().unwrap(),
        ]);
        assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
        let out = env.at(
            "2026-10-04T09:00:00+02:00",
            &["capture", "--source", "config", "--json"],
        );
        assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
        let (_, v) = doctor(&env, &["--path", root.to_str().unwrap()]);
        let rows = states(&v);
        assert!(
            rows.iter().all(|r| !r["message"]
                .as_str()
                .unwrap()
                .starts_with("the last capture recorded")),
            "{v}"
        );
        // WP-083: the next capture here will take a new baseline
        assert_eq!(rows.len(), 2, "{v}");
        assert_eq!(
            rows[0]["message"],
            "the next capture will record a state reset for config: cursors in ~/.local/state/seldon bound to another logbook, so changes made since the last capture may not be recorded"
        );
        // review F2: a plain `seldon capture` would capture `other`
        assert_eq!(
            rows[0]["fix"],
            format!(
                "nothing to restore: the state belongs to another logbook path; run seldon --logbook {} capture to accept the new baseline (user guide: Moving or copying the logbook)",
                root.display()
            )
        );
        assert_eq!(rows[1]["status"], "ok");
    }

    /// WP-083: before the capture that would record a state reset, doctor
    /// says so while a restore still prevents it; never for a fresh logbook
    /// or a collector that never ran here; after the capture the WP-081
    /// row takes over.
    #[test]
    fn a_state_reset_is_predicted_before_the_capture() {
        const PREDICTED: &str = "the next capture will record a state reset for ";
        let env = Env::new(Snapper::Missing);
        init(&env);
        let capture = |now: &str| {
            let out = env.at(now, &["capture", "--source", "config", "--json"]);
            assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
            json(&out)
        };
        let states = |v: &serde_json::Value| -> Vec<serde_json::Value> {
            let checks = v["checks"].as_array().unwrap();
            checks
                .iter()
                .filter(|c| c["name"] == "state")
                .cloned()
                .collect()
        };
        let quiet = |why: &str| {
            let (_, v) = doctor(&env, &[]);
            let rows = states(&v);
            assert_eq!(rows.len(), 1, "{why}: {v}");
            assert_eq!(rows[0]["status"], "ok", "{why}: {v}");
        };
        quiet("a fresh logbook");
        let cursors = env.home.join(".local/state/seldon/cursors.json");
        std::fs::remove_file(&cursors).ok();
        quiet("a fresh logbook without cursors.json");

        let conf = env.home.join(".config/hypr/hyprland.conf");
        std::fs::create_dir_all(conf.parent().unwrap()).unwrap();
        std::fs::write(&conf, "a = 1\n").unwrap();
        capture("2026-10-04T10:00:00+02:00");
        std::fs::write(&conf, "a = 2\n").unwrap();
        assert_eq!(capture("2026-10-04T10:05:00+02:00")["written"], 1);
        quiet("every cursor here");
        // review F1: the cursors are bound to the canonical path; a
        // logbook configured through a symlink is the same logbook
        let link = env.tmp.path().join("link");
        std::os::unix::fs::symlink(env.tmp.path().join("logbook"), &link).unwrap();
        let config = env.config_file();
        let text = std::fs::read_to_string(&config).unwrap();
        let mut toml: toml::Table = text.parse().unwrap();
        toml.insert(
            "logbook".into(),
            toml::Value::String(link.to_string_lossy().into_owned()),
        );
        std::fs::write(&config, toml.to_string()).unwrap();
        let (_, v) = doctor(&env, &[]);
        assert_eq!(v["logbook"], link.to_string_lossy().as_ref(), "{v}");
        quiet("logbook configured through a symlink");
        std::fs::write(&config, &text).unwrap();
        let saved = std::fs::read_to_string(&cursors).unwrap();

        // a collector that never ran here (no entry while bound to this
        // logbook) takes its first baseline without a loss
        let mut v: serde_json::Value = serde_json::from_str(&saved).unwrap();
        v["collectors"].as_object_mut().unwrap().remove("config");
        std::fs::write(&cursors, v.to_string()).unwrap();
        quiet("config never ran here");

        // unreadable
        let mut v: serde_json::Value = serde_json::from_str(&saved).unwrap();
        v["collectors"]["config"]["cursor"] = serde_json::json!("not a cursor");
        std::fs::write(&cursors, v.to_string()).unwrap();
        let (code, v) = doctor(&env, &[]);
        assert_eq!(code, Some(0), "degraded is no error: {v}");
        let rows = states(&v);
        assert_eq!(rows.len(), 2, "{v}");
        assert_eq!(rows[0]["status"], "degraded");
        assert_eq!(
            rows[0]["message"],
            format!(
                "{PREDICTED}config: cursors unreadable in ~/.local/state/seldon, so changes made since the last capture may not be recorded"
            )
        );

        // missing; a disabled collector would not run
        std::fs::remove_file(&cursors).unwrap();
        let (code, v) = doctor(&env, &[]);
        assert_eq!(code, Some(0), "{v}");
        let rows = states(&v);
        assert_eq!(rows.len(), 2, "{v}");
        assert_eq!(rows[0]["status"], "degraded");
        assert_eq!(
            rows[0]["message"],
            format!(
                "{PREDICTED}config: cursors missing in ~/.local/state/seldon, so changes made since the last capture may not be recorded"
            )
        );
        assert_eq!(
            rows[0]["fix"],
            "restore ~/.local/state/seldon from a backup now (user guide: Back up and restore the state directory), or run seldon capture to accept the new baseline"
        );
        // review F2: a logbook from --path is named in the fix
        let root = env.tmp.path().join("logbook");
        let (_, v) = doctor(&env, &["--path", root.to_str().unwrap()]);
        let rows = states(&v);
        assert_eq!(
            rows[0]["fix"],
            format!(
                "restore ~/.local/state/seldon from a backup now (user guide: Back up and restore the state directory), or run seldon --logbook {} capture to accept the new baseline",
                root.display()
            )
        );
        let human = stdout(&env.seldon(&["doctor"]));
        assert!(
            human.contains(&format!("degraded  state    {PREDICTED}config: ")),
            "{human}"
        );
        let mut toml: toml::Table = text.parse().unwrap();
        let mut off = toml::Table::new();
        off.insert("config".into(), toml::Value::Boolean(false));
        toml.insert("collectors".into(), toml::Value::Table(off));
        std::fs::write(&config, toml.to_string()).unwrap();
        quiet("config disabled");
        let (_, v) = doctor(&env, &[]);
        assert_eq!(check(&v, "config")["status"], "ok", "{v}");
        std::fs::write(&config, &text).unwrap();

        // the restore prevents it
        std::fs::write(&cursors, &saved).unwrap();
        quiet("restored");
        std::fs::remove_file(&cursors).unwrap();

        // the capture records it; the WP-081 row takes over
        let out = capture("2026-10-04T10:10:00+02:00");
        assert_eq!(out["written"], 1, "the note: {out}");
        let (_, v) = doctor(&env, &[]);
        let rows = states(&v);
        assert_eq!(rows.len(), 2, "{v}");
        let message = rows[0]["message"].as_str().unwrap();
        assert!(
            message.starts_with("the last capture recorded a state reset: config "),
            "{message}"
        );
        assert_eq!(rows[1]["status"], "ok");
        capture("2026-10-04T10:15:00+02:00");
        quiet("after the next capture");
    }

    /// Review N4: an unreadable state file's row says what reading it
    /// needs, matching its chmod fix (not "move it away").
    #[test]
    fn an_unreadable_state_file_says_what_its_fix_does() {
        use std::os::unix::fs::PermissionsExt as _;
        let env = Env::new(Snapper::NoPermissions);
        init(&env);
        let state = env.home.join(".local/state/seldon");
        std::fs::create_dir_all(&state).unwrap();
        let file = state.join("cursors.json");
        std::fs::write(&file, "{}").unwrap();
        std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o000)).unwrap();
        if std::fs::read(&file).is_ok() {
            return; // root reads it anyway
        }
        let (code, v) = doctor(&env, &[]);
        std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o644)).unwrap();
        assert_eq!(code, Some(1), "{v}");
        let c = check(&v, "state");
        let message = c["message"].as_str().unwrap();
        assert!(
            message.starts_with("~/.local/state/seldon/cursors.json: cannot read: "),
            "{message}"
        );
        assert!(
            message.ends_with("; every capture fails until it can be read again"),
            "{message}"
        );
        assert!(!message.contains("moved away"), "{message}");
        assert_eq!(c["fix"], format!("chmod u+rw {}", file.display()));
    }

    /// F-132: ledger lines that are not events are skipped by every
    /// reader; doctor names the month, the count and the lines.
    #[test]
    fn bad_ledger_lines_are_counted() {
        let env = Env::new(Snapper::NoPermissions);
        let root = init(&env);
        let (_, v) = doctor(&env, &[]);
        assert_eq!(check(&v, "ledger")["status"], "ok", "{v}");
        let month = root.join("ledger/2026-10.jsonl");
        // not JSON, then a write torn inside `ü`
        std::fs::write(&month, b"not an event\n{\"subject\":\"L\xc3").unwrap();
        let (code, v) = doctor(&env, &[]);
        assert_eq!(code, Some(0), "degraded is not an error: {v}");
        let c = check(&v, "ledger");
        assert_eq!(c["status"], "degraded", "{c}");
        let message = c["message"].as_str().unwrap();
        assert!(
            message.starts_with("ledger/2026-10.jsonl: 2 lines skipped, ")
                && message.contains(": line 1, 2;"),
            "{message}"
        );
        assert!(c["fix"].as_str().unwrap().starts_with("repair or delete"));
    }

    /// WP-057 follow-up: a case id in two files is an error in doctor too.
    #[test]
    fn a_case_id_twice_is_an_error() {
        let env = Env::new(Snapper::NoPermissions);
        let root = init(&env);
        let out = env.seldon(&["plan", "new", "--json", "--", "Twice"]);
        assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
        let id = json(&out)["case"]["id"].as_str().unwrap().to_string();
        let (_, v) = doctor(&env, &[]);
        assert_eq!(check(&v, "cases")["status"], "ok", "{v}");

        let queued = common::find_file(&root.join("work/queued"), &id);
        let name = queued.file_name().unwrap().to_string_lossy().into_owned();
        std::fs::write(
            root.join("work/completed").join(&name),
            std::fs::read_to_string(&queued)
                .unwrap()
                .replace("status: queued", "status: completed"),
        )
        .unwrap();
        let (code, v) = doctor(&env, &[]);
        assert_eq!(code, Some(1), "{v}");
        let c = check(&v, "cases");
        assert_eq!(c["status"], "error", "{c}");
        let message = c["message"].as_str().unwrap();
        assert!(
            message.starts_with(&format!("case {id} exists more than once (")),
            "{message}"
        );
        for rel in [
            format!("work/queued/{name}"),
            format!("work/completed/{name}"),
        ] {
            assert!(message.contains(&rel), "{message}");
        }
        assert!(c["fix"].as_str().unwrap().contains("stale copy"), "{c}");
    }

    /// WP-065 follow-up: a damaged STATUS.md or DECISIONS.md fence is
    /// reported with the markers to restore; an end marker that closes no
    /// fence is reported with what doctor cannot tell.
    #[test]
    fn damaged_fences_and_stray_end_markers_are_reported() {
        const END: &str = "<!-- seldon:end -->";
        let env = Env::new(Snapper::NoPermissions);
        let root = init(&env);
        let out = env.seldon(&["status"]);
        assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
        let status_md = root.join("STATUS.md");
        let decisions_md = root.join("DECISIONS.md");
        let status = std::fs::read_to_string(&status_md).unwrap();
        let decisions = std::fs::read_to_string(&decisions_md).unwrap();
        assert_eq!(status.matches(END).count(), 1, "{status}");
        assert_eq!(decisions.matches(END).count(), 1, "{decisions}");
        let fences = |env: &Env| {
            let (code, v) = doctor(env, &[]);
            assert_eq!(code, Some(0), "degraded is not an error: {v}");
            check(&v, "fences").clone()
        };
        assert_eq!(fences(&env)["status"], "ok");

        // STATUS.md: the end marker removed, notes below it
        let removed = status.replace(END, "my notes");
        std::fs::write(&status_md, &removed).unwrap();
        let c = fences(&env);
        assert_eq!(c["status"], "degraded", "{c}");
        assert!(
            c["message"]
                .as_str()
                .unwrap()
                .starts_with("STATUS.md: the status fence has no end marker of its own; "),
            "{c}"
        );
        assert_eq!(
            c["fix"],
            "restore the marker lines `<!-- seldon:begin status -->` and `<!-- seldon:end -->` in STATUS.md"
        );
        // doctor and `status` agree: status leaves the file alone
        let out = env.seldon(&["status"]);
        assert!(
            stdout(&out).contains("warning: STATUS.md: the status fence has no end marker"),
            "{}",
            stdout(&out)
        );
        assert_eq!(std::fs::read_to_string(&status_md).unwrap(), removed);
        std::fs::write(&status_md, &status).unwrap();

        // DECISIONS.md: the same for its decisions.index fence
        std::fs::write(&decisions_md, decisions.replace(END, "")).unwrap();
        let c = fences(&env);
        assert_eq!(c["status"], "degraded", "{c}");
        assert!(
            c["message"].as_str().unwrap().starts_with(
                "DECISIONS.md: the decisions.index fence has no end marker of its own"
            ),
            "{c}"
        );
        std::fs::write(&decisions_md, &decisions).unwrap();
        assert_eq!(fences(&env)["status"], "ok");

        // not UTF-8: `status` stops on it, so doctor says error
        std::fs::write(&decisions_md, b"# Decisions \xc3\n").unwrap();
        let (code, v) = doctor(&env, &[]);
        assert_eq!(code, Some(1), "{v}");
        let c = check(&v, "fences");
        assert_eq!(c["status"], "error", "{c}");
        assert!(
            c["message"]
                .as_str()
                .unwrap()
                .starts_with("DECISIONS.md: cannot read ("),
            "{c}"
        );
        assert_eq!(c["fix"], "make the file readable UTF-8 text again");
        assert_eq!(env.seldon(&["status"]).status.code(), Some(2));
        std::fs::write(&decisions_md, &decisions).unwrap();

        // the end marker removed, and two stale ones further down: the
        // writer would take "my notes" as fence body; doctor sees the
        // stray one at the end but cannot tell which end is the fence's
        assert!(status.ends_with(&format!("{END}\n")), "{status}");
        std::fs::write(
            &status_md,
            format!(
                "{}my notes\n{END}\nmore\n{END}\n",
                status.strip_suffix(&format!("{END}\n")).unwrap()
            ),
        )
        .unwrap();
        let text = std::fs::read_to_string(&status_md).unwrap();
        let last = text.lines().count();
        let c = fences(&env);
        assert_eq!(c["status"], "degraded", "{c}");
        let message = c["message"].as_str().unwrap();
        assert!(
            message.starts_with(&format!(
                "STATUS.md: the end marker on line {last} closes no fence. "
            )),
            "{message}"
        );
        assert!(message.contains("doctor cannot tell"), "{message}");
        assert!(c["fix"].as_str().unwrap().contains("stray"), "{c}");
    }

    /// WP-053 follow-up: doctor probes the program the snapper collector
    /// runs (`SELDON_SNAPPER`), not a bare `snapper`.
    #[test]
    fn the_snapper_probe_honours_seldon_snapper() {
        let env = Env::new(Snapper::Missing);
        let root = init(&env);
        env.stub(
            "snapper-other",
            r#"echo '{"root":[{"number":0,"description":"current"},{"number":5}]}'"#,
        );
        let out = env
            .command(&["doctor", "--path", root.to_str().unwrap(), "--json"])
            .env("SELDON_SNAPPER", "snapper-other")
            .output()
            .unwrap();
        let v = json(&out);
        let c = check(&v, "snapper");
        assert_eq!(c["status"], "ok", "{c}");
        assert!(
            c["message"]
                .as_str()
                .unwrap()
                .starts_with("1 snapshots (config root)."),
            "{c}"
        );
        // without the variable: the bare name, which is missing here
        let (_, v) = doctor(&env, &[]);
        assert_eq!(check(&v, "snapper")["status"], "degraded", "{v}");
    }

    /// doctor stays read-only with every new check failing: the state
    /// directory, the config, the logbook and its `.git` are
    /// byte-identical afterwards, and no index or lock appears.
    #[test]
    fn doctor_writes_nothing_while_reporting_problems() {
        let env = Env::new(Snapper::NoPermissions);
        let root = init(&env);
        let out = env.seldon(&["plan", "new", "--json", "--", "Twice"]);
        assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
        let id = json(&out)["case"]["id"].as_str().unwrap().to_string();
        let out = env.seldon(&["status"]);
        assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
        // a case id in two files, an invalid redaction pattern
        let queued = common::find_file(&root.join("work/queued"), &id);
        std::fs::copy(
            &queued,
            root.join("work/completed")
                .join(queued.file_name().unwrap()),
        )
        .unwrap();
        let config = env.config_file();
        let text = std::fs::read_to_string(&config).unwrap();
        std::fs::write(
            &config,
            text.replace("\npatterns = []", "\npatterns = [\"(unclosed\"]"),
        )
        .unwrap();
        let state = env.home.join(".local/state/seldon");
        std::fs::remove_file(state.join("index.json")).unwrap();
        let _ = std::fs::remove_file(env.lock_file());
        for name in ["cursors.json", "manifest.json", "owned.json"] {
            std::fs::write(state.join(name), "{").unwrap();
        }
        std::fs::write(root.join("ledger/2026-10.jsonl"), b"\xc3\n").unwrap();
        let status = std::fs::read_to_string(root.join("STATUS.md")).unwrap();
        std::fs::write(
            root.join("STATUS.md"),
            status.replace("<!-- seldon:end -->", ""),
        )
        .unwrap();
        let before = (snapshot(&env.home), snapshot(&root));
        let (code, v) = doctor(&env, &[]);
        assert_eq!(code, Some(1), "{v}");
        for name in ["config", "cases", "ledger", "fences", "state"] {
            assert_ne!(check(&v, name)["status"], "ok", "{name}: {v}");
        }
        assert!(
            before == (snapshot(&env.home), snapshot(&root)),
            "doctor wrote into the state directory, the config or the logbook"
        );
        assert!(!state.join("index.json").exists());
        assert!(!env.lock_file().exists());
        if env.has_git {
            assert!(root.join(".git").is_dir());
        }
    }
}

/// `doctor --only rules` (WP-101 round 3): what the panel asks on open.
/// The engine and rules rows only; no probe runs (stubs that record their
/// calls stay silent), while the full doctor does start them.
#[test]
fn only_rules_starts_no_program() {
    let env = Env::new(Snapper::Missing);
    let root = init(&env);
    let calls = env.tmp.path().join("probe-calls");
    for program in ["snapper", "omarchy-version"] {
        env.stub(
            program,
            &format!("echo {program} >> '{}'; echo 4.0.4-1", calls.display()),
        );
    }
    let out = env.seldon(&["doctor", "--only", "rules", "--json"]);
    assert_eq!(
        out.status.code(),
        Some(0),
        "{}{}",
        stdout(&out),
        stderr(&out)
    );
    let v = json(&out);
    let names: Vec<&str> = v["checks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, ["engine", "rules"]);
    assert_eq!(check(&v, "rules")["status"], "ok");
    assert_eq!(v["logbook"], root.to_str().unwrap());
    assert!(
        !calls.exists(),
        "a probe ran: {}",
        std::fs::read_to_string(&calls).unwrap_or_default()
    );
    // the full doctor does run them: the stubs are the ones it finds
    let out = env.seldon(&["doctor", "--json"]);
    assert!(out.status.code().is_some());
    assert!(
        std::fs::read_to_string(&calls)
            .unwrap()
            .contains("omarchy-version")
    );
    // an outdated block is the row's state, as in the full doctor
    std::fs::write(root.join("AGENTS.md"), "# AGENTS.md\n\nold rules\n").unwrap();
    let v = json(&env.seldon(&["doctor", "--only", "rules", "--json"]));
    assert_eq!(check(&v, "rules")["status"], "degraded");
    assert_eq!(
        check(&v, "rules")["fix"],
        "seldon rules update (archives your copy)"
    );
    // an unknown check: clap's usage error, exit 1; no logbook: exit 3
    let out = env.seldon(&["doctor", "--only", "probes", "--json"]);
    assert_eq!(out.status.code(), Some(1));
    let none = env.tmp.path().join("nothing");
    let out = env.seldon(&[
        "doctor",
        "--only",
        "rules",
        "--path",
        none.to_str().unwrap(),
        "--json",
    ]);
    assert_eq!(out.status.code(), Some(3));
}

/// The skill asks for a command report from outside the logbook only when
/// the hooks record there (WP-111): doctor names the scope.
#[test]
fn doctor_names_the_hook_scope() {
    let env = Env::new(Snapper::Allowed);
    env.init_logbook();
    let v = json(&env.seldon(&["doctor", "--json"]));
    assert_eq!(
        v["hooks"],
        serde_json::json!({ "scope": "logbook", "installed": "none" })
    );
    let config = env.config_file();
    let text = std::fs::read_to_string(&config).unwrap();
    std::fs::write(&config, format!("{text}\n[hooks]\nscope = \"all\"\n")).unwrap();
    let v = json(&env.seldon(&["doctor", "--json"]));
    assert_eq!(
        v["hooks"],
        serde_json::json!({ "scope": "all", "installed": "none" })
    );
}

/// ADR-0030 §5, acceptance 5: the `hooks` row says where the Claude Code
/// hooks are — user-wide, logbook only, both, none — with the fix.
#[test]
fn doctor_says_where_the_hooks_are() {
    let env = Env::new(Snapper::Allowed);
    let root = env.init_logbook();
    let row = || {
        let v = json(&env.seldon(&["doctor", "--json"]));
        let row = v["checks"]
            .as_array()
            .unwrap()
            .iter()
            .find(|c| c["name"] == "hooks")
            .unwrap()
            .clone();
        (row, v["hooks"]["installed"].clone())
    };
    let user = env.home.join(".claude/settings.json");
    let local = root.join(".claude/settings.json");
    let install = |settings: &Path| {
        let out = env.seldon(&[
            "hook",
            "install",
            "claude-code",
            "--settings",
            settings.to_str().unwrap(),
        ]);
        assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    };

    // none, no harness configured: ok
    let (r, installed) = row();
    assert_eq!(
        (r["status"].as_str(), installed.as_str()),
        (Some("ok"), Some("none"))
    );
    assert!(r["message"].as_str().unwrap().starts_with("none"), "{r}");
    assert!(r.get("fix").is_none(), "{r}");

    // logbook only: degraded, the fix installs user-wide
    install(&local);
    let (r, installed) = row();
    assert_eq!(
        (r["status"].as_str(), installed.as_str()),
        (Some("degraded"), Some("logbook"))
    );
    assert!(
        r["message"]
            .as_str()
            .unwrap()
            .contains("sessions started from ~/Work are not recorded"),
        "{r}"
    );
    assert_eq!(r["fix"], "seldon hook install claude-code");

    // both: ok, with the optional tidy-up
    install(&user);
    let (r, installed) = row();
    assert_eq!(
        (r["status"].as_str(), installed.as_str()),
        (Some("ok"), Some("both"))
    );
    assert_eq!(
        r["fix"],
        format!(
            "optional: seldon hook uninstall claude-code --settings {}",
            local.display()
        )
    );

    // user-wide: ok, no fix
    std::fs::remove_file(&local).unwrap();
    let (r, installed) = row();
    assert_eq!(
        (r["status"].as_str(), installed.as_str()),
        (Some("ok"), Some("user-wide"))
    );
    assert!(
        r["message"]
            .as_str()
            .unwrap()
            .starts_with("user-wide (~/.claude/settings.json)"),
        "{r}"
    );
    assert!(r.get("fix").is_none(), "{r}");

    // incomplete: degraded
    let text = std::fs::read_to_string(&user)
        .unwrap()
        .replace("seldon hook session-stop", "something else");
    std::fs::write(&user, text).unwrap();
    let (r, _) = row();
    assert_eq!(r["status"], "degraded");
    assert!(
        r["message"]
            .as_str()
            .unwrap()
            .contains("holds 2 of Seldon's 3 hooks"),
        "{r}"
    );
    assert_eq!(r["fix"], "seldon hook install claude-code");

    // not JSON: degraded, never touched
    std::fs::write(&user, "{ not json").unwrap();
    let (r, _) = row();
    assert_eq!(r["status"], "degraded");
    assert!(
        r["message"].as_str().unwrap().contains("is not valid JSON"),
        "{r}"
    );
    assert_eq!(std::fs::read_to_string(&user).unwrap(), "{ not json");

    // none, with the Claude Code harness configured: degraded
    std::fs::remove_file(&user).unwrap();
    let config = env.config_file();
    let text = std::fs::read_to_string(&config).unwrap();
    assert!(text.contains("\nharnesses = []\n"), "{text}");
    let text = text.replace("\nharnesses = []\n", "\nharnesses = [\"claude-code\"]\n");
    std::fs::write(&config, text).unwrap();
    let (r, installed) = row();
    assert_eq!(
        (r["status"].as_str(), installed.as_str()),
        (Some("degraded"), Some("none"))
    );
    assert_eq!(r["fix"], "seldon hook install claude-code");
}

/// WP-143: the `workpieces` row names the `work/<case-id>/` folders that
/// no case owns or that a closed case left large: count, size, the
/// oldest by case id (999 before 1000). Information only: always ok, no
/// fix.
#[test]
fn doctor_reports_leftover_workpiece_folders() {
    let env = Env::new(Snapper::Allowed);
    let root = env.init_logbook();
    let row = || {
        let v = json(&env.seldon(&["doctor", "--json"]));
        check(&v, "workpieces").clone()
    };
    assert_eq!(row()["message"], "no workpiece folders");
    let run = |args: &[&str]| {
        let out = env.seldon(args);
        assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    };
    for title in ["big", "small", "open"] {
        run(&["plan", "new", "--no-commit", "--", title]);
    }
    for id in ["C-2026-001", "C-2026-002", "C-2026-003"] {
        run(&["plan", "start", "--no-commit", id]);
    }
    for id in ["C-2026-001", "C-2026-002"] {
        run(&["plan", "verify", "--no-commit", "--no-capture", id]);
        run(&["plan", "done", "--no-commit", "--no-capture", id]);
    }
    let work = root.join("work");
    let folder = |name: &str, bytes: u64| {
        let dir = work.join(name);
        std::fs::create_dir_all(dir.join("sub")).unwrap();
        let f = std::fs::File::create(dir.join("sub/file")).unwrap();
        f.set_len(bytes).unwrap();
    };
    folder("C-2026-002", 1024);
    folder("notes", 20 << 20);
    folder("C-2026-01x", 20 << 20);
    assert_eq!(
        row()["message"],
        "1 workpiece folder(s), none orphaned or oversized"
    );
    // the oldest by id, not by name
    folder("C-2026-1000", 10);
    folder("C-2026-999-old", 5);
    assert_eq!(
        row()["message"],
        "2 of 3 workpiece folder(s) left behind: 2 orphaned (no case), 0 oversized (a closed \
         case, over 10.0 MiB), 15 B in all; the oldest: work/C-2026-999-old/"
    );
    folder("C-2026-001-big", 11 << 20);
    folder("C-2026-003-open", 20 << 20);
    std::os::unix::fs::symlink(work.join("notes"), work.join("C-2024-001")).unwrap();
    let r = row();
    assert_eq!(r["status"], "ok", "{r}");
    assert!(r.get("fix").is_none(), "{r}");
    assert_eq!(
        r["message"],
        "3 of 5 workpiece folder(s) left behind: 2 orphaned (no case), 1 oversized (a closed \
         case, over 10.0 MiB), 11.0 MiB in all; the oldest: work/C-2026-001-big/"
    );
    // a name is shown without its control characters
    folder("C-2025-001-\u{1b}[2J", 5);
    assert_eq!(
        row()["message"],
        "4 of 6 workpiece folder(s) left behind: 3 orphaned (no case), 1 oversized (a closed \
         case, over 10.0 MiB), 11.0 MiB in all; the oldest: work/C-2025-001-?[2J/"
    );
}
