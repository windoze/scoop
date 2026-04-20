# Claude Plan

## 约束说明

- 按用户要求，先建立本文件作为执行计划与进度记录。
- 受系统安全策略限制，这里记录的是可审计的执行计划、决策依据与进度更新，不写入逐字的内部推理。

## 初始执行计划

1. 检查最近一次 Git 提交信息，确认是否提到需要先修复的既有问题。
2. 读取 `TODO.md`，定位第一个未完成任务。
3. 读取 `PLAN.md`，核对当前任务与总体计划是否一致。
4. 如首个未完成任务过大，先把任务拆分为可执行子任务，并更新 `PLAN.md` 与 `TODO.md`；本次只执行拆分后的第一个子任务。
5. 实现本次目标任务。
6. 运行与改动直接相关的测试，再补充执行用户要求的质量检查，包括 `cargo clippy --all-targets -- -D warnings`（若作用域合理则执行全量）。
7. 更新 `TODO.md`、`PLAN.md` 和本文件，记录完成状态或阻塞原因。
8. 检查工作区变更，使用清晰的提交信息提交本次工作。
9. 停止，不进入下一个任务。

## 当前状态

- 已创建计划文件。
- 已检查最近一次提交、`TODO.md`、`PLAN.md` 与工作区状态。
- 已确认最新提交显式提到一个既有阻塞：legacy `Continuation` 简写会把 continuation answer-hole 泄漏到 LLVM codegen。
- 已确认 `TODO.md` 中首个未完成且应立即处理的任务是 `T4016b4`：收口 legacy `Continuation<Resume, eff E>` / `Continuation<Resume>` 兼容 shorthand，避免 answer-hole 泄漏到 codegen。

## T4016b4 执行计划

1. 复现 `continuation_escape_binder_resume_effect_row_runtime_basic.scoop` 的失败，确认当前报错栈与触发路径。
2. 检查 shorthand continuation 的解析、type lowering、typecheck、monomorph 与 codegen 路径，定位 answer-hole 何时以 `TypeKind::Param` 形式泄漏。
3. 选择正确收口方式：
   - 若 shorthand 在该位置本可被安全具体化，则在前端尽早具体化 answer type；
   - 若该位置无法保持正确语义，则改为更早给出兼容/移除诊断，禁止继续进入 codegen。
4. 更新受影响 fixture，确保其 surface 与当前 answer-return continuation 语义一致。
5. 补充或调整 regression，覆盖 shorthand 的允许路径或拒绝路径，避免再次把 answer-hole 带入 codegen。
6. 运行定向测试，再执行必要的全量验证与 `cargo clippy --all-targets -- -D warnings`。
7. 更新 `TODO.md`、`PLAN.md` 与本文件，记录 `T4016b4` 完成情况。
8. 提交改动并停止。

## 当前定位结果

- 已复现失败，并确认错误来自 LLVM effect state machine 的 frame slot：
  - `field_index=8`
  - `slot_name=__resume_site0`
  - `slot_ty=_`（即 continuation answer-hole 本体）
- 结论：泄漏点是 non-Pure `Continuation.resume(...)` 的恢复槽，而不是保存 continuation 对象的字段本身。
- 当前准备采取的收口规则：
  - 移除 `Continuation<Resume, eff E>` 这种带显式 `eff` 的 legacy shorthand，要求改写为 `Continuation<Resume, Answer, eff E>`；
  - 继续暂时保留 `Continuation<Resume>` 的默认 `Pure` compatibility shorthand，避免一次性重写大批现有 pure fixtures；
  - 同步把当前 blocker fixture / 单测迁到显式 answer type，并补对应 typecheck regression。

## 最新进展

- 已完成 `T4016b4a` 方向的实现：
  1. type lowering 新增 removed/compatibility diagnostic，拒绝 `Continuation<Resume, eff E>`。
  2. 已迁移首批直接受影响的 fixture / 单测到显式 answer type。
  3. 已把一批明显 answer=`Unit` 的 resume payload fixtures 改成 `Continuation<Payload, Unit>`。
- 已完成验证：
  - `cargo run -p scoop -- build tests/fixtures/run-pass/continuation_escape_binder_resume_effect_row_runtime_basic.scoop -o /tmp/cont-shorthand.out && /tmp/cont-shorthand.out`
  - `cargo run -p scoop -- test --fixtures tests/fixtures/typecheck`
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
- 新发现：
  - 全量 `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass` 仍会在 pure shorthand fixtures 上失败，例如：
    - `continuation_resume_continuation.scoop`
    - `continuation_resume_enum.scoop`
    - `effect_escape_continuation_async_executor_fifo.scoop`
  - 这些失败与首个 blocker 同源：legacy `Continuation<Resume>` 仍可能把 answer-hole 泄漏到 `__resume_site*` frame slot。
- 因此已把原 `T4016b4` 拆为：
  - `T4016b4a`：已完成，本次提交；
  - `T4016b4b`：保留为下一轮首个未完成任务，继续系统迁移 pure shorthand fixtures/source。
