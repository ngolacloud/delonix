# 04 — Images: inspect, build, push/pull by digest, integrity

SUT: `delonix 5.1.1` @ `bca34f6c3`, rootless. Local registry: `registry:2` on `127.0.0.1:5000`.

| # | Case | Command | Independent verification | Observed | State |
|---|---|---|---|---|---|
| IMAGE-1 | inspect/digest | `image describe` / `image history` | — | ID `sha256:4ac41adb…`, 1 layer `sha256:bd5db2d0…`, config (Cmd/User/Env) | **PASSED** |
| IMAGE-2 | build | `build -t relc-img:v1 <dir>` (Dockerfile `FROM ubuntu:22.04` + RUN + CMD) | run the result | build OK (RUN+package), `run` prints `relc-layer-1792753040` | **PASSED** |
| IMAGE-3a | push to local registry | `image tag` + `push 127.0.0.1:5000/relc-img:v1` | HTTP to the registry | push OK; **delonix digest == registry `Docker-Content-Digest`** (`sha256:4056b726…`) | **PASSED** |
| IMAGE-3b | pull by digest + integrity | `pull …@sha256:4056b726…` after local `rmi` | run and compare content | recovers id `4ac41adbafec`; `run` prints **identical `relc-layer-1792753040`** | **PASSED** |

## Observations

- **End-to-end digest integrity confirmed:** the manifest digest that `push`
  reported matches exactly what the registry returns in
  `Docker-Content-Digest`, and a `pull` by that digest (after deleting the local copy)
  recovers byte-identical content (the baked `RUN` layer). This satisfies §7
  "push followed by pull must recover the expected artefact by digest".
- `http` registries on `127.0.0.1` are accepted without an extra flag (localhost treated as insecure by default) — reasonable.
- **Parity note (not a defect):** `delonix rmi` accepts **one** `<IMAGE>` (docker `rmi` is variadic). For batches: `image rm`/`image prune`. A surface difference, documented.
- Rich and coherent image surface: `describe/history/sbom/scan/verify/sign/export/save/load/tag/prune/apply`. Not tested this round: `scan`/`sbom`/`sign`/`verify` (covered under §16 security — deferred, see risks).

## Cleanup

`relc-registry` container removed; `relc-img` images (all forms, incl. the one pulled by digest) removed; **port 5000 freed**; `kaeso-*` preserved; `registry:2` stays cached (reusable, not an orphan).
