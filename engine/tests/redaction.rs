//! Redaction (SPEC-ENGINE §7): one row per built-in pattern, user patterns,
//! the secret hook fixture, and the ledger path end to end. Every secret
//! here is made up.

mod support;

use seldon::collectors::pacman::Pacman;
use seldon::error::Exit;
use seldon::ledger::Ledger;
use seldon::logbook::lock;
use seldon::model::event::{Event, Kind, Meta, Source};
use seldon::redact::{BUILTIN, REDACTED, Redactor};
use support::{Bench, Scratch, fixture, ts};

/// (rule, input, secret that must disappear, text that must stay)
const TABLE: &[(&str, &str, &str, &str)] = &[
    (
        "password-option",
        "mysqldump --user root --password hunter2 db",
        "hunter2",
        "--password ‹redacted› db",
    ),
    (
        "password-option",
        "tool --password=\"two words\" --next",
        "two words",
        "--password=‹redacted› --next",
    ),
    (
        "token-assignment",
        "curl https://api.example.com/x?token=abc123&page=2",
        "abc123",
        "token=‹redacted›&page=2",
    ),
    (
        "token-assignment",
        "GITHUB_TOKEN=gho_secretvalue gh pr list",
        "gho_secretvalue",
        "GITHUB_TOKEN=‹redacted› gh pr list",
    ),
    (
        "authorization-header",
        "curl -H 'Authorization: Bearer eyJhbGciOi.x.y' https://example.com",
        "eyJhbGciOi",
        "'Authorization: ‹redacted›' https://example.com",
    ),
    (
        "aws-access-key",
        "AWS_ACCESS_KEY_ID=AKIAIOSFODNN7EXAMPLE aws s3 ls",
        "AKIAIOSFODNN7EXAMPLE",
        "aws s3 ls",
    ),
    (
        "github-token",
        "git clone x --header ghp_EXAMPLEexample0000000000000000000000",
        "ghp_EXAMPLE",
        "git clone x --header ‹redacted›",
    ),
    (
        "sk-key",
        "OPENAI_API_KEY=sk-EXAMPLE0000000000000000000000 run",
        "sk-EXAMPLE",
        "OPENAI_API_KEY=‹redacted› run",
    ),
    (
        "sk-key",
        "key sk-proj-EXAMPLE_0000000000000000-abc",
        "EXAMPLE_0000",
        "key ‹redacted›",
    ),
    (
        "db-client-password",
        "mysql -u root -p hunter2 shop",
        "hunter2",
        "mysql -u root -p ‹redacted›",
    ),
    (
        "db-client-password",
        "mysql -u root -phunter2 shop",
        "hunter2",
        "mysql -u root -p‹redacted›",
    ),
    (
        "db-client-password",
        "psql -h db -p 5432 -U app",
        "5432",
        "psql -h db -p ‹redacted›",
    ),
    (
        "db-client-password",
        "smbclient //nas/share -U user -p s3cret",
        "s3cret",
        "-p ‹redacted›",
    ),
    (
        "url-userinfo",
        "git push https://user:pass@github.com/example/x.git main",
        "user:pass",
        "https://‹redacted›@github.com/example/x.git main",
    ),
    (
        "url-userinfo",
        "pacman -U https://me:pw@mirror.example/foo-1-1-x86_64.pkg.tar.zst",
        "me:pw",
        "https://‹redacted›@mirror.example/",
    ),
    (
        "url-userinfo",
        "curl https://user:p@ss@h.example/path?q=1",
        "ss@h",
        "curl https://‹redacted›@h.example/path?q=1",
    ),
    (
        "url-userinfo",
        "git clone ssh://git:se@cr@t@host.example:22/repo.git",
        "se@cr",
        "ssh://‹redacted›@host.example:22/repo.git",
    ),
    // a password with `/`, `?`, `#` or `:`
    (
        "url-userinfo",
        "curl https://bob:fake/pw1@example.invalid/r -o r",
        "fake/pw1",
        "curl https://‹redacted›@example.invalid/r -o r",
    ),
    (
        "url-userinfo",
        "git clone https://bob:fake?pw2@git.example/x.git",
        "fake?pw2",
        "https://‹redacted›@git.example/x.git",
    ),
    (
        "url-userinfo",
        "git clone https://bob:fake#pw3@git.example/x.git",
        "fake#pw3",
        "https://‹redacted›@git.example/x.git",
    ),
    (
        "url-userinfo",
        "git clone https://bob:fake:pw4@git.example/x.git",
        "fake:pw4",
        "https://‹redacted›@git.example/x.git",
    ),
    (
        "url-userinfo",
        "git clone https://faketoken0000@git.example/x.git",
        "faketoken0000",
        "git clone https://‹redacted›@git.example/x.git",
    ),
    (
        "secret-option",
        "gh auth login --with-token fakeTokenValue1",
        "fakeTokenValue1",
        "--with-token ‹redacted›",
    ),
    (
        "secret-option",
        "tool --api-key=fakeKeyValue2 --verbose",
        "fakeKeyValue2",
        "--api-key=‹redacted› --verbose",
    ),
    (
        "secret-option",
        "deploy --token 'fake token 3' --env prod",
        "fake token 3",
        "--token ‹redacted› --env prod",
    ),
    (
        "secret-option",
        "az login --client-secret fakeSecret4",
        "fakeSecret4",
        "--client-secret ‹redacted›",
    ),
    (
        "secret-assignment",
        "export API_KEY=fakeKey5",
        "fakeKey5",
        "export API_KEY=‹redacted›",
    ),
    (
        "secret-assignment",
        "PASSWORD=fakePw6 ./run.sh",
        "fakePw6",
        "PASSWORD=‹redacted› ./run.sh",
    ),
    (
        "secret-assignment",
        "PGPASSWORD=fakePw7 psql -h db -U app",
        "fakePw7",
        "PGPASSWORD=‹redacted› psql -h db -U app",
    ),
    (
        "secret-assignment",
        "MYSQL_PWD=fakePw8 mysqldump shop",
        "fakePw8",
        "MYSQL_PWD=‹redacted› mysqldump shop",
    ),
    (
        "secret-assignment",
        "curl 'https://api.example/v1?api_key=fakeKey9&page=2'",
        "fakeKey9",
        "api_key=‹redacted›&page=2",
    ),
    (
        "secret-header",
        "curl -H 'X-Api-Key: fakeKey10' https://api.example",
        "fakeKey10",
        "'X-Api-Key: ‹redacted›' https://api.example",
    ),
    (
        "secret-header",
        "curl --header \"PRIVATE-TOKEN: fakeToken11\" https://git.example/api",
        "fakeToken11",
        "\"PRIVATE-TOKEN: ‹redacted›\" https://git.example/api",
    ),
    (
        "aws-access-key",
        "AWS_ACCESS_KEY_ID=ASIAIOSFODNN7EXAMPLE aws sts get-caller-identity",
        "ASIAIOSFODNN7EXAMPLE",
        "aws sts get-caller-identity",
    ),
    // prefixed token forms are spelled in parts, so no source line holds one
    (
        "github-token",
        concat!(
            "token ",
            "gho",
            "_FAKEfakeFAKEfakeFAKEfakeFAKEfake0000 used"
        ),
        "FAKEfake",
        "token ‹redacted› used",
    ),
    (
        "github-token",
        concat!("export GH=", "github", "_pat_FAKE0fake0FAKE0fake0FAKE_0"),
        "FAKE0fake",
        "export GH=‹redacted›",
    ),
    (
        "github-token",
        concat!("a ", "ghp", "_FAKEfakeFAKEfakeFAKEfakeFAKEfake00001234 b"),
        "1234",
        "a ‹redacted› b",
    ),
    (
        "gitlab-token",
        concat!("glab auth login --stdin ", "gl", "pat-FAKEfakeFAKEfake0000"),
        "FAKEfake",
        "glab auth login --stdin ‹redacted›",
    ),
    (
        "slack-token",
        concat!("SLACK=", "xo", "xb-0000-FAKEfake"),
        "FAKEfake",
        "SLACK=‹redacted›",
    ),
    (
        "sk-key",
        concat!("STRIPE=", "sk", "_live_FAKE0000fake0000x"),
        "FAKE0000",
        "STRIPE=‹redacted›",
    ),
    (
        "curl-user",
        "curl -u admin:fakePw12 https://h.example/x",
        "fakePw12",
        "curl -u ‹redacted› https://h.example/x",
    ),
    (
        "curl-user",
        "curl -sS --user=admin:fakePw13 https://h.example",
        "fakePw13",
        "--user=‹redacted› https://h.example",
    ),
    (
        "curl-user",
        "curl -uadmin:fakePw14 https://h.example",
        "fakePw14",
        "curl -u‹redacted› https://h.example",
    ),
    (
        "sshpass-password",
        "sshpass -p fakePw15 ssh me@host.example",
        "fakePw15",
        "sshpass -p ‹redacted› ssh me@host.example",
    ),
    (
        "registry-login-password",
        "docker login -u me -p fakePw16 registry.example",
        "fakePw16",
        "docker login -u me -p ‹redacted› registry.example",
    ),
    (
        "registry-login-password",
        "podman login -pfakePw17 quay.example",
        "fakePw17",
        "podman login -p‹redacted› quay.example",
    ),
];

/// Text that looks close to a rule and must come out unchanged.
const CLEAR: &[&str] = &[
    "pacman -Syu --noconfirm --overwrite /usr/share/omarchy/*",
    "yay -S --noconfirm zed",
    "git -C ~/.config/hypr status --short",
    "sed -i 's/^bindd = SUPER, E, Editor, exec, .*/bindd = SUPER, E, Editor, exec, zeditor/' ~/.config/hypr/bindings.conf",
    "omarchy update",
    "skip-this sk-short",
    "Logbuch angelegt. Theme osaka-jade, 39 Plugins.",
    "yay -S python-task-manager-application",
    "docker run -p 8080:80 nginx",
    "ssh -p 2222 me@host.example",
    "sort -u names.txt",
    "curl --user-agent seldon https://example.com",
    "curl https://example.com:8443/path?q=1#top",
    "open https://blog.example/@someone/post",
    "PWD=/tmp OLDPWD=/var ls",
    "app --token-file ~/.config/app/token.txt",
    "docker login --password-stdin -u me registry.example",
    "mysql -u root shop",
    "Rotated the deploy key today",
    "XKB_DEFAULT_LAYOUT=de EDITOR=nvim zeditor",
];

mod redaction {
    use super::*;

    #[test]
    fn every_builtin_pattern() {
        let r = Redactor::builtin();
        for (rule, input, secret, kept) in TABLE {
            let out = r.redact(input);
            assert!(!out.contains(secret), "{rule}: `{input}` → `{out}`");
            assert!(
                out.contains(kept),
                "{rule}: `{input}` → `{out}`, expected `{kept}`"
            );
            assert!(
                r.matching_rules(input).contains(rule),
                "{rule} did not match `{input}`"
            );
        }
        // the table covers every built-in rule
        for rule in BUILTIN {
            assert!(TABLE.iter().any(|(r, ..)| *r == rule), "no row for {rule}");
        }
    }

    #[test]
    fn harmless_text_stays() {
        let r = Redactor::builtin();
        for text in CLEAR {
            assert_eq!(r.redact(text), *text, "{:?}", r.matching_rules(text));
        }
    }

    #[test]
    fn masking_twice_changes_nothing() {
        // a user pattern that also matches inside the marker itself
        let r = Redactor::with_patterns(&["red|act".into()]).unwrap();
        let mut texts: Vec<String> = TABLE
            .iter()
            .map(|(_, input, ..)| input.to_string())
            .collect();
        texts.push(
            TABLE
                .iter()
                .map(|(_, input, ..)| *input)
                .collect::<Vec<_>>()
                .join("; "),
        );
        texts.push(
            TABLE
                .iter()
                .map(|(_, input, ..)| *input)
                .collect::<Vec<_>>()
                .join("\n"),
        );
        texts.push("a red car, token=fakeValue and an act".into());
        for text in &texts {
            let once = r.redact(text);
            assert_eq!(r.redact(&once), once, "`{text}`");
        }
        assert_eq!(
            r.redact("a red car, token=fakeValue"),
            format!("a {REDACTED} car, token={REDACTED}")
        );
    }

    #[test]
    fn user_patterns() {
        let r = Redactor::with_patterns(&["corp-[0-9]{6}".into(), "(?i)internal\\.example".into()])
            .unwrap();
        assert_eq!(
            r.redact("ssh corp-123456@INTERNAL.example"),
            format!("ssh {REDACTED}@{REDACTED}")
        );
        let err = Redactor::with_patterns(&["(unclosed".into()]).unwrap_err();
        assert_eq!(err.exit(), Exit::UserError);
        assert!(err.to_string().contains("(unclosed"));
    }

    #[test]
    fn secret_hook_fixture_leaves_nothing() {
        let payload: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(fixture("hooks/claude-code-secret.json")).unwrap(),
        )
        .unwrap();
        let command = payload["tool_input"]["command"].as_str().unwrap();
        let out = Redactor::builtin().redact(command);
        for secret in ["AKIAIOSFODNN7EXAMPLE", "ghp_EXAMPLE", "user:", "hunter2"] {
            assert!(!out.contains(secret), "{secret} in `{out}`");
        }
        assert!(out.contains(REDACTED));
        assert!(out.contains("git -C ~/.config/hypr push"));
    }

    #[test]
    fn ledger_redacts_detail_and_command() {
        let s = Scratch::new("redact-ledger");
        let ledger = Ledger::at(
            s.path("ledger"),
            Redactor::with_patterns(&["hunter[0-9]".into()]).unwrap(),
        );
        let lock = lock::acquire(&s.path("lock")).unwrap();
        let e = Event::new(
            ts("2026-10-01T10:00:00+02:00"),
            Source::Agent,
            Kind::Command,
            "curl",
        )
        .actor("agent:codex")
        .detail("token=abc123 and hunter7")
        .meta(Meta {
            command: Some("curl -H 'Authorization: Bearer xyz' https://u:p@h.example".into()),
            ..Meta::default()
        });
        let written = ledger.append(&lock, vec![e]).unwrap();
        let text = std::fs::read_to_string(ledger.month_file("2026-10")).unwrap();
        for secret in ["abc123", "hunter7", "Bearer xyz", "u:p@"] {
            assert!(
                !text.contains(secret),
                "{secret} reached the ledger: {text}"
            );
        }
        assert_eq!(
            written[0].detail.as_deref(),
            Some("token=‹redacted› and ‹redacted›")
        );
    }

    #[test]
    fn collected_command_lines_are_redacted() {
        let mut b = Bench::new("redact-collector");
        let log = b.scratch.path("pkg.log");
        b.sources.pacman_log = log.clone();
        std::fs::write(
            &log,
            "[2026-10-01T10:00:00+0200] [PACMAN] Running 'pacman -U https://me:pw@mirror.example/foo-1-1-x86_64.pkg.tar.zst'\n\
             [2026-10-01T10:00:01+0200] [ALPM] transaction started\n\
             [2026-10-01T10:00:02+0200] [ALPM] installed foo (1-1)\n\
             [2026-10-01T10:00:02+0200] [ALPM] transaction completed\n",
        )
        .unwrap();
        b.baseline = ts("2026-10-01T00:00:00+02:00");
        let out = b.run(&Pacman, "2026-10-01T11:00:00+02:00");
        assert_eq!(out.events.len(), 1);
        assert_eq!(out.events[0].explicit, Some(true), "the URL names foo");
        assert_eq!(
            out.events[0].meta.command.as_deref(),
            Some("pacman -U https://‹redacted›@mirror.example/foo-1-1-x86_64.pkg.tar.zst")
        );
    }
}

