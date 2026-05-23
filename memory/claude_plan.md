# 执行计划

## 约束说明

- 我会记录可审计的计划、决策依据和执行进度，但不会写入私有逐字推理链。
- `TODO.md` 是任务排序和完成状态的唯一来源；只完成第一个未以 `[DONE]` 标记的任务。
- 如遇到阻塞当前任务的缺陷、缺失功能或未调度的测试失败，会先修复；无法立即修复时，将最小必要前置任务写入 `TODO.md` 并提交后停止。
- 除非阶段级计划确实变化，否则不更新 `PLAN.md`。

## 初始执行计划

1. 读取 `TODO.md`，确认第一个未完成任务的编号、内容、依赖和验证要求。
2. 查看最近提交，仅判断是否存在与该任务直接相关的未完成事项。
3. 检查工作区状态，避免覆盖用户或其他代理的改动。
4. 按任务要求阅读相关代码、测试和文档，确认实现边界。
5. 实施最小正确修改；若发现当前任务被具体前置缺陷阻塞，则按要求更新 `TODO.md` 并停止。
6. 运行相关测试；若发现未调度失败，修复或新增必要前置任务。
7. 更新 `TODO.md`：将完成任务标题前缀改为 `[DONE]`，补充完成记录。
8. 必要时更新本文件记录关键进展。
9. 检查差异并提交本次任务相关改动。
10. 提交后停止，不继续下一个任务。

## 当前状态

- 已读取 `TODO.md`。
- 第一个未完成任务是 `P7-T05`：`P7 全包清场、文档同步与依赖审计`，任务详情位于 `TODO-6.md`。
- 已读取 `TODO-6.md` 中 `P7-T05` 完整要求：需要最终清场 LLVM backend 输入 residual，确认 codegen/reachability 无普通 dispatch 去虚化残留，补齐或记录 dependency gate，并同步文档/TODO。
- 最近提交为 `P7-T04R` 记录和复审 LLVM handoff cleanup，未看到要求当前任务前置修复的未完成事项。
- 工作区当前仅有本计划文件改动；下一步检查现有 dependency gate、README、PIPELINE-CLEANUP 与相关 residual 命中。
- 初步 residual 检查发现 `crates/scoopc/src/pipeline/llvm_codegen_stage.rs` 仍有 `precheck_invalid_integer_literals` 直接遍历 `LoweredHir` body/expr。这属于当前任务要求清除的 backend HIR body 输入，不能只记录为已知问题。
- 计划调整：先把 integer literal overflow/underflow 预检查迁出 LLVM backend（优先迁到 HIR/frontend 屏障或复用已有诊断入口），删除 LLVM stage 的 HIR body precheck，再继续 P7-T05 文档/gate 清场。
- 已完成实现改动：integer literal 目标范围校验已迁到 typecheck/when-pattern 边界，LLVM stage HIR body precheck 已删除，相关 overflow build fixtures 期望错误码已切到 `scoop::typecheck::invalid_integer_literal`。
- 已完成 gate/文档改动：`dependency-gate` 新增 LLVM stage handoff、emit handoff、reachability source-boundary 检查；`README.md`、`PIPELINE-CLEANUP.md`、`PIPELINE_REFACTOR.md` 已同步 P7-T05 清场状态与未来 C backend 共享输入边界。
- 已通过针对性验证：`cargo test -p scoopc --no-default-features int_literal`；四个 integer literal overflow build fixtures；`cargo test -p scoop_tools dependency_gate`；`cargo run -p scoop_tools -- dependency-gate`。
- P7-T05 指定验证集已通过：`cargo fmt`、`cargo run -p scoop_tools -- dependency-gate`、`cargo test -p scoopc_lir_facts`、`cargo test -p scoopc --no-default-features llvm_codegen_stage`、`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass`（421/421 passed）、`cargo clippy --all-targets -- -D warnings`、`git diff --check`。
- 已同步 `TODO.md` 与 `TODO-6.md`：`P7-T05` 标记为 `[DONE]` 并写入完成记录；下一步检查最终 diff，然后提交本任务改动。
