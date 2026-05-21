# 执行计划

说明：本文件记录可审计的执行计划、关键决策和进度更新，不记录隐藏推理链。

## 初始计划

1. 读取 `TODO.md`，按文件顺序找出第一个标题未显式带 `[DONE]` 的任务。
2. 检查该任务的要求、依赖、验证命令和完成记录；只在与当前任务直接相关时查看 `PLAN.md`、最近提交或代码上下文。
3. 确认当前工作树状态，避免覆盖用户或其他代理的未提交修改。
4. 实现第一个未完成任务；如果发现阻塞当前任务的真实前置问题，按要求把最小前置任务插入 `TODO.md` 并停止。
5. 运行当前任务要求的验证；若出现与任务相关的失败，修复后重新验证。
6. 完成后更新 `TODO.md`：在任务标题前加 `[DONE]`，并填写完成记录。
7. 仅在阶段级计划或依赖结构变化时更新 `PLAN.md`。
8. 检查差异，提交本次任务相关的全部未提交变更。
9. 停止，不处理下一个任务。

## 当前状态

- 已识别第一个未完成任务：`P3-T01`，建立 `scoopc_mir_facts` crate 与 MIR facts 数据模型。
- 最近提交为 `95c5fdb7 [TODO-4-INIT] Detail P3 MIR task package`，未显示需要抢先处理的直接未完成阻塞。
- 初始工作树检查时只有本计划文件新增/修改；后续变更均为本任务实现与记录。

## P3-T01 执行步骤

1. 对照已有 fact/base crate 的结构，确认 workspace 成员、crate 命名、文档和测试风格。
2. 检查 `tools/scoop_tools` dependency gate 的现有规则，加入 `scoopc_mir_facts` fact crate 约束。
3. 如基础 identity 不足，优先在 `scoopc_ids` 中添加 stage-independent key；避免暴露 MIR 内部 key。
4. 新增 `crates/scoopc_mir_facts/`，实现 `MirFacts`、root inventory、snapshot binding、instance/callable family inventory、pass artifact metadata、pass pipeline metadata、dump/verifier skeleton 和单元测试。
5. 将新 crate 加入 workspace，并在 `crates/scoopc/Cargo.toml` 中仅加入必要依赖或 re-export anchor。
6. 更新 `README.md` 的 workspace/crate 概览。
7. 运行 P3-T01 指定验证并修复相关失败。
8. 更新 `TODO.md` 与 `TODO-4.md` 完成记录，提交变更后停止。

## 进度更新

- 已新增 `scoopc_mir_facts` crate，包含 `MirFacts` 顶层结构、root inventories、snapshot binding、instance/family inventory、pass artifact metadata、pass pipeline metadata、dump/verifier skeleton 和单元测试。
- 已在 `scoopc_ids` 新增 `StageArtifactKey`，用于表达 stage-independent artifact identity，避免 MIR 内部 key 外泄。
- 已更新 workspace 成员、`scoopc` facade anchor、dependency gate 和 README crate 概览。
- 已通过验证：`cargo fmt`、`cargo check -p scoopc_mir_facts`、`cargo test -p scoopc_mir_facts`、`cargo run -p scoop_tools -- dependency-gate`、`cargo clippy --all-targets -- -D warnings`、`cargo test -p scoopc_ids`、`cargo test -p scoop_tools dependency_gate`。
- 已将 `P3-T01` 在 `TODO.md` 与 `TODO-4.md` 标记为 `[DONE]` 并填写完成记录。
- 已检查最终差异与 staged diff，无空白错误；下一步提交本任务变更。
