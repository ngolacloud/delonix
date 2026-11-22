# Landing the two patches (when there is a slot in the WIP cap)

Both prepared in *detached* worktrees (they do not count towards the WIP cap),
compiled and verified live. The `gitflow start` needs ONE free slot per patch.

## 1. fix/vm-ssh-hint-rootless  (base: main)

```sh
cd <repo>
python3 scripts/gitflow.py start fix/vm-ssh-hint-rootless      # creates worktree off main
cd ../.worktrees/<...>/vm-ssh-hint-rootless
git apply /path/to/reports/vm-native-ch-audit-2026-10-24/fix-vm-ssh-hint.patch
cargo build -p delonix-runtime-bin                              # green (verified)
# commit (Conventional), push, PR -> develop/main via GitFlow propagation
```
Verified: `cargo build` rc=0; `vm create` prints
`delonix vm ssh <name>` as the primary step and `ssh <user>@<ip>` as a labelled
fallback.

## 2. feat/vm-capability-graduation  (base: develop)

```sh
python3 scripts/gitflow.py start feat/vm-capability-graduation  # creates worktree off develop
cd ../.worktrees/<...>/vm-capability-graduation
git apply /path/to/reports/vm-native-ch-audit-2026-10-24/feat-vm-capability-graduation.patch
# the patch includes the new file scripts/vm-native-ch-lifecycle.sh
chmod +x scripts/vm-native-ch-lifecycle.sh
cargo test -p delonix-runtime-bin cmd::provider::tests::     # 4/4 green (verified)
```
Verified: both gate tests pass; the VMM-level checks run live 14/14; CH discovery
becomes **12 supported / 6 partial** in vm-compute.

## Detached worktrees (can be removed after applying the patches)
```sh
git worktree remove --force /tmp/wt-vm-ssh-hint
git worktree remove --force /tmp/wt-vm-cap-grad
```
