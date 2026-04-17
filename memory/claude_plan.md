# 当前执行计划

注意：按安全要求，这里记录的是精简后的执行摘要与步骤，不包含逐字内部推理。

## 目标

本轮只完成 `TODO.md` 中第一个未完成任务；如遇到前置缺陷或规格缺口，先把缺陷转化为更靠前的任务并处理其首个必要步骤，然后停止。

## 本轮目标确认

- 已定位首个未完成任务为 `T3009b0`：为 escaped continuation 的 `Continuation.resume(...)` 接回 scalar/ref payload 专用 lowering。
- 审查后确认当前分支里相关代码已存在于生产路径：
  - 普通 call 路径在 `crates/scoopc/src/llvm/codegen/mod.rs` 中按 `continuation_resume_call_sites` 分派到 dedicated builtin lowering。
  - dedicated lowering 位于 `crates/scoopc/src/llvm/codegen/effect/mod.rs`，复用 continuation runtime ABI 与 `resume_word` / `resume_gc_ref` transport。
- 本轮重点改为正式验收该实现，并同步 `TODO.md` / `PLAN.md` 状态，而不是再追加生产代码补丁。

## 初始步骤

1. 检查最新一次提交，确认是否提到了需要先修复的既有问题。
2. 读取 `TODO.md`、`PLAN.md`，定位第一个未完成任务，并核对依赖关系与现有计划。
3. 检查工作区状态，避免覆盖用户未提交修改。

## 执行策略

1. 若最新提交暴露出既有问题，先评估其是否必须在当前任务前修复。
2. 若第一个未完成任务过大，先在 `PLAN.md` 和 `TODO.md` 中拆分为可执行子任务，本轮只执行拆分后的第一个子任务。
3. 在实现前先阅读相关代码与测试，确认规格边界，不使用规避性方案。
4. 完成实现后运行相关测试，并补充必要测试；再跑格式化、lint 与能覆盖该改动的命令。
5. 更新 `TODO.md`、`PLAN.md` 与本文件，记录完成状态或阻塞原因。
6. 使用清晰的提交信息提交当前这一轮的所有改动，然后停止。

## 本轮已完成的关键步骤

1. 已检查最新提交；提交信息未额外声明需要先处理的新既有缺陷。
2. 已确认首个未完成任务为 `T3009b0`，且任务边界已足够细，不需要继续拆分。
3. 已定向验证任务验收矩阵：
   - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_escape_continuation_resume_unit.scoop`
   - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_escape_continuation_resume_bool.scoop`
   - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_escape_continuation_resume_string.scoop`
   - `cargo run -p scoop --features llvm -- run tests/fixtures/run-pass/effect_resume_nested_escape_handle_tail.scoop`
   - 以上均通过，输出与任务目标一致，未再出现 `call callee` 回退。
4. 已完成质量门验证：
   - `cargo test --all`
   - `cargo clippy --all-targets -- -D warnings`
   - 两者均通过。
5. 下一步：同步 `TODO.md` / `PLAN.md` / 本文件并创建本轮提交。

## 完成判定

- 任务代码实现完整。
- 相关测试通过；若全量检查可承受，也应一并验证。
- `TODO.md`/`PLAN.md`/`memory/claude_plan.md` 已同步。
- 已创建 git 提交。
