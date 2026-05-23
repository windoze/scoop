执行计划

1. 读取 `TODO.md`，按文件顺序找到第一个标题未带 `[DONE]` 的任务，并核对该任务的依赖、验证要求和完成记录格式。
2. 查看最近提交信息；只有当最新提交明确提到与当前任务直接相关的未完成问题时，才把它纳入当前任务或作为前置项记录到 `TODO.md`。
3. 收集当前任务所需的最小代码上下文，避免做开放式历史问题扫描。
4. 按当前任务要求实现变更；如果发现阻塞当前任务的语言特性缺口、规范不一致或测试失败，优先修复，或把最小前置任务插入 `TODO.md` 后停止。
5. 运行与任务直接相关的测试；必要时再运行更广范围测试，确保没有未排期的失败测试或夹具。
6. 更新 `TODO.md`：只有任务完整实现并通过要求验证后，才在任务标题前加 `[DONE]` 并填写完成记录；仅当阶段级计划变化时才更新 `PLAN.md`。
7. 检查工作区差异，确认只包含本任务相关变更；按要求提交所有本次任务需要提交的未提交文件。
8. 提交后停止，不继续处理下一个任务。

当前状态

- 已读取 `TODO.md`，第一个未完成任务为 `P9-T06`：抽出 `scoopc_effect_facts_stage` 与 `scoopc_lir` crate。
- 已读取 `TODO-7.md` 中 `P9-T06` 的完整要求。最新提交为 `[P9-T06-a] Narrow LIR source payload boundary`，是已完成直接前置，没有额外未完成事项需要先插入。
- 已收集 `effect/`、`effect_facts/builder.rs`、`effect_lowered/`、pipeline stage、Cargo workspace 与 dependency gate 的最小上下文。
- 发现阻塞 `P9-T06` 的直接依赖边界：LLVM production code 仍使用 `crate::effect::{analysis,state_machine}` 中的 ordinary-callee suspend analysis，而 `P9-T06` 要求同时把 `effect/` 移入 `scoopc_effect_facts_stage` 并让 `scoopc_codegen_llvm` direct 依赖 `scoopc_lir`，不能继续依赖 effect stage 或 `scoopc` façade。
- 已在 `TODO.md` / `TODO-7.md` 插入最小前置任务 `P9-T06-b`，并把 `P9-T06` 依赖改为 `P9-T06-b`。下一步只验证文档改动并提交，然后停止。
