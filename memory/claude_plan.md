# Claude Plan

## 约束说明
- 不写入不可公开的内部推理；此文件记录可公开的执行计划、发现、决策与进度。
- 本轮目标是：先检查最新提交是否提到已有问题并优先修复；然后执行 `TODO.md` 中第一个未完成任务；完成后测试、更新文档、提交并停止。

## 初始执行计划
1. 检查最新一次 git 提交信息，确认是否提到需要先修复的既有问题。
2. 阅读 `TODO.md` 与 `PLAN.md`，确定第一个未完成任务，并核对当前计划。
3. 如果该任务过大，则把它拆分为更小的子任务，更新 `PLAN.md` 和 `TODO.md`，并只执行拆分后的第一个子任务。
4. 在实现前先阅读相关代码，确认现状、边界和可能阻塞任务的既有问题。
5. 实现当前目标任务；若途中发现既有 bug、回归、规格不匹配或实现边界缺口，优先修复，或将其作为前置任务写回 `TODO.md`/`PLAN.md` 后停止。
6. 运行与改动直接相关的测试；必要时补充测试并修复失败。
7. 更新 `TODO.md` 与 `PLAN.md`，记录完成情况或阻塞关系。
8. 按仓库提交风格创建一次 git commit，然后停止，不继续下一个任务。

## 进度
- 已创建初始计划，等待仓库检查结果。
- 已检查最新提交 `533ea2b`：提交主题是修复 review 暴露的 closure MIR source-context 缺口，提交说明里没有新的未修复事项。
- 已定位 `TODO.md` 中首个未完成任务为 `T5000j3b2 扩展 CaptureBox* / 剩余 opaque higher-order 调用到 production MIR 主线`。
- 已确认 `T5000j3b2` 无需继续拆分：现有代码已经具备 closure object / indirect-call / pass-artifact 基础设施，缺的是 production MIR bridge 对 `CaptureBox*` 与 `CallKind::FunValue` 的接入。
- 已完成实现：
  1. 为 `llvm/codegen/mir_body.rs` 补齐 `CaptureBoxNew/Get/Set` 的 supported 判定与 LLVM lowering。
  2. 为 production MIR bridge 接入 `CallKind::FunValue` 的 supported 判定与 indirect-call lowering。
  3. 为 raw candidate / reachability 同步放宽 `CaptureBox*` 与 opaque `FunValueCall` 的 production 边界。
  4. 在实现过程中发现并修复了一个既有缺口：materialized MIR `FunctionType` 曾直接携带另一套 `TypeStore` 的 `TypeId` 进入 codegen，导致 sysroot/task helper 的 production MIR `FunValueCall` 触发 `value coercion`；现已在 `mir_body.rs` 中新增 function type / effect-row 的等价 type-store 映射。
  5. 已新增 LLVM 回归测试：mutable-capture closure raw MIR body、opaque fun-value raw MIR body。
  6. 已完成验证：`cargo test --all`、`cargo clippy --all-targets -- -D warnings`、`cargo run -p scoop -- test` 全部通过。

## 当前任务判断
- `T5000j3` 已经在 `TODO.md` 内拆成 `j3a`、`j3b1`、`j3b2` 与 review 条目；当前先检查 `j3b2` 是否还需要继续细分。
- 预期检查点：
  1. `llvm/codegen/mir_body.rs` 当前对 `CaptureBoxNew/Get/Set`、`CallKind::FunValue` 的 supported/fallback 边界。
  2. `llvm/reachability.rs` 当前对这些 MIR 形状的扫描与 fallback 规则。
  3. `mir`/summary/escape facts 中是否已有可直接复用的 higher-order 事实，还是存在新的既有缺口需要先修复或前插任务。
  4. 现有 LLVM / fixture 测试是否已经覆盖 mutable capture closure 与 opaque fun-value 相关回归。

## 下一步执行计划
1. 阅读与 `T5000j3b2` 直接相关的 MIR bridge、reachability、summary/escape-facts 与测试代码。
2. 判断当前任务是否可在本轮直接完成；若发现它仍过大或被既有缺口阻塞，则按要求更新 `TODO.md` / `PLAN.md` 并停止。
3. 若可直接完成，则最小化实现缺失的 production MIR lowering / reachability 支持，并补充针对 mutable capture 与剩余 higher-order 形状的回归测试。
4. 运行相关测试，再跑 `cargo test --all` 与 `cargo clippy --all-targets -- -D warnings` 进行验证。
5. 更新 `TODO.md` / `PLAN.md` / `memory/claude_plan.md`，提交一次 git commit，然后停止。

## 收尾
- 已更新 `TODO.md` 与 `PLAN.md`：`T5000j3b2` 标记为完成，下一条待执行任务切换为 `T5000j3b2R`。
- 剩余动作：检查 git 状态与差异，按仓库风格创建本轮提交，然后停止。

## 追加进度（2026-04-29）
- 已检查最新提交：`[T5000j3b2] Lower capture-box and fun-value MIR in production path`；提交信息本身没有新增待先修的遗留问题。
- 已完成 `T5000j3b2R` 复核，未发现需要前插到 `T5000j3bR` 之前的新缺陷任务。
- 本轮 review 结论：
  1. `CaptureBox*` lowering 只消费 materialized MIR 结构、pass view 与共享类型/运行时布局事实。
  2. `FunValueCall` 仍作为 opaque indirect call 进入 production MIR bridge，backend 没有重新承担 higher-order target-set 收缩。
  3. effect/suspendability 相关判断继续通过 shared facts 与 pass summary/escape facts 进入 backend bridge，而不是在 LLVM lowering 现场重算。
- 已补跑验证：
  1. `cargo test -p scoopc production_codegen_lowers_raw_mir_mutable_capture_closure_body -- --nocapture`
  2. `cargo test -p scoopc production_codegen_lowers_raw_mir_fun_value_call_body -- --nocapture`
  3. `cargo test --all`
  4. `cargo clippy --all-targets -- -D warnings`
- 下一条待执行任务已更新为 `T5000j3bR`；本轮到此停止。
