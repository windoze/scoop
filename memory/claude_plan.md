# 执行计划与决策记录

## 约束说明

- 按用户要求，先记录执行计划，再执行任何仓库检查命令。
- 这里记录的是可审阅的执行计划、关键决策与进展，不包含逐字的内部推理。
- 本轮目标是：
  1. 检查最新提交是否提到需要先修复的既有问题。
  2. 读取 `TODO.md`，定位第一个未完成任务。
  3. 如任务过大，则拆分并同步更新 `PLAN.md` / `TODO.md`。
  4. 只完成一个任务，补充测试、文档、计划状态，并提交 Git commit 后停止。

## 初始步骤计划

1. 查看最新提交信息，确认是否显式提到需要优先修复的历史问题。
2. 查看 `TODO.md`、`PLAN.md`、`README.md` 的当前状态，识别第一个未完成任务及相关上下文。
3. 检查当前工作区状态，避免覆盖用户已有修改。
4. 评估首个未完成任务的范围：
   - 若可在本轮完整交付，则直接实现。
   - 若过大或存在前置依赖/规格缺口，则先在 `TODO.md` / `PLAN.md` 中拆分或前移依赖任务。
5. 实施代码修改，并同步补充必要注释/文档。
6. 运行与改动相关的测试；如涉及整体质量要求，再运行 `cargo fmt`、`cargo test`、`cargo clippy --all-targets -- -D warnings` 或与任务更匹配的子集命令。
7. 更新 `TODO.md`、`PLAN.md`、本文件，标记本轮结果。
8. 使用清晰的提交信息提交本轮变更，然后停止。

## 进展记录

- 已创建本文件并写入初始计划，尚未开始仓库检查命令。
- 已检查最新提交 `f1424d5f0cde3a1fe6d25487e35cfba3ae7ffa6b`，提交标题为“`[T3010b2b1b0] Track synthetic resume slot ID collision blocker`”，说明当前有一个刚被显式前移的 blocker，需要优先处理。
- 已读取 `TODO.md` / `PLAN.md` / `README.md`，确认当前第一个未完成任务是 `T3010b2b1b0`：修正 synthetic resume slot 的 `SymbolId` 冲突与 nested handle frame seeding 合同。

## 当前轮聚焦任务

- 任务编号：`T3010b2b1b0`
- 任务摘要：修复 synthetic resume slot 与现有局部变量共享 `SymbolId` 导致的 env / frame-slot 查找冲突，并同步修正 nested handle 入口 frame seeding 合同，避免 inner handle 把 outer 普通局部误识别为 synthetic resume slot。

## 当前实现计划

1. 复现 `T3010b2b1b0` 对应失败：
   - 优先查看相关 fixture / 单测；
   - 跑最小定向命令确认实际错误形态和触发链路。
2. 审查 `resume_path`、synthetic resume slot 分配、frame seeding、env 查找相关实现：
   - `state_machine_plan.rs`
   - `state_machine_transform.rs`
   - `state_machine_emitter.rs`
   - 以及 nested handle frame seeding 的调用点
3. 设计并实施修复：
   - 保证 synthetic resume slot 拥有不会与用户符号冲突的稳定标识；
   - 保证 nested handle seeding 只按正确的 slot 身份/来源写入 frame。
4. 补充回归测试：
   - 至少覆盖 `SymbolId` 冲突不再发生；
   - 覆盖 nested handle / outer frame seeding 不再把普通局部写成 synthetic resume slot。
5. 跑定向验证与质量命令。
6. 更新 `TODO.md`、`PLAN.md`、本文件，并提交 commit 后停止。

## 当前结果

- 已完成 `T3010b2b1b0` 的实现：
  - synthetic symbol 分配改为共享 floor/cursor 模式，避免 outer/local/nested handle 间复用同一 `SymbolId`；
  - `seed_outer_scope_frame_slots` 只再 seed 显式 outer-scope slot，不再按 `env.get(id)` 的偶然命中误种 synthetic resume slot。
- 已新增两条回归单测：
  - `nested_handles_allocate_unique_synthetic_resume_slot_ids`
  - `nested_handle_outer_scope_seeding_marks_only_real_outer_slots`
- 已通过的验证：
  - `cargo test -p scoopc nested_handle_ -- --nocapture`
  - `cargo test -p scoopc nested_handles_allocate_unique_synthetic_resume_slot_ids -- --nocapture`
  - `cargo fmt --all`
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_escape_continuation_nested_arm_indirect_performs_outer.scoop`
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
- 额外结果：
  - 原本用于定位本任务的 `effect_escape_continuation_nested_arm_indirect_performs_outer.scoop` 现已直接通过。
  - 继续复跑 `cargo run -p scoop --features llvm -- test` 时，先后发现多条历史 xfail expectation 已过时并回收为 pass：
    - `effect_custom_nonresuming_nested_nearest_and_arm_outside_scope.scoop`
    - `effect_escape_continuation_arm_performs_outer_effect.scoop`
    - `effect_escape_continuation_reperform_from_escape_arm.scoop`
    - `effect_handle_yield_and_step_finally.scoop`
    - `effect_handler_stack_nearest_and_arm_outside_scope.scoop`
  - suite 目前继续停在另一条 stale xfail `effect_handler_stack_nearest_three_levels_and_arm_outside_scope.scoop`；我没有在本轮继续无限扩张为完整 xfail 清理，而是把该 expectation cleanup 留给现有 `T3017` 统一处理。
