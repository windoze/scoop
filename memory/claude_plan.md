# 执行计划

## 说明

按要求，我会先把可执行计划写入此文件，再开始读取仓库状态、最新提交和任务列表。这里记录的是可审阅的行动计划、判断依据和进度，不直接暴露内部推理细节。

## 初始步骤

1. 检查最新一次 Git 提交，确认提交信息里是否提到已知遗留问题。
2. 如果最新提交提到需要先修复的遗留问题，先定位、修复、补测，并在继续前更新本文件。
3. 阅读 `TODO.md`，识别第一个未完成任务。
4. 阅读 `PLAN.md`，确认该任务现有计划、依赖和上下文。
5. 判断该任务是否足够小、能在一次迭代内完整完成。

## 任务决策规则

1. 如果第一个未完成任务可以直接完成：
   - 实现代码。
   - 补充或调整测试。
   - 运行相关验证，包括必要时的 `cargo test`、`cargo clippy --all-targets -- -D warnings`、以及任务相关命令。
   - 更新 `TODO.md` 和 `PLAN.md`。
   - 提交一个清晰的 Git commit，然后停止。
2. 如果第一个未完成任务过大或存在明确前置依赖：
   - 将其拆分为更小的子任务。
   - 更新 `PLAN.md` 说明拆分后的执行顺序。
   - 更新 `TODO.md`，把新的首个可执行子任务放到正确位置。
   - 本轮只执行拆分后的第一个子任务；如果只是做了重排或依赖修正，则提交并停止。
3. 如果实现过程中发现与规范不符的缺口、缺失特性或已有 bug：
   - 不绕过问题。
   - 在 `TODO.md` 中新增前置修复任务并调整顺序。
   - 在 `PLAN.md` 中记录阻塞原因。
   - 仅在依赖问题解决后才继续原任务；若本轮只能完成依赖重排，则提交并停止。

## 执行时的检查点

1. 在开始修改代码前，先确认将要编辑的模块和影响范围。
2. 每完成一个关键步骤后更新本文件，记录：
   - 已完成的检查或实现内容。
   - 新发现的风险、依赖或阻塞。
   - 接下来的具体动作。
3. 在提交前再次确认：
   - 没有把未完成事项错误标记为已完成。
   - 测试和 lint 结果与任务范围匹配。
   - `TODO.md` / `PLAN.md` / 本文件内容一致。

## 当前状态

- 最新提交：`c9552be [T2003r3d2c1] Reconnect stack-reentry-only multi-resuming leaf`。
- 检查结果：提交信息本身没有额外点名“必须先补的遗留问题”，因此继续按 `TODO.md` 主线执行。
- 已确认第一个未完成任务：`T2003r3d2c2`。

## 当前任务判定

- 任务：接回 unified multi-resuming leaf 的 `heap-continuation-only` 基线。
- 依赖：`T2003r3d2c1`，已完成。
- 复杂度判断：可在本轮直接完成，不需要继续拆分 `TODO.md` / `PLAN.md`。
- 当前实现现状：
  - `nonresuming.rs` 在 `MultiResuming` 分支里已经接回 `stack-reentry-only`，但对 `heap-continuation-only` 仍返回显式 pending 诊断。
  - `multi_resuming.rs` 目前只有 unified `stack-reentry-only` leaf。
  - `single_escape.rs` 与 `shared.rs` 中已经存在可复用的 plan-driven escape-site 解析、capture 恢复、payload decode 等 helper。

## 本轮执行计划

1. 在 `multi_resuming.rs` 中新增 unified `heap-continuation-only` multi-resuming leaf。
2. 复用当前 unified plan metadata / escape-site resolver / sibling non-resuming dispatch helper，不恢复任何已删除的 shape-based route 名称。
3. 如有必要，在 `shared.rs` 中补回一个通用的 heap state capture helper，供新的 unified leaf 使用。
4. 在 `nonresuming.rs` 中把 `counts.stack_reenter == 0 && counts.heap_continuation >= 2` 的分支改为调用新的 unified leaf，而不是返回 pending 诊断。
5. 更新 LLVM 定向测试：
   - 把当前“heap-continuation-only route pending”测试改成成功样例。
   - 视实现范围补一个 sibling non-resuming 或 `finally` 的正向样例。
6. 新增或更新一个 representative run-pass fixture，覆盖多个 escape-continuation arms，并至少带一例 sibling non-resuming 或 `finally`。
7. 运行定向验证：
   - `cargo fmt --all`
   - `cargo test -p scoopc unified_multi_resuming_codegen_ -- --nocapture`
   - `cargo run -p scoop --features llvm -- run <代表性 fixture>`
   - `cargo clippy --workspace --all-targets -- -D warnings`
8. 若验证通过，更新 `TODO.md` / `PLAN.md` / 本文件，提交本轮改动并停止。

## 进度更新

- 已完成：
  - `shared.rs` 已补回 `capture_escape_state_with_pc(...)`，供 heap continuation state 在下一次 suspension 前统一写回 captures + `pc`。
  - `nonresuming.rs` 的 `MultiResuming` 分支已把 `counts.stack_reenter == 0 && counts.heap_continuation >= 2` 接到新的 unified leaf。
  - 已新增模块文件 `crates/scoopc/src/llvm/codegen/effect/multi_resuming_heap.rs`，把历史上已验证过的 pure heap-continuation multi-resuming 逻辑迁到当前 unified helper/命名契约下。
  - `effect/mod.rs` 已将新模块接入编译单元。
- 当前阶段：
  - 实现与验证已完成，正在同步任务状态并准备提交。
- 下一步：
  1. 复核工作区差异，确认只完成了 `T2003r3d2c2`。
  2. 提交本轮改动并停止。

## 本轮结果

- 已完成任务：`T2003r3d2c2`。
- 关键实现：
  - 新增 `crates/scoopc/src/llvm/codegen/effect/multi_resuming_heap.rs`，接回 unified `heap-continuation-only` multi-resuming leaf。
  - `nonresuming.rs` 的 `MultiResuming` 入口现已把 `counts.stack_reenter == 0 && counts.heap_continuation >= 2` 路由到新的 unified leaf。
  - `shared.rs` 已补回 `capture_escape_state_with_pc(...)`，供 heap continuation state 在多次 suspension 间统一写回 captures 与 `pc`。
  - 已新增 LLVM 定向单测与 representative fixture，覆盖多个 escape-continuation arms，以及 sibling non-resuming tail。
- 已完成验证：
  - `cargo fmt --all`
  - `cargo test -p scoopc unified_multi_resuming_codegen_ -- --nocapture`
  - `cargo test -p scoopc llvm::codegen::effect::tests:: -- --nocapture`
  - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_handle_escape_arms_with_abort_tail.scoop`
  - `cargo clippy --workspace --all-targets -- -D warnings`
