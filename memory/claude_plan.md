# Claude 执行计划

本文件记录本次调用的可审计执行计划与进度更新。内容为行动计划与决策摘要，不包含私密推理链。

## 范围

以 `TODO.md` 为任务顺序、需求、依赖、验证和完成状态的唯一来源，只完成第一个标题未带 `[DONE]` 的任务，提交后停止。

## 执行计划

1. 先读取 `TODO.md`，确认第一个未完成任务。
2. 只检查与当前任务直接相关的最近提交信息，不做开放式历史问题扫描。
3. 审计当前任务列出的代码、测试和文档位置。
4. 按任务要求实施最小且完整的修复，不引入 workaround、shim 或缩窄测试形态。
5. 如遇到阻塞当前任务的真实规格缺口，则在 `TODO.md` 插入最小 prerequisite，保持当前任务未完成，提交后停止。
6. 运行任务指定验证；若失败且与本任务相关，立即修复并重跑。
7. 将完成的任务标题改为 `[DONE]`，填写完成记录。
8. 仅在阶段级计划或依赖结构变化时更新 `PLAN.md`。
9. 提交本任务相关全部变更。
10. 不继续处理下一个任务。

## 进度日志

- 已在读取项目任务文件前初始化本次执行计划。
- 已确认第一个未完成任务为 `C2-T01D`：删除 LLVM / effect-lowered codegen 中的 CaptureBox lowering。
- 最新提交为 `[C2-T01C] Close MIR CaptureBox pass cleanup`，是本任务的已完成前置项，没有新增需要插入的未完成 blocker。
- 已审计任务列出的 LLVM / effect-lowered 文件。实际 CaptureBox lowering 分支已经不存在；剩余相关 source 命中是一处过期模块注释和 pipeline 负向测试断言字符串。
- 已更新 pipeline 断言：继续检查 emitted IR 不含 legacy mutable-capture allocation / descriptor marker，同时不在 source 中保留禁止的 CaptureBox spelling；并刷新 `value_args.rs` 模块注释。
- 已完成验证：`cargo build -p scoopc`、任务 grep gate、`closure_env_transport`、`composite_transport`、`llvm`、`cargo clippy -p scoopc --all-targets -- -D warnings` 均通过。
- 已将 `TODO.md` 中 `C2-T01D` 标记为 `[DONE]`，并记录改动范围、核心决策、验证结果和计划/设计闭合；`PLAN.md` 无需更新。
