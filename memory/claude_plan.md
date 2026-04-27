# 执行计划

## 约束

- 本轮只处理 `TODO.md` 中第一个未完成任务，完成后停止。
- 在推进计划任务前，先检查最新提交是否提到既有问题；如有，先修复该问题。
- 任何在检查、实现或测试中发现的既有 bug、回归、规格不匹配、未完成边界或 workaround 都立即纳入当前范围。
- 不采用 fixture-only hack、规避实现、弱化测试或偏离规格的做法。
- 完成实现后更新 `TODO.md` 和 `PLAN.md`，运行相关测试，并提交 Git commit。

## 初始步骤

1. 检查最新 Git commit 的标题和正文，判断是否提到需要优先修复的既有问题。
2. 阅读 `TODO.md`，定位第一个未完成任务。
3. 阅读相关 `PLAN.md` 内容，确认任务背景、依赖和预期实现范围。
4. 如果第一个未完成任务过大，先将其拆分为更小任务，更新 `TODO.md` 和 `PLAN.md`，提交拆分结果并停止。

## 执行步骤

1. 定位任务涉及的代码、测试、fixtures 或文档。
2. 实现最小但完整的规格正确修改。
3. 添加或更新针对该行为的测试，优先使用仓库已有测试模式。
4. 运行相关测试；如果修改影响范围较大，再运行更广的验证命令。
5. 修复测试或编译中暴露出的真实问题；如果发现阻塞性的缺失能力，按要求把前置任务写入 `TODO.md` 并停止。
6. 更新 `TODO.md` 标记本任务完成，并更新 `PLAN.md` 记录当前状态。
7. 检查工作区差异，确认没有无关回退或意外改动。
8. 提交 Git commit，提交信息包含任务编号或清晰描述。

## 当前状态

- 已接手上一轮挂起后的工作区。当前存在未提交改动：
  - `PLAN.md` / `TODO.md`
  - `crates/scoopc/src/llvm/codegen/mod.rs`
  - `crates/scoopc/src/llvm/codegen/mir_body.rs`
  - `crates/scoopc/src/llvm/emit.rs`
  - `crates/scoopc/src/llvm/tests.rs`
  - `crates/scoopc/src/mir/pass_view.rs`
  - `memory/claude_plan.md`
- 最新提交为 `[T5000h0e1] Consume pass view in production codegen queries`；目前未从提交标题或统计信息看到必须先修复的额外既有问题，但还需要结合 diff 和测试继续确认。
- `TODO.md` 中 `T5000h0e2` 已被上一轮改动标为 `[DONE]`，且已有完成记录；本轮目标是按 `PROMPT.md` 收尾该任务，而不是继续推进后续 `T5000h0eR`。
- 已发现 `T5000h0e2` 当前完成记录只列出较窄测试。收尾步骤需要：
  1. 复核当前 diff，确认实现确实是 pass-rewritten MIR callable body 的 production LLVM lowering，不是 HIR workaround。
  2. 运行 `cargo fmt --all`、任务相关测试、`cargo test --all`、`cargo run -p scoop -- test`、`cargo clippy --all-targets -- -D warnings`。
  3. 如测试或复核暴露真实缺陷，先修复缺陷并同步任务记录。
  4. 更新 `PLAN.md` / `TODO.md` 的完成记录，补齐最终验证命令。
  5. 检查最终 diff 后提交 `[T5000h0e2] Lower pass-rewritten MIR bodies in production LLVM`，然后停止。
- 已完成收尾复核与验证：
  - 当前 diff 方向确认：显式 pass body override 走 `codegen_top_level_mir_fun(...)`，未改写 raw body 继续走 HIR 兼容路径，unsupported MIR 节点返回结构化错误；
  - `cargo fmt --all`：通过；
  - `cargo test -p scoopc production_codegen_lowers_overridden_pass_mir_body -- --nocapture`：通过；
  - `cargo test -p scoopc llvm::tests -- --nocapture`：通过；
  - `cargo test -p scoopc --no-default-features`：通过；
  - `cargo test --all`：通过；
  - `cargo run -p scoop -- test`：通过，`fixtures: ok (1201)`；
  - `cargo clippy --all-targets -- -D warnings`：通过。
- 已更新 `PLAN.md` / `TODO.md` 的 `T5000h0e2` 完成记录，下一步检查最终 diff 并提交。
