# 执行计划

## 说明

你要求在执行任何代码或命令前先写入“完整思考过程和逐步计划”。我不会写入逐字内部推理，但会提供足够详细的执行计划、判断依据摘要和进度记录，便于你检查当前工作状态。

## 初始计划

1. 检查最新一次 Git 提交的信息，确认是否提到需要优先修复的既有问题。
2. 阅读 `TODO.md`，定位第一个未完成任务。
3. 阅读 `PLAN.md`，理解现有任务分解、依赖关系和当前阶段。
4. 如果首个未完成任务过大或依赖不明确：
   - 细化为更小的子任务；
   - 更新 `PLAN.md`；
   - 更新 `TODO.md`，把子任务放到正确的优先级位置；
   - 本次只执行第一个子任务。
5. 实施当前目标任务，过程中检查是否暴露新的规范不匹配、实现缺口或既有缺陷。
6. 为实现补充或调整测试，并运行相关验证命令。
7. 更新 `TODO.md` 与 `PLAN.md`，记录完成状态或阻塞原因。
8. 提交一次 Git commit，然后停止，不进入下一个任务。

## 执行原则

- 本次调用只完成一个任务或一个新拆分出的首个子任务。
- 如果遇到规范缺口、语言特性缺失、运行时错误或测试只能靠临时绕过通过，则必须先把问题写入 `TODO.md`，调整依赖顺序，再提交并停止。
- 不回退或覆盖我未创建的现有修改，除非你明确要求。

## 进度记录

- 已创建本文件，准备开始仓库检查。
- 已检查最新提交 `8fb127f5046ccc899cbb81b1e9e0743e45844e1c`，提交说明为 `[T3010a] 收口纯表达式 fragment-only op`，未在提交信息中显式提出需先修复的额外既有问题。
- 已读取 `TODO.md` / `PLAN.md`，确认当前第一个未完成任务是 `T3010b`：为跨 suspend 的复合表达式接回可恢复 continuation 片段，禁止 resume 后重放原表达式。
- 当前判断：先不立即拆分 `T3010b`；先通过最小复现和相关代码审查判断缺口边界。如果发现它仍包含多条可独立交付的问题轴，再按规则拆成更小子任务并更新 `TODO.md` / `PLAN.md`。

## 当前执行检查点

1. 运行 `effect_resume_yield_int_basic.scoop` 等最小复现，确认 resume 后是否确实重放原始 `perform` / 复合表达式路径。
2. 审查 `crates/scoopc/src/llvm/codegen/effect/state_machine_plan.rs`、`state_machine_segments.rs`、`state_machine_transform.rs` 与 `state_machine_emitter.rs`，定位跨 suspend 表达式在 plan/segment/unified machine/emitter 各阶段是如何被表示与恢复的。
3. 判断 `T3010b` 是否可通过“建立统一的 post-suspend continuation 片段合同”一次完成；如果不能，则把任务继续拆分，并更新 `TODO.md` / `PLAN.md`。
4. 若边界清晰，则直接实现、补测试、跑验证、更新文档并提交。

## 本轮结论与调整

- 结论：原 `T3010b` 对当前仓库来说仍然过大，直接实现会把两类工作混在一起：
  1. 合同层需要先冻结“恢复值要注入到哪个 consumer、在该 consumer 内位于哪条 expr path”。
  2. 执行层才是基于这份合同真正构造/消费 post-suspend continuation fragment，避免 resume 后重放原表达式。
- 额外发现：
  - `effect_resume_yield_int_basic.scoop` 在当前基线下仍先停在 `call callee`，因为 `ImmediateResume` arm 还保留 `resume_placeholder`，这属于后续 `T3009` 的直接 `resume(...)` lowering 缺口。
  - 即使补上 `resume(...)` 的专用 lowering，如果没有更细的 continuation fragment 合同，`val x = Yield.next()` / `add(Yield.next() + 1, 2)` 这类表达式在 resume landing 仍会重放原始 `perform` / operand 路径。
- 因此已把原 `T3010b` 细化为：
  - `T3010b1`：冻结 `resume_path` 合同（本轮执行）。
  - `T3010b2`：真正消费该合同，接回可执行的 post-suspend fragment（留待下轮）。

## 已完成：T3010b1

- 代码层：
  - 在 `SuspendSitePlan` / `HandleSegmentSuspendSite` / `UnifiedSuspendSite` 中新增 `resume_path` 元数据。
  - `resume_path` 由 `consumer root`（`val-init` / `expr-stmt` / `assign-lhs` / `assign-rhs` / `return-value` / `while-cond`）和 `expr frame path`（如 `call-arg#0`、`binary-lhs`、`when-arm#0-body`）组成。
  - 在 `HandlePlanBuilder` 中新增 `attach_suspend_resume_paths`，遍历 `handle.body` 与 `finally` cleanup block，为 `Perform` / `CallMaySuspend` / `CallStateMachineCallee` / `ClassCtorInit` suspend site 冻结该合同。
  - 收紧 segment/unified contract validation：需要恢复注入点的 suspend site 必须带 `resume_path`；`RuntimeRaise` / `ObjectInitAccess` / `NestedHandleBoundary` 仍禁止带该元数据。
- 测试层：
  - 新增 segment dump 测试，锁定 `resume-path=val-init -> call-arg#0 -> binary-lhs`。
  - 新增 transform 测试，锁定 `resume_path` 在 `plan -> segments -> unified machine` 之间稳定保留。
- 文档层：
  - 已把 `TODO.md` / `PLAN.md` 中原 `T3010b` 拆成 `T3010b1`（已完成）与 `T3010b2`（下一步）。

## 验证结果

- `cargo test -p scoopc resume_path -- --nocapture`：通过。
- `cargo test -p scoopc`：通过。
- `cargo clippy --all-targets -- -D warnings`：通过。
- `cargo test --all`：通过。

## 待提交前检查

1. 再确认 `TODO.md` / `PLAN.md` / 本文件的状态描述与实际代码一致。
2. 检查工作区，避免把用户已有的 `run_agent.sh` 非本任务改动混入提交。
3. 只提交本轮子任务 `T3010b1` 的相关改动，然后停止。
