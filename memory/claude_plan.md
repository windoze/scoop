# 本次执行计划（决策摘要版）

说明：按要求记录执行计划、关键决策与进度更新。这里写的是可审计的决策摘要与步骤，不包含冗长的内部推理原文。

## 目标

完成 `TODO.md` 中第一个未完成任务；如果存在前置阻塞或最新提交提到的遗留问题，则先处理这些问题。完成后更新计划与任务状态，提交 git commit，然后停止。

## 初始步骤

1. 检查最新一次 git commit 的提交信息与改动，确认是否明确提到遗留问题需要先修复。
2. 阅读 `TODO.md`，定位第一个未完成任务。
3. 阅读 `PLAN.md`，确认当前计划、依赖关系与任务上下文。
4. 如首个未完成任务过大或存在缺失前置能力，先把任务拆分或重排，并同步更新 `TODO.md` 与 `PLAN.md`。

## 执行步骤

1. 实现当前应执行的那个任务。
2. 运行相关测试，并补充必要测试。
3. 如测试暴露规范不一致、缺失能力或遗留问题，先修复；若无法在本轮直接修复，则按要求把阻塞显式写回 `TODO.md`/`PLAN.md`。
4. 更新 `TODO.md` 与 `PLAN.md`，记录完成情况或依赖调整。
5. 提交一次 git commit，随后停止，不继续做下一个任务。

## 质量要求

1. 优先保证实现与规范一致，不接受临时绕过。
2. 需要关注 `cargo test --all`、相关定向测试，以及 `cargo clippy --all-targets -- -D warnings` 是否通过。
3. 不回退用户已有改动；若工作区存在无关修改，仅在必要范围内工作。

## 进度记录

- [已完成] 创建本计划文件并写入初始执行方案。
- [已完成] 检查最新提交、`TODO.md`、`PLAN.md`，确认本轮目标。
- [已完成] 确认最新提交 `c0152aa` 为 `T3009b0a1dR` review 提交；提交信息未声明额外遗留 bug 需要先于当前任务处理。
- [已完成] 确认 `TODO.md` 中第一个未完成任务为 `T3009b0a1e`：修正 unified `NestedHandleBoundary` 的 inactive-continue / active-dispatch 合同。
- [已完成] 阅读相邻任务与代码实现，确认该任务可直接实现，无需再拆分 `TODO.md`。
- [已完成] 在统一 state-machine 合同内实现 `NestedHandleBoundary` 的 inactive/active 分流，并补上 authoritative `resume_path` + synthetic resume slot，避免 inactive-path 重跑 inner handle。
- [已完成] 修复上游 HIR lowering：`ExprKind::Handle` 改为保留真实 result type，而不是一律写成 `Any`，从而避免 nested-boundary resume slot 被错误降成 `Ref`。
- [已完成] 新增 run-pass fixture `effect_handle_nested_handle_boundary_inactive_basic.scoop`、transform 单测 `nested_handle_boundary_preserves_resume_path_and_slot`，并同步更新 HIR golden。
- [已完成] 验证通过：
  - `cargo test -p scoopc nested_handle_boundary_preserves_resume_path_and_slot -- --nocapture`
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_handle_nested_handle_boundary_inactive_basic.scoop`
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
- [已完成] 更新 `TODO.md` / `PLAN.md` / 本文件，已把下一步推进到 `T3009b0a1eR`。
- [进行中] 提交 `T3009b0a1e` 相关变更并停止。

## 当前任务理解

- 当前问题位于统一 state-machine emitter 的共享 suspend boundary 合同。
- 已完成的 `SuspendCall` / `ObjectInitAccessBoundary` 都已接入“TLS inactive 时留在当前 state machine 内继续执行 caller-tail；TLS active 时才 outward dispatch”的合同。
- 当前待补的是 `NestedHandleBoundary`：outer `handle` 包 inner `handle` 时，inner handle 如果 inactive 成功返回，outer state machine 不应被误判为 suspend 并提前退出。
- 该任务看起来仍然是共享 boundary 规则收口问题，优先尝试直接实现；若发现它依赖新的缺失前置能力，再按要求回写 `TODO.md` / `PLAN.md`。

## 下一步

1. 把 `T3009b0a1e` 标记为完成，并把执行顺序推进到 `T3009b0a1eR`。
2. 检查工作区 diff，确认仅包含本轮任务相关变更。
3. 提交 git commit，然后停止。
