# Claude 执行计划

本文件记录本次调用的可审计执行计划与进度更新。内容为行动计划与决策摘要，不包含私密推理链。

## 范围

以 `TODO.md` 为任务顺序、需求、依赖、验证和完成状态的唯一来源，只完成第一个标题未带 `[DONE]` 的任务，提交后停止。

## 执行计划

1. 先读取 `TODO.md`，确认第一个未完成任务。
2. 只检查与当前任务直接相关的最近提交信息，不做开放式历史问题扫描。
3. 审计当前任务列出的代码、测试和 fixture 范围。
4. 如 source-level Rust tests/helper 仍期待 CaptureBox，按任务要求更新为普通 local / closure env metadata 断言。
5. 运行任务指定验证；若失败且与本任务相关，立即修复并重跑。
6. 将剩余 fixture 旧 expect 明确归入后续 C4-T01A 刷新清单，不在本任务手改大量 snapshot。
7. 将完成的任务标题改为 `[DONE]`，填写完成记录。
8. 仅在阶段级计划或依赖结构变化时更新 `PLAN.md`。
9. 提交本任务相关全部变更。
10. 不继续处理下一个任务。

## 进度日志

- 已在读取项目任务文件前初始化本次执行计划。
- 已确认第一个未完成任务为 `C2-T01E`：收口 CaptureBox 删除后的全仓审计。
- 最新提交为 `9ac449f6 [C2-T01D] Remove LLVM CaptureBox references`，是本任务直接前置项，没有新增需要插入的 blocker。
- 已审计 `crates/scoopc/src` 与 `sysroot`，没有剩余 CaptureBox source references。
- 已审计 `crates/scoopc/src/pipeline/mir_stage.rs`，相关 Rust tests 已验证普通 captured local assignment、closure env 与 mutable capture metadata，不再期待 CaptureBox。
- 已审计 `tests/fixtures`，旧 expect 仅剩在 `closure_capture_var.*`、`aggregate_transport.*`、`assignment_places.*` 三组，已在 `TODO.md` 中明确由 C4-T01A 刷新。
- 已完成验证：`cargo build -p scoopc`、`cargo test -p scoopc mir_place_contract -- --nocapture`、`cargo test -p scoopc aggregate_transport -- --nocapture`、source/sysroot CaptureBox grep、全仓 CaptureBox grep、`cargo clippy -p scoopc --all-targets -- -D warnings` 均通过或符合预期。
- 已将 `TODO.md` 中 `C2-T01E` 标记为 `[DONE]`，并记录改动范围、核心决策、验证结果和计划/设计闭合；`PLAN.md` 无需更新。
