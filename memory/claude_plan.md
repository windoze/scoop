# 执行计划

## 目标

本轮只完成 `TODO.md` 中第一个未完成任务；如果在检查最新提交、阅读任务、运行测试或实现过程中发现任何既有问题，则先修复这些问题，或在确认其为前置依赖后把它们插入 `TODO.md` 的当前任务之前，并更新 `PLAN.md` 后停止。

## 初始步骤

1. 检查最新一次 Git 提交信息，确认其中是否提到尚未修复的问题。
2. 阅读 `TODO.md`，定位第一个未完成任务。
3. 阅读 `PLAN.md`，理解现有计划、依赖和任务编号。
4. 结合任务复杂度判断是否需要把第一个未完成任务拆分为更小的子任务。
5. 如需拆分：
   - 更新 `PLAN.md`；
   - 更新 `TODO.md`，把拆分后的子任务插入正确顺序；
   - 选择第一个子任务作为本轮执行目标。
6. 实现目标任务时，先阅读相关代码与测试，确认实现边界。
7. 运行相关测试、必要的完整测试，以及 `cargo clippy --all-targets -- -D warnings`。
8. 修复执行过程中暴露的所有既有问题，不以规避方式推进。
9. 完成后更新 `TODO.md` 与 `PLAN.md`，记录实际进展和剩余任务。
10. 提交 Git commit，然后停止，不继续下一个任务。

## 约束

- 不接受绕过、fixture 特判、降级实现或缩小范围来“完成”任务。
- 若被前置缺陷阻塞，必须先把缺陷修复任务插入 `TODO.md` 的正确位置，并在 `PLAN.md` 说明原因。
- 在执行过程中如果计划变化或关键步骤完成，需要持续更新本文件。

## 当前状态

- 已创建初始计划文件。
- 已检查最新提交：最新提交仅更新计划文档，提交说明未声明待修的既有缺陷。
- 已读取 `TODO.md` / `PLAN.md`，确认当前主线顺序一致。
- 已确认第一个未完成任务为 `T4017f`：补齐 vtable / itable / object init / top-level init / extern thunk 等剩余边界，并删除 effect TLS 的主语义职责。

## 针对 T4017f 的当前执行计划

1. 读取 `TODO.md` / `PLAN.md` 中 `T4017f` 及其前置 `T4017a-e3` 的上下文，明确验收口径。
2. 搜索 effect TLS、`EffectCtx`、`EffectOutcome`、vtable/itable、object init、top-level init、extern thunk 相关实现，定位仍依赖 TLS side channel 的剩余边界。
3. 评估 `T4017f` 是否仍过大：
   - 如果可以在本轮内完成，则直接实现。
   - 如果存在明确且不可在本轮安全收口的前置 blocker，则按要求先在 `TODO.md` / `PLAN.md` 中插入前置任务并停止。
4. 修改实现并补回归测试，重点覆盖剩余边界路径不再以 effect TLS 为主语义来源。
5. 运行相关测试、全量测试与 `clippy`。
6. 完成后更新 `TODO.md`、`PLAN.md` 与本文件，提交 commit，并停止。

## 已完成的上下文确认

- 已确认当前尚未迁移到显式 `ctx + outcome` 的剩余 boundary 主要是：
  - vtable dispatch；
  - itable dispatch；
  - object init access（object value / object property）；
  - top-level immutable value init access；
  - direct `@Extern` / native call 边界。
- 已确认 direct / closure / funptr 路径已经具备可复用的显式 boundary helper 模式：
  - 先捕获当前 `EffectCtx`；
  - 在 boundary 上安装 handler stack top；
  - 调用 legacy callee；
  - 立即把 TLS 中的 legacy signal `consume` 到显式 `EffectOutcome`；
  - 恢复 handler stack top；
  - 再按 outcome 决定继续或向外传播。
- 已确认现有回归对 object-property/class-init 隐式 suspend 已有部分覆盖，但对 vtable / itable / top-level init / object value access / effectful extern boundary 仍缺少显式 outcome 形状或端到端覆盖。

## 接下来的具体改动

1. 在 effect contract / codegen 中抽取或复用统一的 legacy-call boundary helper。
2. 将 vtable / itable / object init / top-level init / extern 调用点接入该 helper，并删除这些路径上的 post-call TLS active probing。
3. 补 LLVM IR 测试，锁定相关函数体中出现 `scoop_effect_outcome_consume_current` / `publish`，且不再出现 `scoop_effect_is_active`。
4. 补 run-pass fixture，覆盖 virtual dispatch、interface dispatch、top-level init access，以及 object value access 的 outward-effect 行为。
## 2026-04-24 收尾更新

### 当前目标
- 本轮只完成 `TODO.md` 中第一个未完成任务 `T4017f`，不推进后续任务。

### 已知状态
- 先前实现已经把剩余 legacy effect boundary 迁移到 explicit outcome 模式，并补齐了相关 LLVM 单测与 run-pass fixtures。
- 先前验证记录显示以下命令已经通过：
  - `cargo fmt --all`
  - `cargo test -p scoopc --features llvm explicit_outcome_boundary -- --nocapture`
  - `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass`
  - `cargo run -p scoop -- test`
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`

### 收尾执行计划
1. 检查当前工作树与 `TODO.md` / `PLAN.md` / `memory/claude_plan.md` 状态，确认待收尾内容。
2. 更新 `TODO.md`，将 `T4017f` 标记为完成，并同步当前剩余实现顺序与验证记录。
3. 更新 `PLAN.md`，记录 `T4017f` 已完成并把主线切换到下一个未完成任务。
4. 回写 `memory/claude_plan.md`，记录关键步骤完成情况。
5. 复查差异后提交 git，提交信息使用 `[T4017f] ...` 形式。
6. 提交后停止，不继续处理后续任务。

### 约束
- 不回退用户已有修改。
- 若收尾过程中发现新的既有问题，先修复该问题或按要求把前置任务插入 `TODO.md`，然后再决定是否提交停止。

### 已完成的关键步骤
- 已检查 `git log -1 --stat --oneline`，最新提交 `d240395a` 仅更新计划文档，未声明需要先修的既有缺陷。
- 已检查工作树与 `TODO.md` / `PLAN.md` 当前内容，确认本轮剩余工作只差任务状态落档与提交。
- 已更新 `TODO.md`：
  - 将 `T4017f` 标记为 `[DONE]`。
  - 补充 explicit outcome boundary 收口、hidden-suspend 修复、LLVM/run-pass 覆盖与验证命令记录。
  - 将顶部“当前剩余实现顺序”切换为从 `T4017R` 开始。
- 已更新 `PLAN.md`：
  - 顺序总览改为 `T4017f` 已完成、当前主线转入 `T4017R`。
  - 在 P1.6 中补写 `T4017f` 的完成记录、测试覆盖与验证结果。
  - 将阶段性“当前状态”改为 `T4017R -> T4012b3 -> ...`。
- 已复查本轮 diff，确认实现文件、fixture、新增 native test helper 与计划文档变更都对应 `T4017f` 收口范围。
- 已重新执行并确认通过：
  - `cargo fmt --all`
  - `cargo test -p scoopc --features llvm explicit_outcome_boundary -- --nocapture`
  - `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass`（`fixtures: ok (397)`）
  - `cargo run -p scoop -- test`（`fixtures: ok (1174)`）
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`

### 剩余动作
1. 提交 git commit：`[T4017f] Migrate remaining effect boundaries to explicit outcome`。
2. 提交后停止。
