## Execution Plan

This file records the actionable plan and progress for the current invocation. It intentionally contains a concise rationale and step-by-step plan, not hidden internal reasoning.

### Current Objective

- Complete exactly the first incomplete task in `TODO.md`, then stop.
- Selected task: `TC-02-PRE1` - add LIR plain lowering support for effect-typed closure adapters.

### Initial Plan

1. Read `TODO.md` and identify the first task whose title is not prefixed with `[DONE]`.
2. Check the latest commit message only for unfinished work directly relevant to that task.
3. Inspect the files and tests relevant to the selected task.
4. Implement the task as specified, without narrowing scope or using workarounds.
5. Run formatting first, then clippy with warnings denied, then relevant and full validation as required.
6. If an unscheduled failing test or concrete blocker appears, either fix it or add the minimum prerequisite task to `TODO.md` before stopping.
7. Mark the completed task title with `[DONE]` and update its completion record.
8. Commit all intended changes with a clear task-tagged message.

### Progress Log

- Plan file initialized before repository inspection.
- Read `TODO.md`; the first incomplete task is `TC-02-PRE1`.
- Next checks are limited to the latest commit and code paths directly relevant to this task.
- Reproduced the task's targeted failure: `cargo test -p scoop --test p7_default_pipeline single_pipeline_runs_higher_order_function_value_handled_effect_cli -- --nocapture` exits with status 1 instead of expected fixture exit 10.
- Implementation focus: teach LIR plain rvalue lowering to choose effect-typed closure adapter fn pointers for direct `MakeClosure`, propagated closure locals, and struct literal function fields.
- Added LIR-native adapter helper code and connected it to LIR plain rvalue/struct lowering.
- Rebuilt `scoopc` and confirmed the targeted regression now passes: `cargo test -p scoop --test p7_default_pipeline single_pipeline_runs_higher_order_function_value_handled_effect_cli -- --nocapture`.
- `cargo clippy --all-targets -- -D warnings` initially found one local `too_many_arguments` warning in the new helper; added the same scoped allow pattern used nearby, re-ran `cargo fmt`, and clippy passed.
- `cargo test --all --all-targets` exposed 11 existing plain-LIR `scoopc` LLVM unit failures outside the adapter parity change; recorded the exact failing tests under `TC-02` in `TODO.md` so they are explicitly scheduled before TC-02 completion.
- `python3 tools/run_fixtures.py` exposed a broad `268/1625` plain-LIR fixture baseline failure set; recorded the command, count, affected fixture families, and representative root stack points under `TC-02` in `TODO.md`.
- Fixed the new helper's dependency-gate violation by using the LIR `mir_source` boundary alias instead of a direct `crate::mir::` path; re-ran `cargo fmt`, `cargo clippy --all-targets -- -D warnings`, `python3 tools/dependency_gate.py`, and `python3 tools/spec_fixtures.py check` successfully.
- Rebuilt `scoop`/`scoopc` and re-ran the targeted `TC-02-PRE1` p7 regression successfully after the final code cleanup.
- Marked `TC-02-PRE1` as `[DONE]` in `TODO.md` and added its completion record, including passed validations and the TC-02-scheduled baseline failures.
