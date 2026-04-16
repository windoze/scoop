# 执行计划摘要

## 约束说明

按要求先写入计划文件，再执行仓库检查与实现工作。这里记录的是可公开的步骤摘要，不包含私有推理细节。

## 当前计划

1. 检查最近一次提交信息，确认是否提到任何现存问题；如果有，优先修复这些问题。
2. 阅读 `TODO.md`，定位第一个未完成任务。
3. 判断该任务是否过大：
   - 如果可直接完成，进入实现。
   - 如果过大，先更新 `PLAN.md` 和 `TODO.md`，拆分为更小的子任务，并执行其中第一个。
4. 实现当前目标任务，必要时同时修复执行过程中暴露出的、阻碍规格正确性的前置问题。
5. 运行相关测试与质量检查，至少覆盖受影响范围；如有必要，补充或修复测试。
6. 更新文档状态：
   - 在 `TODO.md` 中标记当前任务完成，或在受阻时调整依赖顺序并保持为待办。
   - 在 `PLAN.md` 中更新进展、风险、依赖与后续顺序。
   - 在本文件中同步记录关键进展与计划变更。
7. 检查工作区变更，避免覆盖用户已有修改。
8. 使用清晰的提交信息提交本轮完成内容，然后停止，不继续下一个任务。

## 进度记录

- 已创建计划文件，准备开始检查最近提交与任务列表。
- 已确认最近一次提交只有标题 `[T3010b2b0a0] Front-load hidden ordinary-frame suspend blocker`，没有额外正文说明；当前无需先处理独立于 `TODO.md` 的提交遗留事项。
- 已定位本轮首个未完成任务为 `T3010b2b0a0`。
- 已阅读 `TODO.md` / `PLAN.md` 对 `T3010b2b0a0` 的定义，当前目标是补齐 ordinary callee 对 hidden-suspend boundary 的 propagation 合同，随后再由 `T3010b2b0a` 处理 caller-side 分类问题。
- 初步代码检查结果：
  - `codegen_object_property_access` 在调用 object init 后没有执行 `emit_ordinary_call_effect_propagation_check`，属于明确缺口。
  - `codegen_class_ctor_call` / `codegen_class_ctor_invoke_inner` 路径内没有统一的 ordinary-frame active 检查；property initializer / init block / ctor body 中若触发 hidden suspend，当前 ctor 可能继续执行并返回对象。
  - `emit_raise_runtime_error_variant` 当前只有 `as` cast 失败路径消费，builtin runtime raise 相关路径需要继续确认是否还有其它隐式边界未接 propagation。
- 下一步：
  1. 已完成：运行 `object_init_raise_try_catch_basic.scoop`、`class_init_raise_cleanup_property_init_gc_basic.scoop`，确认顶层路径已过，缺口只剩 helper 包裹 hidden-suspend boundary。
  2. 已完成：新增 `object_property_init_raise_helper_try_catch_basic.scoop`，成功复现修复前会打印 `helper_unreachable`。
  3. 已完成：在 `codegen_object_property_access` 的 object init 调用后接入 `emit_ordinary_call_effect_propagation_check`；新增 `class_init_hidden_raise_helper_try_catch_basic.scoop` 进一步覆盖 class ctor property initializer 路径。
  4. 已完成：新旧相关 fixture、`cargo test --all`、`cargo clippy --all-targets -- -D warnings` 均已通过。
  5. 已确认：`cargo run -p scoop --features llvm -- test` 的首个失败点仍是已知后续 blocker `effect_escape_continuation_finally_arm_raise.scoop`（`T3010b2b1`），未出现更早 hidden-suspend helper 回归。
  6. 下一步：整理 `TODO.md` / `PLAN.md` / 本文件的完成记录，检查 diff，然后提交 `T3010b2b0a0` 并停止。
