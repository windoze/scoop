# TODO-4：MIR boundary + MIR pass pipeline

> 生成时间：2026-05-21
> 计划基线：[`PLAN.md`](./PLAN.md) §4/P3
> 索引：[`TODO.md`](./TODO.md)
> 当前状态：范围说明，待 `TODO-3.md` 完成后细化。

## 范围

- 收口 `MirStageOutput`，使其只发布 MIR-owned 产物。
- 在 P2 已清理 HIR source-site contract 泄漏的基础上，继续收口 optional `MaterializedMir`、root inventories、snapshot binding 与 pass artifacts。
- 建立独立 `mir_facts` / pass artifacts 查询面。
- 把 escape analysis、devirtualization、summary-driven inlining、closure simplification、cleanup / summary refresh 重排成显式 MIR pass pipeline。
- 删除 HIR 层 dispatch 去虚化。

## 细化要求

- 每个小阶段后必须插入独立 review 任务。
- 细化时必须列出 `MirStageOutput` 当前字段、构造点和所有下游读取点。
- MIR 去虚化/内联必须只有一个 authoritative owner。

## [TODO] TODO-4-INIT：初始化并细化本任务包

- 目标：
  - 分析 `PLAN.md` §4/P3、`PIPELINE_REFACTOR.md` 和当前 MIR materialization/pass/output 的真实边界；
  - 生成本任务包的详细任务列表，覆盖 `MirStageOutput` 收口、`mir_facts`、pass artifacts 查询面和显式 MIR pass pipeline；
  - 更新 `TODO.md` 的具体任务索引，用新生成的任务替换或扩展 `TODO-4-INIT` 所在索引行。
- 必须实现的内容：
  1. 列出 `MirStageOutput`、`LoweredMir`、`MaterializedMir`、当前 MIR-owned root inventories / pass artifacts 的字段、构造点和下游读取点。
  2. 列出现有 escape analysis、devirtualization、inlining、closure simplification 和 cleanup/summary refresh 的入口与执行顺序。
  3. 把 P3 拆成数量适中的实现小阶段，每个阶段必须有明确目标、修改范围、验证命令和完成条件。
  4. 在每个实现小阶段后插入独立 review 任务，review 任务必须复审前一阶段是否满足 MIR owner 和输出边界约束。
  5. 同步更新 `TODO.md` 中的具体任务索引，确保任务 ID、状态和顺序与本文件一致。
- 完成条件：
  - `TODO-4.md` 不再只是范围说明，而是包含可直接执行的详细任务列表；
  - `TODO.md` 的具体任务索引已经同步反映 `TODO-4.md` 的新任务和 `[TODO]` 状态；
  - 本任务完成记录说明 MIR pipeline 拆分依据和仍需确认的下游兼容风险。
- 完成记录：
  - 待填写。
