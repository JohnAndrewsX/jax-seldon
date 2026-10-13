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

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;

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
/// counts as a target — that errs towards red (ADR-0013 §3). pacman's,
/// then yay's (`man 8 yay`) and paru's (WP-113: `yay -Syu --answerdiff
/// None` names no package). Options whose value is optional (paru's
/// `--chroot`, `--localrepo`, `--sign`) take it only as `--opt=value`.
const LONG_WITH_ARG: [&str; 46] = [
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
    // yay
    "aururl",
    "aurrpcurl",
    "builddir",
    "editor",
    "editorflags",
    "makepkg",
    "pacman",
    "git",
    "gitflags",
    "gpg",
    "gpgflags",
    "makepkgconf",
    "requestsplitn",
    "completioninterval",
    "sortby",
    "searchby",
    "answerclean",
    "answerdiff",
    "answeredit",
    "answerupgrade",
    "mflags",
    "sudo",
    "sudoflags",
    // paru
    "clonedir",
    "pacman-conf",
    "fm",
    "fmflags",
    "bat",
    "batflags",
    "limit",
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

/// Shell reserved words a command can follow in one simple command as the
/// parser splits it (`for p in a; do sudo lpadmin …; done`, `if ! sudo …`):
/// read past like an assignment, so the command after them is the one
/// classified.
const RESERVED_BEFORE_COMMAND: [&str; 9] = [
    "if", "then", "elif", "else", "do", "while", "until", "!", "{",
];

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
    /// The line itself can feed the command (WP-157): a `|` or `|&` (not
    /// `||`), a here-string (`<<<`) or a process substitution (`<(`)
    /// stands before its first word on the line, or among its own words
    /// (`passwd alice <<< PW`, `--key-file <(echo PW)`); also every
    /// command of a `sh -c` or `eval` that is fed itself. Sticky, not
    /// scoped: the parser reads compounds flat, so a pipe into `( … )`,
    /// `while … done`, `if … fi` or `>( … )` feeds every command after it
    /// (WP-157 round 2), and so does an unrelated one (`a | b; passwd
    /// -S x`: masked too much). A pipe after the command feeds it nothing
    /// (`passwd -S alice | grep x`). A heredoc is cut from the record and
    /// feeds nothing.
    pub fed: bool,
    /// The `$(…)`, backtick or `<(…)` substitution the command runs in,
    /// numbered on the line from 1 as each one opens, one number for each
    /// on the line, those of an opened `sh -c` or `eval` script included
    /// ([`simple_commands`]); 0 for a command of the line itself (WP-209).
    /// Its commands come before the command it stands in, as the shell
    /// runs them; it starts in the directories of the one around it, and
    /// a `cd` among its commands moves no command outside it
    /// ([`workdirs`]; round 2).
    pub substitution: usize,
    /// The substitution around [`Segment::substitution`] (0: the line).
    pub parent: usize,
    /// An `sh -c` or `eval` script this command was opened from nests a
    /// substitution too deep to read ([`ShellLine::too_deep`],
    /// [`too_deep`]; WP-209 round 2).
    pub too_deep: bool,
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
    /// A substitution nests deeper than [`SUBSTITUTION_DEPTH_MAX`]: it was
    /// skipped unread, its commands, feeds and heredocs unknown, so the
    /// hook records the line as `<program> ‹redacted›` (WP-209).
    pub too_deep: bool,
}

/// How deep `$(…)`, backticks and `<(…)` are read as commands (WP-209): a
/// deeper one is skipped to its end without recursion, so a line nested
/// thousands deep neither overflows the stack nor costs more than this
/// many copies of its text.
pub const SUBSTITUTION_DEPTH_MAX: usize = 16;

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
        skips: Vec::new(),
        substitutions: 0,
        depth: 0,
        too_deep: false,
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
        too_deep: reader.too_deep,
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
    /// The substitutions skipped unread ([`SUBSTITUTION_DEPTH_MAX`]), in
    /// the order of the line: from and to which character, and the
    /// brackets that stand for the text in a word (round 2).
    skips: Vec<(usize, usize, &'static str)>,
    /// The substitutions opened so far ([`Segment::substitution`]).
    substitutions: usize,
    /// The substitutions being read around the current position.
    depth: usize,
    /// One was nested too deep to read ([`ShellLine::too_deep`]).
    too_deep: bool,
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
                    // `|` and `|&` feed every command after them, `||` does not
                    lexer.fed |= c == '|' && self.at(i + 1) != Some('|');
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
                        // a here-string: this command's, and an `exec 3<<<`
                        // a later one reads through the descriptor
                        lexer.pending = Some(Pending::Read);
                        lexer.current.fed = true;
                        lexer.fed = true;
                        i += 2;
                    } else if self.at(i) == Some('<') {
                        i += 1;
                        let strip_tabs = self.at(i) == Some('-');
                        if strip_tabs {
                            i += 1;
                        }
                        lexer.pending = Some(Pending::Heredoc { strip_tabs });
                    } else if self.at(i) == Some('(') {
                        // `<(…)`: its commands are the line's, its text is
                        // not kept; it feeds this command and, through
                        // `exec 3< <(…)`, a later one
                        let mut inner = self.inside(lexer);
                        lexer.current.fed = true;
                        lexer.fed = true;
                        i = if self.depth >= SUBSTITUTION_DEPTH_MAX {
                            self.too_deep = true;
                            let end = skip_end(&self.chars, i + 1, Stop::Paren);
                            self.skips.push((i, end, "(…)"));
                            end
                        } else {
                            self.depth += 1;
                            let end = self.read(i + 1, Stop::Paren, &mut inner);
                            self.depth -= 1;
                            end
                        };
                        Reader::close(inner, lexer);
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
    /// bodies it holds; returns the index after it. The commands of a
    /// `$(…)` or backticks are the line's ([`Reader::inside`], WP-209); a `|`
    /// (not `||`), `<<<` or `<(` inside them feeds the line from there on,
    /// as one outside does ([`Segment::fed`]; `--key-file $(printf PW |
    /// tee k; echo k)`, WP-157 round 3). `$((…))`, `$[…]` and `${…}` run
    /// no command of their own and are not opened, nor is one nested
    /// deeper than [`SUBSTITUTION_DEPTH_MAX`] ([`ShellLine::too_deep`]).
    fn substitution(&mut self, i: usize, lexer: &mut Lexer) -> usize {
        let mut inner = self.inside(lexer);
        let deep = self.depth >= SUBSTITUTION_DEPTH_MAX;
        self.depth += 1;
        // a substitution skipped unread leaves only its brackets in the
        // word, so its text (a secret too) names no program (round 2)
        let mut skipped = None;
        let end = match (self.at(i), self.at(i + 1), self.at(i + 2)) {
            (Some('`'), ..) if deep => {
                self.too_deep = true;
                skipped = Some("`…`");
                skip_end(&self.chars, i + 1, Stop::Backtick)
            }
            (Some('`'), ..) => self.read(i + 1, Stop::Backtick, &mut inner),
            (_, Some('('), Some('(')) => closing(&self.chars, i + 1, '(', ')'),
            (_, Some('('), _) if deep => {
                self.too_deep = true;
                skipped = Some("$(…)");
                skip_end(&self.chars, i + 2, Stop::Paren)
            }
            (_, Some('('), _) => self.read(i + 2, Stop::Paren, &mut inner),
            (_, Some('['), _) => closing(&self.chars, i + 1, '[', ']'),
            _ => closing(&self.chars, i + 1, '{', '}'),
        };
        self.depth -= 1;
        Reader::close(inner, lexer);
        if let Some(brackets) = skipped {
            self.skips.push((i, end, brackets));
        }
        // the text into the word, a skipped substitution's as its brackets
        let mut skip = self.skips.partition_point(|&(from, ..)| from < i);
        let mut k = i;
        while k < end {
            if let Some(&(from, to, brackets)) = self.skips.get(skip)
                && from == k
            {
                brackets.chars().for_each(|c| lexer.push(c));
                k = to;
                skip += 1;
                continue;
            }
            let (b, c) = self.chars[k];
            if !self.cuts.iter().any(|&(s, e)| s <= b && b < e) {
                lexer.push(c);
            }
            k += 1;
        }
        end
    }

    /// The lexer of a substitution that opens inside `outer`'s current
    /// command: its commands go on the line's list, before that command,
    /// numbered as the next substitution ([`Segment::substitution`]; one
    /// list for every depth, so nesting costs no copy). It is fed when the
    /// line feeds that command so far (`echo PW | x $(passwd …)`: the
    /// substitution reads the pipe too).
    fn inside(&mut self, outer: &mut Lexer) -> Lexer {
        self.substitutions += 1;
        Lexer {
            segments: std::mem::take(&mut outer.segments),
            fed: outer.fed,
            substitution: self.substitutions,
            parent: outer.substitution,
            ..Lexer::default()
        }
    }

    /// Ends the substitution `inner` read ([`Reader::inside`]): its
    /// commands back on `outer`'s list, and its feed: what a substitution
    /// feeds, the line reads from there on.
    fn close(mut inner: Lexer, outer: &mut Lexer) {
        inner.end_segment();
        outer.segments = inner.segments;
        outer.fed |= inner.fed;
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

/// The index after the end of a substitution skipped unread (deeper than
/// [`SUBSTITUTION_DEPTH_MAX`]) whose text starts at `at`: the `)` (`stop`
/// [`Stop::Paren`]) or backtick ([`Stop::Backtick`]) that closes it,
/// outside quotes, nested parentheses and nested backticks (`` `a $(b
/// `c`)` ``), else the end. One pass, no recursion; a backslash escapes
/// the character after it outside single quotes.
fn skip_end(chars: &[(usize, char)], at: usize, stop: Stop) -> usize {
    let mut parens = 0usize;
    let mut backticks = 0usize;
    let mut quote: Option<char> = None;
    let mut i = at;
    while let Some(&(_, c)) = chars.get(i) {
        i += 1;
        match (quote, c) {
            (Some('\''), '\'') => quote = None,
            (Some('\''), _) => {}
            (_, '\\') => i += 1,
            (Some(_), '"') => quote = None,
            (Some(_), _) => {}
            (None, '\'' | '"') => quote = Some(c),
            (None, '(') => parens += 1,
            (None, ')') if parens > 0 => parens -= 1,
            (None, ')') if stop == Stop::Paren && backticks == 0 => return i,
            (None, '`') if stop == Stop::Backtick && parens == 0 => return i,
            // a backtick substitution nested inside parentheses
            (None, '`') => backticks ^= 1,
            _ => {}
        }
    }
    chars.len()
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
    /// A `|`, `|&`, `<<<` or `<(` stood earlier on the line: every command
    /// from here on is [`Segment::fed`].
    fed: bool,
    /// The substitution this lexer reads ([`Segment::substitution`]).
    substitution: usize,
    /// The one around it ([`Segment::parent`]).
    parent: usize,
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
        let mut segment = std::mem::take(&mut self.current);
        if !segment.words.is_empty() || !segment.writes.is_empty() {
            segment.fed |= self.fed;
            segment.substitution = self.substitution;
            segment.parent = self.parent;
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
    /// Short options whose value is optional: only the rest of the
    /// cluster (`xargs -i{}`, `-l1`, `-eEND`), never the next word.
    short_attached: &'static str,
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
    /// It runs a command of its own when given none (`xargs` runs
    /// `echo`): it is then the command.
    alone: bool,
}

/// `--help` and `--version`: print and exit.
const PRINTS: &[&str] = &["--help", "--version"];

/// The wrappers [`command_argv`] reads past. `sudo -k` drops the cached
/// credentials and still runs the command; `-l`, `-v`, `-K`, `-V` do not.
const WRAPPERS: [Wrapper; 14] = [
    Wrapper {
        name: "sudo",
        short_value: "ughpCDrtUTR",
        short_attached: "",
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
        alone: false,
    },
    Wrapper {
        name: "doas",
        short_value: "u",
        short_attached: "",
        long_value: &[],
        // `-C config`: only checks the rule; `-L`: forgets the login
        probe_short: "CL",
        probe_long: &[],
        chdir: None,
        operands: 0,
        alone: false,
    },
    Wrapper {
        name: "pkexec",
        short_value: "u",
        short_attached: "",
        long_value: &["--user"],
        probe_short: "",
        probe_long: PRINTS,
        chdir: None,
        operands: 0,
        alone: false,
    },
    Wrapper {
        name: "run0",
        short_value: "ugD",
        short_attached: "",
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
        alone: false,
    },
    Wrapper {
        name: "env",
        short_value: "uCS",
        short_attached: "",
        long_value: &["--unset", "--chdir", "--split-string"],
        probe_short: "",
        probe_long: PRINTS,
        chdir: Some(('C', "--chdir")),
        operands: 0,
        alone: false,
    },
    Wrapper {
        name: "nice",
        short_value: "n",
        short_attached: "",
        long_value: &["--adjustment"],
        probe_short: "",
        probe_long: PRINTS,
        chdir: None,
        operands: 0,
        alone: false,
    },
    Wrapper {
        name: "timeout",
        short_value: "sk",
        short_attached: "",
        long_value: &["--signal", "--kill-after"],
        probe_short: "",
        probe_long: PRINTS,
        chdir: None,
        operands: 1,
        alone: false,
    },
    Wrapper {
        name: "exec",
        short_value: "a",
        short_attached: "",
        long_value: &[],
        probe_short: "",
        probe_long: &[],
        chdir: None,
        operands: 0,
        alone: false,
    },
    Wrapper {
        name: "command",
        short_value: "",
        short_attached: "",
        long_value: &[],
        probe_short: "vV",
        probe_long: &[],
        chdir: None,
        operands: 0,
        alone: false,
    },
    Wrapper {
        name: "nohup",
        short_value: "",
        short_attached: "",
        long_value: &[],
        probe_short: "",
        probe_long: PRINTS,
        chdir: None,
        operands: 0,
        alone: false,
    },
    // GNU xargs (WP-157): the program it runs gets the words from stdin as
    // further arguments (`… | sudo xargs chpasswd`)
    Wrapper {
        name: "xargs",
        short_value: "adEILnPs",
        short_attached: "eil",
        long_value: &[
            "--arg-file",
            "--delimiter",
            "--max-args",
            "--max-procs",
            "--max-chars",
            "--process-slot-var",
        ],
        probe_short: "",
        probe_long: PRINTS,
        chdir: None,
        operands: 0,
        alone: true,
    },
    Wrapper {
        name: "time",
        short_value: "fo",
        short_attached: "",
        long_value: &["--format", "--output"],
        probe_short: "",
        probe_long: PRINTS,
        chdir: None,
        operands: 0,
        alone: false,
    },
    // coreutils' stdbuf and util-linux's setsid (WP-209): `… | sudo
    // stdbuf -i0 cryptsetup open …` runs cryptsetup
    Wrapper {
        name: "stdbuf",
        short_value: "ioe",
        short_attached: "",
        long_value: &["--input", "--output", "--error"],
        probe_short: "",
        probe_long: PRINTS,
        chdir: None,
        operands: 0,
        alone: false,
    },
    Wrapper {
        name: "setsid",
        short_value: "",
        short_attached: "",
        long_value: &[],
        probe_short: "hV",
        probe_long: PRINTS,
        chdir: None,
        operands: 0,
        alone: false,
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
    /// A wrapper reads the password from stdin ([`STDIN_PASSWORD`]: `echo
    /// PW | sudo -S …`), also when it only probes (`sudo -S -v`): the
    /// password is then on the command line.
    pub password_on_stdin: bool,
}

/// The wrapper option that reads the password from stdin: sudo's `-S`,
/// `--stdin` (getopt also takes `--st`, `--std`, `--stdi`). `doas`,
/// `pkexec` and `run0` have none: doas asks on the terminal, pkexec and
/// run0 through the polkit agent.
const STDIN_PASSWORD: (&str, char, &str) = ("sudo", 'S', "--stdin");

/// Whether the long option `option` of `wrapper` is [`STDIN_PASSWORD`]'s,
/// written whole or as an unambiguous prefix.
fn is_stdin_password_long(wrapper: &str, option: &str) -> bool {
    let (name, _, long) = STDIN_PASSWORD;
    wrapper == name && option.len() >= "--st".len() && long.starts_with(option)
}

/// [`command_argv`], with the directories the wrappers move to.
pub fn unwrap_command(words: &[String]) -> Unwrapped<'_> {
    let mut chdirs = Vec::new();
    let mut privilege = None;
    let mut password_on_stdin = false;
    let mut i = 0;
    while let Some(w) = words.get(i) {
        if is_assignment(w) || RESERVED_BEFORE_COMMAND.contains(&w.as_str()) {
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
        let at = i;
        i += 1;
        // a probe runs nothing; its other options are still read, for a
        // password on stdin (`sudo -vS`)
        let mut probing = false;
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
                password_on_stdin |= is_stdin_password_long(wrapper.name, option);
                if wrapper.probe_long.contains(&option) {
                    probing = true;
                    continue;
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
                password_on_stdin |= (wrapper.name, letter) == (STDIN_PASSWORD.0, STDIN_PASSWORD.1);
                if wrapper.probe_short.contains(letter) {
                    probing = true;
                    continue;
                }
                if wrapper.short_attached.contains(letter) {
                    // the rest of the cluster is its value
                    break;
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
        if probing {
            return Unwrapped {
                password_on_stdin,
                ..Unwrapped::default()
            };
        }
        i += wrapper.operands;
        if wrapper.alone && i >= words.len() {
            // `… | sudo xargs`: xargs runs `echo` itself
            i = at;
            break;
        }
    }
    Unwrapped {
        argv: &words[i.min(words.len())..],
        chdirs,
        privilege,
        password_on_stdin,
    }
}

/// `words` after leading `VAR=value` assignments and the wrappers that run
/// their arguments as a command (`sudo`, `doas`, `pkexec`, `run0`, `env`,
/// `command`, `exec`, `nohup`, `time`, `nice`, `timeout`, `xargs`,
/// `stdbuf`, `setsid`), with the
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
    /// `next` is the highest substitution number given out so far: a
    /// script's own numbers come after it, and its commands outside a
    /// substitution of its own stay in the one the shell command stands
    /// in (round 2). A script that nests too deep marks every command it
    /// opens, or a command of its own when it has none.
    fn open(segment: &Segment, depth: usize, next: &mut usize, out: &mut Vec<Segment>) {
        match nested_script(segment.argv()).filter(|_| depth < NESTING_MAX) {
            Some(script) => {
                let privilege = segment.privilege();
                let parsed = parse_shell(&script);
                let base = *next;
                let number = |n: usize| {
                    if n == 0 {
                        segment.substitution
                    } else {
                        base + n
                    }
                };
                *next += parsed
                    .segments
                    .iter()
                    .map(|s| s.substitution)
                    .max()
                    .unwrap_or(0);
                let too_deep = parsed.too_deep || segment.too_deep;
                if too_deep && parsed.segments.is_empty() {
                    out.push(Segment {
                        too_deep,
                        substitution: segment.substitution,
                        parent: segment.parent,
                        ..Segment::default()
                    });
                }
                for inner in parsed.segments {
                    let inner = Segment {
                        privileged_by: privilege,
                        fed: inner.fed || segment.fed,
                        too_deep: inner.too_deep || too_deep,
                        parent: if inner.substitution == 0 {
                            segment.parent
                        } else {
                            number(inner.parent)
                        },
                        substitution: number(inner.substitution),
                        ..inner
                    };
                    open(&inner, depth + 1, next, out);
                }
                if !segment.writes.is_empty() {
                    out.push(Segment {
                        words: Vec::new(),
                        writes: segment.writes.clone(),
                        substitution: segment.substitution,
                        parent: segment.parent,
                        ..Segment::default()
                    });
                }
            }
            None => out.push(segment.clone()),
        }
    }
    let mut next = line
        .segments
        .iter()
        .map(|s| s.substitution)
        .max()
        .unwrap_or(0);
    let mut out = Vec::new();
    for segment in &line.segments {
        open(segment, 0, &mut next, &mut out);
    }
    out
}

/// Whether a substitution of `line`, or of an `sh -c` or `eval` script it
/// opens, nests too deep to read ([`ShellLine::too_deep`]): the hook then
/// records the line as `<program> ‹redacted›` (WP-209 round 2).
pub fn too_deep(line: &ShellLine) -> bool {
    line.too_deep || opened_commands(line).iter().any(|s| s.too_deep)
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
/// for the hook's write targets and its `skipPaths` check (WP-071). The
/// commands of a substitution ([`Segment::substitution`]) run in a
/// subshell: they follow their own `cd`, and the line goes on where it was
/// before them (`D=$(cd /x && pwd); tee y`: `y` in the line's directory;
/// WP-209).
pub fn workdirs(
    segments: &[Segment],
    cwd: &Path,
    home: &Path,
    resolve: impl Fn(&str, &Path) -> PathBuf,
) -> Vec<Workdir> {
    let parents: HashMap<usize, usize> = segments
        .iter()
        .map(|s| (s.substitution, s.parent))
        .collect();
    let start = Dirs {
        shell: cwd.to_path_buf(),
        previous: cwd.to_path_buf(),
        stack: DirStack::default(),
    };
    // each substitution's directories, from the line's (0) down
    let mut states: HashMap<usize, Dirs> = HashMap::from([(0, start)]);
    let mut out = Vec::with_capacity(segments.len());
    for segment in segments {
        let Dirs {
            shell,
            previous,
            stack,
        } = Dirs::of(&mut states, &parents, segment.substitution);
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
            (Some("cd"), Some("-")) => std::mem::swap(shell, previous),
            (Some("cd"), dir) => {
                let to = dir.map_or_else(|| home.to_path_buf(), |d| resolve(d, shell));
                *previous = std::mem::replace(shell, to);
            }
            // `pushd +N` rotates the stack: not followed
            (Some("pushd"), Some(dir)) if !dir.starts_with('+') => {
                let to = resolve(dir, shell);
                *stack = stack.push(std::mem::replace(shell, to));
            }
            (Some("popd"), None) => {
                if let Some((dir, rest)) = stack.pop() {
                    *shell = dir;
                    *stack = rest;
                }
            }
            _ => {}
        }
    }
    out
}

/// The directories of the shell or a substitution: where it is, `cd -`'s
/// directory and the `pushd` stack ([`workdirs`]).
#[derive(Debug, Clone)]
struct Dirs {
    shell: PathBuf,
    previous: PathBuf,
    stack: DirStack,
}

impl Dirs {
    /// The directories of substitution `n`; on its first command a copy
    /// of those of the one around it (`parents`), as they are then
    /// (round 2: `$(cd /x; c $(d))` runs `d` in `/x`). The copy shares
    /// the `pushd` stack, so a line of many substitutions after many
    /// `pushd` stays linear.
    fn of<'s>(
        states: &'s mut HashMap<usize, Dirs>,
        parents: &HashMap<usize, usize>,
        n: usize,
    ) -> &'s mut Dirs {
        if !states.contains_key(&n) {
            // the nearest substitution around `n` with directories; the
            // line's always has them
            let mut chain = vec![n];
            let mut at = n;
            while !states.contains_key(&at) {
                at = parents.get(&at).copied().filter(|p| *p != at).unwrap_or(0);
                chain.push(at);
            }
            let seed = states[&at].clone();
            for id in chain {
                states.entry(id).or_insert_with(|| seed.clone());
            }
        }
        states.get_mut(&n).expect("inserted above")
    }
}

/// A `pushd` stack whose copies share their tails: a push or a pop makes a
/// new head, a copy costs one reference count (round 2, F3).
#[derive(Debug, Clone, Default)]
struct DirStack(Option<Rc<(PathBuf, DirStack)>>);

impl DirStack {
    fn push(&self, dir: PathBuf) -> DirStack {
        DirStack(Some(Rc::new((dir, self.clone()))))
    }

    fn pop(&self) -> Option<(PathBuf, DirStack)> {
        self.0.as_ref().map(|node| (node.0.clone(), node.1.clone()))
    }
}

impl Drop for DirStack {
    /// Frees a long stack node by node, not by recursion (`pushd` ×10 000).
    fn drop(&mut self) {
        let mut next = self.0.take();
        while let Some(node) = next {
            next = match Rc::try_unwrap(node) {
                Ok((_, mut rest)) => rest.0.take(),
                Err(_) => None,
            };
        }
    }
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
                // the commands of a substitution, before the one it stands
                // in (WP-209)
                vec!["h"],
                vec!["i"],
                vec!["j"],
                vec!["k"],
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
        // the substitution stays one word, without the body; its command
        // is read before it (WP-209)
        assert_eq!(
            words("git commit -m \"$(cat <<'EOF'\nmsg\nEOF\n)\""),
            [
                vec!["cat"],
                vec!["git", "commit", "-m", "$(cat <<'EOF'\nEOF\n)"]
            ]
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

    /// WP-109 stage 2: yay's and paru's options with a value consume it,
    /// so a full upgrade with them names no package (`omarchy update`'s
    /// AUR step and a user's `yay -Syu --answerdiff None` are routine).
    #[test]
    fn helper_options_with_a_value() {
        let cmd = |line: &str| parse_command(&split_logged(line)).unwrap();
        for line in [
            "yay -Syu --answerdiff None --answerclean None",
            "yay -Sua --noconfirm --answerupgrade None --mflags --skippgpcheck",
            "yay --editor vim --sudo doas -Syu",
            "yay -Syu --aururl https://aur.example.org --builddir /tmp/b",
            "paru -Syu --fm vifm --clonedir /tmp/c --limit 5",
            "paru -Syu --pacman-conf /etc/pacman.conf --bat bat --batflags -p",
            "paru -Syu --chroot=/tmp/root",
        ] {
            let c = cmd(line);
            assert!(c.is_plain_full_upgrade(), "{line}: {:?}", c.targets);
        }
        assert_eq!(cmd("yay -S --editor vim zed").targets, ["zed"]);
        // an unknown option still takes no value: its word is a target
        assert_eq!(cmd("yay -S --frobnicate x").targets, ["x"]);
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
        // a substitution's `cd` moves its own commands only (WP-209)
        assert_eq!(
            dirs("cd a; D=$(cd /x && tee p) `cd b`; tee y $(tee z)"),
            [
                pair("/w", "/w"),
                pair("/w/a", "/w/a"),
                pair("/x", "/x"),
                pair("/w/a", "/w/a"),
                pair("/w/a", "/w/a"),
                pair("/w/a", "/w/a"),
                pair("/w/a", "/w/a"),
            ]
        );
        // round 2 (F2): a nested substitution starts where the one around
        // it is (`q` under `/x`), also two levels down before any command
        // of the middle one, and goes back to it after
        assert_eq!(
            dirs("cd a; $(cd /x; tee p $(tee q) r)"),
            [
                pair("/w", "/w"),
                pair("/w/a", "/w/a"),
                pair("/x", "/x"),
                pair("/x", "/x"),
                pair("/w/a", "/w/a"),
            ]
        );
        assert_eq!(
            dirs("$(cd /x; $( $(tee q) )) && tee r"),
            [
                pair("/w", "/w"),
                pair("/x", "/x"),
                pair("/x", "/x"),
                pair("/x", "/x"),
                pair("/w", "/w"),
                pair("/w", "/w"),
            ]
        );
        // a `pushd` stack copied into many substitutions, popped in one
        assert_eq!(
            dirs("pushd /p; pushd q; $(popd; tee x); popd; tee y"),
            [
                pair("/w", "/w"),
                pair("/p", "/p"),
                pair("/p/q", "/p/q"),
                pair("/p", "/p"),
                pair("/p/q", "/p/q"),
                pair("/p/q", "/p/q"),
                pair("/p", "/p"),
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

    /// `xargs` runs its command with the words from stdin (WP-157): its
    /// options with a value, the optional ones only in their cluster, and
    /// `xargs` alone runs `echo` itself.
    #[test]
    fn xargs_is_a_wrapper() {
        let argv = |line: &str| parse_shell(line).segments[0].argv().to_vec();
        let line = "sudo xargs -0 -n 1 -P4 -I {} -a list -d , -E END -s 99 cp {} /etc/x";
        assert_eq!(argv(line), ["cp", "{}", "/etc/x"]);
        assert_eq!(argv("xargs -i{} -l1 -eEND rm {}"), ["rm", "{}"]);
        // optional values never take the next word
        assert_eq!(argv("xargs -i rm"), ["rm"]);
        assert_eq!(argv("xargs -l -e rm"), ["rm"]);
        // `P` inside `-e`'s value is no option
        assert_eq!(argv("xargs -eSTOP rm"), ["rm"]);
        assert_eq!(
            argv(
                "xargs --max-args 1 --arg-file=l --delimiter , --max-procs 2 --max-chars 9 --process-slot-var V chpasswd"
            ),
            ["chpasswd"]
        );
        assert_eq!(argv("xargs --max-lines=1 --replace --null -r rm"), ["rm"]);
        assert_eq!(argv("sudo xargs"), ["xargs"]);
        assert_eq!(argv("xargs -0 -n1"), ["xargs", "-0", "-n1"]);
        assert!(argv("xargs --help").is_empty());
        let s = &parse_shell("printf 'a:b' | sudo xargs chpasswd").segments[1];
        assert_eq!(s.privilege(), Some("sudo"));
    }

    /// A command the line feeds: every one after `|` or `|&` (not `||`),
    /// `<<<` or `<(`, across `(`, loops, `if` and `>(…)`; one with a
    /// here-string or a process substitution of its own; the commands of a
    /// fed `sh -c` (WP-157 round 2). A pipe after a command feeds it
    /// nothing.
    #[test]
    fn fed_commands() {
        let fed = |line: &str| -> Vec<bool> {
            simple_commands(&parse_shell(line))
                .iter()
                .map(|s| s.fed)
                .collect()
        };
        assert_eq!(fed("a | b || c |& d"), [false, true, true, true]);
        assert_eq!(fed("a | (b; c) && d"), [false, true, true, true]);
        assert_eq!(
            fed("a | while read p; do b; done"),
            [false, true, true, true]
        );
        assert_eq!(fed("a | if c; then b; fi"), [false, true, true, true]);
        assert_eq!(fed("a | sudo tee >(b)"), [false, true, true]);
        // `a` inside `<( )` is read too (WP-209); the line does not feed it
        assert_eq!(fed("exec 3< <(a); b"), [false, true, true]);
        assert_eq!(fed("exec 3<<< a; b"), [true, true]);
        assert_eq!(fed("a; b | c"), [false, false, true]);
        assert_eq!(fed("a || b"), [false, false]);
        assert_eq!(
            fed("a < f; b << EOF\nx\nEOF\nc <(d) e; f"),
            [false, false, false, true, true]
        );
        assert_eq!(fed("a <<< x; b"), [true, true]);
        assert_eq!(fed("a |\n b"), [false, true]);
        assert_eq!(fed("a | sudo sh -c 'b; c | d'"), [false, true, true, true]);
        assert_eq!(fed("sudo sh -c 'b; c' <<< x; e"), [true, true, true]);
        assert_eq!(fed("x > out; y | z"), [false, false, true]);
        // round 3: an operator inside `$( )` or backticks, not `||` or a
        // quoted `|`; the commands inside come first (WP-209)
        assert_eq!(fed("a $(b | c); d"), [false, true, true, true]);
        assert_eq!(fed("a `b <<< c`; d"), [true, true, true]);
        assert_eq!(
            fed("a $(b || c); d $((1 | 2)) '$(x | y)'"),
            [false, false, false, false]
        );
        assert_eq!(fed("a $(echo '|'); d"), [false, false, false]);
        // WP-209: a substitution in a fed command reads the feed too
        assert_eq!(fed("echo PW | x $(passwd y)"), [false, true, true]);
        assert_eq!(fed("x <<< PW $(passwd y)"), [true, true]);
        assert_eq!(fed("x $(passwd y) | z"), [false, false, true]);
    }

    /// WP-209: the commands of `$( )`, backticks and `<( )` are commands of
    /// the line, read before the command they stand in and numbered by
    /// their substitution; `$(( ))`, `$[ ]` and `${ }` run none, nor does
    /// a quoted one.
    #[test]
    fn substitutions_are_opened() {
        let read = |line: &str| -> Vec<(Vec<String>, usize)> {
            simple_commands(&parse_shell(line))
                .into_iter()
                .map(|s| (s.argv().to_vec(), s.substitution))
                .collect()
        };
        let row = |argv: &[&str], n: usize| (argv.iter().map(|w| w.to_string()).collect(), n);
        assert_eq!(
            read("x=$(sudo chpasswd <<< a); echo \"`id -u`\" <(sudo cat /etc/x)"),
            [
                row(&["chpasswd"], 1),
                row(&[], 0),
                row(&["id", "-u"], 2),
                row(&["cat", "/etc/x"], 3),
                row(&["echo", "`id -u`"], 0),
            ]
        );
        // nested: numbered as they open, run inner first
        assert_eq!(
            read("a $(b $(c) d) e"),
            [
                row(&["c"], 2),
                row(&["b", "$(c)", "d"], 1),
                row(&["a", "$(b $(c) d)", "e"], 0)
            ]
        );
        assert_eq!(
            read("a $((1 + 2)) $[3] ${x:-$(b)} '$(c)'"),
            [row(&["a", "$((1 + 2))", "$[3]", "${x:-$(b)}", "$(c)"], 0)]
        );
        // a `sh -c` script's substitutions too, with its wrapper's privilege
        let segments = simple_commands(&parse_shell("sudo sh -c 'tee x < $(mktemp)'"));
        assert_eq!(segments[0].argv(), ["mktemp"]);
        assert_eq!(segments[0].privilege(), Some("sudo"));
        // round 2 (F2): an opened script's substitutions are numbered after
        // the line's, around the substitution its shell stands in
        let numbers = |line: &str| -> Vec<(usize, usize)> {
            simple_commands(&parse_shell(line))
                .iter()
                .map(|s| (s.substitution, s.parent))
                .collect()
        };
        assert_eq!(
            numbers("sh -c 'a $(b $(c))'; d $(e)"),
            [(3, 2), (2, 0), (0, 0), (1, 0), (0, 0)]
        );
        assert_eq!(
            numbers("x $(sh -c 'a $(b)'); eval 'c `d`'"),
            [(2, 1), (1, 0), (0, 0), (3, 0), (0, 0)]
        );
        // a substitution nested deeper than the limit is skipped, not read,
        // and the line says so; the commands after it are read
        let deep = |open: &str, close: &str, n: usize| {
            format!("{}x{} ; pacman -S zed", open.repeat(n), close.repeat(n))
        };
        // (open, close, substitutions each opens)
        for (open, close, levels) in [("$(", ")", 1), ("<(", ")", 1), ("$(echo `", "`)", 2)] {
            let at_limit = parse_shell(&deep(open, close, SUBSTITUTION_DEPTH_MAX / levels));
            assert!(!at_limit.too_deep, "{open}");
            let over = parse_shell(&deep(open, close, 40_000));
            assert!(over.too_deep, "{open}");
            // the skipped text is no word's: only its brackets stay
            for s in &over.segments {
                let w = s.words.concat();
                assert!(!w.contains('x'), "{open}: {}", &w[..w.len().min(300)]);
            }
            assert_eq!(over.segments.last().unwrap().words, ["pacman", "-S", "zed"]);
            assert!(
                over.segments.len() <= 2 * SUBSTITUTION_DEPTH_MAX + 2,
                "{open}"
            );
        }
        assert!(!parse_shell("a $(b) `c` <(d) $((1)) ${e}").too_deep);
        // round 2 (F1): in an opened `sh -c` or `eval` script too, also two
        // scripts down; one level less is read
        let nest = |n: usize| format!("{}secret{}", "$(".repeat(n), ")".repeat(n));
        for line in [
            format!("sudo sh -c 'lpadmin -x a {}'", nest(17)),
            format!("eval 'sudo lpadmin -x a {}'", nest(17)),
            format!("sh -c 'eval '\\''b {}'\\'''", nest(17)),
        ] {
            let parsed = parse_shell(&line);
            assert!(!parsed.too_deep, "{line}");
            assert!(too_deep(&parsed), "{line}");
            assert!(
                simple_commands(&parsed).iter().all(|s| s.too_deep),
                "{line}"
            );
        }
        assert!(!too_deep(&parse_shell(&format!("sh -c 'a {}'", nest(16)))));
        // a script of process substitutions only: no command, still marked
        let only = format!("sh -c '{}x{}'", "<(".repeat(17), ")".repeat(17));
        assert!(too_deep(&parse_shell(&only)));
        // a skipped backtick substitution ends at its own backtick, not at
        // one nested in `$( )` inside it
        let nested = format!("{}`a $(b `qq`) zz`{}", "$(".repeat(16), ")".repeat(16));
        let parsed = parse_shell(&nested);
        assert!(parsed.too_deep);
        for s in &parsed.segments {
            let w = s.words.concat();
            assert!(!w.contains("qq") && !w.contains("zz"), "{w}");
        }
        // a `<( )` skipped inside `$( )`: its text is no word's either
        let inside = format!("{}cat <(x){}", "$(".repeat(16), ")".repeat(16));
        let parsed = parse_shell(&inside);
        assert!(parsed.too_deep);
        assert!(
            parsed
                .segments
                .iter()
                .all(|s| !s.words.concat().contains('x'))
        );
        assert!(too_deep(&parse_shell(&format!("x {}", nest(17)))));
        // an unclosed one reads to the end of the line
        assert_eq!(
            read("a $(b c"),
            [row(&["b", "c"], 1), row(&["a", "$(b c"], 0)]
        );
    }

    /// WP-209: stdbuf and setsid run their command; their probes run none.
    #[test]
    fn stdbuf_and_setsid_are_wrappers() {
        let argv = |line: &str| parse_shell(line).segments[0].argv().to_vec();
        assert_eq!(
            argv("sudo stdbuf -i0 -oL --error=0 --input 0 cryptsetup open x"),
            ["cryptsetup", "open", "x"]
        );
        assert_eq!(argv("stdbuf -o L pacman -S zed"), ["pacman", "-S", "zed"]);
        assert_eq!(
            argv("setsid -f -w sudo -E lpadmin -x p"),
            ["lpadmin", "-x", "p"]
        );
        assert_eq!(argv("setsid --fork --wait --ctty yay"), ["yay"]);
        assert!(argv("setsid -V yay").is_empty());
        assert!(argv("setsid -h yay").is_empty());
        assert!(argv("stdbuf --help pacman").is_empty());
        let s = &parse_shell("setsid sudo passwd alice").segments[0];
        assert_eq!(s.privilege(), Some("sudo"));
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

    /// ADR-0039 round 2: sudo's `-S`/`--stdin` puts the password on the
    /// command line; a probe that takes it still runs nothing.
    #[test]
    fn a_password_on_stdin_is_seen() {
        let stdin = |line: &str| {
            let words = &parse_shell(line).segments[0].words;
            let u = unwrap_command(words);
            (u.password_on_stdin, u.argv.join(" "))
        };
        for (line, argv) in [
            ("sudo -S lpadmin -x X", "lpadmin -x X"),
            ("sudo -Su root lpadmin", "lpadmin"),
            ("sudo -u root -kS lpadmin", "lpadmin"),
            ("sudo --stdin lpadmin", "lpadmin"),
            ("sudo --st lpadmin", "lpadmin"),
            ("sudo --stdi lpadmin", "lpadmin"),
            ("env LANG=C sudo -S lpadmin", "lpadmin"),
            ("sudo -S -v", ""),
            ("sudo -vS", ""),
            ("sudo -l -S", ""),
        ] {
            assert_eq!(stdin(line), (true, argv.to_string()), "{line}");
        }
        for line in [
            "sudo lpadmin -S x",
            "sudo -uS lpadmin",
            "sudo -s lpadmin",
            "sudo --set-home lpadmin",
            "sudo --s lpadmin",
            "doas -s lpadmin",
            "pkexec lpadmin -S",
            "run0 lpadmin -S",
            "lpadmin -S",
        ] {
            assert!(!stdin(line).0, "{line}");
        }
    }
}
