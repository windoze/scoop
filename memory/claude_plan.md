# 执行计划

说明：本文件记录可审计的执行计划、关键决策和进度更新，不记录隐藏推理链。

## 本轮范围

- 以 `TODO.md` 作为任务顺序与完成状态的唯一权威来源。
- 找到第一个标题未标记 `[DONE]` 的任务，只完成该任务后停止。
- 不在选定任务前做开放式历史问题扫描。
- 不接受 workaround；如果发现阻塞当前任务的真实前置问题，就在 `TODO.md` 中加入最小前置任务并停止。

## 执行步骤

1. 读取 `TODO.md`，定位第一个未完成任务。
2. 读取该任务所在的详细 TODO 文件，确认目标、复查范围、验证命令和完成条件。
3. 只查看与当前任务直接相关的最近提交、工作树状态和代码/文档上下文。
4. 对当前任务要求的范围做人工复查；如果发现阻塞项，在本 review 内修复或按规则登记前置任务。
5. 运行任务要求的验证命令和必要的补充验证。
6. 在 `TODO.md` 和对应详细 TODO 文件中把当前任务标记为 `[DONE]`，并填写完成记录。
7. 更新本文件记录关键进展和验证结果。
8. 检查 `git status`、`git diff`、`git diff --check` 和最近提交，提交本轮变更。
9. 停止，不处理下一个任务。

## 当前任务

- 第一个未完成任务：`P3-T01R`。
- 任务类型：review 任务，复查 `P3-T01` 建立的 `scoopc_mir_facts` crate 与 MIR facts 数据模型。
- 最近相关提交：`f49dfe96 [P3-T01] Establish MIR facts crate`。

## 复查结论

- `scoopc_mir_facts` 的直接依赖只有允许的基础 crate：`scoopc_span`、`scoopc_source`、`scoopc_types`、`scoopc_ids`、`scoopc_project_model`。
- `scoopc_mir_facts` 源码没有引用 `scoopc` facade、HIR/MIR stage 类型、backend/LLVM 类型、其它 fact crate、`TemplateKey` 或 `InstanceKey`。
- `MirFacts` 已按 root inventories、materialized snapshot binding、instance/callable family inventory、pass artifact metadata、MIR pass pipeline metadata 分组。
- `README.md`、`scoopc` facade anchor、workspace 成员和 `tools/scoop_tools` dependency gate 均已覆盖 `scoopc_mir_facts`。
- 未发现需要在本 review 内修复的阻塞项。

## 验证记录

- 已通过：`cargo fmt`
- 已通过：`cargo check -p scoopc_mir_facts`
- 已通过：`cargo test -p scoopc_mir_facts`
- 已通过：`cargo run -p scoop_tools -- dependency-gate`
- 已通过：`cargo clippy --all-targets -- -D warnings`
- 已通过：`cargo tree -p scoopc_mir_facts`
- 已通过：`cargo test -p scoopc_ids`
- 已通过：`cargo test -p scoop_tools dependency_gate`
- 已通过：`git diff --check`

## 进度更新

- 已在 `TODO.md` 中将 `P3-T01R` 标记为 `[DONE]`。
- 已在 `TODO-4.md` 中将 `P3-T01R` 标题标记为 `[DONE]`，并填写 review 结论、dependency gate 结论、验证命令和残余风险。
- 下一步：最终检查差异并提交本轮 review 任务变更。
