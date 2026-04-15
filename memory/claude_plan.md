# 当前执行计划

## 约束

- 本轮只完成 `TODO.md` 中第一个未完成任务，然后停止。
- 在执行任务前，先检查最新提交是否提到已有问题；若有，优先修复这些问题。
- 若首个未完成任务过大，需要先拆分任务，并同步更新 `PLAN.md` 与 `TODO.md`。
- 实现后必须补充或运行充分测试，并确保相关检查通过。
- 完成后需要更新 `TODO.md`、`PLAN.md`、本文件，并提交 git commit。

## 初始步骤

1. 查看最新一次 git 提交信息，确认是否提到需要先处理的既有问题。
2. 阅读 `TODO.md`，定位第一个未完成任务。
3. 阅读 `PLAN.md`，理解当前计划与依赖关系。
4. 如任务可直接执行，开始实现；如过大或被前置缺陷阻塞，则先拆分/重排任务，并更新 `TODO.md` 与 `PLAN.md`。
5. 运行相关测试、格式化与必要检查。
6. 更新文档与计划状态，提交本轮变更。

## 备注

- 这里先记录执行框架；在读取仓库现状后，我会把更具体的任务判断、实施步骤、阻塞项和进展继续写回本文件。

## 当前判断（读取仓库后）

- 最新提交是 `c1dfbf9 [T3002] Refine effect codegen dead_code boundaries, expose skeleton types`。
- 该提交说明没有列出需要在继续任务前先修复的既有缺陷；主要内容是开放统一 state-machine 骨架类型、精确化 ABI 的 `dead_code` 边界，并标记 flag-based unwind 为非主线。
- `TODO.md` 中第一个未完成任务是 `T3002R`：审查 `crates/scoopc/src/llvm/codegen/**`，确认 effect codegen 生产代码中不存在 shape-based 主分流。
- 当前工作区只有本文件修改，未见其他脏改动。

## 本轮执行步骤（细化）

1. 定向检索 `crates/scoopc/src/llvm/codegen/**` 中与 effect codegen 路径有关的入口和 helper，重点检查：
   - 是否存在按源码形状、call-site 形状、arm 形状或 callee 形状选路的主路径。
   - `mod.rs` / `effect/mod.rs` / 相关 helper 是否重新引入了旧 scanner、旧 mode、旧分类器或等价分流。
2. 若审查发现问题：
   - 直接修复生产代码；
   - 重新复审相关路径；
   - 运行必要测试与检查。
3. 若审查未发现问题：
   - 记录审查结论；
   - 运行本任务所需验证命令，至少覆盖 `cargo check -p scoopc`、`cargo clippy --all-targets -- -D warnings`，并视情况运行更广测试。
4. 更新 `TODO.md`、`PLAN.md`、`memory/claude_plan.md`，将 `T3002R` 标记完成并写入审查结论。
5. 提交 git commit，然后停止。

## 已完成的关键检查

- 已检索 `crates/scoopc/src/llvm/codegen/**` 中与 `shape`、`scan_for`、`CalleeSuspend`、`suspendable` 等旧分流命名相关的残留，未发现命中。
- 已复查 `crates/scoopc/src/llvm/codegen/expr.rs`：
  - `hir::ExprKind::Perform` 直接调用 `self.codegen_perform_expr(...)`；
  - `hir::ExprKind::Handle` 直接调用 `self.codegen_handle_expr(...)`；
  - 中间没有额外的源码形状分派层。
- 已复查 `crates/scoopc/src/llvm/codegen/effect/mod.rs`：
  - 当前 `perform` / `handle` 入口仍是统一 lowering 重新接线前的占位错误；
  - 文件中保留的 effect 相关生产 helper 只剩 flag-based unwind 三方法与 sysroot intrinsic lowering；
  - 未发现新的 shape-based helper 或等价入口。
- 已复查 `crates/scoopc/src/llvm/codegen/mod.rs` 的普通调用链：
  - `codegen_call` 的分发按 callee 身份、vtable/itable、sysroot intrinsic、函数值 / funptr 等语义类别选择；
  - 未发现按源码/callee 形状决定 effect lowering 路径的逻辑；
  - 当前残留的 `emit_effect_unwind_if_active` 调用点是统一挂在若干调用后的 flag-based unwind 机制，属于后续 `T3005` 计划移除对象，但不是 shape-based 主分流。

## 当前结论

- 到目前为止，`T3002R` 的定向审查没有发现需要立即修复的 shape-based effect codegen 主分流残留。
- 已完成验证：
  - `cargo check -p scoopc`
  - `cargo clippy --all-targets -- -D warnings`
  - `cargo test --all`
  - 以上命令均已通过。
- 已完成收尾：
  - 已更新 `TODO.md`，将 `T3002R` 标记为完成并写入审查结论。
  - 已更新 `PLAN.md`，记录 `T3002R` 的 review 结果，并将下一执行项前移到 `T3003`。
  - 已复查当前 diff，确认本轮只修改了 `TODO.md`、`PLAN.md` 与本文件。
- 剩余动作：
  - 提交本轮 commit 并停止。
