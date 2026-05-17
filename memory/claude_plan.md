# Claude Execution Plan

## Scope
- Follow `TODO.md` as the authoritative task list.
- Identify the first task whose heading is not prefixed with `[DONE]`.
- Complete exactly that task, then stop after committing changes.

## Execution Steps
1. Read `TODO.md` and recent git context to identify the current task and any directly relevant unfinished issue from the latest commit.
2. Read only the files needed to understand and implement the selected task.
3. Implement the task without workarounds or spec deviations.
4. Add or update the smallest relevant tests or fixtures.
5. Run focused validation first, then broader validation required by the task.
6. Update `TODO.md` by prefixing the completed task heading with `[DONE]` and filling its completion record.
7. Update this file when key steps complete or if the plan changes.
8. Commit all task-related changes with a clear task-tagged commit message.
9. Stop without starting the next task.

## Current Status
- Identified first incomplete task: `P12-T04` in `TODO-5.md`.
- Latest commit is `[P12-T03] Record execution completion`; no directly relevant unfinished issue was found in the commit message.
- `P12-T04` requires removing the sysroot-only exemption from missing-body typecheck policy, verifying no internal `regular_fun_requires_body` sysroot exception remains, then running build and fixture validation.
- Code change applied: `regular_fun_requires_body` now runs for sysroot and user files alike.
- Added a build fixture with a companion sysroot overlay containing a normal no-body function to verify sysroot files receive the same `fun_must_have_body` diagnostic.
- Full fixture validation exposed four existing sysroot overlay fixtures with ordinary no-body `MutableArray` helper declarations. These overlays were updated to mirror production sysroot wrapper bodies and runtime extern declarations instead of relying on the removed exemption.
- Validation completed: targeted new fixture passed; `cargo build` passed; build fixture directory passed; full `cargo run -p scoop -- test` passed; `cargo clippy --all-targets -- -D warnings` passed; `cargo test --all --all-targets` passed.
- `TODO.md` and `TODO-5.md` now mark `P12-T04` as `[DONE]` with completion record.
- Next step: commit task-related changes only. Existing untracked `CLOSURE_FIX.md`, `OVERLOAD_RESOLUTION.md`, and `UnsupportedMainBody_FIX.md` are unrelated and will not be staged.
