# Claude Plan

## 约束说明
- 不写入私有推理细节；本文件记录可审阅的执行计划、决策摘要、阻塞点与进度更新。

## 初始执行计划
1. 读取 `TODO.md`，确认它仅作为索引使用。
2. 按 `TODO.md` 给出的顺序读取对应 `TODO-Px.md`，定位第一个标题未带 `[DONE]` 的详细任务。
3. 查看最近一次提交，判断是否存在与该任务直接相关且未完成的问题需要并入当前任务或登记为前置任务。
4. 阅读该任务涉及的代码、测试、规范和相关文档，确认实现边界与验收条件。
5. 直接完成该任务；若出现真实阻塞，则只引入最小必要前置任务，并同步 `TODO.md` / 对应 `TODO-Px.md`；仅当阶段计划发生变化时更新 `PLAN.md`。
6. 运行与该任务相关的验证，包括必要测试，以及要求的无 warning 检查（至少覆盖本次改动影响范围）。
7. 将任务在对应 `TODO-Px.md` 中标记为 `[DONE]`，补全完成记录；若索引受影响则同步更新 `TODO.md`。
8. 提交当前任务的全部相关改动，提交信息使用任务号。

## 进度日志
- 已创建初始计划，下一步开始读取任务索引并定位首个未完成详细任务。
- 已读取 `TODO.md` 与 `TODO-P6-part2.md`：当前第一个未完成详细任务为 `P6-T03`（`TODO-P6-part2.md` 第 686 行起）。
- 已检查最新提交：`[P6-T02qb] Publish pending payload transport contract`。提交信息中未额外声明需要优先并入 `P6-T03` 的未完成问题；`P6-T03` 自身完成记录中的 blocker 也已通过 `P6-T02c/d/e/f/g/h/i/j/k/l/m/n/o/p/q/qb` 逐步前置化处理。

## 针对 P6-T03 的细化计划
1. 检查当前工作树状态，确认是否存在未提交改动需要在本次任务完成时一并纳入提交。
2. 阅读当前 refactor LLVM stage、effect_refactor ABI/query、以及现有 body/codegen 入口，确认 `P6-T03` 尚未落地的缺口。
3. 以最小改动实现 refactor LLVM body lowering：优先新增/补全 `effect_refactor/body.rs` 及其接线，确保只消费 P5/P6 已发布 handoff，而不回 legacy effect emitter 补语义。
4. 根据实现过程中遇到的真实约束决定：
   - 若 handoff 已足够，则直接完成 `P6-T03`；
   - 若仍存在无法按 contract-first 落地的缺口，则在 `TODO-P6-part2.md` / `TODO.md` 中新增最小前置任务并停止。
5. 运行任务要求的定向测试、相关 fixture、以及 `cargo clippy -p scoopc -p scoop --all-targets -- -D warnings`。
6. 更新 `TODO-P6-part2.md`（必要时同步 `TODO.md`）、刷新本文件进度日志，并提交一次原子 commit。

## 当前实现判断
- 初步阅读代码时未发现新的未跟踪 contract blocker。现状的主要缺口看起来是生产代码接线与 body emitter 本身：
  - `crates/scoopc/src/llvm/emit.rs` 当前仍在 refactor 路径上执行 ABI materialization 后继续走 legacy surface/main body lowering；
  - `crates/scoopc/src/llvm/codegen/effect_refactor/body.rs` 仍为空。
- 当前准备采取的实现方向：
  1. 在 `emit.rs` 中保留并消费 `RefactorAbiQuery`，停止仅用它做 fail-fast/声明壳层。
  2. 在 `effect_refactor/body.rs` 中实现 callable direct-entry lowering，并用 P5 state graph 驱动 CFG，而不是回 HIR/legacy effect emitter。
  3. 先让 `Call / Perform / Resume / HandleDispatch / Return / Branch / Goto / LocalRuntimeError / ResumeUnwind / Abandon` 都按 late-lowered contract 进入同一套 state graph lowering。
  4. 直线 source-slice 继续复用 canonical MIR statement lowering，但 boundary 语义全部以 published contract 为准；必要时补齐 `mir_body.rs` 对 `MemberAccess` / `StoreMember` 等 contract 节点的 lowering 支撑。

## 阻塞更新
- 2026-05-04：在真正设计 `ResumeBoundaryOnly` shared surface-resume body 时确认新的未跟踪 contract blocker：当前 handoff 已发布 `wrapper schema -> underlying route`，但没有继续发布 `underlying owner step -> wrapper step` 的 authoritative projection contract。
- 具体例子：`effect_multi_escape_indirect_direct_while.scoop` 中 shared surface schema `k5` 桥接到 handle-binder schema `k3`，再回 owner `main` 的 `StepSchema s1`；现有 handoff 只发布 caller 侧的 forward 消费（例如 `s4.c0 -> s1.c2`），没有发布 shared surface body 本身需要的 reverse/projection relation。
- 因此已按流程新增前置任务 `P6-T02qc`，并同步更新：
  - `TODO-P6-part2.md` 未完成链路与新任务正文；
  - `TODO.md` 索引；
  - `P6-T03` 依赖与完成记录。
- 由于这是当前任务的真实前置缺口，本轮不继续实现 `P6-T03` 代码，改为提交任务分解结果并停止。
