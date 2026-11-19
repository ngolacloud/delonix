# Resource structure

What can be written in a manifest, with which `apiVersion`, and what happens to
each document once loaded.

This page is not written by hand from a list: it comes from the same registry the
parser, the schema, completion and the reconciler read. To see it on your own
engine, instead of here:

```bash
delonix api-resources
delonix api-resources -o json
```

## The groups

Every Kind lives in a group, and the group is part of its identity — an
`apiVersion: storage.delonix.io/v1alpha1` on a `kind: Pod` is refused, with both
accepted forms named in the error.

| group | Kinds |
|---|---|
| `core.delonix.io/v1alpha1` | `Secret`, `Stack` |
| `compute.delonix.io/v1alpha1` | `Pod`, `VirtualMachine`, `Container`, `Workload` |
| `networking.delonix.io/v1alpha1` | `Network`, `NetworkRoute`, `NetworkPolicy`, `Dependency` |
| `gateway.delonix.io/v1alpha1` | `Gateway`, `HTTPRoute`, `Ingress` |
| `storage.delonix.io/v1alpha1` | `Volume` |
| `artifact.delonix.io/v1alpha1` | `Image` |
| `infrastructure.delonix.io/v1alpha1` | `KubernetesCluster` |

## `apiVersion: delonix.io/v1` still loads

It is not a legacy spelling on its way out the door. The [stability
promise](estabilidade.html) says `delonix.io/v1` only changes with a `v2` that
still accepts it, and that promise holds: **a manifest written before this
reorganisation loads without a single change**.

The clean cut applies to **commands**. A file that is in git, reviewed in a PR and
pointed at by `$schema` in an editor, gains a step — not an error.

## What happens to each document

The `FORM` column of `api-resources` is the one you cannot guess: it says whether
a document of that Kind **survives loading under its own name**. That is the
answer to «why does my `kind: Dependency` never show up in the plan under that
name».

| form | meaning |
|---|---|
| `primary` | has its own apply and survives loading |
| `sugar → X` | is rewritten as `X` for convenience |
| `compat → X` | a foreign schema accepted as-is, compiled over `X`'s mechanism |
| `aggregate` | expands into the documents it contains |
| `sunset → X` | **works and is not rewritten** — but `X` is the way forward |

### `sunset` is different from `deprecated`, and the difference is the point

A `deprecated` Kind is **rewritten** on load, and whoever wrote it gets the
successor's behaviour for free. A `sunset` Kind is **not** rewritten, because
rewriting it would change what the engine DOES.

`kind: Container` is the case that forced the distinction. Lowering it to a
`kind: Pod` with one container looks like a rename and is not: a Pod always
builds a shared netns and its members join it by re-exec, so every declared
container would gain an extra netns holder and a different network path. The
name half was solvable; the netns half is not.

So it is **announced, not rewritten**: it keeps working, with a warning on every
load saying `kind: Pod` is the way forward. A future major removes it, once
manifests have migrated.

## Removed Kinds

Three Kinds **stopped existing**. The refusal names what to write instead, rather
than saying «unknown Kind» — which would make a manifest that was correct until
yesterday look like a typo:

| removed | write |
|---|---|
| `Storage` | `kind: Volume` with an `nfs:`/`cifs:`/`webdav:` block |
| `ShareVolume` | `kind: Volume` with a `share:` block |
| `Egress` | `kind: NetworkPolicy` with `direction: egress` |

All three were **rewritten** on load into exactly these forms, so what the engine
does did not change — what changed is who has to write the final form. See the
release notes of the version that removed them for migration.

## Old names that still resolve

Four Kinds were renamed. The old name is a **silent alias** — there is no warning,
because a rename does not change what the document means and there is nothing to
migrate:

| before | now |
|---|---|
| `Vm` | `VirtualMachine` |
| `FirewallPolicy` | `NetworkPolicy` |
| `Tunnel` | `Gateway` |
| `Cluster` | `KubernetesCluster` |

The alias works everywhere, not only in the loader: `delonix explain Cluster`
resolves just as `delonix explain KubernetesCluster` does.

## Plurals and abbreviations

`get`, `describe` and `explain` accept four spellings of each Kind — the
canonical name, the lowercase singular, the plural and the declared
abbreviation:

```bash
delonix explain Pod
delonix explain pod
delonix explain pods
delonix explain po
```

Abbreviations are deliberately few — one exists when it is unambiguous and worth
writing. `delonix api-resources` lists the ones there are.

## What does not exist yet

`kind: Service` is planned and **not yet implemented**. Today port publishing is
done via `container run`'s `-p` and `delonix net ingress publish`; the
declarative form comes in a later version.
