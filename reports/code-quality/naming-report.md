# Naming report — Delonix Runtime (`cebf895`)

## The misleading number

The `scripts/lang_ratchet.py` gate counts **1,039 identifiers** still in
Portuguese and is aligned with the baseline. That reads as "resolved" and
it is not. But the distribution, once production is separated from test, flips
the priority entirely:

| Class | Count | Severity |
|---|---|---|
| Test names (`#[cfg(test)]`, `tests/`) | **985** | LOW |
| Identifiers in production code | **54** | see below |

Method: for each occurrence, braces were counted from each `#[cfg(test)]` to
delimit the test module, and we checked whether the 4 preceding lines carry
`#[test]`/`#[tokio::test]`. Script in `scratchpad/classify.py`.

## NAMING-0001 — Zero PUBLIC identifiers in Portuguese — **PASSES**

Of the 54 in production, **a single one** is `pub`: `pub fn cap_num` — and it is
a false positive (`num` is the English abbreviation of *number*, not Portuguese).

Practical consequence: **Golden Rule §1 is not violated in any public contract.**
No external consumer — CLI, API, SDK, manifest — sees a Portuguese name. This is
the difference between cosmetic debt and compatibility debt, and here it is
cosmetic.

## NAMING-0002 — Three privately NAMED items in Portuguese — LOW

| File | Item | Proposal |
|---|---|---|
| `crates/delonix-net/src/infra.rs:4457` | `fn netdef_path_legado` | `fn legacy_netdef_path` |
| `crates/delonix-runtime/src/lib.rs:4086` | `static AVISO` | `static WARNED_ONCE` |
| `crates/delonix-runtime-bin/src/cmd/dockerapi.rs:1188` | `const CONSUMIDOS_TOPO` | `const CONSUMED_TOP_LEVEL` |

All private. Safe change (§97): no impact on serialization, config, CLI or API.
Lexicon false positives NOT to touch: `fn todo<T>` (English *to-do*),
`fn topo_sort` (topological), `enum StringOrNum`, `pub fn cap_num`.

## NAMING-0003 — ~44 local bindings in Portuguese — LOW

`let alvos`, `let regra`, `let trinco`, `let legado`, `let erros`, `let espera`,
`let estado`, `let pedido`, `let nomes`, `let vivos`, `let orfaos`, …
Concentrated in `delonix-net/src/infra.rs` (12) and
`delonix-runtime-bin/src/cmd/kindmode.rs` (8). Auto-fixable under §97;
full list in `scratchpad/ids_prod.txt`.

## NAMING-0004 — 985 test names in Portuguese — LOW, but it is the real debt

Examples:

```
crates/delonix-cri/src/cap_ceiling.rs:288: valor_invalido_e_erro_nunca_tecto_vazio
crates/delonix-cri/src/runtime_svc/lifecycle.rs:1958: ceiling_reduces_so_avisa_quando_um_pedido_explicito_foi_cortado
```

Against §59: the names **describe behaviour** and are good test names —
they fail only on language. That is why they rate LOW and not MEDIUM: the damage
is to consistency, not to comprehension. It is also 95 % of all identifier debt,
and it is what makes the number 1,039 look alarming.

## NAMING-0005 — Rust conventions (§6) — **PASSES**

`cargo clippy --workspace --all-targets` run now: **zero warnings**.
The `non_snake_case`, `non_camel_case_types` and `non_upper_case_globals` lints
are on by default, so §6 is proven by execution, not by inspection.

## NAMING-0006 — `cmd/util.rs` — LOW (§46, §47)

A single vaguely named module across 147 files. It is not growing out of control.
Recommendation: classify its contents and rehome it when someone touches it; it
does not justify a dedicated change.

## NAMING-0007 — Provider name inside the logic — MEDIUM (§43)

```
crates/delonix-runtime-bin/src/cmd/conditions.rs:319: if backend == "libvirt" {
crates/delonix-vm/src/lib.rs:3981: net_mode: (vm.backend == "libvirt").then(|| vm.tap.clone())
```

The `VmBackend` port exists (see `architecture-report.md`), but these two places
branch on the **provider name string** instead of on a port capability.
They are exactly the leak that `ngolacloud-arch` records as ARCH-002, seen from
the engine side.
