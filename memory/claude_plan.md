# Claude Execution Plan

## Scope

- Follow `TODO.md` as the authoritative task list.
- Identify the first task whose heading is not prefixed with `[DONE]`.
- Complete exactly that one task, then stop.
- If the task is blocked by a concrete prerequisite, update `TODO.md` with the minimum required prerequisite task, commit that bookkeeping change, and stop.

## Step-by-Step Plan

1. Read `TODO.md` to identify the first incomplete task and its validation requirements.
2. Check recent git context only as needed to detect whether the latest commit mentions unfinished work directly relevant to that task.
3. Inspect the smallest relevant part of the codebase for the selected task.
4. Implement the task with the smallest correct change, avoiding workaround behavior or spec deviations.
5. Add or update focused tests/fixtures required by the task.
6. Run the task-specific validation commands first, then broader checks required by the task or repository guidance where feasible.
7. Update `TODO.md` by prefixing the completed task heading with `[DONE]` and filling in its completion record.
8. Update this file when key milestones complete or if the plan changes.
9. Commit all changes related to this task with a clear task-tagged message.
10. Stop without starting the next task.

## Progress Log

- Initialized execution plan before inspecting project files.
- Selected first incomplete task: `CG-T07R` (`Review CG-T07 extern global 与 GC surface`).
- Review scope: rerun CG-T07 validation, inspect extern global lowering/storage metadata use, and inspect GC pin/handle lowering/runtime surface for unsafe-pointer shortcuts or root verifier bypass.
- Inspection milestone: extern global access/store paths prefer materialized MIR `ExternGlobalRoot`; sysroot `GC.pin` / `GC.unpin` / `GC.handle*` lower through runtime pin/handle APIs rather than raw pointer shortcuts. Running targeted validation next.
- Validation milestone: `CG-T07` targeted commands passed (`refactor_llvm_extern_global`, extern global run/negative fixtures, `gc_pin_unpin_basic`, `gc_handle_roundtrip`, `codegen_gap_inventory`, runtime ABI allowlist). Running `cargo clippy --all-targets -- -D warnings` next.
- Validation milestone: `cargo clippy --all-targets -- -D warnings` passed. Additional GC moving/stress/verify-roots runs for `gc_pin_unpin_basic` and `gc_handle_roundtrip` also passed.
- Completed documentation milestone: `TODO.md` now marks `CG-T07R` as `[DONE]` and records the review conclusion plus validation commands.
- Next step: commit the task changes and stop.
