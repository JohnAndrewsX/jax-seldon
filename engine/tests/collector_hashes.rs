//! WP-113 (ADR-0028 §8 WP-E) through the command line: the hook-path
//! blind spots (a linked hook directory, a binary script, a script over
//! 1 MiB) yield their events, the toggles directory is hashed with
//! Omarchy's flags as evidence, and `~/.ssh/authorized_keys` is a crisis
//! once the user watches it. Always through `common::Env` (a temp HOME;
//! Omarchy's tree is `<tmp>/omarchy`, never the host's).

mod common;

use std::path::Path;

use serde_json::Value;

use common::{Env, Snapper, json, read, stderr};

fn write(path: impl AsRef<Path>, bytes: impl AsRef<[u8]>) {
    let path = path.as_ref();
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, bytes).unwrap();
}

fn symlink(target: &Path, link: &Path) {
    std::fs::create_dir_all(link.parent().unwrap()).unwrap();
    std::os::unix::fs::symlink(target, link).unwrap();
}

/// `dir` and every directory below it get `mode` (the guard's owner owns
/// them: under `SELDON_TEST_GUARD` that stands in for root).
fn trust(dir: &Path, mode: u32) {
    use std::os::unix::fs::PermissionsExt as _;
    for entry in std::fs::read_dir(dir).unwrap() {
        let p = entry.unwrap().path();
        if p.is_dir() {
            trust(&p, mode);
        }
    }
    std::fs::set_permissions(dir, std::fs::Permissions::from_mode(mode)).unwrap();
}

fn ok(env: &Env, args: &[&str]) -> Value {
    let out = env.seldon(args);
    assert_eq!(out.status.code(), Some(0), "{args:?}: {}", stderr(&out));
    json(&out)
}

fn capture_config(env: &Env) -> Value {
    let c = ok(env, &["capture", "--source", "config", "--json"]);
    assert_eq!(c["ok"], true, "{c}");
    c
}

/// The config collector's message of a capture, empty without one.
fn message(capture: &Value) -> String {
    capture["collectors"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["name"] == "config")
        .and_then(|c| c["message"].as_str())
        .unwrap_or_default()
        .to_string()
}

/// The config events of the ledger: (kind, subject, meta).
fn config_events(env: &Env) -> Vec<(String, String, Value)> {
    common::ledger(&env.tmp.path().join("logbook"))
        .into_iter()
        .filter(|e| e["source"] == "config")
        .map(|e| {
            (
                e["kind"].as_str().unwrap().to_string(),
                e["subject"].as_str().unwrap().to_string(),
                e["meta"].clone(),
            )
        })
        .collect()
}

/// `seldon drift --all --json`: "class rule" of the item of `subject`.
fn class(env: &Env, subject: &str) -> String {
    ok(env, &["drift", "--all", "--json"])["drift"]
        .as_array()
        .unwrap()
        .iter()
        .find(|d| d["subject"] == subject)
        .map(|d| {
            format!(
                "{} {}",
                d["class"].as_str().unwrap(),
                d["rule"].as_str().unwrap()
            )
        })
        .unwrap_or_else(|| panic!("no drift item {subject}"))
}

/// WP-109 stage 1: three ways a hook ran without an event. Each now yields
/// one on the persistence path, hash only, and is a crisis; outside the
/// persistence paths nothing changes. A second capture writes nothing.
#[test]
fn hook_blind_spots_yield_crises() {
    let env = Env::new(Snapper::Missing);
    env.init_logbook();
    let home = &env.home;
    write(
        home.join(".config/omarchy/hooks/post-update.d/x.sample"),
        "#!/bin/bash\n",
    );
    write(home.join(".config/hypr/hyprland.lua"), "-- mine\n");
    capture_config(&env); // baseline

    // a hook directory that is a link (dotfile managers)
    write(
        home.join("dotfiles/boot.d/run.sh"),
        "#!/bin/bash\necho boot\n",
    );
    symlink(
        &home.join("dotfiles/boot.d"),
        &home.join(".config/omarchy/hooks/post-boot.d"),
    );
    // bash runs a script with a NUL after its first line
    let binary = b"#!/bin/bash\necho hi\n\0\0payload\n".to_vec();
    write(
        home.join(".config/omarchy/hooks/post-update.d/nul.sh"),
        &binary,
    );
    // a script over 1 MiB
    let mut big = b"#!/bin/bash\n".to_vec();
    big.resize(2 << 20, b'#');
    write(home.join(".config/omarchy/hooks/theme-set.d/big.sh"), &big);
    // outside the persistence paths: a binary file stays skipped and a
    // linked directory is not followed
    write(home.join(".config/hypr/blob.lua"), b"--\0\0");
    write(home.join("dotfiles/hypr/extra.lua"), "-- extra\n");
    symlink(
        &home.join("dotfiles/hypr"),
        &home.join(".config/hypr/linked"),
    );

    capture_config(&env);
    let events = config_events(&env);
    let added: Vec<&str> = events
        .iter()
        .filter(|e| e.0 == "config-add")
        .map(|e| e.1.as_str())
        .collect();
    assert_eq!(
        added,
        [
            "~/.config/omarchy/hooks/post-boot.d/run.sh",
            "~/.config/omarchy/hooks/post-update.d/nul.sh",
            "~/.config/omarchy/hooks/theme-set.d/big.sh",
        ]
    );
    let meta = |s: &str| &events.iter().find(|e| e.1 == s).unwrap().2;
    assert_eq!(
        meta("~/.config/omarchy/hooks/post-update.d/nul.sh")["hashTo"],
        seldon::sys::sha256_hex(&binary)
    );
    assert_eq!(
        meta("~/.config/omarchy/hooks/theme-set.d/big.sh")["hashTo"],
        seldon::sys::sha256_hex(&big)
    );
    for s in &added {
        assert_eq!(class(&env, s), "crisis always-red-paths", "{s}");
    }
    // hashes only: neither content nor the link target reaches the ledger
    let ledger = std::fs::read_dir(env.tmp.path().join("logbook/ledger"))
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|x| x == "jsonl"))
        .map(|p| read(&p))
        .collect::<String>();
    assert!(
        !ledger.contains("payload") && !ledger.contains("dotfiles"),
        "{ledger}"
    );

    // idempotent
    let n = config_events(&env).len();
    capture_config(&env);
    assert_eq!(config_events(&env).len(), n);

    // a change behind the link and in the binary script: one event each
    write(
        home.join("dotfiles/boot.d/run.sh"),
        "#!/bin/bash\necho evil\n",
    );
    write(
        home.join(".config/omarchy/hooks/post-update.d/nul.sh"),
        b"#!/bin/bash\n\0x",
    );
    capture_config(&env);
    let changed: Vec<String> = config_events(&env)[n..]
        .iter()
        .map(|e| format!("{} {}", e.0, e.1))
        .collect();
    assert_eq!(
        changed,
        [
            "config-change ~/.config/omarchy/hooks/post-boot.d/run.sh",
            "config-change ~/.config/omarchy/hooks/post-update.d/nul.sh",
        ]
    );
    let n = config_events(&env).len();
    capture_config(&env);
    assert_eq!(config_events(&env).len(), n);
}

/// The hook directory itself may be a link; a link back into the tree is
/// not followed, and one link records at most `LINKED_FILES` files. Both
/// are counted in the collector's message.
#[test]
fn a_linked_hook_directory_loops_and_its_budget() {
    let env = Env::new(Snapper::Missing);
    env.init_logbook();
    let home = &env.home;
    let real = home.join("dotfiles/omarchy-hooks");
    write(real.join("post-update.d/one.sh"), "#!/bin/bash\n");
    symlink(&real, &home.join(".config/omarchy/hooks"));
    capture_config(&env); // baseline

    write(real.join("post-update.d/two.sh"), "#!/bin/bash\necho two\n");
    symlink(&real, &real.join("post-update.d/back"));
    let many = seldon::collectors::config::LINKED_FILES + 3;
    for i in 0..many {
        write(real.join(format!("zz.d/f{i:04}")), format!("{i}\n"));
    }
    let c = capture_config(&env);
    let message = message(&c);
    assert!(
        message.contains("1 linked director(ies) not followed"),
        "{message}"
    );
    // a directory's files first: two.sh, then zz.d fills the budget
    // (one.sh is no new file)
    assert!(
        message.contains("5 file(s) not watched: more than 1024"),
        "{message}"
    );
    let added: Vec<String> = config_events(&env)
        .into_iter()
        .filter(|e| e.0 == "config-add")
        .map(|e| e.1)
        .collect();
    assert!(added.contains(&"~/.config/omarchy/hooks/post-update.d/two.sh".to_string()));
    assert!(added.iter().all(|s| !s.contains("/back/")), "{added:?}");
    // a file past the budget is skipped, not removed
    assert!(
        config_events(&env).iter().all(|e| e.0 != "config-remove"),
        "{:?}",
        config_events(&env)
    );
}

/// Upgrading to WP-113 hashes a binary or large hook that an earlier
/// engine listed as skipped: no event for it (the move is no change), and
/// its later changes are seen. Stands in for the earlier engine: a config
/// without persistence paths.
#[test]
fn a_skipped_hook_becomes_hashed_without_an_event() {
    let env = Env::new(Snapper::Missing);
    env.init_logbook();
    let home = &env.home;
    let hook = home.join(".config/omarchy/hooks/post-update.d/nul.sh");
    write(&hook, b"#!/bin/bash\n\0");
    let text = read(&env.config_file());
    let mut t: toml::Table = text.parse().unwrap();
    t["drift"].as_table_mut().unwrap().insert(
        "alwaysRedPaths".into(),
        toml::Value::Array(vec!["~/nothing".into()]),
    );
    std::fs::write(env.config_file(), toml::to_string(&t).unwrap()).unwrap();
    capture_config(&env); // baseline: skipped
    std::fs::write(env.config_file(), &text).unwrap();
    capture_config(&env);
    assert!(config_events(&env).is_empty(), "{:?}", config_events(&env));
    write(&hook, b"#!/bin/bash\n\0\0");
    capture_config(&env);
    let events = config_events(&env);
    assert_eq!(events.len(), 1, "{events:?}");
    assert_eq!(events[0].0, "config-change");
}

/// `omarchy-hyprland-toggle <flag> on` copies Omarchy's flag into the
/// toggles directory: routine by evidence. Lua of unknown origin there is
/// attention. Turning a flag off removes it: the removal carries the
/// evidence (recorded for ADR-0028 amendment B) and is attention until
/// that amendment.
#[test]
fn toggles_are_hashed_with_omarchy_flags_as_evidence() {
    let env = Env::new(Snapper::Missing);
    env.init_logbook();
    let home = &env.home;
    let omarchy = env.tmp.path().join("omarchy");
    let flag = "-- window-no-gaps\nhl.config({ general = { gaps_in = 0 } })\n";
    write(
        omarchy.join("default/hypr/toggles/window-no-gaps.lua"),
        flag,
    );
    write(omarchy.join("default/hypr/toggles/flags.lua"), "-- flags\n");
    trust(&omarchy, 0o755);
    let toggles = home.join(".local/state/omarchy/toggles/hypr");
    write(toggles.join("flags.lua"), "-- flags\n");
    capture_config(&env); // baseline

    write(toggles.join("window-no-gaps.lua"), flag);
    write(toggles.join("mine.lua"), "hl.exec('x')\n");
    capture_config(&env);
    let events = config_events(&env);
    let mark = |s: &str| events.iter().find(|e| e.1 == s).unwrap().2["matches"].clone();
    let key = |n: &str| format!("~/.local/state/omarchy/toggles/hypr/{n}");
    assert_eq!(mark(&key("window-no-gaps.lua")), "omarchy-default");
    assert_eq!(mark(&key("mine.lua")), Value::Null);
    assert_eq!(
        class(&env, &key("window-no-gaps.lua")),
        "routine omarchy-default"
    );
    assert_eq!(class(&env, &key("mine.lua")), "attention config");

    // off
    std::fs::remove_file(toggles.join("window-no-gaps.lua")).unwrap();
    capture_config(&env);
    let events = config_events(&env);
    let off = events.last().unwrap();
    assert_eq!(
        (off.0.as_str(), off.1.as_str()),
        ("config-remove", key("window-no-gaps.lua").as_str())
    );
    assert_eq!(off.2["matches"], "omarchy-default");
    assert_eq!(
        class(&env, &key("window-no-gaps.lua")),
        "attention config-remove"
    );
    let n = events.len();
    capture_config(&env);
    assert_eq!(config_events(&env).len(), n, "idempotent");
}

/// `~/.ssh/authorized_keys` is no default watch path; once the user adds
/// it to `watchPaths` (the opt-in), a change is a crisis by the default
/// `alwaysRedPaths`, and only its hash is recorded.
#[test]
fn authorized_keys_is_a_crisis_once_watched() {
    let env = Env::new(Snapper::Missing);
    env.init_logbook();
    let keys = env.home.join(".ssh/authorized_keys");
    write(
        &keys,
        "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIEXAMPLE user@workstation\n",
    );
    capture_config(&env); // baseline
    write(
        &keys,
        "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIEXAMPLE2 other@example\n",
    );
    capture_config(&env);
    assert!(config_events(&env).is_empty(), "not watched by default");

    let text = read(&env.config_file());
    let opted = text.replacen(
        "watchPaths = [",
        "watchPaths = [\"~/.ssh/authorized_keys\", ",
        1,
    );
    assert_ne!(opted, text);
    std::fs::write(env.config_file(), opted).unwrap();
    let c = capture_config(&env);
    assert!(
        config_events(&env).is_empty(),
        "entering the scope is no change"
    );
    assert!(message(&c).contains("watch scope changed"), "{c}");

    write(
        &keys,
        "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIEXAMPLE3 attacker@example\n",
    );
    capture_config(&env);
    let events = config_events(&env);
    assert_eq!(events.len(), 1, "{events:?}");
    assert_eq!(events[0].1, "~/.ssh/authorized_keys");
    assert_eq!(
        class(&env, "~/.ssh/authorized_keys"),
        "crisis always-red-paths"
    );
    let ledger = common::ledger(&env.tmp.path().join("logbook"));
    assert!(
        !serde_json::to_string(&ledger).unwrap().contains("attacker"),
        "hashes only"
    );
}

/// A third-party plugin edited in place is quiet attention (ADR-0028 §2,
/// `plugin-*` row); Seldon's own plugin edited in place is explained by
/// SPEC-ENGINE §5 rule 8 (the dev install, an update through Omarchy).
#[test]
fn plugin_tree_changes_classify_by_the_plugin_rows() {
    let env = Env::new(Snapper::Missing);
    env.init_logbook();
    env.stub(
        "omarchy",
        r#"case "$2" in
list) printf '%s\n' '[{"id":"jax.seldon","enabled":true,"firstParty":false},{"id":"io.github.example.x","enabled":true,"firstParty":false}]';;
*) exit 1;;
esac"#,
    );
    let plugins = env.home.join(".config/omarchy/plugins");
    for id in ["jax.seldon", "io.github.example.x"] {
        write(
            plugins.join(id).join("manifest.json"),
            format!("{{\"id\":\"{id}\",\"version\":\"1.0.0\"}}"),
        );
        write(plugins.join(id).join("Widget.qml"), "Item {}\n");
    }
    let capture = || {
        let c = ok(&env, &["capture", "--source", "plugins", "--json"]);
        assert_eq!(c["ok"], true, "{c}");
    };
    capture(); // baseline
    for id in ["jax.seldon", "io.github.example.x"] {
        write(
            plugins.join(id).join("Widget.qml"),
            "Item { visible: false }\n",
        );
    }
    capture();
    let ledger = common::ledger(&env.tmp.path().join("logbook"));
    let updates: Vec<&Value> = ledger
        .iter()
        .filter(|e| e["kind"] == "plugin-update")
        .collect();
    assert_eq!(updates.len(), 2, "{ledger:?}");
    let own = updates
        .iter()
        .find(|e| e["subject"] == "jax.seldon")
        .unwrap();
    assert!(
        ledger.iter().any(|r| r["kind"] == "resolution"
            && r["refersTo"] == own["id"]
            && r["detail"] == "seldon's own plugin"),
        "{ledger:?}"
    );
    assert_eq!(class(&env, "io.github.example.x"), "attention plugin");
    let open: Vec<String> = ok(&env, &["drift", "--json"])["drift"]
        .as_array()
        .unwrap()
        .iter()
        .map(|d| d["subject"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(open, ["io.github.example.x"]);
}
