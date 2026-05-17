# Claude Execution Plan

## Current Invocation

Goal: complete exactly the first incomplete task in `TODO.md`, then stop after validation and a git commit.

Selected task: `C3-T02` (`在 sysroot/compiler 添加 Atomic 一族`).

Task source of truth: `TODO.md` lines for `C3-T02`; `PLAN.md` is only consulted for phase-level context.

## Execution Plan

1. Confirm the latest commit only for unfinished work directly relevant to `C3-T02`.
2. Inspect the existing atomic-int implementation in `sysroot/scoop.unsafe`, direct HIR LLVM lowering, effect-lowered LLVM lowering, and GC write barrier helpers.
3. Inspect sysroot core conventions for ordinary classes and methods, including existing generic-bound syntax using `AnyRef` / `AnyValue`.
4. Design the smallest spec-correct surface for `AtomicInt`, `AtomicBool`, `Atomic<T: AnyRef>`, and `AtomicValue<T: AnyValue>` in sysroot, with unsafe atomic-ref intrinsics kept internal.
5. Implement atomic-ref primitive support in the compiler for direct HIR LLVM and effect-lowered LLVM paths, ensuring ref store and successful CAS obey the GC barrier protocol.
6. Implement sysroot public APIs on top of existing atomic-int intrinsics and the new internal atomic-ref intrinsics.
7. Add focused compiler tests for atomic-ref IR generation and GC barrier behavior, plus fixture coverage for public API usage and marker-bound rejection.
8. Run focused validation first, then task-required validation (`cargo build`, targeted LLVM tests, and `cargo run -p scoop -- test` with a long timeout). Fix in-scope failures.
9. Mark `C3-T02` as `[DONE]` in `TODO.md` and fill its completion record with scope, decisions, validation, and plan/design closure.
10. Commit all relevant changes with a clear `[C3-T02]` message.
11. Stop without starting `C4-T01A`.

## Guardrails

- Do not weaken the API shape: `AtomicValue<T>::cas` must take `expected: Box<T>`.
- Do not expose unsafe atomic-ref intrinsics as user-facing API.
- Do not bypass the GC write barrier for atomic ref store/CAS success paths.
- Do not add compatibility shims for old behavior.
- If a missing prerequisite prevents spec-correct implementation, add the minimum prerequisite task before `C3-T02`, keep `C3-T02` incomplete, commit that bookkeeping change, and stop.

## Progress Log

- Read `TODO.md`; first incomplete heading is `C3-T02`.
- Wrote the current invocation plan before making implementation changes.
- Latest commit is `[C3-T01] Add RefCell and Box sysroot types`; it is the direct dependency for `C3-T02` and does not mention an unfinished blocker.
- Working tree initially contains only this plan-file update.
- Inspected existing `__AtomicInt` sysroot declarations, direct LLVM lowering, effect-lowered LLVM lowering, and GC write barrier ABI.
- Implementation direction: keep public `Atomic*` classes in `scoop.core`, add internal generic `__atomicRef*` declarations in `scoop.unsafe`, lower atomic-ref pointer operations in both LLVM paths, and call the GC write barrier with a null slot on ref writes so the barrier handles promotion/polling without performing a non-atomic slot write.
- Implemented the first compiler/sysroot pass: added `__atomicRef*` declarations, public `AtomicInt` / `AtomicBool` / `Atomic<T>` / `AtomicValue<T>` APIs, direct LLVM atomic-ref lowering, effect-lowered atomic-ref lowering, and a null-slot GC barrier helper.
- `cargo fmt` and `cargo build -p scoopc` pass after the first implementation pass.
- Public `AtomicValue<T>` exposed a relevant generic-class initializer gap: concrete generic `ClassInit` entries substituted field and ctor parameter types but not property initializer / ctor body expressions. Fixed this class-wide by recursively substituting HIR types in generic class init steps, ctor defaults, delegations, and bodies.
- Added targeted coverage: LLVM atomic-ref IR/barrier unit test, run-pass fixture for all four public Atomic APIs, and typecheck reject fixtures for `Atomic<Int>` / `AtomicValue<Class>` bound failures. The targeted fixtures and atomic-ref unit test pass.
- Generic/LLVM targeted runs found old sysroot overlay fixtures missing `AnyRef` / `AnyValue`; updated those overlay core files so the new internal atomic-ref unsafe declarations can resolve their bounds under overlay sysroots. The overlay intrinsic LLVM tests now pass.
- Re-ran `cargo test -p scoopc generic -- --nocapture`, `cargo test -p scoopc llvm -- --nocapture`, and the updated `sysroot_overlay_core_array_interface_bridge` fixture; all pass after the overlay update.
- Full fixture run exposed `AtomicValue` internals conflicting with user-defined `Box` names. Fixed this by supporting qualified nominal constructor calls (`scoop.core.Box(...)`) in typecheck, HIR call-site contracts, and direct HIR codegen, then using qualified `scoop.core.Box` inside `AtomicValue`.
- Re-ran the atomic fixture plus representative generic `Box` conflict fixtures; they pass after qualified constructor support.
- Full fixture suite now has only the three pre-tracked `C4-T01A` MIR snapshot failures (`mir/closure_capture_var`, `mir_lowered/aggregate_transport`, `mir_lowered/assignment_places`); all C3-T02 fixtures and previously exposed generic `Box` conflict fixtures pass.
- Final validation so far: `cargo build` passed; `cargo clippy --all-targets -- -D warnings` passed; `cargo test --all --all-targets` has one failure in `fixtures::tests::run_all_recreates_session_between_independent_fixtures`, caused by the same pre-tracked `C4-T01A` `mir_lowered/aggregate_transport` snapshot mismatch.
- Marked `C3-T02` as `[DONE]` in `TODO.md` and filled its completion record. `PLAN.md` was not changed because phase-level sequencing and acceptance criteria did not change.
