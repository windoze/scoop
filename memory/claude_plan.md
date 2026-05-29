# 当前执行计划

## 目标
- 按照 `TODO.md` 的权威顺序，只完成第一个未以 `[DONE]` 标记的任务。
- 完成实现、验证、任务记录更新和 Git 提交后停止，不进入下一项任务。

## 执行步骤
1. 读取 `TODO.md`，确认第一个未完成任务的编号、范围、依赖和验证要求。
2. 检查最新提交摘要，仅在它明确提到与当前任务直接相关的未完成问题时纳入当前任务或作为前置任务处理。
3. 阅读当前任务涉及的代码、测试、规格或夹具，确定最小正确实现路径。
4. 如发现阻塞当前任务的缺失特性、规格不匹配或未安排的测试失败，优先修复；若无法在当前任务内正确修复，则把最小前置任务插入 `TODO.md`、保持当前任务未完成、提交并停止。
5. 实施当前任务所需代码或文档改动，避免绕过规格或夹具特判。
6. 按要求运行格式化、lint、测试和夹具验证：先 `cargo fmt`，再 `cargo clippy --all-targets -- -D warnings`，再按任务需要运行完整 Rust 测试和夹具套件。
7. 若发现未安排的失败测试或夹具，修复或在 `TODO.md` 中安排到当前任务完成前。
8. 将当前任务标题加上 `[DONE]`，更新完成记录；仅在阶段级计划变化时更新 `PLAN.md`。
9. 检查工作区差异，提交本次任务相关全部改动，提交信息使用任务编号和简要说明。
10. 停止，不处理下一个任务。

## 当前状态
- 已读取 `TODO.md` / `TODO-5.md`，第一个未完成任务是 `P5-T04：贯通 selected callable identity，修复 concrete / arity / generic-concrete codegen bug`。
- 已检查最新提交摘要，未发现直接声明 P5-T04 前置阻塞的未完成问题。
- 已开始实现：`TopLevelFunCallBinding` 将携带选中 overload 的 `param_tys` / `return_ty`，并通过 HIR facts 传到 MIR lowering；MIR lowering 将优先使用该选中签名，而不是按 bare FQN 查询参数表。
- 已修复 materializer：direct binding 精确 span 优先于重叠兜底；selected non-generic binding 优先于 generic instance 推断；generic-family 判断改为基于模板身份而不是 bare FQN。
- 已启用并通过三个 P5-T04 targeted run-pass fixtures：`overload_concrete_bug.scoop`、`overload_arity_bug.scoop`、`overload_gvc_ok.scoop`。
- 已处理验证中暴露的 TypeId remap 回归：`TopLevelFunCallBinding.param_tys` / `return_ty` 在 HIR compilation-unit lowering 中也会从 typecheck store remap 到 HIR store。
- 完整 fixture 首轮暴露 4 个直接相关问题：两个 MIR golden 因 selected call signature 更精确而变化，两个 run-pass 因非泛型 direct call 结果 transport 未按选中 callee 返回类型修复而失败。
- 已修复：非泛型 materialized direct call 也记录 result type 供 rewrite repair；MIR call result local 默认使用 HIR 表达式类型，仅 `Any` 时才回退 callee contract；已重新生成相关 MIR golden。
- 已通过最终验证：`cargo fmt`、`cargo clippy --all-targets -- -D warnings`、`cargo test --all --all-targets`、`python3 tools/spec_fixtures.py check`、`python3 tools/run_fixtures.py`。
- 已更新 `TODO.md` 与 `TODO-5.md`，将 `P5-T04` 标记为 `[DONE]` 并填写完成记录。
- 已检查工作区差异；`REFLECTION.md` 是既有未跟踪无关文件，不纳入提交。
- 下一步：提交本任务相关改动并停止。
