# TODO-3：HIR barrier + hir_facts

> 生成时间：2026-05-21
> 计划基线：[`PLAN.md`](./PLAN.md) §4/P2
> 索引：[`TODO.md`](./TODO.md)
> 当前状态：范围说明，待 `TODO-2.md` 完成后细化。

## 范围

- 把 `AST -> HIR` 变成 semantic frontend barrier。
- 从 `LoweredHir` 拆出独立 `hir_facts`。
- 重新分配 `LoweredHir` side tables、`TypedHirEffectContracts`、`ProgramFacts` 的职责。
- 在 HIR 屏障内收口 declaration legality：`@CallingConvention` non-generic、global roots non-generic、top-level `var` storage policy gate。

## 细化要求

- 每个小阶段后必须插入独立 review 任务。
- 细化时必须列出当前 `LoweredHir` 的 side table 清单和下游使用点。
- 通过 HIR 屏障后，后续阶段不得再报普通源码语义错误。

## [TODO] TODO-3-INIT：初始化并细化本任务包

- 目标：
  - 分析 `PLAN.md` §4/P2、`PIPELINE_REFACTOR.md` 和当前 HIR/typecheck/effect contract 相关代码的真实职责分布；
  - 生成本任务包的详细任务列表，覆盖 HIR semantic barrier、`hir_facts` 拆分、declaration legality gate 和后续阶段错误边界；
  - 更新 `TODO.md` 的具体任务索引，用新生成的任务替换或扩展 `TODO-3-INIT` 所在索引行。
- 必须实现的内容：
  1. 列出 `LoweredHir` 当前字段、side tables、构造点和下游读取点。
  2. 列出 `TypedHirEffectContracts`、`ProgramFacts` 与 HIR facts 候选项的当前使用点。
  3. 把 P2 拆成数量适中的实现小阶段，每个阶段必须有明确目标、修改范围、验证命令和完成条件。
  4. 在每个实现小阶段后插入独立 review 任务，review 任务必须复审前一阶段是否满足 HIR barrier 约束。
  5. 同步更新 `TODO.md` 中的具体任务索引，确保任务 ID、状态和顺序与本文件一致。
- 完成条件：
  - `TODO-3.md` 不再只是范围说明，而是包含可直接执行的详细任务列表；
  - `TODO.md` 的具体任务索引已经同步反映 `TODO-3.md` 的新任务和 `[TODO]` 状态；
  - 本任务完成记录说明 HIR facts 拆分依据和仍需实现阶段验证的风险。
- 完成记录：
  - 待填写。
