## 执行计划

1. 读取 `TODO.md`，只把它当作任务索引使用。
2. 按索引顺序读取对应的 `TODO-Px.md` 详细任务文件，定位第一个标题未带 `[DONE]` 的任务。
3. 检查最近一次提交是否有与该任务直接相关且明确未完成的事项；如果有，将其并入当前任务范围，或按要求作为前置任务写回详细 TODO。
4. 阅读与当前任务直接相关的代码、测试、规范与任务约束，确认需要修改的最小范围。
5. 实现当前任务；如遇到阻塞当前任务且不能规避的问题，按要求新增最小前置任务，并同步 `TODO.md`。
6. 运行与当前任务直接相关的验证；随后运行要求中的质量检查，至少包括相关测试，以及在可行时运行 `cargo clippy --all-targets -- -D warnings`。
7. 更新 `memory/claude_plan.md` 记录关键进展或计划变化。
8. 在对应 `TODO-Px.md` 中将当前任务标题标记为 `[DONE]` 并补充完成记录；若索引受影响，同步更新 `TODO.md`；仅在阶段计划变化时更新 `PLAN.md`。
9. 检查工作区状态，确保提交包含本次任务要求纳入的所有未提交文件。
10. 使用清晰的任务号提交信息创建一次 git 提交，然后停止，不继续下一个任务。

## 进度记录

- 已创建初始执行计划，下一步开始读取 `TODO.md` 与详细任务文件。
- 已确认首个未完成详细任务为 `P5-T04a`；它是 `P5-T04R` 明确插入的前置修复任务，目标是修复 frame lifting 目前基于 `LocalDecl.name.starts_with("tmp")` 的错误来源分类。
- 当前实现方案：
  1. 在 `crates/scoopc/src/mir/mod.rs` 为 `LocalDecl` 增加稳定的 local 来源枚举；
  2. 在 `crates/scoopc/src/mir/lower.rs` 让 `push_named_local` 标记源码 local，让 `push_temp_local` 标记 compiler temporary；
  3. 更新所有手工构造 `LocalDecl` 的测试/辅助代码以补齐新字段；
  4. 在 `crates/scoopc/src/effect_lowered/frame.rs` 改为读取 MIR 来源元数据，不再依赖名字启发式；
  5. 新增回归测试：源码名以 `tmp*` 开头且跨 boundary 存活时仍应被 frame 标为 `SourceLocal`，同时真正的 MIR temp 仍应被标为 `CompilerTemporary`；
  6. 运行任务要求的测试与 clippy，随后回写 `TODO-P5.md` / `TODO.md` / git 提交。
- 代码实现已完成：
  - `LocalDecl` 已新增稳定来源枚举 `LocalSourceKind`；
  - `push_named_local` / `push_temp_local` 已分别写入 `SourceLocal` / `CompilerTemporary`；
  - `effect_lowered/frame.rs` 已移除 `starts_with("tmp")`，改为读取 MIR 元数据；
  - 已新增 `refactor_frame_lifting_uses_stable_mir_local_source_metadata` 回归测试，覆盖源码 `tmp*` local 与真正 compiler temp 的 frame 分类。
- 验证已通过：
  - `cargo test -p scoopc --no-default-features refactor_frame_lifting`
  - `cargo test -p scoopc --no-default-features refactor_effect_lowered_stage`
  - `cargo clippy -p scoopc --no-default-features --all-targets -- -D warnings`
- `TODO-P5.md` 与 `TODO.md` 已同步：`P5-T04a` 标题已补齐 `[DONE]`，完成记录已写入，`PLAN.md` 保持不变。
- 提交策略更新：当前工作区存在与本任务链同区域、且显然早于本轮最终落盘的未提交状态（例如 `TODO-P5.md` 中 `P5-T04a`/`P5-T04R` 的前置阻塞记录）；按用户要求，将当前所有未提交文件一并纳入本次任务提交，以原子化记录“恢复并完成 `P5-T04a`”的最终状态。
- 下一步：检查 git 状态/差异与最近提交风格，生成提交消息并创建本任务提交。
- 当前会话恢复说明：先重新核对 `TODO.md` / `TODO-P5.md` 与工作区状态，确认首个未完成详细任务是否仍为 `P5-T04a`，以及此前记录的实现与验证结果是否已经真实落盘；若一致，则直接完成提交步骤；若不一致，则按最新事实继续修复并更新记录。
- 已重新核对任务顺序：`TODO.md` 与 `TODO-P5.md` 当前首个未完成详细任务为 `P5-T04R`，`P5-T04a` 已在最新提交 `[P5-T04a] Stabilize MIR local source classification` 中完成并落盘。
- `P5-T04R` 复核结果：未发现新的实现 blocker。`frame.rs` / `segment.rs` / `builder.rs` / `ir.rs` / `dump.rs` 已满足 review 关注点；搜索也未发现 P5 新主线依赖 pending-finally / pending-cleanup / handler-stack / cleanup-hook / legacy state-machine 作为 correctness 前提。
- 本轮验证已通过：`refactor_effect_lowered_stage`、`refactor_late_boundary_selection`、`refactor_owner_resume_state`、`refactor_late_lowered_ir`、`refactor_frame_lifting`、`refactor_late_control_flow`、`refactor_dropped_continuation`、`refactor_runtime_error_boundary` 与 `cargo clippy -p scoopc --no-default-features --all-targets -- -D warnings`。
- `TODO-P5.md` 与 `TODO.md` 已同步写回 `P5-T04R` 的 `[DONE]` 标记与完成记录；`PLAN.md` 仍无需改动。
- 下一步：检查本轮 diff，按仓库风格创建 `[P5-T04R] ...` 提交，然后停止。
