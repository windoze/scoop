## 当前执行计划

1. 读取 `TODO.md` 作为索引，并按顺序检查对应的 `TODO-Px.md` 详细任务文件。
2. 确认第一个标题未带 `[DONE]` 的详细任务，必要时结合最近一次提交信息判断是否存在与该任务直接相关且未完成的问题需要先处理。
3. 阅读该任务涉及的代码、测试、规范与依赖，确认实现边界与验证要求。
4. 直接实现该任务；若遇到会阻塞规格正确实现的真实缺口，则在对应 `TODO-Px.md` 中插入最小前置任务，并同步 `TODO.md`，然后停止。
5. 运行与该任务直接相关的测试、格式化、以及需要的质量检查；若仓库要求，补充执行 `cargo clippy --all-targets -- -D warnings`。
6. 更新任务记录：在对应 `TODO-Px.md` 中把任务标题标为 `[DONE]` 并填写完成记录；若索引有变化，同步更新 `TODO.md`；仅在阶段计划变化时更新 `PLAN.md`。
7. 检查工作区中与本任务相关的改动，使用清晰的提交信息创建一次 git 提交，然后停止，不进入下一个任务。

## 进度记录约定

- 在确认当前目标任务后，补充更具体的实施步骤。
- 在实施过程中如果计划变化、发现阻塞、完成关键实现或完成验证，会继续更新本文件。
- 本文件记录的是可审计的执行计划与进度摘要，不包含隐式推理细节。

## 当前目标任务

- 已根据 `TODO.md` 与 `TODO-P6-part2.md` 确认首个未完成详细任务为 `P6-T02qd`：发布 continuation resume payload -> resumed local/home 注入 contract。
- 最近一次提交信息为 `[P6-T02qd] Track resumed local/home binding prerequisite`，说明这是从 `P6-T03` 落地过程中显式抽出的当前前置任务，应作为本次唯一执行目标。

## 针对 P6-T02qd 的细化步骤

1. 检查当前工作区状态，确认是否存在未提交改动，并在后续提交时一并考虑用户要求的恢复场景。
2. 阅读 `TODO-P6-part2.md` 中 `P6-T02qd` 的完整要求，以及相关实现文件：
   - `crates/scoopc/src/effect_lowered/{frame,materialize,ir}.rs`
   - `crates/scoopc/src/llvm/codegen/effect_refactor/{types,body}.rs`
3. 搜索现有 `ResumePayload`、`BoundaryResult`、`result_local`、continuation surface-resume / wrapper projection 等 handoff 结构，定位“恢复值写回 local/home”信息当前缺失的发布点与消费点。
4. 以最小改动为原则实现 authoritative published contract，并补齐 query/fail-fast：
   - 覆盖 call/resume/perfom 相关 continuation resume consumer；
   - 覆盖 shared wrapper / owner trampoline 所需注入信息；
   - 对缺失、歧义和漂移显式拒绝。
5. 补充或修正 dump/query/tests，使 `dump-effect-lowered` 与 LLVM query 测试能直接观察该 contract。
6. 运行与任务直接相关的测试、必要的 `cargo clippy --all-targets -- -D warnings`，若失败则先修复。
7. 更新 `TODO-P6-part2.md`（必要时同步 `TODO.md`），将 `P6-T02qd` 标记为 `[DONE]` 并记录完成结果；若未能规格正确完成，则只记录 blocker 与新增前置任务。
8. 创建一次 git 提交并停止，不进入 `P6-T03`。

## 当前实现方案（已确认）

- 现有 `BoundaryResult(boundary, local)`、`result_local` 与 `ResumePayload(boundary, case)` 彼此分散，仍不足以让后续 continuation object method / owner trampoline 仅凭 published handoff 找到恢复值写回位置。
- 采用最小且统一的落点：
  1. 在 late-lowered IR 的 `LateLoweredFrameSchema` 上补充一个 authoritative 的 resume 注入绑定表，显式发布“某个 boundary / resume state 的 incoming resume payload/answer 应写回哪个 local，以及它的 frame home 是哪个 slot（如果有）”。
  2. 在 builder 阶段基于已物化的 `boundary_map + frame_schema` 生成这张表；对 paired runtime-error boundary 显式继承或校验其对应 binding；若缺失或冲突则 fail fast。
  3. 在 stable dump 中直接展示该绑定表，便于 `dump-effect-lowered` 验证。
  4. 在 LLVM ABI/query 层继续物化成可查询 layout，补上 boundary-keyed 与 resume-state-keyed 查询面，并验证 frame slot / field index 漂移。
- 这样可以同时覆盖：
  - `fetch` 的 `PerformResult` 恢复目标；
  - `Resume` boundary 的 assign target；
  - shared wrapper / handle-binder owner route 通过 captured `resume_state` 选择正确 consumer local/home；
  - paired runtime-error route 的显式绑定继承与 fail-fast。

## 已完成的关键步骤

1. 已在 `LateLoweredFrameSchema` 上新增 authoritative `resume_payload_bindings` 表，并补充访问器。
2. 已在 late-lowered builder/materialize 阶段生成这张表：
   - `Call` / `Resume` 直接基于 `result_local` + `BoundaryResult` home；
   - `Perform` 基于 `BoundaryResult` slot 发布 `PerformResult` consumer；
   - paired `RuntimeError` boundary 显式继承对应 resume boundary 的 consumer；
   - 对缺失、重复、resume-state 冲突显式 fail fast。
3. 已在 stable dump 中显示 `resume_payload_bindings`，`dump-effect-lowered` 可直接观察该 contract。
4. 已在 LLVM ABI/query 层新增 resumed local/home layout 与查询面：
   - boundary-keyed query；
   - resume-state-keyed query；
   - frame field index 校验与 drift/missing fail-fast。
5. 已补充定向回归：
   - `refactor_effect_lowered_resume_payload_binding_*`
   - `refactor_llvm_resume_payload_binding_*`

## 当前验证结果

- `cargo test -p scoopc refactor_effect_lowered_resume_payload_binding` 通过。
- `cargo test -p scoopc refactor_llvm_resume_payload_binding` 通过。
- `cargo test -p scoopc refactor_llvm_` 通过。
- `cargo run -p scoop -- --effect-pipeline refactor dump-effect-lowered tests/fixtures/run-pass/effect_multi_escape_indirect_direct_while.scoop` 通过，并可见 `resume_payload_bindings`。
- `cargo run -p scoop -- --effect-pipeline refactor dump-effect-lowered tests/fixtures/run-pass/effect_resume_if_else_branch_single_perform.scoop` 通过，并可见 `resume_payload_bindings`。
- `cargo clippy -p scoopc --all-targets -- -D warnings` 通过。
- 额外说明：`cargo clippy --workspace --all-targets -- -D warnings` 会输出 `scoop_runtime` 的现存 macOS C SDK 弃用警告（`runtime/c/scoop_stackmap.c` 使用 `getsectbynamefromheader_64`）；该警告不在本任务修改范围内，但已记录为验证时观察到的外部噪音。

## 当前收尾状态

- 已将 `TODO-P6-part2.md` 中的 `P6-T02qd` 标记为 `[DONE]`，并同步更新 `TODO.md` 索引。
- `PLAN.md` 未改动：本次没有改变阶段顺序、依赖结构或退出条件。
- 下一步仅剩：复核 git 状态、创建一次提交，然后停止，不进入 `P6-T03`。
