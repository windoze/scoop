# Claude Execution Plan

## Scope

- Execute exactly the first incomplete task in `TODO.md`, then stop.
- Use `TODO.md` as the authoritative ordering and completion source.
- Update this file as the task is identified, implemented, validated, documented, and committed.
- This file records the execution plan and progress notes; it does not include hidden chain-of-thought.

## Step-by-Step Plan

1. Read `TODO.md` and identify the first task whose heading is not prefixed with `[DONE]`.
2. Check recent Git context only for information directly relevant to that first incomplete task.
3. Read the task details, dependencies, validation requirements, and any relevant code/spec files.
4. Decide whether the task can be completed as written or requires a concrete prerequisite task.
5. If implementable, make the smallest correct code/documentation/test changes needed for the task.
6. Run formatting first, then linting, then relevant/full validation as required by the task instructions.
7. If validation exposes unscheduled failures, fix them or add the minimum required prerequisite/follow-up task before marking completion.
8. Mark the completed task title in `TODO.md` with `[DONE]` and update its completion record.
9. Commit all intended changes with a clear task-tagged message.
10. Stop without starting the next task.

## Progress Log

- Initial plan recorded before reading `TODO.md`.
- Identified first incomplete task: `P10-T06-d` in `TODO-7.md` (`收紧 scoop facade 剩余 lib API 依赖与 single-file virtual cone 零回退`).
- Recent commits checked for directly relevant unfinished context. Latest commit is `[P10-T06-b] Extract scoopld link-cone boundary`; `TODO-7.md` records `P10-T06-c` as completed with remaining single-file fallback and `scoopc` lib dependency intentionally moved to `P10-T06-d`.
- Execution focus for this invocation: remove remaining `scoop` facade dependencies on `scoopc` / `scoopld`, route single-file executable builds through materialized virtual cone artifacts and `scoopc` subprocesses, tighten dependency gate checks, validate, mark `P10-T06-d` done, and commit.
- Implementation completed: `scoop` now invokes `scoopc` tooling through subprocesses for build/dump/test boundaries, single-file executable builds materialize a virtual cone and compile through `build-single-cone` + `link-cone`, fixture runner moved under `scoopc`, and dependency gate now enforces the `scoop` direct-dependency/source residual boundary.
- Validation completed so far: `cargo fmt`; `cargo clippy --all-targets -- -D warnings`; `cargo build --workspace`; `cargo run -p scoop_tools -- dependency-gate`; `cargo test --all --all-targets`; `cargo run -p scoop -- test`; metadata/source residual audits; single-file build; explicit `--sysroot-dep` run with fixture overlay; `git diff --check`.
- Final step completed: staged all task-related changes (including pre-existing uncommitted P10 facade/project-model state that this task depends on) and committed them as `[P10-T06-d] Finish scoop facade subprocess boundary` (`22b8f1af`).
