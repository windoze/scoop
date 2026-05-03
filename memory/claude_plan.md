## 当前执行计划

说明：按安全约束，这里记录的是可审计的执行摘要与步骤计划，不包含内部私有推理。

1. 读取 `TODO.md` 作为索引，并结合最近一次提交信息，定位当前应处理的第一个未完成详细任务。
2. 打开对应的 `TODO-Px.md`，以标题是否带有 `[DONE]` 作为唯一完成判定标准，确认任务正文、约束、依赖、验证要求与完成记录格式。
3. 检查当前工作区是否已有与该任务直接相关且未提交的改动；若存在，纳入本次任务上下文，但不回退无关改动。
4. 实现该任务要求的代码或文档修改；若发现会阻塞该任务的真实前置缺陷或缺失能力，则在正确的 `TODO-Px.md` 中插入最小必要前置任务，并同步 `TODO.md`。
5. 运行与当前任务直接相关的验证；至少覆盖任务要求中的测试与必要的编译/静态检查，尽量保证无新增告警。
6. 更新 `memory/claude_plan.md` 记录关键进展、计划变更、阻塞信息与最终结果。
7. 将已完成任务在对应 `TODO-Px.md` 标题前标记为 `[DONE]`，补全完成记录；如任务索引发生变化，同步更新 `TODO.md`。仅在阶段计划发生变化时更新 `PLAN.md`。
8. 按要求创建一次 git 提交，提交信息聚焦当前任务编号与结果，然后停止，不进入下一个任务。

## 进展记录

- 已创建初始执行计划，下一步开始读取任务索引与详细任务文件。
- 已读取 `TODO.md` 与 `TODO-P6.md`，确认当前首个未完成详细任务是 `P6-T02f`：为 straight-line source-slice 非 boundary dynamic call 发布 callable-object ABI/query contract。
- 已确认最近一次提交 `[P6-T02f] Track non-boundary dynamic invoke blocker` 与当前任务直接相关；当前按该任务继续执行，而不是跳到 `P6-T03`。
- 下一步：检查 `effect_lowered` 与 `llvm/codegen/effect_refactor/{types,layout}.rs`，确定现有 query 仅覆盖 `boundary_map` 的具体缺口，然后实施最小修复。
- 已确认具体缺口：`layout.rs` 当前只从 `LateLoweredBoundaryMap` 发布 dynamic invoke contract；而 straight-line `source_slices` 中的 non-boundary `Rvalue::Call` 需要结合 `MaterializedMirPassView` 的 canonical MIR call kind 与 `MaterializedEffectFacts::body(...).site(site_id)` 的 call facts 才能获得 authoritative ABI/query。
- 当前实现方向：在 refactor LLVM stage handoff 中把 ABI materializer 显式接到 `MaterializedEffectFacts`，扩展 dynamic invoke query 发布范围到 source-slice non-boundary dynamic call，并增加缺失/漂移时的 fail-fast 测试与 fixture。
- 已完成代码实现：
  - `crates/scoopc/src/llvm/emit.rs` 现在把 ABI visibility 对应的 `MaterializedEffectFacts` 显式传入 refactor ABI materializer。
  - `crates/scoopc/src/llvm/codegen/effect_refactor/layout.rs` 现在除了扫描 `boundary_map` 外，还会扫描 `state_graph.source_slices()` 中的 non-boundary dynamic call，并结合 canonical MIR + published `BodyEffectFacts` 发布 `(owner_step_schema, site_id)` 级别的 dynamic invoke contract。
  - 同时新增了 call-kind drift 校验与 source-slice contract 缺失时的 fail-fast。
- 已新增 fixture：`tests/fixtures/build/effect_refactor_non_boundary_dynamic_call_emit_llvm.scoop`，用于锁定 pure source-slice non-boundary virtual dynamic call 的 handoff。
- 已完成验证：
  - `cargo test -p scoopc refactor_llvm_dynamic_invoke_query`
  - `cargo test -p scoopc refactor_llvm_callable_carrier_layout`
  - `cargo run -p scoop -- --effect-pipeline refactor dump-effect-lowered tests/fixtures/build/effect_refactor_non_boundary_dynamic_call_emit_llvm.scoop`
  - `cargo run -p scoop -- --effect-pipeline refactor test --fixtures tests/fixtures/build/effect_refactor_non_boundary_dynamic_call_emit_llvm.scoop`
  - `cargo clippy -p scoopc -p scoop --all-targets -- -D warnings`
- 下一步：更新 `TODO-P6.md` / `TODO.md` 的完成标记与完成记录，随后创建本任务提交并停止。
