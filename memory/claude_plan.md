# 本轮执行计划（T4008c3）

## 目标

- 只完成 `TODO.md` 中当前首个未完成任务：`T4008c3 [TODO] 收口 handler arm head 的 effect-op 绑定主线`。
- 不进入后续任务；完成后更新文档、验证、提交并停止。

## 已知上下文

- 先前实现已经覆盖三条主线：
  - parser 允许 handler arm head 使用显式 type args；
  - typecheck 复用 effect-op 调用的共享签名 lowering / 实例化逻辑；
  - HIR lowering 让 generic effect-op call 继续走 `Perform` 主线，而不是残留为普通 `Call(TypeApply(...))`。
- 已补充 parse / typecheck / run-pass 夹具，并修复一个 parse recovery 回归。
- 已有定向验证通过，但还缺少本轮最终收口所需的格式化、完整测试、文档更新和提交。

## 执行步骤

1. 检查工作区状态，确认当前改动范围以及是否存在临时探针文件需要清理。
2. 运行 `cargo fmt`，确保格式一致。
3. 运行完整验证：
   - `cargo fmt --check`
   - `cargo run -q -p scoop -- test`
   - `cargo test --all`
   - `cargo clippy --all-targets -- -D warnings`
4. 如果验证暴露真实问题，先修复问题；若属于规范缺口或前置依赖，则按要求调整 `TODO.md` / `PLAN.md` 并停止。
5. 在验证通过后更新文档：
   - `memory/claude_plan.md` 记录完成情况与验证结果；
   - `TODO.md` 将 `T4008c3` 标记为完成；
   - `PLAN.md` 反映 `T4008c3` 已完成、下一个待办切换到后续任务；
   - 如有必要，调整 `ISSUES.md` 中已过时的描述。
6. 复查 `git diff` / `git status`，确保只包含本轮需要提交的变更。
7. 使用清晰的提交信息提交，例如：`[T4008c3] Unify handler arm head effect-op binding`。
8. 停止，不继续处理 `T4008c4`。

## 风险点

- parser recovery、generic effect-op lowering 和 handler arm binder 类型传播是本次最容易出现回归的区域。
- 若全量 fixture 测试暴露此前未覆盖的 effect-op / handler 退化，需要先修复该真实问题，不能以局部通过替代。

## 当前进展

- 工作区已核对完毕，临时探针 `memory/t4008c3_probe.scoop` 已移除，不会进入最终提交。
- `T4008c3` 的实现已完成：
  - parser 现支持 handler arm head 的显式 effect type args / op type args；
  - AST `HandleOp` 新增 `op_type_args`，handler arm 与 effect-op call 复用同一套签名 lowering / 实例化主线；
  - HIR lowering 把 generic effect-op call 的 `TypeApply(callee)` 视为透明包装，继续进入 `Perform` 主线；
  - 已新增 parse / typecheck / run-pass 回归夹具覆盖这些路径。

## 验证结果

- `cargo fmt`
- `cargo fmt --check`
- `cargo run -q -p scoop -- test`：`fixtures: ok (1068)`
- `cargo test --all`
- `cargo clippy --all-targets -- -D warnings`

## 收口结论

- `TODO.md` 中的 `T4008c3` 可以标记为完成。
- `ISSUES.md` 第 1 条里 “handler arm head 仍只接受 effect operation” 这一缺口已收窄移除。
- 本轮剩余动作只包括整理提交并创建 `T4008c3` 的 commit，随后停止，不进入 `T4008c4`。
