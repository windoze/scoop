# 当前执行计划

说明：按安全约束，这里记录可审计的执行思路摘要、决策依据与步骤计划，不记录不可审计的原始内部推理。

## 目标

按 `TODO.md` 的顺序完成第一个未完成任务，完成后更新相关文档、验证、提交，然后停止。

## 计划步骤

1. 读取 `TODO.md`，定位第一个标题未标记 `[DONE]` 的任务，并确认其正文要求、依赖、验证方式。
2. 检查最近一次提交是否直接提到与该任务相关且未完成的问题；若是，则将其视为该任务的一部分或作为前置依赖写回 `TODO.md`。
3. 阅读实现与测试相关代码，定位当前任务涉及的模块、快照、渲染器或基线输出位置。
4. 在不偏离任务要求的前提下，实施最小且正确的代码修改；若遇到阻塞当前任务的真实缺口，先把该缺口作为新的前置任务写入 `TODO.md`，再停止继续实现。
5. 运行任务要求的验证命令，以及受影响范围内的测试、格式化、lint；修复发现的问题，直到通过或确认存在必须先处理的新前置任务。
6. 更新 `TODO.md`：将已完成任务标题改为 `[DONE]`，补全完成记录；仅在阶段计划确有变化时更新 `PLAN.md`。
7. 检查工作区状态，确认提交范围；按仓库风格创建一次提交，包含本次任务完成所需全部改动。
8. 更新本文件，记录实际完成情况、偏差、验证结果与提交信息，然后停止。

## 进度记录

- 已创建本计划文件。
- 已读取 `TODO.md`，确认首个未完成任务为 `P5-T02：重写 effect facts / effect lowered dump renderer，并刷新相关 snapshot`。
- 已检查最近一次提交：`[P5-T01] Stabilize HIR/MIR dump surfaces`，未发现直接声明且阻塞当前任务的未完成后续项。
- 当前实施方案：
  1. 复用 MIR dump 已有的稳定 `local` / `bb` / `site` 标签生成逻辑，对外暴露为可复用 helper。
  2. 为 `LateLoweredProgram` 增加 dump 所需元数据（至少包括类型文本与按 body/version 组织的 MIR 局部标签），由 builder 构建、由 optimizer 保留。
  3. 重写 `effect_facts/dump.rs`：用语义/稳定标签替换 `step_schema#N`、`continuation_schema#N`、`case#N`、`bbN`、`siteN`。
  4. 重写 `effect_lowered/dump.rs`：用语义/稳定标签替换 `t/s/k/c/ri/ko/st/bd/fs/local/bb/site` 等 dense-id surface。
  5. 更新相关 stage/unit tests 与 `tests/fixtures/effect_facts/**`、`tests/fixtures/effect_lowered/**` golden。
  6. 运行格式化、定向测试、全量 `cargo test -p scoopc`、`cargo clippy -p scoopc --all-targets -- -D warnings`，随后回写 `TODO.md` 并提交。
- 已完成实现：
  - `effect_facts` / `effect_lowered` dump 都已改为独立稳定 renderer。
  - `LateLoweredProgram` 已持有 dump 所需类型文本与 body label 元数据；optimizer 会保留这些元数据。
  - `tests/fixtures/effect_facts/*.effectfacts` 与 `tests/fixtures/effect_lowered/*.effectlowered` 已刷新到新协议。
- 已完成验证：
  - `cargo fmt`
  - `cargo test -p scoopc`
  - `cargo test -p scoop`
  - `cargo run -p scoop -- test --fixtures tests/fixtures/effect_facts`
  - `cargo run -p scoop -- test --fixtures tests/fixtures/effect_lowered`
  - `cargo clippy -p scoopc --all-targets -- -D warnings`
  - `cargo clippy -p scoop --all-targets -- -D warnings`
  - 精确文本审计：effect facts / effect lowered fixtures 与 renderer 源码中已无 `step_schema#0`、`continuation_schema#0`、`case#0`、`k0`、`ri0`、`ko0`、`st0`、`bd0`、`fs0`、`local0`、`bb0`、`site0` 等旧协议命中。
- 已更新 `TODO.md`：`P5-T02` 已标记为 `[DONE]`，完成记录已补全。
