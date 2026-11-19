# Post-fix result + regression check

## Suites

| Run | Binary (base) | Result |
|---|---|---|
| Baseline | `delonix-cri` campaign (base **develop** `ee615f3b`) | `84 Passed \| 20 Failed \| 18 Skipped` |
| Post-fix (full) | fix (base **main** `ab4751ac` + fix) | `85 Passed \| 19 Failed \| 18 Skipped` |
| Focused `Image Manager` | fix | `10 Passed \| 0 Failed` |

The two fixed specs (**#18** digest, **#19** Uid/Username) drop out of the
failure list in every post-fix run. Confirmed by `grep` on the
*Summarizing Failures* block of `postfix.log`.

## The trap that cross-base nearly hid (and its resolution)

The `85/19` does **not compare directly** with the `84/20`: the baseline was
measured with a binary based on **develop** and the fix sits on top of **main**.
Between the two bases lies the whole main↔develop delta, not just my commit. The
symptom was a NEW spec failing in post-fix — **`Container OOM … exitCode 137
OOMKilled`** — which passed in the baseline.

My diff touches ONE file (`lib.rs`, image path + tests); OOM detection lives in
`lifecycle.rs`, untouched. Decisive test (A/B on the SAME `main` base):

```
focused "Container OOM"  @ main ab4751ac  SEM fix  → FAIL
focused "Container OOM"  @ main ab4751ac  COM fix  → FAIL
focused "Container OOM"  @ develop (binário baseline) → PASS
```

→ OOM fails on the **main** base regardless of my commit: it is a gap in `main`
(an OOM fix that exists in `develop` and has not yet propagated to `main`),
**not a regression of this change**. On the same base, the fix only improves
(image #18/#19 fail→pass) and regresses nothing.

## Regression conclusion

- **No regression attributable to this commit**: image-only diff + the single
  spec that "changed" (OOM) fails the same on the base without the fix.
- **Method note for the next session**: measure baseline and post-fix with
  binaries from the **same base**. The `84→85` stands as indicative, not as a
  direct measure; the valid measure is the A/B on the `main` base above and the
  focused `Image Manager` 10/0.
- **Uncovered follow-up**: `Container OOM` fails on `main` and passes on
  `develop` — confirm that the `develop` OOM fix reaches `main` (GitFlow
  propagation).
