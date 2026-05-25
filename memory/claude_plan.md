# Claude Execution Plan

## Scope

- Follow `TODO.md` as the authoritative task list.
- Complete exactly the first task whose heading is not prefixed with `[DONE]`.
- Do not proceed to the next task after completion.
- Keep this file updated when the plan changes or when key milestones are completed.

## Step-by-Step Plan

1. Read `TODO.md` and identify the first incomplete task by heading prefix.
2. Review the selected task details, dependencies, validation requirements, and completion record.
3. Check recent repository state and relevant files only after selecting the current task.
4. Implement the selected task as written, without narrowing scope or using workaround behavior.
5. Add or update focused tests/fixtures required by the task.
6. Run validation in the required order: `cargo fmt`, `cargo clippy --all-targets -- -D warnings`, full Rust tests, then full fixture suite when applicable.
7. If validation exposes an unscheduled failure, fix it or add the minimum prerequisite/follow-up task to `TODO.md` before marking the current task complete.
8. Mark the task heading in `TODO.md` with `[DONE]` and update its completion record.
9. Commit all task-related changes with a clear task-tagged commit message.
10. Stop after the commit.

## Progress Log

- Initial execution plan recorded before running repository commands.
- Selected first incomplete task: `P10-T04R` (`Review per-cone fingerprint cache`) from `TODO.md` / `TODO-7.md`.
- Latest commit is `58da69d8 [P10-T04-c-4] Link cached dependency objects`; it directly satisfies the final prerequisite for `P10-T04R`, so the review will cover `P10-T04`, `P10-T04-b`, and `P10-T04-c` behavior.
- Review finding: `compute_cone_build_fingerprint` currently folds the entire `sysroot/` tree into the global toolchain input. This over-invalidates builds when an unselected sysroot cone changes. Plan update: make sysroot hashing graph-scoped and let selected sysroot cone changes propagate through existing per-cone dependency fingerprints.
- Implemented the graph-scoped sysroot fingerprint fix and added `selected_sysroot_digest_ignores_unselected_sysroot_cones` regression coverage. `cargo fmt`, `cargo clippy --all-targets -- -D warnings`, and `cargo test -p scoop --bin scoop selected_sysroot_digest_ignores_unselected_sysroot_cones` have passed.
- Full validation progress: `cargo test --all --all-targets`, `cargo run -p scoop -- test --fixtures tests/fixtures/run_pass_cone/source_path_dependency_public_call`, `cargo run -p scoop -- test --fixtures tests/fixtures/run_pass_cone`, and `cargo run -p scoop -- test` passed. Manual release reproducer passed: cold 2.56s, warm 0.18s, consumer-edit rebuild 0.84s without dep recompile, post-edit warm 0.19s; rebuilt executable printed `42`; `nm` showed consumer `main.o` has `dependencyValue` as `U` and dep `objs/scoop.o` defines it as `T`.
- Marked `P10-T04R` as `[DONE]` in `TODO-7.md` and `TODO.md`, with completion notes for the sysroot invalidation fix and validation results.
