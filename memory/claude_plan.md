# Claude Execution Plan

## Scope

- Work from `TODO.md` as the authoritative task list.
- Complete exactly the first task whose heading is not prefixed with `[DONE]`, then stop.
- Keep `PLAN.md` unchanged unless phase-level sequencing or dependencies actually change.
- Avoid workarounds; any blocker that prevents spec-correct completion must be fixed or scheduled as a prerequisite in `TODO.md`.

## Step-by-step Plan

1. Read `TODO.md` to identify the first incomplete task and its validation requirements.
2. Inspect recent git state and the latest commit only as needed to detect unfinished work directly relevant to that task.
3. Read the files and tests relevant to the selected task; avoid broad unrelated triage.
4. Implement the smallest correct change that fully satisfies the selected task.
5. Add or update focused tests/fixtures required by the task.
6. Run formatting first, then clippy with warnings denied, then the relevant and full validation commands required by the task.
7. If any unscheduled test or fixture failure appears, either fix it or add the minimum prerequisite/follow-up task to `TODO.md` before completion.
8. Update `TODO.md` by prefixing the completed task heading with `[DONE]` and filling in its completion record.
9. Update this file after key progress points or plan changes.
10. Inspect git status/diff/log, stage the intended files, commit with a task-tagged message, and stop without starting the next task.

## Current Status

- Initial execution plan written before running project commands.
- Read `TODO.md`; first incomplete task is `TC-01: LIR lift 落地为全函数，填满所有 callable body`.
- Inspected TC-01 implementation points. `lift.rs` still returns `Result` through `invalid_lift`; callers are `builder.rs` and `segment.rs`.
- Latest commit only updates planning files and does not introduce an unfinished issue directly changing TC-01 execution.
- MIR production validation already rejects Todo placeholders, but `UnresolvedName` can still pass the current production guard, so the MIR→LIR guard must cover it explicitly.
- Added the MIR-side LIR-lift placeholder guard and made the lift chain total.
- Fixed strict clippy blockers discovered during validation.
- First full `cargo test --all --all-targets` exposed two TC-01 regressions: direct bodyless callable references needed stable non-FQN LIR refs, and boundary source statement anchors needed mapping through state-owned LIR slices rather than block ids.
- Implemented LIR callable refs for direct/bodyless targets and state-slice based boundary anchor mapping.
- `cargo fmt` and `cargo clippy --all-targets -- -D warnings` now pass after those fixes.
- The targeted `p7_default_pipeline` failures were fixed by preserving/rebasing source slices through state rewrites and by storing real source-slice coordinates in statement classifications.
- `cargo test -p scoop --test p7_default_pipeline` now passes after rebuilding `scoopc`.
- Final §9 validation passed: `cargo fmt`, `cargo clippy --all-targets -- -D warnings`, `cargo test --all --all-targets`, `cargo build -p scoop -p scoopc`, `python3 tools/dependency_gate.py`, `python3 tools/spec_fixtures.py check`, and `python3 tools/run_fixtures.py`.
- Updated `TODO.md` to mark TC-01 `[DONE]` and recorded the completion summary.
- Next step: inspect git status/diff/log, stage intended files, commit TC-01, and stop.
