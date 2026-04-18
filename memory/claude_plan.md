# 本轮执行计划（2026-04-19）

## 任务目标

- 按 `TODO.md` 当前顺序完成第一个未完成任务。
- 结合上轮已完成实现的交接信息，当前目标任务为 `T4003b`：支持顶层泛型函数值与 `callee<T>` 作为一等值传递。
- 完成后立即停止，不继续处理下一个任务。

## 已知上下文

- 已检查最新提交 `cb6a2a9 [T4003a] 打通 FunPtr receiver 调用语义`，提交说明未暴露需要先修复的遗留问题。
- 上轮实现已完成以下代码改动，但尚未完成本轮收尾：
  - AST / parser / typecheck / HIR 已贯通“顶层函数值”表示与 lowering。
  - parser 已允许 `callee<T>` 作为值表达式，而不强制后接调用。
  - HIR lowering 采用“零捕获 closure 包装”复用现有 function-value 调用主线。
  - 已新增对应 run-pass 与 typecheck fixtures。
- 已完成的验证：
  - `cargo check -p scoopc`
  - 定向构建运行正例
  - 定向 fixture 测试
  - 全量 `tests/fixtures/typecheck`
  - `cargo test --all`
- 尚未完成：
  - `cargo clippy --all-targets -- -D warnings`
  - 根据最终结果更新 `TODO.md` / `PLAN.md`
  - 记录本文件的完成状态
  - 提交 git commit

## 执行步骤

1. [已完成] 检查工作树状态，确认上轮改动仍在且未混入不应提交的产物。
2. [已完成] 复核 `TODO.md` 与 `PLAN.md` 当前内容，确认 `T4003b` 仍是首个未完成任务，且无需进一步拆分。
3. [已完成] 运行 `cargo clippy --all-targets -- -D warnings`，结果通过，无新增 warning。
4. [未触发] 如果 clippy 报错：
   - 修复所有 warning/error。
   - 重新运行相关测试，至少覆盖受影响模块与必要全量命令。
   - 更新本文件记录修复点。
5. [已完成] clippy 通过后已更新 `TODO.md`，将 `T4003b` 标记为完成，并补充实现摘要与验证命令。
6. [已完成] 已更新 `PLAN.md`，记录 `T4003b` 完成并把下一步推进到 `T4003c`。
7. [已完成] 已更新本文件，记录当前进度。
8. [已完成] 已检查 `git status`，当前仅包含本轮源代码、fixture 与计划文件改动，未混入 `target/t4003b-fixtures/**` 或临时二进制。
9. [待执行] 使用明确的提交信息完成提交，例如：
   - `[T4003b] 支持顶层泛型函数值与 callee<T> 一等值传递`
10. [待执行] 提交后停止。

## 本轮已完成的关键结果

- `cargo clippy --all-targets -- -D warnings` 已通过。
- `cargo fmt --all` 已执行，随后 `cargo fmt --all --check` 已通过。
- `TODO.md` 已将 `T4003b` 标记为完成，记录了以下实现结论：
  - bare 顶层函数值与 `callee<T>` 已建立 typecheck side table，并贯通到 AST / parser / typecheck / HIR。
  - generic function value 现可从 expected function type 反推 type args。
  - higher-order 调用预收集阶段会延迟 bare 顶层泛型函数值的报错时机，等待 expected-context 生效。
  - HIR lowering 统一把顶层函数值转成零捕获 closure 包装，复用既有 function-value call / codegen 主线。
- `PLAN.md` 已记录 `T4003b` 完成，并把下一项推进到 `T4003c`。

## 剩余收尾

- 提交 commit 后停止，不继续处理 `T4003c`。

## 风险与约束

- 不得以 workaround 方式绕过规格缺口；如果发现新的规格不匹配，必须先把缺口写入 `TODO.md` / `PLAN.md`，调整依赖顺序，然后提交并停止。
- 不得回退用户已有改动。
- 所有文件编辑继续使用 `apply_patch`。
- 必须保证最终 `cargo clippy --all-targets -- -D warnings` 无告警。
