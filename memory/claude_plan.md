# Claude Execution Plan

## Current Invocation

Goal: complete exactly the first incomplete task in `TODO.md`, validate it, mark it `[DONE]`, commit the result, then stop.

## Plan

1. Read `TODO.md` to identify the first task whose heading is not prefixed with `[DONE]`.
2. Check recent git context only as needed for the selected task, especially if the latest commit mentions an unfinished issue relevant to it.
3. Inspect the code and tests directly related to that task.
4. Implement the smallest spec-correct change required for the selected task, without workaround behavior.
5. Run focused validation first, then required broader validation for the task.
6. Update `TODO.md` by prefixing the completed task heading with `[DONE]` and adding an accurate completion record.
7. Update this plan file with progress and validation results.
8. Commit all task-related changes with a descriptive task-tagged message.
9. Stop without starting the next task.

## Progress

- Initial execution plan recorded.
- Read `TODO.md`; first incomplete task is `C2-T01A`: remove CaptureBox types and transport model from MIR core.
- Scope for this invocation is limited to C2-T01A plus any direct compile errors caused by removing the MIR core model.
- Removed the MIR CaptureBox rvalue variants, CaptureBox transport kind/metadata, and capture-box stable-id test role.
- Cleaned direct downstream compile references exposed by the core removal without introducing a replacement implicit box model.
- Validation passed: `cargo build -p scoopc`; required `rg` check produced no MIR/stable-id matches; targeted MIR/LLVM tests passed; `cargo clippy -p scoopc --all-targets -- -D warnings` passed.
- Updated `TODO.md` completion record for C2-T01A; `PLAN.md` did not need changes.
