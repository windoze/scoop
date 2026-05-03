## 执行计划

说明：我不会记录或暴露不可审计的内部逐步推理，但会在这里持续维护可执行计划、关键判断依据、进度与变更记录，便于检查当前工作状态。

1. 读取 `TODO.md`，确认它仅作为索引使用。
2. 按 `TODO.md` 中给出的顺序读取对应的 `TODO-Px.md` 详细任务文件。
3. 依据详细任务标题是否带有 `[DONE]`，定位第一个未完成的详细任务；若索引与详细文件不一致，以详细文件为准。
4. 查看最近一次提交信息，确认是否存在与该任务直接相关且明确未完成的问题；如有，视情况并入当前任务或作为前置任务处理。
5. 阅读与当前任务直接相关的代码、测试、规范与任务约束，确认实现边界与验证要求。
6. 实现当前任务；若发现阻塞且无法按规范正确完成，则仅引入最小必要前置任务，并同步更新 `TODO-Px.md` 与 `TODO.md`。
7. 运行与该任务相关的验证：至少包括针对性测试；如任务影响范围要求更高，再运行更广泛校验，并尽量确保无告警。
8. 更新文档记录：在对应 `TODO-Px.md` 中将任务标题标记为 `[DONE]` 并填写完成记录；如索引受影响，同步更新 `TODO.md`；仅在阶段计划变化时更新 `PLAN.md`。
9. 检查工作区中当前任务相关的未提交变更，按要求提交本次结果。
10. 停止，不进入下一个任务。

## 进度记录

- 已创建本文件，并完成任务索引读取。
- 已确认首个未完成的详细任务为 `TODO-P6.md` 中的 `P6-T02ma`：发布 authoritative surface-resume dispatch-source inventory。
- 已检查最新提交：`[P6-T02ma] Track surface-resume dispatch-source prerequisite`。该提交正是为当前任务插入前置项，不存在需要额外抢先处理的其它历史未完成问题。
- 已完成对 `effect_lowered` 与 `llvm/codegen/effect_refactor` 现状的定向审查。
- 已确认两个关键 published 形状：
  - `effect_refactor_step_enum_single_case.scoop` 中，`k0` 同时出现在 `ko1` 的 `c0/c1` surface case，但只有 `ri0::c0` 是可达 method；`k1` 仅以 unreachable shell 出现。
  - `effect_resume_if_else_branch_single_perform.scoop` 中，`k3` 仅出现在 `Resume(site9)` lowering；`ko1` 的 object shell 只发布 `k0/k1`。同时 handle arm continuation binder 仍依赖 `k0`。
- 已确认当前错误边界：LLVM ABI materializer 仍把 resume-site 与 handle-binder source 伪装成 `continuation_object.surface_resume_bindings()`，这会把 object source、resume-site-only source 与 binder source 混成一类。
- 接下来的实现步骤：
  1. 在 `effect_lowered::ir` 中为 `LateLoweredProgram` 增加 surface-resume dispatch inventory 及稳定查询 API。
  2. 让 `LateLoweredProgram::new(...)` 基于 continuation object surface/method、resume boundary、handle continuation binder 自动派生该 inventory，并把它纳入 stable dump。
  3. 让 LLVM `effect_refactor` layout 改为消费 inventory 生成 `surface_resume_layout`，并只把 object 自己的 surface case 作为 object-side binding；停止把 resume boundary/binder 混成 object binding。
  4. 更新 handle continuation binder layout，使其引用 authoritative inventory/layout，而不是假设 object binding 必然存在。
  5. 补充 late-lowered/LLVM 定向测试，覆盖 shared-schema、resume-site-only、handle-binder 三类 schema。
- 上述 1-5 已完成：
  - `LateLoweredProgram` 已新增 surface-resume dispatch inventory，并在 stable dump 中发布；
  - inventory 已覆盖 shared-schema object/method、resume-site-only、handle-binder-only、以及 unreachable source；
  - LLVM ABI materializer 已改为从 inventory 生成 `surface_resume_layout`；
  - continuation object layout 现在只发布 object-side surface case binding，不再把 resume boundary/binder 混入 object binding；
  - handle continuation binder layout 现在直接读取 authoritative dispatch-source kind / return-step schema，而不是假设 object binding 必然存在。
- 已完成验证：
  - `cargo test -p scoopc refactor_surface_resume_dispatch_inventory`
  - `cargo test -p scoopc refactor_llvm_surface_resume_layout`
  - `cargo test -p scoopc refactor_handle_arm_continuation_binding`
  - `cargo test -p scoopc refactor_llvm_continuation_layout`
  - `cargo run -p scoop -- --effect-pipeline refactor dump-effect-lowered tests/fixtures/build/effect_refactor_step_enum_single_case.scoop`
  - `cargo run -p scoop -- --effect-pipeline refactor dump-effect-lowered tests/fixtures/run-pass/effect_resume_if_else_branch_single_perform.scoop`
  - `cargo clippy -p scoopc -p scoop --all-targets -- -D warnings`
- 下一步：把 `P6-T02ma` 标记为 `[DONE]`，补齐完成记录，然后准备按任务要求提交。
