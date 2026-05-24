# Claude Execution Plan

## Current objective
Complete exactly the first incomplete task in TODO.md, then stop after documenting and committing the result.

## Step-by-step plan
1. Read TODO.md to identify the first task whose heading is not prefixed with [DONE].
2. Check the latest commit message for an unfinished issue only if it is directly relevant to that selected task.
3. Read the task details, dependencies, validation requirements, and nearby project context needed for that task.
4. Implement the selected task completely, avoiding workarounds or scope narrowing.
5. Run formatting, clippy, relevant tests, then full validation required by the task policy.
6. If validation exposes an unscheduled failure, fix it or add the minimum prerequisite/follow-up task in TODO.md before marking completion.
7. Mark the task heading [DONE] in TODO.md and update its completion record. Update PLAN.md only if phase-level sequencing changes.
8. Commit all task-related changes with a clear task-tagged commit message and the required co-author trailer.

## Progress log
- Plan initialized before task execution.
- Selected first incomplete task: P10-T03-a ("补齐 ConeArtifact frontend import payload"). Latest commit also references this prerequisite, so it is directly relevant and in scope.
- Next: inspect existing artifact, ScoopIR, annotation, visibility, pre-specialize, and consume APIs to add a persisted frontend import payload and helper surface.
- Implementation approach chosen: reuse the existing `.cone` ScoopIR/annotation/visibility/pre-specialize schemas as `ConeArtifactFrontendImport`, persist it as `frontend_import.bin`, version it in `manifest.json`, and add artifact-based frontend injection helpers.
- Adjustment: `frontend_import.bin` was changed to `frontend_import.json` because the reused ScoopIR schema is JSON-oriented and bincode does not support its serde-tagged enum shape.
- Completed implementation and validation for P10-T03-a. Updated TODO.md and TODO-7.md completion records; next step is final diff check and commit.
- Selected first incomplete task for this invocation: P10-T03 ("run_frontend 改造为按 cone DAG 拓扑顺序运行"). Latest commit completed its direct prerequisite P10-T03-a, so it is relevant and in scope.
- Next: inspect P10-T03 requirements and current frontend/build orchestration, then implement artifact-based upstream frontend imports without reading upstream sources.
- Implemented the first pass: `scoopc_cone` can now build frontend import payloads from in-memory typechecked cone state, and `run_frontend` processes cone compilation units in topological order while injecting dependency artifacts into downstream `Index`/`TypeEnv`.
- Validation found run-pass `with_update_*` failures because frontend-import export was consuming typecheck side tables needed by final codegen. Adjusted orchestration so default sysroot and the final consumer are not exported as dependency artifacts, and dependency export uses cloned AST/context data.
- Completed P10-T03 validation and updated TODO.md / TODO-7.md completion records. Next: final status review and commit only this task's relevant changes plus the required progress file.
