//! `seldon inbox add` (WP-166, E28 step 1): a text filed into the
//! logbook's `inbox/`, redacted as `import task` redacts a task file; the
//! same text again changes nothing, another text under a taken name gets
//! `-2`; one commit of the new file alone.

mod common;

use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Output, Stdio};

use common::{Env, Snapper, json, mode, read, stderr, stdout, tree};
use serde_json::Value;

const T0: &str = "2026-10-08T14:03:00+02:00";
const T1: &str = "2026-10-09T09:00:00+02:00";

const REPORT: &str = "\
## What crashed

waybar (PID 4242) died with SIGSEGV in libgtk-3.so.0.

## Mechanism

- proven: the crashing thread was the tray watcher
- inferred: a tray icon went away while it was drawn
";

/// `seldon <args>` at `now` with `stdin` piped in (none: an empty stdin).
fn run(env: &Env, now: &str, args: &[&str], stdin: Option<&str>) -> Output {
    let mut cmd = env.command(args);
    cmd.env("SELDON_NOW", now)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = cmd.spawn().expect("run seldon");
    let mut pipe = child.stdin.take().unwrap();
    pipe.write_all(stdin.unwrap_or("").as_bytes()).unwrap();
    drop(pipe);
    child.wait_with_output().unwrap()
}

/// `inbox add --json --title <title> --file - <more>` with `text` on stdin;
/// exits 0.
fn add(env: &Env, now: &str, title: &str, text: &str, more: &[&str]) -> Value {
    let mut args = vec!["inbox", "add", "--json", "--title", title, "--file", "-"];
    args.extend(more);
    let out = run(env, now, &args, Some(text));
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    json(&out)
}

/// `inbox/` of the logbook: names and texts, `.gitkeep` left out.
fn inbox(root: &Path) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = std::fs::read_dir(root.join("inbox"))
        .unwrap()
        .map(|e| e.unwrap())
        .map(|e| e.file_name().into_string().unwrap())
        .filter(|n| n != ".gitkeep")
        // a dangling link reads as empty
        .map(|n| {
            let text = std::fs::read_to_string(root.join("inbox").join(&n)).unwrap_or_default();
            (n, text)
        })
        .collect();
    out.sort();
    out
}

fn git_out(env: &Env, root: &Path, args: &[&str]) -> String {
    let out = env.git(root, args);
    assert!(out.status.success(), "git {args:?}: {}", stderr(&out));
    stdout(&out)
}

fn commits(env: &Env, root: &Path) -> usize {
    git_out(env, root, &["rev-list", "--count", "HEAD"])
        .trim()
        .parse()
        .unwrap()
}

#[test]
fn a_report_on_stdin_is_filed_and_committed_alone() {
    let env = Env::new(Snapper::Missing);
    let root = env.init_logbook();
    // the user's own edit, pending: it stays out of the inbox commit
    std::fs::write(root.join("inbox/zed.md"), "# Zed\n\nmine\n").unwrap();
    let before = env.has_git.then(|| commits(&env, &root));

    let mut cmd = env.command(&[
        "inbox",
        "add",
        "--json",
        "--title",
        "Crash: waybar (SIGSEGV)",
        "--tag",
        "crash",
        "--file",
        "-",
    ]);
    cmd.env("SELDON_NOW", T0)
        .env("SELDON_ACTOR", "agent:claude-code")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = cmd.spawn().unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(format!("\n\n{REPORT}\n\n").as_bytes())
        .unwrap();
    let out = child.wait_with_output().unwrap();
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    let v = json(&out);

    let rel = "inbox/2026-10-08-crash-waybar-sigsegv.md";
    assert_eq!(v["filed"], true);
    assert_eq!(v["path"], rel);
    assert_eq!(v["title"], "Crash: waybar (SIGSEGV)");
    assert_eq!(v["actor"], "agent:claude-code");
    assert_eq!(v["tags"], serde_json::json!(["crash"]));
    assert_eq!(v["redactedLines"], 0);
    assert_eq!(v["privatePaths"], 0);
    assert_eq!(v["droppedCharacters"], 0);
    // blank lines around the text dropped, one final newline
    assert_eq!(
        read(&root.join(rel)),
        format!(
            "---\ntype: inbox\ncreated: {T0}\nactor: agent:claude-code\ntags: [crash]\n---\n# Crash: waybar (SIGSEGV)\n\n{REPORT}"
        )
    );
    assert_eq!(mode(&root.join(rel)) & 0o777, 0o600);

    if let Some(before) = before {
        assert_eq!(v["git"]["committed"], true, "{v}");
        assert_eq!(v["git"]["message"], "seldon: inbox add");
        assert_eq!(commits(&env, &root), before + 1);
        let files = git_out(&env, &root, &["show", "--name-only", "--format=", "HEAD"]);
        assert_eq!(files, format!("{rel}\n"));
        let status = git_out(&env, &root, &["status", "--porcelain"]);
        assert!(status.contains("inbox/zed.md"), "{status}");
        // the index was rebuilt after the commit
        let index: Value =
            serde_json::from_str(&read(&env.home.join(".local/state/seldon/index.json"))).unwrap();
        let head = git_out(&env, &root, &["rev-parse", "HEAD"]);
        let git = &index["logbook"]["git"];
        let short = git["head"].as_str().unwrap_or_default();
        assert!(short.len() >= 7 && head.starts_with(short), "{git}");
        assert_eq!(
            index["logbook"]["git"]["autocommit"]["message"],
            "seldon: inbox add"
        );
    }
}

#[test]
fn a_file_is_filed_with_its_actor_and_the_human_line() {
    let env = Env::new(Snapper::Missing);
    let root = env.init_logbook();
    let file = env.tmp.path().join("report.md");
    std::fs::write(&file, REPORT).unwrap();
    let out = env.at(
        T0,
        &[
            "inbox",
            "add",
            "--title",
            "Zed folgt dem Theme nicht",
            "--file",
            file.to_str().unwrap(),
            "--actor",
            "agent:codex",
            "--tag",
            "#theme",
            "--tag",
            "dev/editor",
        ],
    );
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    assert!(
        stdout(&out).starts_with(
            "Filed inbox/2026-10-08-zed-folgt-dem-theme-nicht.md: Zed folgt dem Theme nicht"
        ),
        "{}",
        stdout(&out)
    );
    let text = read(&root.join("inbox/2026-10-08-zed-folgt-dem-theme-nicht.md"));
    assert!(
        text.starts_with(&format!(
            "---\ntype: inbox\ncreated: {T0}\nactor: agent:codex\ntags: [theme, dev/editor]\n---\n# Zed folgt dem Theme nicht\n\n"
        )),
        "{text}"
    );
    assert!(text.ends_with(REPORT), "{text}");
    // the source is only read
    assert_eq!(read(&file), REPORT);

    // no tag, human by default; CRLF read as LF
    let v = add(&env, T0, "Ohne Schlagwort", "eins\r\nzwei\r\n", &[]);
    assert_eq!(v["actor"], "human");
    let text = read(&root.join(v["path"].as_str().unwrap()));
    assert!(
        text.ends_with("actor: human\ntags: []\n---\n# Ohne Schlagwort\n\neins\nzwei\n"),
        "{text}"
    );
}

#[test]
fn secrets_home_paths_and_format_characters_never_reach_the_inbox() {
    let env = Env::new(Snapper::Missing);
    let root = env.init_logbook();
    let key_body = "MIIEvQIBADANBgkqhkiG9w0BAQEFAASCBKcwggSjAgEAAoIBAQC7\nb3RoZXJsaW5lb2ZiYXNlNjRrZXltYXRlcmlhbGhlcmU0Mg==";
    let text = format!(
        "env: GITHUB_TOKEN=ghp_0123456789abcdefghijABCDEFGHIJ012345\n\
         core: /home/alice/.cache/waybar/core.4242\n\
         cmd: curl -H 'Authorization: Bearer s3cr3tbearervalue' https://x\n\
         split: to\u{200B}ken=zerowidthsecret42\n\
         -----BEGIN PRIVATE KEY-----\n{key_body}\n-----END PRIVATE KEY-----\n\
         rest is plain\n"
    );
    let v = add(
        &env,
        T0,
        "Crash in /home/alice/bin/tool with password=hunter2hunter2",
        &text,
        &[
            "--tag",
            "crash",
            "--tag",
            "ghp_0123456789abcdefghijABCDEFGHIJ012345",
        ],
    );
    let path = root.join(v["path"].as_str().unwrap());
    let filed = read(&path);
    for secret in [
        "/home/alice",
        "ghp_0123456789abcdefghijABCDEFGHIJ012345",
        "s3cr3tbearervalue",
        "zerowidthsecret42",
        "MIIEvQIBADANBgkqhkiG9w0BAQEFAASCBKcwggSjAgEAAoIBAQC7",
        "b3RoZXJsaW5lb2ZiYXNlNjRrZXltYXRlcmlhbGhlcmU0Mg==",
        "hunter2hunter2",
        "\u{200B}",
    ] {
        assert!(!filed.contains(secret), "{secret} in {filed}");
        assert!(!v.to_string().contains(secret), "{secret} in {v}");
    }
    assert!(
        filed.contains("core: ~/.cache/waybar/core.4242\n"),
        "{filed}"
    );
    assert!(
        filed.contains("# Crash in ~/bin/tool with password=‹redacted›\n"),
        "{filed}"
    );
    assert!(filed.contains("split: token=‹redacted›\n"), "{filed}");
    // the key's label words stay, its body is one marker, and its line
    // breaks come after it (`redact_keeping_lines`, as `import task`)
    assert!(
        filed.contains(
            "\n-----BEGIN PRIVATE KEY-----‹redacted›-----END PRIVATE KEY-----\n\n\n\nrest is plain\n"
        ),
        "{filed}"
    );
    assert_eq!(v["privatePaths"], 2, "{v}");
    assert_eq!(v["droppedCharacters"], 1, "{v}");
    assert_eq!(v["tags"], serde_json::json!(["crash", "‹redacted›"]));
    assert!(
        filed.contains("\ntags: [crash, \"‹redacted›\"]\n"),
        "{filed}"
    );
    // env, cmd, split, the key's four lines, the title (its line 1 is not
    // the text's line 1)
    assert_eq!(v["redactedLines"], 8, "{v}");
    assert_eq!(
        v["path"],
        "inbox/2026-10-08-crash-in-bin-tool-with-password-redacted.md"
    );
    assert_eq!(v["title"], "Crash in ~/bin/tool with password=‹redacted›");
    // nor the git history
    if env.has_git {
        let log = git_out(&env, &root, &["log", "-p"]);
        assert!(
            !log.contains("hunter2hunter2") && !log.contains("ghp_0123"),
            "{log}"
        );
    }
}

#[test]
fn the_same_text_is_filed_once_and_another_under_a_taken_name_gets_a_number() {
    let env = Env::new(Snapper::Missing);
    let root = env.init_logbook();
    let first = add(&env, T0, "Crash: waybar", REPORT, &["--tag", "crash"]);
    assert_eq!(first["path"], "inbox/2026-10-08-crash-waybar.md");
    let snapshot = tree(&root);
    let count = env.has_git.then(|| commits(&env, &root));

    // the same title and text: nothing written, the file named — also on
    // another day, by another actor, with other tags, from a CRLF file
    for (now, more) in [
        (T0, &["--tag", "crash"][..]),
        (T1, &["--actor", "agent:codex"][..]),
    ] {
        let again = add(&env, now, "Crash: waybar", REPORT, more);
        assert_eq!(again["filed"], false, "{again}");
        assert_eq!(again["path"], "inbox/2026-10-08-crash-waybar.md");
        assert_eq!(again["git"]["committed"], false);
        assert_eq!(again["git"]["reason"], "nothing changed");
    }
    let crlf = env.tmp.path().join("crlf.md");
    std::fs::write(&crlf, REPORT.replace('\n', "\r\n")).unwrap();
    let out = env.at(
        T0,
        &[
            "inbox",
            "add",
            "--title",
            "Crash: waybar",
            "--file",
            crlf.to_str().unwrap(),
        ],
    );
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    assert!(
        stdout(&out)
            .starts_with("Already filed as inbox/2026-10-08-crash-waybar.md: Crash: waybar"),
        "{}",
        stdout(&out)
    );
    assert_eq!(tree(&root), snapshot);
    if let Some(count) = count {
        assert_eq!(commits(&env, &root), count);
    }

    // another text under the same title: a file of its own, numbered
    let second = add(&env, T0, "Crash: waybar", "It crashed again.\n", &[]);
    assert_eq!(second["filed"], true);
    assert_eq!(second["path"], "inbox/2026-10-08-crash-waybar-2.md");
    let third = add(&env, T0, "Crash: waybar", "And again.\n", &[]);
    assert_eq!(third["path"], "inbox/2026-10-08-crash-waybar-3.md");
    // the second text again: found under its numbered name
    let again = add(&env, T0, "Crash: waybar", "It crashed again.\n", &[]);
    assert_eq!(again["filed"], false);
    assert_eq!(again["path"], "inbox/2026-10-08-crash-waybar-2.md");

    // the first text again, now behind the numbered names
    let again = add(&env, T0, "Crash: waybar", REPORT, &[]);
    assert_eq!(again["filed"], false);
    assert_eq!(again["path"], "inbox/2026-10-08-crash-waybar.md");

    // the same text under another title is another filing
    let other = add(&env, T0, "Waybar crash", REPORT, &[]);
    assert_eq!(other["filed"], true);
    assert_eq!(other["path"], "inbox/2026-10-08-waybar-crash.md");

    // a name the user took (a file, a dangling link) is never written
    std::fs::write(
        root.join("inbox/2026-10-08-mine.md"),
        "# Mine\n\nhand-written\n",
    )
    .unwrap();
    std::os::unix::fs::symlink(
        env.tmp.path().join("nowhere.md"),
        root.join("inbox/2026-10-08-mine-2.md"),
    )
    .unwrap();
    let v = add(&env, T0, "Mine", "filed by the engine\n", &[]);
    assert_eq!(v["path"], "inbox/2026-10-08-mine-3.md");
    assert_eq!(
        read(&root.join("inbox/2026-10-08-mine.md")),
        "# Mine\n\nhand-written\n"
    );
    assert!(!env.tmp.path().join("nowhere.md").exists());

    let names: Vec<String> = inbox(&root).into_iter().map(|(n, _)| n).collect();
    assert_eq!(
        names,
        [
            "2026-10-08-crash-waybar-2.md",
            "2026-10-08-crash-waybar-3.md",
            "2026-10-08-crash-waybar.md",
            "2026-10-08-mine-2.md",
            "2026-10-08-mine-3.md",
            "2026-10-08-mine.md",
            "2026-10-08-waybar-crash.md",
        ]
    );
}

#[test]
fn a_title_without_letters_and_a_missing_inbox_folder() {
    let env = Env::new(Snapper::Missing);
    let root = env.init_logbook();
    std::fs::remove_dir_all(root.join("inbox")).unwrap();
    let v = add(&env, T0, "→ ✓ !", "text\n", &[]);
    assert_eq!(v["path"], "inbox/2026-10-08-note.md");
    assert!(root.join("inbox/2026-10-08-note.md").is_file());
    assert_eq!(mode(&root.join("inbox")) & 0o777, 0o700);
}

#[test]
fn what_is_refused_writes_nothing() {
    let env = Env::new(Snapper::Missing);
    // no logbook
    let out = run(
        &env,
        T0,
        &["inbox", "add", "--title", "t", "--file", "-"],
        Some("x"),
    );
    assert_eq!(out.status.code(), Some(3), "{}", stderr(&out));

    let root = env.init_logbook();
    let snapshot = tree(&root);
    let file = |name: &str, bytes: &[u8]| -> PathBuf {
        let p = env.tmp.path().join(name);
        std::fs::write(&p, bytes).unwrap();
        p
    };
    let good = file("good.md", b"text\n");
    let link = env.tmp.path().join("link.md");
    std::os::unix::fs::symlink(&good, &link).unwrap();
    let latin1 = file("latin1.md", b"caf\xe9\n");
    let big = file("big.md", &vec![b'a'; 1024 * 1024 + 1]);
    let missing = env.tmp.path().join("missing.md");
    let long = "x".repeat(121);
    let big_stdin = "a".repeat(1024 * 1024 + 1);
    let cases: Vec<(Vec<&str>, Option<&str>, &str)> = vec![
        (
            vec!["--title", "t", "--file", "-"],
            Some(""),
            "the text must not be empty",
        ),
        (
            vec!["--title", "t", "--file", "-"],
            Some(" \n\t\n"),
            "the text must not be empty",
        ),
        (
            vec!["--title", "t", "--file", "-"],
            Some("\u{200B}\n"),
            "the text must not be empty",
        ),
        (
            vec!["--title", "a\nb", "--file", "-"],
            Some("x"),
            "the title must be one line",
        ),
        (
            vec!["--title", "a\u{2028}b", "--file", "-"],
            Some("x"),
            "the title must be one line",
        ),
        (
            vec!["--title", " ", "--file", "-"],
            Some("x"),
            "the title must not be empty",
        ),
        (
            vec!["--title", "\u{200B}", "--file", "-"],
            Some("x"),
            "the title must not be empty",
        ),
        (
            vec!["--title", &long, "--file", "-"],
            Some("x"),
            "longer than 120 characters",
        ),
        (
            vec!["--title", "t", "--file", link.to_str().unwrap()],
            None,
            "a symbolic link",
        ),
        (
            vec!["--title", "t", "--file", latin1.to_str().unwrap()],
            None,
            "not UTF-8",
        ),
        (
            vec!["--title", "t", "--file", big.to_str().unwrap()],
            None,
            "more than 1048576",
        ),
        (
            vec!["--title", "t", "--file", missing.to_str().unwrap()],
            None,
            "no such file",
        ),
        (
            vec!["--title", "t", "--file", env.tmp.path().to_str().unwrap()],
            None,
            "a directory",
        ),
        (
            vec!["--title", "t", "--file", "-"],
            Some(&big_stdin),
            "larger than 1024 KiB",
        ),
        (
            vec!["--title", "t", "--file", "-", "--tag", "two words"],
            Some("x"),
            "is not a tag",
        ),
        (
            vec!["--title", "t", "--file", "-", "--actor", "system"],
            Some("x"),
            "cannot write",
        ),
    ];
    for (more, stdin, message) in cases {
        let mut args = vec!["inbox", "add"];
        args.extend(&more);
        let out = run(&env, T0, &args, stdin);
        assert_eq!(out.status.code(), Some(1), "{more:?}: {}", stderr(&out));
        assert!(stderr(&out).contains(message), "{more:?}: {}", stderr(&out));
    }
    assert_eq!(tree(&root), snapshot);

    // the lock held: exit 4, nothing written
    let _lock = seldon::logbook::lock::acquire(&env.lock_file()).unwrap();
    let out = run(
        &env,
        T0,
        &["inbox", "add", "--title", "t", "--file", "-"],
        Some("x"),
    );
    assert_eq!(out.status.code(), Some(4), "{}", stderr(&out));
    assert!(inbox(&root).is_empty());
}

#[test]
fn the_skill_s_recipe_files_the_report_verbatim_and_runs_none_of_it() {
    let Some(bash) = ["/usr/bin/bash", "/bin/bash"]
        .iter()
        .map(PathBuf::from)
        .find(|p| p.exists())
    else {
        eprintln!("skipped: bash not installed");
        return;
    };
    let env = Env::new(Snapper::Missing);
    let root = env.init_logbook();
    let skill = read(&Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/skills/seldon/SKILL.md"));
    let start = skill.find("```bash\nseldon inbox add").expect("the recipe") + "```bash\n".len();
    let recipe = &skill[start..start + skill[start..].find("```").unwrap()];
    let report = "## What crashed\n\n`$(touch MARK1)` and $(touch MARK2) and `touch MARK3`\n$HOME stays as written\n";
    let script = recipe
        .replace("<title>", "Crash: waybar")
        .replace("agent:<name>", "agent:pi")
        .replace("<your report>\n", report);
    for placeholder in ["<title>", "<name>", "<your report>"] {
        assert!(!script.contains(placeholder), "{script}");
    }
    let bin = env.tmp.path().join("recipe-bin");
    std::fs::create_dir(&bin).unwrap();
    std::os::unix::fs::symlink(env!("CARGO_BIN_EXE_seldon"), bin.join("seldon")).unwrap();
    let out = std::process::Command::new(&bash)
        .arg("-c")
        .arg(&script)
        .env_clear()
        .env("HOME", &env.home)
        .env("PATH", &bin)
        .env("LANG", "C")
        .env("SELDON_NOW", T0)
        .env("SELDON_TEST_GUARD", env.tmp.path())
        .env("SELDON_TEST_ROOT_PROBE", env.user_probe())
        .current_dir(env.tmp.path())
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    for mark in ["MARK1", "MARK2", "MARK3"] {
        assert!(
            !env.tmp.path().join(mark).exists(),
            "{mark}: part of the report ran"
        );
    }
    let filed = inbox(&root);
    assert_eq!(filed.len(), 1, "{filed:?}");
    assert_eq!(filed[0].0, "2026-10-08-crash-waybar.md");
    assert!(
        filed[0].1.ends_with(&format!(
            "actor: agent:pi\ntags: [crash]\n---\n# Crash: waybar\n\n{report}"
        )),
        "{}",
        filed[0].1
    );
}

#[test]
fn the_limits_hold_up_to_their_edge() {
    let env = Env::new(Snapper::Missing);
    let root = env.init_logbook();
    // a title of 120 characters and a text of 1 MiB on stdin are filed
    let title = "x".repeat(120);
    let v = add(&env, T0, &title, &"a".repeat(1024 * 1024), &[]);
    assert_eq!(v["title"], title.as_str());
    // Latin-1 on stdin is not
    let mut cmd = env.command(&["inbox", "add", "--title", "t", "--file", "-"]);
    cmd.env("SELDON_NOW", T0)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = cmd.spawn().unwrap();
    child.stdin.take().unwrap().write_all(b"caf\xe9\n").unwrap();
    let out = child.wait_with_output().unwrap();
    assert_eq!(out.status.code(), Some(1), "{}", stderr(&out));
    assert!(stderr(&out).contains("not UTF-8"), "{}", stderr(&out));

    // a title's name and 97 numbered ones taken: the 99th is the last
    let taken = |n: u32| match n {
        1 => root.join("inbox/2026-10-08-full.md"),
        n => root.join(format!("inbox/2026-10-08-full-{n}.md")),
    };
    for n in 1..=98 {
        std::fs::write(taken(n), format!("# Full\n\nhand-written {n}\n")).unwrap();
    }
    let v = add(&env, T0, "Full", "the 99th\n", &[]);
    assert_eq!(v["path"], "inbox/2026-10-08-full-99.md");
    let snapshot = tree(&root);
    let out = run(
        &env,
        T0,
        &["inbox", "add", "--title", "Full", "--file", "-"],
        Some("the 100th\n"),
    );
    assert_eq!(out.status.code(), Some(1), "{}", stderr(&out));
    assert!(
        stderr(&out).contains("inbox/2026-10-08-full.md and 98 more of that name are taken"),
        "{}",
        stderr(&out)
    );
    assert_eq!(tree(&root), snapshot);
}

#[test]
fn a_linked_or_odd_inbox_is_refused_and_nothing_written_through_it() {
    let env = Env::new(Snapper::Missing);
    let root = env.init_logbook();
    let outside = env.tmp.path().join("outside");
    std::fs::create_dir(&outside).unwrap();
    std::fs::remove_dir_all(root.join("inbox")).unwrap();
    std::os::unix::fs::symlink(&outside, root.join("inbox")).unwrap();
    let out = run(
        &env,
        T0,
        &["inbox", "add", "--title", "Link test", "--file", "-"],
        Some("hello crash\n"),
    );
    assert_eq!(out.status.code(), Some(1), "{}", stderr(&out));
    assert!(
        stderr(&out).contains("is a symbolic link"),
        "{}",
        stderr(&out)
    );
    assert_eq!(std::fs::read_dir(&outside).unwrap().count(), 0);

    std::fs::remove_file(root.join("inbox")).unwrap();
    std::fs::write(root.join("inbox"), "a file\n").unwrap();
    let out = run(
        &env,
        T0,
        &["inbox", "add", "--title", "File test", "--file", "-"],
        Some("hello crash\n"),
    );
    assert_eq!(out.status.code(), Some(1), "{}", stderr(&out));
    assert!(stderr(&out).contains("is no directory"), "{}", stderr(&out));
    assert_eq!(read(&root.join("inbox")), "a file\n");
}

#[test]
fn a_link_at_an_inbox_name_is_never_read_as_filed() {
    let env = Env::new(Snapper::Missing);
    let root = env.init_logbook();
    // a file outside with exactly what the filing would hold, linked in
    let outside = env.tmp.path().join("same.md");
    std::fs::write(
        &outside,
        format!(
            "---\ntype: inbox\ncreated: {T0}\nactor: human\ntags: []\n---\n# Linked\n\n{REPORT}"
        ),
    )
    .unwrap();
    std::os::unix::fs::symlink(&outside, root.join("inbox/linked.md")).unwrap();
    let v = add(&env, T0, "Linked", REPORT, &[]);
    assert_eq!(v["filed"], true, "{v}");
    assert_eq!(v["path"], "inbox/2026-10-08-linked.md");
}

#[test]
fn a_title_s_invisible_and_control_characters_are_dropped_and_counted() {
    let env = Env::new(Snapper::Missing);
    let root = env.init_logbook();
    // the text holds none: the count is the title's alone
    let v = add(
        &env,
        T0,
        "Crash\u{200B} in \u{1b}[31mwaybar\u{7}\u{9b}",
        "plain\n",
        &[],
    );
    assert_eq!(v["title"], "Crash in [31mwaybar", "{v}");
    assert_eq!(v["droppedCharacters"], 4, "{v}");
    let filed = read(&root.join(v["path"].as_str().unwrap()));
    assert!(filed.contains("\n# Crash in [31mwaybar\n"), "{filed}");
    // nor does an ESC reach the human line
    let out = run(
        &env,
        T0,
        &[
            "inbox",
            "add",
            "--title",
            "\u{1b}]0;x\u{7}Other",
            "--file",
            "-",
        ],
        Some("other\n"),
    );
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    // no control character but the final line break
    let line = stdout(&out);
    assert!(
        !line.trim_end_matches('\n').chars().any(char::is_control),
        "{line:?}"
    );
    assert!(stdout(&out).contains(": ]0;xOther"), "{}", stdout(&out));
    // a title of control characters only is empty
    let out = run(
        &env,
        T0,
        &["inbox", "add", "--title", "\u{1b}\u{7}", "--file", "-"],
        Some("x\n"),
    );
    assert_eq!(out.status.code(), Some(1), "{}", stderr(&out));
    assert!(
        stderr(&out).contains("the title must not be empty"),
        "{}",
        stderr(&out)
    );
}

#[test]
fn a_proc_view_is_not_filed() {
    let status = Path::new("/proc/self/status");
    if !status.exists() {
        eprintln!("skipped: no /proc");
        return;
    }
    let env = Env::new(Snapper::Missing);
    let root = env.init_logbook();
    let snapshot = tree(&root);
    for file in ["/proc/self/status", "/proc/self/environ"] {
        let out = run(
            &env,
            T0,
            &["inbox", "add", "--title", "t", "--file", file],
            None,
        );
        assert_eq!(out.status.code(), Some(1), "{file}: {}", stderr(&out));
        assert!(
            stderr(&out).contains("a file of size 0 that holds data"),
            "{file}: {}",
            stderr(&out)
        );
    }
    // an empty regular file is refused as empty text, not as a view
    let empty = env.tmp.path().join("empty.md");
    std::fs::write(&empty, "").unwrap();
    let out = env.at(
        T0,
        &[
            "inbox",
            "add",
            "--title",
            "t",
            "--file",
            empty.to_str().unwrap(),
        ],
    );
    assert_eq!(out.status.code(), Some(1), "{}", stderr(&out));
    assert!(
        stderr(&out).contains("the text must not be empty"),
        "{}",
        stderr(&out)
    );
    assert_eq!(tree(&root), snapshot);
}

#[test]
fn a_terminal_on_stdin_is_refused() {
    // util-linux `script` gives the command a pseudo-terminal as stdin
    let Some(script) = ["/usr/bin/script", "/bin/script"]
        .iter()
        .map(PathBuf::from)
        .find(|p| p.exists())
    else {
        eprintln!("skipped: script (util-linux) not installed");
        return;
    };
    let Some(sh) = ["/usr/bin/sh", "/bin/sh"]
        .iter()
        .map(PathBuf::from)
        .find(|p| p.exists())
    else {
        eprintln!("skipped: no sh");
        return;
    };
    let env = Env::new(Snapper::Missing);
    let root = env.init_logbook();
    let snapshot = tree(&root);
    let base = env.command(&[]);
    let mut cmd = std::process::Command::new(&script);
    cmd.env_clear();
    for (k, v) in base.get_envs() {
        if let Some(v) = v {
            cmd.env(k, v);
        }
    }
    let line = format!(
        "'{}' inbox add --title t --file -",
        env!("CARGO_BIN_EXE_seldon")
    );
    let out = cmd
        .env("SHELL", &sh)
        .env("SELDON_NOW", T0)
        .args(["-qec", &line, "/dev/null"])
        .current_dir(env.tmp.path())
        .stdin(Stdio::null())
        .output()
        .unwrap();
    let shown = format!("{}{}", stdout(&out), stderr(&out));
    assert_eq!(out.status.code(), Some(1), "{shown}");
    assert!(shown.contains("pipe the text on stdin"), "{shown}");
    assert_eq!(tree(&root), snapshot);
}
