## 当前目标

按 `TODO.md` 的顺序执行第一个未完成任务；在开始具体实现前，先检查最新提交是否提到已有问题，并优先修复这些问题。

## 约束与执行原则

- 先确认最新提交、任务列表、计划文件和工作树状态。
- 如果首个未完成任务过大，先拆分任务，并同步更新 `PLAN.md` 与 `TODO.md`。
- 只完成一个任务，然后测试、更新文档、提交 git commit，并停止。
- 如果遇到规范缺口或前置依赖缺失，不做规避实现；改为补充前置任务、调整顺序、记录原因并提交。

## 初始执行计划

1. 查看最新提交信息，确认是否存在提交信息中提到但尚未修复的问题。
2. 查看 `TODO.md`，定位第一个未完成任务。
3. 查看 `PLAN.md` 与相关上下文，判断该任务是否需要拆分。
4. 阅读相关代码与测试，确认实现范围。
5. 实现任务，必要时同步补充测试与文档。
6. 运行相关验证，包括至少：
   - 受影响测试
   - `cargo test --all`
   - `cargo clippy --all-targets -- -D warnings`
7. 更新 `TODO.md`、`PLAN.md` 与本文件，记录完成情况或阻塞原因。
8. 进行一次清晰的 git 提交，然后停止。

## 进度记录

- 已创建初始计划，待开始仓库检查。
- 已检查最新提交、`TODO.md` 与 `PLAN.md`。
- 当前首个未完成任务确认是 `T3010b2b1b1`：同步 `tests/fixtures/mir/handle_perform.mir` golden，恢复全量 fixture 验证入口。
- 最新提交本身没有单独声明新的生产代码 bug；它主要把 effect 主线顺序细化，并把当前前置问题收敛到 `handle_perform` 的 MIR snapshot mismatch。
- 已复审 `tests/fixtures/hir/handle_perform.hir`、`crates/scoopc/src/hir/lower/expr.rs` 与 `crates/scoopc/src/mir/lower.rs`，确认 `handle` 表达式在 HIR 中就是 `TypeId(5)`，而 MIR lowering 会直接用 `expr.ty` 分配 handle result local，因此 golden 中 `tmp0: TypeId(0)` 属于漏同步。
- 已更新 `tests/fixtures/mir/handle_perform.mir`，把 `tmp0` 类型改为 `TypeId(5)`。
- 已验证：
  - `diff -u tests/fixtures/mir/handle_perform.mir <(cargo run -p scoop -- dump-mir tests/fixtures/mir/handle_perform.scoop)` 通过。
  - `cargo run -p scoop -- test --fixtures tests/fixtures/mir` 通过（`fixtures: ok (6)`）。
  - `cargo test --all` 通过。
  - `cargo clippy --all-targets -- -D warnings` 通过。
  - `cargo run -p scoop --features llvm -- test` 已越过 `handle_perform.mir` mismatch；新的首个失败点是 `tests/fixtures/run-pass/continuation_resume_continuation.scoop` 的 stale `EXPECT: fail`，该问题已由 `T3017` 跟踪。
- 当前任务 `T3010b2b1b1` 已完成；接下来只需同步 `TODO.md` / `PLAN.md`、检查 diff 并提交。
- 已完成 `TODO.md` / `PLAN.md` / `memory/claude_plan.md` 同步与 `git diff --check`，当前准备提交。

## 当前执行细化

1. 检查 `tests/fixtures/mir/handle_perform.scoop`、对应 golden，以及当前 `dump-mir` 输出差异。
2. 判断差异是否仅来自既有的 `ExprKind::Handle` result type 修正。
3. 若确认属于预期演进，则更新 `.mir` golden。已完成。
4. 运行 MIR 相关验证，再运行仓库要求的测试与 lint。已完成。
5. 更新 `TODO.md`、`PLAN.md` 与本文件，提交后停止。已完成文档同步与校验，待提交。
