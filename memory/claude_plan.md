## 当前计划摘要

说明：不记录内部详细推理，仅记录可审阅的执行计划、关键决策和进度。

### 初始执行顺序

1. 检查最新一次 Git 提交，确认是否提到了需要先修复的既有问题。
2. 阅读 `TODO.md`，定位第一个未完成任务。
3. 阅读 `PLAN.md`，核对该任务是否已有计划或依赖说明。
4. 如果第一个未完成任务过大，则将其拆分为更小的子任务，并同步更新 `PLAN.md` 与 `TODO.md`；本次仅执行拆分后的第一个子任务。
5. 实现当前目标任务，并补充必要的测试与文档。
6. 运行相关验证，包括至少：
   - 受影响范围测试
   - `cargo test --all`
   - `cargo clippy --all-targets -- -D warnings`
   - 如任务涉及格式或规范文件，则运行对应检查命令
7. 更新 `TODO.md`、`PLAN.md`、`memory/claude_plan.md` 记录完成状态与必要说明。
8. 提交 Git commit，然后停止，不继续处理下一个任务。

### 进度

- 已检查最新提交：`[T3014a] Implement same-op multi-arm handler dispatch`，提交正文未额外列出需先修复的遗留问题。
- 已读取 `TODO.md` / `PLAN.md`，确认第一个未完成任务为 `T3014R`：复审 multi-op handler registration、unmatched outward propagation、same-op multi-arm dispatch 是否已按统一合同收口。

### 当前任务：`T3014R`

本轮仅执行 `T3014R`，不推进后续任务。

#### 复审步骤

1. 阅读 `TODO.md` 中 `T3014`、`T3014a`、`T3014R` 的任务描述，整理需要核对的合同：
   - handler registration 必须与 `dispatch_entries()` 一一对应
   - unmatched perform 必须 outward propagate，不能走 `handle_done`
   - 同一 `op_fqn` 的多 arm dispatch 不能退化成首 arm 特例
2. 审查生产代码中的关键路径：
   - `crates/scoopc/src/llvm/codegen/effect/state_machine_emitter.rs`
   - `crates/scoopc/src/llvm/codegen/mod.rs`
   - 如有必要，连带检查 runtime ABI / runtime C 实现
3. 审查现有测试与 fixture，确认它们确实锁定上述合同，而不是只覆盖表面现象。
4. 如果发现生产代码问题：
   - 直接修复实现
   - 补充或收紧测试
   - 重新运行验证
5. 如果未发现问题：
   - 运行复审所需验证
   - 更新 `TODO.md` / `PLAN.md` / 本文件，标记 `T3014R` 完成并记录审查结论
6. 提交本轮变更并停止。

#### 当前状态

- 已完成 `T3014/T3014a` 相关生产代码审查：
  - `state_machine_emitter.rs` 当前由同一份 `contract.dispatch_entries()` 同时驱动 handler 注册与 dispatch。
  - `dispatch_unmatched` 会直接进入 outward propagation 路径，不经过 `handle_done`。
  - same-op multi-arm dispatch 已不再收缩为首 arm，runtime 会读取 `effect_instance_key` 并逐 arm 检查。
- 已完成定向验证：
  - `cargo test -p scoopc --features llvm multi_dispatch_handle_ir_registers_every_op_tag_on_handler_stack -- --nocapture`
  - `cargo test -p scoopc --features llvm same_op_multi_arm_dispatch_ir_reads_effect_instance_key -- --nocapture`
  - `cargo test -p scoop_runtime --test effect_tls -- --nocapture`
  - 5 条关键 run-pass fixture
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
- 在补跑 `cargo run -p scoop --features llvm -- test` 时，先发现并修复了两个 stale HIR golden：
  - `tests/fixtures/hir/handle_mixed_arm_kinds.hir`
  - `tests/fixtures/hir/safe_call_not_null_assert.hir`
- 清掉 snapshot 漂移后，full fixture runner 暴露新的真实 blocker：
  - `tests/fixtures/run-pass/class_init_hidden_raise_helper_try_catch_basic.scoop`
  - 直接运行报错：`UnsupportedMainBody { kind: "effect instance key" }`
  - 说明 ordinary hidden-suspend `Raise.raise(RuntimeError.*)` 路径在当前 `effect_instance_key` 合同下仍未闭环

### 计划变更

1. 将当前任务 `T3014R` 保持为未完成。
2. 在 `TODO.md` / `PLAN.md` 中新增前置任务 `T3014b` / `T3014bR`，先修 ordinary hidden-suspend runtime-error raise 的 `effect_instance_key` 回归。
3. 本轮提交内容只包括：
   - 复审过程中修掉的 stale HIR golden
   - 新 blocker 的任务重排与说明更新
4. 停止，等待下一轮先处理 `T3014b`。
