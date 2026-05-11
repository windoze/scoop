## 当前执行计划

说明：按安全约束，本文件记录完整的高层执行计划、关键观察、决策依据和进度更新；不记录隐藏推理细节。

1. 读取 `TODO.md`，按标题是否带有 `[DONE]` 判定完成状态，锁定第一个未完成任务。
2. 查看最近一次提交信息，确认是否存在与该任务直接相关且明确未完成的事项；若有，则将其视为当前任务范围或在 `TODO.md` 中登记为前置依赖。
3. 阅读当前 review 任务涉及的代码、测试、规范和上一任务完成记录，确认本轮需要人工复核的实现边界与验证要求。
4. 运行面向当前 review 任务的验证：代码搜索、必要文件复核、`cargo check -p scoopc`，并确认失败前沿仍停在后续任务缺口，而不是 `G5` 回退或新引入问题。
5. 若 review 发现当前实现否定了上一任务的完成结论，则在本轮直接修复；若发现新的真实前置阻塞，则按要求写回 `TODO.md` 并停止。
6. 若 review 通过，则更新 `TODO.md`：将当前 review 任务标记为 `[DONE]`，补全 completion record；仅在阶段计划变化时才更新 `PLAN.md`。
7. 更新本文件中的关键结论与验证结果，随后创建一次包含当前未提交更改的 Git 提交并停止。

## 进展记录

- 已读取 `TODO.md`，确认首个未完成任务是 `G5-T06R：Review continuation object model / generated resume driver，确认 owner 已迁回 codegen`。
- 已检查最近一次提交信息：`[G5-T05a] Add outcome-return continuation step core`。该提交信息未声明额外、未登记且直接阻塞 `G5-T06R` 的 unfinished issue。
- 已确认 `PLAN.md` 的阶段顺序未变化；当前无需调整 phase-level plan。

## 当前细化计划（G5-T06R）

1. 复核 `TODO.md` 中 `G5-T06` 的完成记录，提炼本次 review 需要验证的三个核心结论：continuation 字段集合、generated resume helper owner、thread resume integration 边界。
2. 阅读以下实现位置，确认 active implementation 未回退到 runtime-owned continuation policy：
   - continuation layout/type descriptor 生成处
   - generated resume helper 生成处
   - thread resume integration 处
3. 对实现源码做定向 grep，确认仓库活跃代码中没有重新出现 deleted runtime continuation/effect bridge 名字。
4. 运行 `cargo check -p scoopc`，确认失败前沿仍然只位于 `G6/G7` 缺口，而不是 `G5` 回退。
5. 若 review 通过，更新 `TODO.md` 的 `[DONE]` 标记与 completion record，并同步本文件中的最新进展。
6. 执行 git 状态/差异/日志检查，按仓库风格创建一次以 `G5-T06R` 为主题的提交，然后停止。

## 最新进展

- 已确认当前任务是 review 任务，不默认新增实现；只有在 review 发现上一任务的完成结论被否定时，才会在本轮直接修补。
- 已完成对 `effect_lowered/{layout,types,body,value}.rs`、`intrinsics/thread.rs`、`runtime_{abi,symbols}.rs`、`llvm/tests.rs` 的人工复核。
- 关键结论：
  1. continuation authoritative 字段集合已经稳定为 `header/resumed/resume_state_tag/captured_effect_ctx_ref/state_ref/step_fn/resume_word/resume_gc_ref/captured_callee_suspend_state_ref`；未发现 stable handle、native handler snapshot、`release_fn` 或 replay-state 残留。
  2. generated resume driver 已明确迁回 codegen：`emit_generated_continuation_resume_driver(...)` 使用 `cmpxchg` one-shot、显式 payload store、显式读取 captured ctx/token/state/step_fn，并以 `ScoopEffectOutcome *outcome` 调用 owner step core；complete path 直接回写 answer slot。
  3. active cross-thread resume integration 仍走 `effect_lowered/value.rs` 的 thunk + generic thread spawn/join substrate；`intrinsics/thread.rs` 中兼容 helper 入口未发现 active refactor path 调用点，不构成当前 review blocker。
- 验证结果：
  - 对 `crates/scoopc/src`、`runtime/c`、`sysroot` grep `scoop_continuation_|scoop_callee_suspend_state_|captured_handler_stack_top|pending_continuation|scoop_effect_handler_stack_|scoop_effect_outcome_`：无命中。
  - `cargo fmt --check`：通过。
  - `cargo check -p scoopc`：失败前沿仍停在后续 `G6/G7` 缺口；未出现 `G5` continuation object model / generated resume driver 回退问题。
  - `cargo clippy -p scoopc --all-targets -- -D warnings`：失败前沿与 `cargo check -p scoopc` 一致；未新增本任务范围内的 lint/warning 问题。
- 已将 `G5-T06R` 在 `TODO.md` 中标记为 `[DONE]` 并写回完成记录。
- 下一步：检查最终 diff 与 git 状态，创建一次 `G5-T06R` 提交，然后停止。
