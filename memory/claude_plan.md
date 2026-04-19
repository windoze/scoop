# 执行计划记录

说明：按要求记录本轮执行计划、关键步骤进展与必要调整。这里保留的是可审阅的执行摘要，不包含逐字内部推理。

## 本轮初始计划（2026-04-19）

1. 检查最新一次 Git 提交，确认提交说明或提交内容里是否提到必须先修复的既有问题。
2. 阅读 `TODO.md`，定位第一个未完成任务。
3. 阅读 `PLAN.md`，确认该任务的上下文、依赖和预期交付物。
4. 判断该任务是否能在本轮完整落地；如果过大或发现前置缺口，则先拆分任务并更新 `TODO.md` / `PLAN.md`。
5. 实现当前第一个未完成任务，严格避免 workaround；若遇到规范缺口或真实 blocker，则转化为更前置的任务并重排。
6. 运行与改动直接相关的验证，包括定向测试、必要的全量测试、`cargo fmt`、`cargo clippy --all-targets -- -D warnings`。
7. 更新 `TODO.md`、`PLAN.md` 和本文件，记录完成情况或阻塞调整。
8. 检查工作区差异，提交一次清晰的 Git commit，然后停止，不继续后续任务。

## 本轮进展

- 已写入本轮执行计划。
- 已检查最新提交、`TODO.md` 与 `PLAN.md`，确认本轮起始任务是复审项 `T4005SR`。
- 已审阅 `T4005S` 的实现，确认它补齐了 `typecheck -> HIR lowering -> LLVM bind_when_pat` 三层上的 `when` binder 类型回写与恢复。
- 已用扩展 probe 继续复审 callable-value 主线，结果如下：
  - `/tmp/t4005sr_local_pattern_function_probe.scoop`：通过，输出 `12`；说明局部 destructuring 函数值调用正常。
  - `/tmp/t4005sr_when_receiver_function_probe.scoop`：通过，输出 `14`；说明 `when` binder 上的 receiver function value 调用正常。
  - `/tmp/t4005sr_top_level_named_function_value_probe.scoop`：失败，typecheck 报 `callee_not_callable`。
  - `/tmp/t4005sr_top_level_pattern_function_probe.scoop`：失败，typecheck 报 `callee_not_callable`。
  - `/tmp/t4005sr_top_level_funptr_probe.scoop`：失败，LLVM codegen 报 `call callee type`。
- 已据此定位新的更前置 blocker：问题不再是局部 / `when` pattern binder，而是“顶层 callable value（含顶层 pattern binder / `FunPtr`）调用语义”尚未接入统一主线。
- 已定位到两处直接根因：
  - `crates/scoopc/src/typecheck/expr/call.rs` 的 `infer_call_expr_type` 只对顶层 `FunPtr` 做了 direct-call 特判，普通顶层函数值在未命中 `top_level_funs` 时会直接报 `CalleeNotCallable`。
  - `crates/scoopc/src/llvm/codegen/mod.rs` 的 `codegen_call` 对 `ValueRef::TopLevel` 仍一律按“普通顶层函数名”处理，没有读取顶层 immutable value 的 callable metadata，因此顶层 `FunPtr` call 落入 `call callee type`。
- 结论：`T4005SR` 不能在本轮标记完成，必须先在它之前插入新的实现任务，收口顶层 callable value 主线，再回到复审。
- 下一步：更新 `TODO.md` / `PLAN.md`，新增前置任务 `T4005T`，把 `T4005SR` 顺延到其后；随后提交并停止。
