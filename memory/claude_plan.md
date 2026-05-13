## Execution Plan

1. Read `TODO.md` to identify the first task whose title is not prefixed with `[DONE]`.
2. Check the latest commit message for anything directly relevant to that task.
3. Inspect only the files needed to understand and implement that task.
4. Implement the task fully, or if a concrete blocker is found, update `TODO.md` with the minimum prerequisite task required before proceeding.
5. Run the relevant tests and required validation commands for the task, plus `cargo clippy --all-targets -- -D warnings` when the affected scope makes that practical for task completion requirements.
6. Update `memory/claude_plan.md` as key steps complete or if the plan changes.
7. Mark the task as `[DONE]` in `TODO.md` with a completion record once the task is fully validated.
8. Commit all current uncommitted changes that belong to finishing this task, then stop.

## Current Task

- First incomplete task: `P5-T01`.
- Scope:
  - replace raw/debug-based HIR dump with a stable renderer
  - replace raw/debug-plus-canonicalize MIR dump with a stable renderer
  - replace `dump-ir` materialized MIR debug output with a stable renderer using stable instance display/local labels
  - refresh affected fixture expectations under `tests/fixtures/hir/**`, `tests/fixtures/mir/**`, and `tests/fixtures/mir_refactor/**`

## Current Working Plan

1. Inspect current HIR, MIR, and materialized MIR data structures and identify the minimal renderer surface required by fixtures and dump commands.
2. Add dedicated stable dump renderers instead of relying on raw `Debug` text or post-processing rewrites.
3. Wire the new renderers into stage outputs, fixture runner paths, and CLI commands.
4. Update tests that still assert old raw-debug/canonicalize behavior.
5. Refresh affected fixture outputs and run the required validation commands.
6. Update `TODO.md` completion record and commit the task.

## Progress

- Completed: added shared dump helpers plus dedicated stable HIR/MIR/materialized MIR renderers.
- Completed: switched typed HIR stage, direct MIR stage, `dump-hir`, `dump-mir`, `dump-ir`, fixture runner, and relevant tests to the stable renderer surfaces.
- Completed: refreshed `tests/fixtures/hir/**`, `tests/fixtures/mir/**`, and `tests/fixtures/mir_refactor/**` to the new protocol.
- Completed validation:
  - `cargo test -p scoopc`
  - `cargo run -p scoop -- test --fixtures tests/fixtures/hir`
  - `cargo run -p scoop -- test --fixtures tests/fixtures/mir`
  - `cargo run -p scoop -- test --fixtures tests/fixtures/mir_refactor`
  - `cargo test -p scoop`
  - `cargo clippy -p scoopc --all-targets -- -D warnings`
  - `cargo clippy -p scoop --all-targets -- -D warnings`
- Completed audit: refreshed HIR/MIR fixture outputs no longer contain `TypeId(`, `S0`, `C0`, `bb0`, or `site0`.

Note: This file records a concise execution plan and progress updates rather than private chain-of-thought.
