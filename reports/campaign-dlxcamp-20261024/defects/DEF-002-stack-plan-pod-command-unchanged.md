# DEF-002 — `stack plan`/`apply` reports "unchanged" when changing the `command` of an existing Pod (silently incomplete convergence)

- **Requirement/test:** MANIFEST-01 (§9 — "change a mutable field", "change that requires replacement", "detect and reconcile")
- **Severity:** P2 (correctness / operational safety — silent divergence between desired and observed)
- **Environment:** delonix v5.1.1 binary `350a6320…` (develop `558e5f20`), rootless host uid 1000

## Expected
The "cloud native — declarative and convergent" principle and the ADRs establish that a change requiring recreation either (a) **refuses** the apply without `--replace` (ADR-0059:856 "`stack apply` refuses without `--replace`, as every change to this Kind does") or (b) is **named** in a `FieldsNotCompared` condition (ADR-0055, VM). In either case, the `plan` must **reveal the divergence**.

## Observed
Changing `spec.containers[0].command` from `["sleep","600"]` to `["sleep","500"]` on an **already applied** Pod:
- `stack plan -f v2.yaml` → `= Pod/dlxcamp-pod` / **`Summary: 1 unchanged`** (does not detect the change)
- `stack apply -f v2.yaml` → `pod/dlxcamp-pod: already exists (1 container(s)), nothing to do` (does not converge)
- the real process keeps running `sleep 600`; `container describe` → `Command: sleep 600`
- No error, no `--replace` request, no `FieldsNotCompared` condition.

`apply --replace Pod/dlxcamp-pod` converges correctly (`describe` → `Command: sleep 500`) — but it requires the operator to know in advance what changed, precisely what the `plan` should be telling them.

## Minimal reproduction steps
```
cat > m.yaml <<'Y'
apiVersion: compute.delonix.io/v1alpha1
kind: Pod
metadata: {name: p, namespace: ns}
spec: {containers: [{name: app, image: busybox:latest, command: ["sleep","600"]}]}
Y
delonix stack apply -f m.yaml
sed 's/"600"/"500"/' m.yaml > m2.yaml
delonix stack plan -f m2.yaml      # => "1 unchanged"  (EXPECTED: change flagged or replace required)
delonix stack apply -f m2.yaml     # => "nothing to do" (EXPECTED: converges or refuses)
delonix container describe p-app | grep Command   # => sleep 600 (did not converge)
```

## Impact
An operator who edits the manifest and runs `apply` receives confirmation of convergence ("unchanged"/"nothing to do") while the machine still runs the old spec. Silent divergence between desired and observed — the opposite of the declarative guarantee. Affects any Pod field that requires recreation.

## Suspected cause
The Pod's convergence path compares identity/existence (ensure-present) and not the content of the `command` spec, without declaring it as `FieldsNotCompared` (as the VM does in ADR-0055) nor refusing without `--replace` (as the ADR-0059 Kinds do). Inconsistency between Kinds in the `stack plan` comparator.

## Closure criterion
`stack plan` of a Pod whose `command` (or another field requiring recreation) changed reports the divergence — as `~`/replace-needed or by naming the field in `FieldsNotCompared` — instead of `unchanged`; and `stack apply` converges or refuses explicitly, requesting `--replace`.

## Note (non-defects ruled out in this area, for rigour)
- `stack validate --strict` rejects an unknown field with **exit 1** (verified in isolation; the apparent "exit 0" was an artefact of `| tail`).
- Stopping the container is NOT drift: the engine is daemonless (no continuous reconciler, principle 2); `drift` compares declared fields, not running/stopped state — consistent behaviour, not a defect.
