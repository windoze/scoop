# Claude Plan

## Session Goal
- 按 `TODO.md` 索引与对应 `TODO-Px.md` 明细文件，定位第一个未完成的详细任务。
- 在本次调用中只处理这一个任务：优先完整实现；若遇到真实阻塞，则按要求补充最小前置任务并同步索引后停止。

## Constraints
- `TODO.md` 只是索引；任务细节与完成状态以 `TODO-Px.md` 为准。
- 只有标题带 `[DONE]` 的任务才算完成。
- 不做开放式历史问题排查；只处理当前任务直接相关的阻塞或回归。
- 不用规避方案、兼容性垫片或缩小范围来“完成”任务。
- 如需新增前置任务，必须同时更新对应 `TODO-Px.md` 与 `TODO.md`，必要时再更新 `PLAN.md`。
- 本次调用结束前需要提交 git commit；完成一个任务后即停止，不进入下一个任务。

## Execution Plan
1. 读取 `TODO.md`，确认它引用了哪些 `TODO-Px.md` 文件以及任务顺序。
2. 逐个检查相关 `TODO-Px.md`，定位第一个标题未带 `[DONE]` 的详细任务。
3. 阅读该任务的要求、约束、依赖、验证方式，并检查最近一次提交是否有与当前任务直接相关且未收尾的问题。
4. 审视当前工作树状态，避免覆盖他人改动；仅在当前任务相关范围内工作。
5. 实现任务所需代码/测试/文档改动；若发现当前任务被真实前置问题阻塞，则先在明细 TODO 中插入最小前置任务并同步索引。
6. 运行与任务直接相关的验证；若任务涉及通用质量门禁，再运行必要的格式化、测试、`clippy` 等命令并修复问题。
7. 更新 `memory/claude_plan.md` 记录关键进展与结论。
8. 在对应 `TODO-Px.md` 中将任务标题标记为 `[DONE]` 并填写完成记录；如有必要同步 `TODO.md`，仅在阶段计划变化时更新 `PLAN.md`。
9. 按仓库提交风格创建一次 git commit，包含本次任务的全部相关改动，然后停止。

## Progress Log
- 已创建本计划文件，下一步开始读取 `TODO.md` 与详细任务文件来定位当前应执行的任务。
- 已读取 `TODO.md` / `TODO-P6.md` 并定位首个未完成详细任务为 `P6-T03`。
- 已检查最近一次提交与当前任务衔接；最新提交停在 `P6-T02i`，与当前任务直接相关，但没有额外未记录的收尾说明。
- 已审视现有 refactor LLVM 代码面：`effect_refactor/body.rs` 仍为空壳，`emit.rs` 当前对 effectful callable 仍以 fail-fast 阻止进入 legacy lowering。
- 已进一步核对 `LateLoweredStateTerminator::HandleDispatch`、`frame_schema` 的 `StateTag/CompletionTag` 系统槽位，以及现有 `TODO-P6.md` / `EFFECT_REFACTOR.md` / `effect_lowered` published handoff。
- 结论：当前不是单纯“实现量大”，而是存在一个尚未显式跟踪的前置 contract 缺口：P5/P6 handoff 还没有 authoritative 地发布 handle body / arm / finally / exit 之间的 internal completion-state lowering 协议；当前唯一可见的具体协议仍是 legacy `state_machine_emitter.rs` 中的 backend-private magic tags。
- 按用户约束，这个缺口不能由 `P6-T03` 在 backend 现场猜测或借壳 legacy 私有常量补过去。因此本次调用改为：
  1. 在 `TODO-P6.md` 中插入最小新前置任务 `P6-T02j`；
  2. 在 `P6-T03` 的完成记录中显式记录该 blocker，并把依赖补到 `P6-T02j`；
  3. 同步 `TODO.md` 索引；
  4. 不改 `PLAN.md`（阶段级计划未变化）；
  5. 提交这些任务编排更新后停止。
