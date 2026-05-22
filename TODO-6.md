# TODO-6：Global init + LLVM backend cleanup + final verification

> 生成时间：2026-05-21
> 计划基线：[`PLAN.md`](./PLAN.md) §4/P6-P8
> 索引：[`TODO.md`](./TODO.md)
> 当前状态：范围说明，待 `TODO-5.md` 完成后细化。

## 范围

- 闭合 global object、top-level `val`、annotated top-level `var` 的初始化根分类。
- 分离 object once 与 top-level eager init 语义。
- 建立 per-cone init routine 与 final entry init order contract。
- 让 `@Global` / `@ThreadLocal` storage policy 全链路闭合。
- 清理 LLVM backend，使其只依赖 `LIR + LIR facts + base context`。
- 做最终全仓验证，为未来 C backend 固定干净输入边界。

## 进入门禁

- P4/P5 已完成：effect facts stage 不修改 MIR 输出本体，`EffectFactsStageOutput = { effect_facts }`，`LirStageOutput = { lir, lir_facts }`，且 LIR opt family 只消费 LIR-owned 输入。
- `scoopc_lir_facts` 已发布 P5-owned backend-neutral callable ABI、dynamic invoke、dispatch owner/slot、continuation/resume publication 与 LIR opt metadata；TODO-6 不应重新让 LLVM 从 HIR/raw MIR/effect facts 推导这些合同。
- TODO-6-INIT 应从剩余边界开始细化：global init/storage/entry init order、LLVM HIR scaffold、crate-private MIR pass-view residual、LLVM physical ABI/layout、backend reachability、多 `TypeStore` 桥接和最终全仓验证。

## 细化要求

- 每个小阶段后必须插入独立 review 任务。
- 细化时必须把 P6 global init 与 P7 backend cleanup 分开，避免在 codegen 里补前端语义。
- 最终验收必须搜索确认：无旧 comptime surface、无 HIR/codegen 层去虚化、无 stage output 嵌套上游整包。

## [TODO] TODO-6-INIT：初始化并细化本任务包

- 目标：
  - 分析 `PLAN.md` §4/P6-P8、`PIPELINE_REFACTOR.md` 和当前 global init、storage policy、LLVM backend 输入依赖的真实边界；
  - 生成本任务包的详细任务列表，覆盖 global init model、LLVM backend cleanup 和 final verification；
  - 更新 `TODO.md` 的具体任务索引，用新生成的任务替换或扩展 `TODO-6-INIT` 所在索引行。
- 必须实现的内容：
  1. 列出当前 object once、top-level `val`、top-level `var`、`@Global`、`@ThreadLocal` 的 HIR/MIR/LIR/codegen 入口和语义分歧点。
  2. 列出 LLVM backend 当前直接读取 HIR、raw MIR、effect facts 或上游整包 stage output 的位置。
  3. 把 P6、P7、P8 拆成数量适中的实现小阶段，每个阶段必须有明确目标、修改范围、验证命令和完成条件。
  4. 在每个实现小阶段后插入独立 review 任务，review 任务必须复审前一阶段是否满足 global init contract 或 backend 输入边界约束。
  5. 同步更新 `TODO.md` 中的具体任务索引，确保任务 ID、状态和顺序与本文件一致。
- 完成条件：
  - `TODO-6.md` 不再只是范围说明，而是包含可直接执行的详细任务列表；
  - `TODO.md` 的具体任务索引已经同步反映 `TODO-6.md` 的新任务和 `[TODO]` 状态；
  - 本任务完成记录说明 P6/P7/P8 的拆分依据、最终验收命令和未来 C backend 输入边界风险。
- 完成记录：
  - 待填写。
