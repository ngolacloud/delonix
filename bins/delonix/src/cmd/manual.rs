//! `--help` as a MANUAL: command map, worked examples and cross-references,
//! attached to the `clap::Command` tree before the parse.
//!
//! # The problem this exists to fix
//!
//! Measured before it existed: **1 of the 234 commands carried an example**
//! (the top-level `SHORTCUTS` block), `-h` and `--help` printed byte-identical
//! output, and a group like `container` answered with 27 subcommands in a flat
//! list ordered by nothing in particular — with descriptions that ran to five
//! lines of unbroken prose. Everything needed to USE the command was there;
//! nothing was arranged so it could be read.
//!
//! # The split: `-h` is a reminder, `--help` is the manual
//!
//! clap already distinguishes them (`about` vs `long_about`, `after_help` vs
//! `after_long_help`) and this repo was defining only the short side, so the
//! long side fell back to it. Here the two are given different jobs:
//!
//! * `-h` — one line of `about`, the usage line, the flags. What you want when
//!   you already know the command and forgot a flag's name.
//! * `--help` — the above plus a COMMAND MAP (for groups), EXAMPLES and
//!   SEE ALSO. What you want the first time.
//!
//! # Why a central table and not `after_long_help` on each derive
//!
//! Same reason `po::translate_help` rewrites the tree instead of the source
//! carrying both languages: the material is UNIFORM (every command wants a map,
//! examples, cross-references), and 234 hand-written `after_long_help`
//! attributes are 234 places for the format to drift. One table is auditable in
//! one read, and it is what lets `every_command_has_a_manual_entry` be a real test —
//! a coverage test cannot walk attributes that were never written.
//!
//! The map and the breadcrumb are DERIVED from the `Command` itself, never
//! typed: a subcommand added tomorrow shows up in its parent's map without
//! anyone remembering to add it. Only the editorial material — which category a
//! command belongs to, its examples, what to read next — lives in the table.
//!
//! # i18n
//!
//! The example COMMANDS are the same in every language; their comments are not.
//! Comments go through `po::t` like every other user-facing string, so
//! `--l18n=pt` does not leave an English block in the middle of a Portuguese
//! screen. `see_also` and command names are identifiers — never translated.

use super::kinds as k;
use super::po;

/// The editorial material for one command. Everything structural (the tree, the
/// breadcrumb, the usage line) is read off the `clap::Command` instead.
pub struct Entry {
    /// Space-separated path WITHOUT the binary name: `"container run"`, or
    /// `""` for the root command.
    pub path: &'static str,
    /// Which section of the parent's COMMAND MAP this command appears under.
    /// Empty means the parent has no categories (a small group, where a flat
    /// list reads fine and headings would be ceremony).
    pub group: &'static str,
    /// `(comment, command line)`. The comment is translated; the command is
    /// not. Ordered from the most common use to the most specific — someone
    /// reading the first two lines and stopping should still have the answer.
    pub examples: &'static [(&'static str, &'static str)],
    /// Full paths of related commands, for the SEE ALSO line.
    pub see_also: &'static [&'static str],
}

/// Order in which a group's categories are printed. A category absent here
/// sorts last, alphabetically — a new category shows up rather than vanishing.
const GROUP_ORDER: &[&str] = &[
    "Lifecycle",
    "Create",
    "Inspect",
    "Interact",
    "Configure",
    "Networking",
    k::STORAGE,
    "Declarative",
    "Maintenance",
    "Dashboards",
    "Advanced",
];

/// The order of the ROOT's own map — a separate list, and it has to be.
///
/// The root and a group share three category NAMES (`Networking`, `Storage`,
/// `Declarative`) on purpose: they mean the same thing at both levels, and a
/// second spelling would be a second vocabulary to learn. But they do not share
/// a POSITION. Inside `container`, `Declarative` belongs at the end, after the
/// lifecycle verbs; at the root it belongs next to the other ways of describing
/// what you want. Ranking both from one list put `Storage` and `Networking`
/// after `Engine` — the engine's own housekeeping ahead of two resource
/// families, which reads as an afterthought.
const ROOT_GROUP_ORDER: &[&str] = &[
    "Workloads",
    "Artifacts",
    k::STORAGE,
    "Networking",
    "Clusters",
    "Declarative",
    "Resources",
    "Serve",
    "Engine",
];

include!("manual_entries.rs");

/// The entry for a path, if the table has one.
///
/// `pub` for one reason: `cmd::man` renders the SAME examples and
/// cross-references into roff. A manpage that carried its own copy of the
/// editorial material would drift from `--help` the first time either was
/// edited — and nobody re-reads a manpage to check.
pub fn entry(path: &str) -> Option<&'static Entry> {
    ENTRIES.iter().find(|e| e.path == path)
}

/// Category of a command, for its parent's map.
fn group_of(path: &str) -> &'static str {
    entry(path).map(|e| e.group).unwrap_or("")
}

/// Sort key for a category, in the order that level uses.
fn group_rank(g: &str, root: bool) -> usize {
    let order = if root { ROOT_GROUP_ORDER } else { GROUP_ORDER };
    order.iter().position(|x| *x == g).unwrap_or(order.len())
}

/// Wraps at `width` columns, preserving paragraph breaks and never splitting a
/// word. Deliberately counts CHARS and not display width: help text is prose,
/// where the two only diverge on CJK — and this repo already has
/// `output::display_width` for the tables where the difference bites.
fn wrap(text: &str, width: usize, indent: &str) -> String {
    let mut out = String::new();
    for (i, para) in text.split("\n\n").enumerate() {
        if i > 0 {
            out.push('\n');
        }
        let mut line = String::new();
        for word in para.split_whitespace() {
            if !line.is_empty() && line.chars().count() + 1 + word.chars().count() > width {
                out.push_str(indent);
                out.push_str(&line);
                out.push('\n');
                line.clear();
            }
            if !line.is_empty() {
                line.push(' ');
            }
            line.push_str(word);
        }
        if !line.is_empty() {
            out.push_str(indent);
            out.push_str(&line);
            out.push('\n');
        }
    }
    out
}

/// The COMMAND MAP of a group: its children arranged by category, derived from
/// the live `Command` so it cannot omit a subcommand that exists.
///
/// Returns `None` for a leaf — a command with no children has no map, and an
/// empty heading is worse than no heading.
fn command_map(cmd: &clap::Command, path: &str) -> Option<String> {
    let kids: Vec<&clap::Command> = cmd
        .get_subcommands()
        .filter(|s| s.get_name() != "help" && !s.is_hide_set())
        .collect();
    if kids.is_empty() {
        return None;
    }
    let child_path = |name: &str| {
        if path.is_empty() {
            name.to_string()
        } else {
            format!("{path} {name}")
        }
    };
    // Group name -> the child names under it, in declaration order (which is
    // the order the author chose, and reads as a workflow: run before stop).
    let mut cats: Vec<(&'static str, Vec<String>)> = Vec::new();
    for k in &kids {
        let g = group_of(&child_path(k.get_name()));
        let name = k.get_name().to_string();
        match cats.iter_mut().find(|(c, _)| *c == g) {
            Some((_, v)) => v.push(name),
            None => cats.push((g, vec![name])),
        }
    }
    // A group where nothing was categorised gets no map: the flat `Commands:`
    // list clap already prints says the same thing without a second copy.
    if cats.iter().all(|(g, _)| g.is_empty()) {
        return None;
    }
    cats.sort_by_key(|(g, _)| group_rank(g, path.is_empty()));
    let label_w = cats
        .iter()
        .map(|(g, _)| po::t(g).chars().count())
        .max()
        .unwrap_or(0);
    let mut out = String::new();
    for (g, names) in cats {
        // The uncategorised leftovers of a categorised group are still real
        // commands — printing them under a heading beats dropping them.
        let label = if g.is_empty() {
            po::t("Other")
        } else {
            po::t(g)
        };
        out.push_str(&format!(
            "  {label:<label_w$}  {}\n",
            names.join(" · "),
            label_w = label_w
        ));
    }
    Some(out)
}

/// Assembles the `--help` tail of one command: map, examples, cross-references
/// and the breadcrumb saying where in the tree this command lives.
fn tail(cmd: &clap::Command, path: &str) -> Option<String> {
    let e = entry(path);
    let map = command_map(cmd, path);
    // **The condition this comment described was an AND, and the code tested
    // only the first half.** "Nothing editorial and no map to derive" — but
    // `e?` returned early whenever the manual had no entry, map or no map, and
    // the one path that reaches here with a map and no entry is the ROOT. So
    // every group printed a COMMAND MAP except the single place where the shape
    // of the CLI is decided: `delonix --help` listed all 33 groups flat, with
    // the imperative surface, the generic verbs and the declarative ones in one
    // undifferentiated column.
    let Some(e) = e else {
        return map.map(|m| {
            let mut out = String::new();
            out.push_str(po::t("COMMAND MAP"));
            out.push_str(":\n");
            out.push_str(&m);
            out.trim_end().to_string()
        });
    };
    let mut out = String::new();
    if let Some(map) = map {
        out.push_str(po::t("COMMAND MAP"));
        out.push_str(":\n");
        out.push_str(&map);
        out.push('\n');
    }
    if !e.examples.is_empty() {
        out.push_str(po::t("EXAMPLES"));
        out.push_str(":\n");
        for (i, (comment, line)) in e.examples.iter().enumerate() {
            if i > 0 {
                out.push('\n');
            }
            // The comment is prose and can be long in either language; the
            // command line is code and is NEVER wrapped — a wrapped command is
            // a command that does not run when pasted.
            out.push_str(&wrap(po::t(comment), 74, "  # "));
            out.push_str(&format!("  {line}\n"));
        }
        out.push('\n');
    }
    if !e.see_also.is_empty() {
        out.push_str(po::t("SEE ALSO"));
        out.push_str(":\n");
        let refs: Vec<String> = e.see_also.iter().map(|r| format!("delonix {r}")).collect();
        out.push_str(&wrap(&refs.join(" · "), 76, "  "));
        out.push('\n');
    }
    // The breadcrumb closes every page: it is the answer to "what is this a
    // part of", which a deep tree makes easy to lose.
    if !path.is_empty() {
        let crumb: Vec<&str> = std::iter::once("delonix").chain(path.split(' ')).collect();
        out.push_str(&format!("  {}\n", crumb.join(" › ")));
    }
    Some(out.trim_end().to_string())
}

/// Attaches the manual to the whole tree, recursively. Called once, before the
/// parse, next to `po::translate_help` — and AFTER it, so a map built here
/// carries the already-translated category labels.
pub fn apply(cmd: clap::Command) -> clap::Command {
    apply_at(cmd, String::new())
}

fn apply_at(mut cmd: clap::Command, path: String) -> clap::Command {
    if let Some(t) = tail(&cmd, &path) {
        // `after_long_help` and NOT `after_help`: this is the manual, and the
        // whole point of the split is that `-h` stays short.
        cmd = cmd.after_long_help(t);
    }
    // Categories also decide the ORDER of clap's own `Commands:` list, so the
    // short `-h` gets the same arrangement without a second rendering of it.
    let names: Vec<String> = cmd
        .get_subcommands()
        .map(|s| s.get_name().to_string())
        .collect();
    for name in names {
        let child = if path.is_empty() {
            name.clone()
        } else {
            format!("{path} {name}")
        };
        // The same order the map uses, so clap's flat `Commands:` list and the
        // COMMAND MAP above it never disagree about what comes first.
        let rank = group_rank(group_of(&child), path.is_empty());
        cmd = cmd.mut_subcommand(&name, |s| {
            // `help` is clap's own and has no place in a category.
            let s = if s.get_name() == "help" {
                s
            } else {
                s.display_order(rank)
            };
            apply_at(s, child)
        });
    }
    cmd
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **The root has a COMMAND MAP, and every top-level group is in it.**
    ///
    /// Two failures at once, and the second is the one that rots: a root with no
    /// map prints 33 groups as one flat column, and a map built by hand goes
    /// stale the first time a group is added. This derives the expected set from
    /// the live `clap` tree, so a new group either lands in a category or shows
    /// up here as missing.
    #[test]
    fn the_root_help_maps_every_top_level_group() {
        use clap::CommandFactory;
        let cmd = apply(<crate::Cli as CommandFactory>::command());
        let tail = cmd
            .get_after_long_help()
            .map(|s| s.to_string())
            .unwrap_or_default();
        assert!(
            tail.contains("COMMAND MAP"),
            "the root should have a COMMAND MAP; tail = {tail:?}"
        );
        let missing: Vec<String> = cmd
            .get_subcommands()
            .filter(|s| s.get_name() != "help" && !s.is_hide_set())
            .map(|s| s.get_name().to_string())
            .filter(|n| !tail.contains(n.as_str()))
            .collect();
        assert!(
            missing.is_empty(),
            "top-level group(s) outside the root's COMMAND MAP: {missing:?}"
        );
        // And no group may be dumped under the uncategorised heading: `Other` at
        // the ROOT means somebody added a group and never said what it is.
        assert!(
            !tail.contains("\n  Other "),
            "top-level group(s) without a category — give them one in `manual_entries.rs`:\n{tail}"
        );
    }

    /// **Every group in the tree, not just the root.**
    ///
    /// `the_root_help_maps_every_top_level_group` above only ever looked at the
    /// ROOT's own map. Measured 2026-09-11: four groups deeper in the tree had
    /// the exact defect that test exists to catch, and none of them tripped it —
    /// `config get/set/unset` all had `group: ""`, which made `command_map`'s
    /// own "nothing categorised here" guard fire and suppress the map ENTIRELY
    /// (`delonix config --help` printed no COMMAND MAP at all); `net httproute
    /// apply/rm` did the same for `net httproute --help`; `system boot
    /// disable/enable/status` did the same for `system boot --help`; and
    /// `system namespace` — the ONE empty child among many categorised
    /// siblings — landed under a bare `Other` heading in `system --help`,
    /// which reads as "nobody decided what this is" to anyone reading it.
    ///
    /// This walks the WHOLE tree and asks `command_map` the same two questions
    /// at every node that has real children: did it produce a map at all
    /// (catches the "all empty, suppressed" case), and if so, does any of its
    /// lines start with the uncategorised `Other` heading (catches the "one
    /// empty sibling" case). The root itself is skipped — the test above
    /// already covers it, with an additional check (that the rendered
    /// `--help` text actually reaches the reader) that this one does not need
    /// to repeat.
    #[test]
    fn no_group_has_uncategorized_subcommands() {
        use clap::CommandFactory;

        fn walk(cmd: &clap::Command, path: String, bad: &mut Vec<String>) {
            let kids: Vec<&clap::Command> = cmd
                .get_subcommands()
                .filter(|s| s.get_name() != "help" && !s.is_hide_set())
                .collect();
            if kids.is_empty() {
                return;
            }
            if !path.is_empty() {
                match command_map(cmd, &path) {
                    None => bad.push(format!(
                        "{path}: no subcommand is categorised — the COMMAND MAP is suppressed entirely"
                    )),
                    Some(map) => {
                        if map.lines().any(|l| l.trim_start().starts_with("Other")) {
                            bad.push(format!("{path}: has subcommand(s) under 'Other'"));
                        }
                    }
                }
            }
            for k in &kids {
                let child_path = if path.is_empty() {
                    k.get_name().to_string()
                } else {
                    format!("{path} {}", k.get_name())
                };
                walk(k, child_path, bad);
            }
        }

        let root = <crate::Cli as CommandFactory>::command();
        let mut bad = Vec::new();
        walk(&root, String::new(), &mut bad);
        assert!(
            bad.is_empty(),
            "group(s) with incomplete categorisation in manual_entries.rs:\n  {}",
            bad.join("\n  ")
        );
    }

    #[test]
    fn wraps_text_without_cutting_words() {
        let w = wrap("one two three four five", 12, "  ");
        for line in w.lines() {
            assert!(line.chars().count() <= 14, "line too wide: {line:?}");
        }
        // No word may have been split in the middle.
        let joined: String = w.split_whitespace().collect::<Vec<_>>().join(" ");
        assert_eq!(joined, "one two three four five");
    }

    #[test]
    fn a_word_longer_than_the_width_does_not_loop() {
        // The guard that is missing here is the one that turns a wrap into an
        // infinite loop: a lone word longer than the width has to come out ON A
        // SINGLE line, neither split nor repeated.
        let w = wrap("aaaaaaaaaaaaaaaaaaaaaaaa end", 5, "");
        assert_eq!(w.lines().count(), 2);
        assert_eq!(w.lines().next().unwrap(), "aaaaaaaaaaaaaaaaaaaaaaaa");
    }

    /// A path in the table that does not exist in the CLI is an entry that will
    /// NEVER be shown — and there is no symptom: that command's `--help` ends up
    /// without examples and the table still looks complete. Only a test that
    /// confronts the two trees catches it.
    #[test]
    fn no_entry_points_to_a_nonexistent_command() {
        use clap::CommandFactory;
        let root = crate::Cli::command();
        let mut missing = Vec::new();
        for e in ENTRIES {
            if e.path.is_empty() {
                continue;
            }
            let mut cur = &root;
            let mut ok = true;
            for seg in e.path.split(' ') {
                match cur.get_subcommands().find(|s| s.get_name() == seg) {
                    Some(s) => cur = s,
                    None => {
                        ok = false;
                        break;
                    }
                }
            }
            if !ok {
                missing.push(e.path);
            }
        }
        assert!(
            missing.is_empty(),
            "manual entries for commands that do not exist: {missing:?}"
        );
    }

    /// The inverse: a command without an entry is a `--help` without an example.
    /// STRICT — a `<=` would leave the installed debt reading as green, which is
    /// exactly the defect the i18n ratchet exists to not repeat.
    #[test]
    fn every_command_has_a_manual_entry() {
        use clap::CommandFactory;
        fn walk(cmd: &clap::Command, path: String, out: &mut Vec<String>) {
            // Present-but-empty is the failure this test exists to catch: the
            // skeleton was generated for all 234 commands at once, so "has an
            // entry" is true from the first minute and says nothing. What has
            // to be true is that someone WROTE an example.
            if !path.is_empty() && entry(&path).is_none_or(|e| e.examples.is_empty()) {
                out.push(path.clone());
            }
            for s in cmd.get_subcommands() {
                if s.get_name() == "help" || s.is_hide_set() {
                    continue;
                }
                let child = if path.is_empty() {
                    s.get_name().to_string()
                } else {
                    format!("{path} {}", s.get_name())
                };
                walk(s, child, out);
            }
        }
        let mut without_entry = Vec::new();
        walk(&crate::Cli::command(), String::new(), &mut without_entry);
        assert!(
            without_entry.is_empty(),
            "{} command(s) without an entry in manual_entries.rs:\n  {}",
            without_entry.len(),
            without_entry.join("\n  ")
        );
    }

    /// `about` is what appears NEXT TO the name in the parent command's list; a
    /// whole paragraph there pushes everything else off the screen and the list
    /// can no longer be scanned by eye.
    ///
    /// RATCHET, not tolerance: measured at 74 of 236 commands before this
    /// session (the worst at 447 characters). The test fails if the number goes
    /// UP (a new command with a doc-comment that does not break) and also if it
    /// goes DOWN — you shorten one and lower the constant in the same commit. A
    /// `<=` left the debt reading as green, which is the defect the i18n ratchet
    /// already exists to not repeat.
    ///
    /// The fix is always the same and deletes nothing: an empty `///` line after
    /// the first sentence sends the rest to `long_about`, which is what `--help`
    /// shows.
    #[test]
    fn the_short_description_fits_on_one_line() {
        use clap::CommandFactory;
        const LIMIT: usize = 110;
        const PENDING_LONG: usize = 0;
        fn walk(cmd: &clap::Command, path: String, out: &mut Vec<(usize, String)>) {
            if let Some(a) = cmd.get_about() {
                let n = a.to_string().chars().count();
                if !path.is_empty() && n > LIMIT {
                    out.push((n, path.clone()));
                }
            }
            for s in cmd.get_subcommands() {
                if s.get_name() == "help" || s.is_hide_set() {
                    continue;
                }
                let child = if path.is_empty() {
                    s.get_name().to_string()
                } else {
                    format!("{path} {}", s.get_name())
                };
                walk(s, child, out);
            }
        }
        let mut long_ones = Vec::new();
        walk(&crate::Cli::command(), String::new(), &mut long_ones);
        long_ones.sort_by_key(|(n, _)| std::cmp::Reverse(*n));
        let list: Vec<String> = long_ones
            .iter()
            .map(|(n, p)| format!("{p} ({n} characters)"))
            .collect();
        assert_eq!(
            long_ones.len(),
            PENDING_LONG,
            "short descriptions over {LIMIT} characters: {} (expected \
             {PENDING_LONG}). Break the doc-comment with an empty `///` line \
             after the first sentence — nothing is deleted, the rest moves to \
             long_about.\n  {}",
            long_ones.len(),
            list.join("\n  ")
        );
    }

    /// Splits a published example into the argv `clap` would receive, or `None`
    /// when there is nothing to validate (a pipeline whose first half is not
    /// ours, an example that does not invoke the binary).
    ///
    /// Deliberately conservative: cuts at the first `|`, `&&`, `;` or `>`, drops
    /// an environment prefix (`DELONIX_ROOT=… delonix …`) and replaces a command
    /// substitution with an opaque value — what is validated is the SHAPE of the
    /// command, not what a shell would make of it.
    fn example_argv(line: &str) -> Option<Vec<String>> {
        let mut cut = line.to_string();
        for sep in ["|", "&&", ";", ">"] {
            if let Some(i) = cut.find(sep) {
                cut.truncate(i);
            }
        }
        while let (Some(a), Some(b)) = (cut.find("$("), cut.find(')')) {
            if b < a {
                break;
            }
            cut.replace_range(a..=b, "SUBST");
        }
        let mut argv: Vec<String> = Vec::new();
        let mut current = String::new();
        let mut quote: Option<char> = None;
        for ch in cut.chars() {
            match (quote, ch) {
                (Some(q), c) if c == q => quote = None,
                (Some(_), c) => current.push(c),
                (None, c @ ('"' | '\'')) => quote = Some(c),
                (None, c) if c.is_whitespace() => {
                    if !current.is_empty() {
                        argv.push(std::mem::take(&mut current));
                    }
                }
                (None, c) => current.push(c),
            }
        }
        if !current.is_empty() {
            argv.push(current);
        }
        while argv
            .first()
            .is_some_and(|t| t.contains('=') && !t.starts_with('-'))
        {
            argv.remove(0);
        }
        match argv.first().map(String::as_str) {
            Some("delonix") => Some(argv),
            _ => None,
        }
    }

    /// **A published example has to PARSE against the live `clap` tree.**
    ///
    /// The sibling test below requires an example to NAME its command, and that
    /// is a different question: an example can name the right command and pass
    /// it a flag that does not exist. Measured 2026-09-10 against the `--help` of
    /// all 244 leaves: **five published examples were refused by the binary
    /// itself** — `container rm -f -v web` (docker's `-v`, which this engine
    /// never had), `cluster drain lab lab-w1` and `cluster uncordon lab lab-w1`
    /// (the cluster is `--name`, not a second positional) and `cluster upgrade …
    /// --node lab-w1` (the node is positional; the flag never existed, and the
    /// command's own prose said the same).
    ///
    /// The cost is the usual one for wrong documentation: whoever copies gets
    /// `rc=2`, and whoever "fixes" it by hand can hit something else —
    /// `cluster drain lab` parses, and drains a NODE called `lab`.
    #[test]
    fn published_examples_parse_against_the_live_clap_tree() {
        use clap::error::ErrorKind;
        use clap::CommandFactory;
        let mut wrong = Vec::new();
        for e in ENTRIES {
            for (_, line) in e.examples {
                let Some(argv) = example_argv(line) else {
                    continue;
                };
                let cmd = <crate::Cli as CommandFactory>::command();
                if let Err(err) = cmd.try_get_matches_from(argv) {
                    // Only the classes that prove the EXAMPLE is wrong. An
                    // invalid value (`<version>`, `…`) or a missing file is a
                    // consequence of an example being an example.
                    if matches!(
                        err.kind(),
                        ErrorKind::UnknownArgument
                            | ErrorKind::InvalidSubcommand
                            | ErrorKind::TooManyValues
                    ) {
                        let why = err
                            .to_string()
                            .lines()
                            .next()
                            .unwrap_or_default()
                            .to_string();
                        wrong.push(format!("{}: {line}\n      -> {why}", e.path));
                    }
                }
            }
        }
        assert!(
            wrong.is_empty(),
            "example(s) the binary itself refuses:\n  {}",
            wrong.join("\n  ")
        );
    }

    /// An example has to start with the command it documents — otherwise it
    /// documents another. Catches the copy-paste mistake between neighbouring
    /// entries, which is invisible to the eye in a table of 234 lines.
    #[test]
    fn examples_invoke_the_command_they_document() {
        let mut bad = Vec::new();
        for e in ENTRIES {
            if e.path.is_empty() {
                continue;
            }
            for (_, line) in e.examples {
                // An example can legitimately be a pipeline or carry an
                // environment prefix (`DELONIX_ROOT=… delonix …`); what is
                // required is that the command path APPEARS in it.
                if !line.contains(e.path) {
                    bad.push(format!("{}: {line}", e.path));
                }
            }
        }
        assert!(
            bad.is_empty(),
            "example(s) that do not invoke their own command:\n  {}",
            bad.join("\n  ")
        );
    }
}
