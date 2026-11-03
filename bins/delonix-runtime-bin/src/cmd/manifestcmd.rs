//! `delonix manifest` — what can be done to a manifest without touching the host.
//!
//! The three verbs here were already implemented and reachable, each under a
//! different roof: `stack validate`, `stack apply --dry-run` and `schema print`.
//! What they have in common is the property that names the group — **none of
//! them reads or writes the machine** — and that is the one thing a person
//! needs to know before running an unfamiliar command against production.
//!
//! They route to the existing implementations. A second validator beside the
//! first is how the two start disagreeing about what a valid manifest is.

use delonix_model::Result;
use std::path::PathBuf;

#[derive(clap::Subcommand)]
pub enum ManifestCmd {
    /// Check a manifest against the schema, the references and the graph.
    Validate {
        #[arg(short = 'f', long = "file", value_hint = clap::ValueHint::FilePath)]
        file: Option<PathBuf>,
        /// Refuse an unknown field instead of warning about it.
        #[arg(long)]
        strict: bool,
    },
    /// Print the manifest as the engine will read it, with defaults filled in.
    ///
    /// The spelling the CLI restructuring asks for is `render`: round-trip every
    /// document through its typed spec so the `#[serde(default)]` values become
    /// visible — the manifest `stack apply --dry-run` prints, without the plan
    /// it then runs against the machine. Secrets are NOT resolved: a rendered
    /// manifest is meant to be read by a person and pasted into a review.
    Render {
        #[arg(short = 'f', long = "file", value_hint = clap::ValueHint::FilePath)]
        file: Option<PathBuf>,
    },
    /// The JSON Schema, generated from the Rust types (ADR-0007).
    Schema {
        /// One Kind instead of the whole document.
        #[arg(long)]
        kind: Option<String>,
    },
    /// Rewrite an old manifest to the catalog-v1 apiVersions (ADR-0086).
    ///
    /// The nine `v1alpha1` groups fold into the six the catalog is born with at
    /// `v1beta1` (`security`→`policy`, `artifact`/`gitops`→`core`,
    /// `gateway`→`networking`, `infrastructure`→`cluster`). Only the `apiVersion`
    /// lines change — the specs are unchanged in this phase, and every comment and
    /// byte of layout is preserved. The old groups still load with a warning; this
    /// is how the warning is made to stop. Prints the result by default; `--write`
    /// rewrites the file in place.
    Convert {
        #[arg(short = 'f', long = "file", value_hint = clap::ValueHint::FilePath)]
        file: Option<PathBuf>,
        /// Rewrite the file in place instead of printing the converted manifest.
        #[arg(long)]
        write: bool,
    },
}

pub fn run(action: ManifestCmd) -> Result<()> {
    match action {
        ManifestCmd::Validate { file, strict } => {
            super::stack::run(super::stack::StackCmd::Validate { file, strict })
        }
        // The renderer `apply --dry-run` prints with, and only that: the
        // dry-run goes on to plan against the machine and refuse what the
        // apply would, and a review copy must not depend on who owns what.
        ManifestCmd::Render { file } => super::stack::render(file),
        ManifestCmd::Schema { kind } => {
            super::schema::run(super::schema::SchemaCmd::Print { kind })
        }
        ManifestCmd::Convert { file, write } => convert(file, write),
    }
}

/// `delonix manifest convert` — the file rewriter of ADR-0086 D5.
fn convert(file: Option<PathBuf>, write: bool) -> Result<()> {
    use delonix_model::Error;
    let path = super::manifest::resolve_path(file)?;
    let text = std::fs::read_to_string(&path)
        .map_err(|e| Error::Invalid(format!("{}: {e}", path.display())))?;
    let (converted, changes) = super::manifest::convert_manifest_text(&text);
    if changes.is_empty() {
        eprintln!(
            "{}",
            super::po::tf(
                "{path}: already on the catalog-v1 apiVersions — nothing to convert",
                &[("path", &path.display().to_string())],
            )
        );
        if !write {
            print!("{text}");
        }
        return Ok(());
    }
    // Explain each change of behaviour (ADR-0086 D5): which group/version moved.
    for (old, new) in &changes {
        eprintln!(
            "{}",
            super::po::tf("  {old} -> {new}", &[("old", old), ("new", new)])
        );
    }
    eprintln!(
        "{}",
        super::po::tf(
            "{n} apiVersion(s) converted (group/version only; the specs are unchanged)",
            &[("n", &changes.len().to_string())],
        )
    );
    if write {
        std::fs::write(&path, &converted)
            .map_err(|e| Error::Invalid(format!("{}: {e}", path.display())))?;
        eprintln!(
            "{}",
            super::po::tf("{path}: rewritten", &[("path", &path.display().to_string())])
        );
    } else {
        print!("{converted}");
    }
    Ok(())
}

/// Why the conversion verb is `convert` and not `migrate`, now that it exists.
///
/// It was deliberately absent while every spelling was accepted identically —
/// a rewritten file would have behaved exactly like the one it replaced, so the
/// verb would have promised work it did not do. ADR-0086 D5 changed that: the
/// old `v1alpha1` groups now load with a DEPRECATION warning and leave in the
/// next major, so a rewrite has real work — it stops the warning and future-
/// proofs the file. The verb is named `convert` (the ADR's word), rewrites the
/// `apiVersion` lines only, and preserves every comment and field; `migrate`
/// the spelling stays absent, there being one verb for the one job.
#[cfg(test)]
pub(crate) const WHY_CONVERT_EXISTS: &str =
    "ADR-0086 D5 deprecates the old groups, so a convert has real work: it stops the warning";

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    /// The group must not grow a verb that touches the host. That property is
    /// the only reason to put these three together, and it is not enforced by
    /// anything except this test and the reviewer reading it.
    #[test]
    fn the_group_only_carries_host_free_verbs() {
        let cmd = crate::Cli::command();
        let manifest = cmd
            .get_subcommands()
            .find(|c| c.get_name() == "manifest")
            .expect("`manifest` is declared");
        let mut names: Vec<&str> = manifest.get_subcommands().map(|c| c.get_name()).collect();
        names.sort_unstable();
        assert_eq!(
            names,
            ["convert", "render", "schema", "validate"],
            "a verb that reads or writes the host does not belong in `manifest`"
        );
    }

    /// The conversion verb is `convert` (ADR-0086 D5), and `migrate` stays out —
    /// one verb for the one job, and the reason is written rather than left as a
    /// gap someone later reads as an omission.
    #[test]
    fn convert_exists_and_migrate_does_not() {
        assert!(!WHY_CONVERT_EXISTS.is_empty());
        let cmd = crate::Cli::command();
        let manifest = cmd
            .get_subcommands()
            .find(|c| c.get_name() == "manifest")
            .unwrap();
        let names: Vec<&str> = manifest.get_subcommands().map(|c| c.get_name()).collect();
        assert!(names.contains(&"convert"), "`convert` is the catalog-v1 rewriter");
        assert!(
            !names.contains(&"migrate"),
            "`migrate` stays absent — `convert` is the one verb for the job: {WHY_CONVERT_EXISTS}"
        );
    }
}
