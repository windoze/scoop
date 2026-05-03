# Claude Plan

## Constraints
- 不记录或暴露内部逐字思维链；此文件只保留可审计的执行计划、关键决策依据、阻塞项与完成记录。
- 本次调用只处理一个详细任务：先从 `TODO.md` 索引定位，再以对应 `TODO-Px.md` 为准。
- 若遇到阻塞当前任务的真实前置问题，先修复；若本次无法直接修复，则在对应 `TODO-Px.md` 中插入最小必要前置任务，并同步 `TODO.md`，然后提交并停止。

## Initial Execution Plan
1. 读取 `TODO.md`，确认其仅作为索引使用，并找出引用的详细任务文件。
2. 按任务顺序检查相关 `TODO-Px.md`，以标题是否带 `[DONE]` 作为完成判定，定位首个未完成详细任务。
3. 查看最近一次提交信息，判断是否存在与该任务直接相关且明确未完成的问题；如有，将其纳入当前任务或记为前置依赖。
4. 阅读当前任务涉及的代码、测试、规范与现状实现，确认约束、依赖与验证方式。
5. 直接实现当前任务；若发现阻塞当前任务的规范不匹配或实现缺口，先处理阻塞或最小化地补充前置任务并同步索引。
6. 运行与当前任务相关的测试与必要校验；若变更范围要求较高，再补充格式化、lint 或更广测试。
7. 更新 `memory/claude_plan.md` 记录关键发现、计划变更、验证结果。
8. 在对应 `TODO-Px.md` 中更新完成记录，并在任务标题前加 `[DONE]`；如任务索引内容有变化，同步更新 `TODO.md`。
9. 仅在阶段计划或依赖结构变化时更新 `PLAN.md`。
10. 按仓库约定创建一次 git 提交，然后停止，不继续下一个任务。

## Progress Log
- 已创建初始执行计划；下一步读取 `TODO.md` 与相关 `TODO-Px.md` 定位首个未完成任务。
- 已读取 `TODO.md` 与 `TODO-P6.md`；当前首个未完成详细任务确认为 `P6-T02R`（`P6-T02`/`P6-T02a`/`P6-T02b` 已标记 `[DONE]`，`P6-T02R` 仍未完成）。
- 已检查最近提交：`2c9f255d [P6-T02b] Enforce resume-interface method completeness`。该提交与当前 review 直接相关，说明本次任务需要复审 `P6-T02b` 是否已消除先前在 `P6-T02R` 完成记录里记下的 blocker。
- 当前审查重点：
  1. 复核 `crates/scoopc/src/llvm/codegen/effect_refactor/{types,layout}.rs` 的 ABI contract 是否由 P5 authoritative handoff 决定。
  2. 复核 `Step_F` / frame / continuation / resume-interface / dynamic invoke 的查询 API 是否已闭合，且 `Unit` ABI 是否已零载荷退化。
  3. 搜索 refactor LLVM ABI 主实现中是否仍残留 `EffectSignal` / `EffectOutcome` / `LegacyEffectBoundary` 等 legacy ABI 载体。
  4. 运行 `P6-T02R` 要求的测试与命令；若发现新 blocker，则按规则补前置任务并停止；否则把 `P6-T02R` 标记为 `[DONE]` 并提交。

## Blocker Discovered During Review
- `cargo test -p scoopc refactor_llvm_` 失败，失败点为：
  - `refactor_llvm_step_layout_keeps_canonical_case_set_for_single_case_callable`
  - `refactor_llvm_frame_layout_preserves_slot_indices_and_system_fields`
- 失败原因：`materialize_refactor_program_abi(...)` 现在会对 published resume-interface method completeness fail fast；而这两个测试仍默认把 `effect_lowered_stage_output.program()`（authoritative reachable-body handoff）直接送入 ABI materializer。该 handoff 在常规 late-opt 下允许裁剪 ABI shell 上不再可达的方法，因此不再满足 `P6-T02b` 之后对 ABI materialization 的完整 method 集要求。
- 结论：这不是要绕过的新限制，而是测试/审查入口与真实 refactor LLVM stage 已经不一致。真实 stage 在 ABI 物化时会使用单独的 `abi_visibility_effect_lowered_stage_output`，并在该 handoff 上保留 published resume shells。
- 处理方案：最小化修正 `layout.rs` 的测试夹具，让默认 ABI materialization 路径改为使用“保留 published resume shells”的 ABI-visibility late-lowered program；保留 authoritative handoff 以便继续构造 fail-fast 负例测试。

## Completed Key Steps
- 已在 `crates/scoopc/src/llvm/codegen/effect_refactor/layout.rs` 的测试夹具中新增 `abi_visibility_program`，并让默认 ABI materialization 测试改用该程序；这样与真实 refactor LLVM stage 的 ABI shell 发布路径一致。
- 已保留 authoritative `effect_lowered_stage_output` 供负例测试继续构造“缺失 interface / method 必须 fail fast”的场景，没有放宽实际 ABI contract。
- 已重新运行完整 `P6-T02R` 验证矩阵，全部通过：
  - `cargo test -p scoopc refactor_llvm_`
  - `cargo test -p scoopc refactor_resume_interface_completeness_groups_methods_by_effect_family`
  - `cargo test -p scoop refactor_build_`
  - `cargo test -p scoop build_fixtures_propagate_refactor_session_options_to_build_command`
  - 三个 refactor build fixtures
  - 一个 legacy build fixture 抽样
  - `cargo clippy -p scoopc -p scoop --all-targets -- -D warnings`
- 已重新执行关键字搜索：`EffectSignal` / `EffectOutcome` / `LegacyEffectBoundary` 仅出现在 legacy `llvm/codegen/effect/**`；refactor ABI 主实现未发现继续把这些 legacy contract 当作最终 ABI 模型的残留。
- 已更新 `TODO-P6.md` 与 `TODO.md`，把 `P6-T02R` 标记为完成；`PLAN.md` 无需变更。
- 下一步仅剩提交当前变更并停止，不继续下一个任务。
