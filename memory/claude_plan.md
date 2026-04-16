# 执行计划

## 说明

按要求先记录计划，并在执行过程中持续更新。本文件只记录可共享的执行思路、步骤、检查点和进展，不包含不可验证的内部推理细节。

## 初始计划

1. 检查最新一次提交，确认提交信息中是否提到已知问题、遗留修复或待处理事项。
2. 阅读 `TODO.md`，定位第一个未完成任务。
3. 评估该任务是否过大：
   - 如果可直接完成，则进入实现。
   - 如果过大或依赖缺失，则拆分任务，更新 `PLAN.md` 与 `TODO.md`，并只处理拆分后的第一个子任务。
4. 在实现前先阅读相关代码、测试和规范，确认没有通过规避缺陷来“完成任务”的风险。
5. 实现第一个未完成任务所需改动。
6. 运行相关验证：
   - 最小相关测试
   - 必要时运行更广覆盖的测试
   - `cargo fmt`
   - `cargo clippy --all-targets -- -D warnings`
7. 更新文档状态：
   - 在 `TODO.md` 标记完成，或在阻塞时重排任务并保持 `[TODO]`
   - 更新 `PLAN.md`
   - 继续更新本文件的进展记录
8. 检查工作区改动，确认没有误改或遗漏。
9. 提交改动，提交信息使用任务编号或清晰描述。
10. 停止，不继续处理下一个任务。

## 进展记录

- 已创建本计划文件。
- 已检查最新提交：提交信息为 `[T3008aR] 审查并确认 effect frame ABI 收口`，未在提交信息中直接提出新的必须先修遗留问题。
- 已读取 `TODO.md` / `PLAN.md`，确认当前排在最前的未完成任务是 `T3009`：为 `resume(...)` / `Continuation.resume(...)` 接回专用 lowering，删除 placeholder local。
- 初步判断：`T3009` 的边界清晰，先不拆分子任务；先验证当前失败形态与专用 lowering 缺口是否刚好落在统一 emitter / 普通 call lowering 两处接线。
- 当前检查点：
  1. 复查 `state_machine_emitter.rs` 中 `ImmediateResume` / `ArmResumeMatchedSite` 的实现。
  2. 复查 `Continuation.resume` 的 side table / call lowering 接线。
  3. 运行定向 fixture 复现失败，确认是否正是 `call callee` / generic member access 回落。
  4. 实现专用 lowering，并删除 `resume_placeholder`。
  5. 跑定向验证、全量 LLVM fixture、`cargo fmt`、`cargo clippy --all-targets -- -D warnings`。
  6. 更新 `TODO.md`、`PLAN.md`、本文件并提交。
- 已完成的实现草稿：
  - 已做过一轮专用 lowering 试探实现，用于确认 `T3009` 的真实阻塞面。
- 试探结果：
  - `resume(...)` / `Continuation.resume(...)` 的 generic fallback 确实是当前表层错误来源；移除后，`effect_resume_yield_int_basic.scoop` 可以进入运行期，不再停在 `call callee` / `member access target`。
  - 但随后暴露出更底层的前置缺口：`val x = Yield.next()` 的 resume landing 会重新发射原始 `perform` / fragment op，导致 `resume(41)` 后重复进入同一 handler arm，而不是继续执行 handled computation；这与 `T3010` 完全一致。
  - 同时，`continuation_resume_enum.scoop` 的验收仍要求 composite resume payload transport，与 `T3013` 的目标重合。
- 已采取动作：
  - 已撤回试探性生产代码改动，避免把仓库留在半完成状态。
  - 已将 `T3009` / `T3009R` 后移到 `T3013R` 之后，并把 `T3010` 提升为当前首个未完成任务。
- 已验证：
  - 当前工作区只剩 `PLAN.md`、`TODO.md`、`memory/claude_plan.md` 三处文档变更；未保留试探性生产代码改动。
  - `cargo check -p scoopc` 在回退试探代码后通过，基线未被破坏。
- 下一步：只提交任务重排与阻塞说明，然后停止。
