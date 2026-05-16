# Claude Execution Plan

## Scope

- Source of truth: `TODO.md`.
- Goal for this invocation: identify and complete exactly the first incomplete task, then stop after committing.
- Note: this file records the execution plan and progress in a concise, auditable form; it does not include private chain-of-thought.

## Initial Plan

1. Read `TODO.md` to identify the first heading not explicitly prefixed with `[DONE]`.
2. Check the latest commit message only for a directly relevant unfinished issue tied to that task.
3. Read the selected task details, dependencies, and validation requirements.
4. Inspect only the code, fixtures, docs, and tests relevant to that task.
5. Implement the smallest spec-correct change needed for the task, without workaround behavior.
6. Add or update focused tests/fixtures required by the task.
7. Run the task-specified validations and any narrowly relevant checks; fix failures caused by this work.
8. Update `TODO.md` by prefixing the completed task title with `[DONE]` and filling its completion record.
9. Run formatting if needed and re-run relevant validation.
10. Commit all task-related changes with a clear task-tagged message.
11. Stop without starting the next task.

## Progress Log

- Started invocation and recorded the initial execution plan.
- Identified first incomplete task: `P5-T00` (`补跑 Linux/amd64 LLVM ABI 矩阵并固化可复现执行环境`).
- Latest commit is `4688fa91 [P5-T00] Record Linux ABI matrix prerequisite`, directly relevant to the selected task; treat the recorded Linux runner prerequisite and prior Docker timeout as part of this task context.

## P5-T00 Execution Plan

1. Inspect existing CI/Docker/tooling and ABI documentation relevant to P5-T00.
2. Add the smallest reproducible Linux/amd64 runner artifact (prefer CI workflow or Dockerfile) that installs Rust stable, clang, and LLVM 21 compatible with the current `llvm-sys`/`inkwell` configuration.
3. Execute the P5-T00 Linux/amd64 validation matrix if the runner can be used locally; otherwise capture the concrete blocker and add the minimal prerequisite task rather than marking P5-T00 done.
4. If validations pass, update `TODO.md` completion record and ABI documentation with exact commands, platform, and LLVM version.
5. Run formatting/lint or relevant checks for changed files.
6. Commit all changes for this task and stop.

## P5-T00 Findings

- Remote CI run `25950480831` for commit `4688fa91` completed successfully on GitHub Actions `ubuntu-24.04` / `x86_64-unknown-linux-gnu` with LLVM `21.1.8` and clang target `x86_64-pc-linux-gnu`.
- That run covers `cargo test --all --all-targets`, no-default/runtime feature tests, spec fixture check, and full `cargo run -p scoop -- test` fixture smoke (`1367` checks).
- The current CI workflow does not yet run Linux `cargo clippy --all-targets -- -D warnings`, which is an explicit P5-T00 validation item.
- Local Docker attempt using `docker.1ms.run/rust:bookworm` with `--platform linux/amd64` timed out after 90 minutes while still installing/downloading LLVM 21 packages and never reached `cargo clippy`; the container was stopped. This confirms local emulation is not a reproducible P5-T00 validation path.

## Adjusted Plan

1. Attempt a local `linux/amd64` Docker Clippy run using an LLVM 21 environment to fill the missing Linux Clippy validation without changing task scope.
2. If Docker/emulation remains non-reproducible or too slow, update the CI runner artifact to include platform diagnostics, explicit P5 ABI targeted checks, and Linux Clippy, then keep P5-T00 incomplete with the remaining requirement clearly recorded.

## Current Next Step

- Patch `.github/workflows/ci.yml` to make the Linux/amd64 runner explicitly record platform/LLVM details and run the missing P5 ABI/Clippy matrix. Since pushing is not permitted in this invocation, `P5-T00` must remain incomplete until that updated workflow is run remotely.

## Progress Update

- Updated `.github/workflows/ci.yml` with `Platform diagnostics`, explicit `P5 ABI targeted matrix`, and Linux `Clippy` steps.
- Updated `TODO.md` P5-T00 current record with the successful partial GitHub Actions Linux run, the failed local Docker Clippy attempt, and the remaining requirement to push/run the updated CI workflow before marking the task complete.
- `P5-T00` remains intentionally incomplete because the required Linux Clippy/P5 targeted matrix logs do not exist yet for the updated workflow.
- Validation for this partial/blocker commit: `git diff --check` passed.
