# 本轮执行计划（关键判断摘要）

## 目标

按 `TODO.md` 的顺序只完成第一个未完成任务；若在执行前或执行中发现阻塞该任务的既有问题、规格偏差或缺失能力，则先把问题整理为更前置的任务并更新 `TODO.md` / `PLAN.md`，提交后停止。

## 约束与执行原则

1. 先检查最新提交是否提到任何已知问题；若有，必须先修复这些问题，再处理 `TODO.md` 中的任务。
2. 在确认首个未完成任务前，不对实现方案做未经仓库上下文验证的假设。
3. 若首个任务过大或存在隐含依赖，需要把它拆分为可执行子任务，并同步更新 `PLAN.md` 与 `TODO.md`。
4. 实现后必须运行相关测试，并尽量补齐格式化、`clippy`、以及任务相关回归验证。
5. 完成后更新文档状态、提交 git commit，然后停止，不继续下一个任务。

## 初始步骤

1. 查看最新一次 git 提交信息，确认是否包含待修复的既有问题。
2. 读取 `TODO.md`、`PLAN.md`，确定第一个未完成任务及其上下文。
3. 检查工作区状态，识别是否存在用户未提交改动，避免误覆盖。
4. 根据任务所在模块阅读相关代码与测试，确认实现边界。
5. 如需拆分任务，先更新规划文件，再开始代码改动。

## 进度记录

- 已检查最新提交：`15409ac2ca8167fb4055fb3ed859da7da7805afd`，提交信息未显式记录需优先修复的既有 issue。
- 已读取 `TODO.md` / `PLAN.md`，当前首个未完成任务为 `T2003r3d4`。
- 已阅读 unified multi-resuming 入口与现有 leaf：
  - `MultiResuming` 目前只接三条路：`multiple immediate only`、`multiple escape only`、`1 immediate + 1 escape`。
  - `UnsupportedMixedMultipleEscapeWithImmediate` 与 `UnsupportedMixedMultipleImmediateWithEscape` 仍由 simplification / entrypoint 分类器硬编码保留。
  - 代码结构显示 `1 immediate + N escape` 与 `N immediate + 1 escape` 不是同一个“小开关”：
    - 前者涉及多个 escape arm 的 site 解析、dispatch 与 handler-frame/state contract；
    - 后者涉及多个 immediate site 的 stack-reentry 状态与单个 heap-continuation 路径的组合。
- 结论：`T2003r3d4` 需要先拆分为更小子任务，避免把两类正交状态机改动耦合到同一轮中。

## 更新后的执行策略

1. 先把 `T2003r3d4` 拆成共享 contract / resolver 子任务与后续 emitter 子任务，并同步更新 `TODO.md` / `PLAN.md`。
2. 立即实现拆分后的第一个子任务：
   - 优先收口 multi-resuming arm mix 的 shared resolver / metadata contract；
   - 让现有 leaf 复用该 shared contract，补 plan 级测试；
   - 暂不放开 `UnsupportedMixedMultiple*` 的 emitter 选路。
3. 跑与该子任务直接相关的定向测试与 `clippy`。
4. 更新 `memory/claude_plan.md`、`TODO.md`、`PLAN.md`，提交拆分与实现结果，然后停止。

## 说明

这里记录的是可审计的关键判断与执行计划摘要，用于追踪进展；不会逐字暴露原始内部推理。

## 本轮结果

- 已完成 `T2003r3d4a`：
  - shared 层新增 multi-resuming arm/site contract，统一恢复 immediate / escape arm metadata、ordered site sequence 与 capture 聚合；
  - `multi_resuming.rs`、`multi_resuming_heap.rs`、`multi_resuming_mixed.rs` 已切到复用这套 contract；
  - 保持了既有行为边界：multiple-immediate 仍保留 top-level gate，mixed `1 immediate + 1 escape` 不回退 nested immediate 支持。
- 已新增定向测试：
  - `resolve_multi_resuming_immediate_sites_from_plan_keeps_arm_metadata`
  - `resolve_multi_resuming_escape_sites_from_plan_keeps_nested_direct_arm_dispatch`
  - `resolve_multi_resuming_escape_sites_from_plan_duplicates_indirect_sites_per_arm`
- 已完成验证：
  - `cargo fmt --all`
  - `cargo test -p scoopc llvm::codegen::effect::tests:: -- --nocapture`
  - `cargo clippy --workspace --all-targets -- -D warnings`
- 下一步（不在本轮执行）：`T2003r3d4b`，即放开 unified emitter 的 `1 immediate + N escape`。
