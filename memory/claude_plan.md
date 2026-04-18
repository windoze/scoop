# 当前执行计划

## 约束说明

- 按用户要求，本文件会持续记录“可审计的执行计划、关键判断依据摘要、进度更新”。
- 不记录模型内部私有推理细节；仅记录足以复查工作的步骤、结论、变更原因与后续动作。

## 初始目标

本轮只完成 `TODO.md` 中**第一个未完成任务**，并在完成后停止。

## 执行步骤

1. 检查最新一次 Git 提交的提交信息与改动，确认是否提到需要先修复的既有问题。
2. 阅读 `TODO.md`，定位第一个未完成任务。
3. 阅读 `PLAN.md`，核对该任务是否已有分解或依赖说明。
4. 判断该任务是否足够小且可在本轮完整实现。
   - 若可直接完成：继续实现。
   - 若过大或被前置缺陷阻塞：先更新 `TODO.md`/`PLAN.md` 做任务分解或依赖重排，然后提交并停止。
5. 实现当前任务。
6. 运行相关格式化、lint、测试，至少覆盖：
   - 与改动直接相关的测试；
   - `cargo clippy --all-targets -- -D warnings`；
   - 必要时运行更大范围测试以验证没有回归。
7. 更新文档与计划：
   - 在 `TODO.md` 中标记任务完成，或在阻塞时按要求重排任务；
   - 在 `PLAN.md` 中反映当前状态；
   - 持续更新本文件记录关键进展。
8. 用清晰的提交信息提交所有改动。
9. 停止，不进入下一个任务。

## 当前状态

- 状态：已完成初始梳理。
- 已确认事项：
  - 最新提交 `7aaf2e9134532e37087b2364c53c5932bf9c013f` 为 `[T3016cR] Review outer-body continuation resume lowering`。
  - 该提交信息本身未声明新的、需先于 `TODO.md` 首项任务处理的既有问题。
  - `TODO.md` 中首个未完成任务为 `T3016d`：修正 GC stress 下 escaped continuation 与深对象图捕获的存活 / 恢复缺口。
  - `PLAN.md` 当前执行顺序也已明确下一项为 `T3016d`。
- 当前判断：
  - 需要先复现 `T3016d` 列出的 3 条目标 fixture / 相关 runtime 测试，确认失败形态与最小根因。
  - 若问题范围明显超出单轮可控修复，再按要求拆分任务并更新 `TODO.md` / `PLAN.md`；否则直接实现。
- 下一步：
  1. 运行 `T3016d` 指定的 3 条 GC stress 相关 fixture，记录失败现象。
  2. 检查 continuation/runtime tracing 相关实现，定位是根集合、trace、payload transport 还是恢复路径问题。
  3. 视结果决定直接修复或先拆分任务。

## 关键进展（执行中）

- 已复现 `T3016d` 的三类失败：
  - `effect_escape_continuation_gc_stress_multi_string.scoop` / `gc_continuation_escape_alloc_heavy_resume.scoop` 在 `SCOOP_GC_STRESS=1` 下都读到 `missing1/missing2/missing3`；
  - `gc_continuation_escape_deep_object_graph.scoop` 在同一环境下长时间无输出并超时。
- 已定位第一层根因：
  - 统一 effect lowering 生成的 `scoop.effect.step.*` / `scoop.effect.dispatch.*` 函数没有设置 `gc "statepoint-example"`；
  - 结果是这些函数内部的 `scoop_alloc_typed` / `scoop_println` / `scoop_continuation_alloc` 等调用不会进入 `rewrite-statepoints-for-gc`，escaped continuation 与 captured refs 在 GC stress 下不可见。
- 已实施第一轮修复：
  - 为 `step_fn` 与 `dispatch_loop_fn` 补上 `gc "statepoint-example"`。
  - 新增 IR 回归测试，锁定 effect runtime functions 在跑过 pass pipeline 后必须带 GC strategy 且包含 statepoint。
- 第一轮修复后暴露出第二层真实缺口：
  - `EscapeContinuation` arm 中的 continuation binder 在 state block 内落成临时 `alloca`，跨 `println("arm")` safepoint 存活；
  - 该栈槽不等价于普通入口块 spill slot，导致拿到陈旧 continuation 指针，出现“能 resume 但值/控制流错乱”的中间失败态。
- 已实施第二轮修复：
  - 将 arm binder 与 fallback continuation binder 的临时栈槽改为入口块 `alloca`（复用 `create_entry_alloca`），与普通局部变量一致地进入 GC spill-slot 合同。
- 当前验证结果：
  - 3 条目标 GC stress fixtures 现均可在 `SCOOP_GC_STRESS=1` 下输出预期结果。
  - 下一步：跑 `cargo test -p scoop_runtime continuation_ -- --nocapture`、`cargo test --all`、`cargo clippy --all-targets -- -D warnings`，然后更新 `TODO.md` / `PLAN.md` 并提交。
## 2026-04-18 本轮接手补充

### 当前判断
- 已接手上一轮实现结果；根据现有交接信息，`TODO.md` 的首个未完成任务仍是 `T3016d`，且核心代码修复与主要测试已完成，但尚未执行 `cargo clippy --all-targets -- -D warnings`，也尚未更新 `TODO.md`、`PLAN.md`，更未提交 commit。
- 最新提交 `7aaf2e9` 仅提到 `T3016cR` review，未显式引入额外必须先处理的遗留问题；当前优先级仍是完成并收尾 `T3016d`。

### 本轮执行计划
1. 检查工作区状态，并复核 `TODO.md`、`PLAN.md`、`memory/claude_plan.md` 与已修改代码，确认交接摘要与当前文件内容一致。
2. 运行 `cargo clippy --all-targets -- -D warnings`。若发现 lint，立即做最小必要修复并重新验证。
3. 视情况补跑与 `T3016d` 直接相关的关键测试，确保 GC stress escaped continuation 修复没有回退。
4. 更新任务文档：
   - 在 `TODO.md` 将 `T3016d` 标记为完成；
   - 在 `PLAN.md` 记录本轮完成内容与下一项顺序；
   - 在本文件继续记录关键结果。
5. 用明确的 commit message 提交本轮改动，然后停止，不进入下一项任务。

### 约束与执行原则
- 只完成一个任务：`T3016d`。
- 若 `clippy` 或最终验证暴露新的规范性缺口，必须先把该缺口作为前置任务写入 `TODO.md` / `PLAN.md`，然后按依赖顺序处理，不能用绕过方式把当前任务算作完成。
- 编辑文件时使用 `apply_patch`，不回滚用户已有修改，不使用破坏性 git 命令。

## 2026-04-18 收尾进展

- 已复核最新提交 `7aaf2e9 [T3016cR] Review outer-body continuation resume lowering`；提交信息与改动范围都没有引入需要先于 `T3016d` 处理的额外遗留问题。
- 已执行 `cargo clippy --all-targets -- -D warnings`，结果通过，没有新增 lint 或需要拆分的新前置任务。
- 沿用上一阶段已完成的验证结果：新增 IR 回归 `effect_runtime_functions_use_gc_statepoint_strategy` 通过；三条 `SCOOP_GC_STRESS=1` 目标 fixture 均恢复；`cargo test -p scoop_runtime continuation_ -- --nocapture` 与 `cargo test --all` 已通过。
- 已更新 `TODO.md`：将 `T3016d` 标记为完成，并记录两层真实修复内容：
  - effect runtime functions 进入 GC statepoint pipeline；
  - arm binder / escaped continuation fallback binder 改为 entry-block spill slot。
- 已更新 `PLAN.md`：把 effect 主线下一项推进到 `T3016dR`。
- 下一步：提交本轮改动，commit message 计划使用 `[T3016d] Fix GC-stress escaped continuation root visibility`，提交后立即停止。
