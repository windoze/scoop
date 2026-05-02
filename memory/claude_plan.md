## 当前轮次计划

1. 按要求先记录本轮执行计划与可见决策依据；后续如计划变化或关键步骤完成，持续更新本文件。
2. 读取 `TODO.md` 作为索引，再按其中引用顺序检查对应 `TODO-Px.md`，定位第一个标题未标记 `[DONE]` 的详细任务。
3. 检查最近提交是否有与该任务直接相关且明确未完成的问题；若存在且构成当前任务前置，则将其并入当前任务范围或在对应 `TODO-Px.md` 中补充最小前置任务，并同步 `TODO.md`。
4. 阅读当前任务涉及的代码、规格、测试与依赖，确认需要修改的最小范围；若存在阻塞且无法在本轮直接完成，则按要求只添加最小前置任务并停止。
5. 实现当前任务，避免规避式修补；必要时补充或调整测试，确保行为与任务要求一致。
6. 运行与本任务直接相关的验证；随后运行要求中的质量检查，至少包含相关测试、`cargo test --all`（如影响范围需要）以及 `cargo clippy --all-targets -- -D warnings`（若仓库当前状态允许）。若发现问题，立即修复。
7. 将任务在对应 `TODO-Px.md` 中标记为 `[DONE]` 并更新完成记录；若任务标题、顺序或依赖变化，同步更新 `TODO.md`。仅在阶段计划发生真实变化时更新 `PLAN.md`。
8. 检查工作区差异，避免回退他人修改；按要求提交本轮所有相关更改，提交信息包含任务号；提交后停止，不继续下一个任务。

## 执行约束

- 不使用变通方案绕过规格缺口；若发现阻塞当前任务的真实缺陷，先修复或补充最小前置任务。
- 不把仅填写完成记录视为完成，只有任务标题显式加上 `[DONE]` 才算完成。
- 如本轮是在恢复上次未完成任务且当前存在未提交改动，完成后需一并提交。
- 进度更新仅记录可见决策、发现、变更和验证结果，不记录隐藏推理。

## 进度记录

- 已创建本轮计划文件，尚未开始读取任务索引。
- 已读取 `TODO.md` 与 `TODO-P5.md`，确认首个未完成详细任务为 `P5-T04`：实现 frame lifting，以及 `return` / `break` / `continue` / `finally` / cleanup / dropped continuation 的显式状态机合同。
- 已检查最近一次提交：`[P5-T03R] Record fact-driven segmentation review`。提交信息未声明与 `P5-T04` 直接相关且尚未完成的额外前置问题，因此当前按 `P5-T04` 原顺序继续。
- 下一步：阅读 `EFFECT_REFACTOR.md` 中 `§5.3.7`、`§5.3.9`、`§5.5.5-§5.5.6` 以及 `effect_lowered/{ir,builder,segment}.rs`、`mir/{mod,escape}.rs` 的现状，判断本任务是否可直接实现，或是否存在必须先补的最小前置任务。
- 本次调用继续沿用上述执行计划，先核对当前工作区与任务文档状态，再完成 `P5-T04` 或在确认阻塞后只补最小前置任务。
- 已复核 `TODO.md` / `TODO-P5.md` / 最新提交正文，当前首个未完成详细任务仍是 `P5-T04`，且最近提交未声明与之直接相关的未完成前置问题。
- 已阅读 `EFFECT_REFACTOR.md` §5.3.7、§5.3.9、§5.5.5-§5.5.6，以及 `effect_lowered/{ir,builder,segment,dump}.rs`、`mir/{mod,lower}.rs`。确认当前实现仍停留在 P5-T03/T02 骨架：`frame_schema` 为空、continuation captures 为空、state graph 只有无标签 successor，尚未显式记录 `return` / loop edge / handle dispatch / cleanup / drop / runtime-error 控制流合同。
- 当前实现具备继续推进 `P5-T04` 的必要输入：P3 direct-style MIR 已显式保留 `Return/Goto/CondBr/Perform/Handle/ResumeUnwind`、loop `break/continue` target、cleanup block、handle body/arm/finally target；P4 body/site facts 也已发布 handle solver facts、runtime-error outward、continuation schema。暂未发现必须前插的新前置任务。
- 实施方向已收敛为：
  1. 扩展 late-lowered IR，给 state graph 增加显式 terminator/edge contract，并让 frame slot 记录来源分类与读写点。
  2. 新增 `effect_lowered/frame.rs`，基于 MIR + boundary/resume state + effect facts 做 frame lifting，至少覆盖 source local、compiler temp、join value、handle binder、resume payload/result slot 与系统字段。
  3. 调整 `segment.rs`，让 handle/cleanup/runtime-error/drop 等控制流在 state graph 中显式可见，而不再只有裸 successor。
  4. 回填 continuation captures / dump / 测试，并在通过定向测试与 clippy 后再更新 `TODO-P5.md`/`TODO.md`/提交。
- 已完成代码实现：
  - `effect_lowered/ir.rs` 增加 `LateLoweredStateTerminator`、frame slot 读写点、`BoundaryResult` 等 frame kind；
  - `effect_lowered/segment.rs` 现发布显式 `Suspend/Goto/Branch/Return/HandleDispatch/ResumeUnwind/Abandon` terminator，并保留 handle body/arm/finally/cleanup 的显式 state edge；
  - `effect_lowered/frame.rs` 新增 frame lifting pass，发布 source local / compiler temp / join value / handle binder / resume payload / boundary result / system slot，并为 outward callable 附加独立 drop state；
  - `effect_lowered/builder.rs` 已接入 frame pass，continuation captures 不再为空；`dump.rs` 已输出新 contract。
- 已完成验证：
  - `cargo test -p scoopc --no-default-features refactor_effect_lowered_stage`
  - `cargo test -p scoopc --no-default-features refactor_late_boundary_selection`
  - `cargo test -p scoopc --no-default-features refactor_owner_resume_state`
  - `cargo test -p scoopc --no-default-features refactor_late_lowered_ir`
  - `cargo test -p scoopc --no-default-features refactor_frame_lifting`
  - `cargo test -p scoopc --no-default-features refactor_late_control_flow`
  - `cargo test -p scoopc --no-default-features refactor_dropped_continuation`
  - `cargo test -p scoopc --no-default-features refactor_runtime_error_boundary`
  - `cargo clippy -p scoopc --no-default-features --all-targets -- -D warnings`
- 已完成文档回写：`TODO-P5.md` 中 `P5-T04` 已加 `[DONE]` 并补齐 completion record，`TODO.md` 索引已同步；`PLAN.md` 无需改动。
- 下一步：检查工作区差异、确认仅提交本轮任务相关文件，然后创建 `P5-T04` 提交并停止。
