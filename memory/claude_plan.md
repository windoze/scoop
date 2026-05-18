## Execution Plan

Status: initialized.

I will follow the repository task workflow and complete exactly the first incomplete task in `TODO.md`, then stop after committing the result.

Steps:

1. Read `TODO.md` first and identify the first task whose heading is not prefixed with `[DONE]`.
2. Check the latest commit only for unfinished work directly relevant to that task.
3. Inspect the task requirements, dependencies, and validation instructions.
4. Implement the task as written, without weakening scope or using workaround fixtures.
5. Run the relevant targeted tests, then broader validation if required by the task.
6. If a real blocker prevents spec-correct completion, update `TODO.md` with the minimum prerequisite task, keep the current task incomplete, commit that bookkeeping, and stop.
7. If completed, update `TODO.md` by prefixing the task heading with `[DONE]` and filling in the completion record.
8. Update this file at key milestones.
9. Review git status/diff/log, stage only intended files, commit with a task-tagged message, and stop.

Selected task: `P7-0-T02：把 audit 常量改成退场倒计时`.

Latest commit check: `419e3b25 [P7-0-T01] Add stable UMB inventory IDs`; it is the direct prerequisite and does not indicate unfinished relevant work.

Task-specific plan:

1. Inspect `umb_inventory` audit code, `umb-audit` CLI output, and user-visible failure policy tests.
2. Replace fixed active/kind assertions with countdown-safe active/retired/initial accounting while preserving `INITIAL_ENTRY_COUNT` for `active + retired == initial`.
3. Ensure stats output includes active, retired, initial, by_class, and by_bucket.
4. Ensure diff reports concrete inventory drift instead of failing merely because active rows fall below initial.
5. Improve audit failure messages with actionable ID, bucket, and file context.
6. Run the task-required validation commands.
7. Mark `P7-0-T02` `[DONE]`, fill its completion record, commit, and stop.

Progress update:

- Selected and scoped `P7-0-T02` only.
- Replaced fixed active-count and fixed literal/dynamic-kind assertions with current active inventory counting.
- Added explicit active + retired = initial countdown checks using `INITIAL_ENTRY_COUNT = 1_284`.
- Updated `umb-audit stats` to print active, retired, and initial counts.
- Added diff-mode stable ID matching so `umb-audit diff` can report unretired source additions/deletions instead of panicking solely because active rows are below the initial baseline.

Final task progress before commit:

- `TODO.md` now marks `P7-0-T02` as `[DONE]` and records scope, decisions, inventory/ledger status, and validation results.
- Validation passed:
  - `cargo run -p scoopc --bin umb-audit -- stats`
  - `cargo run -p scoopc --bin umb-audit -- diff`
  - `cargo test -p scoopc audit:: -- --nocapture`
  - `cargo test -p scoopc pipeline_user_visible_failure_policy -- --nocapture`
  - `cargo clippy --all-targets -- -D warnings`

Current next action: inspect git status/diff/log, stage intended files, commit `P7-0-T02`, and stop.
