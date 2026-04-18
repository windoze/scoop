# 执行计划

## 约束与目标

- 本轮只处理 `TODO.md` 中第一个未完成任务，完成后停止。
- 在开始实际实现前，先检查最新提交是否提到遗留问题；若有，优先修复。
- 若当前任务过大，需要先拆分任务，并同步更新 `TODO.md` 与 `PLAN.md`。
- 任何发现的规范不匹配、缺失特性或阻塞项，都必须先记录为新的前置任务，不能用变通方案绕过。
- 需要在过程中持续更新本文件，记录计划调整、关键结论、执行进度与验证结果。

## 初始执行步骤

1. 查看最新一次 Git 提交，确认是否提到已知问题、遗留缺陷或需要优先处理的事项。
2. 读取 `TODO.md`，识别第一个未完成任务。
3. 读取 `PLAN.md`，核对该任务的上下文、依赖关系与已有规划。
4. 根据任务复杂度判断是否需要拆分；如需要，先更新 `TODO.md` 与 `PLAN.md`，然后只执行拆分后的第一个子任务。
5. 阅读相关代码、测试、规范或夹具，确认正确实现边界。
6. 实现任务，并补充或调整测试。
7. 运行与该改动相关的验证命令；若任务影响面较大，再运行更完整的检查，例如格式化、测试与 `clippy`。
8. 更新 `TODO.md`、`PLAN.md` 和本文件，记录完成情况与任何依赖调整。
9. 提交本轮修改，提交信息对应当前任务。

## 推理摘要

- 当前尚未读取仓库状态，因此具体任务和受影响模块仍待确认。
- 为满足流程要求，本文件先记录可审阅的执行计划与约束，而不是未整理的原始思维草稿。
- 后续如果发现最新提交中已经显式指出某个未修复问题，该问题将先于 `TODO.md` 中的普通任务处理。

## 进度记录

- 已创建本文件并写入初始计划。
- 已检查最新提交 `062983e`（`[T3103a0] Track statement-call gate blocker`）：提交本身是在记录既有阻塞，没有额外需要先修复但尚未入列的实现问题。
- 已读取 `TODO.md` / `PLAN.md`，确认当前第一个未完成任务是 `T3103a0`：恢复 statement-position 普通调用的 `@Unsafe` / `@Extern` / `@NoGC` / `const` 门禁。
- 已初步检查 `crates/scoopc/src/typecheck/expr/stmt.rs`：
  - `check_expr_stmt` 对 `ExprKind::Call` 目前只在 `@NoGC` 上下文里做完整 `infer`；
  - 其它 statement-position 普通调用仅递归检查 callee/args、effect-op、`Continuation.resume(...)` 和 lambda non-local return，导致 value-position 已有的统一调用门禁没有闭环复用。
- 当前判断：`T3103a0` 可以直接实现，不需要再拆分子任务。实现方向是让 statement-position 普通调用走与 value-position 相同的调用 typecheck，同时保留现有 lambda non-local return 的特判逻辑，避免回归。
- 已完成生产代码修改：
  - `crates/scoopc/src/typecheck/expr/stmt.rs` 的 statement-position `Call` 现在会复用共享调用 gate，覆盖 `@Unsafe` / `@Extern` / `@NoGC` / `const`。
  - 为避免把未单独跟踪的“普通 callee effect row 在 statement 位置传播”语义变更混入本轮，普通调用检查改为“统一 infer + 暂停普通 effects 收集”，而 effect op / `Continuation.resume(...)` 继续按原语义记录立即 effects。
  - 为避免 lambda non-local return 预检误把 implicit `it` / 未完整推断的 binder 当成完整调用 typecheck，已将语句层递归拆成 `WithUnifiedGate` / `StructuralOnly` 两种模式。
- 在验证过程中发现并修复了一个与当前任务直接相关的测试工具问题：
  - `crates/scoop/src/fixtures/mod.rs` 之前会把 `cargo run -p scoop -- test --fixtures tests/fixtures/unsafe_nogc` 这类“根目录直接指向单 phase 子目录”的调用误判成 parse phase；
  - 现已修复 phase-root 判定，并补了单测，确保 `unsafe_nogc` / `mir` 等子目录可直接作为 fixtures 根目录运行。
- 已新增回归 fixtures：
  - `tests/fixtures/unsafe_nogc/unsafe_extension_statement_call_requires_unsafe_is_error.scoop`
  - `tests/fixtures/unsafe_nogc/nogc_statement_call_non_nogc_function_is_error.scoop`
  - `tests/fixtures/unsafe_nogc/nogc_function_value_statement_call_is_error.scoop`
  - `tests/fixtures/typecheck/const_fun_statement_call_non_const_fun_is_error.scoop`
- 已完成验证：
  - `cargo run -p scoop -- test --fixtures tests/fixtures/unsafe_nogc` → `fixtures: ok (31)`
  - `cargo run -p scoop -- run tests/fixtures/run-pass/kotlin_ranges_progressions_basic.scoop` → 输出恢复为预期
  - `cargo run -p scoop -- test` → `fixtures: ok (1000)`
  - `cargo test --all` → 通过
  - `cargo clippy --all-targets -- -D warnings` → 通过
- 待完成事项：更新 `TODO.md` / `PLAN.md` 状态并提交本轮修改。
