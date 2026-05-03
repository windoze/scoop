# 当前执行计划

## 约束说明
- 这里记录可公开的执行计划、关键决策、阻塞与进度，不记录内部推理细节。
- 本次目标：先定位 `TODO.md` 所指向的第一个未完成详细任务，只完成这一个任务，然后停止。

## 初始计划
1. 读取 `TODO.md`，确认任务索引结构与详细任务文件引用。
2. 按索引顺序读取对应 `TODO-Px.md`，定位第一个标题未标记 `[DONE]` 的详细任务。
3. 检查最近一次提交信息，确认是否存在与该任务直接相关且未完成的问题；若有，按要求将其视为任务一部分或前置依赖。
4. 阅读该任务的详细要求、约束、验证标准与完成记录。
5. 检查工作区现状，避免覆盖非本次任务的已有改动。
6. 实施任务所需的最小正确修改；若遇到真实阻塞，新增最小前置任务并同步 `TODO.md`。
7. 运行与该任务直接相关的验证；若任务涉及通用质量门禁，再补充运行格式化、测试、`clippy` 等必要检查。
8. 更新 `TODO-Px.md` 中该任务标题为 `[DONE]` 并填写完成记录；如索引受影响，同步更新 `TODO.md`；仅在阶段计划变化时更新 `PLAN.md`。
9. 把本次执行的关键进展补充到本文件。
10. 按仓库约定创建一次 git 提交，然后停止，不进入下一个任务。

## 进度记录
- 已创建本计划文件，准备开始读取任务索引并定位当前应执行任务。
- 已读取 `TODO.md` 与 `TODO-P6.md`，确认首个未完成详细任务为 `P6-T03`：按 P5 state graph / boundary contract 完成 refactor LLVM body lowering。
- 已检查最新提交与当前任务关系：最新提交为 `P6-T02R` review 完成，不包含需要额外前插的新 unfinished issue。
- 当前下一步：检查 `crates/scoopc/src/llvm/codegen/effect_refactor/**`、现有 LLVM body emission 入口，以及 P5 late-lowered representation，确认要替换/新建的最小实现边界。
- 已确认当前 refactor 路径现状：`llvm/emit.rs` 只在 stage 边界做 fail-fast 和 ABI materialization，实际函数声明与 body emission 仍走 `declare_top_level_fun(...)` + `codegen_top_level_mir_fun(...)` / `codegen_top_level_fun(...)` 旧流程。
- 这意味着 `P6-T03` 的核心工作至少包括：
  1. 建立 refactor body emitter 与 verifier；
  2. 为 refactor callable 声明/生成专用 LLVM 函数体；
  3. 让 refactor build/run 能从入口函数实际调用这套 lowering，而不是继续经旧主体路径。
- 当前正在确认 `Step_F` / direct-entry / dynamic-invoke ABI 的实际调用接线点，以便选择最小正确改动面。
- 对 `tests/fixtures/run-pass/effect_resume_if_else_branch_single_perform.scoop` 做定向复现后发现新的硬阻塞：
  - refactor `build` 与 `dump-effect-lowered` 都在 P5 late-lowering 阶段失败；
  - 错误为“无法把 `scoop.core.Raise.raise` 从 input StepSchema 投影到 output StepSchema”；
  - 因此当前还不能开始 `P6-T03` 的 LLVM body emitter 验证，这属于直接阻塞当前任务的前置问题。
- 当前下一步变更为：
  1. 搜索该错误来源与相关现有 TODO；
  2. 判断该问题是否已被跟踪；
  3. 若未跟踪，则在合适的详细 TODO 文件中插入最小前置任务，并同步 `TODO.md`；
  4. 如能在本次以小改动直接修复，也优先尝试修复后再回到 `P6-T03`。
- 已完成 blocker 归档：
  - 在 `TODO-P5.md` 末尾新增未完成任务 `P5-T07a`，专门跟踪“pure caller 经 call boundary 消费 compiler-generated runtime-error case 时的 late-lowering case 投影”修复；
  - 已同步更新 `TODO.md` 索引，把 `P5-T07a` 插入到 `P5-T07R` 与 `P6-T01` 之间；
  - 已把 `TODO-P6.md` 中 `P6-T03` 的依赖改为 `P6-T02R，P5-T07a`；
  - 已补充 `P5-T07R` 完成记录，说明该 post-review blocker 的来源与为什么必须先修 P5 handoff。
- 本次不继续实现代码修复；按工作流，这里应提交“新增前置任务并停止”，由下一次调用从 `P5-T07a` 开始执行。
