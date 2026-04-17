# 执行计划

## 说明

按要求先记录执行计划，再开始读取仓库状态与任务列表。这里记录的是可审计的执行步骤与决策摘要，不包含冗长的内部推演。

## 初始步骤

1. 检查最新一次 git 提交的提交信息与变更，确认是否提到需要先修复的既有问题。
2. 阅读 `TODO.md`，定位第一个未完成任务。
3. 阅读 `PLAN.md`，确认当前计划与 `TODO.md` 是否一致。
4. 如首个未完成任务过大，则将其拆分为更小子任务，并同步更新 `PLAN.md` 与 `TODO.md`；本次仅执行拆分后的第一个子任务。

## 执行原则

1. 优先处理最新提交中明确提及的既有问题；若存在，先修复这些问题。
2. 只完成一个任务，然后停止。
3. 若发现规范不匹配、缺失能力或前置缺陷，不能绕过，必须先在 `TODO.md`/`PLAN.md` 中登记为前置任务，并按要求调整顺序后停止。
4. 修改代码前后持续更新本文件，记录当前进展、计划变化和关键完成节点。

## 预期执行顺序

1. 审查最新提交。
2. 审查 `TODO.md` / `PLAN.md`。
3. 确定本轮目标任务。
4. 实现任务。
5. 运行相关格式化、lint、测试。
6. 更新 `TODO.md`、`PLAN.md`、本文件。
7. 提交 git commit，并停止。

## 当前进展（2026-04-18）

- 最新提交 `85b17ae8e4e822a37c48915d93a99f00c9c57d7e` 的提交信息未额外引入需要先修复的新遗留问题；当前待做项仍以 `TODO.md` 最前面的未完成任务为准。
- 已确认本轮首个未完成任务是 `T3009b2b1`：去掉 ordinary callee resumed-body restore 对 block 形状扫描的前提。
- 已定位当前问题根源：
  - `crates/scoopc/src/llvm/codegen/mod.rs` 中的 `build_block_callee_suspend_plan()` 通过扫描 block 语句形状，专门识别“单个 direct-perform 的 `val` 绑定”来决定是否生成 fresh/resume 双入口。
  - `CalleeSuspendPlan` 也直接编码了 `perform_stmt_index` / `perform_binding_id` / `perform_binding_ty`，说明恢复路径依赖该特定源码形状。
  - 统一 state-machine 侧已经有更通用的 suspend-site / resume-path 合同与重写逻辑，可用于替换这段 shape-based 入口判定。

## 当前实现计划

1. 在统一 state-machine 规划模块中增加一个只服务于 ordinary callee 的小型合同构建入口：
   - 复用现有 suspend-site / resume-path 分析。
   - 对单个 ordinary suspend site 产出：
     - 需要保存的 locals 元数据；
     - synthetic resume slot；
     - 已重写好的 resumed tail block。
2. 改写 `crates/scoopc/src/llvm/codegen/mod.rs` 的 ordinary callee 入口：
   - 删除 `build_block_callee_suspend_plan()` 及其 shape-scan 依赖；
   - top-level fun / closure body 改为从统一合同构建 `CalleeSuspendPlan`。
3. 改写 `crates/scoopc/src/llvm/codegen/effect/mod.rs` 的 resume prologue：
   - 不再恢复“perform 绑定”；
   - 改为恢复 synthetic resume slot，供 resumed tail block 统一读取。
4. 运行定向 fixture 与全量质量门槛：
   - 先跑已恢复的 indirect callee 相关 fixture；
   - 再跑 `cargo test --all`；
   - 最后跑 `cargo clippy --all-targets -- -D warnings`。

## 当前完成情况（2026-04-18）

- `T3009b2b1` 已完成。
- 实现结果：
  - `crates/scoopc/src/llvm/codegen/effect/state_machine_plan.rs` 新增基于统一 suspend-site / resume-path 元数据构建 ordinary callee plan 的入口，输出 `saved_locals + synthetic resume slot + rewritten resume_tail`。
  - `crates/scoopc/src/llvm/codegen/mod.rs` 不再保留 `build_block_callee_suspend_plan()` 这类源码形状扫描 helper；`CalleeSuspendPlan` 改为直接承载统一 contract 派生出的 resume tail。
  - `crates/scoopc/src/llvm/codegen/effect/mod.rs` 的 resume prologue 改为恢复 synthetic resume slot，而不是重建旧的 `perform` 绑定 local。
- 验证结果：
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_escape_continuation_indirect_perform_basic.scoop` 通过。
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_escape_continuation_indirect_perform_closure_locals.scoop` 通过。
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_escape_continuation_indirect_perform_resume_string.scoop` 通过。
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_escape_continuation_indirect_perform_resume_struct_with_ref.scoop` 通过。
  - `cargo test --all` 通过。
  - `cargo clippy --all-targets -- -D warnings` 通过。
- 剩余收尾：
  - 更新 git 状态并提交。
  - 本轮停止，不继续处理 `T3009b2bR`。
