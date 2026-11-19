//! Docker-style grouped help for `delonix`.
//!
//! clap 4.6 does not group subcommands — `#[command(help_heading = …)]` exists
//! only for arguments, and `next_help_heading` likewise. So clap's flat
//! `Commands:` list put every subcommand of a node in one undifferentiated
//! column: ~79 at the root, ~35 under `container`.
//!
//! This module renders the SAME help clap would, then regroups the flat command
//! block into semantic sections à la `docker --help`. It reuses clap's own
//! rendered lines verbatim — about text, alias annotations and wrapping all come
//! from clap — and only moves them under section headings. Everything else (the
//! about paragraph, `Usage:`, `Options:` and the editorial tail) is kept
//! byte-for-byte.
//!
//! Two taxonomies, one mechanism:
//! * the ROOT groups by reach — Common / Management / Orchestration / Commands;
//! * every other GROUP (`container`, `image`, `net netns`, …) groups by the
//!   categories `manual::group_of` already assigns for the `COMMAND MAP`, so
//!   there is a single source of truth. The root keeps its `COMMAND MAP` tail
//!   (a different lens, and a `docs/gen.py` contract); sub-groups drop it, since
//!   their listing now carries the same categories.
//!
//! `delonix <leaf> --help` (e.g. `delonix run --help`) is untouched — a leaf has
//! no subcommand list to group, and its flags are a later slice.

use std::collections::BTreeMap;

use super::manual;

/// The most reached-for verbs — shown first, in workflow order (not alphabetical).
const COMMON: &[&str] = &[
    "run", "exec", "ps", "logs", "build", "pull", "push", "images", "apply", "stack", "vm",
    "cluster", "init",
];

/// Noun-first management groups: each owns a family of resources.
const MANAGEMENT: &[&str] = &[
    "container",
    "image",
    "pod",
    "workload",
    "volume",
    "secret",
    "bucket",
    "network",
    "net",
    "system",
    "provider",
    "policy",
    "systemcontainer",
    "oci-runtime",
];

/// Declarative / orchestration surface: plan, converge, inspect, generic verbs.
const ORCHESTRATION: &[&str] = &[
    "plan",
    "diff",
    "drift",
    "compose",
    "gitops",
    "migrate",
    "manifest",
    "delete",
    "describe",
    "get",
    "explain",
    "api-resources",
];

/// Root section headings, in display order. Index 3 (`Commands:`) is the
/// fallback for anything not named in the three explicit lists — so a future
/// subcommand lands in the catch-all instead of vanishing. English for now; the
/// i18n of these headings is a later slice (see the session contract).
const HEADINGS: [&str; 4] = [
    "Common Commands:",
    "Management Commands:",
    "Orchestration Commands:",
    "Commands:",
];

/// Flag sections for `run`/`create` (the shared `RunArgs`), keyed by clap arg id
/// (the struct field name). One central map instead of ~100 scattered
/// `#[arg(help_heading = …)]`, so the taxonomy is in one testable place and
/// `flag_heading_covers_every_runargs_flag` can prove nothing falls through.
///
/// Headings are English for now (like the command-listing slices); their i18n is
/// a later slice. Positionals (`image`, `command`) are intentionally absent — a
/// heading would pull them out of clap's `Arguments:` block.
const FLAG_SECTIONS: &[(&str, &[&str])] = &[
    (
        "General",
        &[
            "detach",
            "interactive",
            "tty",
            "name",
            "hostname",
            "user",
            "entrypoint",
            "workdir",
            "pull",
            "platform",
            "init",
            "umask",
            "cidfile",
            "rm",
        ],
    ),
    (
        "Environment & Config",
        &[
            "env",
            "env_file",
            "labels",
            "label_file",
            "annotations",
            "secret",
            "secret_files",
            "config",
            "config_files",
        ],
    ),
    (
        "Networking",
        &[
            "net",
            "namespace",
            "expose",
            "publish",
            "publish_all",
            "ip",
            "network_alias",
            "add_host",
            "dns",
            "dns_search",
            "dns_option",
            "net_bps",
            "net_burst",
            "knows",
            "knows_none",
            "pod",
        ],
    ),
    (
        "Storage & Mounts",
        &[
            "volumes",
            "volumes_from",
            "mounts_flag",
            "tmpfs",
            "shm_size",
            "devices",
            "read_only",
            "masked_path",
            "readonly_path",
            "gpus",
        ],
    ),
    (
        "Resources",
        &[
            "memory",
            "memory_reservation",
            "memory_swap",
            "pids_limit",
            "cpus",
            "cpu_weight",
            "cpu_shares",
            "cpuset",
            "oom_score_adj",
            "io_weight",
            "blkio_weight",
            "device_read_bps",
            "device_write_bps",
            "device_read_iops",
            "device_write_iops",
            "ulimit",
            "sysctl",
        ],
    ),
    (
        "Security",
        &[
            "privileged",
            "cap_add",
            "cap_drop",
            "security_opt",
            "apparmor",
            "selinux",
            "userns",
            "no_userns",
            "allow_source",
            "no_source_check",
            "detect",
            "group_add",
        ],
    ),
    (
        "Health & Lifecycle",
        &[
            "restart",
            "stop_signal",
            "stop_timeout",
            "wait_healthy",
            "wait_timeout",
            "health_cmd",
            "health_interval",
            "health_timeout",
            "health_retries",
            "health_start_period",
            "no_healthcheck",
            "health_http",
            "health_tcp",
        ],
    ),
    (
        "Runtime & Namespaces",
        &[
            "host_pid",
            "host_ipc",
            "pid_mode",
            "ipc_mode",
            "uts_mode",
            "cgroupns",
            "log_driver",
            "log_file",
            "log_cri",
        ],
    ),
];

/// The heading for a `RunArgs` flag id, or `None` if it is not in the map
/// (a positional, a hidden-only internal flag, or a flag added without a home).
fn flag_section(id: &str) -> Option<&'static str> {
    FLAG_SECTIONS
        .iter()
        .find(|(_, ids)| ids.contains(&id))
        .map(|(h, _)| *h)
}

/// Distinctive `RunArgs` flags — a command carrying all of them IS `run`/`create`
/// (top-level or under `container`), and no other command shares the set.
fn is_run_args(cmd: &clap::Command) -> bool {
    let has = |id: &str| cmd.get_arguments().any(|a| a.get_id() == id);
    has("detach") && has("publish") && has("entrypoint") && has("volumes")
}

/// Attach the `RunArgs` flag headings wherever that struct is used (`run`,
/// `create`, `container run`, `container create`), recursively. A no-op for
/// every other command. Called from `build_command`, after translation and the
/// manual tail, so it composes with both.
pub fn with_flag_headings(mut cmd: clap::Command) -> clap::Command {
    if is_run_args(&cmd) {
        let ids: Vec<String> = cmd
            .get_arguments()
            .map(|a| a.get_id().as_str().to_string())
            .collect();
        for id in ids {
            if let Some(heading) = flag_section(&id) {
                cmd = cmd.mut_arg(&id, |a| a.help_heading(Some(heading)));
            }
        }
    }
    let names: Vec<String> = cmd
        .get_subcommands()
        .map(|s| s.get_name().to_string())
        .collect();
    for name in names {
        cmd = cmd.mut_subcommand(name, with_flag_headings);
    }
    cmd
}

/// `(section index, order within the explicit list)` for a top-level command.
/// Total by construction: a name in none of the three lists falls into section 3
/// (`Commands:`), whose order is decided by appearance, not by `ord`.
fn place(name: &str) -> (usize, usize) {
    for (sec, list) in [COMMON, MANAGEMENT, ORCHESTRATION].iter().enumerate() {
        if let Some(i) = list.iter().position(|n| *n == name) {
            return (sec, i);
        }
    }
    (3, usize::MAX)
}

/// argv minus the program name and the global `--l18n <v>`/`--l18n=<v>` flag,
/// which parses ahead of the subcommand (it selects the help language via
/// `po::peek_lang`) and must not mask a help request.
fn effective_tokens(argv: &[String]) -> Vec<&str> {
    let mut rest = argv.iter().skip(1);
    let mut tokens = Vec::new();
    while let Some(a) = rest.next() {
        if a == "--l18n" {
            rest.next(); // drop its value
            continue;
        }
        if a.starts_with("--l18n=") {
            continue;
        }
        tokens.push(a.as_str());
    }
    tokens
}

/// True when the argv is a request for the TOP-LEVEL help — `delonix` with no
/// command, `delonix help`, `delonix --help` or `delonix -h`.
///
/// `delonix help <sub>` and `delonix <sub> --help` are NOT top-level: they are
/// routed to [`group_help_path`] (groups) or left to clap (leaves).
pub fn wants_top_help(argv: &[String]) -> bool {
    matches!(
        effective_tokens(argv).as_slice(),
        [] | ["--help"] | ["-h"] | ["help"]
    )
}

/// The path of a GROUP whose help is explicitly requested — `delonix <path…>
/// --help`, `delonix <path…> -h`, or `delonix help <path…>` — provided the path
/// resolves to a command that HAS subcommands. Returns `None` for a leaf (clap
/// renders it) or an unknown path (clap errors), so only the groupable case is
/// intercepted. The top level is handled by [`wants_top_help`].
pub fn group_help_path(argv: &[String], root: &clap::Command) -> Option<Vec<String>> {
    let toks = effective_tokens(argv);
    let path: &[&str] = if toks.first() == Some(&"help") {
        &toks[1..]
    } else if matches!(toks.last(), Some(&"--help") | Some(&"-h")) {
        &toks[..toks.len() - 1]
    } else {
        return None;
    };
    if path.is_empty() {
        return None;
    }
    let mut cur = root;
    for seg in path {
        cur = cur.find_subcommand(seg)?;
    }
    cur.get_subcommands()
        .any(|s| s.get_name() != "help")
        .then(|| path.iter().map(|s| (*s).to_string()).collect())
}

/// Render the grouped TOP-LEVEL help for `cmd` (the root command, already
/// translated and with the manual tail attached by `build_command`).
pub fn render(mut cmd: clap::Command) -> String {
    let full = cmd.render_long_help().to_string();
    regroup(&full, |name, appear| {
        let (sec, idx) = place(name);
        let order = if sec == 3 { appear } else { idx };
        (sec, order, HEADINGS[sec].to_string())
    })
}

/// Render the grouped help of the GROUP at `path`, from the `native_help` string
/// clap produced for it. Children are placed under the same categories
/// `manual::group_of` assigns to the `COMMAND MAP`, in the same order.
pub fn render_group(native_help: &str, path: &[String], _root: &clap::Command) -> String {
    let prefix = path.join(" ");
    regroup(native_help, |name, appear| {
        let child_path = format!("{prefix} {name}");
        let label = manual::group_of(&child_path);
        let rank = manual::group_rank(label, false);
        let heading = if label.is_empty() {
            "Commands:".to_string()
        } else {
            format!("{label} Commands:")
        };
        (rank, appear, heading)
    })
}

/// Shared core: take a clap-rendered help string, replace its single flat
/// `Commands:` block with sections, and keep everything else byte-for-byte.
///
/// `classify(name, appearance)` returns `(section_rank, order_within_section,
/// heading)` for each child. Sections render in ascending `section_rank`;
/// children render in ascending `order` within a section. The command LINES are
/// clap's own, moved verbatim, so wrapped about text rides with its command.
fn regroup(full: &str, classify: impl Fn(&str, usize) -> (usize, usize, String)) -> String {
    const CMDS: &str = "\nCommands:\n";
    const OPTS: &str = "\nOptions:";
    let Some(cmds_at) = full.find(CMDS) else {
        return full.to_string();
    };
    let block_start = cmds_at + CMDS.len();
    let Some(opts_rel) = full[block_start..].find(OPTS) else {
        return full.to_string();
    };
    let opts_at = block_start + opts_rel;

    let head = full[..cmds_at].trim_end(); // about paragraph + `Usage:` line
    let block = &full[block_start..opts_at]; // the flat command lines
    let after_options = &full[opts_at + OPTS.len()..]; // `-h`/`-V` + editorial tail

    // One entry per command: clap lists each command at column 2 and indents a
    // wrapped about line past the name column, so a line whose third byte is
    // non-space starts a command and anything else rides with the one above it.
    let mut entries: Vec<(String, String)> = Vec::new();
    for line in block.lines() {
        let is_head = line.starts_with("  ")
            && line
                .as_bytes()
                .get(2)
                .is_some_and(|b| !b.is_ascii_whitespace());
        if is_head {
            let name = line[2..]
                .split_whitespace()
                .next()
                .unwrap_or("")
                .to_string();
            entries.push((name, line.to_string()));
        } else if let Some(last) = entries.last_mut() {
            last.1.push('\n');
            last.1.push_str(line);
        }
    }
    // Drop clap's auto-generated `help` subcommand from the listing, like the
    // `COMMAND MAP` already does and like `docker <group> --help` does — it is
    // not a resource verb, and leaving it in produced a lonely one-line section.
    // This also makes `delonix <group> --help` and `delonix help <group>` match
    // (clap renders the `help` entry in the former but not the latter).
    entries.retain(|(name, _)| name != "help");

    // Bucket by `(section_rank, heading)` so sections sort by rank (headings only
    // tie-break the rare case of two labels sharing a rank).
    let mut secs: BTreeMap<(usize, String), Vec<(usize, String)>> = BTreeMap::new();
    for (appear, (name, rendered)) in entries.into_iter().enumerate() {
        let (rank, order, heading) = classify(&name, appear);
        secs.entry((rank, heading))
            .or_default()
            .push((order, rendered));
    }

    let mut grouped = String::new();
    for ((_, heading), mut items) in secs {
        items.sort_by_key(|(o, _)| *o);
        if !grouped.is_empty() {
            grouped.push_str("\n\n");
        }
        grouped.push_str(&heading);
        for (_, rendered) in items {
            grouped.push('\n');
            grouped.push_str(&rendered);
        }
    }

    let mut out = format!("{head}\n\n{grouped}\n\nOptions:{after_options}");
    if !out.ends_with('\n') {
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;
    use std::collections::HashSet;

    /// Every clap subcommand lands in exactly one root help group, and the three
    /// explicit lists name only real, non-overlapping commands. The fallback to
    /// `Commands:` is what makes "exactly one" hold for a command added later.
    #[test]
    fn every_command_is_in_exactly_one_help_group() {
        let cmd = crate::Cli::command();
        let known: HashSet<&str> = cmd.get_subcommands().map(|s| s.get_name()).collect();

        for name in &known {
            let hits = [COMMON, MANAGEMENT, ORCHESTRATION]
                .iter()
                .filter(|l| l.contains(name))
                .count();
            assert!(
                hits <= 1,
                "`{name}` appears in {hits} explicit help groups; a command must be in at most one"
            );
            let (sec, _) = place(name);
            assert!(sec < HEADINGS.len());
        }

        for n in COMMON.iter().chain(MANAGEMENT).chain(ORCHESTRATION) {
            assert!(
                known.contains(n),
                "help group lists `{n}`, which is not a top-level subcommand — a rename or removal left it dangling"
            );
        }
    }

    /// The four root sections render in order and the editorial `COMMAND MAP`
    /// tail, plus a flagship command in its section, survive the regrouping.
    #[test]
    fn top_help_renders_four_sections_in_order() {
        let out = render(crate::build_command());
        let common = out
            .find("Common Commands:")
            .expect("Common Commands heading");
        let mgmt = out
            .find("Management Commands:")
            .expect("Management Commands heading");
        let orch = out
            .find("Orchestration Commands:")
            .expect("Orchestration Commands heading");
        let commands = out.find("\nCommands:\n").expect("Commands heading");
        assert!(
            common < mgmt && mgmt < orch && orch < commands,
            "sections must render in Common → Management → Orchestration → Commands order"
        );
        assert!(
            out.contains("COMMAND MAP:"),
            "the root manual tail must survive"
        );
        assert!(
            out.contains("Usage: delonix"),
            "the usage line must survive"
        );
        let run = out.find("\n  run ").expect("run command line");
        assert!(
            common < run && run < mgmt,
            "`run` must render under Common Commands"
        );
    }

    #[test]
    fn wants_top_help_matches_only_the_four_entrypoints() {
        let argv = |xs: &[&str]| {
            std::iter::once("delonix")
                .chain(xs.iter().copied())
                .map(String::from)
                .collect::<Vec<_>>()
        };
        assert!(wants_top_help(&argv(&[])));
        assert!(wants_top_help(&argv(&["--help"])));
        assert!(wants_top_help(&argv(&["-h"])));
        assert!(wants_top_help(&argv(&["help"])));
        assert!(wants_top_help(&argv(&["--l18n", "pt"])));
        assert!(wants_top_help(&argv(&["--l18n", "pt", "--help"])));
        assert!(wants_top_help(&argv(&["--l18n=pt", "help"])));
        assert!(!wants_top_help(&argv(&["run"])));
        assert!(!wants_top_help(&argv(&["help", "container"])));
        assert!(!wants_top_help(&argv(&["container", "--help"])));
        assert!(!wants_top_help(&argv(&["--version"])));
        assert!(!wants_top_help(&argv(&["-V"])));
    }

    /// `group_help_path` fires only for a group path, by any of the three
    /// spellings, and never for a leaf, the top level, or `--version`.
    #[test]
    fn group_help_path_fires_only_for_group_paths() {
        let root = crate::build_command();
        let argv = |xs: &[&str]| {
            std::iter::once("delonix")
                .chain(xs.iter().copied())
                .map(String::from)
                .collect::<Vec<_>>()
        };
        assert_eq!(
            group_help_path(&argv(&["container", "--help"]), &root).as_deref(),
            Some(["container".to_string()].as_slice())
        );
        assert_eq!(
            group_help_path(&argv(&["help", "image"]), &root).as_deref(),
            Some(["image".to_string()].as_slice())
        );
        assert_eq!(
            group_help_path(&argv(&["net", "netns", "-h"]), &root).as_deref(),
            Some(["net".to_string(), "netns".to_string()].as_slice())
        );
        // A leaf, the top level and non-help argv do not fire.
        assert_eq!(group_help_path(&argv(&["run", "--help"]), &root), None);
        assert_eq!(group_help_path(&argv(&["--help"]), &root), None);
        assert_eq!(
            group_help_path(&argv(&["container", "run", "--help"]), &root),
            None
        );
        assert_eq!(group_help_path(&argv(&["container"]), &root), None);
        assert_eq!(group_help_path(&argv(&["nope", "--help"]), &root), None);
    }

    /// Render EVERY group's help and assert no visible child is dropped by the
    /// regrouping, that the headings are sections (not the single flat list),
    /// and that no sub-group still carries a `COMMAND MAP` tail.
    #[test]
    fn render_group_keeps_every_child_of_every_group() {
        let root = crate::build_command();

        fn visible_children(node: &clap::Command) -> Vec<String> {
            node.get_subcommands()
                .filter(|s| s.get_name() != "help" && !s.is_hide_set())
                .map(|s| s.get_name().to_string())
                .collect()
        }

        fn walk(root: &clap::Command, node: &clap::Command, path: Vec<String>) {
            let kids = visible_children(node);
            if kids.is_empty() {
                return;
            }
            if !path.is_empty() {
                let mut argv = vec!["delonix".to_string()];
                argv.extend(path.iter().cloned());
                argv.push("--help".to_string());
                let native = crate::build_command()
                    .try_get_matches_from_mut(&argv)
                    .expect_err("--help always yields a DisplayHelp error")
                    .render()
                    .to_string();
                let out = render_group(&native, &path, root);
                for k in &kids {
                    assert!(
                        out.contains(&format!("\n  {k} ")) || out.contains(&format!("\n  {k}\n")),
                        "`{}`: child `{k}` was dropped from the grouped help",
                        path.join(" ")
                    );
                }
                assert!(
                    !out.contains("COMMAND MAP:"),
                    "`{}`: a sub-group must not keep a COMMAND MAP tail",
                    path.join(" ")
                );
            }
            for child in node
                .get_subcommands()
                .filter(|s| s.get_name() != "help" && !s.is_hide_set())
            {
                let mut p = path.clone();
                p.push(child.get_name().to_string());
                walk(root, child, p);
            }
        }

        walk(&root, &root, vec![]);
    }

    /// Every VISIBLE, non-positional flag of `RunArgs` is assigned exactly one
    /// heading — so a flag added to `RunArgs` later cannot silently fall back to
    /// the bare `Options:` section, and the map names no stale ids.
    #[test]
    fn flag_heading_covers_every_runargs_flag() {
        let root = crate::build_command();
        let run = root.find_subcommand("run").expect("run exists");
        assert!(is_run_args(run), "`run` must carry the RunArgs signature");

        for arg in run.get_arguments() {
            let id = arg.get_id().as_str();
            // Positionals live under `Arguments:`, hidden args never show.
            if arg.is_positional() || arg.is_hide_set() {
                continue;
            }
            // clap's auto `--help`/`--version` legitimately stay under `Options:`.
            if matches!(id, "help" | "version") {
                continue;
            }
            assert!(
                flag_section(id).is_some(),
                "`run --{id}` has no flag section — add it to FLAG_SECTIONS"
            );
            // And the heading actually took on the built command.
            assert!(
                arg.get_help_heading().is_some(),
                "`run --{id}` was not assigned its help_heading by with_flag_headings"
            );
        }

        // The map names only real RunArgs ids (catch a rename/removal).
        let ids: HashSet<&str> = run.get_arguments().map(|a| a.get_id().as_str()).collect();
        for (_, flags) in FLAG_SECTIONS {
            for f in *flags {
                assert!(
                    ids.contains(f),
                    "FLAG_SECTIONS lists `{f}`, not a RunArgs flag"
                );
            }
        }
    }
}
