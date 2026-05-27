# TODO-2：P2 Parser / AST 语法 surface 收敛

> 索引：[`TODO.md`](./TODO.md)  
> 计划基线：[`PLAN.md`](./PLAN.md)  
> 覆盖阶段：P2  
> 包目标：让 parser / AST 只表达目标语言 surface，为 P3 typecheck/lowering 与 P5 overload resolution 提供稳定输入。

## P2：Parser / AST 语法 surface 收敛

### [TODO] P2-T01：删除 `perform` prefix，并迁移 effect op 调用语法

- 参考：
  - [`PLAN.md`](./PLAN.md) §5 / P2
  - [`SPEC_FIX.md`](./SPEC_FIX.md) B4
- 目标：
  - 移除 `perform Effect.op(args)` 语法；effect operation 调用成为普通 qualified call `Effect.op(args)`，由 resolution 结果区分。
- 必须修改的文件/位置：
  - `crates/scoopc_ast/src/syntax/token.rs::Keyword::Perform`
  - `crates/scoopc_ast/src/syntax/lexer.rs::lex_ident_or_keyword`
  - `crates/scoopc_ast/src/parser/expr.rs::try_parse_expr_prefix`
  - `crates/scoopc_hir/src/typecheck/expr/call/effect_op.rs::{infer_effect_op_call_expr_type,record_typechecked_effect_op_call_binding,record_inferred_performed_effect_ty}`
  - `crates/scoopc_hir/src/hir/lower/expr/members.rs::try_lower_effect_op_call_expr`
  - `crates/scoopc_mir/src/mir/lower/fn_lowering_effect.rs::lower_perform_expr`
  - `tests/fixtures/**/**/*perform*.scoop`
  - all fixtures / spec snippets found in P0-T01 containing `perform `
- 必须实现的内容：
  1. Remove parser acceptance of prefix `perform expr` in `try_parse_expr_prefix`.
  2. Keep effect op call binding through ordinary call resolution path; `Effect.op(args)` must still reach `infer_effect_op_call_expr_type` based on callee resolution, not syntax.
  3. Decide whether `perform` remains lexed as reserved keyword solely for a better diagnostic; if kept, parser must emit a clear “`perform` keyword was removed; call effect operation directly” diagnostic.
  4. Mechanically rewrite positive fixtures from `perform Raise.raise(x)` to `Raise.raise(x)` and equivalent effect op calls.
  5. Add one negative fixture for old `perform` syntax with a clear expected diagnostic.
- 必须遵从的约束：
  - Do not introduce a long-term soft alias for `perform`.
  - Do not change effect propagation semantics except removing syntax dependency.
- 验证：
  1. `python3 tools/run_fixtures.py`
  2. targeted run for fixtures changed from `perform` syntax.
  3. `cargo test --all --all-targets`
- 完成条件：
  - Positive language surface no longer uses `perform`; effect op calls still typecheck/lower as effect operations.
- 依赖：P1-T02R
- 完成记录：
  - 改动范围：
  - 核心决策：
  - 验证结果：
  - 与 `PLAN.md` / 设计文档对应闭合：

### [TODO] P2-T01R：Review `perform` 删除结果

- 参考：
  - P2-T01 完成记录
  - [`SPEC_FIX.md`](./SPEC_FIX.md) B4
- 目标：
  - 复核 `perform` 是否不再是 positive syntax，且 effect op 普通 qualified call 仍完整工作。
- 必须检查的文件/位置：
  - P2-T01 修改的 parser/typecheck/HIR/MIR 文件
  - `tests/fixtures/**/**/*perform*.scoop`
  - fixtures/spec snippets changed by P2-T01
- 必须实现的内容：
  1. 确认 parser 不再接受 `perform expr` 作为正向语法。
  2. 确认 `Effect.op(args)` 仍按 effect operation resolution 进入 typecheck/lowering。
  3. 确认旧 `perform` 只存在于 negative fixture、diagnostic 文本或历史/design baseline。
  4. 如发现 old alias 仍可通过，必须修复或阻塞 P2-T02。
- 必须遵从的约束：
  - Review 必须包含至少一个 migrated positive fixture 和一个 old syntax negative fixture。
- 验证：
  1. `python3 tools/run_fixtures.py`
  2. targeted effect op fixtures。
- 完成条件：
  - B4 parser surface 切换完成且无 effect op regression。
- 依赖：P2-T01
- 完成记录：
  - 改动范围：
  - 核心决策：
  - 验证结果：
  - 与 `PLAN.md` / 设计文档对应闭合：

### [TODO] P2-T02：将 handler keyword 从 `with` 改为 `on`

- 参考：
  - [`PLAN.md`](./PLAN.md) §5 / P2
  - [`SPEC_FIX.md`](./SPEC_FIX.md) C2
- 目标：
  - `handle { body } on { Effect.op(args) -> ... } finally { ... }` 成为唯一 handler surface。
  - value-type update `with` 保持不变。
- 必须修改的文件/位置：
  - `crates/scoopc_ast/src/syntax/token.rs`，新增 `Keyword::On` 或选择 context keyword 策略
  - `crates/scoopc_ast/src/syntax/lexer.rs::lex_ident_or_keyword`
  - `crates/scoopc_ast/src/parser/expr.rs::{parse_handle_expr,parse_handle_arm,parse_handle_op,parse_with_update_expr}`
  - `crates/scoopc_hir/src/typecheck/expr/infer.rs::infer_handle_expr_type`
  - `crates/scoopc_hir/src/hir/lower/expr/members.rs::{lower_handle_expr,lower_handle_op}`
  - `crates/scoopc_mir/src/mir/lower/fn_lowering_effect.rs::lower_handle_expr`
  - `tests/fixtures/parse/*handle*`
  - fixtures containing `handle` and ` with {`
- 必须实现的内容：
  1. Parser accepts `handle { ... } on { ... } finally { ... }`.
  2. Parser rejects `handle { ... } with { ... }` with a clear diagnostic; do not confuse this with value-type `expr with { ... }`.
  3. Update try/catch desugaring comments/spec snippets to lower to `handle ... on ...`.
  4. Migrate all positive handler fixtures to `on`.
  5. Add a negative parse fixture for old handler `with`.
- 必须遵从的约束：
  - Do not remove value-update `with` grammar.
  - Do not change handler type/effect semantics in this parser task.
- 验证：
  1. `python3 tools/run_fixtures.py`
  2. targeted parse fixtures for handler and with-update.
  3. `cargo test --all --all-targets`
- 完成条件：
  - Handler keyword `on` is accepted and old handler `with` is rejected without breaking value `with`.
- 依赖：P2-T01R
- 完成记录：
  - 改动范围：
  - 核心决策：
  - 验证结果：
  - 与 `PLAN.md` / 设计文档对应闭合：

### [TODO] P2-T02R：Review handler `on` 切换结果

- 参考：
  - P2-T02 完成记录
  - [`SPEC_FIX.md`](./SPEC_FIX.md) C2
- 目标：
  - 复核 handler `on` 已成为唯一正向 surface，value `with` 未被破坏。
- 必须检查的文件/位置：
  - `crates/scoopc_ast/src/parser/expr.rs::{parse_handle_expr,parse_with_update_expr}`
  - handler parse/typecheck/lowering fixtures
  - with-update fixtures
- 必须实现的内容：
  1. 确认 `handle ... on ...` 正例覆盖 parser/typecheck/lowering。
  2. 确认 `handle ... with ...` negative fixture 报错清晰。
  3. 确认 `expr with { ... }` update syntax 仍通过。
  4. 确认 try/catch desugaring注释/spec 不再指向 handler `with`。
- 必须遵从的约束：
  - 不得把 handler `with` 保留为 soft-deprecated alias。
- 验证：
  1. `python3 tools/run_fixtures.py`
  2. targeted handler / with-update fixtures。
- 完成条件：
  - C2 切换完整，P2-T03 可安全处理 tuple `with` path。
- 依赖：P2-T02
- 完成记录：
  - 改动范围：
  - 核心决策：
  - 验证结果：
  - 与 `PLAN.md` / 设计文档对应闭合：

### [TODO] P2-T03：实现 tuple field `.0` / `.1` 语法并移除 `._0` 正例

- 参考：
  - [`PLAN.md`](./PLAN.md) §5 / P2
  - [`SPEC_FIX.md`](./SPEC_FIX.md) B2
- 目标：
  - 将 tuple field access 从 Scoop-specific `t._0` 改为 Rust-style `t.0`。
- 必须修改的文件/位置：
  - `crates/scoopc_ast/src/syntax/lexer.rs::lex_number_literal`
  - `crates/scoopc_ast/src/syntax/token.rs::Symbol::Dot`
  - `crates/scoopc_ast/src/parser/expr.rs::{parse_member_access_expr,parse_field_path,try_parse_expr_postfix}`
  - `crates/scoopc_ast/src/ast/mod.rs` 中 member access / field path segment representation if needed
  - `crates/scoopc_hir/src/typecheck/expr/member.rs::parse_tuple_member_index`
  - `crates/scoopc_hir/src/typecheck/expr/infer.rs::parse_with_update_tuple_member_index`
  - `crates/scoopc_hir/src/hir/lower/expr/canonical_call.rs::{lower_with_update_expr,build_with_tuple_lit}`
  - `tests/fixtures/**/**/*tuple*.scoop`
  - fixtures containing `._0` / `._1`
- 必须实现的内容：
  1. Update lexer so after emitting `Dot`, the next numeric run in member-access position can be integer-only; `x.1.2` must parse as chained member segments, not a float literal.
  2. Parser accepts numeric member segments after `.` for normal member access and `with` field paths.
  3. Typecheck parses tuple index from numeric segment instead of `_N` identifier.
  4. Lowering for tuple `with` updates continues to use the same tuple reconstruction logic with numeric indices.
  5. Migrate positive fixtures from `._0` to `.0`; add negative fixture for `._0` if parser/typecheck can give a stable diagnostic.
- 必须遵从的约束：
  - Do not break ordinary float literals like `1.2` outside member access.
  - Do not allow arbitrary numeric member names on non-tuple types unless existing member model explicitly supports it.
- 验证：
  1. `python3 tools/run_fixtures.py`
  2. targeted parse/typecheck fixtures for `t.0`, `x.1.2`, tuple `with` update.
  3. `cargo test --all --all-targets`
- 完成条件：
  - `.0` / `.1` works everywhere `._0` / `._1` used to be valid; old spelling is no longer positive surface.
- 依赖：P2-T02R
- 完成记录：
  - 改动范围：
  - 核心决策：
  - 验证结果：
  - 与 `PLAN.md` / 设计文档对应闭合：

### [TODO] P2-T03R：Review tuple field 语法切换结果

- 参考：
  - P2-T03 完成记录
  - [`SPEC_FIX.md`](./SPEC_FIX.md) B2
- 目标：
  - 复核 `.0` / `.1` parser/typecheck/lowering 全链路，以及 float literal 没有回归。
- 必须检查的文件/位置：
  - P2-T03 修改的 lexer/parser/typecheck/lowering 文件
  - tuple access fixtures
  - tuple `with` update fixtures
- 必须实现的内容：
  1. 确认 `t.0`、`t.1`、`x.1.2` 正确解析。
  2. 确认普通 `1.2` float literal 不受影响。
  3. 确认旧 `._0` 不再作为正例。
  4. 确认 tuple with update 使用 numeric path。
- 必须遵从的约束：
  - 不得接受 parser fix-up 中隐藏的 float/member ambiguity regression。
- 验证：
  1. `python3 tools/run_fixtures.py`
  2. targeted tuple / float literal fixtures。
- 完成条件：
  - B2 语法切换完整且无 lexer regression。
- 依赖：P2-T03
- 完成记录：
  - 改动范围：
  - 核心决策：
  - 验证结果：
  - 与 `PLAN.md` / 设计文档对应闭合：

### [TODO] P2-T04：将 f-string 插值从 `{...}` 改为 `${...}`

- 参考：
  - [`PLAN.md`](./PLAN.md) §5 / P2
  - [`SPEC_FIX.md`](./SPEC_FIX.md) B6
- 目标：
  - f-string 中只有 `${...}` 开启插值；literal `{` / `}` 不再需要转义；不支持 `$x` shorthand。
- 必须修改的文件/位置：
  - `crates/scoopc_ast/src/syntax/lexer.rs::lex_ident_or_keyword`
  - `crates/scoopc_ast/src/syntax/string_literal.rs::{parse_f_string_text_bytes,parse_f_string_text_utf8}`
  - `crates/scoopc_ast/src/parser/expr.rs::{parse_interpolated_string_expr,split_interpolated_string_parts}`
  - `crates/scoopc_hir/src/hir/lower/expr/main_lower.rs::desugar_f_string_expr`
  - `tests/fixtures/parse/*f_string*`
  - fixtures/spec snippets containing `f"` or interpolated raw strings
- 必须实现的内容：
  1. Update f-string part splitter so `${` starts an expression and the matching `}` closes it.
  2. Treat bare `{` and `}` as literal text inside f-strings.
  3. Reject `$x` shorthand with a clear diagnostic or treat `$` as literal followed by text; positive shorthand must not be supported.
  4. Keep `f` prefix semantics and existing StringBuilder-style desugaring.
  5. Migrate positive fixtures from `f"hello {name}"` to `f"hello ${name}"`; add JSON literal brace fixture.
- 必须遵从的约束：
  - Do not remove f-string prefix.
  - Do not change non-f-string normal/raw string escaping rules.
- 验证：
  1. `python3 tools/run_fixtures.py`
  2. targeted parse/run-pass fixtures for `${...}`, literal `{}` JSON, no `$x` shorthand.
  3. `cargo test --all --all-targets`
- 完成条件：
  - f-string interpolation is JSON-friendly and old `{...}` interpolation no longer acts as positive surface.
- 依赖：P2-T03R
- 完成记录：
  - 改动范围：
  - 核心决策：
  - 验证结果：
  - 与 `PLAN.md` / 设计文档对应闭合：

### [TODO] P2-T04R：Review f-string 插值切换结果

- 参考：
  - P2-T04 完成记录
  - [`SPEC_FIX.md`](./SPEC_FIX.md) B6
- 目标：
  - 复核 `${...}` 插值、literal braces、no `$x` shorthand 三条规则。
- 必须检查的文件/位置：
  - f-string parser/string literal files modified by P2-T04
  - `tests/fixtures/parse/*f_string*`
  - any run-pass f-string fixtures
- 必须实现的内容：
  1. 确认 `${expr}` positive fixtures 覆盖普通表达式和嵌套 brace edge cases。
  2. 确认 JSON-like literal `{}` 不需要 escaping。
  3. 确认旧 `{expr}` 插值不再 positive。
  4. 确认 `$x` shorthand 未被支持。
- 必须遵从的约束：
  - 不得通过保留旧 `{...}` 插值兼容来通过 fixture。
- 验证：
  1. `python3 tools/run_fixtures.py`
  2. targeted f-string fixtures。
- 完成条件：
  - B6 语法和 fixture 覆盖完整。
- 依赖：P2-T04
- 完成记录：
  - 改动范围：
  - 核心决策：
  - 验证结果：
  - 与 `PLAN.md` / 设计文档对应闭合：

### [TODO] P2-T05：新增 `operator` modifier 的 lexer/parser/AST surface

- 参考：
  - [`PLAN.md`](./PLAN.md) §5 / P2
  - [`SPEC_FIX.md`](./SPEC_FIX.md) A3
- 目标：
  - 先让 declarations 能携带 `operator` modifier，为 P3-T01 的 operator gate 提供 AST/HIR 输入。
- 必须修改的文件/位置：
  - `crates/scoopc_ast/src/syntax/token.rs::Keyword`
  - `crates/scoopc_ast/src/syntax/lexer.rs::lex_ident_or_keyword`
  - `crates/scoopc_ast/src/ast/mod.rs::Modifier`
  - `crates/scoopc_ast/src/parser/decls.rs::parse_decl_prefix`
  - `crates/scoopc_hir/src/resolve/mod.rs::ModifierSet::from_modifiers`
  - any HIR declaration structs carrying modifiers / flags
  - `tests/fixtures/parse/*operator*`
- 必须实现的内容：
  1. Add token/AST modifier for `operator`.
  2. Parser accepts `operator fun plus(...)` and equivalent function/method declarations.
  3. Resolver/HIR modifier set preserves the flag so typecheck can query it.
  4. Add parser fixture proving `operator` is accepted only as modifier position, not as arbitrary expression keyword if keyword strategy forbids it.
- 必须遵从的约束：
  - This task must not change overload/operator resolution yet; P3-T01 owns semantic filtering.
  - Do not allow `operator` on declarations where modifiers are structurally invalid unless existing modifier parser already allows and later diagnostics reject them.
- 验证：
  1. `python3 tools/run_fixtures.py`
  2. targeted parser/typecheck smoke fixtures for `operator fun plus`.
  3. `cargo test --all --all-targets`
- 完成条件：
  - The compiler can carry `operator` modifier from source to typecheck metadata.
- 依赖：P2-T04R
- 完成记录：
  - 改动范围：
  - 核心决策：
  - 验证结果：
  - 与 `PLAN.md` / 设计文档对应闭合：

### [TODO] P2-T05R：Review `operator` modifier surface

- 参考：
  - P2-T05 完成记录
  - [`SPEC_FIX.md`](./SPEC_FIX.md) A3
- 目标：
  - 复核 `operator` modifier 能从 lexer/parser/AST 进入 HIR metadata，且未提前改变 operator resolution。
- 必须检查的文件/位置：
  - `crates/scoopc_ast/src/syntax/token.rs`
  - `crates/scoopc_ast/src/ast/mod.rs`
  - `crates/scoopc_ast/src/parser/decls.rs`
  - `crates/scoopc_hir/src/resolve/mod.rs`
  - operator parser fixtures
- 必须实现的内容：
  1. 确认 `operator fun plus` 可解析并保留 modifier flag。
  2. 确认无效位置有稳定 parser/typecheck 行为。
  3. 确认 P2-T05 未把普通 `plus` 自动变成 operator；语义 gate 留给 P3-T01。
- 必须遵从的约束：
  - Review 不得接受“只在 token 层识别但 HIR 丢失 flag”的实现。
- 验证：
  1. `python3 tools/run_fixtures.py`
  2. targeted operator parser fixtures。
- 完成条件：
  - P3-T01 可直接查询 `operator` modifier。
- 依赖：P2-T05
- 完成记录：
  - 改动范围：
  - 核心决策：
  - 验证结果：
  - 与 `PLAN.md` / 设计文档对应闭合：

### [TODO] P2-T06：解析 inline generic bounds 与 `ref` / `value` bound keywords

- 参考：
  - [`PLAN.md`](./PLAN.md) §5 / P2
  - [`SPEC_FIX.md`](./SPEC_FIX.md) C4
- 目标：
  - 支持 `<T: Bound>` 语法，并让 `<T: ref>` / `<T: value>` 在 AST/type lowering 前有明确表达。
- 必须修改的文件/位置：
  - `crates/scoopc_ast/src/parser/decls.rs::{parse_type_param_list,parse_where_clause_opt}`
  - `crates/scoopc_ast/src/ast/mod.rs::{TypeParam,WhereClause,WhereConstraint}`
  - `crates/scoopc_ast/src/syntax/token.rs` if `ref` / `value` become context tokens
  - `crates/scoopc_hir/src/typecheck/lower.rs::lower_bound_type_ref`
  - `crates/scoopc_hir/src/typecheck/where_clause.rs::{check_file_where_clauses,check_one_where_clause}`
  - `tests/fixtures/parse/*generic*`
  - `tests/fixtures/typecheck/*ref_value_bound*`
- 必须实现的内容：
  1. Parser accepts `<T: Foo>` and lowers it into the same constraint model as `where T: Foo` or a clearly unified AST representation.
  2. Parser/type lowering recognizes `ref` / `value` only in generic bound position.
  3. Add negative fixtures rejecting `ref` / `value` as parameter type, return type, type argument, `is` / `as` target, and supertype.
  4. Do not remove `AnyRef` / `AnyValue` sysroot marker yet; P3-T06 owns semantics and deletion.
- 必须遵从的约束：
  - `ref` / `value` are bound keywords, not ordinary types.
  - Do not implement bound-kind satisfaction logic in parser; only carry enough structure for typecheck.
- 验证：
  1. `python3 tools/run_fixtures.py`
  2. targeted parse/typecheck fixtures for `<T: Foo>`, `<T: ref>`, `<T: value>`, invalid positions.
  3. `cargo test --all --all-targets`
- 完成条件：
  - P3-T06 can implement kind constraints without changing parser again.
- 依赖：P2-T05R
- 完成记录：
  - 改动范围：
  - 核心决策：
  - 验证结果：
  - 与 `PLAN.md` / 设计文档对应闭合：

### [TODO] P2-T06R：Review generic bound parser surface

- 参考：
  - P2-T06 完成记录
  - [`SPEC_FIX.md`](./SPEC_FIX.md) C4
- 目标：
  - 复核 inline bounds 与 `ref` / `value` bound-only surface 已足够支撑 P3-T06。
- 必须检查的文件/位置：
  - `crates/scoopc_ast/src/parser/decls.rs`
  - `crates/scoopc_ast/src/ast/mod.rs`
  - `crates/scoopc_hir/src/typecheck/lower.rs`
  - `crates/scoopc_hir/src/typecheck/where_clause.rs`
  - generic/ref-value bound fixtures
- 必须实现的内容：
  1. 确认 `<T: Bound>` 与现有 `where T: Bound` 进入统一或等价 constraint model。
  2. 确认 `ref` / `value` 在非 bound position 被拒绝。
  3. 确认 `AnyRef` / `AnyValue` 删除未提前发生，语义替换留给 P3-T06。
  4. 确认 parser 选择 keyword/context keyword 后没有污染普通 identifiers。
- 必须遵从的约束：
  - 不得让 `ref` / `value` 作为普通类型进入 AST/typecheck。
- 验证：
  1. `python3 tools/run_fixtures.py`
  2. targeted generic/ref-value fixtures。
- 完成条件：
  - P2 包全部完成；P3 可以开始做语义落地。
- 依赖：P2-T06
- 完成记录：
  - 改动范围：
  - 核心决策：
  - 验证结果：
  - 与 `PLAN.md` / 设计文档对应闭合：
