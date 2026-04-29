# 执行计划

## 约束与原则

- 本文件记录可审阅的执行计划、当前发现和进度更新。
- 为保护推理安全，这里不写逐字内部思考过程，只写结论、步骤和关键依据。
- 本轮只处理 `TODO.md` 中第一个未完成任务；若发现前置缺陷，会先修复缺陷或把前置任务插入 `TODO.md` 后停止。

## 初始步骤

1. 检查最新一次 Git 提交，确认是否提到任何已知问题；若提到，优先修复。
2. 阅读 `TODO.md`，定位第一个未完成任务。
3. 阅读 `PLAN.md`，理解当前计划与该任务的上下文。
4. 评估任务复杂度：
   - 若可在本轮完整落地，则直接实现。
   - 若过大，则先拆分任务，更新 `PLAN.md` 与 `TODO.md`，执行拆分后的第一个子任务。
5. 在实现过程中，如发现任何既有 bug、回归、规范不匹配、实现边界缺口或依赖缺失：
   - 立即视为当前范围内问题；
   - 先修复，或在 `TODO.md` 中插入前置任务并调整顺序；
   - 更新本文件与 `PLAN.md`，然后按要求决定继续或停止。

## 执行与验证

1. 实现首个未完成任务或其首个子任务。
2. 运行相关测试，至少覆盖：
   - 受影响模块的定向测试；
   - 必要的回归测试；
   - `cargo fmt --check`；
   - `cargo clippy --all-targets -- -D warnings`；
   - 若改动影响范围较大，再补充更广的 `cargo test --all` 或对应子集。
3. 若测试失败，先修复失败与相关根因，再重新验证。

## 收尾

1. 更新 `TODO.md`，将本轮完成的任务标记为完成。
2. 更新 `PLAN.md`，反映当前状态、拆分结果或依赖调整。
3. 同步更新本文件，记录关键进展与最终结论。
4. 提交 Git commit，提交信息与任务编号/内容对应。
5. 停止，不继续下一个任务。

## 进度记录

- 2026-04-29：已创建本文件并写入初始执行计划；下一步检查最新提交与任务清单。
- 2026-04-29：已检查最新提交 `3ea47570b674d474f83f29de3b7dace36644730a`，提交信息仅为 `Update plan`，未显式提到需优先修复的既有 issue。
- 2026-04-29：已定位首个未完成任务为 `T5000j3b 扩展更多 higher-order / closure 场景到 production MIR 主线`。
- 2026-04-29：已完成现状探测，关键结论如下：
  - `crates/scoopc/src/llvm/codegen/mir_body.rs` 当前仍把 `MakeTuple`、`TupleGet`、`MakeClosure`、`CaptureBox*`、`CallKind::Closure`、`CallKind::FunValue` 视为 unsupported；
  - `crates/scoopc/src/llvm/reachability.rs` 已能扫描这些 MIR 节点，但 `mir_fun_requires_hir_compat_scan(...)` 仍会把它们整体压回 HIR-compatible boundary；
  - `mir::inline` 已会生成 pass-visible `ClosureCall` 形状，说明 production MIR 主线确实缺少 higher-order / closure 覆盖；
  - mutable capture 目前在 HIR closure lowering 侧仍明确报 `mutable capture (not supported yet)`，与 non-capturing / immutable-capturing closure 属于不同复杂度边界。
- 2026-04-29：据此判定 `T5000j3b` 单轮过大，准备拆分为更细子任务后执行第一个子任务。
- 2026-04-29：已确认工作区中的未提交代码已经实现 `T5000j3b1` 主体：
  - `crates/scoopc/src/llvm/codegen/mir_body.rs` 已补齐 `MakeTuple` / `TupleGet` / `MakeClosure` / `CallKind::Closure` 的 production MIR bridge；
  - `crates/scoopc/src/llvm/reachability.rs` 已允许结构已知 closure/env 形状直接走 production MIR 主线，并继续把 `CaptureBox*` / opaque `FunValueCall` / implicit tail-return 等保留在 fallback；
  - `crates/scoopc/src/llvm/tests.rs` 与 `crates/scoopc/src/mir/inline.rs` 已补齐对应回归。
- 2026-04-29：已完成定向与全量验证：
  - `cargo test -p scoopc production_codegen_lowers_raw_mir_non_capturing_closure_body -- --nocapture`
  - `cargo test -p scoopc production_codegen_lowers_raw_mir_immutable_capture_closure_body -- --nocapture`
  - `cargo test -p scoopc production_codegen_lowers_pass_visible_known_closure_call_body -- --nocapture`
  - `cargo fmt --all --check`
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
  - `cargo run -p scoop -- test`
  - 结果：全部通过，未发现需要前插到 `T5000j3b1R` 之前的新阻塞缺陷。
- 2026-04-29：下一步只剩收尾：更新 `TODO.md` / `PLAN.md` 的完成记录，提交 `[T5000j3b1]` 对应 commit，然后停止。

## 当前拆分草案

1. `T5000j3b1`：先接入 non-capturing / immutable-capturing closure 的 production MIR lowering。
   - 目标：
     - 支持 `MakeTuple` / `TupleGet` / `MakeClosure` / `CallKind::Closure` 的 MIR bridge lowering；
     - 放宽 raw candidate / reachability 边界，使非捕获或只捕获不可变值的 closure body 可直接走 production MIR 主线；
     - 保持 `CaptureBox*`、opaque `FunValueCall`、隐式 tail-return 等仍在 fallback 边界。
2. 后续子任务：再处理 `CaptureBox*` / mutable-capture 与剩余 opaque higher-order fun-value 场景。

## 下一步

1. 检查文档更新后的 diff，确保任务状态与验证记录准确。
2. 创建 `[T5000j3b1]` commit。
3. 停止，等待下一轮执行 `T5000j3b1R`。
