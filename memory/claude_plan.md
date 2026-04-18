# 本轮执行计划

## 约束说明

- 按要求先记录可审阅的执行计划，再进行仓库检查与实现工作。
- 此文件记录高层步骤、决策依据、进度和调整，不记录冗长的内部推理原文。
- 本轮目标是：处理 `TODO.md` 中第一个未完成任务，完成后更新文档、提交 git，然后停止。

## 初始步骤

1. 检查最新一次 git 提交：
   - 查看提交说明和相关改动。
   - 判断是否显式提到尚未解决的问题、回归或待修复项。
   - 如果存在，先将这些问题纳入本轮范围并优先修复。
2. 阅读 `TODO.md`：
   - 找出第一个未完成任务。
   - 结合代码现状判断任务是否可以在本轮完整完成。
3. 如任务过大：
   - 更新 `PLAN.md`，把任务拆成更小的可执行子任务。
   - 更新 `TODO.md`，把原任务替换或补充为有依赖顺序的子任务。
   - 执行拆分后的第一个子任务。

## 实施步骤

1. 阅读相关代码、规格、测试和任务上下文。
2. 实现当前目标任务，避免规避实现边界或引入临时性 hack。
3. 如果发现规范不匹配、语言特性缺失或已有实现 bug：
   - 先定位根因。
   - 若阻塞当前任务且无法在本轮直接完成，则按要求更新 `TODO.md` / `PLAN.md`，记录依赖关系与阻塞原因，然后提交并停止。
4. 为变更补充或调整测试，确保行为被覆盖。

## 验证步骤

按变更范围选择并执行合适的验证，至少包括：

- 相关单元测试 / 集成测试 / fixture 测试
- `cargo test --all`
- `cargo clippy --all-targets -- -D warnings`（如当前仓库配置允许，需要确保无 warning）
- 其他与当前任务直接相关的命令

如果全量验证成本过高，会先跑与改动直接相关的验证，再视情况补全更大范围检查；最终记录实际运行结果。

## 收尾步骤

1. 更新 `TODO.md`，将本轮完成的任务标记为已完成。
2. 更新 `PLAN.md`，记录当前状态、后续顺序及必要调整。
3. 回写本文件，注明关键进展、实现摘要和验证结果。
4. 使用清晰的提交信息创建 git commit。
5. 停止，不继续处理下一个任务。

## 进度记录

- [x] 已写入本轮初始计划。
- [x] 已检查最新提交：`[T3016c0] Align statement-position continuation resume typecheck`，未发现提交说明中额外声明的待修复遗留问题。
- [x] 已识别 `TODO.md` 中第一个未完成任务：`T3016c0R`。
- [x] 已完成 `T3016c0R` 复审，并修复发现的 effect-op `resume` 名称碰撞缺口。
- [x] 已更新 `TODO.md` / `PLAN.md` / 本文件，并已准备创建 git 提交。

## 本轮复审发现

- `T3016c0` 的 statement-position `Continuation.resume(...)` 补线里存在一个真实生产缺口：
  - `check_expr_stmt()` 在语句位置的 `Call` 分支里，先调用 `infer_effect_op_call_expr_type(...)`，随后无条件调用 `infer_continuation_resume_call_expr_type(...)`。
  - 若存在 effect op 也名为 `resume`（例如 `Echo.resume(1)`），前一个 helper 会成功识别 effect op，但后一个 helper 仍会继续按 builtin `Continuation.resume` 路径尝试推导 receiver。
  - 该路径会把 effect type 限定符当成普通表达式 receiver 处理，违背“只对 typecheck 已证实的 builtin call site 写 side table”的目标，也会让成员名碰撞场景存在误判风险。

## 修复计划调整

1. 在 `crates/scoopc/src/typecheck/expr/stmt.rs` 中让 statement-position `Continuation.resume(...)` helper 仅在当前 call 未被 effect-op helper 接管时才执行。
2. 在 `crates/scoopc/src/hir/lower/mod.rs` 中新增回归单测，锁定 effect op 名称碰撞（`resume`）不会污染 `continuation_resume_call_sites`。
3. 运行定向单测、fixture 验证、`cargo test --all` 与 `cargo clippy --all-targets -- -D warnings`。

## 实施结果

- 已在 `crates/scoopc/src/typecheck/expr/stmt.rs` 中把 statement-position call 路径收口为：
  - 先尝试 effect-op helper。
  - 仅当当前调用没有先被 effect-op helper 接管时，才继续尝试 builtin `Continuation.resume(...)` helper。
- 已在 `crates/scoopc/src/hir/lower/mod.rs` 中新增回归单测：
  - `lower_typed_single_source_file_does_not_record_effect_op_named_resume_as_builtin_call_site`
- 已确认 `TODO.md` 的本轮任务应记为完成，下一项推进到 `T3016c`。

## 验证结果

- `cargo test -p scoopc lower_typed_single_source_file_records_statement_position_continuation_resume_call_site -- --nocapture`
- `cargo test -p scoopc lower_typed_single_source_file_does_not_record_effect_op_named_resume_as_builtin_call_site -- --nocapture`
- 最小 fixture 根目录定向 `scoop test`：包含
  - `continuation_resume_requires_raise_runtime_error_missing_is_error.scoop`
  - `continuation_type_and_resume_pure_ok.scoop`
  - `continuation_resume_in_pure_main_after_handle_is_error.scoop`
  - `effect_escape_continuation_finally_normal.scoop`
- `cargo test --all`
- `cargo clippy --all-targets -- -D warnings`

以上验证均已通过。
