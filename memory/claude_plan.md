# 执行计划

## 范围

- 以 `TODO.md` 为唯一任务顺序来源，找出第一个标题未带 `[DONE]` 的任务。
- 只完成该一个任务；完成后更新 `TODO.md`、验证、提交并停止。
- 不做开放式历史问题扫描；仅处理会阻塞当前任务或直接破坏当前任务要求的问题。

## 步骤

1. 读取 `TODO.md`，识别第一个未完成任务及其要求、依赖和验证方式。
2. 如需要，检查最近提交是否提到与该任务直接相关的未完成问题。
3. 阅读实现相关文件，确定最小正确改动。
4. 实施当前任务；如发现必须先修复的具体前置缺口，更新 `TODO.md` 插入最小前置任务并停止。
5. 运行任务要求的验证；必要时修复失败并重新验证。
6. 将当前任务标题加 `[DONE]`，更新完成记录；只有阶段级计划变化时才更新 `PLAN.md`。
7. 提交本次任务涉及的所有变更，提交信息包含任务编号和简短说明。
8. 停止，不继续下一个任务。

## 当前状态

- 已读取 `TODO.md`，第一个未完成任务为 `MIR-T09：收口 runtime value primitives 的 MIR 表达`。
- 最新提交为 `[MIR-T08R] Review effect site contract`，未发现直接要求先处理的 `MIR-T09` blocker。
- 下一步检查 runtime typecheck/cast/not-null/pattern lowering 相关实现与现有测试，随后实施最小完整改动。
- 已完成 MIR runtime value primitive 元数据的初版实现：`Rvalue::TypeCheck`、`Rvalue::Cast`、`Pattern::Is` 携带 descriptor/static-fold/parameterized contract；materialized verifier/rewrite 同步处理这些类型引用。
- 已新增 `refactor_mir_value_primitives` 单测与 `mir_refactor/runtime_typecheck_cast.scoop`、`not_null_assert.scoop`、`pattern_is_type.scoop` fixtures。
- 验证已通过：`cargo test -p scoopc --no-default-features refactor_mir_value_primitives`、三个新 fixture 的 `dump-mir --effect-pipeline refactor`、`refactor_materialized_mir`、`refactor_mir_no_todo`、`refactor_mir_placeholder_inventory`、`cargo clippy -p scoopc --no-default-features --all-targets -- -D warnings`。
- 下一步更新 `TODO.md` 将 `MIR-T09` 标记为 `[DONE]` 并填写完成记录，然后提交。

## MIR-T09 执行要点

1. 定位现有 HIR/typecheck/MIR 中 `is`、`!is`、`as`、`as?`、`!!`、pattern `is Type`、function type cast 的表示与降级路径。
2. 为 refactor MIR 增补或完善 runtime value primitive metadata，确保 no-placeholder 且后续 codegen 可辨识 descriptor/failure/result contract。
3. 对当前不支持的 function type runtime cast 增加 frontend/typecheck diagnostic，避免进入 MIR。
4. 新增/更新 `refactor_mir_value_primitives` 测试与 `mir_refactor` fixtures。
5. 运行 `TODO.md` 指定验证与必要的 targeted regression/lint。
6. 成功后标记 `MIR-T09` 为 `[DONE]`，填写完成记录并提交。
