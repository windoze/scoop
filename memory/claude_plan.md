# Execution Plan

## Current Status

- Invocation started on 2026-05-16.
- `TODO.md` has been inspected.
- First incomplete task: `P4-T01m` - verify and lock `@Intrinsic class/struct` data-member and memory-layout constraints.
- Added focused typecheck fixtures for intrinsic struct ctor/body fields and class `var` field surfaces.
- Fixed the struct declaration checker so `@Intrinsic struct` direct fields are reported by the dedicated intrinsic layout diagnostic instead of ordinary struct field shape diagnostics.
- Validation complete for `P4-T01m`.
- `TODO.md` has been updated to mark `P4-T01m` as `[DONE]` with completion record and sysroot audit notes.

## Plan

1. Check working tree status and latest commit message for directly relevant unfinished `P4-T01m` context.
2. Inspect the existing intrinsic type field diagnostic implementation and current fixtures.
3. Add focused typecheck fixtures covering intrinsic struct primary constructor fields and body direct-field properties across `val`, `var`, initialized direct fields, and default-value surfaces. Current parser/spec inspection found no `lateinit` syntax entry, so that surface is not currently observable as a fixture input.
4. If inspection reveals a real leak in `check_builtin_annotations_on_type_decl`, fix the diagnostic path without changing the existing diagnostic message or allowing direct fields.
5. Audit current sysroot `@Intrinsic class/struct` declarations and record whether internal state access is exposed only through `@Intrinsic method` or `@Extern function`.
6. Run the validation required by `P4-T01m`: `cargo test -p scoopc`, `cargo run -p scoop -- test --fixtures tests/fixtures/typecheck`, and `cargo clippy --all-targets -- -D warnings`.
7. On success, mark `P4-T01m` as `[DONE]`, update its completion record and this plan file, commit the changes, and stop.

## Validation Plan

- Use the validation requirements specified by the selected `TODO.md` task as authoritative.
- Prefer targeted tests first, then broader `cargo test`/fixture checks if the task touches shared compiler or runtime paths.
- Run formatting or linting checks where applicable before committing.

## Validation Results

- `cargo fmt --all`
- `cargo run -p scoop -- test --fixtures tests/fixtures/typecheck` initially failed on three new `@Intrinsic struct var` cases with `struct_field_must_be_val`; after fixing diagnostic ownership, rerun passed: 445 passed / 0 failed.
- `cargo test -p scoopc`: 863 passed / 0 failed.
- `cargo clippy --all-targets -- -D warnings`: passed.

## Remaining Action

- Commit the `P4-T01m` changes and stop without starting `P4-T01n`.
