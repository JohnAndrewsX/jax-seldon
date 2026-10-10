//! WP-164: the boot configuration through the command line. The config
//! collector hashes `mkinitcpio.conf`, `mkinitcpio.conf.d/*`,
//! `mkinitcpio.d/*`, `default/limine`, `limine-entry-tool.conf` and
//! `limine-entry-tool.d/*` under the system configuration directory,
//! whatever `watchPaths` says; never anything else there, and never the
//! content into the record. Always through `common::Env`: a temp HOME,
//! and the directory `<tmp>/etc` (the guarded default of
//! `SELDON_ETC_DIR`), never the host's.

mod common;

use std::path::{Path, PathBuf};

use serde_json::Value;

use common::{Env, Snapper, json, stderr};

fn write(path: impl AsRef<Path>, bytes: impl AsRef<[u8]>) {
    let path = path.as_ref();
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, bytes).unwrap();
}

/// A new file in place of `path` (a new inode, as pacman extracts one).
fn replace(path: &Path, bytes: &str) {
    let tmp = path.with_extension("seldon-test-new");
    std::fs::write(&tmp, bytes).unwrap();
    std::fs::rename(&tmp, path).unwrap();
}

fn mode(path: &Path, mode: u32) {
    use std::os::unix::fs::PermissionsExt as _;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode)).unwrap();
}

/// Whether the tests run as root (CI): root reads a file whatever its mode.
fn root() -> bool {
    use std::os::unix::fs::MetadataExt as _;
    std::fs::metadata("/proc/self").is_ok_and(|m| m.uid() == 0)
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

/// (kind, subject) of `events`.
fn kinds(events: &[(String, String, Value)]) -> Vec<(String, String)> {
    events
        .iter()
        .map(|(k, s, _)| (k.clone(), s.clone()))
        .collect()
}

fn pair(kind: &str, subject: &str) -> (String, String) {
    (kind.to_string(), subject.to_string())
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

/// The system configuration directory of `env` (`<guard>/etc`).
fn etc(env: &Env) -> PathBuf {
    env.tmp.path().join("etc")
}

fn key(path: &Path) -> String {
    path.to_str().unwrap().to_string()
}

const MKINITCPIO: &str = "MODULES=()\nHOOKS=(base udev autodetect microcode modconf kms keyboard keymap consolefont block filesystems fsck)\n";
const HOOKS: &str = "HOOKS=(base udev plymouth keyboard autodetect microcode modconf kms keymap consolefont block encrypt filesystems fsck btrfs-overlayfs)\n";
const LIMINE: &str = "TARGET_OS_NAME=\"Omarchy\"\nKERNEL_CMDLINE[default]+=\"cryptdevice=PARTUUID=0000:root root=/dev/mapper/root quiet splash\"\n";

/// The boot configuration as Omarchy 4 lays it out, and files beside the
/// listed ones that must never be read.
fn omarchy_boot(env: &Env) {
    let etc = etc(env);
    write(etc.join("mkinitcpio.conf"), MKINITCPIO);
    write(etc.join("mkinitcpio.conf.d/omarchy_hooks.conf"), HOOKS);
    write(
        etc.join("mkinitcpio.conf.d/thunderbolt_module.conf"),
        "MODULES+=(thunderbolt)\n",
    );
    std::fs::create_dir_all(etc.join("mkinitcpio.d")).unwrap();
    write(etc.join("default/limine"), LIMINE);
    write(
        etc.join("limine-entry-tool.conf"),
        "BOOT_ORDER=\"*, *fallback, Snapshots\"\n",
    );
    write(
        etc.join("limine-entry-tool.d/omarchy-defaults.conf"),
        "TIMEOUT=3\n",
    );
    write(etc.join("limine-entry-tool.d/omarchy-uki.conf"), "UKI=no\n");
    // pacman's leftovers in a boot directory: the pacman note covers them
    write(
        etc.join("mkinitcpio.conf.d/omarchy_hooks.conf.pacnew"),
        HOOKS,
    );
    write(
        etc.join("limine-entry-tool.d/omarchy-defaults.conf.pacsave"),
        "TIMEOUT=5\n",
    );
    // not listed (AGENTS.md §6): never hashed, never named
    write(etc.join("mkinitcpio.conf.pacnew"), MKINITCPIO);
    write(etc.join("mkinitcpio.conf.d/old/x.conf"), "MODULES+=(x)\n");
    write(etc.join("default/grub"), "GRUB_TIMEOUT=5\n");
    write(etc.join("pacman.conf"), "[options]\n");
    write(etc.join("crypttab"), "root UUID=secret-uuid none\n");
    write(etc.join("shadow"), "root:$6$secret-hash:19000::::::\n");
    // `/boot/limine*.conf` is not read (root's alone on Omarchy)
    write(env.tmp.path().join("boot/limine.conf"), "/Omarchy\n");
}

/// Every file below `dir` whose bytes hold one of `needles`.
fn holding(dir: &Path, needles: &[&str]) -> Vec<String> {
    let mut found = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return found;
    };
    for entry in entries {
        let p = entry.unwrap().path();
        if p.is_dir() {
            found.extend(holding(&p, needles));
        } else if let Ok(bytes) = std::fs::read(&p) {
            let text = String::from_utf8_lossy(&bytes);
            for n in needles {
                if text.contains(n) {
                    found.push(format!("{} holds {n}", p.display()));
                }
            }
        }
    }
    found
}

/// What the logbook, the state directory and the capture's output may
/// never hold: any of the boot files' text.
const NEVER: [&str; 6] = [
    "HOOKS=",
    "MODULES",
    "cryptdevice",
    "KERNEL_CMDLINE",
    "TIMEOUT",
    "secret-",
];

/// A drop-in added, changed and removed, and `mkinitcpio.conf` edited:
/// each step is one event at the next capture, hash only; a second capture
/// writes nothing; no content reaches the ledger, the index or the state;
/// the files beside the listed ones are never named.
#[test]
fn a_boot_file_change_is_recorded_hash_only() {
    let env = Env::new(Snapper::Missing);
    env.init_logbook();
    omarchy_boot(&env);
    let etc = etc(&env);
    let resume = key(&etc.join("mkinitcpio.conf.d/omarchy_resume.conf"));
    let hooks = key(&etc.join("mkinitcpio.conf.d/omarchy_hooks.conf"));
    let c = capture_config(&env); // baseline
    assert!(config_events(&env).is_empty(), "{c}");
    capture_config(&env);
    assert!(config_events(&env).is_empty());

    // `omarchy-hibernation-setup` writes a drop-in
    write(&resume, "HOOKS+=(resume)\n");
    let c = capture_config(&env);
    let events = config_events(&env);
    assert_eq!(kinds(&events), [pair("config-add", &resume)], "{c}");
    let meta = &events[0].2;
    assert_eq!(meta["hashTo"].as_str().unwrap().len(), 64, "{meta}");
    // readable here: hashed by content
    assert!(meta.get("hashBasis").is_none(), "{meta}");
    assert_eq!(class_of(&env, &resume, "config-add"), "attention config");

    // idempotent
    capture_config(&env);
    capture_config(&env);
    assert_eq!(config_events(&env).len(), 1);

    // an edit of the hooks line, and the same bytes extracted again (an
    // `omarchy-settings` upgrade that leaves the file as it was): one event
    replace(Path::new(&hooks), &HOOKS.replace(" plymouth", ""));
    replace(
        &etc.join("mkinitcpio.conf.d/thunderbolt_module.conf"),
        "MODULES+=(thunderbolt)\n",
    );
    capture_config(&env);
    let events = config_events(&env);
    assert_eq!(kinds(&events[1..]), [pair("config-change", &hooks)]);
    assert_eq!(class_of(&env, &hooks, "config-change"), "attention config");

    // removed: one event, attention
    std::fs::remove_file(&resume).unwrap();
    capture_config(&env);
    let events = config_events(&env);
    assert_eq!(kinds(&events[2..]), [pair("config-remove", &resume)]);
    assert_eq!(
        class_of(&env, &resume, "config-remove"),
        "attention config-remove"
    );

    // the main file and Limine's defaults
    let main = key(&etc.join("mkinitcpio.conf"));
    let limine = key(&etc.join("default/limine"));
    write(&main, MKINITCPIO.replace("MODULES=()", "MODULES=(i915)"));
    write(&limine, LIMINE.replace(" quiet", ""));
    capture_config(&env);
    let events = config_events(&env);
    assert_eq!(
        kinds(&events[3..]),
        [pair("config-change", &limine), pair("config-change", &main)]
    );
    capture_config(&env);
    assert_eq!(config_events(&env).len(), 5);

    // nothing but hashes, anywhere
    let logbook = env.tmp.path().join("logbook");
    let state = env.home.join(".local/state/seldon");
    assert_eq!(holding(&logbook, &NEVER), Vec::<String>::new());
    assert_eq!(holding(&state, &NEVER), Vec::<String>::new());
    let status = env.seldon(&["status", "--json"]);
    assert!(!NEVER.iter().any(|n| common::stdout(&status).contains(n)));
    // only the listed files
    let manifest = common::read(&state.join("manifest.json"));
    for never in [
        ".pacnew",
        ".pacsave",
        "old/x.conf",
        "grub",
        "pacman.conf",
        "crypttab",
        "shadow",
        "boot/limine",
    ] {
        assert!(!manifest.contains(never), "{never} in {manifest}");
    }
    for listed in [
        "etc/mkinitcpio.conf\"",
        "mkinitcpio.conf.d/thunderbolt_module.conf",
        "etc/default/limine",
        "etc/limine-entry-tool.conf\"",
        "limine-entry-tool.d/omarchy-uki.conf",
    ] {
        assert!(manifest.contains(listed), "{listed} not in {manifest}");
    }
}

/// A preset in `mkinitcpio.d` (other layouts than Omarchy's) and a
/// `limine-entry-tool.d` drop-in are watched like the rest; a `.pacnew`
/// that pacman leaves, and its removal by `pacdiff`, write nothing — the
/// merge into the file itself is one change.
#[test]
fn presets_drop_ins_and_pacnew_merges() {
    let env = Env::new(Snapper::Missing);
    env.init_logbook();
    omarchy_boot(&env);
    let etc = etc(&env);
    capture_config(&env); // baseline
    let preset = etc.join("mkinitcpio.d/linux.preset");
    let uki = etc.join("limine-entry-tool.d/omarchy-uki.conf");
    write(&preset, "PRESETS=('default')\n");
    write(&uki, "UKI=yes\n");
    let pacnew = etc.join("limine-entry-tool.d/omarchy-uki.conf.pacnew");
    write(&pacnew, "UKI=no\nUKI_LOCATION=/boot/EFI\n");
    capture_config(&env);
    assert_eq!(
        kinds(&config_events(&env)),
        [
            pair("config-change", &key(&uki)),
            pair("config-add", &key(&preset))
        ]
    );
    // `pacdiff` merges and removes the .pacnew
    write(&uki, "UKI=yes\nUKI_LOCATION=/boot/EFI\n");
    std::fs::remove_file(&pacnew).unwrap();
    capture_config(&env);
    assert_eq!(
        kinds(&config_events(&env)[2..]),
        [pair("config-change", &key(&uki))]
    );
}

/// A drop-in the user cannot read is hashed by its metadata (size,
/// mtime, ctime, inode) and the event says so. Added, rewritten and
/// removed, it is seen each time; an unchanged file is quiet.
#[test]
fn an_unreadable_boot_file_is_hashed_by_its_metadata() {
    if root() {
        eprintln!("skipped: root reads every file");
        return;
    }
    let env = Env::new(Snapper::Missing);
    env.init_logbook();
    omarchy_boot(&env);
    let etc = etc(&env);
    let main = etc.join("mkinitcpio.conf");
    mode(&main, 0o000);
    capture_config(&env); // baseline
    capture_config(&env);
    assert!(config_events(&env).is_empty());

    // `omarchy-provision-owner` writes a key drop-in, mode 0600
    let key_conf = etc.join("mkinitcpio.conf.d/99-omarchy-provisioning-key.conf");
    write(&key_conf, "FILES+=(/etc/omarchy/provisioning.key)\n");
    mode(&key_conf, 0o000);
    let c = capture_config(&env);
    let events = config_events(&env);
    assert_eq!(kinds(&events), [pair("config-add", &key(&key_conf))], "{c}");
    assert_eq!(events[0].2["hashBasis"], "stat", "{:?}", events[0].2);
    // no note: an unreadable boot file is no fault of the walk
    assert_eq!(message(&c), "");
    capture_config(&env);
    assert_eq!(config_events(&env).len(), 1);

    // rewritten in place, its mtime put back: the ctime still moves
    mode(&main, 0o600);
    let before = std::fs::metadata(&main).unwrap().modified().unwrap();
    std::fs::write(&main, MKINITCPIO.replace("fsck", "fsck btrfs")).unwrap();
    std::fs::File::options()
        .write(true)
        .open(&main)
        .unwrap()
        .set_modified(before)
        .unwrap();
    mode(&main, 0o000);
    capture_config(&env);
    let events = config_events(&env);
    assert_eq!(kinds(&events[1..]), [pair("config-change", &key(&main))]);
    assert_eq!(events[1].2["hashBasis"], "stat");

    // removed: seen, although never readable
    std::fs::remove_file(&key_conf).unwrap();
    capture_config(&env);
    let events = config_events(&env);
    assert_eq!(
        kinds(&events[2..]),
        [pair("config-remove", &key(&key_conf))]
    );
    capture_config(&env);
    assert_eq!(config_events(&env).len(), 3);
    let state = env.home.join(".local/state/seldon");
    assert_eq!(holding(&state, &NEVER), Vec::<String>::new());
}

/// The boot files enter the watch scope without events (the first capture
/// after the upgrade), and `[redaction] skipPaths` takes them out again:
/// no file named, no event, no removal.
#[test]
fn the_boot_files_enter_quietly_and_skip_paths_opt_out() {
    let env = Env::new(Snapper::Missing);
    env.init_logbook();
    omarchy_boot(&env);
    let etc = etc(&env);
    let set_skip = |patterns: &[String]| {
        let file = env.config_file();
        let mut config: toml::Table = toml::from_str(&common::read(&file)).unwrap();
        let redaction = config
            .entry("redaction")
            .or_insert_with(|| toml::Value::Table(toml::Table::new()))
            .as_table_mut()
            .unwrap();
        let list = patterns.iter().cloned().map(toml::Value::String).collect();
        redaction.insert("skipPaths".into(), toml::Value::Array(list));
        std::fs::write(&file, toml::to_string(&config).unwrap()).unwrap();
    };
    // a user who opts out: the files are never named
    set_skip(&[
        format!("{}/mkinitcpio*", etc.display()),
        format!("{}/limine*", etc.display()),
        format!("{}/default/limine", etc.display()),
    ]);
    capture_config(&env); // baseline
    write(
        etc.join("mkinitcpio.conf.d/omarchy_hooks.conf"),
        "HOOKS=()\n",
    );
    write(etc.join("default/limine"), "TIMEOUT=1\n");
    capture_config(&env);
    assert!(config_events(&env).is_empty(), "{:?}", config_events(&env));
    let state = env.home.join(".local/state/seldon");
    let manifest: Value =
        serde_json::from_str(&common::read(&state.join("manifest.json"))).unwrap();
    let files = manifest["files"].as_object().unwrap();
    assert!(
        !files
            .keys()
            .any(|k| k.contains("mkinitcpio") || k.contains("limine")),
        "{manifest}"
    );
    assert!(manifest.get("skipped").is_none(), "{manifest}");

    // the files enter the scope (as at the upgrade): no event for them
    set_skip(&[]);
    let c = capture_config(&env);
    assert!(config_events(&env).is_empty(), "{:?}", config_events(&env));
    assert!(
        message(&c).contains("watch scope changed: 0 file(s) left it, 7 entered it"),
        "{c}"
    );
    // and from now on they are watched
    write(etc.join("default/limine"), LIMINE);
    capture_config(&env);
    assert_eq!(
        kinds(&config_events(&env)),
        [pair("config-change", &key(&etc.join("default/limine")))]
    );
}

/// No boot directory at all (another distribution, a container): nothing
/// is hashed, nothing is noted; a boot file that appears later is one
/// addition.
#[test]
fn missing_boot_files_are_quiet() {
    let env = Env::new(Snapper::Missing);
    env.init_logbook();
    let c = capture_config(&env);
    assert_eq!(message(&c), "");
    let limine = etc(&env).join("default/limine");
    write(&limine, LIMINE);
    let c = capture_config(&env);
    assert_eq!(message(&c), "");
    assert_eq!(
        kinds(&config_events(&env)),
        [pair("config-add", &key(&limine))]
    );
}

/// ADR-0028 gives a boot file no crisis row (attention, above); a user
/// who wants one lists the path in `[drift] alwaysRedPaths`.
#[test]
fn always_red_paths_make_a_boot_file_a_crisis() {
    let env = Env::new(Snapper::Missing);
    env.init_logbook();
    omarchy_boot(&env);
    let etc = etc(&env);
    let file = env.config_file();
    let mut config: toml::Table = toml::from_str(&common::read(&file)).unwrap();
    let drift = config
        .entry("drift")
        .or_insert_with(|| toml::Value::Table(toml::Table::new()))
        .as_table_mut()
        .unwrap();
    let pattern = format!("{}/mkinitcpio.conf.d/**", etc.display());
    drift.insert(
        "alwaysRedPaths".into(),
        toml::Value::Array(vec![toml::Value::String(pattern)]),
    );
    std::fs::write(&file, toml::to_string(&config).unwrap()).unwrap();
    capture_config(&env); // baseline
    let hooks = etc.join("mkinitcpio.conf.d/omarchy_hooks.conf");
    write(&hooks, "HOOKS=()\n");
    write(etc.join("default/limine"), "TIMEOUT=1\n");
    capture_config(&env);
    assert_eq!(
        class_of(&env, &key(&hooks), "config-change"),
        "crisis always-red-paths"
    );
    assert_eq!(
        class_of(&env, &key(&etc.join("default/limine")), "config-change"),
        "attention config"
    );
}

/// WP-164 round 3: a drop-in that is a symlink into `/usr/` gets no
/// evidence mark. ADR-0037 §2 gives `system-link` to links under the home
/// directory only; a boot file linked to a shipped file is a boot change
/// like any other: `config-add`, hash only, attention.
#[test]
fn a_boot_symlink_into_usr_is_no_system_link() {
    let Some(target) = [
        "/usr/share/zoneinfo/UTC",
        "/usr/lib/os-release",
        "/usr/share/licenses/glibc/LICENSE",
    ]
    .into_iter()
    .map(Path::new)
    .find(|p| {
        std::fs::canonicalize(p).is_ok_and(|t| t.starts_with("/usr/")) && std::fs::read(p).is_ok()
    }) else {
        eprintln!("skipped: no readable file under /usr/ on this host");
        return;
    };
    let env = Env::new(Snapper::Missing);
    env.init_logbook();
    omarchy_boot(&env);
    capture_config(&env); // baseline
    let link = etc(&env).join("mkinitcpio.conf.d/zz-shipped.conf");
    std::os::unix::fs::symlink(target, &link).unwrap();
    capture_config(&env);
    let events = config_events(&env);
    assert_eq!(kinds(&events), [pair("config-add", &key(&link))]);
    let meta = &events[0].2;
    assert!(meta.get("matches").is_none(), "{meta}");
    assert_eq!(meta["hashTo"].as_str().unwrap().len(), 64, "{meta}");
    assert_eq!(
        class_of(&env, &key(&link), "config-add"),
        "attention config"
    );
    capture_config(&env);
    assert_eq!(config_events(&env).len(), 1);
}
