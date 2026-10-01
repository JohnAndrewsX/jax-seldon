//! The Markdown of `outputs/REBUILD.md`: English headings, prose in the
//! logbook language (SPEC-LOGBOOK §2), one line per item with the ledger
//! event it comes from. Nothing here depends on the clock, so an unchanged
//! logbook renders the same bytes.

use std::fmt::Write as _;

use super::{Origin, Rebuild, Scope, Why};
use crate::index::model::DriftItem;
use crate::model::Language;
use crate::model::event::Resolution;

/// Prose fragments per language.
struct Words {
    intro: &'static str,
    as_of: &'static str,
    last_event: &'static str,
    updated_on: &'static str,
    base_step: &'static str,
    version_unknown: &'static str,
    packages_intro: &'static str,
    packages_intro_end: &'static str,
    packages_before: &'static str,
    explicit_in_dossier: &'static str,
    before_intro: &'static str,
    from_repos: &'static str,
    from_aur: &'static str,
    before_skip: &'static str,
    repo_unknown: &'static str,
    all_repo: &'static str,
    all_aur: &'static str,
    linked: &'static str,
    explained: &'static str,
    open: &'static str,
    deviations_intro: &'static str,
    removed: &'static str,
    no_reason: &'static str,
    plugins_intro: &'static str,
    url_unknown: &'static str,
    then: &'static str,
    stays_disabled: &'static str,
    username: &'static str,
    becomes: &'static str,
    enabled_by_clone: &'static str,
    drop_in: &'static str,
    first_party_disabled: &'static str,
    theme_none: &'static str,
    theme_dossier: &'static str,
    units_intro: &'static str,
    restore: &'static str,
    open_intro: &'static str,
    more: &'static str,
    crisis: &'static str,
    proposed: &'static str,
    group: &'static str,
    dismissed_intro: &'static str,
    none: &'static str,
}

const EN: Words = Words {
    intro: "This guide takes a fresh Omarchy install to the state this logbook documents. Work through the sections in order; each line names the ledger event it comes from.",
    as_of: "As of",
    last_event: "last event",
    updated_on: "updated on",
    base_step: "Install Omarchy, then run `omarchy update` until at least this version runs.",
    version_unknown: "Omarchy version unknown: no `update` event and no `omarchy.summary` in the dossier.",
    packages_intro: "Packages installed explicitly since the logbook began (",
    packages_intro_end: "), grouped by case; their dependencies come along.",
    packages_before: "Packages from before the logbook are not listed. Explicit packages in the dossier:",
    explicit_in_dossier: "Explicit packages in the dossier:",
    before_intro: "Explicit packages from before the logbook (dossier `packages.explicit`):",
    from_repos: "from the repositories",
    from_aur: "from the AUR",
    before_skip: "A fresh Omarchy install already has many of them; the commands skip what is installed.",
    repo_unknown: "repository unknown; from the AUR:",
    all_repo: "All repository packages at once (open ones left out)",
    all_aur: "All AUR packages at once (open ones left out)",
    linked: "linked",
    explained: "explained",
    open: "**open**, see 7",
    deviations_intro: "Files that differ from the Omarchy default. Seldon records path and reason, not the content: take the files from your dotfiles or backup.",
    removed: "remove this file",
    no_reason: "no reason recorded",
    plugins_intro: "Plugins beyond the first-party ones (those come with Omarchy).",
    url_unknown: "source URL not recorded",
    then: "then",
    stays_disabled: "stays disabled",
    username: "<username>",
    becomes: "becomes",
    enabled_by_clone: "enabled by the clone",
    drop_in: "drop-in for",
    first_party_disabled: "First-party plugins disabled here; after the install, disable each with `omarchy plugin disable <id>`:",
    theme_none: "No theme recorded.",
    theme_dossier: "from the dossier",
    units_intro: "systemd units this logbook knows. Take user unit files from your dotfiles or backup.",
    restore: "restore the file",
    open_intro: "Drift nobody has decided yet. Decide before you rebuild: `seldon drift link <id> <case>`, `seldon drift explain <id> -- <reason>` or `seldon drift dismiss <id> -- <reason>`.",
    more: "more: `seldon drift`",
    crisis: "**Crisis**",
    proposed: "proposed",
    group: "events in this transaction",
    dismissed_intro: "Dismissed on purpose; do not set these up again.",
    none: "none",
};

const DE: Words = Words {
    intro: "Diese Anleitung bringt eine frische Omarchy-Installation in den Zustand, den dieses Logbuch dokumentiert. Arbeite die Abschnitte der Reihe nach ab; jede Zeile nennt das Ledger-Ereignis, aus dem sie stammt.",
    as_of: "Stand",
    last_event: "letztes Ereignis",
    updated_on: "aktualisiert am",
    base_step: "Installiere Omarchy und führe dann `omarchy update` aus, bis mindestens diese Version läuft.",
    version_unknown: "Omarchy-Version unbekannt: kein `update`-Ereignis und kein `omarchy.summary` im Dossier.",
    packages_intro: "Explizit installierte Pakete seit Beginn des Logbuchs (",
    packages_intro_end: "), nach Case; ihre Abhängigkeiten kommen von selbst mit.",
    packages_before: "Pakete von vor dem Logbuch fehlen hier. Explizite Pakete laut Dossier:",
    explicit_in_dossier: "Explizite Pakete laut Dossier:",
    before_intro: "Explizite Pakete von vor dem Logbuch (Dossier `packages.explicit`):",
    from_repos: "aus den Repositories",
    from_aur: "aus dem AUR",
    before_skip: "Eine frische Omarchy-Installation bringt viele davon schon mit; die Befehle überspringen, was schon installiert ist.",
    repo_unknown: "Quelle unbekannt; aus dem AUR:",
    all_repo: "Alle Repo-Pakete auf einmal (ohne offene)",
    all_aur: "Alle AUR-Pakete auf einmal (ohne offene)",
    linked: "verknüpft",
    explained: "erklärt",
    open: "**offen**, siehe 7",
    deviations_intro: "Dateien, die vom Omarchy-Standard abweichen. Seldon kennt Pfad und Grund, nicht den Inhalt: übernimm die Dateien aus deinen Dotfiles oder deinem Backup.",
    removed: "Datei entfernen",
    no_reason: "kein Grund notiert",
    plugins_intro: "Plugins außer den Erstanbieter-Plugins (die kommen mit Omarchy).",
    url_unknown: "Quell-URL nicht erfasst",
    then: "dann",
    stays_disabled: "bleibt deaktiviert",
    username: "<benutzername>",
    becomes: "wird zu",
    enabled_by_clone: "vom Klonen aktiviert",
    drop_in: "Drop-in für",
    first_party_disabled: "Hier deaktivierte Erstanbieter-Plugins; nach der Installation jeweils mit `omarchy plugin disable <id>` abschalten:",
    theme_none: "Kein Theme erfasst.",
    theme_dossier: "laut Dossier",
    units_intro: "systemd-Units, die dieses Logbuch kennt. User-Unit-Dateien übernimmst du aus deinen Dotfiles oder deinem Backup.",
    restore: "Datei übernehmen",
    open_intro: "Drift, die noch niemand eingeordnet hat. Entscheide vor dem Nachbau: `seldon drift link <id> <case>`, `seldon drift explain <id> -- <Grund>` oder `seldon drift dismiss <id> -- <Grund>`.",
    more: "weitere: `seldon drift`",
    crisis: "**Krise**",
    proposed: "Vorschlag",
    group: "Ereignisse in dieser Transaktion",
    dismissed_intro: "Bewusst verworfen; nicht wieder einrichten.",
    none: "keine",
};

fn words(language: Language) -> &'static Words {
    match language {
        Language::En => &EN,
        Language::De => &DE,
    }
}

/// The content of the `rebuild` fence.
pub fn text(r: &Rebuild, language: Language) -> String {
    let w = words(language);
    let mut t = format!("# Rebuild — {}\n\n{}\n", r.machine, w.intro);
    if let Some(ts) = &r.last_event {
        let when = ts.get(..16).unwrap_or(ts).replacen('T', " ", 1);
        let _ = write!(t, "\n{}: {} {when}\n", w.as_of, w.last_event);
    }

    t.push_str("\n## 1. Base\n");
    match &r.base.version {
        Some(v) => {
            let _ = write!(t, "- Omarchy {v}");
            if let Some(d) = &r.base.updated {
                let _ = write!(t, " · {} {d}", w.updated_on);
            }
            if let Some(id) = &r.base.event {
                let _ = write!(t, " · {}", code(id));
            }
            let _ = writeln!(t, "\n- {}", w.base_step);
        }
        None => {
            let _ = writeln!(t, "- {}", w.version_unknown);
            let _ = writeln!(t, "- {}", w.base_step);
        }
    }

    t.push_str("\n## 2. Packages\n");
    let _ = writeln!(t, "{}{}{}", w.packages_intro, r.since, w.packages_intro_end);
    if let Some(n) = r.explicit_total {
        let words = match r.before {
            Some(_) => w.explicit_in_dossier,
            None => w.packages_before,
        };
        let _ = writeln!(t, "{words} {n}.");
    }
    if let Some(before) = &r.before {
        t.push_str("\n### Before the logbook\n");
        let _ = writeln!(
            t,
            "{} {} {}, {} {}. {}\n",
            w.before_intro,
            before.repo.len(),
            w.from_repos,
            before.aur.len(),
            w.from_aur,
            w.before_skip
        );
        if before.repo.is_empty() && before.aur.is_empty() {
            let _ = writeln!(t, "- {}", w.none);
        } else {
            t.push_str("```sh\n");
            t.push_str(&command_lines("omarchy pkg add", &before.repo));
            t.push_str(&command_lines("omarchy pkg aur add", &before.aur));
            t.push_str("```\n");
        }
    }
    if r.packages.is_empty() {
        let _ = writeln!(t, "\n- {}", w.none);
    }
    let mut group: Option<Option<&str>> = None;
    for p in &r.packages {
        let case = p.why.case.as_deref();
        if group != Some(case) {
            group = Some(case);
            match case {
                Some(id) => {
                    let _ = write!(t, "\n### [[{id}]]");
                    if let Some(title) = &p.why.case_title {
                        let _ = write!(t, " {}", one_line(title));
                    }
                    t.push('\n');
                }
                None => t.push_str("\n### Without a case\n"),
            }
        }
        let aur = format!("omarchy pkg aur add {}", p.name);
        let repo = format!("omarchy pkg add {}", p.name);
        let _ = match p.origin {
            Origin::Repo => write!(t, "- {}", code(&repo)),
            Origin::Aur => write!(t, "- {}", code(&aur)),
            Origin::Unknown => write!(t, "- {} ({} {})", code(&repo), w.repo_unknown, code(&aur)),
        };
        if let Some(v) = &p.version {
            let _ = write!(t, " — {}", one_line(v));
        }
        t.push_str(&suffix(&p.why, w, false));
        t.push('\n');
    }
    for (origin, label, cmd) in [
        (Origin::Repo, w.all_repo, "omarchy pkg add"),
        (Origin::Aur, w.all_aur, "omarchy pkg aur add"),
    ] {
        let mut names: Vec<&str> = r
            .packages
            .iter()
            .filter(|p| p.origin == origin && !p.why.open)
            .map(|p| p.name.as_str())
            .collect();
        if names.len() > 1 {
            names.sort_unstable();
            let _ = writeln!(
                t,
                "\n{label}: {}",
                code(&format!("{cmd} {}", names.join(" ")))
            );
        }
    }

    t.push_str("\n## 3. Deviations\n");
    let _ = writeln!(t, "{}\n", w.deviations_intro);
    if r.deviations.is_empty() {
        let _ = writeln!(t, "- {}", w.none);
    }
    for d in &r.deviations {
        let _ = write!(t, "- {}", code(&d.path));
        if d.removed {
            let _ = write!(t, " — {}", w.removed);
        }
        match &d.why.reason {
            Some(reason) => {
                let _ = write!(t, " — {}", one_line(reason));
            }
            None if d.why.case.is_none() && !d.why.open => {
                let _ = write!(t, " — {}", w.no_reason);
            }
            None => {}
        }
        let mut why = d.why.clone();
        // the reason (the dossier's or the explanation) is already printed
        if why.resolution == Some(Resolution::Explained) {
            why.resolution = None;
        }
        why.reason = None;
        t.push_str(&suffix(&why, w, true));
        t.push('\n');
    }

    t.push_str("\n## 4. Plugins\n");
    let _ = writeln!(t, "{}\n", w.plugins_intro);
    if r.plugins.is_empty() {
        let _ = writeln!(t, "- {}", w.none);
    }
    for p in &r.plugins {
        let _ = write!(t, "- {} — ", code(&p.id));
        match (&p.cloned_from, &p.url) {
            (Some(from), _) => {
                // `omarchy plugin clone` names the copy `$USER.<id without
                // omarchy.>` and enables it (Omarchy 4.0.4)
                let clone = format!(
                    "{}.{}",
                    w.username,
                    from.strip_prefix("omarchy.").unwrap_or(from)
                );
                let _ = write!(
                    t,
                    "{} ({} {}, {})",
                    code(&format!("omarchy plugin clone {from}")),
                    w.becomes,
                    code(&clone),
                    w.enabled_by_clone
                );
                if p.enabled == Some(false) {
                    let _ = write!(
                        t,
                        ", {} {}",
                        w.then,
                        code(&format!("omarchy plugin disable {clone}"))
                    );
                }
                t.push_str(&suffix(&p.why, w, true));
                t.push('\n');
                continue;
            }
            (None, Some(url)) => {
                let _ = write!(t, "{}", code(&format!("omarchy plugin add {url}")));
            }
            (None, None) => {
                let _ = write!(
                    t,
                    "{} ({})",
                    code("omarchy plugin add <url>"),
                    w.url_unknown
                );
            }
        }
        match p.enabled {
            Some(true) => {
                let _ = write!(
                    t,
                    ", {} {}",
                    w.then,
                    code(&format!("omarchy plugin enable {}", p.id))
                );
            }
            Some(false) => {
                let _ = write!(t, ", {}", w.stays_disabled);
            }
            None => {}
        }
        t.push_str(&suffix(&p.why, w, true));
        t.push('\n');
    }
    if !r.first_party_disabled.is_empty() {
        let ids: Vec<String> = r.first_party_disabled.iter().map(|i| code(i)).collect();
        let _ = writeln!(t, "\n{} {}", w.first_party_disabled, ids.join(", "));
    }

    t.push_str("\n## 5. Theme\n");
    match &r.theme {
        Some(theme) => {
            let _ = write!(
                t,
                "- {}",
                code(&format!("omarchy theme set {}", theme.name))
            );
            if theme.why.event.is_none() {
                let _ = write!(t, " · {}", w.theme_dossier);
            }
            t.push_str(&suffix(&theme.why, w, true));
            t.push('\n');
        }
        None => {
            let _ = writeln!(t, "- {}", w.theme_none);
        }
    }

    t.push_str("\n## 6. User units\n");
    let _ = writeln!(t, "{}\n", w.units_intro);
    let user: Vec<_> = r.units.iter().filter(|u| u.scope == Scope::User).collect();
    let system: Vec<_> = r
        .units
        .iter()
        .filter(|u| u.scope == Scope::System)
        .collect();
    if user.is_empty() {
        let _ = writeln!(t, "- {}", w.none);
    }
    for u in user {
        let enable = code(&format!("systemctl --user enable --now {}", u.unit));
        match &u.path {
            Some(path) => {
                let reload = code("systemctl --user daemon-reload");
                let (step, then) = match (u.removed, u.enabled) {
                    (true, _) => (w.removed, reload),
                    (false, true) => (w.restore, enable),
                    (false, false) => (w.restore, reload),
                };
                let _ = write!(t, "- {}", code(path));
                if u.drop_in {
                    let _ = write!(t, " ({} {})", w.drop_in, code(&u.unit));
                }
                let _ = write!(t, " — {step}, {} {then}", w.then);
            }
            None => {
                let _ = write!(t, "- {} — {enable}", code(&u.unit));
            }
        }
        t.push_str(&suffix(&u.why, w, true));
        t.push('\n');
    }
    if !system.is_empty() {
        t.push_str("\n### System units\n");
        for u in system {
            let _ = write!(
                t,
                "- {} — {}",
                code(&u.unit),
                code(&format!("sudo systemctl enable --now {}", u.unit))
            );
            t.push_str(&suffix(&u.why, w, true));
            t.push('\n');
        }
    }

    t.push_str("\n## 7. Open questions\n");
    let _ = writeln!(t, "{}\n", w.open_intro);
    if r.open.is_empty() {
        let _ = writeln!(t, "- {}", w.none);
    }
    for d in &r.open {
        t.push_str(&drift_line(d, w));
    }
    if r.open_total > r.open.len() {
        let _ = writeln!(t, "- … {} {}", r.open_total - r.open.len(), w.more);
    }
    t.push_str("\n### Deliberately not reproduced\n");
    let _ = writeln!(t, "{}\n", w.dismissed_intro);
    if r.dismissed.is_empty() {
        let _ = writeln!(t, "- {}", w.none);
    }
    for d in &r.dismissed {
        let _ = write!(t, "- {} {} {}", d.source, d.kind, code(&d.subject));
        if d.members > 1 {
            let _ = write!(t, " · {} {}", d.members, w.group);
        }
        if let Some(reason) = &d.reason {
            let _ = write!(t, " — {}", one_line(reason));
        }
        let _ = writeln!(t, " · {}", code(&d.event));
    }
    t
}

/// ` · [[case]] title · linked · explained: … · open · agent · `id``;
/// `with_case` false when a heading already names the case.
fn suffix(why: &Why, w: &Words, with_case: bool) -> String {
    let mut s = String::new();
    if with_case && let Some(case) = &why.case {
        let _ = write!(s, " · [[{case}]]");
        if let Some(title) = &why.case_title {
            let _ = write!(s, " {}", one_line(title));
        }
    }
    match (why.resolution, &why.reason) {
        (Some(Resolution::Linked), _) => {
            let _ = write!(s, " · {}", w.linked);
        }
        (Some(Resolution::Explained), Some(reason)) => {
            let _ = write!(s, " · {}: {}", w.explained, one_line(reason));
        }
        (Some(Resolution::Explained), None) => {
            let _ = write!(s, " · {}", w.explained);
        }
        _ => {}
    }
    if why.open {
        let _ = write!(s, " · {}", w.open);
    }
    if let Some(agent) = &why.agent {
        let _ = write!(s, " · {agent}");
    }
    if let Some(id) = &why.event {
        let _ = write!(s, " · {}", code(id));
    }
    s
}

fn drift_line(d: &DriftItem, w: &Words) -> String {
    let mut line = String::from("- ");
    if d.crisis {
        let _ = write!(line, "{} ", w.crisis);
    }
    let _ = write!(line, "{} {} {}", d.source, d.kind, code(&d.subject));
    if d.source != "config"
        && let Some(detail) = &d.detail
    {
        let _ = write!(line, " {}", one_line(detail));
    }
    if d.actor != "system" {
        let _ = write!(line, " · {}", d.actor);
    }
    if let Some(n) = d.members {
        let _ = write!(line, " · {n} {}", w.group);
    }
    if let Some(case) = &d.proposed_case {
        let _ = write!(line, " · {} [[{case}]]", w.proposed);
    }
    let _ = writeln!(line, " · {}", code(&d.event_id));
    line
}

/// Width a command line of the "Before the logbook" block stays within.
const BLOCK_WIDTH: usize = 76;

/// `cmd name…` as shell lines of at most [`BLOCK_WIDTH`] characters,
/// continued with ` \`; nothing for no names.
fn command_lines(cmd: &str, names: &[String]) -> String {
    if names.is_empty() {
        return String::new();
    }
    let one = format!("{cmd} {}", names.join(" "));
    if one.chars().count() <= BLOCK_WIDTH {
        return one + "\n";
    }
    let mut lines = vec![cmd.to_string()];
    let mut line = String::from(" ");
    for name in names {
        // room for the name and the trailing ` \`
        if line.len() > 1 && line.chars().count() + 1 + name.chars().count() + 2 > BLOCK_WIDTH {
            lines.push(std::mem::replace(&mut line, String::from(" ")));
        }
        line.push(' ');
        line.push_str(name);
    }
    lines.push(line);
    lines.join(" \\\n") + "\n"
}

/// An inline code span that survives backticks in `s`.
fn code(s: &str) -> String {
    if s.contains('`') {
        format!("`` {s} ``")
    } else {
        format!("`{s}`")
    }
}

/// User text on one line.
fn one_line(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn long_commands_are_continued_lines() {
        let names = |n: usize| {
            (0..n)
                .map(|i| format!("package-{i:02}"))
                .collect::<Vec<_>>()
        };
        assert_eq!(command_lines("omarchy pkg add", &[]), "");
        assert_eq!(
            command_lines("omarchy pkg add", &names(2)),
            "omarchy pkg add package-00 package-01\n"
        );
        let text = command_lines("omarchy pkg add", &names(14));
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines[0], "omarchy pkg add \\");
        assert!(
            lines.iter().all(|l| l.chars().count() <= BLOCK_WIDTH),
            "{text}"
        );
        assert!(lines[1..lines.len() - 1].iter().all(|l| l.ends_with(" \\")));
        assert!(lines[1..].iter().all(|l| l.starts_with("  package-")));
        let listed: Vec<&str> = text
            .split_whitespace()
            .filter(|w| w.starts_with("package-"))
            .collect();
        assert_eq!(listed.len(), 14);
    }
}
