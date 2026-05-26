Execution plan for this invocation:

1. Read `TODO.md` first and identify the first task whose heading is not prefixed with `[DONE]`.
2. Check the latest commit message only for unfinished work directly relevant to that selected task.
3. Inspect the selected task's nearby requirements, dependencies, validation notes, and completion record.
4. Implement the task as written, without narrowing scope or using workarounds for missing language/runtime behavior.
5. If a concrete prerequisite blocks the task, update `TODO.md` with the minimum prerequisite task in dependency order, commit that bookkeeping, and stop.
6. Otherwise, run formatting, linting, relevant tests, and then any required full suites in the required order.
7. Address any unscheduled test or fixture failures by fixing them or scheduling them explicitly before marking the task complete.
8. Mark the completed task heading in `TODO.md` with `[DONE]`, update its completion record, and update this progress file at key milestones.
9. Commit all changes for this single completed task with an appropriate message and stop without starting the next task.

Progress update:

- Selected first incomplete task: `P1-T00` (`scoopc check-source` command surface).
- Planned implementation steps:
  1. Inspect `TODO.md`, `PLAN.md`, `TEST_INFRA_CLEANUP.md`, `docs/fixtures.md`, and the existing `scoopc` CLI/frontend APIs for parse, resolve, typecheck, and infer behavior.
  2. Add a non-fixture `scoopc check-source` CLI command that supports `parse`, `resolve`, `typecheck`, and `infer` phase-only validation for single-file and cone project inputs, including `--source <path>` and `--target-platform <id>`.
  3. Document stdout, stderr, and exit-code behavior in `docs/fixtures.md`.
  4. Add or update focused tests covering argument parsing and command behavior without introducing any fixture-specific API or naming.
  5. Run `cargo fmt`, `cargo clippy --all-targets -- -D warnings`, relevant tests, and required suites; fix any unscheduled failures.
  6. Mark `P1-T00` as `[DONE]` in `TODO.md`, update its completion record, update this progress file, commit all changes, and stop.
- Completed implementation:
  - Added `scoopc check-source` parsing and dispatch plus frontend-only command execution for single-file and cone project source inputs.
  - Added target-platform propagation through session sysroot loading, source-cone graph loading, frontend type environments, and typed-HIR support-source loading.
  - Documented the command contract in `docs/fixtures.md` and marked `P1-T00` complete in `TODO.md`.
  - Validation passed: `cargo fmt`, `cargo clippy --all-targets -- -D warnings`, targeted `target/debug/scoopc check-source` smoke checks, `cargo test --all --all-targets`, and `cargo run -p scoop -- test` (1532 checks).
