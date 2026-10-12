//! Programs that take a secret as a plain argument, or from stdin that the
//! line itself feeds (SPEC-ENGINE §8, WP-140): no redaction rule of
//! SPEC-ENGINE §7 can tell their secret from the other words, so the hook
//! records a line that runs one as `<program> ‹redacted›`, as it records a
//! line in which `sudo -S` reads the password from stdin (ADR-0039).
//! `nmcli`'s secrets are named by their property and have a §7 rule
//! (`nmcli-secret`), so a line with them keeps its other words.

use crate::pkgcmd::{Segment, ShellLine, simple_commands};

/// A program whose arguments can put a secret on the line.
struct SecretArgs {
    name: &'static str,
    /// Its subcommand, the first word after it (`openssl passwd`); empty:
    /// any.
    subcommand: &'static str,
    /// Its short options that do (`htpasswd -b`), also in a cluster
    /// (`-bc`, `-Bbc`).
    short: &'static str,
    /// Its short options that take a value: the rest of the cluster, else
    /// the next word, is that value, so the scan of the word stops there.
    short_value: &'static str,
    /// Its short options whose value is the secret (`ssh-keygen -N PW`):
    /// they do when that value is not empty (`-N ''` sets none).
    short_secret: &'static str,
    /// Its long options that do (`passwd --stdin`), also with `=…` and
    /// as a prefix, which getopt takes when it is unique (`--std`).
    long: &'static [&'static str],
    /// Its long options that take the next word as their value unless
    /// written `--opt=value`, also as a prefix (`mkpasswd --method
    /// sha-512`): that word is no operand.
    long_value: &'static [&'static str],
    /// An operand, a word that is no option or option value, is the secret
    /// (`mkpasswd PW`; without one it asks).
    positional: bool,
    /// When it does whatever its options: always (`chpasswd` reads
    /// `user:password` lines from stdin), or when the line can feed it
    /// stdin or a key file itself (`cryptsetup`, `passwd`), see [`Feed`].
    feed: Feed,
}

/// When a program puts its secret on the line whatever its options.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Feed {
    /// Only through the options of [`SecretArgs`].
    Options,
    /// Always.
    Always,
    /// When the line feeds it ([`fed`]): a pipe, a here-string, a process
    /// substitution or a file the line writes (`echo -n PW | cryptsetup
    /// open …`, `<<< PW`, `--key-file <(echo PW)`, `printf PW > k;
    /// cryptsetup … -d k`).
    Fed,
    /// Through its options, or when the line feeds it (`passwd`: PAM reads
    /// the new password from stdin when there is no terminal, WP-140
    /// round 2).
    OptionsOrFed,
}

/// The programs (WP-129 Fable stage 2, WP-140, WP-157).
const SECRET_ARGS: [SecretArgs; 14] = [
    SecretArgs {
        name: "chpasswd",
        subcommand: "",
        short: "",
        short_value: "",
        short_secret: "",
        long: &[],
        long_value: &[],
        positional: false,
        feed: Feed::Always,
    },
    SecretArgs {
        name: "chgpasswd",
        subcommand: "",
        short: "",
        short_value: "",
        short_secret: "",
        long: &[],
        long_value: &[],
        positional: false,
        feed: Feed::Always,
    },
    // `-b`: the password is the last argument; `-i`: read from stdin
    SecretArgs {
        name: "htpasswd",
        subcommand: "",
        short: "bi",
        short_value: "Cr",
        short_secret: "",
        long: &[],
        long_value: &[],
        positional: false,
        feed: Feed::Options,
    },
    // `-s`: the password from stdin; `-w PW`: the LDAP admin password
    SecretArgs {
        name: "smbpasswd",
        subcommand: "",
        short: "sw",
        short_value: "cDrRU",
        short_secret: "",
        long: &[],
        long_value: &[],
        positional: false,
        feed: Feed::Options,
    },
    // shadow's `-s`/`--stdin` (not `-S`, the status)
    SecretArgs {
        name: "passwd",
        subcommand: "",
        short: "s",
        short_value: "inrwxRP",
        short_secret: "",
        long: &["--stdin"],
        long_value: &[],
        positional: false,
        feed: Feed::OptionsOrFed,
    },
    // `-p HASH`, `--password HASH`: an encrypted password, still a secret
    // (SPEC-ENGINE §7's `password-option` masks a plain `--password X`,
    // not a prefix such as `--passw` or a value such as `$(openssl
    // passwd -6 PW)`)
    SecretArgs {
        name: "useradd",
        subcommand: "",
        short: "p",
        short_value: "bcdefgGkKsuRPZ",
        short_secret: "",
        long: &["--password"],
        long_value: &[],
        positional: false,
        feed: Feed::Options,
    },
    SecretArgs {
        name: "usermod",
        subcommand: "",
        short: "p",
        short_value: "cdefgGlsuRPZvVwW",
        short_secret: "",
        long: &["--password"],
        long_value: &[],
        positional: false,
        feed: Feed::Options,
    },
    SecretArgs {
        name: "groupadd",
        subcommand: "",
        short: "p",
        short_value: "gKRP",
        short_secret: "",
        long: &["--password"],
        long_value: &[],
        positional: false,
        feed: Feed::Options,
    },
    SecretArgs {
        name: "groupmod",
        subcommand: "",
        short: "p",
        short_value: "gnRP",
        short_secret: "",
        long: &["--password"],
        long_value: &[],
        positional: false,
        feed: Feed::Options,
    },
    SecretArgs {
        name: "cryptsetup",
        subcommand: "",
        short: "",
        short_value: "",
        short_secret: "",
        long: &[],
        long_value: &[],
        positional: false,
        feed: Feed::Fed,
    },
    // `openssl passwd PW` hashes the password given as an argument (or
    // read with `-stdin` from what the line pipes in)
    SecretArgs {
        name: "openssl",
        subcommand: "passwd",
        short: "",
        short_value: "",
        short_secret: "",
        long: &[],
        long_value: &[],
        positional: false,
        feed: Feed::Always,
    },
    // `wpa_passphrase SSID PW` (without PW it asks on stdin)
    SecretArgs {
        name: "wpa_passphrase",
        subcommand: "",
        short: "",
        short_value: "",
        short_secret: "",
        long: &[],
        long_value: &[],
        positional: false,
        feed: Feed::Always,
    },
    // `-N new_passphrase`, `-P passphrase` (WP-157); the options with a
    // value of OpenSSH's getopt string
    SecretArgs {
        name: "ssh-keygen",
        subcommand: "",
        short: "",
        short_value: "CDEFIMORVYZabfmnrstwz",
        short_secret: "NP",
        long: &[],
        long_value: &[],
        positional: false,
        feed: Feed::Options,
    },
    // `mkpasswd PW [SALT]` (whois): the password is its first operand;
    // `-s`/`--stdin` reads it from stdin, as it does without a terminal
    // (WP-157)
    SecretArgs {
        name: "mkpasswd",
        subcommand: "",
        short: "s",
        short_value: "mSRP",
        short_secret: "",
        long: &["--stdin"],
        long_value: &["--method", "--salt", "--rounds", "--password-fd"],
        positional: true,
        feed: Feed::OptionsOrFed,
    },
];

impl SecretArgs {
    /// Whether `args` (the words after the program) put the secret on a
    /// line, `fed` when the line feeds the command ([`fed`]).
    fn on_the_line(&self, args: &[String], fed: bool) -> bool {
        if !self.subcommand.is_empty() && args.first().is_none_or(|a| a != self.subcommand) {
            return false;
        }
        match self.feed {
            Feed::Always => return true,
            Feed::Fed => return fed,
            Feed::OptionsOrFed if fed => return true,
            Feed::Options | Feed::OptionsOrFed => {}
        }
        // a long option written whole or as a prefix getopt takes
        let names = |longs: &[&str], name: &str| {
            !name.is_empty() && longs.iter().any(|l| l[2..].starts_with(name))
        };
        let mut words = args.iter();
        while let Some(word) = words.next() {
            if word == "--" {
                // every word after it is an operand
                return self.positional && words.next().is_some();
            }
            if let Some(long) = word.strip_prefix("--") {
                let (name, inline) = match long.split_once('=') {
                    Some((name, value)) => (name, Some(value)),
                    None => (long, None),
                };
                if names(self.long, name) {
                    return true;
                }
                if inline.is_none() && names(self.long_value, name) {
                    words.next();
                }
                continue;
            }
            let Some(cluster) = word.strip_prefix('-') else {
                if self.positional {
                    return true;
                }
                continue;
            };
            for (k, letter) in cluster.char_indices() {
                if self.short.contains(letter) {
                    return true;
                }
                let secret = self.short_secret.contains(letter);
                if secret || self.short_value.contains(letter) {
                    // the value: the rest of the cluster, else the next word
                    let rest = &cluster[k + letter.len_utf8()..];
                    let value = if rest.is_empty() {
                        words.next().map(String::as_str)
                    } else {
                        Some(rest)
                    };
                    if secret && value.is_some_and(|v| !v.is_empty()) {
                        return true;
                    }
                    break;
                }
            }
        }
        false
    }
}

/// Whether a command of `line` (`sh -c` scripts opened, after its
/// wrappers) is one of [`SECRET_ARGS`] that puts its secret on the line,
/// or the line gives a process substitution as a secret
/// ([`secret_substitution`]).
pub(super) fn secret_on_the_line(line: &ShellLine) -> bool {
    let segments = simple_commands(line);
    let writes = writes_a_file(&segments);
    secret_substitution(&line.text)
        || segments.iter().any(|segment| {
            let Some((program, args)) = segment.argv().split_first() else {
                return false;
            };
            let name = program.rsplit('/').next().unwrap_or(program);
            SECRET_ARGS
                .iter()
                .filter(|p| p.name == name)
                .any(|p| p.on_the_line(args, fed(segment, writes)))
        })
}

/// Whether the line feeds `segment` its secret itself: a pipe into it, a
/// here-string or a process substitution among its words
/// ([`Segment::fed`]: `passwd -S alice | grep x` feeds `grep`, not
/// `passwd`; WP-157), or a file one of the line's commands writes
/// (`writes`: `printf PW > k; cryptsetup … -d k`). A heredoc's body is cut
/// from the line before it is recorded (SPEC-ENGINE §7), so a heredoc
/// alone needs no mask.
fn fed(segment: &Segment, writes: bool) -> bool {
    segment.fed || writes
}

/// Whether a command of the line writes a file (not `/dev/null`,
/// `/dev/stdout`, `/dev/stderr`), which another can read as its key.
fn writes_a_file(segments: &[Segment]) -> bool {
    const NO_FILE: [&str; 3] = ["/dev/null", "/dev/stdout", "/dev/stderr"];
    segments
        .iter()
        .flat_map(|s| &s.writes)
        .any(|w| !NO_FILE.contains(&w.as_str()))
}

/// The parts of an option or keyword that names a secret
/// (`--password-file`, `--passphrase-file`, `--key-file`, `--token-file`,
/// `--authfile`, wg's `private-key`).
const SECRET_WORDS: [&str; 7] = ["pass", "pw", "secret", "token", "key", "cred", "auth"];

/// Whether `text` gives a process substitution to an option or keyword
/// that names a secret, of any program (`restic --password-file <(echo
/// PW)`, `--passphrase-file=<(…)`; WP-157): the command inside it prints
/// the secret, from the line itself. One pass over the text, no regex.
fn secret_substitution(text: &str) -> bool {
    text.match_indices("<(").any(|(at, _)| {
        let before = text[..at]
            .trim_end_matches(|c: char| c.is_whitespace() || matches!(c, '=' | '\\' | '\'' | '"'));
        let word = before
            .rsplit(|c: char| !(c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.')))
            .next()
            .unwrap_or_default();
        SECRET_WORDS.iter().any(|w| word.contains(w))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pkgcmd::parse_shell;

    /// Each form puts a secret on the line; the forms next to it do not.
    #[test]
    fn the_forms_that_put_a_secret_on_the_line() {
        for line in [
            "echo 'alice:pw' | sudo chpasswd",
            "sudo chpasswd < users.txt",
            "chgpasswd <<< 'staff:pw'",
            "sudo htpasswd -b /etc/nginx/.htpasswd alice pw",
            "htpasswd -Bbc f alice pw",
            "htpasswd -nb alice pw",
            "htpasswd -C 10 -b f alice pw",
            "echo pw | htpasswd -i f alice",
            "(echo pw; echo pw) | sudo smbpasswd -s -a alice",
            "sudo smbpasswd -as alice",
            "sudo smbpasswd -w ldapPw",
            "sudo smbpasswd -wldapPw",
            "echo pw | sudo passwd --stdin alice",
            "echo pw | sudo passwd -s alice",
            "echo pw | sudo passwd --std alice",
            "sudo useradd -m -p '$6$salt$hash' alice",
            "sudo useradd -mp '$6$x' alice",
            "sudo usermod -p '$6$x' alice",
            "sudo usermod -aG wheel -p x alice",
            "sudo groupadd -p x staff",
            "sudo groupmod -p x staff",
            "echo -n pw | sudo cryptsetup open /dev/sdb1 vault -d -",
            "sudo cryptsetup luksOpen /dev/sdb1 vault <<< pw",
            "sudo cryptsetup open /dev/sdb1 vault --key-file <(printf pw)",
            "sudo sh -c 'echo alice:pw | chpasswd'",
            "sudo /usr/bin/htpasswd -b f alice pw",
            "env LANG=C sudo usermod -p x alice",
            // round 2: a key file the line writes (B1)
            "printf pw > k; sudo cryptsetup open /dev/sdb1 vault -d k",
            "echo -n pw >> /tmp/k && sudo cryptsetup luksAddKey /dev/sdb1 /tmp/k",
            // passwd fed from the line (B2)
            "printf 'pw\\npw' | sudo passwd alice",
            "sudo passwd alice <<< $'pw\\npw'",
            // its options alone, the line feeding nothing
            "sudo passwd -s alice < pw.txt",
            "sudo passwd --stdin alice < pw.txt",
            // the long option and its getopt prefixes (N1)
            "sudo useradd -m --password '$6$x' alice",
            "sudo usermod --passw=x alice",
            "sudo usermod --password $(openssl passwd -6 pw) alice",
            "sudo groupadd --password x staff",
            "sudo groupmod --pass x staff",
            // more programs
            "openssl passwd -6 pw",
            "sudo openssl passwd -1 -salt s pw",
            "wpa_passphrase Home pw | sudo tee /etc/wpa_supplicant/w.conf",
            // WP-157: ssh-keygen's pass phrases, also in a cluster
            "sudo ssh-keygen -t ed25519 -N pw -f /root/k",
            "ssh-keygen -p -P old -N '' -f k",
            "ssh-keygen -qNpw -f k",
            "ssh-keygen -f k -N \"two words\"",
            // mkpasswd's operand, its stdin option, a line that feeds it
            "mkpasswd -m sha-512 pw",
            "mkpasswd --method=sha-512 pw salt",
            "mkpasswd --meth sha-512 -R 5000 pw",
            "mkpasswd -- pw",
            "mkpasswd -s < pw.txt",
            "mkpasswd --stdin < pw.txt",
            "printf pw | mkpasswd -m sha-512",
            // a program run by xargs
            "echo pw | sudo xargs -n1 htpasswd -b f alice",
            "printf 'pw\\npw' | sudo xargs -0 passwd alice",
            "echo pw | xargs mkpasswd -m sha-512",
            // what xargs reads becomes arguments, a secret or not: masked
            // too much, by design
            "printf 'a\\nb' | sudo xargs -n1 passwd -S",
            // a process substitution as a secret, any program
            "sudo restic -r /srv/r --password-file <(echo pw) backup /etc",
            "tool --passphrase-file=<(printf pw)",
            "sudo wg set wg0 private-key <(echo key)",
            "tool --token-file \\\n  <(echo tok)",
            "tool \"--password-file\" <(echo pw)",
            // a fed `sh -c`
            "sudo sh -c 'passwd alice' <<< pw",
        ] {
            assert!(secret_on_the_line(&parse_shell(line)), "{line}");
        }
        for line in [
            "sudo usermod -aG wheel alice",
            "sudo usermod -c 'Peter Parker' alice",
            "sudo useradd -m -s /bin/zsh -c pp alice",
            "sudo useradd -c -p alice",
            "sudo passwd alice",
            "sudo passwd -S alice",
            "sudo passwd -l alice",
            "sudo passwd --status alice",
            "sudo htpasswd -D f alice",
            "sudo htpasswd -c f alice",
            "sudo htpasswd -C 10 -B f alice",
            "sudo smbpasswd -a alice",
            "sudo smbpasswd -x alice",
            "sudo smbpasswd -U sam -r host",
            "sudo cryptsetup open /dev/sdb1 vault",
            "sudo cryptsetup close vault && echo closed",
            "sudo cryptsetup open /dev/sdb1 vault -d /root/key",
            "sudo groupadd staff",
            "sudo nmcli con up Home",
            "echo chpasswd",
            "sudo usermod -- -p",
            // round 2: writes that hold no key, a write before passwd -S
            "sudo cryptsetup open /dev/sdb1 vault 2>/dev/null",
            "sudo cryptsetup status vault > /dev/stderr",
            "sudo passwd -S alice 2>/dev/null",
            "sudo usermod --login bob alice",
            "openssl rand -hex 16",
            "openssl req -new -key k.pem -passin env:PW",
            // WP-157: a pipe out of the program feeds it nothing
            "sudo passwd -S alice | grep alice",
            "sudo passwd -S alice |& grep -q L && echo locked",
            "sudo cryptsetup status vault | grep -i cipher",
            "sudo passwd -S alice || echo none | cat",
            "diff <(sort a) <(sort b); sudo passwd -S alice",
            "sudo tee /etc/x < <(echo hi)",
            // no pass phrase, a value that only looks like the option
            "ssh-keygen -t ed25519 -N '' -f k",
            "ssh-keygen -t ed25519 -C '-N x' -f k",
            "ssh-keygen -lf k.pub",
            "ssh-keygen -N",
            // mkpasswd that asks, or only lists
            "mkpasswd -m help",
            "mkpasswd --method sha-512",
            "mkpasswd -m sha-512 -R 5000 -S salt12345",
            "mkpasswd -P 3 3< pw.txt",
            // xargs running a program that takes no secret
            "find . -name '*.key' | sudo xargs rm",
        ] {
            assert!(!secret_on_the_line(&parse_shell(line)), "{line}");
        }
    }
}
