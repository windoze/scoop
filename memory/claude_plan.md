# 本轮执行计划

## 说明

按要求先写入一份可审计的执行计划与推理摘要。这里记录的是面向实现的决策摘要、检查步骤、风险点与后续更新，不包含与任务无关的冗余内部草稿。

## 当前目标

完成 `TODO.md` 中第一个未完成任务；如果在执行前发现最新提交中提到的遗留问题，则先修复这些问题；完成后更新计划与任务文档，提交 Git commit，然后停止。

## 预定步骤

1. 查看最新一次 Git 提交信息，确认是否明确提到已有缺陷、回归、`FIXME`、临时措施或待修问题。
2. 如果最新提交提到已有问题，先定位并修复这些问题，再运行相关测试验证。
3. 阅读 `TODO.md`，找出第一个未完成任务。
4. 评估该任务复杂度：
   - 如果可以在本轮完整落地，则直接实施。
   - 如果过大或存在明确前置依赖，则将其拆分到 `PLAN.md` / `TODO.md`，调整顺序后只做新的第一个子任务。
5. 在实现前阅读相关代码、测试、规格或文档，确认当前行为与目标行为的差异。
6. 修改代码并补充/调整测试，保证实现符合规格，避免引入规避性方案。
7. 运行必要验证：
   - 与改动直接相关的测试；
   - 必要时运行更广泛测试；
   - 按要求检查无告警，优先考虑 `cargo clippy --all-targets -- -D warnings`。
8. 更新文档状态：
   - 在 `TODO.md` 中标记本轮完成项；
   - 在 `PLAN.md` 中记录当前状态、后续依赖与调整；
   - 在本文件中记录关键进展与计划变更。
9. 查看工作区变更，确认只包含本轮相关修改，不回退用户已有改动。
10. 提交 Git commit，提交信息聚焦本轮任务，然后停止。

## 执行约束

- 不接受绕过实现、夹具特判或临时兼容层来冒充完成。
- 若遇到规格缺口或实现边界，必须把该问题前置成 `TODO.md` 任务并调整依赖顺序，然后停止。
- 不回退非本人改动；若遇到冲突性未知改动，需要先理解其影响再决定是否继续。

## 进展记录

- 2026-04-17：已创建本计划文件，下一步开始检查最新提交信息。
- 2026-04-17：已检查最新提交 `8714a0611aec8c05c7c82b2a864f3c6fe02aef9c`（`[T3009aR] Fix non-block immediate-resume lowering review gap`）。
- 2026-04-17：已读取 `TODO.md` / `PLAN.md`。当前文件中的第一个未完成任务是 `T3010b2b`。
- 2026-04-17：最新提交的任务记录与 fixture 注释明确提到一个未解决问题：`tests/fixtures/run-pass/async_await_minimal_int_basic.scoop` 现可成功 build，但运行时会在打印 `before` 后异常退出。按用户要求，该问题需在继续常规 TODO 任务前先评估并尽量修复；若问题过大，则需把它细化为更明确的前置任务并更新 `TODO.md` / `PLAN.md`。
- 2026-04-17：下一步计划：
  1. 复现 `async_await_minimal_int_basic.scoop` 的运行期失败并收集崩溃位置。
  2. 判断该问题是否是可在本轮完整收口的单一缺陷，还是需要拆成新的前置子任务。
  3. 若可收口，则实现修复并补测试；若不可收口，则按规则调整 `TODO.md` / `PLAN.md` 后提交并停止。
- 2026-04-17：已复现并定位第一层崩溃根因：
  - `state_machine_emitter` 把 suspended continuation 暂存在 frame 的 `resume_gc_ref` 槽位；
  - 但 `step_fn` 每次重入都会先用调用参数刷新 `resume_gc_ref`，arm 重入时传入的是 `null`，导致 continuation 被清空；
  - 后续 `ArmResumeMatchedSite` 对空 continuation 做字段访问，运行时在地址 `0x40` 发生 `SIGSEGV`。
- 2026-04-17：已实现两处生产修复并补上 IR 回归测试：
  - 为 frame 追加独立的 runtime-only continuation 槽位，Suspend/arm resume/escape continuation 都改为读写该槽位；
  - `HandleStateOp::Perform` 的 emitter 只再发射 payload，不再把整棵 `perform` 表达式回送给 `codegen_perform_expr` 以避免 TLS perform slot 被默认值覆盖。
- 2026-04-17：定向 IR 单测 `async_await_ir_preserves_continuation_slot_and_perform_payload` 已通过。
- 2026-04-17：定向运行结果表明崩溃已消失，但语义缺口仍在：
  - `async_await_minimal_int_basic.scoop` 当前输出为 `before / done / 0`；
  - `effect_resume_yield_int_basic.scoop` 当前输出为 `before / in_handler / in_handler / done / 41`；
  - 这与 `T3010b2b` 的描述一致：resume 后仍在重放 suspend 前路径，而不是只继续剩余 tail。
- 2026-04-17：计划调整：继续直接推进 `T3010b2b`，检查 resume landing state 的 HIR 改写与 terminator/state transition 是否仍保留了原 suspend site 或错误的 handle-return 路径。
- 2026-04-17：已继续收口并完成 5 个生产修复，分别覆盖：
  - continuation 不再复用 frame 的 `resume_gc_ref` 槽，而是写入独立 runtime continuation 槽；
  - `HandleStateOp::Perform` 仅重发 payload，避免覆盖 TLS perform slot；
  - continuation 新增独立 `resume_state_tag`，恢复时不再重放 arm entry；
  - dispatch 前仅清 active flag，不提前清 perform slot；
  - 纯 handle-result exit state 与 cleanup 路径在跨 state 跳转前先把结果落到 frame carrier，避免 `last_value` 丢失。
- 2026-04-17：上述修复后，以下定向运行已恢复为预期通过：
  - `effect_resume_yield_int_basic.scoop`
  - `effect_resume_finally_normal.scoop`
  - `async_await_minimal_int_basic.scoop`
  - `effect_resume_nested_block_single_perform.scoop`
- 2026-04-17：当前 `T3010b2b` 尚未结束。直接命中的剩余失败项是 `effect_resume_while_body_single_perform.scoop`，报错 `unsupported_main_body: comparison rhs`。
- 2026-04-17：下一步聚焦 unified state machine plan 层，而不是 emitter 打补丁：
  1. 检查 `state_machine_plan.rs` 中 `record_expr_reads` / `compute_capture_sets` / `build_frame_layout`；
  2. 确认 `while` 条件里 suspend 后继续求值所需的 local/param（如 `limit`）是否被遗漏捕获；
  3. 修复后优先重跑 while 相关 fixtures；
  4. 若通过，再完成 `TODO.md` / `PLAN.md` 状态更新、完整测试、commit。
- 2026-04-17：已确认 `comparison rhs` 不是 emitter 层面的比较算子缺失，而是 unified plan / frame 合同的三处生产缺口叠加：
  - `while` / `if` 的纯 branch condition 没有进入 `state.reads`，导致 resume capture 计算漏掉下一轮条件依赖（例如 `limit`）；
  - outer local / param 首次建 frame slot 时偷用了 `VarRef.expr.ty`，把 `limit` 之类的 authoritative 声明类型错记成 `Any`；
  - handle 入口调用 `step_fn` 前没有把当前 env 中的 outer local 初值拷入 effect frame，修完类型后也会继续读到零初始化值。
- 2026-04-17：已实现上述三处修复：
  - `HandlePlanContext` 新增 authoritative local metadata，outer slot / capture slot 统一按声明环境回填 type + mutability；
  - `build_while` / `build_expr(If)` 在设置 branch terminator 前显式记录整棵条件表达式的 reads；
  - `codegen_handle_expr_via_state_machine` 在初次进入 `step_fn` 前，按 contract.frame slots 把当前 env 中可见的 outer locals/params 复制进 frame。
- 2026-04-17：已把临时 dump 测试改为正式回归单测 `source_plan_preserves_outer_slot_types_and_while_cond_reads_for_resume_capture`，锁定：
  - outer param `limit` 的 frame slot type 必须保持声明类型；
  - while body perform site 的 capture set 必须包含下一轮条件所需的 `limit` 与 `i`。
- 2026-04-17：已定向验证通过：
  - `cargo test -p scoopc source_plan_preserves_outer_slot_types_and_while_cond_reads_for_resume_capture -- --nocapture`
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_resume_while_body_single_perform.scoop`
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_resume_while_nested_perform.scoop`
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_resume_while_nested_block_perform.scoop`
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_resume_if_then_branch_single_perform.scoop`
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_resume_if_else_branch_single_perform.scoop`
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_resume_nested_block_single_perform.scoop`
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_resume_mixed_raise_dispatch.scoop`
- 2026-04-17：已建立 10 条相关 run-pass fixtures 的临时子集，并用 `cargo run -p scoop --features llvm -- test --fixtures <tmpdir>` 做 golden 回归，结果 `fixtures: ok (10)`。
- 2026-04-17：`effect_resume_mixed_source_path_matrix.scoop` 现已不再命中 comparison/member/binary 类 post-suspend tail 错误，而是继续前进到既有 `T3012` 跟踪的 `value coercion` 缺口；这说明当前 round 的 post-suspend tail 主阻塞已基本收口，剩余 expected-context/coercion 交给后续任务。
## 2026-04-17 本轮接续计划（T3010b2b 收口）

### 当前判断
- 交接代码已经修复 `T3010b2b` 相关的多个真实缺口：branch condition 读取集遗漏、outer slot 类型退化为 `Any`、handle 首次进入 step_fn 前未把 outer locals/params 写入 frame、以及 continuation 跨线程恢复时 `resume_state_tag` 误写 `state_tag` 的 runtime 回归。
- 相关新增/修正验证已经覆盖若干 targeted fixture、`cargo test --all` 与 `cargo clippy --all-targets -- -D warnings`，但在最近两处 fixture expectation 调整后，还没有重新跑完一次完整 `cargo run -p scoop --features llvm -- test`。
- 目前已知剩余失败中，`effect_resume_mixed_source_path_matrix.scoop` 和 `effect_escape_continuation_arm_performs_outer_effect.scoop` 都前进到了 `T3012 value coercion`，不应继续在 `T3010b2b` 范围内打补丁。

### 本轮执行计划
1. 重新运行全量 fixture runner：`cargo run -p scoop --features llvm -- test`，确认最新基线下是否还有新的 unexpected success/failure。
2. 如果全量 runner 通过，或剩余失败都明确属于已跟踪后续任务，则将 `T3010b2b` 标记完成，并更新 `PLAN.md`/`TODO.md` 的依赖与当前状态说明。
3. 如果全量 runner 暴露新的未跟踪真实缺口，则按依赖顺序先更新 `TODO.md`/`PLAN.md`，把新任务前置，再停止在“调整计划并提交”的状态。
4. 完成文档更新后检查工作区，提交本轮改动，并停止，不继续处理下一个任务。

### 本轮需要特别关注的信号
- `delegated_property_observable_raise_does_not_poison_mutex.scoop` 现已改为 `EXPECT: pass`，需要确认它在全量运行中稳定通过。
- `effect_escape_continuation_arm_performs_outer_effect.scoop` 已改为 `EXPECT: fail`，需要确认失败原因稳定表现为 `value coercion`，没有新的回退。
- 若全量 runner 暴露 effect/source replay 新断点，需要先判断是否属于 `T3010b2b` 漏修，还是应归并到现有后续任务（尤其 `T3012`）。

### 2026-04-17 全量 fixture 复跑结果
- `cargo run -p scoop --features llvm -- test` 当前首个失败点是 `tests/fixtures/run-pass/effect_escape_continuation_finally_arm_raise.scoop`。
- 直接运行该 fixture 时，实际输出为：
  - `before`
  - `body_start`
  - `arm_start`
  - `arm_unreachable`
  - `result`
  - `0`
  - `done`
- 这说明 `Raise.raise(77)` 在 arm body 内没有向外传播，也没有先执行 `finally`，而是像普通返回值一样继续落到 arm tail。
- 进一步复跑 `tests/fixtures/run-pass/effect_resume_finally_arm_raise.scoop`，也出现相同症状：`Raise.raise(77)` 后继续打印 `arm_unreachable` / `body_unreachable`，再次确认问题并不局限于 escaped continuation。
- 再复跑 `tests/fixtures/run-pass/effect_multi_nonresuming_raise_custom_finally.scoop`，可以看到 sibling `Raise.raise` arm 在 custom arm body 中错误自捕获，`mixed_finally` 也没有按预期在向外传播前执行，说明当前缺口是更前置的“arm body 内 non-resuming effect 外传 / self-inactive / cleanup”语义问题。

### 决策
- `T3010b2b` 已完成的大部分代码修复仍然有效：resume tail capture、outer slot metadata、初始 frame seed、runtime `resume_state_tag` 哨兵都已验证能推进多条相关 fixture。
- 但要把 `T3010b2b` 正式关闭，必须先修复 arm body 内 non-resuming effect 的统一语义；否则全量 LLVM fixture 基线仍不成立。
- 因此本轮不再继续给 `T3010b2b` 打补丁，而是更新 `TODO.md` / `PLAN.md`，把该 blocker 拆成更前置的子任务并调整依赖顺序，然后提交并停止。

### 文档同步状态
- 已在 `TODO.md` 中把新的前置 blocker 拆成 `T3010b2b1`，并把原 `T3010b2b` 改为依赖该子任务；同时把 `T3015` 的描述收窄到 escaped continuation 的剩余 lifetime / delayed-resume 语义。
- 已在 `PLAN.md` 中补写 2026-04-17 更新说明，记录全量 fixture 新首个失败点与任务拆分，并把“当前执行顺序”更新为从 `T3010b2b1` 开始。
- 下一步仅剩整理当前 diff 并提交；本轮到此停止，不继续实现 `T3010b2b1`。
