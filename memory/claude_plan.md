# Claude Execution Plan

## Scope

- Work from `TODO.md` as the authoritative task list.
- Complete exactly the first task whose heading is not prefixed with `[DONE]`, then stop.
- Do not perform broad historical triage before selecting the current task.

## Step-by-Step Plan

1. Read `TODO.md` and identify the first incomplete task by heading prefix.
2. Check the latest commit summary only for unfinished work directly relevant to that selected task.
3. Inspect only the files needed to understand and implement that task.
4. Implement the task without workarounds or spec deviations.
5. Run formatting first, then clippy with warnings denied, then relevant tests, and finally the full required suites unless only documentation changed or an explicit prior green result applies.
6. If an unscheduled failing test, fixture, or blocking implementation gap is found, fix it if in scope; otherwise add the minimum prerequisite task to `TODO.md`, commit that scheduling change, and stop.
7. Mark the completed task title in `TODO.md` with `[DONE]` and update its completion record.
8. Update `PLAN.md` only if phase-level sequencing or completion criteria changed.
9. Commit all task-related changes with a clear task-tagged message.
10. Stop without starting the next task.

## Progress Log

- Initial execution plan recorded before reading task files or running repository commands.
- Selected first incomplete task: `T2-04-R: Review T2-04`.
- Review scope is limited to T2-04 acceptance criteria: per-callable facts owned by callable/program nodes, source/intrinsic lookups not via FQN maps in codegen ABI paths, and no missing-fact fallback reintroduced.
- Latest commit is `[T2-04] Fold per-callable LIR facts into callables`, directly relevant to this review, so its implementation boundaries will be checked as part of T2-04-R.
- Review finding: `published_callable_signature_with_names_impl` reads body-less declaration source signatures from the active LIR program but still returns the primary codegen `TypeStore` as their owner. This can bypass cross-TypeStore remapping when ABI visibility/source signatures come from a distinct handoff.
- Planned fix: return the active published LIR TypeStore for declaration source signatures and add a regression test that fails if a declaration-only signature with non-codegen TypeIds is treated as already codegen-owned.
- Implemented the fix in `call/abi.rs` and added a standalone LLVM codegen regression test for declaration-only source signatures with a distinct published TypeStore owner.
- Validation progress: `cargo fmt`, `cargo clippy --all-targets -- -D warnings`, targeted `cargo test -p scoopc_codegen_llvm declaration_source_signature_uses_published_typestore_owner`, full `cargo test --all --all-targets`, `cargo build -p scoop -p scoopc`, `python3 tools/dependency_gate.py`, `python3 tools/spec_fixtures.py check`, and `python3 tools/run_fixtures.py` passed.
