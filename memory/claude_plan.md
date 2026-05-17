# Claude Execution Plan

本文件记录本次调用的可审计执行计划与进度更新。内容为行动计划与决策摘要，不包含私密推理链。

## 当前目标

- 以 `TODO.md` 为唯一任务顺序与完成状态来源。
- 找到第一个标题未带 `[DONE]` 的任务。
- 完整实现并验证该任务，更新 `TODO.md` 完成记录，提交 Git，然后停止。

## 初始步骤

1. 读取 `TODO.md`，确认第一个未完成任务及其验证要求。
2. 读取相关计划或上下文文件，仅限于理解当前任务所需内容。
3. 检查最近提交是否明确提到与当前任务直接相关的未完成事项。
4. 基于当前任务定位相关代码、测试和 fixtures。
5. 实施最小且完整的代码或文档变更。
6. 运行当前任务要求的验证命令；若发现阻塞性规格缺口，按要求更新 `TODO.md` 并停止。
7. 将已完成任务标题加 `[DONE]` 并填写完成记录。
8. 运行最终相关验证，检查工作区状态。
9. 按任务编号提交所有相关变更。

## 进度日志

- 已写入初始计划。下一步读取 `TODO.md` 确认当前任务。
- 已确认第一个未完成任务为 `C2-T01C`：删除 MIR 分析、dump、materialize、effect facts 与 effect-lowered metadata/classification 中的 CaptureBox arm。
- 已审计目标 source：`mir`、`effect_facts`、`effect_lowered` 中无 `CaptureBox`/`capture_box`/`mir_capture_box`/`__CaptureBox` 残留；仅修正 `mir/escape.rs` 的旧 capture-box 描述注释。
- 已完成验证：`cargo build -p scoopc`、目标 `rg`、`cargo test -p scoopc mir -- --nocapture`、`cargo test -p scoopc effect_facts -- --nocapture`、`cargo clippy -p scoopc --all-targets -- -D warnings` 均通过。
- 已将 `TODO.md` 中 `C2-T01C` 标记为 `[DONE]` 并填写完成记录。下一步检查 diff/status 后提交。

## C2-T01C 执行计划

1. 检查最近提交与工作区状态，确认是否存在与 `C2-T01C` 直接相关的未完成事项或用户改动。
2. 定位 `TODO.md` 指定的 CaptureBox 命中点：`mir/closure_simplify.rs`、`escape.rs`、`inline.rs`、`summary.rs`、`dump.rs`、`mir/materialize/*`、`effect_facts/builder.rs`、`effect_lowered/{frame,segment,materialize/classification}.rs`。
3. 删除仍存在的 CaptureBox 特例；若 C2-T01A/B 已提前删除部分代码，则以审计和残留清理为主。
4. 保留普通 closure env transport 与 `MirBoxingReason::ClosureCapture` 逻辑，不误删 composite env boxing。
5. 运行任务要求验证：`cargo build -p scoopc`、CaptureBox 搜索、`cargo test -p scoopc mir -- --nocapture`、`cargo test -p scoopc effect_facts -- --nocapture`；必要时运行 clippy 定向验证。
6. 更新 `TODO.md`：给 `C2-T01C` 标题加 `[DONE]`，填写改动范围、核心决策、验证结果和计划闭合说明。
7. 检查最终 diff 与状态，提交本任务相关变更后停止。
