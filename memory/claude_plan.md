## 当前轮执行计划

说明：这里记录可审查的执行计划、关键观察、变更决策与进度，不记录逐字内部推理。

1. 读取 `TODO.md`，确认它只作为索引使用，并按其中引用顺序定位对应的 `TODO-Px.md` 文件。
2. 逐个检查详细任务文件中的任务标题，依据是否带有 `[DONE]` 前缀来判断完成状态。
3. 锁定第一个未完成的详细任务，阅读其完整要求、约束、依赖、验证标准与完成记录。
4. 检查最近提交是否存在与该任务直接相关且明确未完成的问题；若有，将其视为当前任务的一部分或必要前置。
5. 在不扩大范围的前提下实现该任务；如果遇到阻塞当前任务的真实缺陷或缺失特性，则在对应 `TODO-Px.md` 中插入最小必要前置任务，并同步 `TODO.md`。
6. 运行与该任务直接相关的验证；如任务涉及通用代码路径，再补充必要的 `cargo test` / `cargo clippy --all-targets -- -D warnings` / 其他指定验证。
7. 更新文档记录：
   - 在对应 `TODO-Px.md` 中将任务标题加上 `[DONE]` 并补全完成记录；若被阻塞，则保持未完成并记录阻塞与新增前置。
   - 若任务索引、标题、顺序或状态变化，更新 `TODO.md` 保持同步。
   - 仅当阶段计划、依赖或完成标准变化时更新 `PLAN.md`。
8. 检查工作区是否有本轮相关改动需要一并提交；按任务号撰写提交信息并提交。
9. 停止，不进入下一个任务。

## 进度日志

- 已创建本计划文件，准备开始读取任务索引并定位首个未完成详细任务。
- 已读取 `TODO.md` 与 `TODO-P4.md`，确认首个未完成详细任务为 `P4-T03：构建 BodyEffectFacts / SiteEffectFacts 与 local-case 结构化分析`。
- 已检查最近提交：`[P4-T02R] Review schema pool and callable facts` 未显式留下会阻塞 `P4-T03` 的未完成事项。
- 已审查现状：`crates/scoopc/src/effect_facts/facts.rs` 目前只有 callable-level 壳层，`BodyEffectFacts` 仍为空；`builder.rs` 只构建了 callable/schema 壳层，尚未把 MIR body/block/site 级 contract 落入 facts。
- 已查看 `tests/fixtures/mir_refactor/{dispatch_and_resume_call,handle_perform,handle_finally_boundary}.scoop` 的 refactor `dump-mir` 输出，确认当前 P3 MIR 已显式携带 `SiteId`、`CallKind::{Direct,Virtual,Interface,Resume}`、`PerformMetadata`、`HandleMetadata`、`HandlerArm`、cleanup block 与 `resume_target`/`finally_target` 等构建 P4-T03 所需信息。

## 当前实现提纲

1. 在 `crates/scoopc/src/effect_facts/facts.rs` 中补齐 P4-T03 所需 public facts 结构：
   - `BlockEffectFacts`
   - `SiteEffectFacts`
   - `CallSiteEffectFacts`
   - `PerformSiteEffectFacts`
   - `ResumeSiteEffectFacts`
   - `HandleSiteEffectFacts`
   - `HandleArmEffectFacts`
   - 配套的 `EffectPrecision` / `CallTargetMode` / nested-handle 分类枚举
2. 扩展 `MaterializedEffectFactsBuilder`：
   - 继续保留现有 callable/schema 壳层构建；
   - 第二阶段按 `InstanceKey -> MIR body` 遍历 pass-view callable bodies，基于 `SiteId` 和 `BasicBlockId` 构建 `BodyEffectFacts`；
   - 直接从 MIR metadata 生成 `Perform` / `Resume` / `Handle` contract，避免回 HIR side tables；
   - 为 direct/closure/fun-value/dispatch 构建 call-site target mode 与 callee schema；精度不足时保守 widen 到 `CandidateSet` 或 `DynamicFallback`。
3. 用结构化 handle 子区域分析计算：
   - `handled_cases`
   - `body_outward_cases`
   - `arm_outward_cases`
   - `finally_outward_cases`
   - nested handle `SelfContained` / `MaySuspendOutward`
4. 新增/更新定向单测，覆盖：
   - direct call / fun-value / virtual / interface / resume site facts
   - `perform` emitted case 与 continuation schema
   - `handle` facts 与 finally/cleanup outward
   - nested handle 分类
5. 运行任务要求的测试与必要的 `clippy` 验证；通过后再更新 `TODO-P4.md` / `TODO.md` 并提交。

## 阻塞更新

- 在实现 `P4-T03` 的过程中，发现一个会直接阻塞正确落地的前置问题：当前 canonical `MaterializedMir::pass_view()` 对普通非泛型样本并不稳定发布 callable family。
- 具体表现：对 `dispatch_and_resume_call`、`handle_perform`、`handle_finally_boundary` 这类普通非泛型 shape，`MaterializedEffectFactsBuilder` 观察到 `pass_view().instances()` 可能为空，从而拿不到 authoritative `InstanceKey -> callable body` 映射；这会直接破坏 `P4-T03` 对 `(callable identity, BasicBlockId / SiteId)` 键空间的依赖。
- 依据任务约束，不能通过扫描 raw `MaterializedMir.file` 或 `caller_side_pass_candidate_bodies()` 额外造一套 fallback owner 键空间来绕过该问题；正确修复位置应在 `mir/materialize` / `callables` / `pass_view` 的 canonical handoff 发布层。
- 已在 `TODO-P4.md` 与 `TODO.md` 中插入新的前置任务 `P4-T02a` / `P4-T02aR`，要求先修复 ordinary non-generic callable body 的 canonical pass-view 发布，再回到 `P4-T03`。
- 因此本轮不会把 `P4-T03` 标记完成；本轮目标改为：记录 blocker、同步任务顺序、保留已验证不会破坏现有绿灯的准备性代码改动，然后提交并停止。
- 已复验当前保留改动不会破坏现有绿灯：`cargo test -p scoopc --no-default-features effect_facts::builder::tests`、`cargo test -p scoopc --no-default-features effect_refactor_pipeline::effect_facts_stage`、`cargo clippy -p scoopc --all-targets --no-default-features -- -D warnings` 全部通过。
