//! `delonix kube generate` — generates a Kubernetes manifest (`kind: Pod`) from
//! a container or pod already running in the engine. It's the "ran it locally,
//! now give me the YAML for k8s" path (equivalent to `podman generate kube`).

use std::path::PathBuf;

use clap::Subcommand;
use serde::Deserialize;

use delonix_compute::pod::PodSpec;
use delonix_compute::Container;
use delonix_model::{Error, Result};
use delonix_state::Store;

use super::util::open_stores;

#[derive(Subcommand)]
pub enum KubeCmd {
    /// Generates a `kind: Pod` from a container (or from every member of a
    /// pod) and prints it to stdout. Equivalent to `podman kube generate`.
    Generate {
        #[arg(add = clap_complete::engine::ArgValueCandidates::new(super::complete::containers_or_pods))]
        name: String,
    },
    /// Runs the workloads described in a **standard Kubernetes YAML** manifest
    /// (`apiVersion: v1, kind: Pod`, or `apps/v1, kind: Deployment` run as a
    /// single pod). Equivalent to `podman kube play`. The delonix `PodSpec`
    /// already uses k8s field names, so a plain k8s Pod runs as-is.
    Play {
        #[arg(value_name = "FICHEIRO")]
        file: PathBuf,
    },
}

pub fn run(action: KubeCmd) -> Result<()> {
    match action {
        KubeCmd::Generate { name } => {
            let (_images, store) = open_stores()?;
            cmd_generate(&store, &name)
        }
        KubeCmd::Play { file } => cmd_play(&file),
    }
}

/// One document of a standard Kubernetes manifest — just enough to route it:
/// the api/kind, the name/namespace, and the raw `spec` we hand to the Pod
/// deserializer (the fields already match, via `serde(rename)` on `PodSpec`).
#[derive(serde::Deserialize)]
struct KubeDoc {
    #[serde(rename = "apiVersion", default)]
    api_version: String,
    #[serde(default)]
    kind: String,
    #[serde(default)]
    metadata: KubeMeta,
    #[serde(default)]
    spec: serde_yaml::Value,
}

#[derive(serde::Deserialize, Default)]
struct KubeMeta {
    #[serde(default)]
    name: String,
    #[serde(default)]
    namespace: Option<String>,
}

/// `delonix kube play <k8s.yaml>` — runs every Pod/Deployment the manifest
/// describes. A Deployment is run as a SINGLE pod from its `spec.template`
/// (replicas are a scheduler concern a single node does not own); other kinds
/// are skipped with a note rather than failing the whole file.
fn cmd_play(file: &std::path::Path) -> Result<()> {
    let text = std::fs::read_to_string(file).map_err(|e| {
        Error::Invalid(super::po::tf(
            "could not read {path}: {err}",
            &[
                ("path", &file.display().to_string()),
                ("err", &e.to_string()),
            ],
        ))
    })?;
    let mut ran = 0usize;
    for doc in serde_yaml::Deserializer::from_str(&text) {
        let doc = KubeDoc::deserialize(doc).map_err(|e| {
            Error::Invalid(super::po::tf(
                "invalid Kubernetes YAML in {path}: {err}",
                &[
                    ("path", &file.display().to_string()),
                    ("err", &e.to_string()),
                ],
            ))
        })?;
        // Recognised api groups (`v1` for Pod, `apps/v1` for the workload
        // kinds, the delonix envelope for a round-tripped manifest). An
        // unfamiliar one still tries — the kind drives the routing — but we say
        // so rather than failing silently on a typo'd apiVersion.
        if !doc.api_version.is_empty()
            && !matches!(doc.api_version.as_str(), "v1" | "apps/v1")
            && !doc.api_version.starts_with("compute.delonix.io/")
        {
            super::output::warn(&super::po::tf(
                "kube play: unfamiliar apiVersion '{av}' — routing by kind anyway",
                &[("av", &doc.api_version)],
            ));
        }
        // The pod spec, and where to read it from: a Pod carries it directly; a
        // Deployment wraps it under `spec.template.spec`.
        let (name, spec_value) = match doc.kind.as_str() {
            "Pod" => (doc.metadata.name.clone(), doc.spec.clone()),
            "Deployment" | "ReplicaSet" | "StatefulSet" | "DaemonSet" => {
                let tmpl = doc
                    .spec
                    .get("template")
                    .and_then(|t| t.get("spec"))
                    .cloned()
                    .ok_or_else(|| {
                        Error::Invalid(super::po::tf(
                            "{kind} '{name}' has no spec.template.spec to run",
                            &[("kind", &doc.kind), ("name", &doc.metadata.name)],
                        ))
                    })?;
                (doc.metadata.name.clone(), tmpl)
            }
            "" => continue, // empty document (trailing `---`)
            other => {
                super::output::warn(&super::po::tf(
                    "kube play: skipping unsupported kind '{kind}'",
                    &[("kind", other)],
                ));
                continue;
            }
        };
        if name.is_empty() {
            return Err(Error::Invalid(
                super::po::t("kube play: a workload has no metadata.name").into(),
            ));
        }
        let spec: PodSpec = serde_yaml::from_value(spec_value).map_err(|e| {
            Error::Invalid(super::po::tf(
                "pod '{name}': {err} (a Kubernetes field delonix does not support yet?)",
                &[("name", &name), ("err", &e.to_string())],
            ))
        })?;
        super::pod::create_pod(&name, doc.metadata.namespace.clone(), spec, false)?;
        println!("{}", super::po::tf("played pod {name}", &[("name", &name)]));
        ran += 1;
    }
    if ran == 0 {
        return Err(Error::Invalid(super::po::tf(
            "no Pod/Deployment found in {path}",
            &[("path", &file.display().to_string())],
        )));
    }
    Ok(())
}

fn cmd_generate(store: &Store, name: &str) -> Result<()> {
    // A pod is DERIVED from its member containers — the member carries the pod
    // name in the `delonix.io/pod` LABEL (its `.pod` field holds the netns name,
    // `pod-<name>`, not the pod name, which is why the old guards never matched a
    // real pod). Detect by that label: any member → generate the whole pod; else
    // a lone container.
    let members: Vec<Container> = store
        .list()?
        .into_iter()
        .filter(|c| c.labels.get(super::pod::POD_LABEL).map(String::as_str) == Some(name))
        .collect();
    let members = if members.is_empty() {
        vec![store.load(name).map_err(|_| {
            Error::Invalid(super::po::tf(
                "nothing to generate for '{name}' (no such pod or container)",
                &[("name", name)],
            ))
        })?]
    } else {
        members
    };
    print!("{}", pod_manifest(name, &members));
    Ok(())
}

/// Pure function: builds the `kind: Pod` YAML (testable without a Store).
fn pod_manifest(name: &str, members: &[Container]) -> String {
    let mut y = String::new();
    y.push_str("apiVersion: v1\n");
    y.push_str("kind: Pod\n");
    y.push_str("metadata:\n");
    y.push_str(&format!("  name: {name}\n"));
    y.push_str("  labels:\n");
    y.push_str(&format!("    app: {name}\n"));
    y.push_str("spec:\n");
    y.push_str("  containers:\n");
    for c in members {
        y.push_str(&format!("    - name: {}\n", c.name));
        y.push_str(&format!("      image: {}\n", c.image));
        if !c.command.is_empty() {
            y.push_str(&format!("      command: [{}]\n", quote(&c.command[0])));
            if c.command.len() > 1 {
                let args: Vec<String> = c.command[1..].iter().map(|a| quote(a)).collect();
                y.push_str(&format!("      args: [{}]\n", args.join(", ")));
            }
        }
        y.push_str("      resources:\n");
        y.push_str("        limits:\n");
        // cpus (cores) → millicores do k8s; memória tal e qual (ex.: `64M` → `64Mi` aprox).
        if let Ok(cpus) = c.cpus.parse::<f64>() {
            y.push_str(&format!("          cpu: \"{}m\"\n", (cpus * 1000.0) as i64));
        }
        if c.memory_max != "max" {
            y.push_str(&format!("          memory: \"{}i\"\n", c.memory_max));
        }
    }
    y
}

/// YAML double-quoted-scalar escaping.
///
/// BUG FOUND: this used to be a bare `format!("\"{s}\"")` with no escaping
/// at all. `command`/`args` come from the container's own record (whatever
/// the user or an image entrypoint originally set), and this manifest's
/// whole point is to be piped into `kubectl apply -f -`. A command
/// containing an embedded `"` (`sh -c 'echo "hi"'`) produced `["echo
/// "hi""]` — invalid YAML, `kubectl` rejects it outright. A command
/// containing a raw newline would be worse: it injects a literal line
/// break into the document, potentially adding sibling keys the emitter
/// never intended. Proper YAML double-quoted scalars escape `\`/`"` and
/// control characters — this covers the realistic ones without pulling in
/// a full YAML emitter for one call site.
fn quote(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            '\r' => out.push_str("\\r"),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctr(name: &str, image: &str, cmd: &[&str], cpus: &str, mem: &str) -> Container {
        let mut c = Container::new(
            "id".into(),
            name.into(),
            image.into(),
            cmd.iter().map(|s| s.to_string()).collect(),
            mem.into(),
        );
        c.cpus = cpus.into();
        c
    }

    #[test]
    fn gera_pod_com_recursos_e_args() {
        let c = ctr(
            "web",
            "nginx:1.27",
            &["nginx", "-g", "daemon off;"],
            "0.5",
            "256M",
        );
        let y = pod_manifest("web", &[c]);
        assert!(y.contains("kind: Pod"));
        assert!(y.contains("name: web"));
        assert!(y.contains("image: nginx:1.27"));
        assert!(y.contains("command: [\"nginx\"]"));
        assert!(y.contains("args: [\"-g\", \"daemon off;\"]"));
        assert!(y.contains("cpu: \"500m\""));
        assert!(y.contains("memory: \"256Mi\""));
    }

    #[test]
    fn memoria_max_nao_vira_limite() {
        // `memory_max = "max"` (sem teto) não deve gerar um `memory: "maxi"` inválido.
        let c = ctr("x", "alpine", &["sh"], "1", "max");
        let y = pod_manifest("x", &[c]);
        assert!(
            !y.contains("memory:"),
            "não devia haver limite de memória: {y}"
        );
    }

    #[test]
    fn quote_escapa_aspas_e_barra_invertida() {
        // BUG regression guard: `quote` used to be a bare `"{s}"` with no
        // escaping — `sh -c 'echo "hi"'` produced `["echo "hi""]`, invalid
        // YAML that `kubectl apply -f -` rejects outright.
        assert_eq!(quote(r#"echo "hi""#), r#""echo \"hi\"""#);
        assert_eq!(quote(r"C:\path"), r#""C:\\path""#);
    }

    #[test]
    fn quote_escapa_quebras_de_linha_em_vez_de_as_injectar() {
        // A raw embedded newline would inject a literal line break into the
        // emitted document — potentially adding sibling keys the emitter
        // never intended. Must come out as the two-character escape `\n`,
        // not an actual newline byte.
        let q = quote("line1\nline2");
        assert_eq!(q, r#""line1\nline2""#);
        assert!(
            !q.contains('\n'),
            "não pode conter uma quebra de linha real"
        );
    }

    #[test]
    fn gera_pod_com_comando_contendo_aspas_produz_yaml_valido() {
        let c = ctr("web", "alpine", &["sh", "-c", r#"echo "hi""#], "0.1", "64M");
        let y = pod_manifest("web", &[c]);
        assert!(y.contains(r#""echo \"hi\"""#), "{y}");
        // A quick sanity check that the offending pattern from the bug
        // report (unescaped adjacent quotes) is gone.
        assert!(!y.contains(r#""echo "hi"""#));
    }
}
