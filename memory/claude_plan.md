# Execution Plan

## Current Invocation

- This is a public execution plan and progress log; it does not include hidden reasoning.
- Read `TODO.md` and identified the first incomplete task: `P7-B3.3` for the B-26 atomic intrinsic contract.
- Check the latest commit only for directly relevant unfinished notes after the current task is known.
- Inspect the B-26 audit strategy/category docs, active inventory rows, fixture directory, and relevant atomic intrinsic lowering/typecheck/sysroot code.
- Implement the smallest spec-correct change needed to close B-26 without workarounds: user-facing misuse should be rejected by frontend/typecheck, while invalid sysroot/internal shapes should become internal invariants rather than `UnsupportedMainBody` diagnostics.
- Retire B-26 rows by synchronizing production code, `audit/UMB_inventory.csv`, `audit/UMB_retired.csv`, B-26 docs, fixture coverage, stale-count baselines, and the task completion record.
- Run focused validation first, then the task-required audit and fixture validation; also run formatting and clippy before committing.
- Update this file when key milestones complete or if the plan changes.
- Commit all intended changes for this task and stop without starting the next task.

## P7-B3.3 Plan

- Use `umb-audit list --bucket B-26` to lock the 102 active IDs and source locations.
- Review `audit/strategies/B-26.md`, `audit/UMB_categories/B-26.md`, `tests/fixtures/umb_fix/B-26-atomic-intrinsics/`, and existing atomic intrinsic code paths.
- Identify current upstream contracts for `atomicInt` and `atomicRef` receiver mutability, width/type, ordering, arity, and return values.
- Add or tighten verifier/typecheck/sysroot contract checks only where missing; otherwise replace B-26 `UnsupportedMainBody` fallbacks with explicit internal invariant helpers/panics.
- Keep unrelated active buckets in place and avoid broad historical triage outside B-26 unless it blocks the task.
- Update inventory/ledger/category/strategy/spec coverage/fixture index/stale count so active goes from 515 to 413 and retired from 769 to 871 if all B-26 rows are retired.
- Validate with `cargo run -p scoopc --bin umb-audit -- list --bucket B-26`, `cargo run -p scoopc --bin umb-audit -- diff`, `cargo run -p scoopc --bin umb-audit -- stats`, `cargo test -p scoopc audit:: -- --nocapture`, `cargo run -p scoop -- test tests/fixtures/umb_fix/B-26-atomic-intrinsics/`, and `cargo clippy --all-targets -- -D warnings`.

## Progress

- Plan initialized for `P7-B3.3` before running repository commands.
- Latest commit is `[P7-B3.2] Retire thread sync UMB rows`; no directly relevant unfinished note was found in the recent commit subject list.
- Working tree initially contained only this plan file modification from the current invocation.
- Locked B-26 scope with `umb-audit list --bucket B-26`: 102 active rows across `effect_lowered/value.rs`, `intrinsics/atomic.rs`, and `main/call.rs`.
- Replaced B-26 LLVM atomic fallback sites with verified intrinsic/internal invariant checks, and added a typecheck gate for raw atomic first-argument addressability/writability where typecheck has the needed facts.
- Synchronized B-26 audit data and fixtures: active 515 -> 413, retired 769 -> 871, B-26 active count is 0, B-26 fixtures are active with retired-ledger coverage.
- Fixed a directly blocking generic HIR assignment verifier issue exposed by `Atomic<T>.exchange`: generic clones may have different local ids but the same source decl span/name.
- Final validation completed: B-26 list/diff/stats, audit tests, pipeline policy tests, B-26 fixtures, focused atomic run/typecheck fixtures, `cargo fmt`, and `cargo clippy --all-targets -- -D warnings` all passed.
- Updated `TODO.md` with `[DONE] P7-B3.3` and the completion record.
