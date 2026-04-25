# 本次执行计划

## 说明

按要求，先记录可审查的执行计划与决策摘要。这里记录的是执行步骤、检查项与后续更新，不包含原始逐字思维流。

## 初始计划

1. 查看最新一次 Git 提交信息，确认是否提到任何已知问题、回归、临时修复或待补漏洞。
2. 如果最新提交提到了现存问题，优先定位并修复该问题，再继续后续步骤。
3. 读取 `TODO.md`，识别第一个未完成任务。
4. 评估该任务规模与依赖：
   - 如果可以在本轮完整实现，则直接执行。
   - 如果任务过大或存在前置缺口，则把任务拆分为更小的子任务，并同步更新 `PLAN.md` 与 `TODO.md`，本轮只执行拆分后的第一个子任务。
5. 在执行过程中检查是否存在任何阻塞当前任务的既有问题、规范不匹配、回归、缺失特性或测试失败：
   - 若发现，立即把该问题视为当前优先事项。
   - 能直接修复则先修复。
   - 若无法在本轮直接修复，则将其作为前置任务插入 `TODO.md` 到当前任务之前，并更新 `PLAN.md` 说明依赖关系，然后停止。
6. 实现当前目标任务，必要时补充或调整代码注释、文档与模块结构。
7. 运行相关验证：
   - 最少运行与改动直接相关的测试。
   - 若改动影响较广，再运行更高层级测试。
   - 按要求检查无警告构建与 lint，优先执行 `cargo clippy --all-targets -- -D warnings`（若时间和作用域允许则完整执行）。
8. 更新进度文档：
   - 在 `TODO.md` 中标记该任务完成，或在阻塞场景下正确重排任务顺序。
   - 在 `PLAN.md` 中更新状态、依赖与后续说明。
   - 回写本文件，记录关键发现、计划变化和已完成步骤。
9. 使用清晰的 Git 提交信息提交本轮结果。
10. 停止，不继续做下一个任务。

## 执行时的记录要求

- 每完成一个关键步骤后，更新本文件。
- 如果计划发生变化，补充“变更原因”和“新计划”。
- 若遇到阻塞，明确记录阻塞原因、对应新增前置任务以及停止点。

## 当前状态

- 已完成：创建计划文件并写入初始计划。
- 已完成：检查最新提交提交信息，确认最新提交 `[T5000b4a] Share codegen layout cache context` 本身没有在提交说明中声明待先修复问题。
- 已完成：读取 `TODO.md` / `PLAN.md`，确认首个未完成任务为 `T5000b4aR Review：确认共享 cache 已脱离 MainCodegen 的函数级状态`。
- 已完成：第一轮代码面复核。
  - 已检查 `crates/scoopc/src/llvm/codegen/mod.rs`、`layout.rs`、`ty.rs`、`effect/state_machine_plan.rs`、`emit.rs`；
  - 已确认 `known_fun_call_suspend_cache`、`type_layout_cache`、`option_niche_cache`、`enum_cg_layout_cache`、`class_init_layout_cache`、`pack_field_indices` 现统一收口到 `CompilationUnitCodegenCx.shared_caches`；
  - 已确认 `CompilationUnitCodegenCx::new(...)` 仍只有一个实现构造入口，`fresh_main_codegen()` / `fresh_child_codegen()` 统一复用同一编译单元共享 cache；
  - 暂未发现仍由每个 `MainCodegen` 单独维护上述 cache 的残留路径。
- 已完成：运行验证命令，确认 review 结论与测试结果一致。
  - `cargo test -p scoopc llvm::` 通过；
  - `cargo test --all` 通过；
  - `cargo clippy --all-targets -- -D warnings` 通过。
- 进行中：回写 `TODO.md` / `PLAN.md` / 本文件，并准备提交 `T5000b4aR` 完成记录。
