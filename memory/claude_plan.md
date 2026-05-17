# Claude Execution Plan

更新时间：2026-05-17

说明：本文件记录本次调用的可审计执行计划、关键决策依据和进度更新；不记录不可公开的内部推理。

## 当前目标

根据 `TODO.md` 的权威排序，完成第一个标题未带 `[DONE]` 前缀的任务，然后停止。

## 初始执行计划

1. 读取 `TODO.md`，只识别第一个未完成任务，不做开放式历史问题扫描。
2. 查看该任务相关上下文，包括任务正文、依赖、验证要求，以及必要时查看最新提交是否明确提到与该任务直接相关的未完成问题。
3. 如果任务可直接完成，按最小正确变更实现，并避免 workaround、fixture-only hack 或削弱测试形状。
4. 如果发现当前任务被具体缺失特性、规格不匹配或实现边界阻塞，则在 `TODO.md` 中插入最少必要的前置任务，保持当前任务未完成，提交后停止。
5. 对实现运行相关验证；若验证暴露当前任务范围内问题，修复后重新验证。
6. 完成后在 `TODO.md` 中给该任务标题添加 `[DONE]`，更新完成记录；仅当阶段级计划改变时才更新 `PLAN.md`。
7. 提交本次所有相关变更，提交信息使用任务编号和简明说明。
8. 停止，不继续处理下一项任务。

## 进度记录

- 已写入初始计划，下一步读取 `TODO.md` 以确认第一个未完成任务。
- 已确认第一个未完成任务为 `P8-T04：HIR / typecheck——binary / unary operator 改写为 method call`。最新提交 `6f971241 [P8-T04c] Canonicalize synthetic member calls` 是该任务的直接前置修复，已完成且不需要新增前置任务。

## P8-T04 具体执行步骤

1. 阅读 HIR lowering 中 `BinaryExpr` / `UnaryExpr` 分支、`lower_binary_expr_type` 类型推断逻辑，以及现有 synthetic member call helper。
2. 复用 canonical synthetic/source-aware member call 机制，把可 method 化的 binary/unary operator 降为 `plus/minus/times/div/rem/and/or/xor/shl/shr/compare/equals/not/inv/unaryMinus/unaryPlus` 等 method call。
3. 明确保留不改写的路径：短路 `&&` / `||`、range、elvis，以及 ref type `==` / `!=` 的既有语义。
4. 增加或更新 HIR owner 测试，覆盖 `+`、比较、unary minus、短路逻辑不走 `Bool.and/or`。
5. 运行任务指定验证和必要的补充测试；如失败属于本任务范围则修复。
6. 更新 `TODO.md` / `TODO-4.md` 完成状态与记录，提交变更后停止。

## 设计细化

- 对算术、位运算、shift、Bool 非短路位逻辑、Char `plus/minus` 等 source operator，在 typecheck 阶段记录被选中的 scalar method binding；HIR lowering 复用现有 typechecked direct-call 路径，以保留重载选择和 literal expected-type。
- 对比较、相等和 unary operator，在 HIR 阶段生成 canonical top-level method call；需要嵌套调用的场景使用独立 synthetic call-site span，避免同一 span 上出现两个不同 intrinsic binding。
- `&&` / `||`、range、elvis、ref type `==` / `!=` 继续保持现有路径。

## 当前进度

- 已完成 P8-T04 代码实现与 owner 测试：`+`、`<`、unary `-` 均降为 method call，`&&` 保留短路输入形态。
- 已修复 operator method 化引起的 HIR/MIR/LLVM 旧形态断言和 effect site id drift；`cargo test -p scoopc` 已通过（857 passed）。
- 已完成验证：`cargo test --all --all-targets` 通过；`cargo clippy --all-targets -- -D warnings` 通过；完整 fixture suite `cargo run -p scoop -- test` 仅剩既有 `run-pass/mutable_array_ops_basic.scoop` 失败（1341/1342 targets passed）。
- 已更新 `TODO.md` 与 `TODO-4.md`，将 `P8-T04` 标记为 `[DONE]` 并写入完成记录。下一步提交本次变更。
