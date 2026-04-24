# T4012b3 执行记录（2026-04-25）

## 目标
- 完成 `TODO.md` 首个未完成任务 `T4012b3`：为 `@Suppress` 建立 warning code 与 expression / declaration / file suppression surface。
- 先修复执行中暴露的 pre-existing issue：`suppress_deprecated_declaration_basic.scoop` 仍错误地产生 `deprecated` warning。

## 实际执行
1. 基于已有接手状态核对工作树，并锁定残余 warning 来源。
2. 确认 `check_class_member_fun_bodies_in_type_decl` 中为 class 自身构造 `this_ty` 的内部 lowering，会在 `decl.name.span` 上误发 deprecated use-site warning。
3. 顺手核对同类内部路径，发现 `@CLayout` 的 GC-free 自类型检查也存在同样风险。
4. 在以下内部辅助路径上关闭 warning emission，而不改变用户真实 use-site 的 warning 合同：
   - `crates/scoopc/src/typecheck/expr/entry.rs`
   - `crates/scoopc/src/typecheck/annotations.rs`
5. 重新格式化并做最小复现，确认 `suppress_deprecated_declaration_basic.scoop` 不再泄漏 stderr warning。
6. 运行全量验证并更新 `TODO.md` / `PLAN.md`。

## 结果
- `@Suppress` 的 parser / AST / builtin annotation / warning-code / suppression collection / run-pass fixture surface 已收口完成。
- declaration 内部辅助 lowering 不再把声明自身 nominal type 误当成 deprecated use-site。
- `TODO.md` 已将 `T4012b3` 标记为 `[DONE]`，`PLAN.md` 已把主线推进到 `T4012c`。

## 已验证
- `cargo run -p scoop -- run tests/fixtures/run-pass/suppress_deprecated_declaration_basic.scoop`
- `cargo run -p scoop -- test`
- `cargo test --all`
- `cargo clippy --all-targets -- -D warnings`
- `cargo run -p scoop_tools -- spec-fixtures check`
