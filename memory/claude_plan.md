## 本次执行计划

1. 读取 `TODO.md`，按标题是否带有 `[DONE]` 判断完成状态，锁定第一个未完成任务。
2. 检查最近一次提交信息是否直接提到与该任务相关且未完成的问题；如果这是当前任务的直接前置，则把该问题视为当前任务范围或在 `TODO.md` 中补充为前置任务。
3. 阅读当前任务在 `TODO.md` 中的要求、约束、依赖、验证条件，并仅围绕该任务收集必要上下文，避免做开放式历史问题排查。
4. 如任务可直接完成，则实现最小且正确的改动；如遇到阻断当前任务的真实缺口或规格不匹配，则先在 `TODO.md` 中加入最小必要前置任务并停止继续推进当前任务。
5. 针对本次改动运行任务要求的验证，以及必要的相关测试、`cargo fmt`、`cargo test`、`cargo clippy --all-targets -- -D warnings`（若适用且可在合理时间内完成）。
6. 完成后更新 `memory/claude_plan.md` 记录关键进展，更新 `TODO.md`：将当前任务标题显式标记为 `[DONE]` 并填写/更新完成记录；仅在阶段级计划实际变化时更新 `PLAN.md`。
7. 按仓库提交风格创建一次 git 提交，提交信息使用当前任务号，包含本次任务相关的所有未提交更改；随后停止，不继续下一个任务。

## 说明

- 我会记录可对外共享的执行计划、决策和进度。
- 不会在该文件中写入不可共享的详细内部推理，但会持续更新关键步骤、发现的问题、阻断项和验证结果，便于你检查进展。

## 进度记录

- 2026-05-08：已读取 `TODO.md` 与最新提交，确认首个未完成任务为 `CG-T07S0a1`：修复 `fun_call_add_basic.scoop` 中 refactor plain return coercion 将 `main(): Int` 尾值误判为 `Ref` 的问题。
- 2026-05-08：最新提交 `[CG-T07S0a0] Fix Option transport trace contract and record return blocker` 已明确把该问题记为当前顺序上的直接 blocker，`TODO.md` 也已将 `CG-T07S0a1` 放在 `CG-T07S0a` 之前，因此本次直接执行 `CG-T07S0a1`，无需再拆分任务。
- 2026-05-08：下一步先复现 `cargo run -p scoop -- build tests/fixtures/run-pass/fun_call_add_basic.scoop -o /tmp/fun_call_add_basic` 的失败，再只围绕 plain return preparation / lowering 收集必要上下文并实施最小修复。
- 2026-05-08：已复现失败；`dump-mir` 显示 `main` 的尾 `if` 结果被放入返回 local 后又经 `Rvalue::Transport` 做了 value erasure，`dump-hir` 进一步确认 `if` 表达式在 typed HIR 中已经丢成了非返回类型。
- 2026-05-08：已实施最小修复：
  - `crates/scoopc/src/hir/lower/expr.rs` 中 `if` / `when` 结果类型在缺少 typecheck side table 时会回退到 `ExpectedExpr.value_ty`。
  - `crates/scoopc/src/hir/lower/mod.rs` 中已知返回类型的函数体与 getter body 现在会把 declared return type 作为 expected hint 传入 tail expression lowering。
  - 新增 HIR 单测，锁定“函数尾 `if` 继承声明返回类型”的回归。
- 2026-05-08：下一步运行 `cargo fmt`、定向单测、`fun_call_add_basic` build/test，以及默认 `cargo run -p scoop -- test` 观察是否越过当前 blocker 并暴露下一个顺序 blocker。
- 2026-05-08：验证结果：`cargo test -p scoopc refactor_hir_tail_if_uses_declared_return_type_hint`、`cargo run -p scoop -- build tests/fixtures/run-pass/fun_call_add_basic.scoop -o /tmp/fun_call_add_basic`、`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/fun_call_add_basic.scoop`、`cargo run -p scoop -- test --fixtures tests/fixtures/mir/when_bind_guard.scoop`、`cargo fmt`、`cargo clippy --all-targets -- -D warnings` 通过。
- 2026-05-08：默认 `cargo run -p scoop -- test` 已越过 `fun_call_add_basic.scoop`，说明 `CG-T07S0a1` 的 blocker 已解除；但 full-suite 继续暴露 `tests/fixtures/run-pass/gc_array_class_elements_cross_function.scoop`，build 诊断为 `println::<String>` arg lowering 的 `Ref -> String` 非法 coercion。
- 2026-05-08：该新失败不在本次修复触达的 `if`/`when` tail expected-type 路径上，已按顺序约束记录为 `TODO.md` 中的新前置任务 `CG-T07S0a2`，并把 `CG-T07S0a1` 标记为 `[DONE]`。
- 2026-05-08：下一步执行提交前检查工作树，随后以 `CG-T07S0a1` 为任务号创建提交并停止，不继续处理 `CG-T07S0a2`。
