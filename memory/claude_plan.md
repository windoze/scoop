# 执行计划与进度记录

说明：按要求先写计划文件，再开始任何仓库检查或命令执行。这里记录的是可审计的执行计划、决策依据、风险与进度更新，不包含原始内部思维链。

## 初始目标

本次调用只完成一件事：定位 `TODO.md` 中第一个未完成任务，完整实现并验证它，然后更新文档、提交 Git commit，并停止。

## 初始执行计划

1. 检查最新一次提交的信息，确认是否提到了已知问题、回归或待修复项。
2. 阅读 `TODO.md`，识别第一个未完成任务。
3. 阅读 `PLAN.md`、`README.md` 与相关代码，建立该任务上下文。
4. 判断该任务是否足够小且可在本次调用内完整完成。
5. 如果任务过大：
   - 将任务拆成更小的子任务。
   - 更新 `PLAN.md`。
   - 更新 `TODO.md`，把当前任务替换或扩展为依赖正确的子任务列表。
   - 执行拆分后的第一个子任务。
6. 如果任务可直接执行：
   - 修改代码实现任务。
   - 为变更补充或调整测试。
   - 运行相关检查与测试，至少覆盖：
     - 受影响模块测试
     - `cargo test --all`
     - `cargo clippy --all-targets -- -D warnings`
     - 必要时 `cargo fmt --check` 或 `cargo fmt`
7. 如果在实现过程中发现任何规范不匹配、缺失特性或现有 bug：
   - 不绕过问题。
   - 先在 `TODO.md` 中新增前置修复任务并调整顺序。
   - 在 `PLAN.md` 中记录阻塞原因。
   - 如因此无法继续当前任务，则提交文档调整并停止。
8. 任务完成后：
   - 在 `TODO.md` 标记完成。
   - 更新 `PLAN.md` 当前状态。
   - 如有必要，更新 `README.md` 或内联注释。
   - 提交一个清晰的 Git commit。
   - 停止，不继续下一个任务。

## 约束与检查点

- 不回退或覆盖用户已有修改。
- 不使用规避性实现、fixture-only hack 或临时兼容层来冒充完成。
- 任何新增修改都应尽量保持模块边界清晰。
- 若发现 `PROMPT.md` 被意外修改，需要纳入本次提交。

## 进度日志

- 已创建计划文件，尚未开始仓库检查。
- 已检查最新提交：`17014d072c0f6a6c729490586dc866b8abe155de [T3009b0a1e] Fix NestedHandleBoundary inactive-path contract`。提交说明未额外标注需要先修的遗留问题。
- 已读取 `TODO.md` 与 `PLAN.md`，当前第一个未完成任务是 `T3009b0a1eR`：Review `NestedHandleBoundary` 的 inactive-path 是否真正统一收口到 state-machine 合同。

## 当前任务：T3009b0a1eR

### 任务目标

1. 审查 `NestedHandleBoundary` 的 inactive/active 分流是否只由统一 contract + TLS active 驱动。
2. 审查 inactive 成功路径是否通过 authoritative resume transport（`resume_path` + synthetic resume slot）继续 caller-tail，而不是重跑 inner handle。
3. 审查生产代码中是否存在 outer emitter、普通 call codegen、shape-based 分流或针对 nested handle 的局部补丁。
4. 若发现问题，直接修复并补验证；若未发现问题，则完成 review 记录、更新 `TODO.md` / `PLAN.md` 并提交。

### 已完成的上下文检查

- 已定位 `state_machine_plan.rs` 中 `hir::ExprKind::Handle` 的建模：会在 `nested_may_suspend` 时创建 `SuspendSiteKind::NestedHandleBoundary`、生成 `ResumeAfterSite`，并为 source expr 分配 synthetic resume slot。
- 已定位 `state_machine_transform.rs` 中的结构测试 `nested_handle_boundary_preserves_resume_path_and_slot`，它锁定了 plan → segment → unified machine 的 `resume_path` 保真和 `__resume_site*` 改写。
- 已定位 `state_machine_emitter.rs` 中的共享分流：`suspend_site_uses_inactive_continue_path()` 已将 `NestedHandleBoundary` 纳入 inactive-continue 集合；`UnifiedStateTerminator::Suspend` 会在 TLS inactive 时写回 authoritative result 并 branch 到 `resume_state`。
- 已定位 `expr.rs` / `hir/lower/expr.rs`：`ExprKind::Handle` 的 codegen 统一走 `codegen_handle_expr`，HIR lowering 也保留 typechecked handle result type，没有发现 nested-handle 专用分流入口。

### 待执行验证

1. 跑 nested-boundary 定向单测与 fixture，确认 inactive-path / active-path 行为。
2. 跑 `cargo test --all`。
3. 跑 `cargo clippy --all-targets -- -D warnings`。
4. 若 review 通过，更新 `TODO.md` / `PLAN.md` / 本文件并提交。

### 结果

- 未发现需要在本任务内修复的新生产代码问题。
- 复审结论：
  1. `NestedHandleBoundary` 的 inactive/active 分流仍只由 unified `Suspend` terminator 读取 `SuspendSiteKind::NestedHandleBoundary` + TLS active 决定。
  2. inactive-path 的 authoritative 结果通路仍是 `resume_path` + synthetic resume slot；resume-after-site 后续表达式会读取 `__resume_site*`，不会重跑 inner handle。
  3. `ExprKind::Handle` 的 lowering / codegen 入口仍统一收口到 state-machine 主线，未发现 outer emitter、普通 call codegen 或 shape-based 旁路。

### 本轮验证结果

- `cargo test -p scoopc nested_handle_boundary_preserves_resume_path_and_slot -- --nocapture`：通过。
- `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_handle_nested_handle_boundary_inactive_basic.scoop`：通过，输出与 golden 一致。
- `cargo test --all`：通过。
- `cargo clippy --all-targets -- -D warnings`：通过。

### 收尾动作

- 已把 `T3009b0a1eR` 在 `TODO.md` 标记为完成，并记录审查结论。
- 已更新 `PLAN.md`，把下一项执行顺序推进到 `T3009b0a1cR`。
