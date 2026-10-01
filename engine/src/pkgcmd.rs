//! Package command lines, parsed as argv (SPEC-ENGINE §4, §8; ADR-0017 §3 §5).
//!
//! One parser for everything that reads a package command: the pacman
//! collector (the logged `[PACMAN] Running` line: explicit packages, the
//! routine class of ADR-0013 §3), attribution (what a hook `command` event
//! asked for, [`command_intent`]) and the hook (is a command mutating at
//! all). Moved here from `collectors/pacman.rs` unchanged (WP-009), so the
//! hook and the drift rules do not depend on a collector.

/// pacman's operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Op {
    Sync,
    Remove,
    Upgrade,
    Database,
    Query,
    DepTest,
    Files,
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
    /// stripped; for `-U`, the name from the package file name).
    pub targets: Vec<String>,
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
    };
    // query letters/long names seen; which count depends on the operation
    let mut flags: Vec<char> = Vec::new();
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
    // `yay` alone is `yay -Syu`; `yay zed` searches and installs
    if cmd.op.is_none() && cmd.program != "pacman" {
        cmd.op = Some(Op::Sync);
        cmd.sysupgrade |= words.is_empty();
    }
    // with -S, `u` means sysupgrade; with -R/-Q it means something else
    if cmd.op != Some(Op::Sync) {
        cmd.sysupgrade = false;
    }
    // with -R, `s` and `c` are --recursive/--cascade, not queries
    let query_letters: &[char] = match cmd.op {
        Some(Op::Sync) => &['s', 'i', 'l', 'g', 'p', 'w', 'c'],
        Some(Op::Remove | Op::Upgrade) => &['p'],
        _ => &[],
    };
    cmd.query = flags.iter().any(|f| query_letters.contains(f));
    cmd.targets = words
        .into_iter()
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

    /// `-S` with `-u` and no package: the routine class of ADR-0013 §3.
    pub fn is_plain_full_upgrade(&self) -> bool {
        self.is_full_upgrade() && self.targets.is_empty()
    }

    /// Operations that change packages: `-S`, `-R`, `-U` without a query
    /// form (ADR-0017 §5). `-Q`, `-T`, `-F`, `-D` never are.
    pub fn is_mutating(&self) -> bool {
        matches!(self.op, Some(Op::Sync | Op::Remove | Op::Upgrade)) && !self.query
    }
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

/// Reads a hook command line: each `&&`, `||`, `;` or `|` segment, after
/// leading `VAR=value` words and `sudo`/`doas` with their options. Knows
/// pacman-like programs, `omarchy update`, `omarchy pkg add|aur add|drop|remove`
/// and the `omarchy-update`/`omarchy-pkg-*` scripts.
pub fn command_intent(line: &str) -> Intent {
    let mut intent = Intent::default();
    'segments: for segment in line.split(['&', '|', ';', '\n']) {
        let mut argv: Vec<&str> = segment
            .split_whitespace()
            .map(|w| w.trim_matches(['"', '\'', '(', ')']))
            .filter(|w| !w.is_empty())
            .collect();
        loop {
            match argv.first() {
                Some(w) if is_assignment(w) => {
                    argv.remove(0);
                }
                Some(&wrapper @ ("sudo" | "doas" | "env" | "command" | "exec")) => {
                    argv.remove(0);
                    while argv.first().is_some_and(|w| w.starts_with('-')) {
                        // `command -v yay` runs nothing (no full upgrade)
                        if is_probe(wrapper, argv[0]) {
                            continue 'segments;
                        }
                        argv.remove(0);
                    }
                }
                _ => break,
            }
        }
        if let Some(cmd) = parse_command(&argv) {
            if cmd.is_mutating() {
                intent.full_upgrade |= cmd.is_full_upgrade();
                intent.packages.extend(cmd.targets);
            }
            continue;
        }
        let program = argv
            .first()
            .map(|p| p.rsplit('/').next().unwrap_or(p))
            .unwrap_or("");
        let rest: Vec<&str> = argv.iter().skip(1).copied().collect();
        let names = |words: &[&str]| -> Vec<String> {
            words
                .iter()
                .filter(|w| !w.starts_with('-'))
                .map(|w| package_name(w))
                .collect()
        };
        match (program, rest.as_slice()) {
            ("omarchy", ["update", ..]) | ("omarchy-update", _) => {
                intent.full_upgrade = true;
                // `omarchy-update-keyring` installs these before the upgrade
                // (ADR-0017 §3)
                intent
                    .packages
                    .extend(OMARCHY_UPDATE_NAMES.map(String::from));
            }
            ("omarchy", ["pkg", "aur", "add", pkgs @ ..])
            | ("omarchy", ["pkg", "add" | "install" | "drop" | "remove", pkgs @ ..]) => {
                intent.packages.extend(names(pkgs));
            }
            (p, pkgs) if p.starts_with("omarchy-pkg-") => intent.packages.extend(names(pkgs)),
            _ => {}
        }
    }
    intent
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
}

impl Segment {
    /// [`command_argv`] of the words.
    pub fn argv(&self) -> &[String] {
        command_argv(&self.words)
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
    /// A file read, or a here-string: dropped.
    Read,
    /// A heredoc delimiter (`<<-`: tabs before the delimiter line).
    Heredoc { strip_tabs: bool },
}

/// Splits a command line the way a POSIX shell would for reading it, never
/// for running it: single and double quotes, backslashes, `$(…)` and
/// backticks (kept literally inside their word), comments, redirections
/// and heredocs. Expansions are not performed.
pub fn parse_shell(line: &str) -> ShellLine {
    let chars: Vec<(usize, char)> = line.char_indices().collect();
    let at = |i: usize| chars.get(i).map(|&(_, c)| c);
    let mut lexer = Lexer::default();
    let mut cuts: Vec<(usize, usize)> = Vec::new();
    let mut i = 0;
    while let Some(c) = at(i) {
        match c {
            '\\' => {
                match at(i + 1) {
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
                while let Some(q) = at(i) {
                    i += 1;
                    if q == '\'' {
                        break;
                    }
                    lexer.push(q);
                }
                continue;
            }
            '"' => {
                lexer.start();
                i += 1;
                while let Some(q) = at(i) {
                    i += 1;
                    match q {
                        '"' => break,
                        '\\' if matches!(at(i), Some('"' | '\\' | '$' | '`')) => {
                            lexer.push(at(i).unwrap_or('\\'));
                            i += 1;
                        }
                        '\\' if at(i) == Some('\n') => i += 1,
                        q => lexer.push(q),
                    }
                }
                continue;
            }
            '$' if at(i + 1) == Some('(') => {
                let end = closing_paren(&chars, i + 1);
                for &(_, k) in &chars[i..end] {
                    lexer.push(k);
                }
                i = end;
                continue;
            }
            '`' => {
                lexer.push('`');
                i += 1;
                while let Some(k) = at(i) {
                    lexer.push(k);
                    i += 1;
                    if k == '`' {
                        break;
                    }
                }
                continue;
            }
            '#' if lexer.word.is_none() => {
                while at(i).is_some_and(|k| k != '\n') {
                    i += 1;
                }
                continue;
            }
            '\n' => {
                lexer.end_segment();
                i += 1;
                // heredoc bodies start on the next line
                for (delimiter, strip_tabs) in std::mem::take(&mut lexer.heredocs) {
                    let start = chars.get(i).map_or(line.len(), |&(b, _)| b);
                    let mut end = line.len();
                    let mut pos = start;
                    while pos < line.len() {
                        let next = line[pos..].find('\n').map_or(line.len(), |n| pos + n + 1);
                        let text = line[pos..next].trim_end_matches(['\n', '\r']);
                        let text = if strip_tabs {
                            text.trim_start_matches('\t')
                        } else {
                            text
                        };
                        pos = next;
                        if text == delimiter {
                            end = next;
                            break;
                        }
                    }
                    cuts.push((start, end));
                    while chars.get(i).is_some_and(|&(b, _)| b < end) {
                        i += 1;
                    }
                }
                continue;
            }
            '&' if at(i + 1) == Some('>') => {
                lexer.end_word();
                lexer.pending = Some(Pending::Write);
                i += 2;
                if at(i) == Some('>') {
                    i += 1;
                }
                continue;
            }
            ';' | '&' | '|' | '(' | ')' => {
                lexer.end_segment();
                i += 1;
                if matches!(
                    (c, at(i)),
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
                    if matches!(at(i), Some('>' | '|')) {
                        i += 1;
                    }
                    if at(i) == Some('&') {
                        // `>&2`: a descriptor, not a file
                        i += 1;
                        while at(i).is_some_and(|d| d.is_ascii_digit() || d == '-') {
                            i += 1;
                        }
                    } else {
                        lexer.pending = Some(Pending::Write);
                    }
                } else if at(i) == Some('<') && at(i + 1) == Some('<') {
                    lexer.pending = Some(Pending::Read);
                    i += 2;
                } else if at(i) == Some('<') {
                    i += 1;
                    let strip_tabs = at(i) == Some('-');
                    if strip_tabs {
                        i += 1;
                    }
                    lexer.pending = Some(Pending::Heredoc { strip_tabs });
                } else if at(i) == Some('(') {
                    i = closing_paren(&chars, i);
                } else if at(i) == Some('&') {
                    i += 1;
                    while at(i).is_some_and(|d| d.is_ascii_digit() || d == '-') {
                        i += 1;
                    }
                } else {
                    lexer.pending = Some(Pending::Read);
                }
                continue;
            }
            c if c.is_whitespace() => lexer.end_word(),
            c => lexer.push(c),
        }
        i += 1;
    }
    lexer.end_segment();

    let mut text = String::with_capacity(line.len());
    let mut from = 0;
    for (start, end) in cuts {
        text.push_str(&line[from..start]);
        from = end;
    }
    text.push_str(&line[from.min(line.len())..]);
    ShellLine {
        segments: lexer.segments,
        text: text.trim_end().to_string(),
    }
}

/// The index after the `)` that closes the `(` at `open` (or the end).
fn closing_paren(chars: &[(usize, char)], open: usize) -> usize {
    let mut depth = 0usize;
    let mut quote: Option<char> = None;
    for (i, &(_, c)) in chars.iter().enumerate().skip(open) {
        match (quote, c) {
            (Some(q), c) if c == q => quote = None,
            (Some(_), _) => {}
            (None, '\'' | '"') => quote = Some(c),
            (None, '(') => depth += 1,
            (None, ')') => {
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

/// `words` after leading `VAR=value` assignments and the wrappers that run
/// their arguments as a command (`sudo`, `doas`, `env`, `command`, `exec`,
/// `nohup`, `time`, `nice`, `timeout`), with the wrappers' options.
pub fn command_argv(words: &[String]) -> &[String] {
    let mut i = 0;
    while let Some(w) = words.get(i) {
        if is_assignment(w) {
            i += 1;
            continue;
        }
        // options that take the next word as their value
        let with_value: &[&str] = match w.rsplit('/').next().unwrap_or(w) {
            "sudo" => &[
                "-u",
                "-g",
                "-h",
                "-p",
                "-C",
                "-D",
                "-r",
                "-t",
                "-U",
                "-T",
                "--user",
                "--group",
                "--host",
                "--prompt",
                "--close-from",
                "--chdir",
                "--role",
                "--type",
                "--other-user",
                "--command-timeout",
            ],
            "doas" => &["-u", "-C"],
            "env" => &["-u", "-C", "-S", "--unset", "--chdir", "--split-string"],
            "nice" => &["-n", "--adjustment"],
            "timeout" => &["-s", "-k", "--signal", "--kill-after"],
            "exec" => &["-a"],
            "command" | "nohup" | "time" => &[],
            _ => break,
        };
        let wrapper = w.rsplit('/').next().unwrap_or(w);
        let duration = wrapper == "timeout";
        i += 1;
        while let Some(o) = words.get(i).filter(|o| o.starts_with('-')) {
            i += 1;
            if o == "--" {
                break;
            }
            if is_probe(wrapper, o) {
                return &[];
            }
            if with_value.contains(&o.as_str()) {
                i += 1;
            }
        }
        if duration {
            i += 1;
        }
    }
    &words[i.min(words.len())..]
}

/// How deep `sh -c '…'` and `eval '…'` are opened ([`simple_commands`]).
const NESTING_MAX: usize = 3;

/// The simple commands of `line`, where `sh|bash|zsh|dash -c '<script>'`
/// and `eval <words>` are replaced by the simple commands of their script
/// (up to `NESTING_MAX` levels; a redirection of the outer command stays
/// as a segment of its own). Read only: nothing is expanded or run.
pub fn simple_commands(line: &ShellLine) -> Vec<Segment> {
    fn open(segment: &Segment, depth: usize, out: &mut Vec<Segment>) {
        match nested_script(segment.argv()).filter(|_| depth < NESTING_MAX) {
            Some(script) => {
                for inner in &parse_shell(&script).segments {
                    open(inner, depth + 1, out);
                }
                if !segment.writes.is_empty() {
                    out.push(Segment {
                        words: Vec::new(),
                        writes: segment.writes.clone(),
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

/// Whether `option` turns `wrapper` into a probe that runs nothing:
/// `command -v|-V` (print what a name is) and `sudo -l|-v|-k|-K` (list,
/// validate, drop credentials). `command -v yay` must never read as `yay`,
/// which is a full upgrade (ADR-0017 §3).
pub fn is_probe(wrapper: &str, option: &str) -> bool {
    match wrapper {
        "command" => option.strip_prefix('-').is_some_and(|c| {
            !c.starts_with('-')
                && c.contains(['v', 'V'])
                && c.chars().all(|ch| matches!(ch, 'p' | 'v' | 'V'))
        }),
        "sudo" => matches!(
            option,
            "-l" | "-ll"
                | "-v"
                | "-k"
                | "-K"
                | "--list"
                | "--validate"
                | "--reset-timestamp"
                | "--remove-timestamp"
        ),
        _ => false,
    }
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
        assert_eq!(
            line.text,
            "cat > ~/.config/hypr/x.conf <<'EOF'\necho done\ncat <<-END | wc -l"
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
        // an unterminated heredoc swallows the rest
        assert_eq!(parse_shell("cat <<EOF\nall\nof it").text, "cat <<EOF");
        // a here-string is part of the line
        assert_eq!(parse_shell("cat <<< word").text, "cat <<< word");
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
}
