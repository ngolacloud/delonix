# DEF-002 — `rm` does not accept `--namespace`: a container outside `default` is only removable by ID

- **Requirement / test:** SCOPE-2 / §8 (namespace isolation), §5 (cleanup).
- **Severity:** **P3 (low)** — usability/surface asymmetry; no data loss and no orphans (removal by ID works and cleans everything up).
- **Type:** product defect (CLI surface gap).
- **Environment:** `delonix 5.1.1` @ `bca34f6c3`, rootless.

## Minimal steps

```
delonix run -d --name dup --namespace teamA --net n nginx…
delonix run -d --name dup --namespace teamB --net n nginx…
delonix rm -f dup --namespace teamA     # error: usage (flag does not exist)
delonix rm -f dup -n teamA              # error: usage
```

## Expected vs observed

- **Expected:** symmetry with `ls`/`vm ls`, which accept `--namespace` (`delonix vm ls --namespace teamA` is a documented example in `manual_entries.rs:2891`). To be able to address `rm <name> --namespace <ns>`.
- **Observed:** `rm`/`container rm` **do not have** `--namespace`. With the same name in two namespaces, the name is ambiguous and only the **global ID** removes the resource (`rm -f <id>` works and cleans up).

## Impact

An operator following the namespace mental model (create with `--namespace`, list with `--namespace`) cannot remove by name+namespace; they have to discover the ID. Friction, not a data failure.

## Closure criterion

Add `--namespace` to `container rm` (and mirror it in `stop`/`restart` if applicable), or explicitly document that removal outside `default` is by ID. Re-run the minimal steps.

## Status

FIXED (local, working tree, verified live on 2026-10-23). See the «Fix» block below.

## Fix

- `bins/delonix-runtime-bin/src/cmd/container.rs`: `RmArgs` gains `-n/--namespace` (symmetric with `ps`);
  a new pure helper `resolve_rm_query(store, token, ns)` qualifies a plain name as `<ns>/<name>`
  (the form `find` already understands), with fallback to the raw token (ids/prefixes keep working).
- Tests: `rm_namespace_tests::namespace_disambiguates_a_shared_name` and
  `namespace_leaves_ids_and_qualified_forms_alone`. **PASS.**
- Live: `rm --namespace teamA relcfix-dup` and `rm -n teamB relcfix-dup` remove by name the right
  container in each namespace (2→1→0), without needing the ID.

**Commit:** `5ce63079` (branch `fix/campaign-defects-doctor-rm-compose`, not pushed).
