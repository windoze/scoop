# Claude Execution Plan

## Scope

- Execute exactly the first incomplete task in `TODO.md`.
- Treat `TODO.md` as authoritative for ordering, dependencies, validation, and completion state.
- Stop after completing and committing that one task, or after committing any required prerequisite/blocker bookkeeping.

## Plan

1. Read `TODO.md` to identify the first task whose heading is not prefixed with `[DONE]`.
2. Check the latest commit message only for unfinished issues directly relevant to that selected task.
3. Read the selected task details, dependencies, and validation requirements.
4. Inspect only the code, fixtures, and documentation relevant to the selected task.
5. Implement the task as specified, avoiding workarounds or scope narrowing.
6. If a concrete blocking prerequisite is discovered, update `TODO.md` with the minimum required prerequisite task in dependency order, document the blocker here, commit that bookkeeping, and stop.
7. Run the task-required validation and any directly relevant tests. Fix failures that are in scope.
8. Mark the task heading in `TODO.md` with `[DONE]` and update its completion record.
9. Run final verification appropriate to the change.
10. Commit all relevant changes with a clear task-tagged message.
11. Stop without starting the next task.

## Progress Log

- Created initial execution plan before reading project task state.
- Read `TODO.md`; first incomplete task is `P5-T01` in `TODO-2.md`.
- Read `TODO-2.md`; `P5-T01` requires runtime C implementations for `scoop_string_from_byte_array`, `scoop_string_from_char_array`, and `scoop_string_from_string_array`, plus runtime API exports and validation.
- Checked latest commit: `[P4-T02] Remove array builder surface`; no directly relevant unfinished issue was indicated.

## Current Task Plan: P5-T01

1. Inspect runtime String allocation helpers, UTF-8 encoding logic, mutable array layout, trap helpers, runtime API list, and existing runtime tests.
2. Add shared runtime helpers only where they keep the implementation small and class-wide, such as mutable-array shape checks or UTF-8 codepoint encoding.
3. Implement the three runtime symbols with single allocation and memcpy/encoding passes.
4. Export the symbols through `SCOOP_RUNTIME_API_X_LIST`.
5. Add or update focused runtime tests for byte, char, string, empty, 4-byte codepoint, and surrogate replacement behavior.
6. Run required validation (`cargo build`) and relevant runtime test commands, then broader checks if needed.
7. Mark `P5-T01` done in `TODO.md` and `TODO-2.md` with completion record.
8. Commit all changes for this task and stop.

## Discovery

- `scoop_mutable_array_new` currently normalizes all WORD arrays to machine-word-sized storage, but `P5-T01` requires byte and char arrays to arrive with `elem_size_bytes == 1` and `elem_size_bytes == 4` respectively. This is directly blocking the task, so the implementation will include the class-wide storage/stride correction needed for WORD `MutableArray<T>` values before adding the new String runtime entries.
- Implemented the runtime-side WORD storage correction and the matching LLVM array intrinsic stride change; also filled the directly relevant `sizeOf/alignOf<Char/Float*>` reflection gap so `mutableArrayNew<Char>()` can materialize a 4-byte element layout.
- Added `scoop_string_from_byte_array`, `scoop_string_from_char_array`, and `scoop_string_from_string_array`, exported them in the runtime ABI list, and covered them with runtime Rust tests.
- Validation completed: targeted runtime tests passed, `cargo build` passed, `cargo test --all --all-targets` passed, and `cargo clippy --all-targets -- -D warnings` passed. Full fixture run completed with only the pre-existing P4-T02 `mutable_array_ops_basic.scoop` failure.
- Marked `P5-T01` as `[DONE]` in `TODO.md` and `TODO-2.md`, with completion record added to `TODO-2.md`.
