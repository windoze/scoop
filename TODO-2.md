# TODO-2：Base crates + cone compilation unit model

> 生成时间：2026-05-21
> 计划基线：[`PLAN.md`](./PLAN.md) §4/P1
> 索引：[`TODO.md`](./TODO.md)
> 当前状态：范围说明，待 `TODO-1.md` 完成后细化。

## 范围

- 建立基础 crate 壳层：`span`、`source`、`types`、`ids`、`project_model`。
- 固定 `ProjectInput` / `ProjectContext` / `SourceConeGraph` 的职责边界。
- 把 “cone = compilation unit” 固化为 facade 层和后续 stage/fact crate 的输入模型。

## 已知起点

- 当前 `span` 在 `crates/scoopc/src/span.rs`。
- 当前 `source` 在 `crates/scoopc/src/source.rs`。
- 当前 cone graph 在 `crates/scoopc/src/cone/graph.rs`。
- 当前 `ProjectInput` / `ProjectContext` 在 `crates/scoopc/src/frontend.rs`。
- 当前 `ConeId` / `ConeInfo` 在 `crates/scoopc/src/resolve/mod.rs`。

## 细化要求

- 每个小阶段后必须插入独立 review 任务。
- 细化时要明确哪些类型先迁入基础 crate，哪些只建立壳层和 re-export 过渡。
- 不得让基础 crate 反向依赖任何 stage crate 或 fact crate。

## [TODO] TODO-2-INIT：初始化并细化本任务包

- 目标：
  - 分析 `PLAN.md` §4/P1、`PIPELINE_REFACTOR.md` 和当前代码中 `span` / `source` / `ty` / `stable_id` / `cone` / `frontend` 的真实依赖关系；
  - 生成本任务包的详细任务列表，覆盖基础 crate 壳层、迁移顺序、re-export 策略、cone compilation unit API 和验证门禁；
  - 更新 `TODO.md` 的具体任务索引，用新生成的任务替换或扩展 `TODO-2-INIT` 所在索引行。
- 必须实现的内容：
  1. 搜索并记录本包会触碰的主要类型、模块和调用点，避免后续任务重复做开放式仓库搜索。
  2. 把 P1 拆成数量适中的实现小阶段，每个阶段必须有明确目标、修改范围、验证命令和完成条件。
  3. 在每个实现小阶段后插入独立 review 任务，review 任务必须复审前一阶段是否满足 P1 约束。
  4. 同步更新 `TODO.md` 中的具体任务索引，确保任务 ID、状态和顺序与本文件一致。
- 完成条件：
  - `TODO-2.md` 不再只是范围说明，而是包含可直接执行的详细任务列表；
  - `TODO.md` 的具体任务索引已经同步反映 `TODO-2.md` 的新任务和 `[TODO]` 状态；
  - 本任务完成记录说明任务拆分依据和未展开的风险。
- 完成记录：
  - 待填写。
