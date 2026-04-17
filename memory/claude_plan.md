# 本轮执行计划

## 说明

按要求先记录本轮的执行计划、检查顺序和进度更新点。这里记录的是可核查的工作思路摘要与执行步骤，不包含冗长的内部推理。

## 初始计划

1. 检查最近一次 Git 提交的信息，确认是否提到了已知问题、临时修复或待补项。
2. 如果最近提交提到任何遗留问题，先定位并修复这些问题，再继续处理 `TODO.md` 中的任务。
3. 阅读 `TODO.md`，定位第一个未完成任务。
4. 阅读 `PLAN.md`，确认当前任务上下文、依赖关系和已有拆分是否一致。
5. 评估该任务是否可以在本轮完整完成：
   - 如果可以，直接实现、补测试、验证并更新文档。
   - 如果过大或被前置缺陷阻塞，则先在 `PLAN.md` / `TODO.md` 中拆分或重排任务，再处理新的首个子任务或按要求停下。
6. 实现任务时，优先保证与规范一致；如果发现任何规范偏差、缺失能力或依赖性缺陷，必须先把问题转化为前置任务并更新计划。
7. 运行相关验证：
   - 任务相关最小测试集
   - 必要时运行更大范围测试
   - `cargo fmt`
   - `cargo clippy --all-targets -- -D warnings`（如果改动范围允许且环境支持）
8. 更新 `TODO.md`、`PLAN.md` 和本文件中的进度记录。
9. 提交 Git commit，完成后停止，不进入下一个任务。

## 进度记录

- 状态：已完成初始化与任务定位。
- 已确认：
  - 最近一次提交 `8805d05abc3071bbde8f3d79f2725aa781ea628b` 未在提交说明中直接引入必须先修的遗留 issue。
  - `TODO.md` 中第一个未完成任务是 `T3009b0a2`：修正 unified `RuntimeRaiseBoundary` 的 inactive-continue / active-dispatch 合同。
  - `PLAN.md` 当前顺序与 `TODO.md` 一致，`T3009b0a2` 位于 `T3009b0a1cR` 之后，是当前 effect 主线的下一步。
- 当前判断：
  - 该任务目前看起来可直接实现，暂不需要先拆分。
  - 高概率需要修改 unified state-machine emitter 中 `RuntimeRaiseBoundary` 对共享 `Suspend` terminator 的接线方式，使其在 boundary 求值后按 TLS active 结果分流：inactive 继续 caller-tail，active 才 outward dispatch。
- 下一步：
  1. 阅读 `state_machine_emitter.rs`、相关 plan/transform 合同。
  2. 运行任务列出的最小复现，确认当前失败形态。
  3. 实现修复并补必要测试。
  4. 跑任务验收、更新 `TODO.md` / `PLAN.md` / 本文件并提交。

## 本轮结果

- 已完成实现：
  - 在 `crates/scoopc/src/llvm/codegen/effect/state_machine_emitter.rs` 中，把 `SuspendSiteKind::RuntimeRaise` 纳入 shared `Suspend` terminator 的 inactive-continue 集合。
  - 这样 `RuntimeRaiseBoundary` 在 boundary 表达式求值后会先检查 TLS active：
    - inactive：把当前结果写入 frame resume 槽并 branch 到 `resume_state`
    - active：保留现有 continuation + outward dispatch 路径
- 已新增测试：
  - `runtime_raise_boundary_ir_branches_between_inactive_continue_and_active_dispatch`
- 已完成验证：
  - `cargo test -p scoopc runtime_raise_boundary_ir_branches_between_inactive_continue_and_active_dispatch -- --nocapture`
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/type_check_cast_is_as_asq_basic.scoop`
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_escape_continuation_resume_unit.scoop`
  - `cargo fmt`
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
- 文档状态：
  - 已将 `TODO.md` 中的 `T3009b0a2` 标记为完成。
  - 已将 `PLAN.md` 当前执行顺序前移到 `T3009b0`。
- 待执行：
  - 提交本轮 commit 后停止。
- 收尾更新：
  - 已检查最终 diff，当前工作区只包含本轮任务相关变更。
