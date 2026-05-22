## Execution Plan

This file records the actionable plan, decisions, and progress for the current invocation. It does not contain private reasoning, but it captures the steps needed to audit the work.

### Current Objective

Complete exactly the first incomplete task in `TODO.md`, validate it, mark it `[DONE]`, commit the resulting changes, and stop.

### Step-by-Step Plan

1. Read `TODO.md` and identify the first task whose heading is not prefixed with `[DONE]`.
2. Check the latest commit message for any explicitly unfinished issue that is directly relevant to that task.
3. Read the selected task details, dependencies, validation requirements, and any nearby completion records.
4. Inspect only the code, tests, fixtures, and documentation needed for that task.
5. Implement the task as written, without narrowing the scope or using workaround behavior.
6. If a concrete blocker or missing prerequisite prevents correct implementation, update `TODO.md` with the minimum prerequisite task in the right order, commit that bookkeeping, and stop.
7. Run the task-specific validation and any relevant broader tests required by the task.
8. If validation exposes unscheduled failures, fix them if in scope or schedule the minimum prerequisite/follow-up task before marking the current task done.
9. Mark the completed task heading in `TODO.md` with `[DONE]` and update its completion record with implementation and validation notes.
10. Review the final diff, run formatting/linting if relevant, commit all intended changes with a task-tagged message, and stop without starting the next task.

### Progress Log

- Initialized plan before reading project task state.
- Read `TODO.md`; first incomplete task is `P7-T04-b-5` in `TODO-6.md`, covering pre-existing LLVM library test failures observed during P7-T04-b.
- Read `TODO-6.md` task details. Scope is the four scheduled LLVM lib failures: closure late-lowering routing, generic-via-interface P4 site facts, native callable aggregate-return ABI constant argument shape, and top-level immutable init helper emission/audit shape. Latest commit is `P7-T04-b-4R`, with no separate unfinished issue requiring insertion before this task.
- Reproduced all four failures. Root-cause plan: align plain closure ABI symbols with private closure body symbols; make late lowering use canonical pass-view bodies for body-dependent lowering; preserve scalar literal operands for typed call args instead of forcing temp store/load; update the top-level immutable audit to the P6 eager-init contract by checking guard/load and absence of hidden init calls.
- Implemented the four fixes and added a P5 pass-view body regression. The four scheduled LLVM tests and the new regression pass individually after `cargo fmt`.
- Full validation completed: `cargo test -p scoopc --lib` passed 900 tests after refreshing the internal failure-policy sentinel baseline; `cargo run -p scoop -- test --fixtures tests/fixtures/effect_lowered` passed 10/10 after regenerating the changed direct/fun-value call golden; `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass` passed 421/421; `cargo clippy --all-targets -- -D warnings` and `git diff --check` passed.
- Marked `P7-T04-b-5` as `[DONE]` in `TODO.md` and `TODO-6.md`; completion record documents root causes, fixes, regenerated golden/sentinel baseline, and validation commands.
