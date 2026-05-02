## 当前执行计划

1. 读取 `TODO.md`，把它当作索引使用，不在这里判断任务细节是否完成。
2. 按 `TODO.md` 引用顺序检查对应的 `TODO-Px.md`，定位第一个标题未带 `[DONE]` 的详细任务。
3. 查看最近一次提交，确认是否存在与该任务直接相关且明确未完成的问题；如果它构成当前任务前置条件，则先按规范补充前置任务并同步索引。
4. 阅读当前任务涉及的代码、测试、规范与依赖关系，只收集完成该任务所需的最小上下文。
5. 直接实现当前任务；若遇到阻塞当前任务的真实缺口或规格不匹配，不做变通，而是在相应 `TODO-Px.md` 中增加最小前置任务，并同步 `TODO.md`。
6. 运行与当前任务相关的验证；至少包含任务要求的测试，并尽量补充必要的回归验证；若改动影响构建或 lint，则运行对应检查。
7. 更新任务记录：在对应 `TODO-Px.md` 中把任务标题改为 `[DONE]`（仅在实际完成时），填写完成记录；如任务顺序、标题、依赖或文件引用有变化，则同步更新 `TODO.md`；仅在阶段计划确有变化时更新 `PLAN.md`。
8. 检查工作区是否存在本次任务需要一并提交的未提交改动，避免遗漏恢复执行时遗留的相关文件。
9. 使用符合仓库风格的提交信息创建一次 git 提交，然后停止，不继续下一个任务。

## 记录约定

- 这里记录可公开的计划、进度、阻塞和验证结果，不记录内部逐字思维。
- 如果计划发生变化、发现阻塞、完成关键实现、完成验证或完成提交，会继续追加更新。

## 进度更新（2026-05-03）

- 已读取 `TODO.md` 索引，并在 `TODO-P5.md` 中确认首个未完成详细任务是 `P5-T05`：物化 `Step_F` enum、canonical dynamic `invoke`、continuation object、internal resume interfaces，并按 `ImplPlan` 完成 boundary lowering。
- `P5-T04R` 已完成，因此当前应直接实现 `P5-T05`，不能跳到 review 或后续任务。
- 接下来会先检查最近一次提交是否留下与 `P5-T05` 直接相关的未完成项，并检查工作区状态；随后只阅读完成 `P5-T05` 所需的设计文档、late-lowered IR/building 代码和现有测试入口。

## 当前轮次追加（2026-05-03）

- 先重新核对 `TODO.md` 与对应 `TODO-Px.md`，确认首个未完成详细任务仍然是 `P5-T05`，再开始实现，避免基于过期状态工作。
- 如果最新提交直接提到 `P5-T05` 的未完成边界、回归或前置缺口，优先把它视为当前任务的一部分或显式前置。
- 只收集完成 `P5-T05` 所需的最小上下文：相关设计文档、continuation/boundary lowering 代码、invoke/resume 接口、以及现有 fixture 或回归测试。
- 一旦确认实现方案，会继续在本文件记录关键实现点、验证命令和最终提交信息。

## 当前发现（2026-05-03）

- 已重新核对：`TODO.md` 与 `TODO-P5.md` 一致，首个未完成详细任务仍是 `P5-T05`；最新提交 `[P5-T04R]` 只完成 review，没有额外声明需要先插入的新前置任务。
- 当前 `crates/scoopc/src/effect_lowered/` 已具备：`Step`/continuation shell、segmentation skeleton、frame schema、owner/resume state、drop state 与稳定 dump；但还没有把 boundary 真正物化为统一的 `Step`/continuation lowering plan。
- 当前 `builder.rs` 里 resume interface 仍按 `StepSchema` 一次性生成，尚未按 effect family 分组；这与 `P5-T05` 对 internal resume interfaces 的要求不符，属于本任务必须补齐的内容。
- 计划中的最小实现方向：
  1. 在 late-lowered IR 中补出可被 dump/测试/后续 P6 消费的 boundary lowering 计划结构，显式覆盖 `perform` / effectful `call` / `resume` / runtime error / outward nested-handle`；
  2. 为 `ConcreteOpKey` 建立稳定的 effect-family 分组键，并把 resume interface / continuation object 改成“按 family 分组、但方法集完整”；
  3. 为 `ImplPlan` 三档、one-shot/runtime error、`()` payload/resume tuple 增补针对性测试；
  4. 通过后再更新 `TODO-P5.md`、`TODO.md`、本文件，并提交一次原子 commit。

## 阻塞结论（2026-05-03）

- 在继续实现 `P5-T05` 前确认到一个真实前置缺口：当前 P4 `StepSchema`/facts 只把源码 `Continuation.resume(...)` 的 ordinary `Raise<RuntimeError>` 纳入 handoff，但没有明确覆盖 compiler-generated continuation object 的 one-shot 重复恢复路径。
- 这会直接阻塞 `P5-T05`：若不先补齐 P4 handoff，P5 只能临时发明 pseudo case、隐藏错误通道或 backend trap，违反 `EFFECT_REFACTOR.md` 与用户要求。
- 已按“最小新前置任务”原则把该缺口写成 `P4-T05a`，放入 `TODO-P4.md` 并同步 `TODO.md`；同时把 `P5-T05` 的依赖更新为 `P4-T05a，P5-T04R`。
- 因此本轮不继续实现 P5 代码，改为提交这次任务重排与阻塞记录；下一轮首个未完成详细任务将变为 `P4-T05a`。
