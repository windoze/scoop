# 本轮执行计划

## 目标

按 `TODO.md` 的顺序完成第一个未完成任务；如果在检查、实现或测试过程中发现任何已存在问题，则先修复该问题或将其作为前置任务插入 `TODO.md`，随后停止在本轮允许的边界内完成一次提交。

## 当前已知步骤

1. 查看最新一次 Git 提交，确认提交说明中是否提到尚未修复的问题；如果提到，则优先处理该问题。
2. 阅读 `TODO.md`，定位第一个未完成任务。
3. 阅读 `PLAN.md`，核对现有计划、依赖关系与任务拆分是否一致。
4. 结合代码现状评估该任务是否可在本轮完整完成。
5. 如果任务过大，则将其拆分为更小的子任务，并同步更新 `PLAN.md` 与 `TODO.md`，然后执行拆分后的第一个子任务。
6. 实现当前目标任务，并在必要时补充或调整测试。
7. 运行与改动相关的验证，包括至少相关测试；如果改动范围允许，再运行更严格的检查（例如 `cargo fmt --check`、`cargo clippy --all-targets -- -D warnings`、定向或全量测试）。
8. 将完成情况写回 `TODO.md`、`PLAN.md` 与本文件。
9. 使用清晰的提交信息创建一次 Git 提交。
10. 停止，不继续处理下一个任务。

## 计划维护规则

- 如果发现阻塞当前任务的既有缺陷、规格不匹配或实现边界缺口，立即把该问题视为当前范围的一部分。
- 若无法在本轮直接修复阻塞问题，则必须先在 `TODO.md` 中插入前置任务、调整依赖顺序，并在 `PLAN.md` 与本文件中记录原因，然后提交并停止。
- 每完成关键节点（例如确认任务、完成拆分、完成实现、完成测试）后，更新本文件，便于追踪执行进展。

## 当前进展

- 已检查最新提交 `559661e06a5b1f0387f3f3a46099bf04040ba34a`，提交说明为 `[T5000b3bR] Complete intrinsics review boundary cleanup`，未在提交说明中发现仍待先修的显式遗留问题。
- 已读取 `TODO.md` 与 `PLAN.md`，确认本轮首个未完成任务为 `T5000b3c 拆出 closure/ 与 class_ctor.rs lowering 模块`。
- 已初步核对 `crates/scoopc/src/llvm/codegen/mod.rs` 中待迁移实现边界：
  - closure 主题当前集中在 `ClosureParamBindings`、`ClosureBodyCodegenSpec`、`build_closure_callee_suspend_plan`、`codegen_closure_expr`、`closure_param_bindings`、`codegen_closure_fun_body`、`llvm_closure_env_type`、`lookup_pure_unit_closure_type` 与 `closure_callee_resume_entry_fn_name`；
  - class ctor 主题当前集中在 `codegen_class_ctor_call`、`pick_class_ctor_by_target`、`codegen_class_ctor_eval_args`、`bind_class_ctor_call_param_value`、`codegen_class_ctor_call_super`、`codegen_class_ctor_run_init_steps`、`codegen_class_ctor_invoke`、`codegen_class_ctor_invoke_inner`。
- 已核对调用面：
  - closure lowering 由 `expr.rs`、`effect/mod.rs`、`intrinsics/sync.rs`、`intrinsics/thread.rs`、`call/abi.rs` 使用；
  - closure resume 名称 helper 由 `call/resume.rs` 使用；
  - class ctor lowering 由 `call/dispatch.rs` 使用。

## 下一步

1. 新建 `crates/scoopc/src/llvm/codegen/closure/mod.rs` 与 `crates/scoopc/src/llvm/codegen/class_ctor.rs`。
2. 将上述 closure/class ctor 实现整体迁入对应模块，按需要收紧为 `pub(in crate::llvm::codegen)` 接口。
3. 回写 `codegen/mod.rs`，仅保留模块声明与必要窄桥接，不继续承载这两类 lowering 主体。
4. 运行格式化、定向测试、全量测试与 `clippy -D warnings`。
5. 若验证通过，更新 `TODO.md`、`PLAN.md` 与本文件，然后提交并停止。

## 完成情况

- 已新增 `crates/scoopc/src/llvm/codegen/closure/mod.rs` 与 `crates/scoopc/src/llvm/codegen/class_ctor.rs`，分别承接 closure 与 class ctor lowering 主体实现。
- 已将 `crates/scoopc/src/llvm/codegen/mod.rs` 收口为主题模块声明、共享上下文与通用 helper；closure resume 命名 helper 也已从根模块迁入 `closure/`，`call/resume.rs` 改为按模块边界导入。
- 已完成验证：
  - `cargo fmt --all`
  - `cargo test -p scoopc llvm::`
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
  - 结果均通过。
- 待完成的收尾动作只剩：
  1. 检查最终 diff 与工作区状态；
  2. 提交本轮变更；
  3. 停止，等待下一轮从 `T5000b3cR` 开始。
