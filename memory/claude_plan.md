# 当前执行计划

## 范围

- 以 `TODO.md` 为唯一任务顺序来源，识别第一个标题未带 `[DONE]` 的任务。
- 只完成该任务；完成后更新任务记录、提交 Git，然后停止。
- 不做开放式历史问题清扫；只处理会阻塞当前任务或直接影响当前任务正确性的缺陷。

## 执行步骤

1. 阅读 `TODO.md`，定位第一个未完成任务，并记录任务编号、要求、依赖和验证命令。
2. 如最近提交明确提到与该任务直接相关的未完成问题，检查是否需要纳入当前任务或作为前置任务写入 `TODO.md`。
3. 检查与当前任务相关的代码、测试、fixture 和文档，确定最小正确实现路径。
4. 若发现当前任务依赖缺失语言特性、运行时行为或测试基础设施，且无法按规范完成，则只更新 `TODO.md` 添加最小前置任务，提交后停止。
5. 若可直接完成，实施最小正确代码/测试/文档变更。
6. 运行当前任务指定的验证，以及必要的相关测试；若失败，优先修复与当前任务相关的问题。
7. 在 `TODO.md` 中给完成的任务标题加 `[DONE]`，并更新完成记录；仅在阶段计划确实变化时更新 `PLAN.md`。
8. 检查 Git 状态和差异，提交本次任务相关的所有未提交变更。
9. 停止，不继续处理后续任务。

## 当前状态

- 已读取 `TODO.md` / `TODO-5.md`，第一个未完成任务是 `P4-T01：建立独立 scoopc_effect_facts crate 与事实数据模型`。
- 最近提交为 `[TODO-5-INIT] Detail P4/P5 task package`，与当前任务相关但不包含额外未完成实现项；继续按 P4-T01 执行。
- 已检查现有 effect facts 模块、fact crate 结构、dependency gate 与 workspace 配置。
- 当前 `MaterializedEffectFacts` 仍使用 `scoopc::mir::InstanceKey`、`BasicBlockId` 和 pass view 进行生产查询；一次性改完 builder/solver 只读化属于 P4-T02，不应塞进 P4-T01。
- P4-T01 采用的实现路径：新增只依赖基础 crate 的 `scoopc_effect_facts` 数据产品，新增必要 stage-independent identity，迁入/固定公开 facts/schema/dump/verifier 数据边界，并在 `scoopc` 内提供 adapter 把现有 materialized facts 转成独立产品。
- 已完成实现与验证：`cargo fmt`、`cargo check -p scoopc_effect_facts`、`cargo test -p scoopc_effect_facts`、`cargo test -p scoopc --no-default-features effect_facts_stage`、`cargo run -p scoop_tools -- dependency-gate`、`cargo clippy --all-targets -- -D warnings`、`git diff --check` 均通过。
- 已将 P4-T01 在 `TODO.md` 与 `TODO-5.md` 标记为 `[DONE]` 并填写完成记录；下一步检查 diff/status 后提交本任务变更。
