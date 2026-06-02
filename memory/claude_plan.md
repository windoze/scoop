# 当前执行计划

说明：此文件记录可审阅的执行计划和进度，不包含隐藏推理过程。

## 初始计划

1. 读取 `TODO.md`，按文件顺序定位第一个标题未以 `[DONE]` 标记的任务。
2. 检查该任务的要求、依赖、验证命令和完成记录，并查看最新提交是否明确提到与该任务直接相关的未完成事项。
3. 读取任务相关代码、测试和文档，只收集完成当前任务所需的上下文。
4. 如果发现当前任务被具体缺失功能、规格不匹配或未计划的失败测试阻塞，则在 `TODO.md` 中加入最小必要的前置任务，提交后停止。
5. 否则实现当前任务，优先做最小且规格正确的修改。
6. 运行格式化、lint、相关测试，以及任务要求的完整验证；发现失败时修复或按规则把未计划失败加入 `TODO.md`。
7. 将当前任务标题标记为 `[DONE]`，更新完成记录；仅在阶段计划确有变化时更新 `PLAN.md`。
8. 检查 `git status` 和变更内容，提交本次任务相关改动，然后停止，不继续下一个任务。

## 当前状态

- 已读取 `TODO.md` 与 `TODO-3.md`。
- 第一个未完成任务是 `TODO-3.md` 的 `T3-04R：Review T3-04`，依赖 `T3-04B` 已标记完成。
- 已检查最新提交和工作树：最新提交为 `[T3-04B] Close residual fact fallback gaps`，未发现提交信息中直接声明的新未完成事项；工作树中既有未跟踪 `FACT_REFACTOR.md`，本次未改动。
- 已完成 T3-04R 定向审查，确认 `T3-04B` 后仍有阻塞级残留，不能完成 review。

## T3-04R 审查计划

1. 检查最新提交是否留下与 `T3-04R` 直接相关的未完成事项，并确认当前未提交改动范围。
2. 搜索 P4/P5/P6 生产路径中的 source-span side table、FQN/string/root/readable-path fallback、intrinsic fallback、dispatch side-table 恢复、`unpublished(...)` / `missing-owner` / signature fallback 残留。
3. 检查 verifier 与 dependency gate 是否覆盖上述边界，并阅读关键实现路径确认不是仅靠命名规避。
4. 如发现阻塞缺口，优先修复；若无法在本任务内正确完成，则在 `TODO-3.md` 加入最小前置任务并提交后停止。
5. 如未发现阻塞问题，按要求运行验证，更新 `TODO-3.md` / `TODO.md` 状态与完成记录，提交后停止。

## 审查发现

1. `lir_facts_builder` 与 LLVM 生产路径仍可通过 `named_intrinsic_entry_name_for_root`、source signature root 扫描或 source-body direct-call root 反推生成/消费 named intrinsic metadata。
2. reflection intrinsic 的类型实参仍可在 P6 通过 `current_source_slice(span)` 解析 `<T>` 并按 display 文本查找 `TypeId`。
3. declaration-only/native/extern ABI symbol 仍存在 root 扫描、declaration key 合成、`AbiMangler.fun_symbol(...)` 或 raw FQN symbol 补洞路径。
4. LIR verifier 仍因 `body#declaration` 或 `symbol.callable.is_none()` 放行部分未发布 target，并且 `DynamicFallback` 拒绝范围不完整。
5. dependency gate 尚未覆盖当前等价残留，例如 `named_intrinsic_entry_name_for_root`、reflection source-slice 解析、declaration ABI 合成、verifier declaration escape、部分 generic/overload 字符串解析与 dispatch/text 恢复路径。

## 计划变更

- 已在 `TODO-3.md` 插入最小前置任务 `T3-04C`，将 `T3-04R` 依赖更新为 `T3-04C`，并在 `TODO.md` 中把当前活跃任务改为 `T3-04C`。
- 本次仅修改任务/进度文档，不修改代码；无需运行完整验证套件。
