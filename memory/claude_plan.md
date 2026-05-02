## 本轮目标

- 先以 `TODO.md` 作为索引，定位第一个未完成的详细任务。
- 以对应 `TODO-Px.md` 作为唯一任务真源，完成该任务或在遇到真实前置阻塞时补入最小 prerequisite 任务。
- 完成后执行相关测试、同步任务文档、创建一次 git 提交，然后停止。

## 高层判断记录

- 不跳过任何标题未带 `[DONE]` 的详细任务。
- 不为方便实现而主动拆分任务；只有遇到真实且未被跟踪的前置阻塞时，才补充最小 prerequisite。
- 如果最新提交中有与当前任务直接相关且明确未完成的事项，需要将其纳入当前任务范围或写成前置依赖。
- 本文件记录的是可审阅的执行计划、判断依据和进度，不写不可见的私有推理细节。

## 执行步骤

1. 读取 `TODO.md`，确认索引顺序和对应的 `TODO-Px.md` 文件。
2. 按顺序读取相关 `TODO-Px.md`，找到第一个标题未带 `[DONE]` 的详细任务。
3. 检查最近一次提交是否存在与该任务直接相关的未完成事项。
4. 阅读任务要求、依赖、验证方式与相关代码，确认实现边界。
5. 实现任务；如果遇到阻塞当前任务的真实缺口，则先把该缺口作为最小前置任务写回对应 `TODO-Px.md`，同步 `TODO.md`，必要时更新 `PLAN.md`，然后提交并停止。
6. 运行该任务要求的测试、格式化和必要的质量检查；修复发现的问题。
7. 将任务在对应 `TODO-Px.md` 中标记为 `[DONE]` 并补齐完成记录；若索引状态变化，同步 `TODO.md`。
8. 记录本文件进展，检查工作区，然后按任务编号创建一次 git 提交并停止。

## 进度记录

- 已写入本轮初始计划。
- 已读取 `TODO.md`，确认索引中的首个未完成任务是 `P5-T05`，对应 `TODO-P5.md`。
- 已读取 `TODO-P5.md` 中 `P5-T05` 的完整任务定义：目标是在 P5 里真正物化 `Step_F`、canonical dynamic `invoke`、continuation object、internal resume interfaces，并按统一 `Step`/continuation 模型完成 boundary lowering。
- 已检查最新提交 `67ce8559 [P5-T04b] Lock continuation surface/out-step contract`；该提交与当前任务直接相关，但没有额外声明必须先补的未完成 prerequisite。
- 已完成现状核查，确认当前实现还停留在“shell + state skeleton”阶段，距离 `P5-T05` 仍有三类关键缺口：
  1. `resume_interfaces` 目前按整个 `StepSchema` 建壳，没有按 effect family 分组，无法满足“每个 effect 一个 internal resume interface”的 contract。
  2. `LateLoweredBoundaryMap` 目前只记录 `source/owner_state/resume_state`，还没有显式的 boundary-lowering contract，P6 若直接消费它仍需要重新设计 call/perform/resume/handle 的 effectful ABI。
  3. `LateLoweredContinuationObject` 目前只有 captures + method reachability shell，尚未显式发布 source-visible `resume(...) -> Step_F` 合同、internal method body kind、one-shot runtime-error policy。
- 当前执行方案：
  1. 在 `effect_facts` schema 中补上稳定的 effect-family identity，并让 `ConcreteOpKey` 显式携带它。
  2. 在 `crates/scoopc/src/effect_lowered/` 新增 `materialize.rs`，把 T05 的 step/interface/continuation/boundary materialization 逻辑从 `builder.rs` 中拆出。
  3. 扩展 late-lowered IR：resume interface 按 effect family 分组；continuation object 补充 surface-resume 与 internal method body contract；boundary 补充 call/perform/resume/runtime-error/handle lowering contract。
  4. 更新 stable dump 与定向单测，覆盖 interface completeness、impl-plan lowering、boundary lowering、one-shot/runtime-error 合同。
- 已完成代码实现：
  1. `crates/scoopc/src/effect_facts/schema.rs` / `builder.rs` 已新增 `EffectFamilyKey`，`ConcreteOpKey` 现在显式携带 effect family identity，不再需要在 P5 里临时猜 effect owner。
  2. `crates/scoopc/src/effect_lowered/materialize.rs` 已正式落地，并接管 T05 的 materialization 职责；`builder.rs` 现在只负责 orchestration。
  3. `crates/scoopc/src/effect_lowered/ir.rs` 已扩展为真正的 T05 contract：
     - dynamic invoke entry 记录 `entry_state` / `complete_state`；
     - resume interface 按 effect family 分组；
     - continuation object 显式发布 `surface_resumes`、internal methods、one-shot body kind；
     - boundary 现在携带 `Call` / `Perform` / `Resume` / `RuntimeError` / `Handle` lowering contract。
  4. `crates/scoopc/src/effect_lowered/dump.rs` 已能稳定输出上述新 contract，便于后续 dump/snapshot/P6 直接消费。
- 已完成验证：
  1. `cargo fmt --all`
  2. `cargo test -p scoopc --no-default-features refactor_step_materialization`
  3. `cargo test -p scoopc --no-default-features refactor_boundary_lowering`
  4. `cargo test -p scoopc --no-default-features refactor_continuation_object`
  5. `cargo test -p scoopc --no-default-features refactor_impl_plan_lowering`
  6. `cargo test -p scoopc --no-default-features refactor_resume_interface_completeness`
  7. `cargo test -p scoopc --no-default-features refactor_effect_lowered_stage`
  8. `cargo test -p scoopc --no-default-features refactor_late_lowered_ir`
  9. `cargo test -p scoopc --no-default-features refactor_late_boundary_selection`
  10. `cargo test -p scoopc --no-default-features refactor_late_segmentation`
  11. `cargo test -p scoopc --no-default-features refactor_owner_resume_state`
  12. `cargo test -p scoopc --no-default-features refactor_frame_lifting`
  13. `cargo test -p scoopc --no-default-features refactor_late_control_flow`
  14. `cargo test -p scoopc --no-default-features refactor_dropped_continuation`
  15. `cargo test -p scoopc --no-default-features refactor_runtime_error_boundary`
  16. `cargo clippy -p scoopc --no-default-features --all-targets -- -D warnings`
- 已完成文档回写：`TODO-P5.md` 已将 `P5-T05` 标为 `[DONE]` 并写入完成记录，`TODO.md` 已同步索引状态；`PLAN.md` 无需修改。
- 当前只剩：检查工作区、创建 `P5-T05` 提交，然后停止。
