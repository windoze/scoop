# TODO-2：P2 Parser / AST 语法 surface 收敛

> 索引：[`TODO.md`](./TODO.md)  
> 计划基线：[`PLAN.md`](./PLAN.md)  
> 覆盖阶段：P2  
> 包目标：让 parser / AST 只表达目标语言 surface，为 P3 typecheck/lowering 与 P5 overload resolution 提供稳定输入。

## P2：Parser / AST 语法 surface 收敛

### [DONE] P2-T01：删除 `perform` prefix，并迁移 effect op 调用语法

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
    - `crates/scoopc_ast`：保留 `perform` 词法关键字用于定向诊断；移除 `try_parse_expr_prefix` 对 `perform expr` 的接受，新增 `scoop::parse::perform_keyword_removed`。
    - `crates/scoopc_hir` / `crates/scoopc_mir`：将 Rust 测试内嵌 Scoop snippets 从 `perform Boom.ping()` 改为普通 `Boom.ping()` effect op call，并更新相关测试命名/注释。
    - `tests/fixtures/parse/perform_keyword_removed.scoop`：新增旧 `perform` prefix negative fixture；现有 positive effect op fixture 继续使用 `Effect.op(args)` 普通 qualified call。
    - `SCOOP_FULL_SPEC.md` 与 `docs/spec/language_spec-part*.md`：将活跃 spec 示例/优先级表迁移到普通 qualified effect op call；split spec 明确 `perform` 仅作为移除前缀的解析期错误提示。
  - 核心决策：
    - `perform` 不作为长期 soft alias 保留；parser 遇到该关键字立即报 “`perform` keyword was removed; call effect operation directly”。
    - `Keyword::Perform` 暂时保留在 lexer/token 层，只服务旧语法的清晰诊断；`perform` 不再是语句起始或 prefix operator。
    - effect op 语义继续由 ordinary qualified call resolution 识别：`Effect.op(args)` 在 call dispatch 中进入 `infer_effect_op_call_expr_type`，HIR lowering 仍根据 typecheck 写回的 effect-op call binding 降到内部 effect operation lowering。
  - 验证结果：
    - `cargo fmt`：通过。
    - `python3 tools/spec_fixtures.py check`：通过，输出 `spec fixtures: ok (1)`。
    - `cargo clippy --all-targets -- -D warnings`：通过。
    - `cargo build -p scoop -p scoopc`：通过，用于刷新 fixture runner 使用的 CLI binaries。
    - Targeted fixtures：`python3 tools/run_fixtures.py tests/fixtures/parse/perform_keyword_removed.scoop`、`tests/fixtures/hir/handle_perform.scoop`、`tests/fixtures/run-pass/effect_escape_continuation_multi_perform_basic.scoop` 均通过。
    - `cargo test --all --all-targets`：通过。
    - `python3 tools/run_fixtures.py`：通过，输出 `fixtures: ok (1535)`。
  - 与 `PLAN.md` / 设计文档对应闭合：
    - 闭合 `SPEC_FIX.md` B4：正向 surface 使用 `Effect.op(args)`，旧 `perform` prefix 只作为 parser diagnostic / negative fixture 存在。
    - 未改变阶段边界、依赖结构或完成条件，因此无需更新 `PLAN.md`。

### [DONE] P2-T01R：Review `perform` 删除结果

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
    - 复核 P2-T01 parser/typecheck/HIR/MIR 路径：`perform` 仍只作为 reserved keyword 提供定向 parser diagnostic，`try_parse_expr_prefix` 不再接受 `perform expr` 正向语法。
    - 复核 ordinary qualified effect operation call 主线：`Effect.op(args)` / `Raise.raise(args)` 通过 member-call dispatch 进入 `infer_effect_op_call_expr_type`，HIR lowering 继续通过 typecheck 记录的 effect-op binding 生成内部 `Perform` 表达式。
    - 复核 active spec / fixture / sysroot / compiler 搜索结果：旧 prefix spelling 只保留在 negative fixture、诊断/help 文本、设计/说明性 prose 或内部 lowering/runtime terminology 中；未发现可通过的 old syntax alias。
  - 核心决策：
    - 接受 P2-T01 的实现边界：保留 `Keyword::Perform` 仅用于清晰错误信息，不作为 soft-deprecated alias。
    - Review 未发现需要阻塞 P2-T02 的 effect op regression；`perform` 命名仍可作为内部 IR / runtime 概念使用，不代表 source syntax。
  - 验证结果：
    - `cargo fmt --all`：通过。
    - `cargo clippy --all-targets -- -D warnings`：通过。
    - Targeted fixtures：`python3 tools/run_fixtures.py tests/fixtures/parse/perform_keyword_removed.scoop`、`python3 tools/run_fixtures.py tests/fixtures/hir/handle_perform.scoop`、`python3 tools/run_fixtures.py tests/fixtures/run-pass/effect_escape_continuation_multi_perform_basic.scoop` 均通过。
    - `cargo test --all --all-targets`：通过。
    - `python3 tools/run_fixtures.py`：通过，输出 `fixtures: ok (1535)`。
  - 与 `PLAN.md` / 设计文档对应闭合：
    - 复核闭合 `SPEC_FIX.md` B4：source positive surface 已切换到普通 qualified effect op call，旧 `perform` prefix 只作为 parser negative case 存在。
    - 未改变阶段边界、依赖结构或完成条件，因此无需更新 `PLAN.md`。

### [DONE] P2-T02：将 handler keyword 从 `with` 改为 `on`

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
    - `crates/scoopc_ast`：新增 `Keyword::On` 与 lexer 映射；`parse_handle_expr` 改为接受 `handle { ... } on { ... }`，旧 `handle { ... } with { ... }` 走新增 `scoop::parse::handler_with_keyword_removed` 定向诊断。
    - 保留 `parse_with_update_expr` 的 `Keyword::With` postfix 路径；`expr with { ... }` value-update 语法未改变。
    - `SCOOP_FULL_SPEC.md` 与 `docs/spec/language_spec-part4.md`：active handler / try-catch desugaring 示例迁移到 `on`。
    - Rust 注释/内嵌 Scoop snippets 与所有 positive handler fixtures 从 `} with {` 迁移到 `} on {`；新增 `tests/fixtures/parse/handle_with_keyword_removed.scoop` 作为旧 handler `with` negative fixture。
    - 重新生成受源码 span 变化影响的 parse / HIR / MIR / effect-facts / effect-lowered golden snapshots。
  - 核心决策：
    - 采用显式 `Keyword::On` 策略，而不是把 `on` 作为普通 identifier 后的 context keyword；当前 active sysroot / fixtures 未使用 `on` 作为标识符。
    - 旧 handler `with` 不保留 soft alias；parser 在 handler keyword 位置报迁移错误，同时为了减少级联错误会消费形态完整的旧 arm/finally block 后再返回该诊断。
    - Handler typecheck、HIR lowering、MIR/effect lowering 的语义输入仍是同一个 AST `Handle` 结构，本任务只更换 source surface，不改变 handler type/effect/dispatch 语义。
  - 验证结果：
    - `cargo fmt --all`：通过。
    - `cargo clippy --all-targets -- -D warnings`：通过。
    - `cargo build -p scoop -p scoopc`：通过，用于刷新 fixture runner / snapshot 生成使用的 CLI binaries。
    - Targeted fixtures：`tests/fixtures/parse/handle_expr_minimal.scoop`、`tests/fixtures/parse/handle_with_keyword_removed.scoop`、`tests/fixtures/parse/handle_immediate_resume_removed.scoop`、`tests/fixtures/parse/with_update_expr.scoop`、`tests/fixtures/hir/handle_perform.scoop`、`tests/fixtures/mir/handle_perform.scoop`、`tests/fixtures/effect_facts/handle_perform.scoop`、`tests/fixtures/effect_lowered/handle_perform.scoop` 均通过。
    - `cargo test --all --all-targets`：通过。
    - `python3 tools/spec_fixtures.py check`：通过，输出 `spec fixtures: ok (1)`。
    - `python3 tools/run_fixtures.py`：通过，输出 `fixtures: ok (1536)`。
  - 与 `PLAN.md` / 设计文档对应闭合：
    - 闭合 `SPEC_FIX.md` C2：`handle { ... } on { ... } finally { ... }` 成为唯一 positive handler surface；旧 handler `with` 只保留为 parser negative case / diagnostic 文本，value-update `with` 继续可用。
    - 未改变阶段边界、依赖结构或完成条件，因此无需更新 `PLAN.md`。

### [DONE] P2-T02R：Review handler `on` 切换结果

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
    - 复核 P2-T02 parser surface：`parse_handle_expr` 只接受 `handle { ... } on { ... }`，旧 `handle { ... } with { ... }` 走 `scoop::parse::handler_with_keyword_removed` 定向诊断；`parse_with_update_expr` 仍保留 `expr with { ... }` postfix value-update 路径。
    - 复核 handler 正例覆盖 parser / HIR / MIR / effect-facts / effect-lowered fixtures，旧 handler `with` negative fixture 存在且错误码稳定。
    - 复核 active spec / split spec / comments：handler 示例与 try/catch desugaring均指向 `on`；review 中修正 `docs/spec/language_spec-part1.md` 的关键字列表，补入 `on` 并说明其 handler arm 用途。
  - 核心决策：
    - 接受 P2-T02 的实现边界：`Keyword::On` 是显式关键字，handler `with` 不作为 soft-deprecated alias 保留。
    - `with` 继续只作为 value-type copy-update surface 使用；本 review 未改变 handler typecheck、HIR lowering、MIR/effect lowering 语义。
    - split spec 关键字列表遗漏 `on` 属于 review 修正，不改变 `PLAN.md` 阶段边界或依赖结构。
  - 验证结果：
    - `cargo fmt --all`：通过。
    - `cargo clippy --all-targets -- -D warnings`：通过。
    - `python3 tools/spec_fixtures.py check`：通过，输出 `spec fixtures: ok (1)`。
    - Targeted fixtures：`tests/fixtures/parse/handle_expr_minimal.scoop`、`tests/fixtures/parse/handle_with_keyword_removed.scoop`、`tests/fixtures/parse/handle_immediate_resume_removed.scoop`、`tests/fixtures/parse/with_update_expr.scoop`、`tests/fixtures/hir/handle_perform.scoop`、`tests/fixtures/mir/handle_perform.scoop`、`tests/fixtures/effect_facts/handle_perform.scoop`、`tests/fixtures/effect_lowered/handle_perform.scoop`、`tests/fixtures/typecheck/with_update_struct_field_ok.scoop`、`tests/fixtures/run-pass/with_update_simple.scoop` 均通过。
    - `cargo test --all --all-targets`：通过。
    - `python3 tools/run_fixtures.py`：通过，输出 `fixtures: ok (1536)`。
  - 与 `PLAN.md` / 设计文档对应闭合：
    - 复核闭合 `SPEC_FIX.md` C2：`handle { ... } on { ... } finally { ... }` 是唯一 positive handler surface，旧 handler `with` 只保留为 parser negative case / diagnostic 文本，value-update `with` 继续可用。
    - 未改变阶段边界、依赖结构或完成条件，因此无需更新 `PLAN.md`。

### [DONE] P2-T03：实现 tuple field `.0` / `.1` 语法并移除 `._0` 正例

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
    - `crates/scoopc_ast`：lexer 在 `.` / `?.` 后把下一段数字按 integer token 切分，使 `x.1.2` 不再被贪婪为 float；parser 的 ordinary/safe member access 与 `with` field path 接受 numeric tuple segment，并对 `with { 1.0: ... }` 这类路径起始 `FloatLiteral` 做路径段拆分。
    - `crates/scoopc_hir` / `crates/scoopc_mir` / `crates/scoopc_codegen_llvm`：tuple index parser 从 `_N` 改为 numeric `N`；tuple destructuring、vararg tuple spread、tuple `with` 重建等合成 HIR member name 同步改为 numeric；新增 `scoop::typecheck::tuple_member_old_syntax` 诊断。
    - Active spec / split spec / Rust embedded Scoop snippets / positive fixtures 迁移到 `.0` / `.1`；新增 `tests/fixtures/parse/tuple_access_numeric_member.scoop`，并新增 `tuple_access_old_member_syntax_is_error.scoop` 与 `with_update_tuple_old_member_syntax_is_error.scoop` negative fixtures。
    - 重新生成受 tuple member spelling 影响的 HIR / MIR golden snapshots。
  - 核心决策：
    - AST 仍复用现有 span-backed `MemberIdent` / `FieldPath` segment representation，不为 numeric field 新增 AST enum；typecheck/lowering/codegen 从源码 span 或 HIR member name 解析 numeric index。
    - 旧 `._N` / `_N` tuple spelling 不保留 soft alias：当 receiver / `with` aggregate 确认为 tuple 时，旧 spelling 走前端 typecheck diagnostic。
    - `with` path 起始的 `1.0` 在 field-path parser 中拆为 `1`、`0` 两段，避免破坏普通 expression 位置的 `1.2` float literal。
  - 验证结果：
    - `cargo fmt --all`：通过。
    - `cargo clippy --all-targets -- -D warnings`：通过。
    - `python3 tools/spec_fixtures.py check`：通过，输出 `spec fixtures: ok (1)`。
    - `cargo build -p scoop -p scoopc`：通过，用于刷新 fixture runner / CLI binaries。
    - Targeted fixtures：`tests/fixtures/parse/tuple_access_numeric_member.scoop`、`tests/fixtures/typecheck/tuple_access_old_member_syntax_is_error.scoop`、`tests/fixtures/typecheck/with_update_tuple_old_member_syntax_is_error.scoop`、`tests/fixtures/typecheck/with_update_tuple_nested_path_ok.scoop`、`tests/fixtures/typecheck/with_update_tuple_overlapping_paths_is_error.scoop`、`tests/fixtures/typecheck/with_update_tuple_field_type_mismatch_is_error.scoop`、`tests/fixtures/run-pass/tuple_access_basic.scoop`、`tests/fixtures/run-pass/with_update_tuple_nested_single_eval_basic.scoop` 均通过。
    - Focused Rust regression：`cargo test -p scoop --test p7_default_pipeline single_pipeline_runs_multi_type_param_effect_payload_dispatch_cli` 通过。
    - `cargo test --all --all-targets`：通过。
    - `python3 tools/run_fixtures.py`：通过，输出 `fixtures: ok (1538)`。
  - 与 `PLAN.md` / 设计文档对应闭合：
    - 闭合 `SPEC_FIX.md` B2：active source surface 使用 Rust-style tuple field `.0` / `.1`，旧 `._0` / `_0` tuple spelling 只保留在 negative fixtures、任务记录和历史/design baseline。
    - 未改变阶段边界、依赖结构或完成条件，因此无需更新 `PLAN.md`。

### [DONE] P2-T03R：Review tuple field 语法切换结果

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
    - 复核 P2-T03 修改的 lexer / parser / typecheck / lowering / codegen 路径：`.` / `?.` 后的 numeric member segment 作为 integer token 进入 parser，ordinary/safe member access 与 `with` field path 均可表达 numeric tuple segment。
    - 复核 tuple index 解析已从旧 `_N` spelling 切换为 numeric `N`，tuple destructuring / vararg spread / tuple `with` reconstruction / LLVM aggregate member lookup 的合成 member name 与解析逻辑保持一致。
    - 复核 fixture 覆盖：`t.0` / `t.1` / chained numeric access、普通 `1.2` float literal、旧 `._0` negative、tuple `with` numeric path 与 nested path 均有覆盖。
  - 核心决策：
    - 接受 P2-T03 的 AST 表示边界：numeric field 继续复用 span-backed member / field-path segment 表示，前端与 lowering/codegen 通过 numeric segment text 解析 tuple index。
    - 旧 `._N` 不作为 soft-deprecated alias 保留；只有在 tuple receiver / aggregate 语境下走清晰 typecheck diagnostic，非 tuple numeric member 仍按现有 member model 报错。
    - `with { 1.0: ... }` 的 field-path 拆分仅限 field path parser，不改变 ordinary expression 位置的 float literal 词法与解析。
  - 验证结果：
    - `cargo fmt --all`：通过。
    - `cargo clippy --all-targets -- -D warnings`：通过。
    - Targeted fixtures（逐个运行）：`tests/fixtures/parse/tuple_access_numeric_member.scoop`、`tests/fixtures/typecheck/tuple_access_old_member_syntax_is_error.scoop`、`tests/fixtures/typecheck/with_update_tuple_old_member_syntax_is_error.scoop`、`tests/fixtures/typecheck/with_update_tuple_nested_path_ok.scoop`、`tests/fixtures/typecheck/with_update_tuple_overlapping_paths_is_error.scoop`、`tests/fixtures/typecheck/with_update_tuple_field_type_mismatch_is_error.scoop`、`tests/fixtures/run-pass/tuple_access_basic.scoop`、`tests/fixtures/run-pass/with_update_tuple_nested_single_eval_basic.scoop`、`tests/fixtures/run-pass/float_literal_runtime_basic.scoop` 均通过。
    - `cargo test --all --all-targets`：通过。
    - `python3 tools/run_fixtures.py`：通过，输出 `fixtures: ok (1538)`。
  - 与 `PLAN.md` / 设计文档对应闭合：
    - 复核闭合 `SPEC_FIX.md` B2：active source surface 使用 `.0` / `.1` tuple field access，旧 `._0` / `_0` tuple spelling 只保留在 negative fixtures、任务记录和历史/design baseline。
    - 未发现 lexer float/member ambiguity regression；未改变阶段边界、依赖结构或完成条件，因此无需更新 `PLAN.md`。

### [DONE] P2-T04：将 f-string 插值从 `{...}` 改为 `${...}`

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
    - `crates/scoopc_ast`：将 f-string splitter 改为仅在 `${` 时进入 interpolation expression；裸 `{` / `}` 在 f-string text 中保留为 literal；移除 f-string text 的 `{{` / `}}` undouble 解码路径和旧 unescaped `}` parser diagnostic。
    - `crates/scoopc_hir` / `crates/scoopc_codegen_llvm`：更新内嵌 Scoop snippets 与注释到 `${...}`，保留现有 StringBuilder-style desugaring 和 ToString 检查。
    - `SCOOP_FULL_SPEC.md` 与 `docs/spec/language_spec-part*.md`：active f-string 示例迁移到 `${...}`，并说明 literal braces 不需要 doubling。
    - `tests/fixtures/**`：迁移所有 positive f-string interpolation fixture 到 `${...}`；更新 `f_string_interpolation` parse snapshot；扩展 `fstring_desugar_basic` 覆盖 JSON literal braces、旧 `{name}` literal text、`$name` no-shorthand literal text。
  - 核心决策：
    - `$x` shorthand 按普通 text 处理而不是诊断报错；因此 `$name` 在 f-string 中输出 `$name`，不进入 name resolution / typecheck。
    - 旧 `{expr}` spelling 不作为 interpolation 兼容层保留；它现在只是 literal brace text，只有 `${expr}` 会产生 `InterpolatedStringPart::Expr`。
    - `f` prefix 与 HIR lowering 形态不变：parser 只改变 Text/Expr 分片边界，lowering 继续生成 StringBuilder `add(...).toString()` 链。
  - 验证结果：
    - `cargo fmt --all`：通过。
    - `cargo clippy --all-targets -- -D warnings`：通过。
    - `cargo build -p scoop -p scoopc`：通过，用于刷新 fixture runner 使用的 CLI binaries。
    - Targeted fixtures：`tests/fixtures/parse/f_string_interpolation.scoop`、`tests/fixtures/run-pass/fstring_desugar_basic.scoop`、`tests/fixtures/codegen/f_string_interpolation.scoop`、`tests/fixtures/typecheck/fstring_interpolation_non_tostring_is_error.scoop` 均通过。
    - `python3 tools/spec_fixtures.py check`：通过，输出 `spec fixtures: ok (1)`。
    - `cargo test --all --all-targets`：通过。
    - `python3 tools/run_fixtures.py`：通过，输出 `fixtures: ok (1538)`。
  - 与 `PLAN.md` / 设计文档对应闭合：
    - 闭合 `SPEC_FIX.md` B6：f-string interpolation opener 已切换为 `${...}`；literal `{}` 在 f-string 中 JSON-friendly；`$x` shorthand 未作为 positive interpolation surface 支持。
    - 未改变阶段边界、依赖结构或完成条件，因此无需更新 `PLAN.md`。

### [DONE] P2-T04R：Review f-string 插值切换结果

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
    - 复核 P2-T04 f-string splitter / string literal text 处理 / dedicated parse + run-pass fixtures，确认 `${...}`、literal braces、旧 `{expr}` text、`$x` text 四条 B6 规则均有覆盖。
    - 补齐 nested brace positive coverage：`tests/fixtures/parse/f_string_interpolation.scoop` 新增 `${if (true) { 1 } else { 2 }}`，并刷新 `f_string_interpolation.ast`；`tests/fixtures/run-pass/fstring_desugar_basic.scoop` 新增 nested brace runtime 覆盖。
    - 补齐 interpolation expression 中 char literal brace 的 scanner 覆盖：parser brace matcher 现在跳过字符字面量，避免 `f"${'}'}"` 中的 `}` 被误判为 interpolation close；parse/run-pass fixtures 覆盖该路径。
  - 核心决策：
    - 不保留旧 `{expr}` interpolation 兼容层；旧写法继续作为 literal text 覆盖。
    - `$x` shorthand 不作为 interpolation surface；继续作为 literal text 覆盖。
    - nested brace / char brace 属于同一 f-string delimiter matching 类问题，因此在 review 任务内直接补齐实现与 fixture，而不是新增后置任务。
  - 验证结果：
    - `cargo fmt --all`：通过。
    - `cargo clippy --all-targets -- -D warnings`：通过。
    - `cargo build -p scoop -p scoopc`：通过，用于刷新 fixture runner 使用的 CLI binaries。
    - Targeted fixtures：`tests/fixtures/parse/f_string_interpolation.scoop`、`tests/fixtures/run-pass/fstring_desugar_basic.scoop`、`tests/fixtures/codegen/f_string_interpolation.scoop`、`tests/fixtures/typecheck/fstring_interpolation_non_tostring_is_error.scoop` 均通过。
    - `cargo test --all --all-targets`：通过。
    - `python3 tools/run_fixtures.py`：通过，输出 `fixtures: ok (1538)`。
  - 与 `PLAN.md` / 设计文档对应闭合：
    - 复核并闭合 `SPEC_FIX.md` B6 review 条件：`${...}` 是唯一 interpolation opener，literal `{}` JSON-friendly，`$x` shorthand 未启用，旧 `{expr}` 不再是 positive surface。
    - 未改变阶段边界、依赖结构或完成条件，因此无需更新 `PLAN.md`。

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
