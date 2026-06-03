# 当前执行计划

说明：本文件记录可审计的执行计划、关键决策和进度更新，不包含隐藏推理过程。

## 初始计划

1. 读取 `TODO.md`，按标题是否带 `[DONE]` 判定第一个未完成任务。
2. 查看最近提交信息，只在其明确提到与当前任务直接相关的未完成事项时纳入当前任务或作为 `TODO.md` 前置项。
3. 阅读当前任务关联的规格、代码和测试，确认要求、依赖与验证命令。
4. 完整实现第一个未完成任务；如发现阻塞当前任务的缺失特性、规格不匹配或测试失败，优先修复，或在 `TODO.md` 插入最小前置任务后停止。
5. 运行格式化、lint 和相关测试；若代码发生变更，再按要求运行完整测试与 fixture 套件。
6. 更新 `TODO.md`：完成时给任务标题加 `[DONE]` 并填写 completion record；只有阶段级计划变化时才更新 `PLAN.md`。
7. 检查工作区差异，提交本次任务相关全部变更，然后停止，不继续下一个任务。

## 进度记录

- 已创建初始计划文件，下一步读取任务列表并定位第一个未完成任务。
- 已确认第一个未完成任务为 `T1-01：新增 LirArtifact / CodegenInput 类型`。
- 最近提交 `d5e0b0ad Pivot to structural fact refactor; archive fact-unify plan` 未明确提到与 T1-01 直接相关的未完成修复。
- 当前任务执行策略：只新增过渡类型与模块导出，保持行为不变；随后按基线运行格式化、lint、测试与 fixture 验证。
- 已新增 `crates/scoopc/src/pipeline/lir_artifact.rs`，并在 `pipeline/mod.rs` 中按 `llvm` feature 导出 `CodegenInput` 与 `LirArtifact`。
- 验证已通过：`cargo fmt`、`cargo clippy --all-targets -- -D warnings`、`cargo test --all --all-targets`、`cargo build -p scoop -p scoopc`、`python3 tools/dependency_gate.py`、`python3 tools/spec_fixtures.py check`、`python3 tools/run_fixtures.py`（fixtures: ok 1664）。
- 已更新 `TODO.md`，将 T1-01 标记为 `[DONE]` 并填写完成记录；下一步检查 diff 并提交。
