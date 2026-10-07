//! Programs that take a secret as a plain argument, or from stdin that the
//! line itself feeds (SPEC-ENGINE §8, WP-140): no redaction rule of
//! SPEC-ENGINE §7 can tell their secret from the other words, so the hook
//! records a line that runs one as `<program> ‹redacted›`, as it records a
//! line in which `sudo -S` reads the password from stdin (ADR-0039).
//! `nmcli`'s secrets are named by their property and have a §7 rule
//! (`nmcli-secret`), so a line with them keeps its other words.

use crate::pkgcmd::{ShellLine, simple_commands};

/// A program whose arguments can put a secret on the line.
struct SecretArgs {
    name: &'static str,
    /// Its short options that do (`htpasswd -b`), also in a cluster
    /// (`-bc`, `-Bbc`).
    short: &'static str,
    /// Its short options that take a value: the rest of the cluster, else
    /// the next word, is that value, so the scan of the word stops there.
    short_value: &'static str,
    /// Its long options that do (`passwd --stdin`), also with `=…` and
    /// as a prefix, which getopt takes when it is unique (`--std`).
    long: &'static [&'static str],
    /// When it does whatever its options: always (`chpasswd` reads
    /// `user:password` lines from stdin), or when the line can feed it
    /// stdin or a key file itself (`cryptsetup`), see [`Feed`].
    feed: Feed,
}

/// When a program puts its secret on the line whatever its options.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Feed {
    /// Only through the options of [`SecretArgs`].
    Options,
    /// Always.
    Always,
    /// When the line holds a pipe, a here-string or a process
    /// substitution (`echo -n PW | cryptsetup open …`, `<<< PW`,
    /// `--key-file <(echo PW)`). A heredoc's body is cut from the line
    /// before it is recorded (SPEC-ENGINE §7), so it needs no mask.
    Fed,
}

/// The programs (WP-129 Fable stage 2, WP-140).
const SECRET_ARGS: [SecretArgs; 10] = [
    SecretArgs {
        name: "chpasswd",
        short: "",
        short_value: "",
        long: &[],
        feed: Feed::Always,
    },
    SecretArgs {
        name: "chgpasswd",
        short: "",
        short_value: "",
        long: &[],
        feed: Feed::Always,
    },
    // `-b`: the password is the last argument; `-i`: read from stdin
    SecretArgs {
        name: "htpasswd",
        short: "bi",
        short_value: "Cr",
        long: &[],
        feed: Feed::Options,
    },
    // `-s`: the password from stdin; `-w PW`: the LDAP admin password
    SecretArgs {
        name: "smbpasswd",
        short: "sw",
        short_value: "cDrRU",
        long: &[],
        feed: Feed::Options,
    },
    // shadow's `-s`/`--stdin` (not `-S`, the status)
    SecretArgs {
        name: "passwd",
        short: "s",
        short_value: "inrwxRP",
        long: &["--stdin"],
        feed: Feed::Options,
    },
    // `-p HASH`: an encrypted password, still a secret (`--password` is
    // SPEC-ENGINE §7's `password-option`)
    SecretArgs {
        name: "useradd",
        short: "p",
        short_value: "bcdefgGkKsuRPZ",
        long: &[],
        feed: Feed::Options,
    },
    SecretArgs {
        name: "usermod",
        short: "p",
        short_value: "cdefgGlsuRPZvVwW",
        long: &[],
        feed: Feed::Options,
    },
    SecretArgs {
        name: "groupadd",
        short: "p",
        short_value: "gKRP",
        long: &[],
        feed: Feed::Options,
    },
    SecretArgs {
        name: "groupmod",
        short: "p",
        short_value: "gnRP",
        long: &[],
        feed: Feed::Options,
    },
    SecretArgs {
        name: "cryptsetup",
        short: "",
        short_value: "",
        long: &[],
        feed: Feed::Fed,
    },
];

impl SecretArgs {
    /// Whether `args` (the words after the program) put the secret on a
    /// line that `fed` says can feed stdin ([`Feed::Fed`]).
    fn on_the_line(&self, args: &[String], fed: bool) -> bool {
        match self.feed {
            Feed::Always => return true,
            Feed::Fed => return fed,
            Feed::Options => {}
        }
        let mut words = args.iter();
        while let Some(word) = words.next() {
            if word == "--" {
                return false;
            }
            if let Some(long) = word.strip_prefix("--") {
                let name = long.split_once('=').map_or(long, |(name, _)| name);
                if !name.is_empty() && self.long.iter().any(|l| l[2..].starts_with(name)) {
                    return true;
                }
                continue;
            }
            let Some(cluster) = word.strip_prefix('-') else {
                continue;
            };
            for (k, letter) in cluster.char_indices() {
                if self.short.contains(letter) {
                    return true;
                }
                if self.short_value.contains(letter) {
                    // the value: the rest of the cluster, else the next word
                    if cluster[k + letter.len_utf8()..].is_empty() {
                        words.next();
                    }
                    break;
                }
            }
        }
        false
    }
}

/// Whether a command of `line` (`sh -c` scripts opened, after its
/// wrappers) is one of [`SECRET_ARGS`] that puts its secret on the line.
pub(super) fn secret_on_the_line(line: &ShellLine) -> bool {
    let fed = ["|", "<<<", "<("].iter().any(|op| line.text.contains(op));
    simple_commands(line).iter().any(|segment| {
        let Some((program, args)) = segment.argv().split_first() else {
            return false;
        };
        let name = program.rsplit('/').next().unwrap_or(program);
        SECRET_ARGS
            .iter()
            .find(|p| p.name == name)
            .is_some_and(|p| p.on_the_line(args, fed))
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
        ] {
            assert!(!secret_on_the_line(&parse_shell(line)), "{line}");
        }
    }
}
