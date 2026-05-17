# Current Invocation Plan

Note: this file records the execution plan, observable rationale, decisions, and progress updates for this invocation. It does not include private chain-of-thought.

## Initial Plan

1. Read `TODO.md` first and identify the first task whose heading is not prefixed with `[DONE]`.
2. Check the latest commit message only after identifying that task, and only treat it as relevant if it explicitly mentions unfinished work directly tied to the selected task.
3. Read the selected task details, including dependencies, validation requirements, and completion-record expectations.
4. Inspect the smallest relevant parts of the codebase needed to implement the selected task.
5. Implement the task without narrowing scope or using workarounds.
6. If a blocking missing feature or spec mismatch prevents correct implementation, update `TODO.md` with the minimum prerequisite task, commit that bookkeeping, and stop.
7. Run the task-specified validation plus relevant focused tests; fix failures caused by this work.
8. Update `TODO.md` by prefixing the completed task heading with `[DONE]` and refreshing its completion record.
9. Update this file after key milestones or any plan change.
10. Commit all relevant changes with a task-scoped commit message, then stop without starting the next task.

## Progress

- Started invocation and recorded the initial execution plan before running project commands.
- Identified the first incomplete task as `P7-T01` from `TODO.md` / `TODO-3.md`.
- Checked the latest commit: `[P6-T02] Remove LLVM f-string fallback`; it is the completed dependency and does not add a new prerequisite for `P7-T01`.

## Selected Task

- `P7-T01`: convert sysroot runtime-symbol wrappers such as scalar `toString`, `String` concat/unsafe slice, `print`/`println`, `panic`, and `__scoop_gc_collect` from audited intrinsic-style wrappers to `@Extern(abi = "scoop")` declarations where appropriate.
- Keep true intrinsics such as `String.byteLength` and `String.getByte`, and do not delete `sysroot/scalar_string_bridge.scoop` or compiler audited bridge dispatch in this task; those are explicitly assigned to `P7-T02`.

## Implementation Plan For P7-T01

1. Add scoop ABI runtime declarations in `sysroot/core.scoop` for scalar `toString`, print/println, panic, GC collect, and the string unsafe slice runtime entry.
2. Update `Bool` / `Char` / `Int` / `Float32` / `Float64` `toString()` bodies to call the new declarations directly.
3. Add an `@Unsafe` `String.unsafeSliceBytes(...)` body that calls the scoop ABI runtime declaration; keep `byteLength` and `getByte` compiler-owned.
4. Update `sysroot/string.scoop` concat bridge from `@Intrinsic("string_concat_bridge")` to `@Extern(name = "scoop_string_concat", abi = "scoop")`.
5. Update `sysroot/print.scoop` to call `__scoop_print` / `__scoop_println` instead of the old intrinsic print names.
6. Adjust the compiler String byte-level special cases so only `byteLength` / `getByte` stay on the intrinsic path; `unsafeSliceBytes` should resolve to the new ordinary String method body.
7. Add or update IR snapshot coverage showing scalar `toString()` calls the runtime symbol directly through scoop ABI without native enter/leave or bridge wrappers.
8. Run focused tests first, then the task-required full fixture suite; update `TODO.md` / `TODO-3.md` only after validation.

## Current Decision Notes

- `__scoop_gc_collect` will be declared as scoop ABI in sysroot, but existing compiler statepoint lowering is not removed in this task because `P7-T03` explicitly tracks the `scoop_gc_collect` vs `scoop_gc_collect_safepoint` runtime-symbol alignment and current GC/statepoint IR tests depend on the safepoint helper shape.

## Progress Updates

- Added scoop ABI declarations and direct calls for scalar `toString`, print/println, panic, GC collect, string concat, and string unsafe slice runtime entries.
- Moved `String.unsafeSliceBytes` onto an ordinary `@Unsafe` String body method and removed it from the compiler's synthetic byte-level String intrinsic path; `byteLength` and `getByte` remain compiler-owned.
- Updated build fixture sysroot overlays so they expose the new print and unsafe-slice surfaces used by shared sysroot support files.
- Added an IR regression that checks scalar `toString()` calls runtime symbols directly without scalar bridge or native enter/leave.
- Focused test exposed a direct blocker in the owner-test shape: `(42).toString()` is currently parsed/lowered with an integer literal span of `"(42)"`, causing an integer-literal panic. This is directly relevant to `P7-T01` because the task specifies that IR snapshot shape, so the plan is updated to fix the parenthesized literal receiver path rather than narrowing the test.
- Fixed the parenthesized expression span issue by preserving the inner expression span for grouping expressions; the `(42).toString()` owner IR regression now passes.
- Focused validations passed for scalar direct runtime IR, String byte-level lowering, concrete scalar receiver lowering, scalar bridge compatibility, unsafe-slice diagnostics, unsafe-slice run-pass, and the affected sysroot overlay build fixtures.
- The first full fixture run exposed three additional snapshot/parse regressions caused by over-narrowing grouped expression spans. The fix was refined so only literal grouping keeps the literal token span while non-literal grouping preserves the historical outer span; the affected parse and MIR snapshot fixtures now pass.
- Final validation status: `cargo test --all --all-targets` passed; `cargo clippy --all-targets -- -D warnings` passed; `cargo run -p scoop -- test` completed with the known 7 baseline fixture failures and 1335 passing targets / 1372 passing checks.
