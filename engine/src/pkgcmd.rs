//! Package command lines, parsed as argv (SPEC-ENGINE §4, §8; ADR-0017 §3 §5).
//!
//! One parser for everything that reads a package command: the pacman
//! collector (the logged `[PACMAN] Running` line: explicit packages, the
//! routine class of ADR-0013 §3), attribution (what a hook `command` event
//! asked for, [`command_intent`]) and the hook (is a command mutating at
//! all). Moved here from `collectors/pacman.rs` unchanged (WP-009), so the
//! hook and the drift rules do not depend on a collector. The hook and
//! attribution read a shell line through the same [`parse_shell`],
//! [`simple_commands`] and [`command_argv`] (WP-071).

use std::path::{Path, PathBuf};

/// pacman's operation, and the yay/paru operations that are not pacman's.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Op {
    Sync,
    Remove,
    Upgrade,
    Database,
    Query,
    DepTest,
    Files,
    /// `-Y`/`--yay`: yay's own operations; only `-Yc` (`--clean`) removes
    /// packages (the unneeded dependencies).
    Yay,
    /// `-P`/`--show`: prints statistics, news, the configuration.
    Show,
    /// `-G`/`--getpkgbuild`: downloads build files, installs nothing.
    GetPkgbuild,
}

/// A pacman-style command line (`pacman`, `yay`, `paru`), parsed as argv.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PacmanCommand {
    pub program: String,
    pub op: Option<Op>,
    /// `-u`/`--sysupgrade` with `-S`.
    pub sysupgrade: bool,
    /// A query or no-op form of the operation (ADR-0017 §5): `-S` with
    /// `s|i|l|g|p|w|c`, `-R`/`-U` with `p`, and their long forms.
    pub query: bool,
    /// Package names the command names (`repo/` and version constraints
    /// stripped; for `-U`, the name from the package file name; not `-`,
    /// which reads them from stdin).
    pub targets: Vec<String>,
    /// A `-` word: pacman reads more targets from stdin, so the command
    /// names packages the argv does not show (no plain full upgrade).
    pub stdin_targets: bool,
    /// `-U` whose every file lies in a package cache (pacman's
    /// `/var/cache/pacman/pkg/`, yay's and paru's `~/.cache/yay|paru/`):
    /// a reinstall or upgrade from what was downloaded before (ADR-0028
    /// §2 `-U <cache path>`). False for any other operation.
    pub from_cache: bool,
}

/// Long options that take the next word as their value (unless written
/// `--opt=value`). An unknown option is assumed to take none, so its value
/// counts as a target — that errs towards red (ADR-0013 §3).
const LONG_WITH_ARG: [&str; 16] = [
    "dbpath",
    "root",
    "cachedir",
    "color",
    "config",
    "gpgdir",
    "hookdir",
    "logfile",
    "arch",
    "sysroot",
    "ask",
    "overwrite",
    "ignore",
    "ignoregroup",
    "assume-installed",
    "print-format",
];

/// Short options with a value: `-b` (dbpath), `-r` (root).
const SHORT_WITH_ARG: [char; 2] = ['b', 'r'];

/// Programs that take pacman's command line.
const PACMAN_LIKE: [&str; 3] = ["pacman", "yay", "paru"];

/// Splits a command line as pacman logs it: on whitespace, unquoted.
pub fn split_logged(command: &str) -> Vec<&str> {
    command.split_whitespace().collect()
}

/// Parses `argv` if its program is pacman-like, else `None`.
pub fn parse_command(argv: &[&str]) -> Option<PacmanCommand> {
    let (&program, args) = argv.split_first()?;
    let program = program.rsplit('/').next().unwrap_or(program);
    if !PACMAN_LIKE.contains(&program) {
        return None;
    }
    let mut cmd = PacmanCommand {
        program: program.to_string(),
        op: None,
        sysupgrade: false,
        query: false,
        targets: Vec::new(),
        stdin_targets: false,
        from_cache: false,
    };
    // query letters/long names seen; which count depends on the operation
    let mut flags: Vec<char> = Vec::new();
    // `-h`/`--help`, `-V`/`--version`: prints and exits
    let mut prints_only = false;
    let mut words = Vec::new();
    let mut it = args.iter();
    while let Some(&a) = it.next() {
        if a == "--" {
            words.extend(it.by_ref().copied());
            break;
        }
        if let Some(long) = a.strip_prefix("--") {
            let (name, inline) = match long.split_once('=') {
                Some((n, _)) => (n, true),
                None => (long, false),
            };
            match name {
                "sync" => cmd.op = Some(Op::Sync),
                "remove" => cmd.op = Some(Op::Remove),
                "upgrade" => cmd.op = Some(Op::Upgrade),
                "database" => cmd.op = Some(Op::Database),
                "query" => cmd.op = Some(Op::Query),
                "deptest" => cmd.op = Some(Op::DepTest),
                "files" => cmd.op = Some(Op::Files),
                "yay" => cmd.op = Some(Op::Yay),
                "show" => cmd.op = Some(Op::Show),
                "getpkgbuild" => cmd.op = Some(Op::GetPkgbuild),
                // a `-Y` option; yay reads it without `-Y` too
                "gendb" => {
                    cmd.op.get_or_insert(Op::Yay);
                }
                "help" | "version" => prints_only = true,
                "sysupgrade" => cmd.sysupgrade = true,
                "search" => flags.push('s'),
                "info" => flags.push('i'),
                "list" => flags.push('l'),
                "groups" => flags.push('g'),
                "print" => flags.push('p'),
                "downloadonly" => flags.push('w'),
                "clean" => flags.push('c'),
                _ if LONG_WITH_ARG.contains(&name) && !inline => {
                    it.next();
                }
                _ => {}
            }
        } else if let Some(cluster) = a.strip_prefix('-').filter(|c| !c.is_empty()) {
            for (i, ch) in cluster.char_indices() {
                match ch {
                    'S' => cmd.op = Some(Op::Sync),
                    'R' => cmd.op = Some(Op::Remove),
                    'U' => cmd.op = Some(Op::Upgrade),
                    'D' => cmd.op = Some(Op::Database),
                    'Q' => cmd.op = Some(Op::Query),
                    'T' => cmd.op = Some(Op::DepTest),
                    'F' => cmd.op = Some(Op::Files),
                    'Y' => cmd.op = Some(Op::Yay),
                    'P' => cmd.op = Some(Op::Show),
                    'G' => cmd.op = Some(Op::GetPkgbuild),
                    'h' | 'V' => prints_only = true,
                    'u' => cmd.sysupgrade = true,
                    c if SHORT_WITH_ARG.contains(&c) => {
                        // `-bDIR` or `-b DIR`
                        if i + c.len_utf8() == cluster.len() {
                            it.next();
                        }
                        break;
                    }
                    c => flags.push(c),
                }
            }
        } else {
            words.push(a);
        }
    }
    // `yay` alone is `yay -Syu`; `yay zed` searches and installs; `yay -V`
    // and `yay --help` only print
    if cmd.op.is_none() && cmd.program != "pacman" && !prints_only {
        cmd.op = Some(Op::Sync);
        cmd.sysupgrade |= words.is_empty();
    }
    // with -S, `u` means sysupgrade; with -R/-Q it means something else
    if cmd.op != Some(Op::Sync) || prints_only {
        cmd.sysupgrade = false;
    }
    // with -R, `s` and `c` are --recursive/--cascade, not queries
    cmd.query = prints_only
        || match cmd.op {
            Some(Op::Sync) => flags
                .iter()
                .any(|f| ['s', 'i', 'l', 'g', 'p', 'w', 'c'].contains(f)),
            Some(Op::Remove | Op::Upgrade) => flags.contains(&'p'),
            // `-Yc` removes; `-Y --gendb` and the rest only look
            Some(Op::Yay) => !flags.contains(&'c'),
            _ => false,
        };
    // `-` reads the targets from stdin: no package name here, but names
    cmd.stdin_targets = words.contains(&"-");
    cmd.from_cache =
        cmd.op == Some(Op::Upgrade) && !words.is_empty() && words.iter().all(|w| is_cache_file(w));
    // `-` reads the targets from stdin: no package name
    cmd.targets = words
        .into_iter()
        .filter(|&w| w != "-")
        .map(|w| {
            if cmd.op == Some(Op::Upgrade) {
                package_file_name(w)
            } else {
                package_name(w)
            }
        })
        .filter(|w| !w.is_empty())
        .collect();
    Some(cmd)
}

impl PacmanCommand {
    /// Whether the command names `package`.
    pub fn names(&self, package: &str) -> bool {
        self.targets.iter().any(|t| t == package)
    }

    /// `-S` with `-u` (with or without named packages).
    pub fn is_full_upgrade(&self) -> bool {
        self.op == Some(Op::Sync) && self.sysupgrade
    }

    /// `-S` with `-u` and no package, none from stdin either: the routine
    /// class of ADR-0013 §3 and ADR-0028 §2.
    pub fn is_plain_full_upgrade(&self) -> bool {
        self.is_full_upgrade() && self.targets.is_empty() && !self.stdin_targets
    }

    /// Operations that change packages: `-S`, `-R`, `-U` without a query
    /// form (ADR-0017 §5), and yay's `-Yc`. `-Q`, `-T`, `-F`, `-D`, `-P`,
    /// `-G` and every `-h`/`-V` never are.
    pub fn is_mutating(&self) -> bool {
        matches!(self.op, Some(Op::Sync | Op::Remove | Op::Upgrade | Op::Yay)) && !self.query
    }
}

/// A package file in a package cache: pacman's `/var/cache/pacman/pkg/`,
/// or yay's and paru's under a home's `.cache/` (`…/.cache/yay/<pkg>/`).
fn is_cache_file(word: &str) -> bool {
    word.contains(".pkg.tar")
        && !word.contains("/../")
        && (word.starts_with("/var/cache/pacman/pkg/")
            || word.contains("/.cache/yay/")
            || word.contains("/.cache/paru/"))
}

/// `extra/zed` → `zed`, `foo>=1.2` → `foo`.
fn package_name(word: &str) -> String {
    let name = word.rsplit('/').next().unwrap_or(word);
    name.split(['<', '>', '='])
        .next()
        .unwrap_or(name)
        .to_string()
}

/// `/var/cache/…/mesa-1:26.2.0-2-x86_64.pkg.tar.zst` → `mesa` (drops
/// pkgver, pkgrel, arch); a word that is no package file is a name.
fn package_file_name(word: &str) -> String {
    let file = word.rsplit('/').next().unwrap_or(word);
    match file.find(".pkg.tar") {
        Some(end) => {
            let parts: Vec<&str> = file[..end].rsplitn(4, '-').collect();
            if parts.len() == 4 {
                parts[3].to_string()
            } else {
                file[..end].to_string()
            }
        }
        None => package_name(word),
    }
}

/// What a shell command line (a hook `command` event) asks for, as far as
/// packages go: a full upgrade, and the packages it names.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Intent {
    pub full_upgrade: bool,
    pub packages: Vec<String>,
}

/// Reads a hook command line with the hook's parser ([`parse_shell`],
/// [`simple_commands`], [`command_argv`]): quotes, `sh -c '…'` and `eval`,
/// wrappers with their options. Knows pacman-like programs, `omarchy
/// update` and `omarchy pkg add|install|drop|remove|aur add|aur install`,
/// also as `omarchy-*` scripts ([`omarchy_route`]).
pub fn command_intent(line: &str) -> Intent {
    segments_intent(&simple_commands(&parse_shell(line)))
}

/// [`command_intent`] of a line's [`simple_commands`].
pub fn segments_intent(segments: &[Segment]) -> Intent {
    let mut intent = Intent::default();
    for segment in segments {
        let argv: Vec<&str> = segment.argv().iter().map(String::as_str).collect();
        if let Some(cmd) = parse_command(&argv) {
            if cmd.is_mutating() {
                intent.full_upgrade |= cmd.is_full_upgrade();
                intent.packages.extend(cmd.targets);
            }
            continue;
        }
        let names =
            |words: &[&str]| -> Vec<String> { words.iter().map(|w| package_name(w)).collect() };
        match omarchy_route(segment.argv()).as_deref() {
            // the update routes the hook records (SPEC-ENGINE §8)
            Some(
                ["update"]
                | [
                    "update",
                    "system" | "aur" | "keyring" | "firmware" | "orphan" | "pkg" | "mise",
                    ..,
                ],
            ) => {
                intent.full_upgrade = true;
                // `omarchy-update-keyring` installs these before the upgrade
                // (ADR-0017 §3)
                intent
                    .packages
                    .extend(OMARCHY_UPDATE_NAMES.map(String::from));
            }
            Some(
                ["pkg", "add" | "install" | "drop" | "remove", pkgs @ ..]
                | ["pkg", "aur", "add" | "install", pkgs @ ..],
            ) => intent.packages.extend(names(pkgs)),
            _ => {}
        }
    }
    intent
}

/// The route of an Omarchy command, without options: `omarchy pkg add zed`
/// and the script `omarchy-pkg-add zed` are both `[pkg, add, zed]`.
pub fn omarchy_route(argv: &[String]) -> Option<Vec<&str>> {
    let (program, args) = argv.split_first()?;
    let program = program.rsplit('/').next().unwrap_or(program);
    let mut route: Vec<&str> = match program {
        "omarchy" => Vec::new(),
        p => p.strip_prefix("omarchy-")?.split('-').collect(),
    };
    route.extend(
        args.iter()
            .map(String::as_str)
            .filter(|a| !a.starts_with('-')),
    );
    Some(route)
}

/// Packages `omarchy update` names besides the full upgrade.
pub const OMARCHY_UPDATE_NAMES: [&str; 2] = ["archlinux-keyring", "omarchy-keyring"];

fn is_assignment(word: &str) -> bool {
    word.split_once('=').is_some_and(|(k, _)| {
        !k.is_empty() && k.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
    })
}

// ---------------------------------------------------------------------------
// Shell command lines (hooks)
// ---------------------------------------------------------------------------

/// One simple command of a shell line: its words with quotes removed, and
/// the files it writes by redirection.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Segment {
    /// The words, without redirections and their targets.
    pub words: Vec<String>,
    /// Targets of `>`, `>>`, `>|`, `&>`.
    pub writes: Vec<String>,
    /// The privilege wrapper of the `sh -c '…'` or `eval` this command was
    /// opened from (`pkexec sh -c 'lpadmin …'`), if any
    /// ([`simple_commands`]).
    pub privileged_by: Option<&'static str>,
}

impl Segment {
    /// [`command_argv`] of the words.
    pub fn argv(&self) -> &[String] {
        command_argv(&self.words)
    }

    /// The privilege wrapper the command runs under: the one of the shell
    /// it was opened from, else its own ([`Unwrapped::privilege`]).
    pub fn privilege(&self) -> Option<&'static str> {
        self.privileged_by
            .or_else(|| unwrap_command(&self.words).privilege)
    }
}

/// A shell command line as an agent ran it (a hook's `tool_input.command`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ShellLine {
    /// The simple commands, split at `&&`, `||`, `;`, `|`, `&`, newlines
    /// and parentheses.
    pub segments: Vec<Segment>,
    /// The line with every heredoc body cut out: a heredoc is the
    /// command's stdin, which is never recorded (SPEC-ENGINE §7).
    pub text: String,
}

/// What the next word is after a redirection operator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Pending {
    /// A file written.
    Write,
    /// After `>&`: a descriptor (`2`, `-`, `2-`) is a copy, any other word
    /// a file written, as after `&>`.
    WriteOrCopy,
    /// A file read, or a here-string: dropped.
    Read,
    /// A heredoc delimiter (`<<-`: tabs before the delimiter line).
    Heredoc { strip_tabs: bool },
}

/// Splits a command line the way a POSIX shell would for reading it, never
/// for running it: single and double quotes, backslashes, `$(…)`,
/// backticks, `${…}`, `$((…))` and `$[…]` (kept literally inside their
/// word), `((…))` (an arithmetic command: no words), comments,
/// redirections and heredocs. A heredoc inside `$(…)`, backticks or `<(…)`
/// is cut like one at the top level, also inside double quotes; `<<` in
/// `((…))`, `$((…))` and `$[…]` is a shift. Expansions are not performed.
pub fn parse_shell(line: &str) -> ShellLine {
    let mut reader = Reader {
        line,
        chars: line.char_indices().collect(),
        cuts: Vec::new(),
    };
    let mut lexer = Lexer::default();
    reader.read(0, Stop::End, &mut lexer);
    lexer.end_segment();

    let mut cuts = reader.cuts;
    cuts.sort_unstable();
    let mut text = String::with_capacity(line.len());
    let mut from = 0;
    for (start, end) in cuts {
        if start >= from {
            text.push_str(&line[from..start]);
        }
        from = from.max(end);
    }
    text.push_str(&line[from.min(line.len())..]);
    ShellLine {
        segments: lexer.segments,
        text: text.trim_end().to_string(),
    }
}

/// Where [`Reader::read`] stops.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Stop {
    /// The end of the line.
    End,
    /// The `)` that closes a `$(` or `<(`.
    Paren,
    /// The backtick that closes a backtick substitution.
    Backtick,
}

/// The line being read, and the heredoc bodies found so far.
struct Reader<'a> {
    line: &'a str,
    chars: Vec<(usize, char)>,
    /// Byte ranges of the heredoc bodies, each with its delimiter line.
    cuts: Vec<(usize, usize)>,
}

impl Reader<'_> {
    fn at(&self, i: usize) -> Option<char> {
        self.chars.get(i).map(|&(_, c)| c)
    }

    /// Reads from `i` into `lexer` until `stop`; returns the index after
    /// the stop (the end of the line when there is none).
    fn read(&mut self, mut i: usize, stop: Stop, lexer: &mut Lexer) -> usize {
        // `(` opened inside a `$(…)`
        let mut depth = 0usize;
        while let Some(c) = self.at(i) {
            match c {
                '\\' => {
                    match self.at(i + 1) {
                        Some('\n') => {}
                        Some(n) => lexer.push(n),
                        None => lexer.push('\\'),
                    }
                    i += 2;
                    continue;
                }
                '\'' => {
                    lexer.start();
                    i += 1;
                    while let Some(q) = self.at(i) {
                        i += 1;
                        if q == '\'' {
                            break;
                        }
                        lexer.push(q);
                    }
                    continue;
                }
                '"' => {
                    i = self.double_quoted(i + 1, lexer);
                    continue;
                }
                '`' if stop == Stop::Backtick => {
                    lexer.end_segment();
                    return i + 1;
                }
                '`' => {
                    i = self.substitution(i, lexer);
                    continue;
                }
                '$' if matches!(self.at(i + 1), Some('(' | '[' | '{')) => {
                    i = self.substitution(i, lexer);
                    continue;
                }
                '#' if lexer.word.is_none() => {
                    while self.at(i).is_some_and(|k| k != '\n') {
                        i += 1;
                    }
                    continue;
                }
                '\n' => {
                    lexer.end_segment();
                    // heredoc bodies start on the next line
                    i = self.heredoc_bodies(i + 1, lexer);
                    continue;
                }
                // an arithmetic command: its `<<` is a shift
                '(' if lexer.word.is_none() && self.at(i + 1) == Some('(') => {
                    lexer.end_segment();
                    i = closing(&self.chars, i, '(', ')');
                    continue;
                }
                ')' if stop == Stop::Paren && depth == 0 => {
                    lexer.end_segment();
                    return i + 1;
                }
                '&' if self.at(i + 1) == Some('>') => {
                    lexer.end_word();
                    lexer.pending = Some(Pending::Write);
                    i += 2;
                    if self.at(i) == Some('>') {
                        i += 1;
                    }
                    continue;
                }
                ';' | '&' | '|' | '(' | ')' => {
                    match c {
                        '(' => depth += 1,
                        ')' => depth = depth.saturating_sub(1),
                        _ => {}
                    }
                    lexer.end_segment();
                    i += 1;
                    if matches!(
                        (c, self.at(i)),
                        ('&', Some('&')) | ('|', Some('|' | '&')) | (';', Some(';' | '&'))
                    ) {
                        i += 1;
                    }
                    continue;
                }
                '>' | '<' => {
                    // a file descriptor number before the operator is not a word
                    if lexer
                        .word
                        .as_deref()
                        .is_some_and(|w| !w.is_empty() && w.chars().all(|d| d.is_ascii_digit()))
                    {
                        lexer.word = None;
                    }
                    lexer.end_word();
                    i += 1;
                    if c == '>' {
                        if matches!(self.at(i), Some('>' | '|')) {
                            i += 1;
                        }
                        lexer.pending = Some(if self.at(i) == Some('&') {
                            // `>&2` copies a descriptor, `>& file` writes
                            i += 1;
                            Pending::WriteOrCopy
                        } else {
                            Pending::Write
                        });
                    } else if self.at(i) == Some('<') && self.at(i + 1) == Some('<') {
                        lexer.pending = Some(Pending::Read);
                        i += 2;
                    } else if self.at(i) == Some('<') {
                        i += 1;
                        let strip_tabs = self.at(i) == Some('-');
                        if strip_tabs {
                            i += 1;
                        }
                        lexer.pending = Some(Pending::Heredoc { strip_tabs });
                    } else if self.at(i) == Some('(') {
                        // `<(…)`: read for its heredocs, not kept
                        i = self.read(i + 1, Stop::Paren, &mut Lexer::default());
                    } else {
                        // `< file`, `<&3`: read
                        if self.at(i) == Some('&') {
                            i += 1;
                        }
                        lexer.pending = Some(Pending::Read);
                    }
                    continue;
                }
                c if c.is_whitespace() => lexer.end_word(),
                c => lexer.push(c),
            }
            i += 1;
        }
        self.chars.len()
    }

    /// The inside of double quotes from `i` into the current word; returns
    /// the index after the closing quote.
    fn double_quoted(&mut self, mut i: usize, lexer: &mut Lexer) -> usize {
        lexer.start();
        while let Some(q) = self.at(i) {
            match (q, self.at(i + 1)) {
                ('"', _) => return i + 1,
                ('\\', Some(n @ ('"' | '\\' | '$' | '`'))) => {
                    lexer.push(n);
                    i += 2;
                }
                ('\\', Some('\n')) => i += 2,
                ('`', _) | ('$', Some('(' | '[' | '{')) => i = self.substitution(i, lexer),
                (q, _) => {
                    lexer.push(q);
                    i += 1;
                }
            }
        }
        self.chars.len()
    }

    /// The `$(…)`, `$((…))`, `$[…]`, `${…}` or backtick substitution at
    /// `i`, pushed literally into the current word without the heredoc
    /// bodies it holds; returns the index after it.
    fn substitution(&mut self, i: usize, lexer: &mut Lexer) -> usize {
        let end = match (self.at(i), self.at(i + 1), self.at(i + 2)) {
            (Some('`'), ..) => self.read(i + 1, Stop::Backtick, &mut Lexer::default()),
            (_, Some('('), Some('(')) => closing(&self.chars, i + 1, '(', ')'),
            (_, Some('('), _) => self.read(i + 2, Stop::Paren, &mut Lexer::default()),
            (_, Some('['), _) => closing(&self.chars, i + 1, '[', ']'),
            _ => closing(&self.chars, i + 1, '{', '}'),
        };
        for &(b, k) in &self.chars[i..end] {
            if !self.cuts.iter().any(|&(s, e)| s <= b && b < e) {
                lexer.push(k);
            }
        }
        end
    }

    /// Cuts the bodies of the heredocs `lexer` has seen on the line that
    /// ended before `i`; returns the index after the last delimiter line.
    /// A delimiter line stays in the text when anything follows it, so the
    /// text reads as the same commands (attribution parses it again).
    fn heredoc_bodies(&mut self, mut i: usize, lexer: &mut Lexer) -> usize {
        let line = self.line;
        for (delimiter, strip_tabs) in std::mem::take(&mut lexer.heredocs) {
            let start = self.chars.get(i).map_or(line.len(), |&(b, _)| b);
            let (mut body_end, mut end) = (line.len(), line.len());
            let mut pos = start;
            while pos < line.len() {
                let next = line[pos..].find('\n').map_or(line.len(), |n| pos + n + 1);
                let text = line[pos..next].trim_end_matches(['\n', '\r']);
                let text = if strip_tabs {
                    text.trim_start_matches('\t')
                } else {
                    text
                };
                if text == delimiter {
                    (body_end, end) = (pos, next);
                    break;
                }
                pos = next;
            }
            let trailing = line[end..].trim().is_empty();
            self.cuts
                .push((start, if trailing { end } else { body_end }));
            while self.chars.get(i).is_some_and(|&(b, _)| b < end) {
                i += 1;
            }
        }
        i
    }
}

/// The index after the `close` that matches the `open` at `at` (or the
/// end); quoted text is skipped.
fn closing(chars: &[(usize, char)], at: usize, open: char, close: char) -> usize {
    let mut depth = 0usize;
    let mut quote: Option<char> = None;
    for (i, &(_, c)) in chars.iter().enumerate().skip(at) {
        match (quote, c) {
            (Some(q), c) if c == q => quote = None,
            (Some(_), _) => {}
            (None, '\'' | '"') => quote = Some(c),
            (None, c) if c == open => depth += 1,
            (None, c) if c == close => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return i + 1;
                }
            }
            _ => {}
        }
    }
    chars.len()
}

#[derive(Debug, Default)]
struct Lexer {
    segments: Vec<Segment>,
    current: Segment,
    /// The word being read; `Some("")` after an empty quoted word.
    word: Option<String>,
    pending: Option<Pending>,
    /// Heredocs whose bodies start after the next newline.
    heredocs: Vec<(String, bool)>,
}

impl Lexer {
    fn start(&mut self) {
        self.word.get_or_insert_with(String::new);
    }

    fn push(&mut self, c: char) {
        self.word.get_or_insert_with(String::new).push(c);
    }

    fn end_word(&mut self) {
        let Some(word) = self.word.take() else {
            return;
        };
        match self.pending.take() {
            Some(Pending::Write) => self.current.writes.push(word),
            Some(Pending::WriteOrCopy) => {
                let fd = word.strip_suffix('-').unwrap_or(&word);
                if !fd.chars().all(|d| d.is_ascii_digit()) {
                    self.current.writes.push(word);
                }
            }
            Some(Pending::Read) => {}
            Some(Pending::Heredoc { strip_tabs }) => self.heredocs.push((word, strip_tabs)),
            None => self.current.words.push(word),
        }
    }

    fn end_segment(&mut self) {
        self.end_word();
        self.pending = None;
        let segment = std::mem::take(&mut self.current);
        if !segment.words.is_empty() || !segment.writes.is_empty() {
            self.segments.push(segment);
        }
    }
}

/// A program that runs its arguments as a command, and how it reads its
/// own options.
struct Wrapper {
    name: &'static str,
    /// Short options that take a value: the next word (`-u root`, also at
    /// the end of a cluster as in `-Eu root`) or the rest of the cluster
    /// (`-uroot`).
    short_value: &'static str,
    /// Long options that take the next word as their value, unless written
    /// `--opt=value`.
    long_value: &'static [&'static str],
    /// Options after which the wrapper runs nothing (a probe):
    /// `command -v yay` must never read as `yay`, which is a full upgrade
    /// (ADR-0017 §3).
    probe_short: &'static str,
    probe_long: &'static [&'static str],
    /// The option whose value is the directory the command runs in.
    chdir: Option<(char, &'static str)>,
    /// Words between the options and the command (`timeout DURATION`).
    operands: usize,
}

/// `--help` and `--version`: print and exit.
const PRINTS: &[&str] = &["--help", "--version"];

/// The wrappers [`command_argv`] reads past. `sudo -k` drops the cached
/// credentials and still runs the command; `-l`, `-v`, `-K`, `-V` do not.
const WRAPPERS: [Wrapper; 11] = [
    Wrapper {
        name: "sudo",
        short_value: "ughpCDrtUTR",
        long_value: &[
            "--user",
            "--group",
            "--host",
            "--prompt",
            "--close-from",
            "--chdir",
            "--chroot",
            "--role",
            "--type",
            "--other-user",
            "--command-timeout",
        ],
        probe_short: "lvKV",
        probe_long: &[
            "--list",
            "--validate",
            "--remove-timestamp",
            "--help",
            "--version",
        ],
        chdir: Some(('D', "--chdir")),
        operands: 0,
    },
    Wrapper {
        name: "doas",
        short_value: "u",
        long_value: &[],
        // `-C config`: only checks the rule; `-L`: forgets the login
        probe_short: "CL",
        probe_long: &[],
        chdir: None,
        operands: 0,
    },
    Wrapper {
        name: "pkexec",
        short_value: "u",
        long_value: &["--user"],
        probe_short: "",
        probe_long: PRINTS,
        chdir: None,
        operands: 0,
    },
    Wrapper {
        name: "run0",
        short_value: "ugD",
        long_value: &[
            "--user",
            "--group",
            "--chdir",
            "--setenv",
            "--nice",
            "--unit",
            "--property",
            "--description",
            "--slice",
            "--background",
            "--machine",
            "--shell-prompt-prefix",
            "--area",
            "--lightweight",
        ],
        probe_short: "h",
        probe_long: PRINTS,
        chdir: Some(('D', "--chdir")),
        operands: 0,
    },
    Wrapper {
        name: "env",
        short_value: "uCS",
        long_value: &["--unset", "--chdir", "--split-string"],
        probe_short: "",
        probe_long: PRINTS,
        chdir: Some(('C', "--chdir")),
        operands: 0,
    },
    Wrapper {
        name: "nice",
        short_value: "n",
        long_value: &["--adjustment"],
        probe_short: "",
        probe_long: PRINTS,
        chdir: None,
        operands: 0,
    },
    Wrapper {
        name: "timeout",
        short_value: "sk",
        long_value: &["--signal", "--kill-after"],
        probe_short: "",
        probe_long: PRINTS,
        chdir: None,
        operands: 1,
    },
    Wrapper {
        name: "exec",
        short_value: "a",
        long_value: &[],
        probe_short: "",
        probe_long: &[],
        chdir: None,
        operands: 0,
    },
    Wrapper {
        name: "command",
        short_value: "",
        long_value: &[],
        probe_short: "vV",
        probe_long: &[],
        chdir: None,
        operands: 0,
    },
    Wrapper {
        name: "nohup",
        short_value: "",
        long_value: &[],
        probe_short: "",
        probe_long: PRINTS,
        chdir: None,
        operands: 0,
    },
    Wrapper {
        name: "time",
        short_value: "fo",
        long_value: &["--format", "--output"],
        probe_short: "",
        probe_long: PRINTS,
        chdir: None,
        operands: 0,
    },
];

/// The wrappers that run their command as another user, root by default
/// (SPEC-ENGINE §8, ADR-0039).
pub const PRIVILEGE_WRAPPERS: [&str; 4] = ["sudo", "doas", "pkexec", "run0"];

/// A simple command's words after its wrappers.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Unwrapped<'a> {
    /// The command's words ([`command_argv`]).
    pub argv: &'a [String],
    /// The directories wrappers move to before the command runs, in order:
    /// `env -C DIR`, `sudo -D DIR`, `run0 --chdir=DIR`.
    pub chdirs: Vec<&'a str>,
    /// The first of [`PRIVILEGE_WRAPPERS`] read past (`env sudo lpadmin`:
    /// `sudo`); none for a probe, which runs nothing.
    pub privilege: Option<&'static str>,
}

/// [`command_argv`], with the directories the wrappers move to.
pub fn unwrap_command(words: &[String]) -> Unwrapped<'_> {
    let probe = Unwrapped::default();
    let mut chdirs = Vec::new();
    let mut privilege = None;
    let mut i = 0;
    while let Some(w) = words.get(i) {
        if is_assignment(w) {
            i += 1;
            continue;
        }
        let name = w.rsplit('/').next().unwrap_or(w);
        let Some(wrapper) = WRAPPERS.iter().find(|x| x.name == name) else {
            break;
        };
        if PRIVILEGE_WRAPPERS.contains(&wrapper.name) {
            privilege = privilege.or(Some(wrapper.name));
        }
        i += 1;
        while let Some(o) = words.get(i).filter(|o| o.starts_with('-')) {
            i += 1;
            if o == "--" {
                break;
            }
            // the value of a value option: inline, else the next word
            let mut next_word = || {
                i += 1;
                words.get(i - 1).map(String::as_str)
            };
            if o.starts_with("--") {
                let (option, inline) = match o.split_once('=') {
                    Some((n, v)) => (n, Some(v)),
                    None => (o.as_str(), None),
                };
                if wrapper.probe_long.contains(&option) {
                    return probe;
                }
                if wrapper.long_value.contains(&option) {
                    let value = inline.or_else(next_word);
                    if wrapper.chdir.is_some_and(|(_, long)| long == option) {
                        chdirs.extend(value);
                    }
                }
                continue;
            }
            // a cluster: `-E`, `-Eu root`, `-uroot`
            for (k, letter) in o.char_indices().skip(1) {
                if wrapper.probe_short.contains(letter) {
                    return probe;
                }
                if wrapper.short_value.contains(letter) {
                    let rest = &o[k + letter.len_utf8()..];
                    let value = if rest.is_empty() {
                        next_word()
                    } else {
                        Some(rest)
                    };
                    if wrapper.chdir.is_some_and(|(short, _)| short == letter) {
                        chdirs.extend(value);
                    }
                    break;
                }
            }
        }
        i += wrapper.operands;
    }
    Unwrapped {
        argv: &words[i.min(words.len())..],
        chdirs,
        privilege,
    }
}

/// `words` after leading `VAR=value` assignments and the wrappers that run
/// their arguments as a command (`sudo`, `doas`, `pkexec`, `run0`, `env`,
/// `command`, `exec`, `nohup`, `time`, `nice`, `timeout`), with the
/// wrappers' options and option clusters (`sudo -Eu root`). Empty when a
/// wrapper only probes (`command -v yay`, `sudo -l …`, `pkexec --version`).
pub fn command_argv(words: &[String]) -> &[String] {
    unwrap_command(words).argv
}

/// How deep `sh -c '…'` and `eval '…'` are opened ([`simple_commands`]).
const NESTING_MAX: usize = 3;

/// The simple commands of `line`, where `sh|bash|zsh|dash -c '<script>'`
/// and `eval <words>` are replaced by the simple commands of their script
/// (up to `NESTING_MAX` levels; a redirection of the outer command stays
/// as a segment of its own), and a word that names a variable the line
/// sets to a literal (`F=x; … $F`, [`Vars`]) holds that value. Read only:
/// nothing else is expanded, nothing is run.
pub fn simple_commands(line: &ShellLine) -> Vec<Segment> {
    let opened = opened_commands(line);
    let vars = Vars::of(&opened);
    opened.into_iter().map(|s| vars.put_in(s)).collect()
}

/// The variables `line` sets ([`Vars`]), for reading the words of its text.
pub fn line_vars(line: &ShellLine) -> Vars {
    Vars::of(&opened_commands(line))
}

/// [`simple_commands`] before the variables are put in. A command opened
/// from `sudo sh -c '…'` keeps the shell's privilege wrapper
/// ([`Segment::privileged_by`]); the outer command's redirection is the
/// calling shell's own.
fn opened_commands(line: &ShellLine) -> Vec<Segment> {
    fn open(segment: &Segment, depth: usize, out: &mut Vec<Segment>) {
        match nested_script(segment.argv()).filter(|_| depth < NESTING_MAX) {
            Some(script) => {
                let privilege = segment.privilege();
                for inner in parse_shell(&script).segments {
                    let inner = Segment {
                        privileged_by: privilege,
                        ..inner
                    };
                    open(&inner, depth + 1, out);
                }
                if !segment.writes.is_empty() {
                    out.push(Segment {
                        words: Vec::new(),
                        writes: segment.writes.clone(),
                        privileged_by: None,
                    });
                }
            }
            None => out.push(segment.clone()),
        }
    }
    let mut out = Vec::new();
    for segment in &line.segments {
        open(segment, 0, &mut out);
    }
    out
}

/// The script of `sh -c '<script>'` (any of `sh`, `bash`, `zsh`, `dash`,
/// `-c` alone or in a cluster like `-lc`) or the words of `eval`.
fn nested_script(argv: &[String]) -> Option<String> {
    let (program, args) = argv.split_first()?;
    match program.rsplit('/').next().unwrap_or(program) {
        "eval" => (!args.is_empty()).then(|| args.join(" ")),
        "sh" | "bash" | "zsh" | "dash" => {
            let mut command_mode = false;
            let mut it = args.iter();
            while let Some(a) = it.next() {
                if a == "-o" || a == "+o" || a == "-O" || a == "+O" {
                    it.next();
                } else if a == "--" {
                    break;
                } else if let Some(cluster) = a.strip_prefix('-').filter(|c| !c.starts_with('-')) {
                    command_mode |= cluster.contains('c');
                } else if a.starts_with('-') || a.starts_with('+') {
                    // a long option (`--norc`) or `+x`
                } else {
                    return command_mode.then(|| a.clone());
                }
            }
            None
        }
        _ => None,
    }
}

/// A path a simple command writes or removes, as the word it was given.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target<'a> {
    /// The file itself: a redirection, `tee`, `sed -i`, an `mv` source,
    /// `install -d`, `unlink`, `truncate`, or a hook's `Edit|Write <path>`.
    File(&'a str),
    /// A destination of `cp|mv|install|ln`: the path itself, or a file
    /// directly in it when it is a directory.
    Into(&'a str),
    /// `rm|rmdir`: the path and everything below it.
    Tree(&'a str),
}

impl<'a> Target<'a> {
    pub fn word(&self) -> &'a str {
        match *self {
            Target::File(w) | Target::Into(w) | Target::Tree(w) => w,
        }
    }

    /// Whether a change of `path` is this target's doing, with the target's
    /// word resolved to `at`.
    pub fn covers(&self, at: &std::path::Path, path: &std::path::Path) -> bool {
        match self {
            Target::File(_) => path == at,
            Target::Into(_) => path == at || path.parent() == Some(at),
            Target::Tree(_) => path.starts_with(at),
        }
    }
}

/// What one simple command writes or removes: its redirection targets and
/// the file operands of the writers Seldon knows (`tee`, `sed -i`,
/// `cp|mv|install|ln`, `rm|rmdir|unlink|truncate`; `Edit|Write|MultiEdit
/// <path>` as a hook writes an edit). Words it only reads are not targets.
pub fn write_targets<'a>(argv: &'a [String], writes: &'a [String]) -> Vec<Target<'a>> {
    let mut out: Vec<Target> = writes.iter().map(|w| Target::File(w)).collect();
    let Some((program, args)) = argv.split_first() else {
        return out;
    };
    match program.rsplit('/').next().unwrap_or(program) {
        "tee" => out.extend(operands(args).into_iter().map(Target::File)),
        "sed" => out.extend(
            sed_in_place_files(args)
                .unwrap_or_default()
                .into_iter()
                .map(Target::File),
        ),
        "install" if args.iter().any(|a| is_directory_option(a)) => {
            out.extend(
                operands_after_values(args, &COPY_WITH_VALUE)
                    .into_iter()
                    .map(Target::File),
            );
        }
        p @ ("cp" | "mv" | "install" | "ln") => {
            if let Some((dest, sources)) = copy_parts(args) {
                out.push(Target::Into(dest));
                // a move removes its sources
                if p == "mv" {
                    out.extend(sources.into_iter().map(Target::File));
                }
            }
        }
        "rm" | "rmdir" => out.extend(operands(args).into_iter().map(Target::Tree)),
        "unlink" => out.extend(operands(args).into_iter().map(Target::File)),
        "truncate" => out.extend(
            operands_after_values(args, &["-s", "-r", "--size", "--reference"])
                .into_iter()
                .map(Target::File),
        ),
        "Edit" | "Write" | "MultiEdit" => out.extend(args.first().map(|a| Target::File(a))),
        _ => {}
    }
    out
}

/// `install -d` / `--directory` (a cluster like `-dm755` too).
fn is_directory_option(a: &str) -> bool {
    a == "--directory"
        || a.strip_prefix('-').is_some_and(|c| {
            !c.starts_with('-') && c.split('m').next().is_some_and(|c| c.contains('d'))
        })
}

/// Options of `cp`, `mv`, `install` and `ln` that take the next word.
const COPY_WITH_VALUE: [&str; 8] = [
    "-m", "-o", "-g", "-S", "--mode", "--owner", "--group", "--suffix",
];

/// The destination and the sources of `cp|mv|install|ln`: `-t DIR` or
/// `--target-directory`, else the last of at least two operands.
fn copy_parts(args: &[String]) -> Option<(&str, Vec<&str>)> {
    let mut dest = None;
    let mut operands = Vec::new();
    let mut it = args.iter();
    while let Some(a) = it.next() {
        if a == "--" {
            operands.extend(it.by_ref().map(String::as_str));
            break;
        }
        if a == "-t" || a == "--target-directory" {
            dest = it.next().map(String::as_str);
        } else if let Some(dir) = a.strip_prefix("--target-directory=") {
            dest = Some(dir);
        } else if a.starts_with('-') && a.len() > 1 {
            if COPY_WITH_VALUE.contains(&a.as_str()) {
                it.next();
            }
        } else {
            operands.push(a.as_str());
        }
    }
    match dest {
        Some(d) => Some((d, operands)),
        None if operands.len() >= 2 => {
            let d = operands.pop()?;
            Some((d, operands))
        }
        None => None,
    }
}

/// Words that are not options (everything after `--`).
fn operands(args: &[String]) -> Vec<&str> {
    operands_after_values(args, &[])
}

/// Words that are not options, skipping the value of each option in
/// `with_value`.
fn operands_after_values<'a>(args: &'a [String], with_value: &[&str]) -> Vec<&'a str> {
    let mut out = Vec::new();
    let mut it = args.iter();
    while let Some(a) = it.next() {
        if a == "--" {
            out.extend(it.by_ref().map(String::as_str));
            break;
        }
        if with_value.contains(&a.as_str()) {
            it.next();
        } else if !a.starts_with('-') || a == "-" {
            out.push(a.as_str());
        }
    }
    out
}

/// The files `sed` edits in place, or `None` without `-i`/`--in-place`.
fn sed_in_place_files(args: &[String]) -> Option<Vec<&str>> {
    let mut in_place = false;
    let mut script_given = false;
    let mut words = Vec::new();
    let mut it = args.iter();
    while let Some(a) = it.next() {
        if a == "--" {
            words.extend(it.by_ref().map(String::as_str));
            break;
        }
        if let Some(long) = a.strip_prefix("--") {
            let name = long.split('=').next().unwrap_or(long);
            match name {
                "in-place" => in_place = true,
                "expression" | "file" => {
                    script_given = true;
                    if !long.contains('=') {
                        it.next();
                    }
                }
                "line-length" if !long.contains('=') => {
                    it.next();
                }
                _ => {}
            }
        } else if let Some(cluster) = a.strip_prefix('-').filter(|c| !c.is_empty()) {
            for (i, c) in cluster.char_indices() {
                match c {
                    // the rest of the cluster is the backup suffix
                    'i' => {
                        in_place = true;
                        break;
                    }
                    'e' | 'f' | 'l' => {
                        script_given |= c != 'l';
                        if i + 1 == cluster.len() {
                            it.next();
                        }
                        break;
                    }
                    _ => {}
                }
            }
        } else {
            words.push(a.as_str());
        }
    }
    if !in_place {
        return None;
    }
    if !script_given && !words.is_empty() {
        words.remove(0);
    }
    Some(words)
}

// ---------------------------------------------------------------------------
// Directories and variables of a line
// ---------------------------------------------------------------------------

/// The directories one simple command works in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Workdir {
    /// The shell's directory: redirections open their files here.
    pub shell: PathBuf,
    /// Where the program runs and finds its own path operands: the shell's
    /// directory after the wrappers' `env -C`, `sudo -D`, `run0 -D` and the
    /// program's own `-C DIR` (`git -C`, `make -C`, `tar -C`).
    pub program: PathBuf,
}

/// The [`Workdir`] of each of `segments` (a line's [`simple_commands`]), in
/// order. The shell starts in `cwd` and follows `cd DIR` (`cd` alone goes
/// to `home`, `cd -` back), `pushd DIR` and `popd`; `resolve` makes a
/// directory word a path against the directory it is read in. One reading
/// for the hook's write targets and its `skipPaths` check (WP-071).
pub fn workdirs(
    segments: &[Segment],
    cwd: &Path,
    home: &Path,
    resolve: impl Fn(&str, &Path) -> PathBuf,
) -> Vec<Workdir> {
    let mut shell = cwd.to_path_buf();
    let mut previous = shell.clone();
    let mut stack: Vec<PathBuf> = Vec::new();
    let mut out = Vec::with_capacity(segments.len());
    for segment in segments {
        let unwrapped = unwrap_command(&segment.words);
        let argv = unwrapped.argv;
        let mut program = shell.clone();
        for dir in unwrapped.chdirs.into_iter().chain(program_dirs(argv)) {
            program = resolve(dir, &program);
        }
        out.push(Workdir {
            shell: shell.clone(),
            program,
        });
        let name = argv.first().map(|w| w.rsplit('/').next().unwrap_or(w));
        let operand = argv
            .get(1..)
            .unwrap_or_default()
            .iter()
            .find(|a| *a == "-" || !a.starts_with('-'))
            .map(String::as_str);
        match (name, operand) {
            (Some("cd"), Some("-")) => std::mem::swap(&mut shell, &mut previous),
            (Some("cd"), dir) => {
                let to = dir.map_or_else(|| home.to_path_buf(), |d| resolve(d, &shell));
                previous = std::mem::replace(&mut shell, to);
            }
            // `pushd +N` rotates the stack: not followed
            (Some("pushd"), Some(dir)) if !dir.starts_with('+') => {
                let to = resolve(dir, &shell);
                stack.push(std::mem::replace(&mut shell, to));
            }
            (Some("popd"), None) => {
                if let Some(dir) = stack.pop() {
                    shell = dir;
                }
            }
            _ => {}
        }
    }
    out
}

/// The `-C DIR` words a program runs in: `git`'s before its sub-command
/// (after it, `commit -C` names a commit), anywhere for the others (`make
/// -C`, `tar -C`); none for the programs whose `-C` is something else
/// (`install -C` compares, `grep -C` and `diff -C` count lines, `ls -C`
/// sets columns).
fn program_dirs(argv: &[String]) -> Vec<&str> {
    let Some((program, args)) = argv.split_first() else {
        return Vec::new();
    };
    match program.rsplit('/').next().unwrap_or(program) {
        "git" => {
            let mut dirs = Vec::new();
            let mut it = args.iter();
            while let Some(a) = it.next() {
                match a.as_str() {
                    "-C" => dirs.extend(it.next().map(String::as_str)),
                    "-c" | "--git-dir" | "--work-tree" | "--namespace" | "--config-env" => {
                        it.next();
                    }
                    a if a.starts_with('-') => {}
                    _ => break,
                }
            }
            dirs
        }
        "install" | "grep" | "egrep" | "fgrep" | "diff" | "ls" => Vec::new(),
        _ => args
            .windows(2)
            .filter(|p| p[0] == "-C")
            .map(|p| p[1].as_str())
            .collect(),
    }
}

/// The variables a line sets to one literal value — `F=x` alone, or after
/// `export`, `local`, `declare`, `readonly`, `typeset` — for reading
/// `$F`/`${F}` in its other words. A variable set twice, set to a value
/// with an unknown part, or set only for one command (`F=x cmd`) is
/// unknown.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Vars {
    /// Name and value; `None` when the line sets it but not to one literal.
    set: Vec<(String, Option<String>)>,
}

/// A word of a line, read with the line's own [`Vars`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Word {
    /// No unknown part: the word with the known variables put in.
    Literal(String),
    /// A glob over every value the word can take ([`globs_overlap`]): an
    /// unknown variable or a substitution is `**`, a `[…]` class `?`, its
    /// own `*` and `?` stay.
    /// `floating`: the word starts with an unknown part, so it can be an
    /// absolute or a relative path. `only_unknown`: nothing in it is known
    /// but `/` (`$1`, `$NAME`, `$D/$F`), so it says nothing about a path.
    Pattern {
        glob: String,
        floating: bool,
        only_unknown: bool,
    },
}

impl Vars {
    /// The variables `segments` (a line's simple commands, in order) set.
    pub fn of(segments: &[Segment]) -> Vars {
        let mut vars = Vars::default();
        for s in segments {
            let words: &[String] = match s.words.first().map(String::as_str) {
                Some("export" | "local" | "declare" | "readonly" | "typeset") => &s.words[1..],
                _ if s.words.iter().all(|w| is_assignment(w)) => &s.words,
                _ => continue,
            };
            for (name, value) in words.iter().filter_map(|w| w.split_once('=')) {
                if !is_assignment(&format!("{name}=")) {
                    continue;
                }
                let value = match vars.expand(value) {
                    Word::Literal(v) => Some(v),
                    Word::Pattern { .. } => None,
                };
                match vars.set.iter_mut().find(|(n, _)| n == name) {
                    Some(entry) => entry.1 = None,
                    None => vars.set.push((name.to_string(), value)),
                }
            }
        }
        vars
    }

    /// `word` with the variables this line sets put in. `$HOME` and
    /// `${HOME}` stay as written unless the line sets `HOME`.
    pub fn expand(&self, word: &str) -> Word {
        let chars: Vec<char> = word.chars().collect();
        let mut glob = String::new();
        let mut wild = false;
        let mut floating = false;
        let mut i = 0;
        while let Some(&c) = chars.get(i) {
            // what `$…` or a substitution at `i` stands for, and its end
            // an unknown part is `\0` until the end, then `**`
            let mut unknown = |glob: &mut String| {
                floating |= glob.is_empty();
                wild = true;
                glob.push('\0');
            };
            let name_at = |from: usize| {
                let end = (from..chars.len())
                    .find(|&k| !(chars[k].is_ascii_alphanumeric() || chars[k] == '_'))
                    .unwrap_or(chars.len());
                (chars[from..end].iter().collect::<String>(), end)
            };
            match (c, chars.get(i + 1)) {
                ('$', Some('{')) => {
                    let (name, end) = name_at(i + 2);
                    match chars.get(end) {
                        Some('}') if !name.is_empty() => {
                            self.put(&name, &mut glob, &mut unknown);
                            i = end + 1;
                        }
                        // `${F:-x}`, `${#F}`: unknown
                        _ => {
                            unknown(&mut glob);
                            i = (end..chars.len())
                                .find(|&k| chars[k] == '}')
                                .map_or(chars.len(), |k| k + 1);
                        }
                    }
                }
                ('$', Some(n)) if n.is_ascii_alphabetic() || *n == '_' => {
                    let (name, end) = name_at(i + 1);
                    self.put(&name, &mut glob, &mut unknown);
                    i = end;
                }
                ('$', Some('(')) => {
                    unknown(&mut glob);
                    let pairs: Vec<(usize, char)> = chars.iter().copied().enumerate().collect();
                    i = closing(&pairs, i + 1, '(', ')');
                }
                // `$1`, `$@`, `$?`, …
                ('$', Some(n)) if n.is_ascii_digit() || "@*#?$!-".contains(*n) => {
                    unknown(&mut glob);
                    i += 2;
                }
                ('`', _) => {
                    unknown(&mut glob);
                    i = (i + 1..chars.len())
                        .find(|&k| chars[k] == '`')
                        .map_or(chars.len(), |k| k + 1);
                }
                ('*' | '?', _) => {
                    wild = true;
                    glob.push(c);
                    i += 1;
                }
                ('[', _) if chars[i + 1..].contains(&']') => {
                    wild = true;
                    glob.push('?');
                    i = (i + 1..chars.len())
                        .find(|&k| chars[k] == ']')
                        .map_or(chars.len(), |k| k + 1);
                }
                _ => {
                    glob.push(c);
                    i += 1;
                }
            }
        }
        if wild {
            Word::Pattern {
                only_unknown: glob.chars().all(|c| c == '\0' || c == '/'),
                glob: glob.replace('\0', "**"),
                floating,
            }
        } else {
            Word::Literal(glob)
        }
    }

    /// Puts the value of `name` into `glob`, or `unknown`.
    fn put(&self, name: &str, glob: &mut String, unknown: &mut impl FnMut(&mut String)) {
        match self.set.iter().find(|(n, _)| n == name) {
            Some((_, Some(value))) => glob.push_str(value),
            Some((_, None)) => unknown(glob),
            None if name == "HOME" => glob.push_str("$HOME"),
            None => unknown(glob),
        }
    }

    /// `segment` with every word and write target that is a [`Word::Literal`]
    /// once its variables are put in replaced by that literal; the others
    /// stay as written.
    fn put_in(&self, mut segment: Segment) -> Segment {
        for w in segment.words.iter_mut().chain(segment.writes.iter_mut()) {
            if w.contains('$')
                && let Word::Literal(v) = self.expand(w)
            {
                *w = v;
            }
        }
        segment
    }
}

/// Whether some path matches both globs: `*` any run of characters but
/// `/`, `?` one character but `/`, `**` any run of characters (the
/// `skipPaths` reading); whether a path word written with a glob or an
/// unknown part can name a path a pattern matches.
pub fn globs_overlap(a: &str, b: &str) -> bool {
    let (a, b) = (glob_parts(a), glob_parts(b));
    // reach[i][j]: a[..i] and b[..j] can match the same text
    let mut reach = vec![vec![false; b.len() + 1]; a.len() + 1];
    reach[0][0] = true;
    for i in 0..=a.len() {
        for j in 0..=b.len() {
            if !reach[i][j] {
                continue;
            }
            let (x, y) = (a.get(i).copied(), b.get(j).copied());
            // a repeating part may match nothing
            if x.is_some_and(Glob::repeats) {
                reach[i + 1][j] = true;
            }
            if y.is_some_and(Glob::repeats) {
                reach[i][j + 1] = true;
            }
            // a character both match: each side moves on unless it repeats
            if let (Some(x), Some(y)) = (x, y)
                && x.meets(y)
            {
                reach[i + usize::from(!x.repeats())][j + usize::from(!y.repeats())] = true;
            }
        }
    }
    reach[a.len()][b.len()]
}

/// One part of a glob.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Glob {
    Char(char),
    /// `?`
    One,
    /// `*`
    Star,
    /// `**`
    Any,
}

impl Glob {
    /// Whether the part matches any run of characters, none too.
    fn repeats(self) -> bool {
        matches!(self, Glob::Star | Glob::Any)
    }

    /// Whether some one character matches both parts.
    fn meets(self, other: Glob) -> bool {
        match (self, other) {
            (Glob::Char(x), Glob::Char(y)) => x == y,
            (Glob::Char(c), Glob::One | Glob::Star) | (Glob::One | Glob::Star, Glob::Char(c)) => {
                c != '/'
            }
            _ => true,
        }
    }
}

fn glob_parts(glob: &str) -> Vec<Glob> {
    let mut parts = Vec::new();
    let mut chars = glob.chars().peekable();
    while let Some(c) = chars.next() {
        parts.push(match c {
            '*' if chars.peek() == Some(&'*') => {
                while chars.next_if_eq(&'*').is_some() {}
                Glob::Any
            }
            '*' => Glob::Star,
            '?' => Glob::One,
            c => Glob::Char(c),
        });
    }
    parts
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(line: &str) -> Vec<Vec<String>> {
        parse_shell(line)
            .segments
            .into_iter()
            .map(|s| s.words)
            .collect()
    }

    #[test]
    fn quotes_operators_and_comments() {
        assert_eq!(
            words(r#"sed -i 's/a|b;c/d/' "$HOME/x y" && git commit -m "a \"b\"" # done"#),
            [
                vec!["sed", "-i", "s/a|b;c/d/", "$HOME/x y"],
                vec!["git", "commit", "-m", "a \"b\""],
            ]
        );
        assert_eq!(
            words("a; b || c | d & e\nf (g) $(h | i) `j; k`"),
            [
                vec!["a"],
                vec!["b"],
                vec!["c"],
                vec!["d"],
                vec!["e"],
                vec!["f"],
                vec!["g"],
                vec!["$(h | i)", "`j; k`"],
            ]
        );
        assert_eq!(words("echo a\\ b ''"), [vec!["echo", "a b", ""]]);
    }

    #[test]
    fn redirections() {
        let line =
            parse_shell("echo x >~/.config/hypr/a.conf 2>&1 >> b < c 2>/dev/null &> d | tee -a e");
        assert_eq!(line.segments[0].words, ["echo", "x"]);
        assert_eq!(
            line.segments[0].writes,
            ["~/.config/hypr/a.conf", "b", "/dev/null", "d"]
        );
        assert_eq!(line.segments[1].words, ["tee", "-a", "e"]);
    }

    #[test]
    fn heredoc_bodies_are_cut() {
        let line = parse_shell(
            "cat > ~/.config/hypr/x.conf <<'EOF'\nsecret = hunter2\nEOF\necho done\ncat <<-END | wc -l\n\tbody\n\tEND",
        );
        // a delimiter line stays when anything follows it
        assert_eq!(
            line.text,
            "cat > ~/.config/hypr/x.conf <<'EOF'\nEOF\necho done\ncat <<-END | wc -l"
        );
        assert!(!line.text.contains("hunter2"));
        let w: Vec<_> = line.segments.iter().map(|s| s.words.clone()).collect();
        assert_eq!(
            w,
            [
                vec!["cat"],
                vec!["echo", "done"],
                vec!["cat"],
                vec!["wc", "-l"]
            ]
        );
        assert_eq!(line.segments[0].writes, ["~/.config/hypr/x.conf"]);
        // the text reads as the same commands (attribution parses it again)
        assert_eq!(parse_shell(&line.text).segments, line.segments);
        // an unterminated heredoc swallows the rest
        assert_eq!(parse_shell("cat <<EOF\nall\nof it").text, "cat <<EOF");
        // a here-string is part of the line
        assert_eq!(parse_shell("cat <<< word").text, "cat <<< word");
    }

    /// F-536: heredocs inside `$(…)`, backticks, double quotes and `<(…)`
    /// are cut too; `<<` in arithmetic is a shift.
    #[test]
    fn heredocs_in_substitutions_and_shifts() {
        let cases = [
            (
                "echo \"$(cat <<EOF\nsecret_line ' \" )\nEOF\n)\" > b.conf\npacman -S zed",
                "echo \"$(cat <<EOF\nEOF\n)\" > b.conf\npacman -S zed",
            ),
            (
                "echo $(cat <<-EOF\n\tsecret_line\n\tEOF\n) && pacman -S zed",
                "echo $(cat <<-EOF\n\tEOF\n) && pacman -S zed",
            ),
            (
                "x=`cat <<EOF\nsecret_line `\nEOF\n`; pacman -S zed",
                "x=`cat <<EOF\nEOF\n`; pacman -S zed",
            ),
            (
                "diff <(cat <<EOF\nsecret_line\nEOF\n) x; pacman -S zed",
                "diff <(cat <<EOF\nEOF\n) x; pacman -S zed",
            ),
            (
                "(( n = 1 << 3 ))\npacman -S zed",
                "(( n = 1 << 3 ))\npacman -S zed",
            ),
            (
                "echo $((1 << 3)) $[2 << 1]\npacman -S zed",
                "echo $((1 << 3)) $[2 << 1]\npacman -S zed",
            ),
        ];
        for (line, text) in cases {
            let parsed = parse_shell(line);
            assert_eq!(parsed.text, text, "{line:?}");
            assert!(!parsed.text.contains("secret_line"), "{line:?}");
            let last = parsed.segments.last().unwrap();
            assert_eq!(last.words, ["pacman", "-S", "zed"], "{line:?}");
            assert_eq!(
                parse_shell(&parsed.text).segments,
                parsed.segments,
                "{line:?}"
            );
        }
        // the substitution stays one word, without the body
        assert_eq!(
            words("git commit -m \"$(cat <<'EOF'\nmsg\nEOF\n)\"")[0],
            ["git", "commit", "-m", "$(cat <<'EOF'\nEOF\n)"]
        );
        assert_eq!(
            words("echo \"${x:-a b}\" ${y}")[0],
            ["echo", "${x:-a b}", "${y}"]
        );
    }

    /// F-534: after `>&` a descriptor is a copy, any other word a file.
    #[test]
    fn redirect_to_a_file_with_and() {
        let writes = |line: &str| parse_shell(line).segments[0].writes.clone();
        assert_eq!(writes("echo x >& ~/.config/hypr/a"), ["~/.config/hypr/a"]);
        assert_eq!(writes("echo x >&out 2>&1"), ["out"]);
        assert_eq!(writes("echo x 1>&2 2>&- 3>&4- >& \"o p\""), ["o p"]);
        assert!(writes("echo x >&2").is_empty());
        assert_eq!(words("cat <&3 x"), [vec!["cat", "x"]]);
    }

    /// F-531: pkexec, run0 and option clusters; `sudo -k` still runs.
    #[test]
    fn more_wrappers() {
        let argv = |line: &str| parse_shell(line).segments[0].argv().to_vec();
        for line in [
            "pkexec pacman -S zed",
            "pkexec --user root --keep-cwd pacman -S zed",
            "run0 pacman -S zed",
            "run0 -u root --setenv FOO=1 --nice=5 pacman -S zed",
            "sudo -Eu root pacman -S zed",
            "sudo -uroot pacman -S zed",
            "sudo --user=root pacman -S zed",
            "sudo -iu root -- pacman -S zed",
            "sudo -k pacman -S zed",
            "doas -nu root pacman -S zed",
            "env -iu FOO pacman -S zed",
            "timeout -sKILL 10 pacman -S zed",
        ] {
            assert_eq!(argv(line), ["pacman", "-S", "zed"], "{line}");
        }
        for probe in [
            "pkexec --version",
            "run0 --help",
            "sudo -nl pacman -S zed",
            "sudo -V",
            "doas -C /etc/doas.conf pacman -S zed",
        ] {
            assert!(argv(probe).is_empty(), "{probe}");
        }
        let chdirs = |line: &str| {
            let line = parse_shell(line);
            unwrap_command(&line.segments[0].words)
                .chdirs
                .iter()
                .map(|d| d.to_string())
                .collect::<Vec<_>>()
        };
        assert_eq!(
            chdirs("sudo -D /a env -C b --chdir=c tee x"),
            ["/a", "b", "c"]
        );
        assert_eq!(chdirs("env --chdir=c tee x"), ["c"]);
        assert_eq!(chdirs("run0 -D/a tee x"), ["/a"]);
    }

    /// F-532: print-only forms and yay's own operations.
    #[test]
    fn yay_prints_and_operations() {
        let cmd = |line: &str| parse_command(&split_logged(line)).unwrap();
        for line in [
            "yay --version",
            "yay -V",
            "yay -h",
            "yay --help",
            "paru -V",
            "yay -S --help",
            "yay -Syu --help",
            "yay -Y --gendb",
            "yay --gendb",
            "yay -Ps",
            "yay -Pw",
            "yay -G zed",
            "paru --getpkgbuild zed",
            "pacman -V",
        ] {
            let c = cmd(line);
            assert!(!c.is_mutating(), "{line}");
            assert!(!c.is_full_upgrade(), "{line}");
            assert_eq!(command_intent(line), Intent::default(), "{line}");
        }
        assert!(cmd("yay -Yc").is_mutating());
        assert!(cmd("yay -Y --clean").is_mutating());
        assert!(cmd("yay").is_plain_full_upgrade());
        assert_eq!(cmd("yay -G zed").op, Some(Op::GetPkgbuild));
        assert_eq!(cmd("yay -Ps").op, Some(Op::Show));
    }

    /// F-530: the intent is read with the hook's parser.
    #[test]
    fn intent_reads_like_the_hook() {
        for line in [
            "sudo -u root pacman -S zed",
            "timeout 600 yay -S zed",
            "nice -n 10 pacman -S zed",
            "time pacman -S zed",
            "nohup yay -S zed",
            "bash -c 'yay -S zed'",
            "sudo sh -lc \"pacman -S zed\"",
            "env -u FOO pacman -S zed",
            "pkexec pacman -S zed",
            "P=zed; pacman -S \"$P\"",
            "cat <<EOF\nnot this\nEOF\npacman -S zed",
        ] {
            assert_eq!(command_intent(line).packages, ["zed"], "{line}");
        }
        // a heredoc body is no command
        assert_eq!(
            command_intent("cat <<EOF\npacman -S zed\nEOF"),
            Intent::default()
        );
        assert_eq!(command_intent("omarchy-pkg-present zed"), Intent::default());
        assert_eq!(
            command_intent("omarchy update available"),
            Intent::default()
        );
        assert_eq!(
            command_intent("omarchy-pkg-aur-install zed").packages,
            ["zed"]
        );
        // `-` reads the names from stdin
        assert_eq!(command_intent("pacman -S - < list.txt"), Intent::default());
        assert_eq!(command_intent("yay -S --needed - zed").packages, ["zed"]);
        assert!(command_intent("omarchy-update-system-pkgs").full_upgrade);
    }

    #[test]
    fn working_directories() {
        let dirs = |line: &str| -> Vec<(String, String)> {
            let segments = simple_commands(&parse_shell(line));
            workdirs(&segments, Path::new("/w"), Path::new("/h"), |w, d| {
                d.join(w)
            })
            .into_iter()
            .map(|d| {
                (
                    d.shell.display().to_string(),
                    d.program.display().to_string(),
                )
            })
            .collect()
        };
        let pair = |s: &str, p: &str| (s.to_string(), p.to_string());
        assert_eq!(
            dirs("cd a; pushd b; x; popd; y; cd; z; cd -; q"),
            [
                pair("/w", "/w"),
                pair("/w/a", "/w/a"),
                pair("/w/a/b", "/w/a/b"),
                pair("/w/a/b", "/w/a/b"),
                pair("/w/a", "/w/a"),
                pair("/w/a", "/w/a"),
                pair("/h", "/h"),
                pair("/h", "/h"),
                pair("/w/a", "/w/a"),
            ]
        );
        assert_eq!(
            dirs("git -C r commit -C HEAD; make -C m; env -C e tee x; install -C a b"),
            [
                pair("/w", "/w/r"),
                pair("/w", "/w/m"),
                pair("/w", "/w/e"),
                pair("/w", "/w"),
            ]
        );
    }

    #[test]
    fn variables_of_a_line() {
        let vars = |line: &str| line_vars(&parse_shell(line));
        let v = vars("D=~/d; export F=$D/a.conf G=\"x y\"; H=1; H=2; I=$(pwd); J=x cmd");
        let lit = |s: &str| Word::Literal(s.to_string());
        let pat = |s: &str, floating: bool| Word::Pattern {
            glob: s.to_string(),
            floating,
            only_unknown: false,
        };
        let unknown = |s: &str| Word::Pattern {
            glob: s.to_string(),
            floating: true,
            only_unknown: true,
        };
        assert_eq!(v.expand("$F"), lit("~/d/a.conf"));
        assert_eq!(v.expand("${G}/z"), lit("x y/z"));
        assert_eq!(v.expand("$HOME/x"), lit("$HOME/x"));
        assert_eq!(v.expand("$H/x"), pat("**/x", true), "set twice");
        assert_eq!(v.expand("$I/x"), pat("**/x", true), "not a literal");
        assert_eq!(v.expand("~/$J"), pat("~/**", false), "only for one command");
        assert_eq!(v.expand("~/d/p*.c[o]nf"), pat("~/d/p*.c?nf", false));
        assert_eq!(v.expand("$(pwd)/x"), pat("**/x", true));
        assert_eq!(v.expand("${F:-y}"), unknown("**"));
        assert_eq!(v.expand("$1"), unknown("**"));
        assert_eq!(v.expand("$A/$B"), unknown("**/**"));
        // a glob of its own is no unknown value
        assert_eq!(v.expand("*"), pat("*", false));
        assert_eq!(v.expand("$A*"), pat("***", true));
        assert_eq!(v.expand("a$1b"), pat("a**b", false));
        // the words of the line carry the values
        let segments = simple_commands(&parse_shell("F=~/x; tee $F > \"$F.bak\""));
        assert_eq!(segments[1].words, ["tee", "~/x"]);
        assert_eq!(segments[1].writes, ["~/x.bak"]);
    }

    #[test]
    fn overlapping_globs() {
        assert!(globs_overlap("/h/d/priv*.conf", "/h/d/private.conf"));
        assert!(globs_overlap("**/private.conf", "/h/d/private.conf"));
        assert!(globs_overlap("/h/d/?rivate.conf", "/h/*/private.*"));
        assert!(globs_overlap("/h/d/x", "/h/d/**"));
        assert!(globs_overlap("/h/**/x", "/h/a/b/x"));
        assert!(
            !globs_overlap("/h/d/*.txt", "/h/d/private.conf/**"),
            "* stays in a directory"
        );
        assert!(!globs_overlap("/h/d/*.txt", "/h/d/private.conf"));
        assert!(!globs_overlap("/h/e/*", "/h/d/private.conf"));
        assert!(!globs_overlap("/h/d/?", "/h/d/ab"));
        assert!(globs_overlap("*", "*"));
        assert!(globs_overlap("", ""));
        assert!(!globs_overlap("a", ""));
    }

    #[test]
    fn wrappers_are_skipped() {
        let argv = |line: &str| parse_shell(line).segments[0].argv().to_vec();
        assert_eq!(
            argv("FOO=1 sudo -E -u root env -i BAR=2 nice -n 5 pacman -S zed"),
            ["pacman", "-S", "zed"]
        );
        assert_eq!(argv("timeout -s KILL 10 yay"), ["yay"]);
        assert_eq!(argv("command yay -Syu"), ["yay", "-Syu"]);
        assert!(argv("FOO=1").is_empty());
    }

    #[test]
    fn probes_run_nothing() {
        // `command -v yay` must not read as bare `yay` (a full upgrade)
        for probe in [
            "command -v yay",
            "command -V pacman",
            "command -pv yay",
            "sudo -l pacman -Syu",
            "sudo -v",
        ] {
            let line = parse_shell(probe);
            assert!(line.segments[0].argv().is_empty(), "{probe}");
            assert_eq!(command_intent(probe), Intent::default(), "{probe}");
        }
        for probe in ["type yay", "which pacman", "hash yay", "whereis paru"] {
            let line = parse_shell(probe);
            let argv: Vec<&str> = line.segments[0].argv().iter().map(String::as_str).collect();
            assert!(parse_command(&argv).is_none(), "{probe}");
            assert_eq!(command_intent(probe), Intent::default(), "{probe}");
        }
        assert_eq!(
            command_intent("command -v yay && yay -S zed").packages,
            ["zed"]
        );
        assert!(!command_intent("command -v yay && yay -S zed").full_upgrade);
    }

    /// `write_targets` of the first simple command of `line`, as strings.
    fn targets(line: &str) -> Vec<String> {
        let line = parse_shell(line);
        let s = &line.segments[0];
        write_targets(s.argv(), &s.writes)
            .iter()
            .map(|t| match t {
                Target::File(w) => format!("file {w}"),
                Target::Into(w) => format!("into {w}"),
                Target::Tree(w) => format!("tree {w}"),
            })
            .collect()
    }

    #[test]
    fn what_writers_write() {
        assert_eq!(
            targets("sed -i -e s/a/b/ -e s/c/d/ f1 f2"),
            ["file f1", "file f2"]
        );
        assert_eq!(targets("sed -Ei s/a/b/ f1"), ["file f1"]);
        assert!(targets("sed -n s/a/b/p f1").is_empty(), "not in place");
        assert_eq!(targets("tee -a x y"), ["file x", "file y"]);
        assert_eq!(targets("cp a b dir/"), ["into dir/"]);
        assert_eq!(targets("cp -t dir a b"), ["into dir"]);
        assert_eq!(targets("cp --target-directory=dir a"), ["into dir"]);
        assert!(targets("cp a").is_empty());
        // a move removes its sources
        assert_eq!(targets("mv a b c"), ["into c", "file a", "file b"]);
        assert_eq!(targets("install -m 644 a b"), ["into b"]);
        assert_eq!(targets("install -d -m 755 d1 d2"), ["file d1", "file d2"]);
        assert_eq!(targets("install -dm755 d1"), ["file d1"]);
        assert_eq!(targets("install -Dm644 a b"), ["into b"], "-D is not -d");
        assert_eq!(targets("ln -sf a b"), ["into b"]);
        assert_eq!(targets("rm -rf a b"), ["tree a", "tree b"]);
        assert_eq!(targets("truncate -s 0 f"), ["file f"]);
        assert_eq!(targets("echo x > a 2>&1 >> b"), ["file a", "file b"]);
        assert_eq!(
            targets("Edit ~/.config/hypr/a.conf"),
            ["file ~/.config/hypr/a.conf"]
        );
        // reading is not writing
        assert!(targets("cat ~/.config/hypr/a.conf").is_empty());
        assert!(targets("grep -r x ~/.config").is_empty());
    }

    #[test]
    fn nested_shells_are_opened() {
        let words = |line: &str| -> Vec<Vec<String>> {
            simple_commands(&parse_shell(line))
                .into_iter()
                .map(|s| s.argv().to_vec())
                .collect()
        };
        assert_eq!(
            words("bash -c 'sed -i s/a/b/ x && yay -S zed'"),
            [vec!["sed", "-i", "s/a/b/", "x"], vec!["yay", "-S", "zed"]]
        );
        assert_eq!(
            words("sudo sh -lc \"pacman -Syu\""),
            [vec!["pacman", "-Syu"]]
        );
        assert_eq!(words("zsh -o pipefail -c 'tee x'"), [vec!["tee", "x"]]);
        assert_eq!(words("eval 'rm -rf' y"), [vec!["rm", "-rf", "y"]]);
        // a script file, not a command string
        assert_eq!(words("bash script.sh"), [vec!["bash", "script.sh"]]);
        // the outer redirection stays
        let line = parse_shell("sh -c 'echo x' > out");
        let segments = simple_commands(&line);
        assert_eq!(segments.len(), 2);
        assert_eq!(segments[1].writes, ["out"]);
        // nesting is bounded
        let deep = "sh -c \"sh -c 'sh -c \\\"sh -c yay\\\"'\"";
        assert_eq!(words(deep)[0][0], "sh");
    }

    /// ADR-0039: which privilege wrapper a command runs under, also inside
    /// the shell a wrapper started; a probe runs nothing and has none.
    #[test]
    fn the_privilege_wrapper_is_named() {
        let privileges = |line: &str| -> Vec<(String, Option<&'static str>)> {
            simple_commands(&parse_shell(line))
                .into_iter()
                .map(|s| (s.argv().join(" "), s.privilege()))
                .collect()
        };
        let one = |line: &str| privileges(line).remove(0);
        let cmd = |argv: &str, w: Option<&'static str>| (argv.to_string(), w);
        assert_eq!(
            one("pkexec lpadmin -p X -E"),
            cmd("lpadmin -p X -E", Some("pkexec"))
        );
        assert_eq!(one("sudo -u root nmcli c up x").1, Some("sudo"));
        assert_eq!(one("doas -n lpadmin -x X").1, Some("doas"));
        assert_eq!(one("run0 --user=root lpadmin -x X").1, Some("run0"));
        assert_eq!(one("/usr/bin/pkexec lpadmin -x X").1, Some("pkexec"));
        // behind other wrappers, after assignments; the first one counts
        assert_eq!(one("FOO=1 env -i nice sudo lpadmin").1, Some("sudo"));
        assert_eq!(one("sudo -E pkexec lpadmin").1, Some("sudo"));
        assert_eq!(one("lpadmin -p X"), cmd("lpadmin -p X", None));
        assert_eq!(one("env nice lpadmin").1, None);
        // inside a shell a wrapper started, after `&&`, in `eval`
        assert_eq!(
            privileges("pkexec sh -c 'lpadmin -p X && cupsenable X'"),
            [
                cmd("lpadmin -p X", Some("pkexec")),
                cmd("cupsenable X", Some("pkexec"))
            ]
        );
        assert_eq!(
            privileges("bash -lc 'true && sudo lpadmin -x X'"),
            [cmd("true", None), cmd("lpadmin -x X", Some("sudo"))]
        );
        assert_eq!(one("eval sudo lpadmin -x X").1, Some("sudo"));
        // the outer shell writes the redirection, not the wrapper
        let line = simple_commands(&parse_shell("sudo sh -c 'lpadmin -x X' > out"));
        assert_eq!(line[1].writes, ["out"]);
        assert_eq!(line[1].privilege(), None);
        // a variable the line sets
        assert_eq!(one("S=sudo; $S lpadmin -x X").1, None, "the assignment");
        assert_eq!(privileges("S=sudo; $S lpadmin -x X")[1].1, Some("sudo"));
        // probes run nothing
        for probe in [
            "sudo -l",
            "sudo -v",
            "sudo -K",
            "pkexec --version",
            "pkexec --help",
            "run0 --help",
            "doas -C /etc/doas.conf lpadmin",
            "command -v sudo",
        ] {
            assert_eq!(one(probe), cmd("", None), "{probe}");
        }
    }
}
