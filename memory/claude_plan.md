# Execution Plan

I will follow `TODO.md` as the source of truth and complete exactly the first incomplete task. I cannot record private chain-of-thought, but this file contains the complete operational plan and progress log for this invocation.

## Selected Task

First incomplete task: `P4-T01`, "Implement overload effective signature and signature equivalence helper".

## Constraints

- Do not mark the task complete until implementation, fixtures, validation, TODO records, and a git commit are complete.
- Return type and effect row must not participate in signature equivalence.
- Equality must use normalized type structure with alias expansion, not rendered `TypeStore::display()` text.
- Diagnostics must identify both conflicting declarations with locations and rendered signatures.
- Stop after this task; do not proceed to `P4-T01R`.

## Step-by-Step Plan

1. Check the latest commit for any unfinished issue directly relevant to `P4-T01`.
2. Inspect the current overload checking implementation and related resolve/type metadata:
   - `crates/scoopc_hir/src/typecheck/overloads.rs`
   - `crates/scoopc_hir/src/resolve/mod.rs`
   - relevant type alias/equality and generic-bound helpers.
3. Inspect `OVERLOAD_RESOLUTION.md` sections referenced by `P4-T01` to confirm the required effective-signature rules.
4. Add or reuse a structural effective-parameter/effective-signature representation:
   - concrete parameter types remain concrete;
   - method-level type parameters are replaced by their declared bound or default `Any`;
   - composite parameter types recursively substitute method type parameters with effective bounds;
   - alias expansion is handled before equivalence.
5. Wire the helper into function and constructor overload conflict checks.
6. Ensure return-only and effect-only differences are rejected as signature conflicts when parameters match.
7. Add typecheck fixtures for:
   - return-only conflict;
   - effect-only conflict;
   - type-parameter alpha-equivalent conflict;
   - `<T>` vs `<T: Any>` conflict.
8. Run formatting and validation in the required order:
   - `cargo fmt`
   - `cargo clippy --all-targets -- -D warnings`
   - targeted fixtures as needed while iterating
   - `cargo test --all --all-targets`
   - `python3 tools/run_fixtures.py`
9. Update `TODO-4.md` and `TODO.md` by marking `P4-T01` as `[DONE]` and filling the completion record.
10. Commit all changes for this task with a descriptive message and the required co-author trailer.

## Progress Log

- Selected `P4-T01` as the first incomplete task.
- Wrote the operational plan before implementation work.
- Checked repository state and latest commit. Latest commit is `[P3-T07R] Review default internal visibility`, which does not introduce an unfinished issue that preempts `P4-T01`. Unrelated untracked `GC_PACING.md` will be left untouched.
- Implemented the effective-type model in the overload checker: method type parameters are substituted by declared/default bounds, structural parameter signatures drive equivalence, and diagnostics now include rendered conflicting signatures.
- Added targeted typecheck fixtures for effect-only conflicts, alpha-equivalent type parameters, `<T>` vs `<T: Any>`, and transparent type alias equivalence. Formatting, clippy, workspace build, and targeted overload fixtures are passing.
- Full Rust tests and full fixture suite passed. Marked `P4-T01` as `[DONE]` in `TODO.md` and `TODO-4.md` with completion record.
- Committed the P4-T01 implementation as `[P4-T01] Implement overload effective signatures`.
