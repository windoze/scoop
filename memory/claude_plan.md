# 执行计划

## 约束说明

- 本文件记录可审阅的执行计划、关键判断依据摘要、执行进展与必要调整。
- 不记录逐字内部思维，但会完整记录可复现的操作步骤、发现的问题、决策原因与后续动作。

## 初始步骤

1. 检查最新一次 Git 提交信息，确认是否提到了需要先修复的既有问题。
2. 阅读 `TODO.md`，定位第一个未完成任务。
3. 阅读 `PLAN.md`，核对当前计划与 `TODO.md` 是否一致。
4. 如果首个未完成任务过大，先把它拆分为更小的子任务，并同步更新 `PLAN.md` 与 `TODO.md`；本次只执行拆分后的第一个子任务。
5. 阅读与该任务直接相关的代码、测试和文档，确认实现边界与依赖。

## 执行步骤

1. 实现首个未完成任务（或拆分后的首个子任务）。
2. 运行与改动直接相关的测试；若范围需要，补充运行：
   - `cargo test --all`
   - `cargo clippy --all-targets -- -D warnings`
   - 以及任务相关的专项命令
3. 如测试失败，先修复失败项，再重新验证。
4. 更新文档与计划：
   - 在 `TODO.md` 中将当前任务标记为完成
   - 在 `PLAN.md` 中更新当前状态与后续安排
   - 在本文件中补充实际进展与任何计划变化
5. 提交 Git commit，提交信息使用任务标签或清晰描述。
6. 完成一个任务后立即停止，不继续下一个任务。

## 风险与决策原则

- 如果最新提交提到待修复问题，则这些问题优先于 `TODO.md` 任务处理。
- 如果任务依赖缺失的语言特性或基础设施，不强行实现；改为调整 `TODO.md` / `PLAN.md` 反映依赖关系，并提交后停止。
- 不回滚用户已有改动；若遇到与当前任务冲突的现有未提交修改，先评估影响，再决定如何兼容。

## 进展记录

- 已创建初始计划文件，下一步开始检查最新提交与任务列表。
- 已检查最新提交 `07cc7aa`，提交标题为 `[T2003c0b2b0] Reorder mixed escape continuation tasks`，提交正文没有额外的“需先修复的既有问题”说明，因此无需在任务前插入提交级 hotfix。
- 已读取 `TODO.md` / `PLAN.md`，确认首个未完成任务为 `T2003c0b2b0`：补 LLVM immediate-resume tail 中嵌套 `handle` 结果表达式。
- 已定位相关代码：
  - `crates/scoopc/src/llvm/codegen/effect.rs`
  - `crates/scoopc/src/llvm/codegen/mod.rs`
- 当前技术判断摘要：
  - `codegen_handle_expr(...)` 需要显式 `expected` 类型上下文；
  - `codegen_immediate_resume_top_level_tail_and_finalize(...)` 在处理 tail 的最后一个表达式时使用的是 `codegen_expr(expr)`，没有向下传递 `out_ty`；
  - 因此若 tail 最后一个表达式本身是嵌套 `handle`，其 codegen 很可能丢失预期结果类型，并在后续 `coerce_value(...)` 时触发 `value coercion`。
- 下一步具体动作：
  1. 构造并复现最小“outer immediate-resume + inner escape handle tail”样例。
  2. 修改 immediate-resume tail lowering，把最后一个表达式改为在 `Some(out_ty)` 期望类型下 codegen；非最后表达式统一走 `Some(Unit)`。
  3. 视需要补齐 statement-unit helper 对 expression statement 的 expected type 传递，避免同类 nested handle 问题残留。
  4. 新增 run-pass 回归 fixture。
  5. 运行定向测试，再跑任务要求的完整验证命令。
- 复现结果：
  - 已用 `/tmp/t2003c0b2b0_repro.scoop` 成功复现旧问题。
  - 复现命令：`cargo run -p scoop --features llvm -- build /tmp/t2003c0b2b0_repro.scoop -o /tmp/t2003c0b2b0_repro.out`
  - 旧行为：LLVM codegen 失败，报 `暂不支持的 main 代码生成节点：value coercion`。
- 已实施修复：
  - 修改 `crates/scoopc/src/llvm/codegen/effect.rs` 中 `codegen_immediate_resume_top_level_tail_and_finalize(...)`。
  - 现在 tail 的 `Expr` 语句会根据位置传入 expected type：
    - 最后一个表达式：`Some(out_ty)`
    - 非最后表达式：`Some(CgTy::Unit)`
  - 该修改使嵌套 `handle` 能在 outer immediate-resume tail 中获得正确的结果类型上下文。
- 已补回归：
  - `tests/fixtures/run-pass/effect_resume_nested_escape_handle_tail.scoop`
  - `tests/fixtures/run-pass/effect_resume_nested_escape_handle_tail.stdout`
- 定向验证结果：
  - 新样例已可成功 `build` 并运行，输出与预期一致。
- 当前进行中：
  - 运行完整验证：`cargo test --all`、`cargo run -p scoop --features llvm -- test`、`cargo clippy --workspace --all-targets -- -D warnings`。
- 完整验证结果：
  - `cargo test --all`：通过。
  - `cargo run -p scoop --features llvm -- test`：通过，`fixtures: ok (925)`。
  - `cargo clippy --workspace --all-targets -- -D warnings`：通过。
- 收尾动作：
  - 将 `T2003c0b2b0` 在 `TODO.md` 标记为完成，并补写完成说明。
  - 更新 `PLAN.md`，把“当前下一步”推进到 `T2003c0b2b1`。
  - 准备检查工作区 diff 并提交本轮变更。
