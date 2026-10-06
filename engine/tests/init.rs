//! `seldon init` in a throw-away home (never the real one).

mod common;

use common::{Env, Snapper, json, stderr, stdout};
use seldon::logbook::Logbook;
use seldon::logbook::layout::{REQUIRED_DIRS, REQUIRED_FILES};
use seldon::model::{self, Area, Language, Memory, Project};

mod init {
    use super::*;

    fn init(env: &Env, extra: &[&str]) -> std::process::Output {
        let path = env.tmp.path().join("logbook");
        let mut args = vec![
            "init",
            "--non-interactive",
            "--no-capture",
            "--path",
            path.to_str().unwrap(),
        ];
        args.extend_from_slice(extra);
        env.seldon(&args)
    }

    /// `init --non-interactive --no-capture --path <root>` with `extra`.
    fn init_at(env: &Env, root: &std::path::Path, extra: &[&str]) -> std::process::Output {
        let mut args = vec![
            "init",
            "--non-interactive",
            "--no-capture",
            "--path",
            root.to_str().unwrap(),
        ];
        args.extend_from_slice(extra);
        env.seldon(&args)
    }

    #[test]
    fn non_interactive_produces_the_spec_layout() {
        let env = Env::new(Snapper::NoPermissions);
        let out = init(&env, &[]);
        assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
        let root = env.tmp.path().join("logbook");

        // SPEC-LOGBOOK §2
        for dir in REQUIRED_DIRS {
            assert!(root.join(dir).is_dir(), "missing dir {dir}");
        }
        for file in REQUIRED_FILES.iter().chain(&["STATUS.md", ".gitignore"]) {
            assert!(root.join(file).is_file(), "missing file {file}");
        }
        for area in seldon::model::area::DEFAULT_AREAS {
            let (a, _) =
                model::load::<Area>(&root.join(format!("areas/{area}/README.md"))).unwrap();
            assert_eq!(a.name, area);
        }
        for f in [
            "hardware",
            "packages",
            "deviations",
            "services",
            "omarchy",
            "plugins",
        ] {
            assert!(
                root.join(format!("system/{f}.md")).is_file(),
                "system/{f}.md"
            );
        }
        model::load::<Memory>(&root.join("memory/lessons.md")).unwrap();
        assert!(root.join(".seldon/templates").is_dir());
        assert!(!root.join(".obsidian").exists());
        assert_eq!(
            std::fs::read_to_string(root.join(".gitignore")).unwrap(),
            ".obsidian/workspace*\n.seldon/active-case\n.*.tmp-*\n"
        );

        // .seldon/logbook.toml and PROJECT.md agree
        let lb = Logbook::open(&root).unwrap();
        assert_eq!(lb.meta.schema_version, 1);
        assert_eq!(lb.meta.language, Language::En);
        let (project, _) = model::load::<Project>(&root.join("PROJECT.md")).unwrap();
        assert_eq!(project.machine_id, lb.meta.machine_id);
        let (host, suffix) = lb.meta.machine_id.rsplit_once('-').unwrap();
        assert!(!host.is_empty() && suffix.len() == 4);

        // generated files carry the header
        let status = std::fs::read_to_string(root.join("STATUS.md")).unwrap();
        assert!(status.starts_with(seldon::GENERATED_HEADER));

        // no absolute paths inside the logbook (SPEC-LOGBOOK §7)
        let home = env.home.to_str().unwrap();
        for file in REQUIRED_FILES {
            let text = std::fs::read_to_string(root.join(file)).unwrap();
            assert!(!text.contains(home), "{file} contains the home path");
        }

        // config.toml points at the logbook
        let config: toml::Table = std::fs::read_to_string(env.config_file())
            .unwrap()
            .parse()
            .unwrap();
        assert_eq!(config["logbook"].as_str(), root.to_str());
        assert_eq!(config["language"].as_str(), Some("en"));
        assert_eq!(config["git"]["autocommit"].as_bool(), Some(true));

        // the read grant is printed, never run (ADR-0026)
        let text = stdout(&out);
        assert!(
            text.contains(
                "  sudo setfacl -m u:$USER:rx /.snapshots   # optional: snapshots in the timeline (ADR-0026)\n"
            ),
            "{text}"
        );
        // with what it grants
        assert!(
            text.contains(seldon::commands::doctor::SNAPPER_FIX_GRANTS),
            "{text}"
        );
    }

    /// ADR-0026: for a user still listed in `ALLOW_USERS`, init's snapper
    /// step is the revert followed by the read grant; for another user
    /// there is no snapper step.
    #[test]
    fn a_listed_user_gets_the_revert_step() {
        let env = Env::new(Snapper::Allowed);
        env.stub(
            "snapper",
            "case \"$*\" in\n\
             '--jsonout list') echo '{\"root\":[{\"number\":1}]}' ;;\n\
             '--jsonout -c root get-config') echo '{\"ALLOW_USERS\": \"alice\"}' ;;\n\
             *) exit 64 ;;\n\
             esac",
        );
        let run = |user: &str, dir: &str| {
            let root = env.tmp.path().join(dir);
            let out = env
                .command(&[
                    "init",
                    "--non-interactive",
                    "--no-capture",
                    "--path",
                    root.to_str().unwrap(),
                ])
                .env("USER", user)
                .output()
                .unwrap();
            assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
            stdout(&out)
        };
        let text = run("alice", "listed");
        assert!(
            text.contains(
                "  sudo snapper -c root set-config ALLOW_USERS=\"\" SYNC_ACL=no && sudo setfacl -m u:$USER:rx /.snapshots   # recommended: a read grant instead of the snapper opt-in (ADR-0026)\n"
            ),
            "{text}"
        );
        assert!(
            text.contains("Snapper: ok — 1 snapshots (config root)."),
            "{text}"
        );
        let text = run("carol", "other");
        assert!(!text.contains("snapper -c root"), "{text}");
        assert!(!text.contains("setfacl"), "{text}");
    }

    #[test]
    fn json_output() {
        let env = Env::new(Snapper::NoPermissions);
        let out = init(&env, &["--json", "--no-git"]);
        assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
        let v = json(&out);
        assert_eq!(
            v["logbook"],
            env.tmp.path().join("logbook").to_str().unwrap()
        );
        assert_eq!(v["language"], "en");
        assert_eq!(v["git"]["repository"], false);
        assert_eq!(v["snapper"]["status"], "degraded");
        assert!(v["files"].as_u64().unwrap() >= 30);
        assert!(
            v["nextSteps"]
                .as_array()
                .unwrap()
                .iter()
                .any(|s| s == "seldon doctor")
        );
    }

    #[test]
    fn git_repository_with_first_commit() {
        let env = Env::new(Snapper::Allowed);
        if !env.has_git {
            eprintln!("skipped: git not installed");
            return;
        }
        let out = init(&env, &["--json"]);
        assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
        assert_eq!(json(&out)["git"]["committed"], true, "{}", stdout(&out));
        let root = env.tmp.path().join("logbook");
        assert_eq!(
            stdout(&env.git(&root, &["log", "--format=%s"])),
            "seldon: init logbook\n"
        );
        assert_eq!(
            stdout(&env.git(&root, &["status", "--porcelain"])),
            "",
            "everything committed"
        );
    }

    #[test]
    fn no_commit_initialises_without_committing() {
        let env = Env::new(Snapper::Allowed);
        if !env.has_git {
            return;
        }
        let out = init(&env, &["--json", "--no-commit"]);
        assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
        let v = json(&out);
        assert_eq!(v["git"]["repository"], true);
        assert_eq!(v["git"]["committed"], false);
    }

    #[test]
    fn german_templates_and_obsidian() {
        let env = Env::new(Snapper::Allowed);
        let out = init(
            &env,
            &[
                "--language",
                "de",
                "--obsidian",
                "--harness",
                "claude-code",
                "--no-git",
            ],
        );
        assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
        let root = env.tmp.path().join("logbook");
        let agents = std::fs::read_to_string(root.join("AGENTS.md")).unwrap();
        assert!(agents.contains("Regeln für jeden Agenten"));
        assert_eq!(Logbook::open(&root).unwrap().meta.language, Language::De);
        let app: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(root.join(".obsidian/app.json")).unwrap(),
        )
        .unwrap();
        assert!(
            app["userIgnoreFilters"]
                .as_array()
                .unwrap()
                .iter()
                .any(|f| f == ".seldon/")
        );
        let daily: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(root.join(".obsidian/daily-notes.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(daily["folder"], "journal");
        let config: toml::Table = std::fs::read_to_string(env.config_file())
            .unwrap()
            .parse()
            .unwrap();
        assert_eq!(
            config["harnesses"].as_array().unwrap()[0].as_str(),
            Some("claude-code")
        );
        // installed by the wizard (WP-024), no longer a next step
        let text = stdout(&out);
        assert!(
            text.contains("Harness claude-code: .claude/settings.json: 3 hook(s) added"),
            "{text}"
        );
        assert!(
            !text.contains("  seldon hook install claude-code"),
            "{text}"
        );
        assert!(root.join(".claude/settings.json").is_file());
    }

    #[test]
    fn language_from_locale() {
        let env = Env::new(Snapper::Allowed);
        let path = env.tmp.path().join("logbook");
        let out = env
            .command(&[
                "init",
                "--non-interactive",
                "--no-capture",
                "--no-git",
                "--path",
                path.to_str().unwrap(),
            ])
            .env("LANG", "de_DE.UTF-8")
            .output()
            .unwrap();
        assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
        assert_eq!(Logbook::open(&path).unwrap().meta.language, Language::De);
    }

    #[test]
    fn a_config_without_language_leaves_it_to_the_locale() {
        // F-542: a config.toml written before init (here: redaction only)
        // has no language; the locale decides, and init stores the result
        let env = Env::new(Snapper::Allowed);
        std::fs::create_dir_all(env.config_file().parent().unwrap()).unwrap();
        std::fs::write(
            env.config_file(),
            "[redaction]\npatterns = [\"mysecret\"]\n",
        )
        .unwrap();
        let init_de = |env: &Env| {
            let path = env.tmp.path().join("logbook");
            let out = env
                .command(&[
                    "init",
                    "--non-interactive",
                    "--no-capture",
                    "--no-git",
                    "--path",
                    path.to_str().unwrap(),
                ])
                .env("LANG", "de_DE.UTF-8")
                .output()
                .unwrap();
            assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
            Logbook::open(&path).unwrap().meta.language
        };
        assert_eq!(init_de(&env), Language::De);
        let config: toml::Table = std::fs::read_to_string(env.config_file())
            .unwrap()
            .parse()
            .unwrap();
        assert_eq!(config["language"].as_str(), Some("de"));
        assert_eq!(
            config["redaction"]["patterns"][0].as_str(),
            Some("mysecret")
        );

        // a language in the config is honoured over the locale
        let env = Env::new(Snapper::Allowed);
        std::fs::create_dir_all(env.config_file().parent().unwrap()).unwrap();
        std::fs::write(env.config_file(), "language = \"en\"\n").unwrap();
        assert_eq!(init_de(&env), Language::En);
    }

    #[test]
    fn a_config_that_cannot_be_saved_leaves_nothing_init_refuses() {
        // F-543: the config is saved before the layout, so a failed save
        // writes nothing into the logbook folder; once the config folder
        // is writable again, the same init runs
        use std::os::unix::fs::PermissionsExt as _;
        let env = Env::new(Snapper::Allowed);
        let dir = env.config_file().parent().unwrap().to_path_buf();
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o555)).unwrap();
        if std::fs::write(dir.join("probe"), "").is_ok() {
            // root ignores the mode: nothing to test here
            std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o755)).unwrap();
            eprintln!("skipped: the read-only config folder is writable (root?)");
            return;
        }
        let root = env.tmp.path().join("logbook");
        let out = init(&env, &["--no-git", "--json"]);
        assert_eq!(out.status.code(), Some(2), "{}", stderr(&out));
        assert!(
            !root.exists() || std::fs::read_dir(&root).unwrap().next().is_none(),
            "nothing written into the logbook folder"
        );
        assert!(!env.config_file().exists());

        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o755)).unwrap();
        let out = init(&env, &["--no-git", "--json"]);
        assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
        assert!(Logbook::is_initialised(&root));
        let config: toml::Table = std::fs::read_to_string(env.config_file())
            .unwrap()
            .parse()
            .unwrap();
        assert_eq!(config["logbook"].as_str(), root.to_str());
    }

    /// A config.toml written by hand: comments and an old logbook, which a
    /// save would drop or change.
    const OWN_CONFIG: &str = "# mine\nlogbook = \"~/Old\"\n[redaction]\npatterns = [\"x\"]\n";

    #[test]
    fn a_layout_that_cannot_start_leaves_the_config_byte_identical() {
        // B1: the parent folder is read-only, so the layout fails at its
        // first folder; the config saved before it is put back byte for
        // byte (or removed when there was none), and after the fix the
        // same init runs
        use std::os::unix::fs::PermissionsExt as _;
        let mode = |dir: &std::path::Path, m: u32| {
            std::fs::set_permissions(dir, std::fs::Permissions::from_mode(m)).unwrap()
        };
        for own in [Some(OWN_CONFIG), None] {
            let env = Env::new(Snapper::Allowed);
            if let Some(text) = own {
                std::fs::create_dir_all(env.config_file().parent().unwrap()).unwrap();
                std::fs::write(env.config_file(), text).unwrap();
            }
            let parent = env.tmp.path().join("read-only");
            std::fs::create_dir_all(&parent).unwrap();
            mode(&parent, 0o555);
            if std::fs::write(parent.join("probe"), "").is_ok() {
                mode(&parent, 0o755);
                eprintln!("skipped: the read-only folder is writable (root?)");
                return;
            }
            let root = parent.join("logbook");
            let args = [
                "init",
                "--non-interactive",
                "--no-capture",
                "--no-git",
                "--path",
                root.to_str().unwrap(),
            ];
            let out = env.seldon(&args);
            assert_eq!(out.status.code(), Some(2), "{}", stderr(&out));
            match own {
                Some(text) => assert_eq!(common::read(&env.config_file()), text, "config restored"),
                None => assert!(!env.config_file().exists(), "config removed again"),
            }
            assert!(!root.exists());
            assert!(
                stderr(&out).contains("nothing was kept"),
                "{}",
                stderr(&out)
            );

            mode(&parent, 0o755);
            let out = env.seldon(&args);
            assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
            assert!(Logbook::is_initialised(&root));
        }
    }

    #[test]
    fn a_layout_stopped_half_way_is_undone() {
        // B1: a logbook path so long that the folders and the first files
        // fit, but `areas/hyprland/README.md` exceeds PATH_MAX (4095 bytes):
        // the layout stops half-way. init removes the highest folder it
        // created and restores the config; a second init then succeeds
        let env = Env::new(Snapper::Allowed);
        std::fs::create_dir_all(env.config_file().parent().unwrap()).unwrap();
        std::fs::write(env.config_file(), OWN_CONFIG).unwrap();
        let top = env.tmp.path().join("long");
        let mut root = top.clone();
        while root.as_os_str().len() < 4075 - 201 {
            root.push("d".repeat(200));
        }
        let rest = 4075 - root.as_os_str().len() - 1;
        root.push("l".repeat(rest));
        assert_eq!(root.as_os_str().len(), 4075);
        let out = init_at(&env, &root, &["--no-git"]);
        assert_eq!(out.status.code(), Some(2), "{}", stderr(&out));
        assert!(
            stderr(&out).contains("README.md"),
            "the layout stopped at a template: {}",
            stderr(&out)
        );
        assert_eq!(
            common::read(&env.config_file()),
            OWN_CONFIG,
            "config restored"
        );
        assert!(!top.exists(), "what init created is gone");

        // an empty folder that was there stays, emptied again
        std::fs::create_dir_all(&root).unwrap();
        let out = init_at(&env, &root, &["--no-git"]);
        assert_eq!(out.status.code(), Some(2), "{}", stderr(&out));
        assert_eq!(common::read(&env.config_file()), OWN_CONFIG);
        assert_eq!(std::fs::read_dir(&root).unwrap().count(), 0, "emptied");
        std::fs::remove_dir_all(&top).unwrap();

        let out = init(&env, &["--no-git"]);
        assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
        let config: toml::Table = common::read(&env.config_file()).parse().unwrap();
        assert_eq!(
            config["logbook"].as_str(),
            env.tmp.path().join("logbook").to_str()
        );
    }

    #[test]
    fn a_rerun_keeps_a_hand_set_autocommit_false() {
        // N1: flags, then the existing config, then the default: without
        // `--git`, `autocommit = false` stays (and no repository is made);
        // `--git` turns it on
        let env = Env::new(Snapper::Allowed);
        std::fs::create_dir_all(env.config_file().parent().unwrap()).unwrap();
        std::fs::write(env.config_file(), "[git]\nautocommit = false\n").unwrap();
        let out = init(&env, &["--json"]);
        assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
        assert_eq!(json(&out)["git"]["repository"], false);
        let config: toml::Table = common::read(&env.config_file()).parse().unwrap();
        assert_eq!(config["git"]["autocommit"].as_bool(), Some(false));
        assert!(!env.tmp.path().join("logbook/.git").exists());

        let other = env.tmp.path().join("other");
        let out = init_at(&env, &other, &["--git"]);
        assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
        let config: toml::Table = common::read(&env.config_file()).parse().unwrap();
        assert_eq!(config["git"]["autocommit"].as_bool(), Some(true));
    }

    #[test]
    fn a_layout_that_stops_half_way_is_no_logbook() {
        // F-543: the marker `.seldon/logbook.toml` is written last, so a
        // layout that fails on the way (here: a folder where a template
        // goes) leaves nothing that `init` or `doctor` takes for a logbook
        use seldon::logbook::layout::{self, NewLogbook};
        let env = Env::new(Snapper::Allowed);
        let root = env.tmp.path().join("logbook");
        std::fs::create_dir_all(root.join("DECISIONS.md")).unwrap();
        let spec = NewLogbook {
            language: Language::En,
            machine_id: "machine-0000".into(),
            created: "2026-10-04T10:00:00+02:00".parse().unwrap(),
            today: chrono::NaiveDate::from_ymd_opt(2026, 10, 4).unwrap(),
            obsidian: false,
        };
        assert!(layout::create(&root, &spec).is_err());
        assert!(!Logbook::is_initialised(&root));
        assert!(root.join("AGENTS.md").is_file() || root.join(".gitignore").is_file());
    }

    #[test]
    fn no_git_is_stored_and_doctor_reads_it_as_chosen() {
        // F-545: `--no-git` writes `autocommit = false`; doctor's git check
        // is then ok, without a `git init` fix
        let env = Env::new(Snapper::Allowed);
        let root = env.tmp.path().join("logbook");
        let out = init(&env, &["--no-git"]);
        assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
        let config: toml::Table = std::fs::read_to_string(env.config_file())
            .unwrap()
            .parse()
            .unwrap();
        assert_eq!(config["git"]["autocommit"].as_bool(), Some(false));
        let out = env.seldon(&["doctor", "--path", root.to_str().unwrap(), "--json"]);
        let v = json(&out);
        let git = v["checks"]
            .as_array()
            .unwrap()
            .iter()
            .find(|c| c["name"] == "git")
            .unwrap();
        assert_eq!(git["status"], "ok", "{git}");
        assert!(git.get("fix").is_none(), "{git}");
    }

    #[test]
    fn default_path_is_home_seldon_and_env_overrides_it() {
        let env = Env::new(Snapper::Allowed);
        let out = env.seldon(&["init", "--non-interactive", "--no-capture", "--no-git"]);
        assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
        assert!(Logbook::is_initialised(&env.home.join("Seldon")));

        let env = Env::new(Snapper::Allowed);
        let target = env.tmp.path().join("from-env");
        let out = env
            .command(&["init", "--non-interactive", "--no-capture", "--no-git"])
            .env("SELDON_LOGBOOK", &target)
            .output()
            .unwrap();
        assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
        assert!(Logbook::is_initialised(&target));
        assert!(!env.home.join("Seldon").exists());
    }

    #[test]
    fn existing_logbook_is_a_user_error_and_untouched() {
        let env = Env::new(Snapper::Allowed);
        assert_eq!(init(&env, &["--no-git"]).status.code(), Some(0));
        let agents = env.tmp.path().join("logbook/AGENTS.md");
        std::fs::write(&agents, "mine\n").unwrap();
        let out = init(&env, &["--no-git", "--json"]);
        assert_eq!(out.status.code(), Some(1));
        assert!(
            json(&out)["error"]["message"]
                .as_str()
                .unwrap()
                .contains("already a logbook")
        );
        assert_eq!(std::fs::read_to_string(&agents).unwrap(), "mine\n");
    }

    #[test]
    fn non_empty_directory_is_a_user_error() {
        let env = Env::new(Snapper::Allowed);
        let root = env.tmp.path().join("logbook");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("notes.txt"), "x").unwrap();
        let out = init(&env, &["--no-git"]);
        assert_eq!(out.status.code(), Some(1));
        assert!(stderr(&out).contains("not empty"), "{}", stderr(&out));
        assert_eq!(std::fs::read_dir(&root).unwrap().count(), 1);
    }

    #[test]
    fn empty_directory_is_fine() {
        let env = Env::new(Snapper::Allowed);
        std::fs::create_dir_all(env.tmp.path().join("logbook")).unwrap();
        assert_eq!(init(&env, &["--no-git"]).status.code(), Some(0));
    }

    #[test]
    fn interactive_without_a_terminal_is_a_user_error() {
        let env = Env::new(Snapper::Allowed);
        let out = env.seldon(&["init", "--json"]);
        assert_eq!(out.status.code(), Some(1));
        assert!(
            json(&out)["error"]["message"]
                .as_str()
                .unwrap()
                .contains("--non-interactive")
        );
    }

    #[test]
    fn held_lock_exits_4() {
        let env = Env::new(Snapper::Allowed);
        let lock = env.lock_file();
        std::fs::create_dir_all(lock.parent().unwrap()).unwrap();
        let file = std::fs::File::create(&lock).unwrap();
        file.lock().unwrap();
        let out = init(&env, &["--no-git", "--json"]);
        assert_eq!(out.status.code(), Some(4));
        assert_eq!(json(&out)["error"]["code"], 4);
        assert!(!env.tmp.path().join("logbook").exists());
        file.unlock().unwrap();
        assert_eq!(init(&env, &["--no-git"]).status.code(), Some(0));
    }

    #[test]
    fn existing_config_keeps_settings_and_unknown_keys() {
        let env = Env::new(Snapper::Allowed);
        std::fs::create_dir_all(env.config_file().parent().unwrap()).unwrap();
        std::fs::write(
            env.config_file(),
            "language = \"de\"\nwatchPaths = [\"~/.config/hypr\"]\nfuture = true\n[collectors]\nsnapper = false\n",
        )
        .unwrap();
        let out = init(&env, &["--no-git", "--json"]);
        assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
        let v = json(&out);
        assert_eq!(v["language"], "de");
        assert_eq!(v["collectors"]["snapper"], false);
        assert_eq!(
            v["snapper"]["status"], "ok",
            "disabled collector is not probed"
        );
        let config: toml::Table = std::fs::read_to_string(env.config_file())
            .unwrap()
            .parse()
            .unwrap();
        assert_eq!(config["future"].as_bool(), Some(true));
        assert_eq!(config["watchPaths"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn malformed_config_is_a_user_error() {
        let env = Env::new(Snapper::Allowed);
        std::fs::create_dir_all(env.config_file().parent().unwrap()).unwrap();
        std::fs::write(env.config_file(), "language = \n").unwrap();
        let out = init(&env, &["--no-git"]);
        assert_eq!(out.status.code(), Some(1));
        assert!(stderr(&out).contains("config.toml"), "{}", stderr(&out));
    }

    #[test]
    fn bad_flag_values_are_user_errors() {
        let env = Env::new(Snapper::Allowed);
        for args in [
            &["init", "--language", "fr", "--json"][..],
            &["init", "--harness", "vim", "--json"][..],
        ] {
            let out = env.seldon(args);
            assert_eq!(out.status.code(), Some(1), "{args:?}");
            let message = json(&out)["error"]["message"].as_str().unwrap().to_string();
            assert!(message.contains(args[2]), "{message}");
        }
    }
}

/// WP-024: the wizard's steps after the layout — first capture, backfill
/// and baseline, harnesses, theme hook — and the templates per language.
/// Every host source is stubbed; nothing reads the real logbook or XDG dirs.
mod setup {
    use std::os::unix::fs::PermissionsExt as _;
    use std::path::{Path, PathBuf};
    use std::process::Output;

    use super::*;
    use seldon::logbook::templates::{self, TEMPLATES};

    fn fixture(rel: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../fixtures")
            .join(rel)
    }

    /// `seldon init --non-interactive --path <tmp>/logbook extra…` with the
    /// collectors' sources stubbed: `pacman_log` as the package log, no
    /// plugins, no theme; `omarchy` from PATH (a test may stub it) or none.
    fn init_with(env: &Env, pacman_log: &Path, extra: &[&str], vars: &[(&str, &Path)]) -> Output {
        let path = env.tmp.path().join("logbook");
        let mut args = vec![
            "init",
            "--non-interactive",
            "--path",
            path.to_str().unwrap(),
        ];
        args.extend_from_slice(extra);
        let tmp = env.tmp.path();
        let mut cmd = env.command(&args);
        cmd.env("SELDON_PACMAN_LOG", pacman_log)
            .env("SELDON_PACMAN_DB_LOCK", tmp.join("no-db.lck"))
            .env("SELDON_OMARCHY_PLUGINS_DIR", tmp.join("plugins"))
            .env("SELDON_THEME_FILE", tmp.join("theme.name"))
            .env("SELDON_HARDWARE_ROOT", common::hardware_root());
        for (k, v) in vars {
            cmd.env(k, v);
        }
        cmd.output().unwrap()
    }

    fn empty_log(env: &Env) -> PathBuf {
        let log = env.tmp.path().join("pacman.log");
        std::fs::write(&log, "").unwrap();
        log
    }

    fn no_omarchy(env: &Env) -> PathBuf {
        env.tmp.path().join("no-omarchy")
    }

    // -- first capture -----------------------------------------------------

    #[test]
    fn init_runs_the_first_capture() {
        let env = Env::new(Snapper::NoPermissions);
        let log = empty_log(&env);
        let out = init_with(
            &env,
            &log,
            &["--json", "--no-git"],
            &[("SELDON_OMARCHY", &no_omarchy(&env))],
        );
        assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
        let v = json(&out);
        let capture = &v["capture"];
        assert_eq!(capture["ran"], true, "{capture}");
        assert_eq!(capture["since"], serde_json::Value::Null);
        assert_eq!(capture["written"], 0);
        assert_eq!(capture["openDrift"], 0);
        assert_eq!(capture["baseline"], serde_json::Value::Null);
        let names: Vec<&str> = capture["collectors"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|c| c["ran"] == true)
            .map(|c| c["name"].as_str().unwrap())
            .collect();
        assert_eq!(
            names,
            ["snapper", "pacman", "omarchy", "plugins", "theme", "config"]
        );
        // WP-013 FINDINGS §3: the stale text is gone, and so is the step
        assert!(
            !v.to_string()
                .contains("no collectors in this engine version")
        );
        let next = v["nextSteps"].as_array().unwrap();
        assert!(
            !next.iter().any(|s| s == "seldon capture --all"),
            "{next:?}"
        );
        // the dossier ran once after the capture (WP-035): the queries
        // that are not stubbed here are skipped with a warning, the
        // hardware comes from the fixture files
        let dossier = &v["dossier"];
        assert_eq!(dossier["ran"], true, "{dossier}");
        assert_eq!(dossier["sections"]["hardware.summary"], "written");
        assert_eq!(dossier["sections"]["packages.summary"], "skipped");
        assert!(!next.iter().any(|s| s == "seldon dossier"), "{next:?}");
        let hardware = common::read(&env.tmp.path().join("logbook/system/hardware.md"));
        assert!(
            hardware.contains("- cpu: Intel(R) Core(TM) i7-14700K\n"),
            "{hardware}"
        );
        // the capture set every collector's cursor and built the index
        assert!(env.home.join(".local/state/seldon/cursors.json").is_file());
        assert!(env.home.join(".local/state/seldon/index.json").is_file());
        // a second capture finds nothing new
        let again = env
            .command(&["--json", "capture", "--all"])
            .env("SELDON_PACMAN_LOG", &log)
            .env("SELDON_PACMAN_DB_LOCK", env.tmp.path().join("no-db.lck"))
            .env("SELDON_OMARCHY", no_omarchy(&env))
            .env("SELDON_OMARCHY_PLUGINS_DIR", env.tmp.path().join("plugins"))
            .env("SELDON_THEME_FILE", env.tmp.path().join("theme.name"))
            .output()
            .unwrap();
        assert_eq!(again.status.code(), Some(0), "{}", stderr(&again));
        assert_eq!(json(&again)["written"], 0);

        let human = init_with(
            &Env::new(Snapper::NoPermissions),
            &log,
            &["--no-git"],
            &[("SELDON_OMARCHY", &no_omarchy(&env))],
        );
        let text = stdout(&human);
        assert!(text.contains("First capture: 0 event(s)"), "{text}");
        assert!(text.contains("\nDossier: Wrote system/"), "{text}");
        assert!(!text.contains("First capture: skipped"), "{text}");

        // with git: the index is rebuilt after the capture's commit, so it
        // names the new head and a clean tree, not the state before it
        let env = Env::new(Snapper::NoPermissions);
        if !env.has_git {
            return;
        }
        // a backfill, so the capture changes the logbook and commits
        let out = init_with(
            &env,
            &fixture("logs/pacman.log"),
            &["--json", "--since", "2026-08-01"],
            &[("SELDON_OMARCHY", &no_omarchy(&env))],
        );
        assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
        let root = env.tmp.path().join("logbook");
        assert_eq!(
            stdout(&env.git(&root, &["log", "--format=%s"])),
            "seldon: dossier\nseldon: first capture\nseldon: init logbook\n"
        );
        let head = stdout(&env.git(&root, &["rev-parse", "HEAD"]));
        let index: serde_json::Value = serde_json::from_str(&common::read(
            &env.home.join(".local/state/seldon/index.json"),
        ))
        .unwrap();
        let git = &index["logbook"]["git"];
        let short = git["head"].as_str().unwrap();
        assert!(
            !short.is_empty() && head.starts_with(short),
            "{git} vs {head}"
        );
        assert_eq!(git["dirty"], false, "{git}");
        assert_eq!(stdout(&env.git(&root, &["status", "--porcelain"])), "");
    }

    #[test]
    fn no_capture_skips_it_and_keeps_the_next_step() {
        let env = Env::new(Snapper::Allowed);
        let out = init_with(
            &env,
            &empty_log(&env),
            &["--json", "--no-git", "--no-capture"],
            &[],
        );
        assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
        let v = json(&out);
        assert_eq!(v["capture"]["ran"], false);
        assert_eq!(v["capture"]["reason"], "--no-capture");
        assert_eq!(v["dossier"]["ran"], false, "no dossier without the capture");
        for step in ["seldon capture --all", "seldon dossier"] {
            assert!(
                v["nextSteps"].as_array().unwrap().iter().any(|s| s == step),
                "{step}"
            );
        }
        assert!(!env.home.join(".local/state/seldon/cursors.json").exists());
    }

    #[test]
    fn backfill_opens_drift_and_the_baseline_dismisses_it() {
        let log = fixture("logs/pacman.log");
        let omarchy = |env: &Env| no_omarchy(env);

        // without the baseline: the backfill is open drift (FINDINGS §2.2)
        let env = Env::new(Snapper::NoPermissions);
        let out = init_with(
            &env,
            &log,
            &["--json", "--no-git", "--since", "2026-08-01"],
            &[("SELDON_OMARCHY", &omarchy(&env))],
        );
        assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
        let v = json(&out);
        let written = v["capture"]["written"].as_u64().unwrap();
        let open = v["capture"]["openDrift"].as_u64().unwrap();
        assert!(written > 0 && open > 0, "{}", v["capture"]);
        assert_eq!(v["capture"]["baseline"], serde_json::Value::Null);
        assert!(
            v["nextSteps"]
                .as_array()
                .unwrap()
                .iter()
                .any(|s| s.as_str().unwrap().starts_with("seldon drift")),
            "{}",
            v["nextSteps"]
        );

        // with it: zero open drift afterwards
        let env = Env::new(Snapper::NoPermissions);
        let out = init_with(
            &env,
            &log,
            &["--json", "--since", "2026-08-01", "--baseline"],
            &[("SELDON_OMARCHY", &omarchy(&env))],
        );
        assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
        let v = json(&out);
        let baseline = &v["capture"]["baseline"];
        assert_eq!(baseline["reason"], "pre-Seldon baseline");
        assert_eq!(baseline["items"].as_u64(), Some(open), "{baseline}");
        assert_eq!(v["capture"]["openDrift"], 0);
        assert_eq!(v["capture"]["crisis"], 0);
        assert!(
            !v["nextSteps"]
                .as_array()
                .unwrap()
                .iter()
                .any(|s| s.as_str().unwrap().starts_with("seldon drift"))
        );

        let drift = env.seldon(&["--json", "drift"]);
        assert_eq!(drift.status.code(), Some(0), "{}", stderr(&drift));
        let d = json(&drift);
        assert_eq!(
            (d["openDrift"].as_u64(), d["crisis"].as_u64()),
            (Some(0), Some(0))
        );

        // one dismissed resolution per open member, each referring to a
        // different drift event; the original lines stay
        let root = env.tmp.path().join("logbook");
        let ledger = common::ledger(&root);
        let resolutions: Vec<&serde_json::Value> = ledger
            .iter()
            .filter(|e| e["kind"] == "resolution")
            .collect();
        assert_eq!(
            resolutions.len() as u64,
            baseline["events"].as_u64().unwrap()
        );
        assert!(resolutions.len() as u64 >= open);
        let mut refers: Vec<&str> = resolutions
            .iter()
            .map(|r| r["refersTo"].as_str().unwrap())
            .collect();
        refers.sort();
        refers.dedup();
        assert_eq!(refers.len(), resolutions.len());
        for r in &resolutions {
            assert_eq!(r["resolution"], "dismissed");
            assert_eq!(r["detail"], "pre-Seldon baseline");
            assert_eq!(r["actor"], "human");
            assert_eq!(r["source"], "seldon");
        }
        assert!(
            ledger.iter().any(|e| e["source"] == "pacman"),
            "the backfill stays in the ledger"
        );

        // committed after the first commit; nothing left over
        if env.has_git {
            assert_eq!(
                stdout(&env.git(&root, &["log", "--format=%s"])),
                "seldon: dossier\nseldon: first capture and pre-Seldon baseline\nseldon: init logbook\n"
            );
            assert_eq!(stdout(&env.git(&root, &["status", "--porcelain"])), "");
        }
    }

    #[test]
    fn since_baseline_and_no_capture_are_checked_first() {
        let env = Env::new(Snapper::Allowed);
        let log = empty_log(&env);
        for (extra, needle) in [
            (&["--since", "last week"][..], "last week"),
            (&["--baseline"][..], "--since"),
            (
                &["--no-capture", "--since", "2026-09-01"][..],
                "--no-capture",
            ),
        ] {
            let mut args = vec!["--json", "--no-git"];
            args.extend_from_slice(extra);
            let out = init_with(&env, &log, &args, &[]);
            assert_eq!(out.status.code(), Some(1), "{extra:?}");
            let message = json(&out)["error"]["message"].as_str().unwrap().to_string();
            assert!(message.contains(needle), "{extra:?}: {message}");
            assert!(!env.tmp.path().join("logbook").exists(), "{extra:?}");
        }
    }

    /// WP-024 review: the incident's shape — `HOME` lost on the way, the
    /// XDG dirs still redirected — is refused before anything is written.
    #[test]
    fn the_test_guard_refuses_a_home_outside_it() {
        let env = Env::new(Snapper::Allowed);
        let outside = common::TempDir::new("outside-home");
        let path = env.tmp.path().join("logbook");
        let out = env
            .command(&[
                "--json",
                "init",
                "--non-interactive",
                "--path",
                path.to_str().unwrap(),
            ])
            .env("HOME", outside.path())
            .env("XDG_CONFIG_HOME", env.home.join(".config"))
            .env("XDG_STATE_HOME", env.home.join(".local/state"))
            .output()
            .unwrap();
        assert_eq!(out.status.code(), Some(2), "{}", stderr(&out));
        let v = json(&out);
        assert_eq!(v["error"]["code"], 2);
        let message = v["error"]["message"].as_str().unwrap();
        assert!(
            message.contains("refusing to run outside the test guard")
                && message.contains("home directory"),
            "{message}"
        );
        assert!(!path.exists());
        assert!(!env.home.join(".config/seldon").exists());
        assert_eq!(std::fs::read_dir(outside.path()).unwrap().count(), 0);
    }

    // -- harnesses ---------------------------------------------------------

    #[test]
    fn claude_code_hooks_are_installed_idempotently() {
        let env = Env::new(Snapper::Allowed);
        let out = init_with(
            &env,
            &empty_log(&env),
            &["--json", "--harness", "claude-code"],
            &[("SELDON_OMARCHY", &no_omarchy(&env))],
        );
        assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
        let v = json(&out);
        let setup = &v["harnessSetup"]["claude-code"];
        assert_eq!(setup["added"].as_array().unwrap().len(), 3, "{setup}");
        let root = env.tmp.path().join("logbook");
        let settings = root.join(".claude/settings.json");
        let text = std::fs::read_to_string(&settings).unwrap();
        let s: serde_json::Value = serde_json::from_str(&text).unwrap();
        for (event, command) in [
            ("PreToolUse", "seldon hook claude-code"),
            ("SessionStart", "seldon hook session-start"),
            ("SessionEnd", "seldon hook session-stop"),
        ] {
            assert_eq!(s["hooks"][event][0]["hooks"][0]["command"], command);
        }
        assert!(
            !v["nextSteps"]
                .as_array()
                .unwrap()
                .iter()
                .any(|s| s == "seldon hook install claude-code")
        );

        // WP-009's installer finds everything in place and changes nothing
        let again = env.seldon(&["--json", "hook", "install", "claude-code"]);
        assert_eq!(again.status.code(), Some(0), "{}", stderr(&again));
        let a = json(&again);
        assert_eq!(a["added"].as_array().unwrap().len(), 0);
        assert_eq!(a["present"].as_array().unwrap().len(), 3);
        assert_eq!(std::fs::read_to_string(&settings).unwrap(), text);

        // the settings are part of the first commit
        if env.has_git {
            assert_eq!(
                stdout(&env.git(
                    &root,
                    &["log", "--format=%s", "--", ".claude/settings.json"]
                )),
                "seldon: init logbook\n"
            );
        }
    }

    /// A kit as the Omarchy-Agent template dir holds it: the layout of
    /// `.claude/`, with a settings file that has the guard hook.
    fn kit(env: &Env) -> PathBuf {
        let kit = env.tmp.path().join("kit");
        std::fs::create_dir_all(kit.join("hooks")).unwrap();
        std::fs::create_dir_all(kit.join("skills/zones")).unwrap();
        std::fs::write(kit.join("hooks/guard.py"), "# guard\n").unwrap();
        std::fs::set_permissions(
            kit.join("hooks/guard.py"),
            std::fs::Permissions::from_mode(0o755),
        )
        .unwrap();
        std::fs::write(kit.join("skills/zones/SKILL.md"), "# zones\n").unwrap();
        std::fs::write(
            kit.join("settings.json"),
            r#"{"hooks":{"PreToolUse":[{"matcher":"Bash","hooks":[{"type":"command","command":"python3 .claude/hooks/guard.py"}]}]}}"#,
        )
        .unwrap();
        kit
    }

    #[test]
    fn omarchy_agent_kit_is_copied_and_claude_code_merged_into_it() {
        let env = Env::new(Snapper::Allowed);
        let kit = kit(&env);
        let out = init_with(
            &env,
            &empty_log(&env),
            &[
                "--json",
                "--no-git",
                "--harness",
                "claude-code",
                "--harness",
                "omarchy-agent",
            ],
            &[
                ("SELDON_OMARCHY", &no_omarchy(&env)),
                ("SELDON_OMARCHY_AGENT_KIT", &kit),
            ],
        );
        assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
        let v = json(&out);
        let setup = &v["harnessSetup"]["omarchy-agent"];
        assert_eq!(setup["present"], true);
        assert_eq!(
            setup["copied"],
            serde_json::json!([
                ".claude/hooks/guard.py",
                ".claude/settings.json",
                ".claude/skills/zones/SKILL.md"
            ])
        );
        let root = env.tmp.path().join("logbook");
        let guard = root.join(".claude/hooks/guard.py");
        assert_eq!(std::fs::read_to_string(&guard).unwrap(), "# guard\n");
        assert_eq!(
            std::fs::metadata(&guard).unwrap().permissions().mode() & 0o111,
            0o111
        );
        assert!(root.join(".claude/skills/zones/SKILL.md").is_file());
        // the kit's guard and Seldon's hooks side by side
        let s: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(root.join(".claude/settings.json")).unwrap(),
        )
        .unwrap();
        let pre: Vec<&str> = s["hooks"]["PreToolUse"]
            .as_array()
            .unwrap()
            .iter()
            .map(|g| g["hooks"][0]["command"].as_str().unwrap())
            .collect();
        assert_eq!(
            pre,
            ["python3 .claude/hooks/guard.py", "seldon hook claude-code"]
        );
        let config: toml::Table = std::fs::read_to_string(env.config_file())
            .unwrap()
            .parse()
            .unwrap();
        assert_eq!(config["harnesses"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn omarchy_agent_without_a_kit_says_what_it_would_do() {
        let env = Env::new(Snapper::Allowed);
        let out = init_with(
            &env,
            &empty_log(&env),
            &["--no-git", "--harness", "omarchy-agent"],
            &[("SELDON_OMARCHY", &no_omarchy(&env))],
        );
        assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
        let text = stdout(&out);
        assert!(
            text.contains("no kit at ~/.local/share/seldon/harness/omarchy-agent; nothing copied"),
            "{text}"
        );
        assert!(text.contains("would be copied into .claude/"), "{text}");
        assert!(!env.tmp.path().join("logbook/.claude").exists());
    }

    // -- theme hook --------------------------------------------------------

    /// `omarchy` on PATH that records its argv, one call per line.
    fn recording_omarchy(env: &Env, exit: u8) -> PathBuf {
        let argv = env.home.join("omarchy.argv");
        env.stub(
            "omarchy",
            &format!(
                "printf '%s\\n' \"$*\" >> \"$HOME/omarchy.argv\"\n[ \"$1\" = hook ] && [ {exit} -ne 0 ] && {{ echo 'Hook file not found' >&2; exit {exit}; }}\nexit 0"
            ),
        );
        argv
    }

    fn hook_calls(argv: &Path) -> Vec<String> {
        std::fs::read_to_string(argv)
            .unwrap_or_default()
            .lines()
            .filter(|l| l.starts_with("hook"))
            .map(String::from)
            .collect()
    }

    #[test]
    fn theme_hook_is_installed_once_on_opt_in() {
        let env = Env::new(Snapper::Allowed);
        let argv = recording_omarchy(&env, 0);
        let out = init_with(
            &env,
            &empty_log(&env),
            &["--json", "--no-git", "--theme-hook"],
            &[],
        );
        assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
        let script = env
            .home
            .join(".local/state/seldon/hooks/seldon-theme-set.sh");
        assert_eq!(
            hook_calls(&argv),
            [format!("hook install theme-set {}", script.display())]
        );
        assert_eq!(
            std::fs::read_to_string(&script).unwrap(),
            common::read(&Path::new(env!("CARGO_MANIFEST_DIR")).join("hooks/theme-set.sh"))
        );
        assert_eq!(
            std::fs::metadata(&script).unwrap().permissions().mode() & 0o777,
            0o755
        );
        let v = json(&out);
        assert_eq!(v["themeHook"]["installed"], true, "{}", v["themeHook"]);
    }

    #[test]
    fn theme_hook_is_never_installed_without_opt_in() {
        let env = Env::new(Snapper::Allowed);
        let argv = recording_omarchy(&env, 0);
        let out = init_with(&env, &empty_log(&env), &["--json", "--no-git"], &[]);
        assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
        assert!(hook_calls(&argv).is_empty());
        let v = json(&out);
        assert_eq!(v["themeHook"]["requested"], false);
        assert!(!env.home.join(".local/state/seldon/hooks").exists());
    }

    #[test]
    fn theme_hook_failure_is_reported_with_the_fix() {
        let env = Env::new(Snapper::Allowed);
        let argv = recording_omarchy(&env, 1);
        let out = init_with(
            &env,
            &empty_log(&env),
            &["--json", "--no-git", "--theme-hook"],
            &[],
        );
        assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
        assert_eq!(hook_calls(&argv).len(), 1);
        let v = json(&out);
        assert_eq!(v["themeHook"]["installed"], false);
        assert!(
            v["themeHook"]["error"]
                .as_str()
                .unwrap()
                .contains("Hook file not found")
        );
        assert!(v["nextSteps"].as_array().unwrap().iter().any(|s| {
            s.as_str()
                .unwrap()
                .starts_with("omarchy hook install theme-set ")
        }));
    }

    #[test]
    fn theme_hook_already_there_is_not_installed_again() {
        let env = Env::new(Snapper::Allowed);
        let argv = recording_omarchy(&env, 0);
        let dir = env.home.join(".config/omarchy/hooks/theme-set.d");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("seldon-theme-set.sh"), "# installed\n").unwrap();
        let out = init_with(
            &env,
            &empty_log(&env),
            &["--json", "--no-git", "--theme-hook"],
            &[],
        );
        assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
        assert!(hook_calls(&argv).is_empty());
        assert_eq!(json(&out)["themeHook"]["already"], true);
    }

    // -- templates ---------------------------------------------------------

    /// What must be the same in every language: frontmatter keys, headings,
    /// fence names and table header rows of every file `init` writes from a
    /// template.
    fn skeleton(root: &Path, machine_id: &str) -> String {
        let mut out = String::new();
        for t in TEMPLATES {
            let text = common::read(&root.join(t.path)).replace(machine_id, "{{machineId}}");
            out.push_str(&format!("== {}\n", t.path));
            let mut lines = text.lines().peekable();
            if text.starts_with("---\n") {
                lines.next();
                for l in lines.by_ref().take_while(|l| *l != "---") {
                    let key = l.split(':').next().unwrap();
                    out.push_str(&format!("key {key}\n"));
                }
            }
            let mut fenced = false;
            while let Some(l) = lines.next() {
                if l.starts_with("```") {
                    fenced = !fenced;
                    continue;
                }
                if fenced {
                    continue;
                }
                if l.starts_with('#') {
                    out.push_str(&format!("{l}\n"));
                } else if let Some(name) = l.strip_prefix("<!-- seldon:begin ") {
                    out.push_str(&format!("fence {}\n", name.trim_end_matches(" -->")));
                } else if l.starts_with('|') && lines.peek().is_some_and(|n| n.starts_with("|---"))
                {
                    out.push_str(&format!("table {l}\n"));
                }
            }
        }
        out
    }

    fn logbook_in(language: &str) -> (Env, PathBuf, String) {
        let env = Env::new(Snapper::Allowed);
        let root = env.init_logbook_at("logbook", language);
        let machine = Logbook::open(&root).unwrap().meta.machine_id;
        (env, root, machine)
    }

    #[test]
    fn templates_have_english_keys_and_headings_in_every_language() {
        let (_en_env, en, en_id) = logbook_in("en");
        let (_de_env, de, de_id) = logbook_in("de");
        let en_skeleton = skeleton(&en, &en_id);
        assert_eq!(skeleton(&de, &de_id), en_skeleton, "de and en differ");
        // every new logbook carries the (empty) packages.explicit fence
        // from the start (WP-036)
        for root in [&en, &de] {
            let packages = common::read(&root.join("system/packages.md"));
            assert!(
                packages.ends_with(
                    "\n## Explicit packages\n\n<!-- seldon:begin packages.explicit -->\n<!-- seldon:end -->\n"
                ),
                "{packages}"
            );
        }

        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden/init-skeleton.txt");
        if std::env::var_os("SELDON_BLESS").is_some() {
            std::fs::write(&path, &en_skeleton).unwrap();
        }
        let golden = std::fs::read_to_string(&path)
            .unwrap_or_else(|_| panic!("{} missing; run with SELDON_BLESS=1", path.display()));
        assert_eq!(en_skeleton, golden, "{} differs", path.display());
    }

    #[test]
    fn templates_are_written_as_rendered_with_prose_per_language() {
        for language in [Language::En, Language::De] {
            let (_env, root, machine_id) = logbook_in(language.as_str());
            let lb = Logbook::open(&root).unwrap();
            let vars = templates::Vars {
                machine_id: &machine_id,
                language,
                date: lb.meta.created.to_string()[..10].parse().unwrap(),
            };
            for t in TEMPLATES {
                assert_eq!(
                    common::read(&root.join(t.path)),
                    templates::render(t.text(language), &vars),
                    "{} ({language})",
                    t.path
                );
            }
            let agents = common::read(&root.join("AGENTS.md"));
            let (word, other) = match language {
                Language::En => ("logbook", "Logbuch"),
                Language::De => ("Logbuch", "logbook"),
            };
            // prose only: code spans (`--logbook`, `SELDON_LOGBOOK`) are English
            // in every language
            let prose: String = agents.split('`').step_by(2).collect();
            assert!(prose.contains(word) && !prose.contains(other), "{language}");
            // the rules every agent needs (WP-024)
            for needle in [
                "seldon log",
                "seldon plan new",
                "seldon plan start",
                "seldon drift",
                "seldon hook install claude-code",
                "seldon hook generic",
                "ledger/*.jsonl",
                "## Zones",
                "## The engine is the only writer",
            ] {
                assert!(agents.contains(needle), "{language}: {needle}");
            }
            // WP-050: the engine maintains the decisions table
            let decisions = common::read(&root.join("DECISIONS.md"));
            assert!(
                decisions.contains("`seldon decide` ") && decisions.contains("`seldon status`"),
                "{language}: {decisions}"
            );
        }
        // prose differs wherever a template has prose
        for t in TEMPLATES {
            let (en, de) = (t.text(Language::En), t.text(Language::De));
            let prose = |s: &str| {
                s.lines()
                    .any(|l| !l.is_empty() && !l.starts_with(['#', '|', '<', '-', '`']))
            };
            if prose(en) && t.path != "STATUS.md" {
                assert_ne!(en, de, "{} has no German prose", t.path);
            }
        }
    }

    #[test]
    fn every_command_in_agents_md_is_one_this_engine_has() {
        // WP-100: docs-check covers the guides, nothing else the rules an
        // agent reads; a rule naming a missing command or option fails here
        let env = Env::new(Snapper::Allowed);
        let mut checked = 0;
        for language in [Language::En, Language::De] {
            let text = templates::find("AGENTS.md").unwrap().text(language);
            for span in text.split('`').skip(1).step_by(2) {
                let Some(rest) = span.strip_prefix("seldon ") else {
                    continue;
                };
                let mut argv: Vec<&str> = rest
                    .split_whitespace()
                    .take_while(|w| {
                        !w.starts_with('-') && w.chars().all(|c| c.is_ascii_lowercase() || c == '-')
                    })
                    .collect();
                argv.push("--help");
                let out = env.seldon(&argv);
                assert_eq!(out.status.code(), Some(0), "{language}: `{span}`");
                let help = stdout(&out);
                for flag in rest.split_whitespace().filter(|w| w.starts_with("--")) {
                    if flag == "--" {
                        continue;
                    }
                    assert!(help.contains(flag), "{language}: `{span}`: {flag}");
                }
                checked += 1;
            }
        }
        assert!(checked > 40, "{checked} commands checked");
    }

    /// Omarchy's agent skill, *Privilege Escalation*, as the rules block
    /// quotes it (WP-111): line for line as in
    /// `$OMARCHY_PATH/default/agents/skills/omarchy/SKILL.md`.
    const OMARCHY_PRIVILEGE: &str =
        "For an interactive script or command run in a visible terminal, use `sudo` for
privileged work. Omarchy may grant passwordless `sudo` access to particular
commands, and the terminal is the appropriate place to request a password
when one is needed.

Use `pkexec` only when the caller cannot interact with a terminal or cannot
enter a password there, such as a command launched by an agent or a graphical
background process. Do not replace `sudo` with `pkexec` merely because a
command changes system state.";

    /// The same skill, *Critical Safety Rules*.
    const OMARCHY_NO_WRAP: &str =
        "Do not wrap commands that already manage privilege elevation themselves.";

    /// On a host with Omarchy: the quoted text is still the skill's.
    #[test]
    fn the_quoted_privilege_wording_is_omarchys() {
        let omarchy = std::env::var_os("OMARCHY_PATH")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| "/usr/share/omarchy".into());
        let Ok(skill) =
            std::fs::read_to_string(omarchy.join("default/agents/skills/omarchy/SKILL.md"))
        else {
            eprintln!("skipped: no Omarchy agent skill on this host");
            return;
        };
        assert!(
            skill.contains(OMARCHY_PRIVILEGE),
            "the skill's wording changed"
        );
        assert!(
            skill.contains(OMARCHY_NO_WRAP),
            "the skill's wording changed"
        );
    }

    #[test]
    fn agents_md_carries_the_agent_rules_in_both_languages() {
        // WP-047: the short form of docs/AGENT-GUIDE.md; one file, no CLAUDE.md
        fn section<'a>(text: &'a str, heading: &str) -> &'a str {
            let start = text.find(&format!("{heading}\n")).unwrap() + heading.len();
            let rest = &text[start..];
            &rest[..rest.find("\n## ").unwrap_or(rest.len())]
        }
        // WP-100: the ADR-0027 rules (v2), Seldon's block first, then the
        // user's part
        const SECTIONS: [&str; 19] = [
            "## Session start",
            "## Attended or not",
            "## Instructions and data",
            "## The engine is the only writer",
            "## Work in cases",
            "## When to ask first",
            "## R3: the one stop",
            "## Privileged steps and snapshots",
            "## Zones and risk",
            "## Installing software",
            "## Omarchy first",
            "## Closing",
            "## Commands",
            "## Journal and memory",
            "## Drift",
            "## Hooks",
            "## Ending a session",
            "## Never",
            "## Your rules",
        ];
        let rules: [(&str, &[&str]); 16] = [
            (
                "## Session start",
                &[
                    "PROJECT.md",
                    "memory/lessons.md",
                    "STATUS.md",
                    "seldon plan list --status active",
                    "seldon hook session-start",
                ],
            ),
            (
                "## Attended or not",
                &[
                    "SELDON_ATTENDED=1",
                    "seldon agent start",
                    "sudo",
                    "SELDON_ACTOR",
                    "agent:<name>",
                ],
            ),
            ("## Instructions and data", &["*Intent*", "READMEs"]),
            (
                "## The engine is the only writer",
                &["ledger/*.jsonl", "STATUS.md", ".seldon/", "seldon:begin"],
            ),
            (
                "## Work in cases",
                &[
                    "seldon plan new",
                    "seldon plan start <ID> --actor agent:<name>",
                    "*Intent*",
                    "About to: install X (+deps a, b); snapshot first; rollback:",
                    "--actor agent:<name>",
                ],
            ),
            (
                "## When to ask first",
                &["PKGBUILD", "curl … | sh", "**R3**"],
            ),
            (
                "## R3: the one stop",
                &[
                    "[drift] alwaysRed",
                    "-Sp --print-format %n",
                    "makedepends",
                    "`-Sy`, `-Syy`",
                    "-Syu",
                    "omarchy update",
                    "checkupdates",
                ],
            ),
            (
                "## Privileged steps and snapshots",
                &[
                    "sudo",
                    "pkexec",
                    "*Privilege Escalation*",
                    "> Do not wrap commands that already manage privilege elevation themselves.",
                    "omarchy pkg add",
                    "snapper --csvout list-configs",
                    "pkexec snapper -c <config> create -c number -p -d \"<ID>\"",
                    "snapshot <N> (<config>) before <step>",
                    "omarchy-snapshot create",
                ],
            ),
            ("## Zones and risk", &["**red**", "`R3`"]),
            (
                "## Installing software",
                &[
                    "omarchy pkg add",
                    "omarchy pkg aur add",
                    "makepkg -si",
                    "pkexec pacman -S --needed <",
                    "pkexec pacman -U <",
                    "~/.local",
                    "curl … | sh",
                ],
            ),
            (
                "## Omarchy first",
                &[
                    "$OMARCHY_PATH/default/agents/skills/omarchy/SKILL.md",
                    "omarchy pkg add",
                    "omarchy hook install",
                    "omarchy theme set",
                    "omarchy refresh",
                    "/usr/share/omarchy",
                ],
            ),
            (
                "## Closing",
                &[
                    "seldon plan verify <ID> --actor agent:<name>",
                    "seldon plan done <ID> --actor agent:<name>",
                    "seldon plan drop <ID>",
                    "systemctl is-active",
                ],
            ),
            (
                "## Commands",
                &[
                    "seldon doctor",
                    "seldon plan show <ID>",
                    "seldon drift show <EVENT>",
                    "seldon open",
                    "seldon capture --all",
                    "seldon init",
                    "seldon import … --apply",
                    "seldon rules update",
                    "--json",
                ],
            ),
            (
                "## Drift",
                &[
                    "seldon drift link <EVENT> <CASE>",
                    "seldon drift explain <EVENT> --",
                    "seldon drift show <EVENT> --json",
                    "*Log*",
                    "alwaysRed",
                    "~/.config/omarchy/hooks",
                    "Seldon shows a crisis without a case: <kind> <subject> (<EVENT>).",
                ],
            ),
            (
                "## Hooks",
                &[
                    "seldon hook install claude-code",
                    "seldon hook generic",
                    "seldon hook session-stop --actor agent:<name>",
                    "xargs",
                ],
            ),
            (
                "## Never",
                &["SELDON_LOGBOOK", "git push --force", "areas/*/AGENTS.md"],
            ),
        ];
        // the v1 rules ADR-0027 drops, in either language
        const DROPPED: [&str; 8] = [
            "Propose one to the user",
            "the user agrees",
            "not with the package manager",
            "before you change anything",
            "Schlag dem Nutzer einen vor",
            "der Nutzer zustimmt",
            "nicht direkt mit dem",
            "bevor du etwas änderst",
        ];
        for language in [Language::En, Language::De] {
            let (_env, root, _) = logbook_in(language.as_str());
            let agents = common::read(&root.join("AGENTS.md"));
            let headings: Vec<&str> = agents.lines().filter(|l| l.starts_with("## ")).collect();
            assert_eq!(headings, SECTIONS, "{language}");
            for (heading, needles) in rules {
                for needle in needles {
                    assert!(
                        section(&agents, heading).contains(needle),
                        "{language}: {heading}: {needle}"
                    );
                }
            }
            for dropped in DROPPED {
                assert!(!agents.contains(dropped), "{language}: {dropped}");
            }
            assert!(
                agents.starts_with("<!-- seldon:begin rules v3 -->\n"),
                "{language}"
            );
            // Omarchy's privilege wording, word for word (WP-111)
            let quoted: String = section(&agents, "## Privileged steps and snapshots")
                .lines()
                .filter_map(|l| l.strip_prefix("> ").or(l.strip_prefix(">")))
                .collect::<Vec<_>>()
                .join("\n");
            assert_eq!(
                quoted,
                format!("{OMARCHY_PRIVILEGE}\n{OMARCHY_NO_WRAP}"),
                "{language}"
            );
            // round 2, N8: around the quote, the agent's case first:
            // `pkexec` before `sudo` in the rules' own lines and examples
            let own: String = section(&agents, "## Privileged steps and snapshots")
                .lines()
                .filter(|l| !l.starts_with('>'))
                .collect::<Vec<_>>()
                .join("\n");
            let first = |word: &str| own.find(word).unwrap_or(usize::MAX);
            assert!(first("`pkexec") < first("`sudo"), "{language}: {own}");
            let install = section(&agents, "## Installing software");
            assert!(
                install.find("pkexec pacman -S").unwrap() < install.find("omarchy pkg add").unwrap(),
                "{language}"
            );
            assert!(agents.contains("docs/AGENT-GUIDE.md"), "{language}");
            assert!(!root.join("CLAUDE.md").exists(), "{language}");
        }
    }
}
