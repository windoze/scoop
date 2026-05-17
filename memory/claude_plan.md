# 执行计划

说明：本文件记录本次调用的可审计执行计划、进度和关键决策依据；不会记录私有推理链。

## 当前目标

完成 `TODO.md` 中第一个标题未以 `[DONE]` 标记的任务，完成后更新任务记录、运行相关验证、提交 Git commit，并停止。

## 步骤计划

1. 读取 `TODO.md`，按文件顺序识别第一个未完成任务；只以标题是否带 `[DONE]` 判断完成状态。
2. 检查最近提交信息是否明确提到与该任务直接相关的未完成问题；如有，将其纳入当前任务或作为前置依赖记录到 `TODO.md`。
3. 阅读当前任务所需的上下文文件，包括任务描述中提到的代码、测试、规范或计划文件；避免无关历史问题扫查。
4. 判断任务是否可直接完成；如发现阻塞当前任务的真实缺陷或缺失特性，按要求在 `TODO.md` 中插入最小前置任务并停止。
5. 对当前任务做最小正确实现，避免 workaround、fixture-only hack 或缩小任务语义。
6. 运行任务要求的验证命令和必要的相关测试；若失败，定位并修复与当前任务相关的问题后重测。
7. 更新 `TODO.md`：在当前任务标题前加 `[DONE]`，并补全完成记录、验证命令和结果；仅在阶段级计划变化时才更新 `PLAN.md`。
8. 检查工作区变更，确保包含本次任务需要提交的所有文件，且不回退用户已有变更。
9. 使用符合仓库风格的提交信息创建 Git commit。
10. 停止，不继续处理下一个任务。

## 进度日志

- 已创建初始执行计划，下一步读取 `TODO.md` 识别第一个未完成任务。
- 已识别第一个未完成任务：`P4-T01`（数组字面量 HIR desugar 切换到 `mutableArrayNew + push + freeze` 路径）。
- 最新提交为 `[P3-T03] Add MutableArray sysroot wrappers`，未发现提交信息中有直接阻塞 `P4-T01` 的未完成事项。
- 下一步读取数组字面量 lowering 实现、P3-T03 wrapper 定义和相关测试结构，随后进行最小正确实现。
- 已确认旧数组字面量和 vararg 合成数组都在 HIR 中直接生成 `__scoop_array_builder_*` 调用。
- 实施方案：新增共享 helper 生成 `mutableArrayNew(capacity=N)`、逐元素 `push`、按目标可选 `freeze`；同时扩展 HIR 泛型实例发现，使返回类型可为 `mutableArrayNew<T>` 这类合成调用提供 `T` 推断。
- 已完成核心 lowering 修改、HIR owner 测试和空数组 run-pass fixture；目标测试 `cargo test -p scoopc array_literal_desugar -- --nocapture` 通过。
- 全量 fixture 首次运行在 484/1338 处超时；已发现需更新的 snapshot golden：`hir/array_lit_lowering.hir`、`hir/lowered_call_args.hir`、`mir_lowered/aggregate_transport.mir`。
- 已更新上述 3 个 snapshot golden 并单独验证通过；5 个含数组字面量的 run-pass fixture 抽样均通过。
- 全量 `cargo run -p scoop -- test` 已在更长超时下完成，通过 1375 项检查。
- `cargo test --all --all-targets` 首次发现若干单元测试仍断言旧 builder 形态，并暴露 MIR array transport metadata 未覆盖 `scoop.core.push/freeze`。已更新 metadata 与相关测试断言，下一步重新运行格式化、clippy 和全量 Rust 测试。
- 最终验证已完成：`cargo test -p scoopc array_literal_desugar -- --nocapture`、`cargo test --all --all-targets`、`cargo clippy --all-targets -- -D warnings`、`cargo run -p scoop -- test` 均通过。
- 已将 `P4-T01` 在 `TODO.md` 和 `TODO-2.md` 标记为 `[DONE]` 并补全完成记录；下一步检查工作区并提交本任务改动。
