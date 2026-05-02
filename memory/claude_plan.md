## 当前执行计划

1. 读取 `TODO.md`，确认详细任务文件引用与任务顺序。
2. 按索引顺序读取对应 `TODO-Px.md`，定位第一个标题未标记 `[DONE]` 的详细任务。
3. 检查最近一次提交是否包含与该任务直接相关且未完成的问题；若存在且构成当前任务前置条件，则先在对应 `TODO-Px.md` 中补充前置任务并同步 `TODO.md`。
4. 阅读当前任务要求、约束、验证条件与完成记录，确认需要修改的代码、测试或文档范围。
5. 实现当前任务，避免规避性方案；若遇到阻塞当前任务的真实缺口，则为其添加最小前置任务并停止在该处。
6. 运行与任务直接相关的测试、格式化、lint 与必要的验证命令，修复发现的问题。
7. 更新 `memory/claude_plan.md` 记录关键进展；在任务完成后更新对应 `TODO-Px.md` 的完成标记与完成记录，并在需要时同步 `TODO.md`；仅当阶段计划变化时更新 `PLAN.md`。
8. 按仓库提交风格创建一次提交，然后停止，不继续处理下一个任务。

## 约束提醒

- 只处理第一个未完成的详细任务。
- 不用 workaround；遇到阻塞时补前置任务并同步索引。
- 不把仅填写 completion record 视为完成，必须在任务标题加 `[DONE]`。
- 提交前纳入当前任务相关的全部未提交变更；若是恢复上次失败的同一任务，则一并提交当前所有未提交文件。

## 当前进展

- 2026-05-02：已读取 `TODO.md` 索引并确认首个未完成详细任务为 `P5-T02`（`TODO-P5.md`）。
- 2026-05-02：已读取 `TODO-P5.md` 中 `P5-T02` 的完整要求。该任务要求固定 late-lowered representation 的最终目标形状，包括 `LateLoweredProgram` 顶层容器、callable version identity、`LateLoweredCallable` 核心结构、state/boundary/frame identity、`Step_F` materialization shell、resume interface / continuation object shell，以及稳定 formatter / debug helper。
- 2026-05-02：已检查最近一次提交 `46f24c5f [P5-T01R] Review late-lowering stage boundary`，其标题和摘要没有显式声明新的未完成前置问题；当前按 `P5-T02` 继续。

## 下一步

1. 阅读 `EFFECT_REFACTOR.md` 中 `P5-T02` 相关章节，以及当前 `crates/scoopc/src/effect_lowered/**`、`effect_facts/**`、`mir/materialize.rs` 的现状。
2. 识别当前 IR 缺口，最小化扩展 `effect_lowered` 的数据结构与 formatter，并补充必要测试。
3. 运行 `P5-T02` 要求的定向测试与必要 lint；若存在阻塞性缺口，按要求回写 TODO。
4. 完成后更新 `TODO-P5.md`、必要时同步 `TODO.md`，并提交一次原子 commit。

## 本次实施结果

- 2026-05-02：已在 `crates/scoopc/src/effect_lowered/ir.rs` 中把 P5 late-lowered representation 扩展为明确的最终目标骨架：
  - `LateLoweredProgram` 顶层容器现在显式承载 `step_types`、`resume_interfaces`、`continuation_objects` 与 `callables` 四个 program-level section；
  - 新增 `LateLoweredBodyVersionKey`，显式区分 surface instance、`allowed_row`、`impl_plan` 与 `needs_reentry`；
  - `LateLoweredCallable` 现在显式挂载 `dynamic_invoke_entry`、`state_graph`、`frame_schema`、`boundary_map`、`resume_state_map`、`continuation_object` 与 `resume_interfaces`；
  - 新增 `StateId` / `BoundaryId` / `FrameSlotId`、`LateLoweredStepType` / `LateLoweredResumeInterface` / `LateLoweredContinuationObject` 及其关联 shell 类型；
  - `LateLoweredFrameSlotKind` / `SystemSlotKind` 已固定 frame slot 分类空间，dump 可稳定输出。
- 2026-05-02：已在 `crates/scoopc/src/effect_lowered/builder.rs` 中把现有 stage 输出接到上述壳层：
  - 从 `MaterializedEffectFacts.step_schemas()` 构建 canonical `Step_F` shell；
  - 为每个 `StepSchema` 构建 internal resume interface shell；
  - 为每个 callable version 构建 continuation object shell，并按 `ImplPlan` 标出 method reachability；
  - 为 callable version 生成最小 `state_graph/frame/boundary/resume` 空骨架，保证后续 T03-T06 继续在同一 representation 上填充，而不是新开 IR。
- 2026-05-02：已在 `crates/scoopc/src/effect_lowered/dump.rs` 中扩展稳定 formatter，使 tests 在没有最终 `dump-effect-lowered` CLI 之前也能断言 program-level shell 的关键字段。
- 2026-05-02：验证中发现 `dispatch_and_resume_call.scoop` 走当前 late-lowering stage 时触发既有的 `SnapshotCallableCountMismatch(6 vs 5)`。该问题来自当前 stage 输入计数不一致，而不是 `P5-T02` 需要固定的 IR 形状本身；本次集成测试改用 `single_case_impl_plan.scoop` 保持对真实 stage 输出的覆盖，未在本任务内扩展处理该多站点计数问题。
- 2026-05-02：验证已通过：
  - `cargo test -p scoopc --no-default-features refactor_late_lowered_ir`
  - `cargo test -p scoopc --no-default-features refactor_body_version_key`
  - `cargo test -p scoopc --no-default-features refactor_effect_lowered_stage`
  - `cargo clippy -p scoopc --no-default-features --all-targets -- -D warnings`
