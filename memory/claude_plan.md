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
- Prior invocation completed P10-T03; TODO.md now marks it `[DONE]`.
- Selected first incomplete task for this invocation: P10-T03R ("Review per-cone frontend orchestration").
- Latest commit is `[P10-T03] Run frontend per cone DAG`, which is directly relevant to this review.
- Review plan: inspect the P10-T03 implementation for source-read isolation, frontend artifact import coverage, and regression proof; fix any review findings; rerun P10-T03 validation; mark P10-T03R `[DONE]`; commit only review-related changes plus this progress file.
- Review finding fixed: per-cone frontend indexing must use the current unit manifest's `export_entry_points`, not the final consumer manifest. Added a regression test for dependency-cone export entry checking and re-exported `import_upstream_artifacts` through the `scoopc::cone` facade.
- Validation completed successfully: formatting, clippy, workspace build, full Rust tests, run_pass_cone fixtures, run-pass fixtures, and diff check.
- Updated TODO.md and TODO-7.md to mark P10-T03R `[DONE]`. Next step: final diff/status review and commit.
