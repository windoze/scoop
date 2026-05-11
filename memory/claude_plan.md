# 本次执行计划

1. 读取 `TODO.md`，定位第一个标题未带 `[DONE]` 的任务，确认它就是本次唯一执行单元。
2. 检查最近一次提交是否明确提到与该任务直接相关且未完成的问题；若存在且构成当前任务前置条件，则先在 `TODO.md` 中记录为前置依赖。
3. 阅读与当前任务直接相关的代码、测试、文档与任务说明，确认约束、依赖、验收方式与禁止事项。
4. 按任务要求做最小正确改动；如遇阻塞当前任务的真实缺口或规格不匹配，不绕过，改为在 `TODO.md` 中补入最小前置任务并停止。
5. 运行与当前任务直接相关的验证；若任务本身要求更广验证，则一并执行，修复出现的问题直到通过或确认存在前置阻塞。
6. 更新 `memory/claude_plan.md` 记录关键进展与计划变化。
7. 若任务完成：在 `TODO.md` 将对应任务标题标记为 `[DONE]`，补全完成记录；仅当阶段级计划发生变化时才更新 `PLAN.md`。
8. 按仓库提交风格创建一次提交，包含本次任务涉及的全部未提交改动，然后停止，不进入下一个任务。

# 执行日志

- 已创建本计划文件，下一步读取 `TODO.md` 并定位当前任务。
- 已确认当前第一个未完成任务为 `G5-T05a：为 continuation resume driver 补齐 outcome-return continuation step core`。
- 下一步：检查最近提交是否提到与 `G5-T05a` 直接相关的未完事项，并阅读 `PLAN.md`、`EFFECT_REFACTOR.md`、`CONTINUATION_RUNTIME_REFACTOR.md` 中与 continuation step core / generated resume driver / outcome-return mode 直接相关的章节。
- 已检查最近提交：`[G4-T05R] Review ordinary callee suspend/reentry`，未发现需要在 `G5-T05a` 前新增的直接相关未完事项。
- 已运行 `cargo check -p scoopc`：当前首批错误仍集中在 `G6/G7`（ordinary effect propagation、call lowering、perform/handle lowering），没有新的 `G5` 前置阻塞直接浮现。
- 当前判断：`G5-T05a` 需要通过阅读现有 continuation/surface-resume lowering 来补“generated resume core 直接写 `EffectOutcome`”的目标形态，并通过源码验证/LLVM 测试证明不再依赖 shared `Step_F` surface-resume wrapper 作为内部内核。
- 当前实现策略已收敛为最小可落地版本：
  1. 新增内部 `outcome` resume wrapper / owner core，而不是提前重写公开 `Continuation.resume -> Step_F` surface。
  2. owner core 直接消费 `state_ref + payload(+ hidden ctx/token/outcome)`，在 resumed path 中把 complete / outward / runtime-error 直接写入 `EffectOutcome`。
  3. continuation composition（caller continuation 恢复底层 callee continuation）改为调用新的内部 `outcome` wrapper，并通过显式 `incoming_resume_token_ref` 传入底层 token，不再把 shared `surface-resume -> Step_F` 当成 resume 内核。
  4. 需要补充的配套能力：`EffectOutcome` 查询 helper（complete transport / op_tag / effect_instance_key）、`EffectOutcome <-> Step`/boundary 消费桥接所需的最小 helper，以及 frame `StateTag` 读写与 composite transport boxing/unboxing。
- 已完成的关键实现：
  - `effect_outcome.rs` 新增 `complete transport` / `signal op_tag` / `signal effect_instance_key` 查询 helper。
  - `effect_lowered/body.rs` 新增内部 outcome surface / owner outcome wrapper / owner core 的命名与生成入口。
  - owner core 新增 `RefactorCallableReturnMode::EffectOutcome`、`StateTag` 读写、direct `EffectOutcome` return、以及 `EffectOutcome -> Step` 的最小重建 helper。
  - composed call-boundary resume 已改为调用 internal outcome surface，并在 caller 侧重建 `Step` 后继续复用既有 boundary dispatch；不再直接调用 shared `surface-resume -> Step_F` 作为 resume 内核。
  - `llvm/tests.rs` 已补两条 IR 断言，用于锁定新 internal outcome surface / owner core 以及 composed resume 的 outcome-first 路径。
- 当前验证状态：
  - `cargo check -p scoopc` 已重新跑通到既有前沿；当前失败仍集中在 `G6/G7` 的普通 effect propagation、call lowering、perform/handle lowering 缺口，没有新增属于本次改动的前沿编译错误。
  - `cargo clippy -p scoopc --all-targets -- -D warnings` 失败前沿与 `cargo check -p scoopc` 一致；没有新增属于本次改动的 clippy/compile 前沿错误。
