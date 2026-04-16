# 执行计划

## 约束说明
- 按要求先写入本文件，再进行仓库检查与命令执行。
- 这里记录的是可审计的执行计划、判断依据摘要和进度更新，不包含逐字内部推理。
- 本轮目标：先处理最新提交中提到的既有问题；若无额外问题，则完成 `TODO.md` 中第一个未完成任务；完成后测试、更新文档、提交并停止。

## 初始步骤
1. 检查最新一次 git 提交信息，判断是否明确提到仍需修复的既有问题。
2. 读取 `TODO.md`，定位第一个未完成任务。
3. 如任务过大，拆分为更小子任务，并同步更新 `PLAN.md` 与 `TODO.md`。
4. 实施当前目标任务。
5. 运行相关测试与必要的 `cargo fmt`、`cargo test`、`cargo clippy --all-targets -- -D warnings`。
6. 更新 `TODO.md`、`PLAN.md` 与本文件，记录完成状态或阻塞原因。
7. 使用清晰提交信息创建 git commit，然后停止。

## 进度日志
- 2026-04-16：初始化计划文件，待检查最新提交与待办任务。
- 2026-04-16：已检查最新提交 `b1d798bf2617066a049ded95387a286d4530969e`，提交说明仅说明把 `T3009` 后移到前置依赖之后，未额外列出需先修的既有问题。
- 2026-04-16：已读取 `TODO.md` / `PLAN.md`，确认第一个未完成任务是 `T3010`：收口 unified state machine 的 expression 分片策略，移除不可独立求值的 fragment op。

## T3010 细化计划
1. 审查 `crates/scoopc/src/llvm/codegen/effect/state_machine_plan.rs` 与 `state_machine_emitter.rs`，定位哪些 `HandleStateOp` 只是为复合表达式整棵重算服务的 fragment-only 伪执行 op。
2. 运行最小复现与必要的失败子集，确认当前失败是否覆盖 `member access target`、`comparison lhs/rhs`、`equality lhs`、`integer binary op lhs` 以及 resume landing 重放原始 `perform`。
3. 修改 plan builder，使其仅为真正独立可执行或承担 suspend/resume 边界的表达式生成生产 op；同步调整 emitter，删除对 fragment-only op 的容错依赖。
4. 增加或更新定向测试，锁定“不会先拆 fragment 再整棵重算”的合同。
5. 运行 `cargo fmt`、相关测试、`cargo clippy --all-targets -- -D warnings`、`cargo test --all`，必要时补跑 LLVM fixture。
6. 更新 `TODO.md`、`PLAN.md` 与本文件，记录完成情况后提交并停止。

## 当前调整
- 2026-04-16：根据代码审查与最小复现，确认原 `T3010` 同时包含两类工作：
  1. 纯表达式在消费型位置和表达式语句中被无意义拆成 fragment-only op，导致 expected context 丢失与 fragment-only unsupported_main_body。
  2. 真正跨 suspend 的复合表达式缺少可恢复 continuation 片段，resume 后会重放原表达式。
- 2026-04-16：已按上面两类问题把 `T3010` 拆成 `T3010a`（本轮执行）与 `T3010b`（后续继续），并同步更新 `TODO.md` / `PLAN.md`。
- 2026-04-16：最小复现结果：
  - `effect_resume_yield_int_basic.scoop` 当前仍报 `暂不支持的 main 代码生成节点：call callee`，对应已跟踪的 `T3009` / `T3010b` 闭环缺口。
  - `std_test_assertions_basic.scoop` 当前报 `enum variant ctor call without expected enum type`，属于 `T3010a` 要先消掉的“消费型位置提前拆片并丢失 expected context”问题。
- 2026-04-16：已完成 `T3010a` 代码改动：
  - `HandlePlanBuilder` 新增 suspend-subtree 判定。
  - 对不含 suspend 子树的 initializer / assign / return / while/if condition，不再生成前置 standalone expr op。
  - 对表达式语句中的复合表达式，只在 suspend 子树上递归，不再为纯 callee / receiver / operand 生成 fragment-only op。
  - 已补两条定向单测，锁定纯 initializer、纯 call arg、纯 if condition 的合同。
- 2026-04-16：验证结果：
  - `cargo test -p scoopc source_plan_skips_pure_initializer_fragment_ops_in_consumer_positions -- --nocapture` 通过。
  - `cargo test -p scoopc source_plan_keeps_only_whole_call_for_pure_statement_args_and_pure_if_condition -- --nocapture` 通过。
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/std_test_assertions_basic.scoop` 通过。
  - `cargo test --all` 通过。
  - `cargo clippy --all-targets -- -D warnings` 通过。
  - `cargo run -p scoop --features llvm -- test` 首个失败点推进到已跟踪的 `T3015` fixture `effect_escape_continuation_arm_performs_outer_effect.scoop`。
- 2026-04-16：已额外确认 `effect_escape_continuation_nested_arm_indirect_performs_outer.scoop` 也落在同一 `T3015` 语义缺口簇，现象是 binder 变成 `0` 且继续执行 `unreachable_arm`，没有命中外层 handler。
