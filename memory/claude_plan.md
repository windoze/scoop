# 当前执行计划

## 约束

- 以 `TODO.md` 为唯一任务顺序与完成状态来源。
- 只完成第一个标题未带 `[DONE]` 的任务，完成后停止。
- 不做开放式历史问题扫查；只处理当前任务相关或测试暴露且未被明确排期的问题。
- 若遇到阻塞当前任务的缺失特性或规格不一致，优先修复；若不能在本轮正确完成，则在 `TODO.md` 中插入最小必要前置任务并提交后停止。
- 完成任务必须更新 `TODO.md` 标题为 `[DONE]`，补全 completion record，并提交所有相关更改。

## 执行步骤

1. 读取 `TODO.md`，定位第一个标题未带 `[DONE]` 的任务，并读取该任务的要求、依赖、验证命令与完成记录。
2. 查看最新提交信息，判断是否明确提到与当前任务直接相关的未完成问题；如有，将其纳入当前任务或作为前置任务记录到 `TODO.md`。
3. 根据当前任务范围检查相关代码、测试和文档，避免无关历史扫查。
4. 实现当前任务要求；如发现当前任务被真实阻塞，按规则更新 `TODO.md` 并停止推进实现。
5. 运行任务要求的验证命令；如验证暴露未排期失败，修复或添加最小必要前置任务，不能带着未排期失败标记完成。
6. 验证通过后，更新 `TODO.md`：在当前任务标题前加 `[DONE]`，补全完成记录与验证结果。
7. 仅在阶段级计划、依赖或完成标准发生变化时更新 `PLAN.md`。
8. 检查 `git status`、`git diff` 和最近提交，确认只提交本轮相关更改；按仓库风格创建提交。
9. 提交后停止，不处理下一个任务。

## 当前状态

- 已读取 `TODO.md`，第一个未完成任务是 `P7-T05-b`。
- 已读取 `TODO-6.md` 中 `P7-T05-b` 的任务体；最新提交为 `[P7-T05-b-0] Publish class ctor init contract`，是该任务的直接前置合同发布提交。
- 已审查 residual 代码路径，确认 `P7-T05-b-0` 已移走主要 class ctor HIR body emission；本轮重点为 `source_signatures` / `fun_index` callable signature fallback、dispatch target declaration、vtable/itable signature 查询和 dependency gate 防回归。
- 当前编辑计划：删除 LLVM codegen handoff 中的 HIR-derived `source_signatures`，保留仅含 source path/span 的 callable source narrow contract；新增 codegen-level callable signature helper，统一从 `LateLoweredProgram` / `LirFacts` 映射到 LLVM TypeStore；改造 dispatch/gc/source lookup 调用点；最后扩展 dependency gate。
- 已完成主要实现编辑：`source_signatures` base handoff 删除，LLVM callable signature/dispatch/gc 查询改走 `published_codegen_callable_signature` / LIR facts，source lookup 使用 callable source narrow contract，dependency gate 增加本轮 residual 防回归规则。
- 已运行 `cargo fmt`、dependency gate、`cargo test -p scoopc_lir_facts`、`cargo test -p scoopc --no-default-features llvm_codegen_stage`、`cargo test -p scoopc --no-default-features llvm::codegen`、`cargo test -p scoopc llvm::codegen`，均通过。
- 完整 run-pass fixture 暴露大面积失败；单独复现确认根因是 body-less/sysroot callable（例如 `scoop.core.__scoop_int_to_string`）缺少 LIR callable signature facts。
- 修复调整：按任务允许的 base-context 窄合同，补充仅含 param names / param types / return type 的 callable signature contract，作为 `LateLoweredProgram` / `LirFacts.source_signatures` 之外的显式窄输入；不恢复旧 `source_signatures` map、source path/function_ty ABI fallback 或 `fun_index.get` fallback。
- 第二轮 run-pass 剩余两个失败：`deprecated_fun_call_warning_basic` 因 ABI visibility callable declaration 仍用主 LIR symbol facts 查 exported identity；已改为 ABI materializer 从当前 `lir_facts.physical_layout.callable_symbols` 取 exported symbol 并传入 declaration。`member_call_struct_body_method_basic` 单独重跑通过。
- 完整 run-pass 已重新通过：421/421。
- 最终验证已通过：`cargo run -p scoop_tools -- dependency-gate`、`cargo test -p scoopc_lir_facts`、`cargo test -p scoopc --no-default-features llvm_codegen_stage`、`cargo test -p scoopc --no-default-features llvm::codegen`、`cargo test -p scoopc llvm::codegen`、`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass`、`cargo clippy --all-targets -- -D warnings`、`git diff --check`。
- 已更新 `TODO.md` 与 `TODO-6.md`，将 `P7-T05-b` 标记为 `[DONE]` 并填写完成记录。
- 下一步：检查 git 状态/diff/最近提交，提交本轮相关更改后停止。
