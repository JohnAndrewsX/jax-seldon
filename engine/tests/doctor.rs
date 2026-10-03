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
        assert_eq!(
            snapper["fix"],
            "sudo snapper -c root set-config ALLOW_USERS=$USER SYNC_ACL=yes"
        );
        // what the fix grants besides listing
        assert_eq!(
            snapper["message"],
            format!(
                "No permissions. Snapshots are not recorded until you allow your user once (ADR-0011). {}",
                seldon::commands::doctor::SNAPPER_FIX_GRANTS
            )
        );
        assert!(
            seldon::commands::doctor::SNAPPER_FIX_GRANTS
                .contains("create, change and delete root snapshots without a password")
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
        assert!(text.contains("fix: sudo snapper"));
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
            text.contains("Snapper: degraded — No permissions."),
            "{text}"
        );
        assert!(
            text.contains(&format!(
                "{}   # optional: snapshots in the timeline (ADR-0011)",
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
        let text = v.to_string();
        assert!(!text.contains("not initialised"), "{text}");
        assert!(!text.contains("seldon init"), "{text}");

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
        let out = env.seldon(&["status"]);
        assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
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
        for name in ["ledger", "fences", "state"] {
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
