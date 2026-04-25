# 当前执行记录（2026-04-26）

## 任务结论

- 本轮目标 `T5000e1b 让 InstanceKey / dump-ir materializer 正确承载 effect-row 实参` 已完成。
- 执行过程中未发现需要再插入到 `TODO.md` 当前任务之前的新前置缺陷任务。

## 前置检查

- 已复核最新提交 `fa1973b2c34fd47b69d3baaaffd7bc5731e677b2` 的提交信息，没有发现额外声明且尚未修复的前置问题。
- 已确认本轮开始时 `TODO.md` 第一条未完成任务为 `T5000e1b`。

## 本轮实际完成项

1. 接通 effect-row 实参的请求收集与 side table
   - `crates/scoopc/src/ast/mod.rs`：
     - `TopLevelFunValueRef` 新增 `decl_file`、`decl_span`、`eff_args`；
     - 暴露 `top_level_fun_value_refs()` / `top_level_fun_call_bindings()` 整表 getter。
   - `crates/scoopc/src/typecheck/lower.rs`：
     - `record_monomorph_call(...)` / `record_top_level_fun_value_ref(...)` 记录真实 `eff_args`；
     - `record_top_level_fun_call_binding(...)` 改成直接接收 `ast::TopLevelFunCallBinding`，顺带收口参数数量。
   - `crates/scoopc/src/typecheck/expr/call.rs`：
     - direct/member/overload call、top-level function value、`TypeApply` callee 路径都已写入 `eff_args`；
     - 显式 `<eff ...>` 实参优先于默认推断路径。

2. 修复表达式级 `<eff ...>` 解析与 effect-row 参数保真
   - `crates/scoopc/src/parser/expr.rs`：
     - 修复 `looks_like_type_apply_expr` lookahead；
     - `scan_type_args_end(...)` / `scan_type_ref_end(...)` 已支持 `<eff ...>`。
   - `crates/scoopc/src/typecheck/lower.rs`、`crates/scoopc/src/typecheck/expr/{entry,stmt}.rs`：
     - effect-row 形参绑定改为 marker-preserving 语义，不再提前退化到 `Pure`。
   - `crates/scoopc/src/hir/{mod.rs,lower/mod.rs,lower/util.rs,lower/expr.rs}`：
     - HIR generic template 现在保留 effect-row param marker；
     - top-level function value mangling / fallback `TypeApply` 已把 `eff_args` 纳入。

3. 完成 MIR materializer 的 effect-row 闭环，并收口 clippy
   - `crates/scoopc/src/mir/materialize.rs`：
     - `InstanceKey`、`instance_fqn(...)`、site binding、instance substitution、effect-row substitution、direct-call fixed-point 与 cache 都已区分 `eff_args`；
     - effect-only generic fun 会真正进入 materializer，不再返回空实例；
     - top-level function value / direct call 在同 type args、不同 effect row 下会 materialize 成不同 callee；
     - 引入 `DumpMaterializeRequestSet` / `RewriteContext`，消除 `too_many_arguments`；
     - 收掉 `collapsible_if` 等 `clippy` 阻塞。

4. 新增并验证回归测试
   - `crates/scoopc/src/monomorph/lower.rs` 新增：
     - `monomorph_materializes_effect_only_generic_instance`
     - `monomorph_distinguishes_same_type_args_with_different_effect_rows`
     - `monomorph_rewrites_top_level_fun_value_effect_instance`

## 验证结果

- `cargo fmt --all`：通过
- `cargo check -p scoopc`：通过
- `cargo clippy --all-targets -- -D warnings`：通过
- `cargo test -p scoopc monomorph::lower -- --nocapture`：通过
- `cargo test --all`：通过

## 文档与任务状态更新

- 已将 `TODO.md` 中 `T5000e1b` 标记为完成，并补充完成记录与验证结果。
- 已更新 `PLAN.md`，记录 effect-row 实参闭环已完成，并把下一条待执行任务切换为 `T5000e1bR`。

## 收尾步骤

1. 检查工作区 diff 与文档更新是否一致。
2. 提交 git commit，提交信息使用 `[T5000e1b] ...` 风格。
3. 停止，不继续下一条任务。
