# 执行计划与进度记录

## 说明

按要求先建立本文件，再开始执行仓库检查与任务实现。
出于安全与协作原因，这里记录的是可审阅的决策摘要、执行计划、关键发现和进度更新，不记录原始思维链。

## 初始执行计划

1. 检查最新一次 Git 提交的信息，确认是否提到任何已知遗留问题。
2. 如最新提交提到遗留问题，先定位并修复这些问题，再进入 `TODO.md` 的任务执行。
3. 读取 `TODO.md`，识别第一个未完成任务。
4. 读取 `PLAN.md`、`README.md`、相关代码与测试，确认该任务的上下文、依赖和现状。
5. 如果该任务过大或存在明确前置缺口：
   - 将任务拆分为更小的子任务；
   - 更新 `PLAN.md`；
   - 更新 `TODO.md`，保证依赖顺序正确；
   - 本次仅执行第一个子任务，然后停止。
6. 如果任务可直接实现：
   - 修改代码；
   - 补充或调整测试；
   - 运行相关验证，包括必要时的 `cargo fmt`、`cargo test`、`cargo clippy --all-targets -- -D warnings`。
7. 更新文档与计划状态：
   - 在 `TODO.md` 标记该任务完成；
   - 在 `PLAN.md` 记录当前状态与后续影响；
   - 若实现中发现规范缺口或阻塞，则新增前置任务并调整顺序。
8. 检查工作区变更，避免误覆盖非本次修改。
9. 提交一次 Git commit，提交信息与任务编号保持清晰一致。
10. 完成一个任务后立即停止，不继续处理下一个任务。

## 进度

- 已创建本文件。
- 已检查最新提交、`TODO.md`、`PLAN.md` 与工作区状态。
- 已确认当前首个未完成任务为 `T3016c0`。
- 已阅读 `crates/scoopc/src/typecheck/expr/stmt.rs`、`crates/scoopc/src/typecheck/expr/call.rs` 与相关 fixtures。

## 当前发现

1. `check_expr_stmt()` 的 `ast::ExprKind::Call` 分支目前只处理三类事情：
   - `@NoGC` 语境下对整条 call 做完整 type inference；
   - inline lambda 的 non-local return 结构门禁；
   - expression-statement 里的 effect-op call required-effects 收集。
2. 同一分支没有复用 `call.rs` 中 `Continuation.resume(...)` 的 builtin 规则。
3. `Continuation.resume(...)` 的 builtin 语义目前只在表达式推导路径里实现：
   - 校验 receiver 必须是 `Continuation<T, eff E>`；
   - 校验参数类型；
   - 记录 `E`；
   - 额外记录 `Raise<RuntimeError>`；
   - 写入 `continuation_resume_call_sites`。
4. 因此，语句位置的 `k.resume(...)` 会漏掉两类关键信息：
   - required effects；
   - `continuation_resume_call_sites` side table。
5. `tests/fixtures/run-pass/effect_escape_continuation_finally_normal.scoop` 当前仍保留 plain `k1.resume(42)`，与“pure main 中 resume 需要 `try/catch` 或显式 effect-row”这一既有合同不一致，必须在本任务里重新分类或改写。

## 细化执行计划

1. 在 `call.rs` 暴露一个可供 `stmt.rs` 复用的 `Continuation.resume(...)` builtin 检查入口，保持与表达式路径共享同一语义实现。
2. 在 `stmt.rs` 的 call-expression 语句路径中接入该入口，使 statement-position `Continuation.resume(...)` 也能：
   - 记录 required effects；
   - 写入 `continuation_resume_call_sites`。
3. 添加或调整测试：
   - 保留/补充 typecheck 覆盖，证明 statement-position `Continuation.resume(...)` 不再绕过 required-effect 检查；
   - 增加 side-table 单测，锁定 statement-position call site 会进入 `continuation_resume_call_sites`；
   - 重写或重分类 `effect_escape_continuation_finally_normal.scoop`，使其与当前 spec/typecheck 合同一致。
4. 运行格式化、相关测试、全量 `cargo test --all` 与 `cargo clippy --all-targets -- -D warnings`。
5. 更新 `TODO.md`、`PLAN.md`、本文件并提交一次 commit，完成 `T3016c0` 后停止。

## 当前进展更新

- 已完成代码实现：
  - `crates/scoopc/src/typecheck/expr/call.rs` 新增语句路径可复用的 `Continuation.resume(...)` builtin helper。
  - `crates/scoopc/src/typecheck/expr/stmt.rs` 已在 expression-statement 的 call 分支接入该 helper。
- 已补测试与 fixture：
  - 新增 `crates/scoopc/src/hir/lower/mod.rs` 单测，锁定 statement-position `Continuation.resume(...)` 会写入 `continuation_resume_call_sites`。
  - 新增 typecheck fixture `tests/fixtures/typecheck/continuation_resume_in_pure_main_after_handle_is_error.scoop`，覆盖 pure main outer body 中的 statement-position resume 必须报 `required_effect_not_declared`。
  - 已将 `tests/fixtures/run-pass/effect_escape_continuation_finally_normal.scoop` 改为 spec-correct 的 `try/catch` 版本，并确认该 fixture 现在应为 `EXPECT: pass`。
- 实现过程中发现并修复一处回归：
  - 初版 helper 会对所有 member call 都尝试推导 receiver，导致 `Ask.get()` 这类 effect-op call 误报 `unsupported_expr: ident（未 resolve）`。
  - 现已改为先按成员名 `resume` 做预筛，再进入 builtin 确认逻辑。
- 已完成的定向验证：
  - `cargo test -p scoopc lower_typed_single_source_file_records_statement_position_continuation_resume_call_site -- --nocapture`
  - `cargo test -p scoopc continuation_resume -- --nocapture`
  - 最小 fixtures 根目录定向 `scoop test`（4 条）全部通过：
    - `continuation_resume_requires_raise_runtime_error_missing_is_error.scoop`
    - `continuation_type_and_resume_pure_ok.scoop`
    - `continuation_resume_in_pure_main_after_handle_is_error.scoop`
    - `effect_escape_continuation_finally_normal.scoop`
- 已完成全量验证：
  - `cargo fmt`
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
- 已完成文档状态更新：
  - `TODO.md`：`T3016c0` 已标记为 `[DONE]`，并记录实现/验证结果。
  - `PLAN.md`：已补充本轮完成结论，并把当前执行顺序推进到 `T3016c0R`。
- 当前剩余步骤：检查最终 diff，提交一次 Git commit，然后停止。
