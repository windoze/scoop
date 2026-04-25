## 执行摘要

用户要求先处理最新提交中提到的遗留问题，再执行 `TODO.md` 中第一个未完成任务，并且整个过程中持续更新本文件。

出于安全约束，这里不记录逐字内部思考；改为记录可审阅的高层判断、执行步骤、发现的问题、决策依据与进度。

## 初始计划

1. 检查最新一次 Git 提交，确认是否明确提到已有问题、回归、规避方案或未完成边界。
2. 阅读 `TODO.md`，定位第一个未完成任务。
3. 如该任务过大，先把它拆分为更小的可执行子任务，并同步更新 `PLAN.md` 与 `TODO.md`。
4. 对当前要执行的首个任务进行实现。
5. 运行与该任务直接相关的测试；如果发现任何既有缺陷、回归或规范不匹配，优先修复，或将其作为前置任务插入 `TODO.md` 并停止。
6. 在完成后更新 `TODO.md`、`PLAN.md` 与本文件，记录结果和后续状态。
7. 提交本次变更，提交信息应清晰描述完成的任务。

## 进度记录

- 已创建计划文件，等待检查最新提交与任务列表。
- 已检查最新提交 `e991a8c653f81bb5cc1e89739a0a7bae4b850b2a`（`[T5000cR] Review shared facts backend boundary`）。
  - 提交中提到的既有问题是一个文档错配，且已在该提交内修复；
  - 未发现“提交明确提到但尚未修复”的前置缺陷，因此可继续读取 `TODO.md`。
- 已确认 `TODO.md` 中首个未完成任务是 `T5000d 扩展现有 MIR，形成最小 generic early MIR / ANF template`。
- 已判断 `T5000d` 单轮过大，需要先拆分。
  - 依据：
    - 当前 `crates/scoopc/src/mir/mod.rs` 只有 CFG / locals / 最小 `Perform` / `Handle` 占位；
    - `crates/scoopc/src/mir/lower.rs` 对普通 `Call` 仍统一产出 `Todo("call lowering pending")`；
    - `T5000d` 同时要求显式 `DirectCall / VirtualCall / InterfaceCall / ClosureCall / FunValueCall`，以及显式 `Perform` / `Resume` 与更稳定的 provenance / dispatch metadata，单次完成风险过高。

## 拆分判断

计划把 `T5000d` 拆成按语义边界递进的子任务，并保持每个子任务后跟一个 review：

1. 先落地普通调用主线的最小 ANF 形状：
   - 显式 `DirectCall / ClosureCall / FunValueCall`；
   - 打通 callable value provenance 的最小基线；
   - 让 MIR 不再把这三类调用统一写成 `Todo(...)`。
2. 再处理分派与 control-transfer 特有调用：
   - `VirtualCall / InterfaceCall`；
   - `Continuation.resume` 等显式 `Resume` 语义；
   - 必要的 receiver / dispatch metadata。
3. 最后收口 `Perform` / `Resume` 与更稳定的 control-flow / provenance 入口，
   为后续 `when` / pattern lowering、operator-overload target materialization 等提供正规化承载点。

## 当前执行项

准备执行拆分后的第一个子任务：

- 目标：
  - 为 MIR 引入显式普通调用节点；
  - 实现 `DirectCall / ClosureCall / FunValueCall` 的 lowering；
  - 保持 MIR 仍然 backend-agnostic，不混入 LLVM 细节。
- 预期改动：
  - `crates/scoopc/src/mir/mod.rs`
  - `crates/scoopc/src/mir/lower.rs`
  - `TODO.md`
  - `PLAN.md`
  - `tests/fixtures/mir/*`（新增或更新与调用形态相关的 fixture）

## 本轮结果

- `T5000d1` 已完成，并已把 `TODO.md` / `PLAN.md` 标记到位。
- 已完成的代码改动：
  - 在 `crates/scoopc/src/mir/mod.rs` 中新增 MIR 普通调用节点：`CallArg`、`CallKind::{Direct, Closure, FunValue}`、`Rvalue::Call`；
  - 在 `crates/scoopc/src/mir/lower.rs` 中实现 `DirectCall / ClosureCall / FunValueCall` lowering，并加入最小 callable provenance 跟踪；
  - 在 `crates/scoopc/src/hir/lower/expr.rs` 中修复 dump 路径的既有阻塞点：调用实参 expected-type 旧早退会丢掉 `value_ty`，导致顶层函数值作为实参时不能合成为 closure；现已补上一般 `value_ty` 透传与 top-level function value fallback。
- 已新增/更新回归：
  - 新增 `tests/fixtures/mir/direct_and_fun_value_call.{scoop,mir}`；
  - 更新 `tests/fixtures/mir/closure_non_capture.mir`、`tests/fixtures/mir/closure_capture_val.mir`、`tests/fixtures/mir/closure_capture_var.mir`。
- 验证已完成：
  - `cargo fmt --all`
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
- 下一条待执行任务应为：
  - `T5000d1R Review：确认普通调用主线已从 HIR 语法形状收口为显式 MIR call kind`
