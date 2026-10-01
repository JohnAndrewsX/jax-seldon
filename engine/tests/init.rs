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
            "--path",
            path.to_str().unwrap(),
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
            ".obsidian/workspace*\n.seldon/active-case\n"
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

        // snapper hint is printed, never run
        let text = stdout(&out);
        assert!(
            text.contains("sudo snapper -c root set-config ALLOW_USERS=$USER SYNC_ACL=yes"),
            "{text}"
        );
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
        assert!(stdout(&out).contains("seldon hook install claude-code"));
    }

    #[test]
    fn language_from_locale() {
        let env = Env::new(Snapper::Allowed);
        let path = env.tmp.path().join("logbook");
        let out = env
            .command(&[
                "init",
                "--non-interactive",
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
    fn default_path_is_home_seldon_and_env_overrides_it() {
        let env = Env::new(Snapper::Allowed);
        let out = env.seldon(&["init", "--non-interactive", "--no-git"]);
        assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
        assert!(Logbook::is_initialised(&env.home.join("Seldon")));

        let env = Env::new(Snapper::Allowed);
        let target = env.tmp.path().join("from-env");
        let out = env
            .command(&["init", "--non-interactive", "--no-git"])
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
