# Claude Execution Plan

## Scope

- Follow `TODO.md` as the authoritative task list.
- Complete exactly the first task whose title is not prefixed with `[DONE]`, then stop.
- Do not perform broad historical triage before selecting the current task.
- Do not use workarounds for spec mismatches; if a prerequisite blocker is discovered, record it in `TODO.md`, commit that bookkeeping, and stop.

## Step-By-Step Plan

1. Inspect `TODO.md` and identify the first incomplete task by heading/title prefix.
2. Check the latest commit message only for unfinished work directly relevant to that task.
3. Read the task body, dependencies, validation requirements, and any completion record.
4. Inspect the relevant source, tests, fixtures, and documentation needed for that task.
5. Implement the smallest complete spec-correct change for the selected task.
6. Add or update focused tests/fixtures required by the task.
7. Run targeted validation first, then broader required validation from the task.
8. Fix any failures caused by the task work; if a real prerequisite blocker appears, update `TODO.md` accordingly and stop after committing.
9. Mark the completed task title in `TODO.md` with `[DONE]` and update its completion record.
10. Run final formatting/linting/tests appropriate to the task.
11. Review `git status`, `git diff`, and recent commits; commit all intended changes with a descriptive task-tagged message.
12. Stop without starting the next task.

## Progress Log

- Initial plan recorded before running repository inspection commands.
- Selected current task: `P8-T02` (`归档 audit ledger 并更新 DONE 记录`) from `TODO.md` lines 663-680.
- Task requirements: archive UMB audit ledger/materials, remove or reshape UMB-only audit tooling/tests while keeping durable coverage, confirm all `umb_fix` fixtures are active, update `UnsupportedMainBody_DONE.md`, update `TODO.md`, validate, commit, then stop.
- Latest commit checked: `604d490e [P8-T01] Remove unsupported main body variant`; no extra unfinished issue was identified for P8-T02.
- Pre-cleanup `umb-audit stats` confirmed active=0, retired=1284, initial=1284.
- Implemented initial P8 cleanup: archived the UMB ledger/empty inventory/schema under `docs/archive/audits/unsupported-main-body/`, removed `umb-audit` and UMB inventory-generation tests, and reshaped the remaining audit module into long-term `umb_fix` fixture coverage checks.
- Targeted audit validation passed: `cargo test -p scoopc audit:: -- --nocapture` (12 passed).
- Required P8-T02 validation passed: `cargo run -p scoop -- test tests/fixtures/umb_fix/` (152 passed), `cargo test --all --all-targets`, `cargo run -p scoop -- test` (1558 checks), and `cargo clippy --all-targets -- -D warnings`.
- Final records updated: `UnsupportedMainBody_DONE.md` records P8 completion and archive locations; `TODO.md` marks `P8-T02` as `[DONE]`; final `PLAN.md` and `TODO.md` copies archived under `docs/archive/plans/`.
