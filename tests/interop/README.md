# `tests/interop/` — CNCF-ecosystem interoperability evidence

This is the area the compatibility campaign (mission H) records its proofs in. It is
deliberately **outside `crates/`, `bins/` and `proto/`**: `scripts/arch_fitness.py`
forbids a product's name in the engine's code and contract (the engine knows no
consumer), but interop tests are *about* named products — so they live here, where the
boundary gate does not reach. Confirmed against `arch_fitness.py` before this area was
created.

## What a record is

Every claim of the form "Delonix works with X" is one JSON line (a record) under
`results/<phase>.jsonl`, validated by `schema.json`. The record carries the five
dimensions the campaign requires — **tool, version, mode, result, proof** — plus the
host capabilities a tool needs. `scripts/gen_compatibility.py` reads every record and
renders `docs/compatibility.md`.

### The doctrine the schema enforces

- A **`PASS`** record (a "compatible" claim) **must** carry the exact `tool_version` it
  was proven against and an `artifact_ref` that proves it. A compatible claim without a
  version and a proof is rejected by the generator.
- `PARTIAL` / `FAIL` records must carry an `artifact_ref` (there is a measurement).
- Where there is no proof yet the state is **`NOT_TESTED`** — never "compatible".

### The two modes, always said

- **Mode 1** — Delonix *under* a real Kubernetes, via the CRI.
- **Mode 2** — Delonix *without* Kubernetes, open interfaces only.

## Layout

```
tests/interop/
  README.md            this file
  schema.json          JSON Schema for one evidence record
  LIMITS.md            the contract's limits (embedded verbatim into the generated doc)
  results/
    h0-spiffe.jsonl    one file per phase; each line is one record
    h1-kubernetes.jsonl
    ...
```

## Adding or updating a claim

1. Run the measurement; keep the console/report under `reports/` (the `artifact_ref`).
2. Add or edit the record line in the matching `results/<phase>.jsonl`.
3. `python3 scripts/gen_compatibility.py` — regenerates `docs/compatibility.md`.
4. Commit both. CI (`compatibility` check) regenerates and fails on any drift, so the
   published matrix is always the one the records produce — never hand-edited.

## Validate locally

```bash
python3 scripts/gen_compatibility.py --check   # validate records + fail on doc drift
```
