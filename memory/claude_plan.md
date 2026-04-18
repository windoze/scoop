# 当前轮执行计划

## 约束说明

- 按要求，本文件会在本轮开始时先建立，并在关键步骤完成或计划变更时持续更新。
- 我不会记录不可公开的原始内部推理；这里记录的是可审计的执行计划、依据、发现的问题与处理决定。
- 本轮目标是：先检查最新提交是否提到需要先修复的既有问题；再读取 `TODO.md` 找到第一个未完成任务；只完成这一个任务，然后测试、更新文档、提交并停止。

## 初始执行步骤

1. 检查最新一次 Git 提交的提交信息与变更摘要，确认是否明确提到尚未修复的既有问题。
2. 阅读 `TODO.md`，识别第一个未完成任务。
3. 如任务过大，拆分为可执行子任务，并同步更新 `PLAN.md` 与 `TODO.md`。
4. 实现当前应执行的首个任务或首个子任务。
5. 运行相关测试，并补齐必要测试直到任务可验证完成。
6. 更新 `TODO.md`、`PLAN.md` 和本文件，记录完成状态或阻塞原因。
7. 提交本轮所有改动，随后停止，不进入下一个任务。

## 进度日志

- 已创建本文件并写入初始计划。
- 已检查最新提交、`TODO.md`、`PLAN.md`，当前首个未完成任务为 `T3016lR`（review）。
- 已开始审查 `emit_raise_runtime_error_variant()`、共享 effect transport 编码，以及 handler dispatch / runtime 相关生产路径。
- 审查中发现一处需要在本 review 任务内直接修复的真实生产问题：
  - `runtime/c/scoop_runtime.c` 的 `scoop_continuation_resume_try()` 在报告 `RuntimeError.ContinuationAlreadyResumed` 时，仍调用 `scoop_effect_perform_slot_write_u64_2(...)` 写入旧的双 word payload。
  - 当前 catch 语义之所以没有明显坏掉，只是因为 `payload_words[0] == 2` 恰好与 `ContinuationAlreadyResumed` 的 enum unit variant tag 一致；但 `payload_len_words == 2` 已经偏离统一的 `encode_effect_transport_value()` 合同，不应继续保留。
- 修正方案：
  1. 把 runtime 侧的 `ContinuationAlreadyResumed` raise 改为复用统一的单 word + `gc_ref = null` transport 语义。
  2. 增加一条 runtime 回归测试，直接验证 double-resume 后 perform slot 的 `len_words == 1`、`word0 == ContinuationAlreadyResumed` tag、`gc_ref == null`。
  3. 重新执行相关测试与 lint。
  4. 若验证通过，则完成 `T3016lR` 的记录、更新 `TODO.md`/`PLAN.md` 并提交。
- 已完成代码修复：
  - `runtime/c/scoop_runtime.c` 新增 `scoop_effect_raise_runtime_error_variant()`，runtime-originated `Raise<RuntimeError>` 现统一走 single-word enum tag + null `gc_ref` transport。
  - `scoop_continuation_resume_try()` 不再写旧的双 word payload；double-resume 现在与编译器生成的 `Raise.raise(RuntimeError.X)` 共享同一套 transport 合同。
  - 已新增 runtime 回归测试 `continuation_double_resume_uses_shared_runtime_error_transport_contract`，直接锁定 `payload_len_words == 1`。
- 已完成验证：
  - `cargo test -p scoop_runtime continuation_double_resume_uses_shared_runtime_error_transport_contract -- --exact`
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/type_check_cast_is_as_asq_basic.scoop`
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
- 当前状态：
  - `T3016lR` 已完成，相关 review 发现已在本任务内修复并复审通过。
  - 已更新 `TODO.md`、`PLAN.md` 和本文件。
  - 下一步：提交本轮改动并停止。
