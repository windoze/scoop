# TODO-5：effect facts purity + LIR output

> 生成时间：2026-05-21
> 计划基线：[`PLAN.md`](./PLAN.md) §4/P4-P5
> 索引：[`TODO.md`](./TODO.md)
> 当前状态：范围说明，待 `TODO-4.md` 完成后细化。

## 范围

- 让 effect facts stage 对 MIR 输入只读。
- 让 `EffectFactsStageOutput` 不再嵌套整份 `MirStageOutput`。
- 把 `effect_lowered` 收实为正式 LIR，并建立独立 `lir_facts` / query layer。
- 补齐 plain callable、effect-step callable、dynamic invoke、dispatch owner/slot、continuation/resume publication 等 LIR contract。
- 建立 LIR optimization family：local state-machine elimination、简单 higher-order wrapper 定向 inline/devirt、wrapper state folding、dead state / dead slot cleanup。

## 细化要求

- 每个小阶段后必须插入独立 review 任务。
- 细化时必须先列出 effect facts 当前修改 MIR 或嵌套 MIR 输出的所有位置。
- LIR optimization 只能是 effect/control 相关窄 pass，不得升级成全程序通用 optimizer。

## [TODO] TODO-5-INIT：初始化并细化本任务包

- 目标：
  - 分析 `PLAN.md` §4/P4-P5、`PIPELINE_REFACTOR.md` 和当前 `effect_facts` / `effect_lowered` / codegen 输入依赖的真实边界；
  - 生成本任务包的详细任务列表，覆盖 effect facts 只读化、`EffectFactsStageOutput` 收口、正式 LIR 输出、`lir_facts` 和 LIR optimization family；
  - 更新 `TODO.md` 的具体任务索引，用新生成的任务替换或扩展 `TODO-5-INIT` 所在索引行。
- 必须实现的内容：
  1. 列出 effect facts stage 当前嵌套 `MirStageOutput`、修改 MIR 输出本体或重算 MIR-derived facts 的位置。
  2. 列出 `effect_lowered` 当前输出结构、构造点、facts/query 候选项和 codegen 读取点。
  3. 把 P4-P5 拆成数量适中的实现小阶段，每个阶段必须有明确目标、修改范围、验证命令和完成条件。
  4. 在每个实现小阶段后插入独立 review 任务，review 任务必须复审前一阶段是否满足 effect facts purity 或 LIR owner 约束。
  5. 同步更新 `TODO.md` 中的具体任务索引，确保任务 ID、状态和顺序与本文件一致。
- 完成条件：
  - `TODO-5.md` 不再只是范围说明，而是包含可直接执行的详细任务列表；
  - `TODO.md` 的具体任务索引已经同步反映 `TODO-5.md` 的新任务和 `[TODO]` 状态；
  - 本任务完成记录说明为何 P4/P5 可以在同一包内推进，以及阶段间验收门禁。
- 完成记录：
  - 待填写。
