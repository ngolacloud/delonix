# A1 — OCI Image + Distribution (Track A) · interop between engines

**Date:** 2026-10-24 · **Binary:** `delonix 5.1.1 @ ee615f3b` (with GAP-RT-1 fix) ·
docker 29.9.2 · podman 4.9.3 · registry `registry:2` local at `127.0.0.1:5000`.

> The `oci-image-tool` tool (`opencontainers/image-tools`) **does not compile** (the classic
> `Sirupsen/logrus` vs `sirupsen/logrus` breakage; abandoned project). Instead of a dead validator, the
> real test of "same as Docker/Podman as an OCI image" is the **interop between engines**: push with one,
> pull/run with the others. Stronger than a schema check and with no collision with CRI.

## Interop matrix (push → pull/run)

| Source → destination | Result | Evidence |
|---|---|---|
| **delonix → docker** | **PASSED** | identical digest `sha256:3cc8f85a…`; `docker run` → `RELC-OCI-7f3a` |
| **delonix → podman** | **FAILED** | `unsupported docker v2s2 media type: "application/vnd.oci.image.layer.v1.tar"` |
| **docker → delonix** | **PASSED** | `delonix pull` + `run` of the image pushed by docker → `delonix-ran-docker-image-OK` |

## GAP-OCI-IMG-1 — non-conformant hybrid manifest on push

The manifest that `delonix push` produces mixes schemas:

```
manifest mediaType: application/vnd.docker.distribution.manifest.v2+json   (Docker schema-2)
config  mediaType: application/vnd.docker.container.image.v1+json          (Docker)
layer   mediaType: application/vnd.oci.image.layer.v1.tar                  (OCI, NOT compressed)
```

A **Docker schema-2** manifest requires **Docker layers** (`application/vnd.docker.image.rootfs.diff.tar.gzip`).
Delonix puts an **OCI layer inside a Docker manifest** — `docker` (tolerant) accepts it, while
`podman`/`containers-image` (strict, correct) rejects it. This ties in with the known gap of "uncompressed
build layers" (`.tar`, not `.tar+gzip`).

**Impact:** images pushed by Delonix **cannot be pulled by podman/skopeo/containers-image** —
breaks interop with half the ecosystem. (Delonix's `pull` is fine: it consumes standard Docker manifests
without trouble — see the docker→delonix row.)

**Fix (two clean options):**
- **(a) Emit an OCI manifest** `application/vnd.oci.image.manifest.v1+json` + config
  `application/vnd.oci.image.config.v1+json` + OCI layers — consistent with the `.tar` layer already in use; it is the
  modern, correct target. **Recommended.**
- **(b) Emit a real Docker v2s2 manifest** with `…diff.tar.gzip` layers (implies compressing the
  layers — also closes the compression gap).

### Fix APPLIED (via a), NOT yet verified

`crates/adapters/delonix-oci/src/registry.rs` `push_to_registry` (~2334): the push went from
`docker_manifest(...)` + `DOCKER_MANIFEST_MEDIA_TYPE` to
`to_oci_manifest(&docker_manifest(...))` + `MediaType::ImageManifest.as_ref()` — reusing the
OCI function **that already existed** (used by the internal registry and by Proxmox). The config/layer blobs
are the same (image id unchanged; only the manifest digest changes). Same pattern as the other two
OCI pushes in the file (lines 2490, 2578), hence high confidence that it compiles.

**Status:** applied, **to be compiled/tested** — deferred because the disk is at **9.8G (99%)** with the
owner's CRI session consuming it; a cold build (I removed `target/debug`) would fill `/`.

**Verify when disk is available:**
```bash
cargo build --release -p delonix-runtime-bin          # incremental
delonix run -d --name r -p 5000:5000 registry:2
delonix build -t x:1 . && delonix tag x:1 127.0.0.1:5000/x:1 && delonix push 127.0.0.1:5000/x:1
podman pull --tls-verify=false 127.0.0.1:5000/x:1     # DEVE passar agora (era o que falhava)
docker pull 127.0.0.1:5000/x:1                        # continua a passar
# + inspeccionar o manifesto: mediaType=application/vnd.oci.image.manifest.v1+json
```

## Distribution-spec (scope note)

The `distribution-spec` conformance suite tests a **registry** (server). Delonix is a **client**
(pull/push) — its `internal_registry.rs` is scaffolding without a CLI. So server-side conformance is
**N/A** for Delonix-as-client; what matters (and was measured) is the client interop above.

## A1 verdict

- **Delonix pull: conformant** (consumes Docker and OCI).
- **Delonix push: NOT conformant** (GAP-OCI-IMG-1) — it is the only real blocker for image interop
  with podman, and it has a clear fix (emit an OCI manifest). It becomes the next Track A task.
- RT-1 (ociVersion 1.3.0) already closes the runtime bundle validation (`oci-runtime-tool` → success).
