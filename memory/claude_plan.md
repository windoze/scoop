# 执行计划

## 约束说明

- 本文件记录可审阅的执行步骤、决策摘要与进度更新。
- 不记录内部详细思维链，只记录足以复核工作的高层原因、计划与结果。

## 初始步骤

1. 检查最新一次提交，确认提交信息里是否提到需要先处理的既有问题。
2. 阅读 `TODO.md`，定位第一个未完成任务。
3. 如首个未完成任务过大，则拆分为更小子任务，并同步更新 `PLAN.md` 与 `TODO.md`，本次只执行拆分后的第一个子任务。
4. 实现该任务。
5. 运行与改动相关的测试、格式化和静态检查，至少包括必要的 Rust 测试以及无 warning 的 lint 检查。
6. 更新 `TODO.md`、`PLAN.md` 和本文件，记录完成状态或阻塞原因。
7. 提交本次修改，随后停止，不继续下一个任务。

## 进度

- 已创建本计划文件。
- 已检查最新提交：`54922f0935ddcc6b5724518b3b3ae92adf40385e` 的提交信息未额外声明需先处理的遗留问题。
- 已读取 `TODO.md` / `PLAN.md`，确认首个未完成任务为 `T0150h-3`：字面量运算/比较/直接方法调用矩阵与诊断锁定。
- 已判断 `T0150h-3` 范围可控，本轮不再拆分子任务，直接执行。
- 已完成盘点并定位真实缺口：call lowering 在 extension/member/default-arg/general call 降糖后把结果类型退化为 `Any`，导致 `val x = (-2.5).abs()` / `val x = id(1)` 等无注解局部绑定在 LLVM 后端失败。
- 已完成实现：
  - 修复 `crates/scoopc/src/hir/lower/expr.rs`，让调用表达式在降糖后优先保留 typecheck side table 的具体结果类型。
  - 新增 LLVM 单测 `lowered_call_results_keep_concrete_types_for_local_bindings`。
  - 新增 run-pass fixture `literal_ops_compare_direct_matrix_basic.*`。
  - 新增 typecheck failure fixtures `literal_compare_bool_is_error.scoop` 与 `literal_direct_call_float_only_is_error.scoop`。
- 已完成验证：
  - `cargo fmt --all`
  - `cargo test --all`
  - `cargo run -p scoop -- test`（`fixtures: ok (879)`）
  - `cargo clippy --workspace --all-targets --message-format short -- -D warnings`
- 已完成文档状态同步；下一步执行本轮提交并停止。
