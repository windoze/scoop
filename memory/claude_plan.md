# 当前执行计划

## 目标

- 以 `TODO.md` 为唯一任务顺序来源，完成第一个标题未标记 `[DONE]` 的任务后停止。
- 当前任务为 `P3-T02`：将 `!!` 与 `as` failure 从 `Raise<RuntimeError>` 改为 `panic`，并确保它们不再参与 effect system。
- 不做开放式历史问题扫查；只处理当前任务要求、直接阻塞当前任务的问题，以及验证中暴露且未明确排期的失败。

## 执行计划

1. 检查最新提交信息，确认是否提到与 `P3-T02` 直接相关的未完成问题。
2. 阅读 `SPEC_FIX.md` 中 B3、任务列出的 typecheck/HIR/MIR/codegen/sysroot 位置，以及现有 `not_null_assert`、`runtime_typecheck_cast`、`as` 相关 fixtures。
3. 定位当前 `!!` 和 `as` 如何记录 `Raise<RuntimeError>` effect、如何在 HIR/MIR lowering 中构造 `Raise.raise(RuntimeError.*)`，以及 `panic` 在 sysroot/MIR/codegen 中的现有调用语义。
4. 修改 typecheck：停止给 `!!` 与 non-null runtime `as` failure 记录 `Raise<RuntimeError>` effect；保留 `as?` 成功/失败返回 `Option<T>` 的现有行为。
5. 修改 lowering：`!!` failure 改为调用 `scoop.core.panic` 的稳定消息；`as` runtime cast failure 改为 `panic`，不得通过 `Raise<RuntimeError>` 间接表达。
6. 审计 `RuntimeError` 剩余用途：若仍被其他路径使用则保留并在完成记录说明；若仅剩本任务删除路径使用则删除或同步文档化。
7. 添加或更新 fixtures：覆盖 `!!` 与 `as` 不再要求 `Raise<RuntimeError>` effect，覆盖成功路径不回归，并确认 `as?` 不变。
8. 按要求验证：先 `cargo fmt`，再 `cargo clippy --all-targets -- -D warnings`，随后运行 targeted fixtures、`cargo test --all --all-targets`、`python3 tools/run_fixtures.py`。
9. 更新 `TODO.md` 和 `TODO-3.md`：将 `P3-T02` 标题与索引标记为 `[DONE]`，补全完成记录；仅在阶段级计划变化时才更新 `PLAN.md`。
10. 检查 git status/diff/log，提交本任务相关全部变更，提交信息使用 `[P3-T02] ...`；提交后停止，不继续 `P3-T02R`。

## 进度记录

- 已读取 `TODO.md` 与 `TODO-3.md`，确认首个未完成任务为 `P3-T02`，依赖 `P3-T01R` 已完成。
- 已写入本次公开执行计划；下一步检查最新提交并开始阅读相关实现与 fixtures。
- 最新提交为 `461f5de7 [P3-T01R] Review operator gate semantics`，未提到 `P3-T02` 相关未完成事项；继续按 `P3-T02` 执行。
- 已完成相关实现阅读：旧路径包括 typecheck 为 `!!`/`as` 记录 `Raise<RuntimeError>`，HIR `!!` 失败合成 `Raise.raise(RuntimeError.NullAssertionFailed)`，MIR / LLVM cast 失败合成 `ClassCastFailed` raise，effect-lowered plan 还把 HIR `as` 视为 runtime raise boundary。下一步按这些路径改为 `panic` 并更新 fixtures。
- 已完成第一轮实现修改：typecheck 不再为 `!!`/`as` 记录 `Raise<RuntimeError>`；HIR `!!` failure 合成 `panic("null assertion failed")`；MIR runtime cast metadata/failure branch 改为 `panic("class cast failed")`；LLVM direct/MIR cast 失败分支改为调用 runtime panic；effect-lowered plan 不再把 HIR `as` 作为 runtime raise boundary。下一步更新 targeted fixtures 与 goldens。
- 已运行 `cargo fmt` 与 `cargo clippy --all-targets -- -D warnings`，均通过。下一步运行 targeted fixtures 以重生成/确认 HIR 与 MIR goldens，并修正因语义变更导致的 fixture 预期。
- 已完成 targeted fixture 更新与验证：新增 `!!` / `as` failure panic 运行期用例，更新 no-effect typecheck 用例、cast run-pass 用例、HIR/MIR goldens；targeted not-null、cast、HIR、MIR、UMB fixtures 均已单独通过。下一步重新确认 fmt/clippy，然后运行完整 Rust 测试与完整 fixture suite。
- 完整 Rust 测试首次发现 `mir_value_primitives_not_null_assert_is_explicit_match_and_raise` 仍断言旧 `Raise<RuntimeError>` 行为；已更新为检查 `scoop.core.panic` direct call。下一步重新运行 fmt/clippy 与完整 Rust 测试。
- 重新运行 `cargo fmt`、`cargo clippy --all-targets -- -D warnings`、`cargo test --all --all-targets` 后均通过。下一步运行完整 fixture suite。
- 完整 fixture suite 已通过：`python3 tools/run_fixtures.py`（`fixtures: ok (1555)`）。下一步更新 `TODO.md` / `TODO-3.md` 的 `P3-T02` 完成记录，然后检查 diff 并提交。
- `TODO.md` 与 `TODO-3.md` 已将 `P3-T02` 标记为 `[DONE]` 并补全完成记录；`PLAN.md` 未发生阶段级变化。下一步检查 git 状态、diff 与最近提交，随后提交本任务变更。
