# Claude Execution Plan

## Scope
- Follow `TODO.md` as the source of truth.
- Complete exactly the first task whose heading is not prefixed with `[DONE]`.
- Stop after implementing, validating, documenting completion, and committing that one task.

## Steps
1. Read `TODO.md` to identify the first incomplete task and its validation requirements.
2. Check recent git context only as needed to detect directly relevant unfinished work for that task.
3. Inspect the smallest relevant set of source, fixture, and documentation files.
4. Implement the task without workarounds or spec deviations.
5. Run the task-specific validation commands, plus broader checks if needed by the task.
6. Fix any failures that are in scope for the task.
7. Update `TODO.md` by prefixing the completed task heading with `[DONE]` and filling its completion record.
8. Update this plan file when key progress changes or if the plan changes.
9. Commit all relevant changes with a task-specific commit message.
10. Stop without starting the next task.

## Current Status
- First incomplete task identified: `P5-T02` in `TODO-2.md`.
- Latest commit `[P5-T01] Add string array runtime constructors` is directly relevant and provides the runtime symbols required by this task.
- Confirmed `abi = "scoop"` extern declarations still reject explicit `@Unsafe`; implement the byte-array path as a private raw extern plus an unsafe public wrapper.
- Confirmed class property initializers and `MutableArray<T>.push/freeze` are available for `StringBuilder`.
- Implemented `sysroot/lang_string.scoop` extern declarations and `StringBuilder`; added `lang_string_builder_basic` run-pass fixture and stdout baseline.
- Owner fixture initially showed `StringBuilder` without an explicit primary constructor is not callable; updated the class to `StringBuilder()` to satisfy the required `StringBuilder()` user surface.
- Rerun exposed a resolver/typecheck blocker: an imported extension `scoop.core.add` can be pre-bound before receiver type is known and can shadow a real receiver member named `add`.
- Fixed member-call typecheck so nominal direct members are late-resolved and preferred over pre-resolved extension functions.
- Adjusted the byte-array subcase to use explicitly typed `Byte` locals, keeping the test focused on the new byte-array extern wrapper instead of generic numeric literal inference.
- Found `lang_string.scoop` with bodies must be a compilable sysroot support source; updated sysroot loading to compile it while retaining a signature-only index copy.
- Direct run now reaches codegen but the byte-array subcase exposes `MutableArray<Byte>.push` lowering failure: `UInt8` is not converted to the word payload expected by the runtime push entry.
- Root cause narrowed to HIR lowering not expanding standard scalar typealiases (`Byte`, `Short`, `Long`, `Double`, etc.) to their builtin/fixed-width representation, even though typecheck expands them.
- Added HIR lowering cases for the standard scalar aliases.
- Link then exposed that reachable support-source class initializers need monomorph call-site bindings for generic calls; updated frontend expression checking to collect monomorph requests for support sources too while relying on MIR root filtering to avoid making them initial roots.
- The remaining missing symbol came from class constructor/property initializer reachability: materialization scanned object/top-level init but not class init steps. Added class init data to the materializer and reachability scanning for class constructor init steps, defaults, delegation, super ctor args, and ctor bodies.
- Owner fixture now passes through the fixture runner.
- Full fixture baseline completed with the pre-existing `tests/fixtures/run-pass/mutable_array_ops_basic.scoop` failure already recorded by P4-T02 for P9-T02; no new fixture failure from this task.
- Rust test suite `cargo test --all --all-targets` passes after formatting and after updating the request-root test to account for support-source monomorph bindings.
- Clippy `cargo clippy --all-targets -- -D warnings` passes after formatting.
- Updated `TODO.md` and `TODO-2.md` to mark `P5-T02` complete with completion record.
- Next action: inspect git diff/status, then commit all task-related changes.
