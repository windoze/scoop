# Current invocation plan — P9-T07R review

## Current task

- First incomplete task: `P9-T07R` in `TODO.md` / `TODO-7.md`.
- Task type: review task for `P9-T07` cone two-layer split.
- Scope:
  - Verify cone data layer is owned by base/project-model crates only.
  - Verify cone operation layer is independent in `scoopc_cone`.
  - Verify no stage crate depends back on `scoopc_cone`.
  - Verify sysroot loader ownership is settled in `scoopc_project_model`.

## Step-by-step plan

1. Inspect repository state and latest commit for any unfinished issue directly relevant to `P9-T07R`.
2. Inspect `P9-T07` implementation surfaces: workspace manifests, `scoopc_project_model`, `scoopc_cone`, `scoopc` facade/imports, sysroot ownership, and dependency gate rules.
3. Run the review-required validation commands from `P9-T07`:
   - `cargo fmt`
   - `cargo build --workspace`
   - `cargo test --all --all-targets`
   - `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass`
   - `cargo run -p scoop_tools -- dependency-gate`
   - `git diff --check`
4. Run the explicit dependency-shape checks for the completion conditions:
   - `cargo tree -p scoopc_project_model`
   - `cargo tree -p scoopc_cone`
5. If review finds issues, fix them in scope, update this plan with the changed approach, and rerun the relevant validations.
6. Mark `P9-T07R` as `[DONE]` in both `TODO.md` and `TODO-7.md`, adding a completion record with review findings and validation results.
7. Commit all resulting changes with a `P9-T07R` message and stop without starting `P9-T08`.

## Progress log

- Plan updated for `P9-T07R` before implementation commands.
- Repository state inspected: latest commit is `[P9-T07] Split cone into project-model base and scoopc_cone op layer`; it does not name an unfinished issue for this review.
- Boundary inspection so far matches the review scope: `scoopc_project_model` owns cone/sysroot data-layer APIs, `scoopc_cone` owns operation-layer APIs, `scoopc::cone` is a façade, and no stage crate references `scoopc_cone`.
- Documentation gap found in scope: `P9-T07` is marked `[DONE]` but lacks a completion record in `TODO-7.md`; repair this while completing `P9-T07R`.
- Review correction: the explicit `cargo tree -p scoopc_cone` completion condition requires cone to sit above all stage crates, but `scoopc_cone` lacked a direct `scoopc_codegen_llvm` dependency. Add it without enabling LLVM default features, then rerun validation.
- Correction completed: `scoopc_cone` now directly depends on `scoopc_codegen_llvm` with `default-features = false`; `TODO.md` and `TODO-7.md` now mark `P9-T07R` done and include completion records for `P9-T07` and `P9-T07R`.
- Final validation after correction passed: `cargo fmt`, `cargo clippy --all-targets -- -D warnings`, `cargo build --workspace`, `cargo test --all --all-targets`, `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass`, `cargo run -p scoop_tools -- dependency-gate`, direct dependency tree checks, and `git diff --check`.
