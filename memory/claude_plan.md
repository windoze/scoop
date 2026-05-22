# 执行计划

## 当前状态

- 已按 `TODO.md` 确认并完成当前任务 `P6-T02`；下一步仅剩提交本次改动。
- 本文件用于记录可审计的计划、关键步骤完成情况和后续计划调整。

## 初始计划

1. 读取 `TODO.md`，按标题是否带 `[DONE]` 识别第一个未完成任务。
2. 检查最新提交信息；只有当其明确提到与当前任务直接相关的未完成问题时，才纳入当前任务或作为前置项写入 `TODO.md`。
3. 阅读当前任务涉及的代码、测试、规范或夹具，确认任务要求和验证命令。
4. 如任务可直接完成，则最小化实现变更并补充或更新相关测试。
5. 运行当前任务要求的验证；若发现与当前任务直接相关的阻塞缺陷，优先修复，或将最小前置任务插入 `TODO.md` 后停止。
6. 完成后在 `TODO.md` 中给任务标题添加 `[DONE]` 并更新完成记录；仅在阶段级计划变化时更新 `PLAN.md`。
7. 检查工作区差异，提交本次任务相关全部改动，然后停止，不继续下一个任务。

## 进度记录

- 2026-05-22: 已创建初始计划；等待读取 `TODO.md` 后确定第一个未完成任务。
- 2026-05-22: 已读取 `TODO.md` / `TODO-6.md`；第一个未完成任务是 `P6-T02：实现 per-cone eager top-level init 与 final entry order`。
- 2026-05-22: 最新提交为 `[P6-T01R] Review global init LIR facts`，属于当前任务的直接前置，不需要新增前置问题。

## P6-T02 执行计划

1. 审查现有 `cone_init`、`immut_value`、`globals`、`emit`、effect-lowered/LIR facts builder 相关代码，确认当前 lazy top-level `val`、top-level `var` initializer 与 final entry 调用路径。
2. 用 P6-T01 已发布的 `LirFacts` global init/storage/final-entry contract 驱动 cone init routine，不让 LLVM 从 HIR/raw MIR 回推 init order。
3. 修改 top-level `val` 访问路径为读取 eager 初始化后的 backing storage，删除或隔离 first-access lazy once 初始化路径。
4. 将 annotated top-level `var` initializer 纳入 per-cone init routine；保留静态物理化仅作为不改变 eager 语义的优化。
5. 确认 final system entry 在 runtime init 后、用户 `main` body 前按 linked source-cone DAG / LIR facts final-entry order 调用 cone init routines。
6. 增补 global init fixture，覆盖 `val` / annotated `var` 的 main 前 eager 初始化和跨 cone/order 行为。
7. 运行任务列出的验证命令，必要时补充更窄的单测；通过后更新 `TODO.md` 与 `TODO-6.md` completion record。
8. 检查 diff/status，提交本任务相关全部改动后停止。

## P6-T02 当前发现

- 当前 `cone_init.rs` 只生成空壳 routine，未执行 roots。
- 当前 `CompilationUnitCodegenCx::cone_init_routine_plans` 按 source map / HIR side table 顺序临时收集 roots，未消费 P6-T01 发布的 LIR facts final-entry contract。
- 当前 top-level `val` 普通访问会调用 `ensure_top_level_immutable_value_init_function_defined`，仍是 lazy first-access 初始化路径；递归初始化错误依赖 initialized check，因此访问路径应保留检查但不能再触发 init。
- 当前 top-level `var` global declaration 会尝试把 initializer 编译成 LLVM static initializer；P6-T02 需要改为由 cone init routine 运行 initializer 并写入 backing storage。

## P6-T02 实施进展

- 已将 LLVM cone init plan 改为消费 `LirFacts.global_init.final_entry_order`，不再从 HIR/source map 临时推导 routine/root 顺序。
- 已在 cone init routine 中执行 top-level `val` eager init helper，并执行 annotated top-level `var` initializer 后写入 backing storage。
- 已将 top-level `val` 普通访问改为只检查并读取 eager 初始化后的 storage，不再 first-access 调用 init helper。
- 已让 top-level `var` 全局 storage 默认零初始化，runtime initializer 由 cone init routine 负责。
- 已补充 LLVM IR 单测和 `tests/fixtures/run-pass/global_init` 运行夹具；下一步运行格式化与 P6-T02 验证。
- 验证进展：`cargo fmt`、`cargo test -p scoopc global_init`、`cargo test -p scoopc --no-default-features global_init`、`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/global_init` 已通过。
- 完整 `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass` 仍有 7 个既有 run-pass 失败，失败集中在未解析 generic `scoop.core.println` 或 `scoop.runtime.test.*` import，和 P6-T02 global init 变更无直接关系；P6-T02 相关的 top-level val golden 已按 eager init 语义更新并单独通过。
- 已更新 `TODO.md` / `TODO-6.md`，将 `P6-T02` 标为 `[DONE]` 并写入完成记录；下一步检查 diff/status 后提交。
