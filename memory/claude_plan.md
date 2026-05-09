## 当前执行计划

1. 读取 `TODO.md`，按标题是否带有 `[DONE]` 判定第一个未完成任务。
2. 检查最近一次提交是否直接提到与该任务相关且未完成的问题；若该问题构成当前任务的直接前置，则把它视为当前任务的一部分或在 `TODO.md` 中登记为前置依赖。
3. 阅读当前任务涉及的代码、文档、测试与约束，确认实现边界、依赖关系和验证要求。
4. 在不引入规避方案的前提下完成该任务；若发现阻塞当前任务的真实缺陷或缺失能力，则先修复该阻塞，或在 `TODO.md` 中以最小新增前置任务显式记录后停止。
5. 运行与该任务直接相关的验证，并补充必要测试；若有失败，立即修复直到通过，必要时再跑更广的检查。
6. 更新文档与跟踪文件：
   - 在 `TODO.md` 中将完成的任务标题改为 `[DONE]` 前缀，并更新完成记录；
   - 仅当阶段计划/依赖发生变化时更新 `PLAN.md`；
   - 在本文件记录关键进展、计划调整和阻塞信息。
7. 检查工作区改动，按要求提交当前任务相关的全部未提交更改，提交信息使用任务编号。
8. 完成后停止，不继续下一个任务。

## 说明

- 这里记录的是可审计的执行计划与关键决策，不包含冗长的内部推理草稿。
- 如果后续发现阻塞、计划变更或关键步骤完成，会继续更新本文件。

## 进展记录

- 2026-05-09：已读取 `TODO.md` 并确认首个未完成任务为 `CG-T08R`（Review CG-T08 codegen phase exit audit）。
- 2026-05-09：已检查最新提交 `dc4251d3`（`[CG-T08] Complete codegen phase exit audit`）；提交信息未直接声明新的未完成缺陷，因此按 `CG-T08R` 既定范围执行复核。
- 2026-05-09：下一步先抽查 `CG-T08` 相关产物（codegen regression matrix、阶段退出审计与 `PIPELINE_GAPS.md` 状态更新），再重跑 `CG-T08` 要求的验证命令；若复核无缺口，则把 `CG-T08R` 标记为完成并提交。
- 2026-05-09：已完成产物抽查：`crates/scoop/tests/cg8_codegen_regression_matrix.rs` 覆盖 `CG-T01`-`CG-T07` 与 `P7-T02Z` 代表样本；`crates/scoop/src/fixtures/mod.rs` 保留 `run_all_recreates_session_between_independent_fixtures`；`crates/scoop/tests/p7_default_pipeline.rs` 持续守护 omission=refactor 与 default-vs-explicit refactor 等价。
- 2026-05-09：已重跑验证并全部通过：`cargo test --all`、`cargo run -p scoop -- test`、`SCOOP_GC_MOVE=1 SCOOP_GC_STRESS=1 SCOOP_GC_VERIFY_ROOTS=1 cargo run -p scoop -- test --fixtures tests/fixtures/runtime_gc`、`cargo clippy --all-targets -- -D warnings`。
- 2026-05-09：复核结论：未发现需要回退到 `CG-T08` 的遗漏缺口；已在 `TODO.md` 将 `CG-T08R` 标记为 `[DONE]`，下一步执行 git 提交并停止。
