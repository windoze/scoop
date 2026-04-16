# 当前执行计划

## 目标
- 本轮只完成 `TODO.md` 中第一个未完成任务，然后停止。
- 在开始任何仓库检查前，先记录执行计划与后续进展。

## 约束与执行原则
- 先检查最新提交是否提到遗留问题；若有，优先修复。
- 不接受规避实现、临时兼容或偏离规范的做法。
- 若当前任务过大，需要先拆分并同步更新 `PLAN.md` 与 `TODO.md`。
- 过程中如计划变化或关键步骤完成，及时更新本文件。

## 初始步骤
1. 查看最新一次 git 提交信息，确认是否包含需要先处理的已知问题。
2. 读取 `TODO.md`，定位第一个未完成任务。
3. 如任务过大，拆解为更小子任务，并更新 `PLAN.md` 与 `TODO.md`。
4. 实现当前目标任务。
5. 运行相关测试与必要的质量检查，至少覆盖任务涉及范围；若可行，补充 `cargo test` / `cargo clippy --all-targets -- -D warnings`。
6. 更新 `TODO.md`、`PLAN.md` 与本文件，记录完成情况。
7. 提交 git commit，然后停止。

## 记录约定
- “进行中”：当前正在做的步骤。
- “已完成”：本轮已经完成并验证的步骤。
- “变更原因”：计划调整、阻塞、拆分任务或发现规范问题时补充说明。

## 当前状态
- 已完成：检查最新提交信息，确认最新 commit 只是在 TODO/PLAN 中把普通 callee non-resuming blocker 前移，没有新增独立的 pre-existing issue 说明。
- 已完成：定位第一个未完成任务为 `T3010b2b0`，并确认现有 `T3010b2b0 -> T3010b2b0R -> T3010b2b1` 拆分已经足够，无需继续细分。
- 已完成：实现 ordinary-frame effect propagation。
  - direct non-resuming `perform/Raise` 现在会立刻结束当前 callee frame，并把 builder 落到 dead block。
  - ordinary user call 返回后统一检查 TLS active；若 active，则当前 frame 直接向 caller 返回默认值。
  - `Nothing` 返回类型在 propagation 路径上发射 `ret void`，不再走普通 `return_bb` 的 `unreachable`。
  - `codegen_cast_as_expr` 的 runtime raise 失败路径也已接到同一套合同。
- 已完成：验证当前任务边界。
  - `cargo check -p scoopc` 通过。
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/nothing_raise_in_helper_basic.scoop` 输出与 golden 一致。
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_indirect_perform_nonresuming_call_chain.scoop` 输出与 golden 一致。
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_multi_nonresuming_raise_custom_finally.scoop` 已不再打印 `throw_alarm_unreachable`，剩余 finally/self-capture mismatch 归 `T3010b2b1`。
  - `cargo run -p scoop --features llvm -- test` 的首个失败点推进到 `effect_escape_continuation_finally_arm_raise.scoop`，与 `T3010b2b1` 描述一致。
  - `cargo test --all`、`cargo clippy --all-targets -- -D warnings` 通过。
- 已完成：更新 TODO/PLAN/memory，当前改动已准备提交。
