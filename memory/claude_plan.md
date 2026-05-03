# 本次执行计划

## 约束说明
- 不在确认当前任务之前做开放式问题排查。
- 以 `TODO.md` 作为索引，以对应的 `TODO-Px.md` 作为任务细节与完成状态的唯一事实来源。
- 本次只完成第一个未完成的详细任务；如果遇到阻塞，只补充最小前置任务并同步索引，然后提交并停止。

## 执行步骤
1. 读取 `TODO.md`，按索引顺序定位对应的 `TODO-Px.md` 文件。
2. 检查详细任务标题是否带有 `[DONE]`，识别第一个未完成的详细任务。
3. 查看最近提交，确认是否存在与该任务直接相关且未完成的遗留工作；若有，将其视为当前任务的一部分或前置条件。
4. 阅读当前任务的详细要求、约束、验证条件与完成记录。
5. 审查与当前任务直接相关的代码、测试、文档与现有实现边界，确认实现路径。
6. 实现当前任务，避免绕过规范或使用临时性变通方案。
7. 运行与任务直接相关的验证；必要时补充或修复测试，直到相关检查通过。
8. 更新任务记录：在对应 `TODO-Px.md` 中将任务标题标记为 `[DONE]` 并填写完成记录；若任务索引状态、标题、顺序或依赖发生变化，同步更新 `TODO.md`；仅在阶段计划确实变化时更新 `PLAN.md`。
9. 检查工作区差异，按要求提交本次任务涉及的全部改动，并停止，不继续下一个任务。

## 进度记录
- 已写入并保留执行计划文件，供本次 invocation 持续更新。
- 已读取 `TODO.md` 作为索引，并核对 `TODO-P6.md` 中的完成标记；当前首个未完成详细任务为 `P6-T02l`，不是 `P6-T03`。
- 已检查最近提交：`HEAD` 为 `[P6-T02l] Track handle region routing prerequisite`，说明上次 invocation 已把 `P6-T03` 的 blocker 升格为当前前置任务；工作区当前干净，无未提交遗留改动。
- 当前任务目标：为 `HandleDispatch` 发布 authoritative 的 state-region / boundary-consumption contract，并把它接入 `dump-effect-lowered` 与 refactor LLVM ABI/query，避免 `P6-T03` 在 backend 现场重建 body/arm/finally 子图归属与 boundary 路由。
- 当前执行步骤：
  1. 审查 `effect_lowered` 中现有 `HandleDispatch` contract、state graph、boundary map、resume-state map 与 dump surface。
  2. 审查 `llvm/codegen/effect_refactor/{types,layout,body}.rs` 中现有 HandleDispatch query/layout 边界，确认应扩展的 published query 形状。
  3. 以最小改动在 P5 authoritative 发布阶段补齐 region membership 与 boundary routing contract，并补充 fail-fast 校验。
  4. 更新 dump surface 与 LLVM query，使 backend 可以按 handle site + state/boundary 稳定回查 region/routing。
  5. 增加/更新定向测试，覆盖 body/arm/finally routing、resume target、pending completion 与漂移 fail-fast。
  6. 运行任务要求的测试与 `clippy`，修复问题后再更新 `TODO-P6.md` / `TODO.md` 的完成记录并提交。
- 已完成关键实现草稿：
  - `LateLoweredHandleDispatchContract` 已扩展为显式发布 `state_regions` 与 `boundary_routings`；同时补充了 case routing action 与 query helper。
  - `effect_lowered/materialize.rs` 现会在 authoritative 发布阶段基于 state graph + boundary map 构造 body/arm/finally/dispatch/exit region membership，以及每个 boundary 的 handled/pending/outward routing。
  - `effect_lowered/opt.rs` 已接上 state redirect，避免 post-opt 后新 contract 漂移。
  - `effect_lowered/dump.rs` 已把 `state_regions:` / `boundary_routings:` / `case_routings:` 暴露到 `dump-effect-lowered` surface。
  - `llvm/codegen/effect_refactor/types.rs` / `layout.rs` 已新增 query API 与 fail-fast 交叉校验，避免 P6-T03 读取漂移 routing。
  - 已补入一组 materialize/query 定向测试草稿，下一步用格式化与测试把编译/契约细节收敛到可提交状态。
- 当前验证结果：
  - `cargo test -p scoopc refactor_handle_dispatch_region_contract` 通过。
  - `cargo test -p scoopc refactor_handle_dispatch_region_routing` 通过。
  - `cargo run -p scoop -- --effect-pipeline refactor dump-effect-lowered tests/fixtures/run-pass/effect_resume_if_else_branch_single_perform.scoop` 通过，并确认 dump 中包含新 `state_regions` / `boundary_routings` surface。
  - `cargo run -p scoop -- --effect-pipeline refactor dump-effect-lowered tests/fixtures/run-pass/effect_multi_escape_indirect_direct_while.scoop` 通过，并确认 mixed body/arm routing 已发布。
  - `cargo clippy -p scoopc -p scoop --all-targets -- -D warnings` 通过。
- 已完成记录更新：`TODO-P6.md` 已将 `P6-T02l` 标记为 `[DONE]` 并写入完成记录 / 验证命令；`TODO.md` 索引也已同步为 `[DONE]`。
- 下一步：复核差异后提交 `[P6-T02l] Publish handle region routing contract`，然后停止，等待下次 invocation 进入 `P6-T03`。
