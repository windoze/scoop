# Claude Plan

## Constraints
- 不写入私有推理细节；这里只记录可审阅的执行计划、发现、决定与进度。
- 本次只处理 `TODO.md` 中第一个未完成任务；若遇到既有问题阻塞，则先修复该问题或把前置任务插入 `TODO.md` 后停止。

## Initial Execution Plan
1. 检查最新一次提交信息，确认是否明确提到需要先修复的既有问题；若有，先定位并修复。
2. 阅读 `TODO.md`，确定第一个未完成任务。
3. 评估该任务规模；若过大，则把它拆分为更小的子任务，并更新 `PLAN.md` 与 `TODO.md`，本次只执行拆分后的第一个子任务。
4. 在实现前阅读相关代码与测试，确认现状、依赖关系与潜在既有缺陷。
5. 完整实现当前目标任务，不采用规避性方案；若发现规格不匹配或实现边界缺口，优先修复，或把前置修复任务插入 `TODO.md`。
6. 运行相关验证：至少覆盖直接相关测试，并在需要时运行更广泛检查；尽量确保无新增告警，必要时运行 `cargo fmt`、`cargo test`、`cargo clippy --all-targets -- -D warnings`。
7. 更新文档与跟踪文件：在 `TODO.md` 标记完成状态，更新 `PLAN.md`，并在本文件记录关键进展与计划调整。
8. 按仓库提交风格创建一次 git commit，然后停止，不继续处理下一个任务。

## Progress Log
- 已创建初始计划，等待检查最新提交与任务列表。
- 已检查最新提交：`[T5000j3b2R] Confirm higher-order MIR backend boundary`，提交说明未声明额外待修既有缺陷。
- 已读取 `TODO.md` / `PLAN.md`，确认当前首个未完成任务是 `T5000j3bR Review：确认 higher-order / closure 场景扩张没有把分析责任倒灌回 backend`。
- 当前 review 范围：`llvm/codegen/mir_body.rs`、`llvm/reachability.rs`、`llvm/codegen/mod.rs`、`llvm/codegen/effect/state_machine_bridge.rs`、相关 MIR/pass-view/测试；重点核对 production MIR higher-order/closure 覆盖是否仍只消费 shared facts / pass artifacts，并确认 backend 没有重新承担 target-set 收缩、escape 推断或 suspendability 分析职责。
- 已完成代码复核：当前 higher-order / closure 覆盖继续依赖 materialized MIR、pass-view、`ProgramFacts`、`EffectAnalysisCtx` 与 shared suspendability/escape facts；未发现需要先修复的新既有缺陷。
- 已完成验证：`cargo fmt --all --check`、5 个定向 LLVM 回归、`cargo test --all`、`cargo clippy --all-targets -- -D warnings`、`cargo run -p scoop -- test` 均通过。
- 已更新 `TODO.md` / `PLAN.md`：将 `T5000j3bR` 标记完成，并把下一条待执行任务切换为 `T5000j3R Review：确认 higher-order / init 场景扩张没有把分析责任倒灌回 backend`。
