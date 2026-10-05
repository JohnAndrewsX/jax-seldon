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
use seldon::redact::{BUILTIN, REDACTED, Redactor, holds_trigger, trigger_text, triggers};
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
    // `/` and `@` in one password: the `@` before the host is no
    // address (rule `email`, WP-093)
    (
        "url-userinfo",
        "git clone https://bob:fake/pw@5@git.example/x.git",
        "fake/pw@5",
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
        "sshpass -p fakePw15 ssh me@host",
        "fakePw15",
        "sshpass -p ‹redacted› ssh me@host",
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
    // white space and a newline around the `:`; `api_key`, `apiKey`
    (
        "json-secret",
        "{\"password\"\n:\n  \"fakeJson7\"}",
        "fakeJson7",
        "{\"password\"\n:\n  ‹redacted›}",
    ),
    (
        "json-secret",
        r#"curl -d '{"api_key": "fakeJson8"}' https://h.example"#,
        "fakeJson8",
        r#"{"api_key": ‹redacted›}'"#,
    ),
    (
        "json-secret",
        r#"{"openaiApiKey" :"fakeJson9"}"#,
        "fakeJson9",
        r#"{"openaiApiKey" :‹redacted›}"#,
    ),
    (
        "json-secret",
        r#"{"apiKeyHint": "x", "monkey": "y", "apiKey": "fakeJson10"}"#,
        "fakeJson10",
        r#"{"apiKeyHint": "x", "monkey": "y", "apiKey": ‹redacted›}"#,
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
    // a `&`, `;` or `|` inside quotes does not end the command (WP-087)
    (
        "curl-user",
        "curl 'https://h.example/q?a=1&b=2' -u admin:fakePw31",
        "fakePw31",
        "'https://h.example/q?a=1&b=2' -u ‹redacted›",
    ),
    (
        "curl-user",
        "curl -H \"X-Note: a;b|c\" --user admin:fakePw32 https://h.example",
        "fakePw32",
        "--user ‹redacted› https://h.example",
    ),
    (
        "curl-user",
        r#"curl -d "{\"q\":\"x;y\"}" -u admin:fakePw33 https://h.example"#,
        "fakePw33",
        "-u ‹redacted› https://h.example",
    ),
    (
        "proxy-option",
        "curl -d 'a=1&b=2' -U bob:fakeProxyPw11 https://h.example",
        "fakeProxyPw11",
        "-d 'a=1&b=2' -U ‹redacted› https://h.example",
    ),
    (
        "proxy-userinfo",
        "curl \"https://h.example/?a=1&b=2\" -x bob:fakeProxyPw12@proxy.example:3128",
        "fakeProxyPw12",
        "-x ‹redacted›@proxy.example:3128",
    ),
    (
        "cookie-option",
        "curl -H 'X-A: x;y' -b 'sid=fakeCookie8' https://h.example",
        "fakeCookie8",
        "-b ‹redacted› https://h.example",
    ),
    (
        "sshpass-password",
        "sshpass -P 'pass;word:' -p fakePw34 ssh me@host",
        "fakePw34",
        "-p ‹redacted› ssh me@host",
    ),
    (
        "registry-login-password",
        "podman login --authfile 'a&b.json' -p fakePw35 quay.example",
        "fakePw35",
        "-p ‹redacted› quay.example",
    ),
    // a backslash escape outside quotes is no separator either
    (
        "curl-user",
        r"curl https://h.example/q?a=1\&b=2 -u admin:fakePw38",
        "fakePw38",
        r"q?a=1\&b=2 -u ‹redacted›",
    ),
    // a quote the line never closes is an ordinary character
    (
        "curl-user",
        "curl's -u admin:fakePw36 did not work",
        "fakePw36",
        "curl's -u ‹redacted› did not work",
    ),
    // … also after a quoted separator, where only the quote-aware
    // reading reaches the option
    (
        "curl-user",
        "curl -H 'X-A: a;b' isn't sent with -u admin:fakePw42",
        "fakePw42",
        "isn't sent with -u ‹redacted›",
    ),
    // quotes pair left to right; where the shell reads them otherwise,
    // the plain reading still reaches the option (WP-087 round 2)
    (
        "curl-user",
        r#"curl -H $'a\'"' -u admin:fakePw39 -o "out""#,
        "fakePw39",
        "-u ‹redacted› -o \"out\"",
    ),
    (
        "curl-user",
        r#"curl -H "$(printf '"')" -u admin:fakePw40 -H "x" https://h.example"#,
        "fakePw40",
        "-u ‹redacted› -H \"x\" https://h.example",
    ),
    (
        "curl-user",
        r"curl \ -u admin:fakePw41 https://h.example",
        "fakePw41",
        "-u ‹redacted› https://h.example",
    ),
    // an unquoted `;` ends the scan-on too
    (
        "proxy-option",
        "curl -U bob:fakeProxyPw13 https://h.example; useradd -U bob",
        "fakeProxyPw13",
        "-U ‹redacted› https://h.example; useradd -U bob",
    ),
    // an option given twice in one command is masked both times; the
    // secret column is the part both values share (WP-087). A quoted
    // proxy value is matched up to its closing quote, so the scan for
    // the next `-x` starts outside the quotes
    (
        "curl-user",
        "curl -u admin:fakeTwiceA1 https://h.example -u bob:fakeTwiceA2",
        "fakeTwiceA",
        "curl -u ‹redacted› https://h.example -u ‹redacted›",
    ),
    (
        "curl-user",
        "curl --user admin:fakeTwiceB1 'https://h.example/?a&b' -uadmin:fakeTwiceB2 -o f",
        "fakeTwiceB",
        "--user ‹redacted› 'https://h.example/?a&b' -u‹redacted› -o f",
    ),
    (
        "proxy-option",
        "curl -U bob:fakeTwiceC1 -U bob:fakeTwiceC2 https://h.example",
        "fakeTwiceC",
        "curl -U ‹redacted› -U ‹redacted› https://h.example",
    ),
    (
        "proxy-option",
        "curl --proxy-user bob:fakeTwiceD1 -x proxy.example:3128 -U bob:fakeTwiceD2 https://h.example",
        "fakeTwiceD",
        "--proxy-user ‹redacted› -x proxy.example:3128 -U ‹redacted› https",
    ),
    (
        "proxy-option",
        "curl --proxy-user bob:fakeTwiceE1 https://h.example --proxy-user=bob:fakeTwiceE2",
        "fakeTwiceE",
        "--proxy-user ‹redacted› https://h.example --proxy-user=‹redacted›",
    ),
    // without the command word nothing scans on: wget's `-U` names the
    // user agent
    (
        "proxy-option",
        "wget --proxy-user=bob -U Wget/1.25 https://h.example",
        "bob",
        "wget --proxy-user=‹redacted› -U Wget/1.25 https://h.example",
    ),
    (
        "proxy-userinfo",
        "curl --proxy bob:fakeTwiceF1@p1.example:3128 -x bob:fakeTwiceF2@p2.example:3128 https://h.example",
        "fakeTwiceF",
        "--proxy ‹redacted›@p1.example:3128 -x ‹redacted›@p2.example:3128 https",
    ),
    (
        "proxy-userinfo",
        "curl -x 'bob:fakeTwiceG1@p1.example:3128' -H 'X-A: a;b' -x \"bob:fakeTwiceG2@p2.example:3128\" https://h.example",
        "fakeTwiceG",
        "-x '‹redacted›@p1.example:3128' -H 'X-A: a;b' -x \"‹redacted›@p2.example:3128\" https",
    ),
    (
        "cookie-option",
        "curl -b 'a=fakeTwiceH1' https://h.example -b 'b=fakeTwiceH2'",
        "fakeTwiceH",
        "curl -b ‹redacted› https://h.example -b ‹redacted›",
    ),
    // the first `-b` names a cookie file and stays; the second is masked
    (
        "cookie-option",
        "curl -b cookies.txt --cookie 'sid=fakeCookie9' https://h.example",
        "fakeCookie9",
        "curl -b cookies.txt --cookie ‹redacted› https://h.example",
    ),
    (
        "registry-login-password",
        "docker login -p fakeTwiceI1 -u me -p fakeTwiceI2 registry.example",
        "fakeTwiceI",
        "login -p ‹redacted› -u me -p ‹redacted› registry.example",
    ),
    // a `-p` after the command that sshpass runs is that command's
    (
        "sshpass-password",
        "sshpass -p fakePw37 ssh -p 2222 me@host",
        "fakePw37",
        "sshpass -p ‹redacted› ssh -p 2222 me@host",
    ),
    // a command continued over lines (`\` before the line end) and a
    // quoted string over several lines stay one command (WP-097)
    (
        "curl-user",
        "curl -sS \\\n  -H 'Accept: a;b' \\\n  -u admin:fakeCont1 \\\n  https://h.example",
        "fakeCont1",
        "-u ‹redacted› \\\n  https://h.example",
    ),
    (
        "proxy-option",
        "curl -X POST -d '{\n  \"a\": 1\n}' -U bob:fakeCont2 https://h.example",
        "fakeCont2",
        "}' -U ‹redacted› https://h.example",
    ),
    (
        "cookie-option",
        "curl -d \"line 1\nline 2\" -b 'sid=fakeCont3' https://h.example",
        "fakeCont3",
        "line 2\" -b ‹redacted› https://h.example",
    ),
    (
        "curl-user",
        "curl -u admin:fakeCont4a \\\n  -u bob:fakeCont4b https://h.example",
        "fakeCont4",
        "-u ‹redacted› \\\n  -u ‹redacted› https://h.example",
    ),
    // … also between an option and its value
    (
        "curl-user",
        "curl -sS -u \\\n  admin:fakeCont5 https://h.example",
        "fakeCont5",
        "-u \\\n  ‹redacted› https://h.example",
    ),
    (
        "password-option",
        "tool --password \\\n  fakeCont6 --verbose",
        "fakeCont6",
        "--password \\\n  ‹redacted› --verbose",
    ),
    // wget's password options before a space (the `=` forms are
    // `…PASSWORD=` assignments)
    (
        "password-option",
        "wget --http-user=me --http-password fakeHttpPw1 https://h.example",
        "fakeHttpPw1",
        "--http-password ‹redacted› https://h.example",
    ),
    (
        "password-option",
        "wget --ftp-password fakeFtpPw1 ftp://h.example/f",
        "fakeFtpPw1",
        "--ftp-password ‹redacted› ftp://h.example/f",
    ),
    (
        "sshpass-password",
        "sshpass -p \\\n  fakeCont7 ssh me@host",
        "fakeCont7",
        "sshpass -p \\\n  ‹redacted› ssh me@host",
    ),
    (
        "db-client-password",
        "mysql -u root \\\n  -pfakeCont8 shop",
        "fakeCont8",
        "mysql -u root \\\n  -p‹redacted›",
    ),
    (
        "db-client-password",
        "mysql -u root -pfakeCont9a \\\n  shop --init-command=fakeCont9b",
        "fakeCont9",
        "mysql -u root -p‹redacted›",
    ),
    // curl's key passphrase and bearer token, also for the proxy, and
    // xh's bearer token (WP-097)
    (
        "secret-option",
        "curl --oauth2-bearer fakeBearer1 https://h.example",
        "fakeBearer1",
        "--oauth2-bearer ‹redacted› https://h.example",
    ),
    (
        "secret-option",
        "curl --cert client.pem --key client.key --pass fakeKeyPass1 https://h.example",
        "fakeKeyPass1",
        "--pass ‹redacted› https://h.example",
    ),
    (
        "secret-option",
        "curl --proxy-pass fakeKeyPass2 -x proxy.example:3128 https://h.example",
        "fakeKeyPass2",
        "--proxy-pass ‹redacted› -x proxy.example:3128",
    ),
    (
        "secret-option",
        "xh --bearer fakeBearer2 h.example/api",
        "fakeBearer2",
        "xh --bearer ‹redacted› h.example/api",
    ),
    // a client certificate with its password: the value with `:` is
    // masked whole; without the command word only the long forms count
    (
        "cert-password",
        "curl -E client.pem:fakeCertPw1 https://h.example",
        "fakeCertPw1",
        "curl -E ‹redacted› https://h.example",
    ),
    (
        "cert-password",
        "curl -sS --cert 'client.pem:fake Cert Pw2' https://h.example",
        "fake Cert Pw2",
        "--cert ‹redacted› https://h.example",
    ),
    (
        "cert-password",
        "curl -Eclient.pem:fakeCertPw3 https://h.example",
        "fakeCertPw3",
        "curl -E‹redacted› https://h.example",
    ),
    (
        "cert-password",
        "curl -x proxy.example:3128 --proxy-cert proxy.pem:fakeCertPw4 https://h.example",
        "fakeCertPw4",
        "--proxy-cert ‹redacted› https://h.example",
    ),
    (
        "cert-password",
        "tool --cert=client.pem:fakeCertPw5 --verbose",
        "fakeCertPw5",
        "tool --cert=‹redacted› --verbose",
    ),
    (
        "cert-password",
        "curl -E client.pem -o out https://h.example -E other.pem:fakeCertPw6",
        "fakeCertPw6",
        "curl -E client.pem -o out https://h.example -E ‹redacted›",
    ),
    // HTTPie and xh: `-a`/`--auth`, any value (WP-097)
    (
        "httpie-auth",
        "http -a admin:fakeHttpie1 GET https://h.example",
        "fakeHttpie1",
        "http -a ‹redacted› GET https://h.example",
    ),
    (
        "httpie-auth",
        "xh --auth=admin:fakeHttpie2 h.example/x",
        "fakeHttpie2",
        "xh --auth=‹redacted› h.example/x",
    ),
    (
        "httpie-auth",
        "https -A bearer -a fakeHttpie3 h.example",
        "fakeHttpie3",
        "https -A bearer -a ‹redacted› h.example",
    ),
    (
        "httpie-auth",
        "xhs -aadmin:fakeHttpie4 h.example",
        "fakeHttpie4",
        "xhs -a‹redacted› h.example",
    ),
    (
        "httpie-auth",
        "http POST h.example/api 'q=a;b' -a admin:fakeHttpie5",
        "fakeHttpie5",
        "'q=a;b' -a ‹redacted›",
    ),
    (
        "httpie-auth",
        "http -a a:fakeHttpie6a h.example -a b:fakeHttpie6b",
        "fakeHttpie6",
        "http -a ‹redacted› h.example -a ‹redacted›",
    ),
    // a redirection is no separator: `2>&1`, `&>`, `<&`, `>|` (WP-097)
    (
        "curl-user",
        "curl -sS https://h.example 2>&1 -u admin:fakeRedir1",
        "fakeRedir1",
        "2>&1 -u ‹redacted›",
    ),
    (
        "proxy-option",
        "curl -sS https://h.example &>/dev/null -U bob:fakeRedir2",
        "fakeRedir2",
        "&>/dev/null -U ‹redacted›",
    ),
    (
        "cookie-option",
        "curl https://h.example <&3 -b 'sid=fakeRedir3'",
        "fakeRedir3",
        "<&3 -b ‹redacted›",
    ),
    (
        "curl-user",
        "curl https://h.example >|out.txt -u admin:fakeRedir4",
        "fakeRedir4",
        ">|out.txt -u ‹redacted›",
    ),
    // quotes as the shell reads them: an ANSI-C string before the option
    // (WP-087 round 2), `\"` inside a quoted value, and a value that joins
    // quoted and bare parts (WP-097)
    (
        "curl-user",
        r#"curl -H $'a;b\'"' -u admin:fakeAnsi1 -o "x""#,
        "fakeAnsi1",
        "-u ‹redacted› -o \"x\"",
    ),
    (
        "password-option",
        "tool --password $'fake\\'Ansi2' --verbose",
        "Ansi2",
        "--password ‹redacted› --verbose",
    ),
    (
        "curl-user",
        r#"curl -u "a\"b;fakeEsc1" https://h.example -u bob:fakeEsc2"#,
        "fakeEsc",
        "-u ‹redacted› https://h.example -u ‹redacted›",
    ),
    (
        "curl-user",
        "curl -u admin:'fake Concat1' https://h.example",
        "Concat1",
        "curl -u ‹redacted› https://h.example",
    ),
    (
        "curl-user",
        r#"curl -u "$USER":fakeConcat2 https://h.example"#,
        "fakeConcat2",
        "curl -u ‹redacted› https://h.example",
    ),
    (
        "curl-user",
        r"curl -u admin:fake\;Concat3 https://h.example",
        "Concat3",
        "curl -u ‹redacted› https://h.example",
    ),
    (
        "curl-user",
        r#"curl -u "fakeUnclosed1\" https://h.example"#,
        "fakeUnclosed1",
        "curl -u ‹redacted› https://h.example",
    ),
    (
        "token-assignment",
        r#"TOKEN="fake\"Esc3" ./run.sh"#,
        "Esc3",
        "TOKEN=‹redacted› ./run.sh",
    ),
    (
        "secret-assignment",
        r#"DB_PASSWORD="fakeUnclosed2\" ./run.sh"#,
        "fakeUnclosed2",
        "DB_PASSWORD=‹redacted›",
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
    // also at the start of a name: the assignment rules have no word
    // boundary (an ASCII one would not see one before `ſ` or `K`)
    (
        "secret-assignment",
        "\u{17F}ECRET=pw ./run.sh",
        "=pw",
        "\u{17F}ECRET=‹redacted› ./run.sh",
    ),
    (
        "key-assignment",
        "\u{212A}EY=fakeKey57",
        "fakeKey57",
        "\u{212A}EY=‹redacted›",
    ),
    // an address keeps its domain (WP-093); a desktop entry's name is a
    // config event's subject
    (
        "email",
        "~/.local/share/applications/Mail (alice@example.com).desktop",
        "alice",
        "~/.local/share/applications/Mail (‹redacted›@example.com).desktop",
    ),
    (
        "email",
        "~/.local/share/applications/bob.smith+web@example.org.desktop",
        "bob.smith",
        "~/.local/share/applications/‹redacted›@example.org.desktop",
    ),
    (
        "email",
        "git config --global user.email \"carol_99@mail.example.com\"",
        "carol_99",
        "user.email \"‹redacted›@mail.example.com\"",
    ),
    (
        "email",
        "Mail an dave-o@example.de geschickt.",
        "dave-o",
        "Mail an ‹redacted›@example.de geschickt.",
    ),
    (
        "email",
        "mailto:erin@example.com",
        "erin",
        "mailto:‹redacted›@example.com",
    ),
    (
        "email",
        "To: frank@example.com,grace@example.org",
        "grace",
        "To: ‹redacted›@example.com,‹redacted›@example.org",
    ),
    (
        "email",
        "Kontakt (jürgen@müller.example)",
        "jürgen",
        "Kontakt (‹redacted›@müller.example)",
    ),
    (
        "email",
        "curl 'https://h.example/a?to=heidi@example.com'",
        "heidi",
        "https://h.example/a?to=‹redacted›@example.com'",
    ),
    // a colon after the address with white space after it is no
    // `host:path`; `.services` is a top-level domain, no unit
    (
        "email",
        "Reply to ivan@example.com: thanks",
        "ivan",
        "Reply to ‹redacted›@example.com: thanks",
    ),
    (
        "email",
        "judy@example.services",
        "judy",
        "‹redacted›@example.services",
    ),
    // after the SSH remote and the unit the line stays
    (
        "email",
        "git remote add origin git@github.com:example/x.git # by kate@example.com",
        "kate",
        "git@github.com:example/x.git # by ‹redacted›@example.com",
    ),
    (
        "email",
        "systemctl restart getty@tty1.service && mail -s done leo@example.com",
        "leo@",
        "getty@tty1.service && mail -s done ‹redacted›@example.com",
    ),
    // text glued to an address stays (WP-093 round 2): CJK, full-width
    // and general punctuation are no part of a local part
    (
        "email",
        "日本語の長い文章です。改行なしnana@example.com",
        "nana",
        "日本語の長い文章です。改行なし‹redacted›@example.com",
    ),
    (
        "email",
        "山田太郎さん（連絡先：oscar@example.com）",
        "oscar",
        "山田太郎さん（連絡先：‹redacted›@example.com）",
    ),
    (
        "email",
        "Kontakt—paul@example.com",
        "paul",
        "Kontakt—‹redacted›@example.com",
    ),
    (
        "email",
        "„quinn@example.com“",
        "quinn",
        "„‹redacted›@example.com“",
    ),
    // a top-level domain beyond ASCII
    (
        "email",
        "olga@пример.испытание",
        "olga",
        "‹redacted›@пример.испытание",
    ),
    // a `:` keeps an address only before other text, not before a
    // further address or at the end; it takes no character of what follows
    (
        "email",
        "a@b.co:c@d.example",
        "c@d",
        "‹redacted›@b.co:‹redacted›@d.example",
    ),
    (
        "email",
        "me@example.org:me2@example.net",
        "me2",
        "‹redacted›@example.org:‹redacted›@example.net",
    ),
    (
        "email",
        "an rita@example.org:",
        "rita",
        "an ‹redacted›@example.org:",
    ),
    (
        "email",
        "a@b.co:git@h.example:o/r",
        "a@b",
        "‹redacted›@b.co:git@h.example:o/r",
    ),
    // an SSH login with a dot in the host reads as an address: masked
    // too, as `ssh://me@host` is a URL with userinfo
    (
        "email",
        "ssh -p 2222 me@host.example",
        "me@",
        "ssh -p 2222 ‹redacted›@host.example",
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
    "ssh -p 2222 me@host",
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
    "curl -H 'Cookie:\nsid=fakeCookie7' https://h.example",
    "cookie: banner fixed",
    "make cookie: all",
    "curl -H \"Cookie: $COOKIE\" https://h.example",
    "curl -b cookies.txt -c cookies.txt https://h.example",
    "curl --cookie-jar jar.txt https://h.example",
    // an unquoted `;`, `&` or `|` ends the command, also after quoted
    // strings (WP-087)
    "curl https://h.example; useradd -U bob",
    "curl -o 'out' https://h.example; useradd -m 'bob' -U bob",
    "curl -s 'https://h.example/?a=1&b=2' | grep -b 'k=v'",
    "curl \"https://h.example\" && wget -U 'agent:x@y' https://h.example",
    "Merged the curl changes; useradd -U is the default now",
    "Tried curl & wget; grep -b 'a=b' found nothing",
    "Fixed curl's output; useradd -U is next",
    // a line end ends the command unless a `\` before it or an open
    // quote continues it; a quote the text never closes does not
    // (WP-097)
    "curl https://h.example\nuseradd -U bob",
    "curl -o 'out' https://h.example\nuseradd -m 'bob' -U bob",
    "curl -o \"out\" https://h.example \\ \nuseradd -U bob",
    "Fixed curl's output today.\nsort -u names.txt",
    // a separator after a redirection still ends it
    "curl -sS https://h.example 2>&1 && useradd -U bob",
    "curl -sS https://h.example >&2; useradd -U bob",
    "curl https://h.example & useradd -U bob",
    // close to the WP-097 rules: a certificate without a password,
    // curl's `-e` (referer), `grep -E` after a pipe, HTTPie's `-A`,
    // `-a` after a URL, options that end in `pass`
    "curl --cert client.pem --key client.key https://h.example",
    "curl --cert-type P12 --cert c.p12 https://h.example",
    "curl -e https://ref.example https://h.example",
    "curl -s https://h.example | grep -E 'a:b'",
    "http -A bearer h.example",
    "wget http://h.example/f -a log.txt",
    "yt-dlp https://h.example/v -a list.txt",
    "app --pass-through on --bypass x --password-stdin",
    // no e-mail address (WP-093): an SSH remote and `host:path`, a host
    // without a dot, versions, npm scopes, systemd units, a scale suffix
    // without a top-level domain, an image digest
    "git clone git@github.com:example/x.git",
    "rsync -a ./ me@host.example:/srv/www",
    "ssh me@localhost",
    "npm i @scope/pkg @scope/other@1.2.3 left-pad@1.3.0 react@18.2.0-rc.1",
    "pnpm add typescript@latest",
    "npm i typescript@5.4.10",
    "systemctl enable --now getty@tty1.service wg-quick@wg0.service",
    "systemctl --user start app@x.timer app@y.socket user@1000.slice",
    "monitor = DP-2, 2560x1440@144, 2560x0, 1",
    "docker pull alpine@sha256:abc123",
    "Bild icon@2x",
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
                triggers(rule).iter().any(|t| holds_trigger(&lower, t)),
                "{rule}: no trigger in `{input}`"
            );
        }
        // the marker can never trigger a rule, nor add a part of one
        let marker = trigger_text(REDACTED);
        for rule in BUILTIN {
            assert!(
                !triggers(rule)
                    .iter()
                    .flat_map(|t| t.split('+'))
                    .any(|part| marker.contains(part)),
                "{rule}"
            );
        }
        // a curl line without their options compiles none of the curl
        // rules (WP-084)
        let plain = trigger_text("curl -fsSL https://h.example/f -o /tmp/f");
        for rule in [
            "curl-user",
            "proxy-option",
            "proxy-userinfo",
            "cookie-option",
        ] {
            assert!(
                !triggers(rule).iter().any(|t| holds_trigger(&plain, t)),
                "{rule}"
            );
        }
    }

    #[test]
    fn harmless_text_stays() {
        let r = Redactor::builtin();
        for text in CLEAR {
            assert_eq!(r.redact(text), *text, "{:?}", r.matching_rules(text));
        }
        // every systemd unit type (WP-093)
        for unit in [
            "service",
            "socket",
            "target",
            "timer",
            "mount",
            "automount",
            "path",
            "slice",
            "scope",
            "swap",
            "device",
        ] {
            let text = format!("systemctl status app@x.{unit}");
            assert_eq!(r.redact(&text), text);
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
        // a user pattern that also matches inside the marker itself, also
        // at its first character
        let r = Redactor::with_patterns(&["‹re|red|act".into()]).unwrap();
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

    /// Rule `email` matches only its own rows and no row of another rule:
    /// a URL with userinfo is `url-userinfo`'s and a proxy's `user:pass@`
    /// is `proxy-userinfo`'s (the import report counts a line once per
    /// rule). A masked address is no address, and a second pass changes
    /// nothing (WP-093).
    #[test]
    fn email_rule_is_disjoint_and_stable() {
        let r = Redactor::builtin();
        for (rule, input, ..) in TABLE {
            let matched = r.matching_rules(input);
            if *rule == "email" {
                assert_eq!(matched, vec!["email"], "`{input}`");
            } else {
                assert!(!matched.contains(&"email"), "{rule}: `{input}`");
            }
            let once = r.redact(input);
            assert!(!r.matching_rules(&once).contains(&"email"), "`{once}`");
            assert_eq!(r.redact(&once), once, "`{input}`");
        }
        // an address on a line with a URL with userinfo is masked too
        assert_eq!(
            r.redact("curl https://u:fakePw@h.example/x -d to=mike@example.com"),
            format!("curl https://{REDACTED}@h.example/x -d to={REDACTED}@example.com")
        );
    }

    /// The WP-097 rules match only their own rows and no row of another
    /// rule (the import report counts a line once per rule), and a second
    /// pass changes nothing; `secret-option`'s new names (`--pass`,
    /// `--oauth2-bearer`) take over no row of another rule either.
    #[test]
    fn cert_and_httpie_rules_are_disjoint_and_stable() {
        const NEW: [&str; 2] = ["cert-password", "httpie-auth"];
        let r = Redactor::builtin();
        for (rule, input, ..) in TABLE {
            let matched = r.matching_rules(input);
            if NEW.contains(rule) || input.contains("--pass ") || input.contains("bearer ") {
                assert_eq!(matched, vec![*rule], "`{input}`");
            } else {
                assert!(
                    !matched.iter().any(|m| NEW.contains(m)),
                    "{rule}: `{input}` also matches {matched:?}"
                );
            }
            let once = r.redact(input);
            assert_eq!(r.redact(&once), once, "`{input}`");
        }
    }

    /// A long line in which an early rule leaves a marker, or which holds
    /// other non-ASCII text, stays on the fast matcher: the word
    /// boundaries are ASCII (WP-084 round 2). Median of 21, warm rules.
    #[test]
    #[ignore = "release timing: cargo test --profile bench --test redaction -- --ignored"]
    fn long_lines_with_a_marker_stay_fast() {
        use std::time::Duration;
        super::common::assert_optimised();
        let r = Redactor::builtin();
        let filled = |head: &str, word: &str, size: usize| {
            let mut line = head.to_string();
            while line.len() < size {
                line.push_str(word);
            }
            line
        };
        for (what, head, word) in [
            // `-u`, `-x` and `-b` inside words: every curl rule is
            // triggered, finds no option and scans the whole line
            (
                "url line",
                "curl https://bob:fakePw@h.example/a -o out ",
                "a-u-x-b ",
            ),
            (
                "german note",
                "curl -sS https://h.example ",
                "Schlüssel-u-x-b geändert ",
            ),
            // quoted separators keep the command open to the line end,
            // through every quoted string (WP-087)
            (
                "quoted line",
                "curl -sS https://h.example ",
                "'a;b' \"c|d\" e-u-x-b ",
            ),
            (
                "apostrophes",
                "curl -sS https://h.example ",
                "it's a-u-x-b ",
            ),
            // one command over many lines, continued by `\` or by quotes
            // around each line end, with every curl and HTTPie rule
            // triggered (WP-097)
            (
                "continued lines",
                "curl -sS https://h.example \\\n",
                "  -H 'a;b' a-u-x-b-E-a 2>&1 \\\n",
            ),
            (
                "quoted line ends",
                "curl -sS https://h.example '\n'",
                "a-u-x-b-E-a $'c;d' '\n'",
            ),
            ("httpie line", "http -v h.example ", "'a;b' a-a-u 2>&1 "),
        ] {
            for (kb, budget) in [(16, 1), (64, 2)] {
                let line = filled(head, word, kb * 1024);
                super::common::assert_within_budget(
                    &format!("redact, {what}, {kb} KB"),
                    Duration::from_millis(budget),
                    21,
                    || {
                        std::hint::black_box(r.redact(&line));
                    },
                );
            }
        }
        // many masked values: every match is checked against the markers
        // of the earlier rules, by binary search (WP-087 round 2); the
        // budget, where there is one, holds at 128 KB
        for (what, head, word, budget) in [
            ("two option kinds", "curl ", "-u a:b -x c:d@e ", Some(20)),
            (
                "five option kinds",
                "curl ",
                "--proxy-user=a:b -U c:d -x e:f@g -b h=i -u j:k ",
                None,
            ),
            (
                "password and token",
                "tool ",
                "--password x token=y ",
                Some(10),
            ),
            // addresses, and `@` in forms that are none, with every curl
            // rule triggered: `email` matches each and keeps the latter
            // (WP-093)
            (
                "addresses",
                "mail ",
                "a.b@example.com x@y.example ",
                Some(20),
            ),
            (
                "at signs",
                "curl -sS https://bob:fakePw@h.example ",
                "pkg@1.2.3 @scope/x getty@tty1.service git@h.example:o/r a-u-x-b ",
                Some(20),
            ),
            // addresses joined by `:`: each match looks at the text after
            // it and at the next match (WP-093 round 2)
            (
                "address colons",
                "mail ",
                "a@b.example:c@d.example:e@f.example: ",
                Some(20),
            ),
            // options on continued lines, values joined from quoted parts
            // (WP-097)
            (
                "continued options",
                "curl ",
                "-u a:'b' \\\n -E c.pem:d \\\n ",
                Some(20),
            ),
            (
                "httpie options",
                "http ",
                "-a a:\"b\" --auth=c:d 2>&1 ",
                Some(20),
            ),
        ] {
            for kb in [16, 64, 128] {
                let line = filled(head, word, kb * 1024);
                let what = format!("redact, {what}, {kb} KB");
                let run = || {
                    std::hint::black_box(r.redact(&line));
                };
                match budget {
                    Some(ms) if kb == 128 => {
                        super::common::assert_within_budget(
                            &what,
                            Duration::from_millis(ms),
                            21,
                            run,
                        );
                    }
                    _ => {
                        let (median, times) = super::common::median_time(21, run);
                        eprintln!("{what}: median {median:?} (no budget), all {times:?}");
                    }
                }
            }
        }
    }

    #[test]
    fn user_patterns() {
        let r = Redactor::with_patterns(&["corp-[0-9]{6}".into(), "(?i)internal\\.example".into()])
            .unwrap();
        assert_eq!(
            r.redact("ssh corp-123456@INTERNAL.example"),
            format!("ssh {REDACTED}@{REDACTED}")
        );
        // guide 06: a personal domain is masked by a pattern of its own
        let r = Redactor::with_patterns(&[r"@smith\.example\b".into()]).unwrap();
        assert_eq!(
            r.redact("mail jo@smith.example and ann@example.org"),
            format!("mail {REDACTED}{REDACTED} and {REDACTED}@example.org")
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
    use super::common::{
        Env, Snapper, copy_dir, find_file, fixture_logbook, json, read, stderr, stdout,
    };

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

    /// A desktop entry named after an account (WP-093): the config
    /// events, the index and every file of the logbook hold the name with
    /// the local part masked; a change and a removal still find the file,
    /// also next to an entry whose masked name is the same, and a second
    /// capture writes nothing. The manifest in the state directory keeps
    /// the real names, as it must to compare the files.
    #[test]
    fn a_desktop_entry_named_after_an_address_is_masked() {
        let env = Env::new(Snapper::NoPermissions);
        let root = env.init_logbook();
        let dir = env.home.join(".local/share/applications");
        std::fs::create_dir_all(&dir).unwrap();
        let alice = dir.join("Mail (alice.webapp@example.com).desktop");
        let bob = dir.join("Mail (bob.webapp@example.com).desktop");
        let capture = || {
            let out = env
                .command(&["capture", "--source", "config", "--json"])
                .output()
                .unwrap();
            assert_eq!(
                out.status.code(),
                Some(0),
                "{}{}",
                stdout(&out),
                stderr(&out)
            );
            json(&out)["written"].as_u64().unwrap()
        };
        std::fs::write(&alice, "[Desktop Entry]\nName=Mail\n").unwrap();
        assert_eq!(capture(), 0, "baseline");
        std::fs::write(&bob, "[Desktop Entry]\nName=Mail\n").unwrap();
        assert_eq!(capture(), 1);
        std::fs::write(&alice, "[Desktop Entry]\nName=Mail 2\n").unwrap();
        assert_eq!(capture(), 1);
        assert_eq!(capture(), 0, "a second capture writes nothing");
        std::fs::remove_file(&bob).unwrap();
        assert_eq!(capture(), 1);
        assert_eq!(capture(), 0);

        let subject = format!("~/.local/share/applications/Mail ({REDACTED}@example.com).desktop");
        let events: Vec<(String, String)> = super::common::ledger(&root)
            .into_iter()
            .filter(|e| e["source"] == "config")
            .map(|e| {
                (
                    e["kind"].as_str().unwrap().to_string(),
                    e["subject"].as_str().unwrap().to_string(),
                )
            })
            .collect();
        let expected: Vec<(String, String)> = ["config-add", "config-change", "config-remove"]
            .iter()
            .map(|k| (k.to_string(), subject.clone()))
            .collect();
        assert_eq!(events, expected);

        run(&env, &["status"]);
        let mut all = Vec::new();
        files(&root, &mut all);
        all.push(env.home.join(".local/state/seldon/index.json"));
        for path in &all {
            let text = String::from_utf8_lossy(&std::fs::read(path).unwrap()).into_owned();
            for local in ["alice.webapp", "bob.webapp"] {
                assert!(!text.contains(local), "{local} in {}", path.display());
            }
        }
        let manifest = read(&env.home.join(".local/state/seldon/manifest.json"));
        assert!(manifest.contains("alice.webapp@example.com"), "{manifest}");
        // guide 06's pattern keeps such a file out altogether
        let skip = seldon::collectors::config::SkipPaths::new(&env.home, &["*@*.desktop".into()]);
        assert!(skip.matches(&alice) && skip.matches(&bob));
        assert!(!skip.matches(&dir.join("Zoom.desktop")));
    }

    /// The hook hands the whole command to the redaction, its continued
    /// lines and a quoted string over several lines included (only a
    /// heredoc body is cut), so a curl option on a later line of the same
    /// command is masked in the recorded line (WP-097).
    #[test]
    fn the_hook_masks_a_command_continued_over_lines() {
        use std::io::Write;
        use std::process::Stdio;

        let env = Env::new(Snapper::NoPermissions);
        let logbook = env.init_logbook();
        let command = "curl -sS \\\n  -H 'Accept: a;b' \\\n  -u admin:fakeHookPw1 \\\n  \
                       -d '{\n  \"a\": 1\n}' -U bob:fakeHookPw2 \\\n  \
                       https://h.example/x -o /tmp/x && yay -S --noconfirm zed";
        let payload = serde_json::json!({
            "command": command,
            "actor": "agent:codex",
            "cwd": logbook,
        });
        let mut child = env
            .command(&["hook", "generic"])
            .env("SELDON_NOW", T0)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        child
            .stdin
            .take()
            .unwrap()
            .write_all(payload.to_string().as_bytes())
            .unwrap();
        let out = child.wait_with_output().unwrap();
        assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
        assert_eq!(stderr(&out), "");

        let line = last_ledger_line(&logbook);
        assert_eq!(line["source"], "agent", "{line}");
        assert_eq!(
            line["meta"]["command"],
            format!(
                "curl -sS \\\n  -H 'Accept: a;b' \\\n  -u {REDACTED} \\\n  \
                 -d '{{\n  \"a\": 1\n}}' -U {REDACTED} \\\n  \
                 https://h.example/x -o /tmp/x && yay -S --noconfirm zed"
            )
        );
        assert_nowhere(&env, &logbook, &["fakeHookPw"]);
    }
}
