//! Names that may appear as arguments of the commands in `REBUILD.md`.
//!
//! Every value from the logbook (fence rows, ledger subjects, `meta.url`)
//! passes one of the checks below before [`render`](super::render) puts it
//! into a command; a value that fails is listed as "not reproduced: invalid
//! name" instead. No check accepts a leading `-`, so a value can never be
//! read as an option. [`quote`] then makes a valid value one shell word:
//! unchanged when it holds only plain characters, else single-quoted.

/// Longest name any check accepts (systemd's `UNIT_NAME_MAX` less the NUL).
const NAME_MAX: usize = 255;

/// Longest URL [`is_https_url`] accepts.
const URL_MAX: usize = 2048;

/// The unit types systemd knows (`systemd.unit(5)`).
const UNIT_TYPES: [&str; 11] = [
    "service",
    "socket",
    "target",
    "device",
    "mount",
    "automount",
    "swap",
    "timer",
    "path",
    "slice",
    "scope",
];

fn bounded(s: &str, max: usize) -> bool {
    !s.is_empty() && s.len() <= max && !s.starts_with('-')
}

/// A package name as makepkg accepts it: letters, digits and `@._+-`, not
/// starting with `-` or `.`.
pub fn is_package_name(s: &str) -> bool {
    bounded(s, NAME_MAX)
        && !s.starts_with('.')
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || "@._+-".contains(c))
}

/// A systemd unit name: `prefix[@instance].type` with letters, digits and
/// `:_.\-` (systemd's escaping uses `\`), at most one `@`, a known type.
pub fn is_unit_name(s: &str) -> bool {
    if !bounded(s, NAME_MAX) {
        return false;
    }
    let Some((name, kind)) = s.rsplit_once('.') else {
        return false;
    };
    if !UNIT_TYPES.contains(&kind) || name.is_empty() {
        return false;
    }
    let (prefix, instance) = match name.split_once('@') {
        Some((p, i)) => (p, Some(i)),
        None => (name, None),
    };
    let plain = |part: &str| {
        part.chars()
            .all(|c| c.is_ascii_alphanumeric() || ":_.\\-".contains(c))
    };
    // a template (`foo@.service`) has an empty instance
    !prefix.is_empty() && plain(prefix) && instance.is_none_or(plain)
}

/// An Omarchy theme name as `omarchy theme set` stores it: letters,
/// digits and `._-`, starting with a letter or digit.
pub fn is_theme_slug(s: &str) -> bool {
    bounded(s, NAME_MAX)
        && s.starts_with(|c: char| c.is_ascii_alphanumeric())
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || "._-".contains(c))
}

/// A plugin id as `omarchy plugin validate` accepts it: letters, digits
/// and `._-`, starting with a letter or digit, no `..`. First-party
/// (`omarchy.*`) ids are valid here; they are named, not added.
pub fn is_plugin_id(s: &str) -> bool {
    bounded(s, NAME_MAX)
        && s.starts_with(|c: char| c.is_ascii_alphanumeric())
        && !s.contains("..")
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || "._-".contains(c))
}

/// An `https://` URL without user info, query or fragment: a host of
/// letters, digits, `.` and `-`, an optional port, and a path of letters,
/// digits and `-._~/%+=:,@`.
pub fn is_https_url(s: &str) -> bool {
    if s.len() > URL_MAX {
        return false;
    }
    let Some(rest) = s.strip_prefix("https://") else {
        return false;
    };
    let (authority, path) = match rest.find('/') {
        Some(i) => rest.split_at(i),
        None => (rest, ""),
    };
    let (host, port) = match authority.split_once(':') {
        Some((h, p)) => (h, Some(p)),
        None => (authority, None),
    };
    !host.is_empty()
        && !host.starts_with(['-', '.'])
        && host
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-')
        && port
            .is_none_or(|p| !p.is_empty() && p.len() <= 5 && p.bytes().all(|b| b.is_ascii_digit()))
        && path
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "-._~/%+=:,@".contains(c))
}

/// `s` as one shell word: unchanged when every character is plain
/// (letters, digits, `@%+=:,./_-`), else in single quotes. Callers pass
/// only values a check above accepted.
pub fn quote(s: &str) -> std::borrow::Cow<'_, str> {
    if !s.is_empty()
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || "@%+=:,./_-".contains(c))
    {
        s.into()
    } else {
        format!("'{}'", s.replace('\'', r"'\''")).into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Values every check refuses: shell syntax, an option, a quote.
    const REFUSED_BY_ALL: [&str; 12] = [
        "",
        "a;b",
        "a$(b)",
        "a`b`",
        "a${IFS}b",
        "a|b",
        "a&&b",
        "-a",
        "--overwrite=x",
        "a'b",
        "a\"b",
        "a b",
    ];

    fn refuses_all(check: fn(&str) -> bool, name: &str) {
        for v in REFUSED_BY_ALL {
            assert!(!check(v), "{name} accepts {v:?}");
        }
        assert!(!check(&"a".repeat(URL_MAX + 1)), "{name}: too long");
        assert!(!check("a\nb"), "{name}: a newline");
    }

    #[test]
    fn package_names() {
        for v in [
            "zed",
            "brave-bin",
            "7zip",
            "lib32-gcc-libs",
            "gtk+3",
            "python3.12",
            "foo@bar",
            "font_x",
            "Zoom",
        ] {
            assert!(is_package_name(v), "{v}");
        }
        refuses_all(is_package_name, "is_package_name");
        for v in [".hidden", "a/b", "a*", "a~", "a<b", "a>b", "a\\b", "ä"] {
            assert!(!is_package_name(v), "{v}");
        }
    }

    #[test]
    fn unit_names() {
        for v in [
            "ollama.service",
            "tailscaled.service",
            "getty@tty1.service",
            "foo@.service",
            "backup.timer",
            "home-x\\x2dy.mount",
            "a:b.socket",
            "x.service.d.service",
        ] {
            assert!(is_unit_name(v), "{v}");
        }
        refuses_all(is_unit_name, "is_unit_name");
        for v in [
            "ollama",
            "ollama.conf",
            ".service",
            "@x.service",
            "a@b@c.service",
            "-.mount",
            "a;b.service",
            "x.service;b",
            "a/b.service",
            "a$(b).service",
        ] {
            assert!(!is_unit_name(v), "{v}");
        }
    }

    #[test]
    fn theme_slugs() {
        for v in [
            "tokyo-night",
            "catppuccin-latte",
            "retro-82",
            "my_theme",
            "a.b",
        ] {
            assert!(is_theme_slug(v), "{v}");
        }
        refuses_all(is_theme_slug, "is_theme_slug");
        for v in [".hidden", "_x", "a/b", "Tokyo Night", "a*"] {
            assert!(!is_theme_slug(v), "{v}");
        }
    }

    #[test]
    fn plugin_ids() {
        for v in [
            "io.github.example.weather-plus",
            "omarchy.clock",
            "user.clock",
            "jax.seldon",
            "a_b",
        ] {
            assert!(is_plugin_id(v), "{v}");
        }
        refuses_all(is_plugin_id, "is_plugin_id");
        for v in [".x", "_x", "a..b", "a/b", "a*", "<username>.clock"] {
            assert!(!is_plugin_id(v), "{v}");
        }
    }

    #[test]
    fn https_urls() {
        for v in [
            "https://github.com/acme/omarchy-weather.git",
            "https://git.example.org/~someone/clock-plus",
            "https://example.org:8443/a/b",
            "https://example.org",
            "https://example.org/a%20b/v1.2+x=y,z@w",
        ] {
            assert!(is_https_url(v), "{v}");
        }
        refuses_all(is_https_url, "is_https_url");
        for v in [
            "http://example.org/a",
            "git@github.com:a/b.git",
            "ssh://example.org/a",
            "https://",
            "https://-x.org/a",
            "https://.x.org/a",
            "https://user:pw@example.org/a",
            "https://example.org:/a",
            "https://example.org:x/a",
            "https://example.org/a;b",
            "https://example.org/a$(b)",
            "https://example.org/a`b`",
            "https://example.org/a${IFS}b",
            "https://example.org/a|b",
            "https://example.org/a&&b",
            "https://example.org/a?b=c&d=e",
            "https://example.org/a#b",
            "https://example.org/a'b",
            "https://example.org/a b",
            "https://example.org/a*",
            "https://example.org/a\nb",
        ] {
            assert!(!is_https_url(v), "{v}");
        }
    }

    #[test]
    fn quote_keeps_plain_words_and_single_quotes_the_rest() {
        assert_eq!(quote("zed"), "zed");
        assert_eq!(quote("lib32-gcc-libs"), "lib32-gcc-libs");
        assert_eq!(
            quote("https://github.com/acme/omarchy-weather.git"),
            "https://github.com/acme/omarchy-weather.git"
        );
        assert_eq!(
            quote("https://git.example.org/~someone/clock-plus"),
            "'https://git.example.org/~someone/clock-plus'"
        );
        assert_eq!(quote("home-x\\x2dy.mount"), "'home-x\\x2dy.mount'");
        assert_eq!(quote(""), "''");
        assert_eq!(quote("a'b"), r"'a'\''b'");
    }
}
