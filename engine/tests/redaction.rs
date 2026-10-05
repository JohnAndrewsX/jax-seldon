//! Redaction (SPEC-ENGINE §7): one row per built-in pattern, user patterns,
//! the secret hook fixture, the ledger path end to end, and every command
//! that writes free text into the logbook. Every secret here is made up.

mod common;
mod support;

use seldon::collectors::pacman::Pacman;
use seldon::error::Exit;
use seldon::ledger::Ledger;
use seldon::logbook::lock;
use seldon::model::event::{Event, Kind, Meta, Source};
use seldon::redact::{BUILTIN, REDACTED, Redactor, trigger_text, triggers};
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
        "key-option",
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
        "key-assignment",
        "export API_KEY=fakeKey5",
        "fakeKey5",
        "export API_KEY=‹redacted›",
    ),
    (
        "secret-assignment",
        "PASSWORD=fakePass6 ./run.sh",
        "fakePass6",
        "PASSWORD=‹redacted› ./run.sh",
    ),
    (
        "secret-assignment",
        "PGPASSWORD=fakePass7 psql -h db -U app",
        "fakePass7",
        "PGPASSWORD=‹redacted› psql -h db -U app",
    ),
    (
        "secret-assignment",
        "MYSQL_PWD=fakePass8 mysqldump shop",
        "fakePass8",
        "MYSQL_PWD=‹redacted› mysqldump shop",
    ),
    (
        "key-assignment",
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
    // a name that can only mean a credential masks any non-empty value
    (
        "secret-option",
        "tool --token short",
        "short",
        "tool --token ‹redacted›",
    ),
    (
        "secret-option",
        "tool --token abc --verbose",
        "abc",
        "--token ‹redacted› --verbose",
    ),
    (
        "secret-option",
        "tool --secret hunter2",
        "hunter2",
        "tool --secret ‹redacted›",
    ),
    (
        "secret-option",
        "gpg --passphrase abc --batch",
        "abc",
        "--passphrase ‹redacted› --batch",
    ),
    (
        "secret-assignment",
        "PASSWORD=x ./run.sh",
        "=x",
        "PASSWORD=‹redacted› ./run.sh",
    ),
    (
        "secret-assignment",
        "PASSWORD=hunter2 ./run.sh",
        "hunter2",
        "PASSWORD=‹redacted› ./run.sh",
    ),
    (
        "secret-assignment",
        "passwd=abc",
        "abc",
        "passwd=‹redacted›",
    ),
    ("secret-assignment", "secret=x1", "x1", "secret=‹redacted›"),
    (
        "secret-assignment",
        "DB_PASS=short ./app",
        "short",
        "DB_PASS=‹redacted› ./app",
    ),
    (
        "secret-assignment",
        "MYSQL_PWD=pw mysql shop",
        "=pw",
        "MYSQL_PWD=‹redacted› mysql shop",
    ),
    (
        "secret-assignment",
        "PGPASSWORD=pw psql -h db",
        "=pw",
        "PGPASSWORD=‹redacted› psql -h db",
    ),
    (
        "secret-assignment",
        "SSHPASS=pw sshpass -e ssh host",
        "=pw",
        "SSHPASS=‹redacted› sshpass -e ssh host",
    ),
    (
        "secret-assignment",
        "PASSWORD=\"hunter2\" ./run.sh",
        "hunter2",
        "PASSWORD=‹redacted› ./run.sh",
    ),
    (
        "secret-assignment",
        "export SECRET='a b c'",
        "a b c",
        "export SECRET=‹redacted›",
    ),
    // proxy credentials (WP-084)
    (
        "proxy-option",
        "curl -U bob:fakeProxyPw1 -x proxy.example:3128 https://h.example",
        "fakeProxyPw1",
        "curl -U ‹redacted› -x proxy.example:3128 https://h.example",
    ),
    (
        "proxy-option",
        "curl -sS -Ubob:fakeProxyPw2 https://h.example",
        "fakeProxyPw2",
        "-U‹redacted› https://h.example",
    ),
    (
        "proxy-option",
        "curl --proxy-user 'bob:fake proxy 3' https://h.example",
        "fake proxy 3",
        "curl --proxy-user ‹redacted› https://h.example",
    ),
    (
        "proxy-option",
        "wget --proxy-user=bob --proxy-password fakeProxyPw4 https://h.example/f",
        "fakeProxyPw4",
        "--proxy-user=‹redacted› --proxy-password ‹redacted› https://h.example/f",
    ),
    (
        "proxy-userinfo",
        "curl -x bob:fakeProxyPw5@proxy.example:3128 https://h.example",
        "fakeProxyPw5",
        "curl -x ‹redacted›@proxy.example:3128 https://h.example",
    ),
    (
        "proxy-userinfo",
        "curl --proxy 'bob:fake@pw6@proxy.example:3128' https://h.example",
        "fake@pw6",
        "--proxy '‹redacted›@proxy.example:3128' https://h.example",
    ),
    (
        "proxy-userinfo",
        "HTTPS_PROXY=bob:fakeProxyPw7@proxy.example:3128 wget https://h.example/f",
        "fakeProxyPw7",
        "HTTPS_PROXY=‹redacted›@proxy.example:3128 wget",
    ),
    (
        "proxy-userinfo",
        "git -c http.proxy=bob:fake/pw8@proxy.example:3128 fetch",
        "fake/pw8",
        "http.proxy=‹redacted›@proxy.example:3128 fetch",
    ),
    (
        "secret-assignment",
        "wget --proxy-password=fakeProxyPw10 https://h.example/f",
        "fakeProxyPw10",
        "--proxy-password=‹redacted› https://h.example/f",
    ),
    // with a scheme the proxy URL is an ordinary URL with userinfo
    (
        "url-userinfo",
        "curl --proxy http://bob:fakeProxyPw9@proxy.example:3128 https://h.example",
        "fakeProxyPw9",
        "--proxy http://‹redacted›@proxy.example:3128 https://h.example",
    ),
    // passwords in inline JSON (WP-084)
    (
        "json-secret",
        r#"curl -d '{"user": "bob", "password": "fakeJsonPw1"}' https://h.example"#,
        "fakeJsonPw1",
        r#"{"user": "bob", "password": ‹redacted›}' https://h.example"#,
    ),
    (
        "json-secret",
        r#"curl --data '{"passwd":"fake \"pw\" 2","n":1}' https://h.example"#,
        "pw\\\" 2",
        r#"{"passwd":‹redacted›,"n":1}'"#,
    ),
    (
        "json-secret",
        r#"http POST h.example/login <<< '{"client_secret": "fakeJson3"}'"#,
        "fakeJson3",
        r#"{"client_secret": ‹redacted›}'"#,
    ),
    (
        "json-secret",
        r#"curl -d "{\"access_token\":\"fakeJson4\",\"x\":1}" https://h.example"#,
        "fakeJson4",
        r#""{\"access_token\":‹redacted›,\"x\":1}""#,
    ),
    // the trigger `password"` holds for the second key only
    (
        "json-secret",
        r#"{"password_hint": "first pet", "password": "fakeJson6"}"#,
        "fakeJson6",
        r#"{"password_hint": "first pet", "password": ‹redacted›}"#,
    ),
    (
        "json-secret",
        r#"{"Token" : "fakeJson5"}"#,
        "fakeJson5",
        r#"{"Token" : ‹redacted›}"#,
    ),
    // cookie headers and options (WP-084)
    (
        "cookie-header",
        "curl -H 'Cookie: session=fakeCookie1; theme=dark' https://h.example",
        "fakeCookie1",
        "-H 'Cookie: ‹redacted›' https://h.example",
    ),
    (
        "cookie-header",
        "wget --header=\"Set-Cookie: id=fakeCookie2; Path=/\" https://h.example",
        "fakeCookie2",
        "--header=\"Set-Cookie: ‹redacted›\" https://h.example",
    ),
    (
        "cookie-header",
        "curl --header 'cookie:sid=fakeCookie3' https://h.example",
        "fakeCookie3",
        "--header 'cookie:‹redacted›' https://h.example",
    ),
    (
        "cookie-option",
        "curl -b 'session=fakeCookie4; theme=dark' https://h.example",
        "fakeCookie4",
        "curl -b ‹redacted› https://h.example",
    ),
    (
        "cookie-option",
        "curl -s --cookie=sid=fakeCookie5 https://h.example",
        "fakeCookie5",
        "--cookie=‹redacted› https://h.example",
    ),
    (
        "cookie-option",
        "curl -bsid=fakeCookie6 https://h.example",
        "fakeCookie6",
        "curl -b‹redacted› https://h.example",
    ),
    // case-insensitive matching folds the Kelvin sign onto `k` and the
    // long s onto `s`; the triggers do the same
    (
        "token-assignment",
        "to\u{212A}en=fakeValue1",
        "fakeValue1",
        "to\u{212A}en=‹redacted›",
    ),
    (
        "key-assignment",
        "API_\u{212A}EY=fakeKey55",
        "fakeKey55",
        "API_\u{212A}EY=‹redacted›",
    ),
    (
        "secret-assignment",
        "PA\u{17F}\u{17F}WORD=pw",
        "=pw",
        "PA\u{17F}\u{17F}WORD=‹redacted›",
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
    "yay -S python-task-manager-application-git",
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
    "the key=value pairs",
    "sort --key=2 names.txt",
    "hotkey=Super",
    "tool --api-key=auto",
    "curl -H 'X-Author: me' https://example.com",
    // close to the WP-084 rules
    "useradd -U -m bob",
    "sudo useradd -U bob && curl https://h.example",
    "curl -x proxy.example:3128 https://h.example",
    "curl -x me@proxy.example:3128 https://h.example",
    "git -c http.proxy=http://proxy.example:3128 fetch",
    r#"curl -d '{"password_hint": "first pet"}' https://h.example"#,
    r#"curl -d '{"token_type": "bearer", "secrets": 2}' https://h.example"#,
    r#"curl -d '{"password": ""}' https://h.example"#,
    r#"curl -d "{\"password\":\"\"}" https://h.example"#,
    "curl -H 'Cookie: ' https://h.example",
    "curl -H 'Cookie:' https://h.example",
    "curl -b cookies.txt -c cookies.txt https://h.example",
    "curl --cookie-jar jar.txt https://h.example",
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
    fn every_row_holds_a_trigger_of_its_rule() {
        for rule in BUILTIN {
            assert!(!triggers(rule).is_empty(), "{rule} has no trigger");
        }
        for (rule, input, ..) in TABLE {
            let lower = trigger_text(input);
            assert!(
                triggers(rule).iter().any(|t| lower.contains(t)),
                "{rule}: no trigger in `{input}`"
            );
        }
        // the marker can never trigger a rule
        let marker = trigger_text(REDACTED);
        for rule in BUILTIN {
            assert!(!triggers(rule).iter().any(|t| marker.contains(t)), "{rule}");
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
    fn a_value_must_look_like_a_credential() {
        use seldon::redact::looks_like_credential;
        for (value, expected) in [
            ("fakePw7", false),         // 7 characters, mixed
            ("fakePw78", true),         // 8, mixed
            ("lowercaseonly", false),   // 13, one class
            ("lowercaseonlyabc", true), // 16
            ("12345678", false),        // 8, one class
            ("'fake pw 9'", true),      // quotes do not count
            ("\"\"", false),
        ] {
            assert_eq!(looks_like_credential(value), expected, "{value}");
        }
        let r = Redactor::builtin();
        assert_eq!(r.redact("API_KEY=fakePw7"), "API_KEY=fakePw7");
        assert_eq!(r.redact("API_KEY=fakePw78"), format!("API_KEY={REDACTED}"));
        assert!(r.matching_rules("hotkey=Super").is_empty());
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

    /// The WP-084 rules match only their own rows and no row of an older
    /// rule (the import report counts a line once per rule), and a second
    /// pass changes nothing.
    #[test]
    fn proxy_json_and_cookie_rules_are_disjoint_and_stable() {
        const NEW: [&str; 5] = [
            "proxy-option",
            "proxy-userinfo",
            "json-secret",
            "cookie-header",
            "cookie-option",
        ];
        let r = Redactor::builtin();
        let rows = TABLE
            .iter()
            .filter(|(rule, input, ..)| NEW.contains(rule) || input.contains("--proxy http"));
        for (rule, input, ..) in rows {
            assert_eq!(r.matching_rules(input), vec![*rule], "`{input}`");
            let once = r.redact(input);
            assert_eq!(r.redact(&once), once, "`{input}`");
        }
        for (rule, input, ..) in TABLE.iter().filter(|(rule, ..)| !NEW.contains(rule)) {
            let matched = r.matching_rules(input);
            assert!(
                !matched.iter().any(|m| NEW.contains(m)),
                "{rule}: `{input}` also matches {matched:?}"
            );
        }
        assert_eq!(
            r.redact(r#"curl -U bob:pw -d '{"token":"t1"}' -b 'a=b' -H 'Cookie: c=d' https://h"#),
            format!(
                r#"curl -U {REDACTED} -d '{{"token":{REDACTED}}}' -b {REDACTED} -H 'Cookie: {REDACTED}' https://h"#
            )
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
    fn ledger_redacts_the_subject_and_every_meta_value() {
        let s = Scratch::new("redact-ledger-fields");
        let ledger = Ledger::at(s.path("ledger"), Redactor::builtin());
        let lock = lock::acquire(&s.path("lock")).unwrap();
        let mut meta = Meta {
            version: Some("token=fakeVersion".into()),
            from: Some("https://u:fakeFrom@h.example/a".into()),
            to: Some("API_KEY=fakeToValue1".into()),
            ..Meta::default()
        };
        meta.extra.insert(
            "url".into(),
            serde_json::json!("https://user:fakeUrl@h.example/x"),
        );
        meta.extra
            .insert("other".into(), serde_json::json!("PASSWORD=fakeOther"));
        meta.extra.insert("count".into(), serde_json::json!(3));
        let e = Event::new(
            ts("2026-10-01T10:00:00+02:00"),
            Source::Manual,
            Kind::Note,
            "deploy token=fakeSubject",
        )
        .meta(meta);
        // a subject at the limit that grows when it is redacted
        let long = format!("{}token=x", "a".repeat(505));
        let e2 = Event::new(
            ts("2026-10-01T10:00:01+02:00"),
            Source::Manual,
            Kind::Note,
            &long,
        );
        let written = ledger.append(&lock, vec![e, e2]).unwrap();
        let text = std::fs::read_to_string(ledger.month_file("2026-10")).unwrap();
        for secret in [
            "fakeVersion",
            "fakeFrom",
            "fakeTo",
            "fakeUrl",
            "fakeOther",
            "fakeSubject",
            "token=x",
        ] {
            assert!(
                !text.contains(secret),
                "{secret} reached the ledger: {text}"
            );
        }
        assert_eq!(written[0].subject, format!("deploy token={REDACTED}"));
        assert_eq!(
            written[0].meta.extra["url"],
            format!("https://{REDACTED}@h.example/x")
        );
        assert_eq!(written[0].meta.extra["count"], 3);
        assert_eq!(written[1].subject.chars().count(), 512);
        assert!(written[1].subject.ends_with('…'));
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

/// Every command that writes free text into the logbook redacts it before
/// the first write: the ledger, the journal, case and decision files,
/// `STATUS.md`, the index and the git history hold the masked text only.
mod commands {
    use std::path::{Path, PathBuf};

    use serde_json::Value;

    use super::REDACTED;
    use super::common::{Env, Snapper, copy_dir, find_file, fixture_logbook, json, read, stderr};

    const T0: &str = "2026-10-03T10:00:00+02:00";

    /// A made-up value of the documented `ghp_` form (36 characters).
    fn token(tag: &str) -> String {
        format!("ghp_{tag}{}", "0".repeat(36 - tag.len()))
    }

    fn run(env: &Env, args: &[&str]) -> Value {
        let mut all = vec!["--json"];
        all.extend_from_slice(args);
        let out = env.at(T0, &all);
        assert_eq!(out.status.code(), Some(0), "{args:?}: {}", stderr(&out));
        json(&out)
    }

    fn files(dir: &Path, out: &mut Vec<PathBuf>) {
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.file_name().is_some_and(|n| n == ".git") {
                continue;
            }
            if path.is_dir() {
                files(&path, out);
            } else {
                out.push(path);
            }
        }
    }

    /// `seldon status`, then: no file of the logbook or the state
    /// directory and no commit holds any of `secrets`.
    fn assert_nowhere(env: &Env, logbook: &Path, secrets: &[&str]) {
        run(env, &["--logbook", logbook.to_str().unwrap(), "status"]);
        assert!(logbook.join("STATUS.md").is_file());
        let mut all = Vec::new();
        files(logbook, &mut all);
        files(&env.home.join(".local/state/seldon"), &mut all);
        for path in &all {
            let text = String::from_utf8_lossy(&std::fs::read(path).unwrap()).into_owned();
            for secret in secrets {
                assert!(
                    !text.contains(secret) && !path.to_string_lossy().contains(secret),
                    "{secret} in {}",
                    path.display()
                );
            }
        }
        if env.has_git && logbook.join(".git").exists() {
            let out = env.git(logbook, &["log", "-p", "--all"]);
            let history = String::from_utf8_lossy(&out.stdout);
            assert!(!history.is_empty());
            for secret in secrets {
                assert!(!history.contains(secret), "{secret} in the git history");
            }
        }
    }

    fn last_ledger_line(logbook: &Path) -> Value {
        let text = read(&logbook.join("ledger/2026-10.jsonl"));
        serde_json::from_str(text.lines().last().unwrap()).unwrap()
    }

    #[test]
    fn log_masks_the_note_in_the_journal_and_the_ledger() {
        let env = Env::new(Snapper::Missing);
        let root = env.init_logbook();
        let secret = token("LOG");
        let v = run(
            &env,
            &[
                "log",
                "--",
                &format!("rotated deploy key, old token {secret}"),
            ],
        );
        let masked = format!("rotated deploy key, old token {REDACTED}");
        assert_eq!(v["event"]["detail"], masked.as_str());
        let day = read(&root.join("journal/2026/2026-10-03.md"));
        assert!(day.contains(&masked), "{day}");
        assert_nowhere(&env, &root, &[&secret]);
    }

    #[test]
    fn log_masks_a_tag_in_the_journal_and_the_ledger() {
        let env = Env::new(Snapper::Missing);
        let root = env.init_logbook();
        let secret = token("TAG");
        let v = run(
            &env,
            &["log", "--tag", &secret, "--tag", "keys", "--", "rotated"],
        );
        assert_eq!(
            v["event"]["meta"]["tags"],
            format!("{REDACTED},keys").as_str()
        );
        let day = read(&root.join("journal/2026/2026-10-03.md"));
        assert!(day.contains(&format!("#{REDACTED} #keys")), "{day}");
        assert_nowhere(&env, &root, &[&secret]);
    }

    #[test]
    fn plan_masks_the_title_and_the_step_reasons() {
        let env = Env::new(Snapper::Missing);
        let root = env.init_logbook();
        let title = token("TITLE");
        let reason = token("REASON");
        let v = run(&env, &["plan", "new", "--", &format!("Rotate {title}")]);
        let id = v["case"]["id"].as_str().unwrap().to_string();
        assert_eq!(v["case"]["title"], format!("Rotate {REDACTED}").as_str());
        let queued = read(&find_file(&root.join("work/queued"), &id));
        assert!(
            queued.contains(&format!("# {id} — Rotate {REDACTED}")),
            "{queued}"
        );
        run(&env, &["status"]);
        let status = read(&root.join("STATUS.md"));
        assert!(
            status.contains(&format!("[[{id}]] Rotate {REDACTED}")),
            "{status}"
        );
        run(
            &env,
            &["plan", "start", &id, "--reason", &format!("with {reason}")],
        );
        run(&env, &["plan", "verify", &id]);
        run(&env, &["plan", "done", &id]);
        let done = read(&find_file(&root.join("work/completed"), &id));
        assert!(done.contains(&format!("with {REDACTED}")), "{done}");
        let journal = read(&root.join("journal/2026/2026-10-03.md"));
        assert!(
            journal.contains(&format!("Case completed: Rotate {REDACTED}")),
            "{journal}"
        );
        assert_nowhere(&env, &root, &[&title, &reason]);
    }

    #[test]
    fn plan_done_masks_a_title_edited_by_hand_in_the_journal() {
        let env = Env::new(Snapper::Missing);
        let root = env.init_logbook();
        let secret = token("HAND");
        let v = run(&env, &["plan", "new", "--", "Rotate the key"]);
        let id = v["case"]["id"].as_str().unwrap().to_string();
        let path = find_file(&root.join("work/queued"), &id);
        let text = read(&path).replace("Rotate the key", &format!("Rotate {secret}"));
        std::fs::write(&path, text).unwrap();
        run(&env, &["plan", "start", &id]);
        run(&env, &["plan", "verify", &id]);
        run(&env, &["plan", "done", &id]);
        let journal = read(&root.join("journal/2026/2026-10-03.md"));
        assert!(
            journal.contains(&format!("Case completed: Rotate {REDACTED}")),
            "{journal}"
        );
    }

    #[test]
    fn decide_masks_the_title() {
        let env = Env::new(Snapper::Missing);
        let root = env.init_logbook();
        let secret = token("ADR");
        let v = run(
            &env,
            &["decide", "--no-edit", "--", &format!("Use {secret} for CI")],
        );
        let path = root.join(v["decision"]["path"].as_str().unwrap());
        let text = read(&path);
        assert!(text.contains(&format!("Use {REDACTED} for CI")), "{text}");
        let index = read(&root.join("DECISIONS.md"));
        assert!(index.contains(&format!("Use {REDACTED} for CI")), "{index}");
        assert_nowhere(&env, &root, &[&secret]);
    }

    #[test]
    fn drift_explain_and_dismiss_mask_their_text() {
        let env = Env::new(Snapper::Missing);
        let lb = env.tmp.path().join("logbook");
        copy_dir(&fixture_logbook(), &lb);
        let lbs = lb.to_str().unwrap();
        let intent = token("INTENT");
        let reason = token("DISMISS");
        // the fixture's theme item and the leader of its 09-30 group
        let v = run(
            &env,
            &[
                "--logbook",
                lbs,
                "drift",
                "explain",
                "01M3VTGNY0NZG4AY80814WSKGR",
                "--",
                &format!("Switched theme, {intent}"),
            ],
        );
        let masked = format!("Switched theme, {REDACTED}");
        assert_eq!(v["case"]["title"], masked.as_str());
        let id = v["case"]["id"].as_str().unwrap();
        let case = read(&find_file(&lb.join("work/completed"), id));
        assert!(case.contains(&masked), "{case}");
        run(
            &env,
            &[
                "--logbook",
                lbs,
                "drift",
                "dismiss",
                "01M3SXBQVR7AW8PJQC1YXDCQ14",
                "--",
                &format!("routine, {reason}"),
            ],
        );
        assert_eq!(
            last_ledger_line(&lb)["detail"],
            format!("routine, {REDACTED}").as_str()
        );
        assert_nowhere(&env, &lb, &[&intent, &reason]);
    }

    #[test]
    fn event_masks_the_subject_and_every_meta_value() {
        let env = Env::new(Snapper::Missing);
        let root = env.init_logbook();
        let v = run(
            &env,
            &[
                "event",
                "manual",
                "note",
                "--subject",
                "deploy token=fakeSubjectValue",
                "--detail",
                "token=fakeDetailValue",
                "--meta",
                "url=https://user:fakeUrlValue@h.example/x",
                "--meta",
                "command=curl -u admin:fakeCommandValue https://h.example",
                "--meta",
                "version=API_KEY=fakeVersionValue",
            ],
        );
        let line = last_ledger_line(&root);
        assert_eq!(line, v["event"]);
        assert_eq!(line["subject"], format!("deploy token={REDACTED}").as_str());
        assert_eq!(line["detail"], format!("token={REDACTED}").as_str());
        assert_eq!(
            line["meta"]["url"],
            format!("https://{REDACTED}@h.example/x").as_str()
        );
        assert_eq!(
            line["meta"]["command"],
            format!("curl -u {REDACTED} https://h.example").as_str()
        );
        assert_eq!(
            line["meta"]["version"],
            format!("API_KEY={REDACTED}").as_str()
        );
        assert_nowhere(
            &env,
            &root,
            &[
                "fakeSubjectValue",
                "fakeDetailValue",
                "fakeUrlValue",
                "fakeCommandValue",
                "fakeVersionValue",
            ],
        );
    }
}
