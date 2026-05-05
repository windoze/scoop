# 当前执行计划

## 目标
- 按 `TODO.md` 索引和对应 `TODO-Px.md` 详情文件，找到第一个标题未带 `[DONE]` 的详细任务。
- 完成且只完成该任务；如果遇到阻塞，则补充最小必要前置任务、同步索引、提交后停止。

## 执行步骤
1. 读取 `TODO.md`，确认任务索引顺序和引用的详细任务文件。
2. 按索引顺序读取对应 `TODO-Px.md` 文件，定位第一个未完成详细任务。
3. 检查最近提交是否明确提到与该任务直接相关的未完成问题；若相关，将其纳入当前任务或作为前置任务记录。
4. 阅读当前任务的完整要求、依赖、验证命令和完成记录。
5. 在不绕开规范要求的前提下实施最小正确修改。
6. 运行相关测试和必要的质量检查；若失败，修复后重跑。
7. 更新当前任务所在 `TODO-Px.md`：在标题前加 `[DONE]` 并补全完成记录。
8. 若索引任务标题或完成状态变化，同步更新 `TODO.md`。
9. 仅当阶段级计划、依赖或完成标准变化时更新 `PLAN.md`。
10. 提交所有与本任务相关的变更，提交信息使用任务编号和简短说明。
11. 停止，不继续处理下一个任务。

## 约束
- 不拆分任务，除非存在无法正确执行的具体前置阻塞。
- 不使用 workaround、夹具特化 hack 或削弱测试形状。
- 不回滚或覆盖非本次修改的用户变更。
- 后续关键步骤完成或计划变化时，继续更新本文件。

## 当前状态
- 已读取 `TODO.md` 与 `TODO-P7.md`。
- 首个未完成详细任务是 `P7-T02Z：闭合 P7-T03 剩余默认 run-pass refactor 阻塞，避免 full regression 依赖 legacy 或 fixture 降级`。
- 最新提交 `fde2c32e [P7-T02Za] Fix dynamic dispatch ABI schema drift` 是该任务的直接前置修复；当前需要在此基础上继续清理 `P7-T02Z` 的剩余 run-pass 阻塞。
- 已确认首个失败为 `effect_escape_continuation_finally_arm_raise.scoop`，frontend prepare 在 finally cleanup 的 `Complete(Unit)` 路径尝试把 `Int` payload 强制为 `Unit`。
- 已做窄修复：`lower_completion_payload_as` 在目标 completion 类型为 `Unit` 时直接返回 elided payload，避免把无 runtime payload 的 `Unit` Complete 误当作值转换。
- `effect_escape_continuation_finally_arm_raise.scoop` 已通过。
- 第二个阻塞 `effect_handle_hidden_suspend_local_closure_helper_basic.scoop` 已定位为局部 closure 未继承静态 hidden-suspend callee 的 effect row，导致外层 handle 看不到 `Raise<RuntimeError>`；已在 facts seed 阶段传播静态 direct/closure callee effect row，并修正 ABI 校验对 `KnownInstance` closure boundary 的 dynamic-invoke 要求。
- `effect_handle_hidden_suspend_local_closure_helper_basic.scoop` 已通过。
- 第三个阻塞 `effect_handle_return_from_function_any_boxing.scoop` 已定位为 `NoOutward` plain local-effect frame 在后续直线 tail / GC 前仍被 root，导致 GC heap object count 多 1；已在无 reentry 且后续 tail 不再经过 boundary/handle dispatch 时清理临时 frame root。
- `effect_handle_return_from_function_any_boxing.scoop` 已通过。
- 第四个阻塞 `effect_handle_yield_and_step_finally.scoop` 已定位为 shared surface wrapper projection 在两个 handle arm resume boundary 之间只因 payload source span 不同被误判冲突；已让 P5 handoff 与 P6 ABI 校验的 projection shape 比较忽略 span，只比较 source type/value identity。
- `effect_handle_yield_and_step_finally.scoop` 已通过。
- 第五个阻塞 `effect_indirect_perform_materialized_mir_closure_basic.scoop` 已定位为 closure carrier/direct-entry args ABI 对 lambda env tuple 的组件边界不一致；已改为把 closure env 作为完整 tuple 组件传递和绑定，而不是把捕获字段展开成 direct args 组件。
- `effect_indirect_perform_materialized_mir_closure_basic.scoop` 已通过。
- 第六个阻塞 `effect_indirect_perform_nonresuming_closure.scoop` 已定位为 known callee invoke tuple 使用实参 closure 的 bottom-return 函数类型，以及无显式参数 lambda 需要 env tuple 展开 ABI；已改为 known-instance call facts 使用 callee authoritative invoke tuple，允许函数值 `Nothing` 返回协变到期望返回类型，并按 lambda direct-entry 形状区分 env 展开/整 tuple 传递。
- `effect_indirect_perform_nonresuming_closure.scoop` 已通过。
- 后续阻塞 `effect_indirect_perform_nonresuming_function_value_higher_order_when_direct.scoop` 已收敛为独立 handoff 缺口：`choose(mode)()` 返回函数值后调用已能生成 MIR `FunValue`，但 solver/late-lowered 仍把 `drive` 内部 handle 应消费的 `Ask.ask` 泄漏到 `main`。
- 已新增 prerequisite `P7-T02Zb` 并同步 `TODO.md`；当前按阻塞处理停止，不继续推进 `P7-T02Z`。
- 已运行 `cargo fmt --all`。
- 已通过定向 fixtures：`effect_escape_continuation_finally_arm_raise.scoop`、`effect_handle_hidden_suspend_local_closure_helper_basic.scoop`、`effect_handle_return_from_function_any_boxing.scoop`、`effect_handle_yield_and_step_finally.scoop`、`effect_indirect_perform_materialized_mir_closure_basic.scoop`、`effect_indirect_perform_nonresuming_closure.scoop`。
- 已通过 `cargo check -p scoopc` 与 `cargo clippy --all-targets -- -D warnings`。
- 下一步检查 git diff/status，并提交当前阻塞拆分与已完成通用修复。
