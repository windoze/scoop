# 本轮执行计划

说明：我不会写入逐字的内部推理过程，但会持续记录可公开的执行计划、关键判断依据、执行进展与结果，便于检查当前状态。

## 初始计划

1. 检查最新一次 Git 提交，确认是否提到了需要先处理的既有问题。
2. 阅读 `TODO.md`，定位第一个未完成任务。
3. 阅读 `PLAN.md`，核对当前计划与任务依赖关系。
4. 判断该任务是否可以在本轮完整完成。
5. 如果任务过大，先细化为更小的子任务，并更新 `PLAN.md` 与 `TODO.md`，然后只执行新的第一个子任务。
6. 实现本轮目标任务，并补充或调整必要测试。
7. 运行相关验证，包括至少与改动直接相关的测试；若涉及全局质量要求，再补跑 `cargo fmt`、`cargo test --all`、`cargo clippy --all-targets -- -D warnings` 或足够覆盖本任务的命令。
8. 更新 `TODO.md`、`PLAN.md` 与本文件，记录完成情况或阻塞原因。
9. 提交 Git commit，随后停止，不继续处理下一个任务。

## 待确认信息

- 最新提交是否提到必须先修复的问题。
- `TODO.md` 中第一个未完成任务的具体内容与依赖。
- 当前工作区是否已有未提交改动，需要在不回退他人修改的前提下协同处理。

## 已确认

- 最新提交 `f6976dba` 的提交说明为 `[T4016b3] Wire Continuation.resume answer-return through typecheck and lowering`，未额外声明需要先修复的既有问题。
- 当前工作区除本文件外无未提交改动。
- `TODO.md` 中首个未完成条目是 `T4016d`：让 `Task` 退化为基于 continuation answer type 的薄封装，并移除剩余 runtime hack / 叙事债务。
- 初步盘点表明本任务可以直接完成，无需继续拆分。

## 发现的具体收口点

1. `async` lowering 里的内部 pending-path 仍在用旧式/擦除过度的 continuation 形状构造 HIR，需要把 task step continuation 的 answer type 显式接到 lowering 产物。
2. `sysroot/core.scoop`、`SCOOP_RUNTIME.md`、`SCOOP_FULL_SPEC.md` 与 runtime 注释里仍保留 “`T4016d` 待收口” 一类过渡表述，需要改成最终叙事。
3. 需要补一条能直接约束该 lowering 结果的测试，避免以后再次回退到旧 continuation 形状。

## 更新后的执行步骤

1. 修正 `async` lowering 的内部 continuation 类型构造，使 task step continuation 明确挂上 `__TaskStepResult` answer。
2. 视实现需要同步调整相关内部 helper 注释或声明，确保 `Task` 被描述为 “ordinary object + private step-result continuation”。
3. 增加/更新单元测试与必要 fixture，覆盖 async lowering 产物中的 task step continuation 形状。
4. 运行格式化与相关测试；若改动范围允许，补跑全量 `cargo test --all` 与 `cargo clippy --all-targets -- -D warnings`。
5. 更新 `TODO.md`、`PLAN.md` 与本文件，记录 `T4016d` 完成。
6. 提交本轮变更并停止。

## 当前进展

- 已修改 `crates/scoopc/src/hir/lower/block.rs` 与 `crates/scoopc/src/hir/lower/mod.rs`：
  - async lowering 生成的私有 task continuation 现在显式带 `__TaskStepResult` answer；
  - 为测试加入了递归查找内部 top-level call 的辅助函数与新单测骨架。
- 已修改 `sysroot/core.scoop`：
  - 将 `__scoop_task_step_pending` 的内部 helper surface 收口为擦除 payload 的
    `Task<Any>` / `Continuation<Any, __TaskStepResult>`；
  - 注释改成最终叙事，不再保留 “T4016d 待收口” 的过渡描述。
- 已修改 `SCOOP_RUNTIME.md`、`SCOOP_FULL_SPEC.md`、`runtime/c/scoop_runtime.c`：
  - 文档/注释已改为最终表述：`Task` 与 expression-position `Continuation.resume(...)`
    共用同一条 continuation answer-return 通道。
- 已完成验证：
  - `cargo fmt`
  - `cargo test -p scoopc erases_async_step_payload`
  - `cargo test -p scoopc async_task_resume_ir_does_not_replay_original_await_site`
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`

## 新发现的阻塞

- 在追加执行 `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass` 时，`continuation_escape_binder_resume_effect_row_runtime_basic.scoop` 失败。
- 单独重现后确认：
  - 失败不是 `Task` thin-wrapper 改动本身，而是 legacy `Continuation<Resume, eff E>` shorthand 兼容路径把 continuation answer-hole 以 `TypeKind::Param` 形式泄漏进 LLVM codegen；
  - 具体症状为 `cg_ty_of: TypeKind::Param(_) encountered in codegen (monomorph miss)`，随后报 `effect frame slot type`。
- 该问题未在现有 `TODO.md` 中单独立项，但它会阻塞 `T4016d` 的完整 run-pass 验收。

## 处理决定

1. 按流程把这个问题新增为 `TODO.md` / `PLAN.md` 中位于 `T4016d` 之前的 blocker 任务。
2. 保留本轮已经完成且通过 `cargo test --all` / `clippy` 的 task-thin-wrapper 收口改动。
3. 不把 `T4016d` 标记完成；本轮提交以“发现并显式排程 blocker，同时保留已验证的部分收口改动”为止。
