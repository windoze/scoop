# 当前执行计划

## 约束与目标

- 本次只处理 `TODO.md` 中第一个未完成任务，完成后停止。
- 在进入具体实现前，先检查最新提交是否提到任何遗留问题；若有，先修复这些问题。
- 实施过程中同步更新本文件，记录当前状态、已完成步骤和计划调整。
- 不写出模型内部逐字推理；此文件记录可审计的执行计划、依据和决策结果。

## 初始步骤

1. 查看最新一次 Git 提交信息，确认是否提到待修复的已知问题。
2. 阅读 `TODO.md`，定位第一个未完成任务。
3. 阅读 `PLAN.md`，确认该任务上下文、依赖与已有拆分。
4. 如第一个未完成任务过大，则将其拆分为更小子任务，并更新 `PLAN.md` / `TODO.md`，随后执行新的第一个子任务。
5. 实现任务所需代码修改。
6. 运行相关格式化、检查、测试，至少覆盖：
   - 受影响模块的针对性测试
   - `cargo test --all`
   - `cargo clippy --all-targets -- -D warnings`
   - 若任务影响规范/夹具，则补充对应命令
7. 更新 `TODO.md` 和 `PLAN.md`，标记本次任务完成情况。
8. 提交 Git commit，提交信息与任务编号对应。
9. 停止，不继续处理下一个任务。

## 当前状态

- 已创建执行计划文件。
- 已检查最新提交：`d741882 [T0149] Array 字面量类型推断：移除无上下文限制`。提交正文未提到额外遗留问题，因此无需先修复“提交中点名的问题”。
- 已读取 `TODO.md` / `PLAN.md`，定位到第一个未完成任务为当前伞型任务 `T0150`。
- `T0150` 范围过大，已按语境拆分为以下子任务，并将按顺序执行：
  1. `T0150e`：`when`/模式匹配字面量完整性
  2. `T0150f`：comptime / `const` 语境
  3. `T0150g`：多文件 + 插值字符串 + 直接方法调用
  4. `T0150h`：类型上下文吸收与运算语义
  5. `T0150i`：边界值与词法/诊断审计
- 已更新 `TODO.md` / `PLAN.md`，使第一个未完成子任务为 `T0150e`。
- 已完成 `T0150e`：
  - typecheck 为 `Int/String/Bool` literal pattern 补上 subject 类型约束与稳定诊断。
  - LLVM `when` codegen 补齐 `String` subject 分派，以及 tuple 元素中的 `String` literal pattern 比较。
  - 新增 run-pass fixture `when_literal_string_bool_char_basic` 与 typecheck failure fixture `when_string_pattern_not_string_is_error`。
- 已完成验证：
  - `cargo fmt --all`
  - `cargo test --all`
  - `cargo run -p scoop -- test`（`fixtures: ok (870)`）
  - `cargo clippy --workspace --all-targets --message-format short -- -D warnings`
- 下一步（供下次调用）：执行 `T0150f`。
