# 02 — CLI, discovery and error handling

SUT: `delonix 5.1.1` @ `bca34f6c3` (digest `725112b6…`), `target/release/delonix`.

## Results

| Case | Command | Expected | Observed | State |
|---|---|---|---|---|
| CLI-1 | `--version` | exit 0, commit=HEAD | exit 0; `commit: bca34f6c3` | **PASSED** |
| CLI-1 | `--help`, `<grp> --help` | coherent help | complete help, direct+grouped verbs listed | **PASSED** |
| CLI-2 | invalid subcommand | exit≠0, suggestion | `exit=2`, "tip: a similar subcommand exists: 'init'" | **PASSED** |
| CLI-2 | invalid flag | exit≠0 | `exit=2`, "unexpected argument" | **PASSED** |
| CLI-2 | missing required arg (`run`) | exit≠0 | `exit=2`, "required arguments were not provided: <IMAGE>" | **PASSED** |
| CLI-2 | invalid port `99999:80` | `DX-` + exit, no effect | `error[DX-1307] invalid port`, exit=1, **0 containers created** | **PASSED** |
| CLI-2 | invalid names `../evil`,`/abs`,`"a b"`,`""` | `DX-` + no effect | all `error[DX-1000]`, exit=1, **0 leaks** | **PASSED** |
| CLI-3 | `ps -a -o json` | JSON with stable schema | valid JSON list; fields `id,image,command,created_unix,status,ports,name,restarts` | **PASSED** |
| CLI-3 | `-o` invalid value | refuses, lists values | exit=2, "possible values: table, json" | **PASSED** |
| CLI-4 | `ps -a` vs `container ls -a` | equivalent | **byte-identical output** (empty `diff`) | **PASSED** |
| CLI-4 | `images` vs `image ls` | equivalent | **byte-identical output** | **PASSED** |

## Observations / discrepancies

- **Consistent and correct exit-code convention:** usage errors (clap) → **exit 2**;
  domain errors → **exit 1** with a stable `DX-NNNN` code (`DX-1000` invalid name,
  `DX-1307` invalid port). Invalid inputs **leave no effect** (verified with `ps -a`).
- **Embedded security rationale:** names containing `.` are refused to prevent
  internal DNS hijack — the message explains why. Good practice.
- **DISCREPANCY (low, DOC/CONTRACT):** `docs/testing/plano-folhas.md §8` (case 8,
  "Output") lists `-o json`/`yaml`/table as contract, but `ps -o` only accepts
  `table|json` — `yaml` is refused. The refusal itself is correct (exit 2, lists the values),
  but the written contract promises `yaml`. To classify: either the plan over-promises, or
  `yaml` is missing from the output surface. **Record for verification** (non-blocking).

## Requirement CLI-5 (Docker/Podman side-by-side) — **BLOCKED**

`docker` and `podman` are not installed (delonix is this host's runtime). Live
comparison not runnable. The repo has a prior measured comparison in
`docs/comparacao-medida.md` / `docs/paridade-docker-podman.md` (not re-measured here).
