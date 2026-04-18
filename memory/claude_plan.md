# 执行记录

说明：根据安全与协作要求，这里记录的是可执行计划、决策依据摘要与进度更新，不包含不可审计的内部长链路推理。

## 初始计划（2026-04-18）

1. 检查最新一次 Git 提交，确认是否提到需要先处理的既有问题。
2. 阅读 `TODO.md`，定位第一个未完成任务。
3. 阅读 `PLAN.md`，核对当前计划与 `TODO.md` 是否一致。
4. 如果首个未完成任务过大，先将其拆分为更小的子任务，并同步更新 `TODO.md` 与 `PLAN.md`。
5. 实现当前应执行的第一个任务。
6. 运行相关测试，并补充必要测试直到任务满足要求。
7. 更新 `TODO.md`、`PLAN.md` 与本文件，记录完成情况或阻塞原因。
8. 提交 Git commit，然后停止，不继续处理下一个任务。

## 进度更新

### 2026-04-18 任务定位

- 已检查最新提交：`edfd1de39b841a49c65154fa662ef56c6dc904cf`，提交标题为 `[T3009b] Close composite continuation resume task`，提交信息未额外点名新的必须先修问题。
- 已读取 `TODO.md` 与 `PLAN.md`，首个未完成任务为 `T3009bR`：`Review：确认 escaped continuation resume 调用不再回落到 generic member access`。
- 当前判断：先执行复审，不需要先拆分任务；若复审发现真实生产缺口，将在本轮内直接修复，并同步更新 `TODO.md` / `PLAN.md`。

### 2026-04-18 `T3009bR` 复审完成

- 已完成生产代码复审：覆盖 `resolve/typecheck/HIR/codegen/state-machine/runtime` 全链路，确认 `Continuation.resume(...)` 的 builtin 语义只由 `continuation_resume_call_sites` side table 驱动，不存在按成员名、FQN 或 receiver 形状回落到 generic member access / generic call 的 production 分支。
- 已确认 transport/runtime 合同保持统一：`codegen_continuation_resume_builtin()` 直接写 continuation 的 `resume_word` / `resume_gc_ref` 并调用 `scoop_continuation_resume()`；runtime 的 `scoop_continuation_resume_common()` 统一负责恢复 handler context 与 callee suspend state。`scoop_continuation_resume_u64()` 仍保留为旧 ABI 兼容 helper，但不是 `Continuation.resume(...)` 的 placeholder glue。
- 已完成验证：
  - `cargo test -p scoopc continuation_resume_hidden_suspend_classification_requires_typechecked_call_site_marker -- --nocapture`
  - `cargo test -p scoop_runtime continuation_resume_ -- --nocapture`
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/continuation_resume_tuple.scoop`
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/continuation_resume_struct.scoop`
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/continuation_resume_struct_with_ref.scoop`
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/continuation_resume_continuation.scoop`
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/continuation_resume_enum.scoop`
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_escape_continuation_indirect_perform_resume_string.scoop`
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_escape_continuation_indirect_perform_resume_struct_with_ref.scoop`
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
- 复跑 `cargo run -p scoop --features llvm -- test` 后，suite 仍停在 `tests/fixtures/run-pass/effect_escape_continuation_async_executor_fifo.scoop` 的 stale `EXPECT: fail`；该问题已由 `T3017` 跟踪，不属于本轮新增 blocker。
- 已更新 `TODO.md` 与 `PLAN.md`，把 `T3009bR` 标记完成，并将下一项推进到 `T3015`。

## 当前状态

- 步骤 1：已完成
- 步骤 2：已完成
- 步骤 3：已完成
- 步骤 4：已完成（当前任务无需拆分）
- 步骤 5：已完成
- 步骤 6：已完成
- 步骤 7：已完成
- 步骤 8：待开始（提交本轮文档更新并停止）
