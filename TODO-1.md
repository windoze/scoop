# TODO-1：P0-P1 基线冻结与 low-risk cleanup

> 索引：[`TODO.md`](./TODO.md)  
> 计划基线：[`PLAN.md`](./PLAN.md)  
> 覆盖阶段：P0-P1  
> 包目标：冻结旧 surface / overload bug 基线，关闭纯 spec drift，并删除 `@Inline` surface。

## P0：冻结当前偏离、建立迁移清单与最小回归矩阵

### [DONE] P0-T01：建立旧 surface / sysroot / fixture 迁移清单

- 参考：
  - [`PLAN.md`](./PLAN.md) §5 / P0
  - [`SPEC_FIX.md`](./SPEC_FIX.md) summary table
  - [`OVERLOAD_RESOLUTION.md`](./OVERLOAD_RESOLUTION.md) §1、§10
- 目标：
  - 在改语言行为前，固定所有旧 surface 命中、sysroot 依赖点、fixture 迁移点和 overload 已知 bug 样例。
  - 后续 P1-P6 agent 应能直接从本任务完成记录读取迁移清单，而不是重新全仓搜索。
- 必须检查的文件/位置：
  - `SCOOP_FULL_SPEC.md`
  - `docs/spec/language_spec-part*.md`
  - `sysroot/lib/scoop.core/src/core.scoop`
  - `sysroot/lib/scoop.unsafe/src/unsafe.scoop`
  - `tests/fixtures/**`
  - `crates/scoopc_ast/src/syntax/{token.rs,lexer.rs,string_literal.rs}`
  - `crates/scoopc_ast/src/parser/{expr.rs,decls.rs,stmt.rs,pattern.rs}`
  - `crates/scoopc_hir/src/typecheck/**`
  - `crates/scoopc_hir/src/hir/lower/expr/**`
  - `crates/scoopc_mir/src/mir/lower/**`
  - `crates/scoopc_cone/src/{scoopir/export.rs,visibility.rs}`
- 必须实现的内容：
  1. 在本条“完成记录”里列出旧 surface 命中摘要，至少覆盖：`perform`、`handle ... with`、tuple `._0` / `._1`、f-string `{...}` / `{{` / `}}`、`@Inline` / `annotation class Inline`、`AnyRef` / `AnyValue`、隐式 public sysroot/API declarations、operator-like functions lacking `operator`。
  2. 列出需要优先关注的 fixture 文件名或 glob，至少覆盖 `tests/fixtures/parse/*handle*`、`tests/fixtures/parse/*f_string*`、`tests/fixtures/parse/*with*`、`tests/fixtures/**/**/*perform*.scoop`、`tests/fixtures/**/**/*overload*.scoop`、`tests/fixtures/**/**/*vararg*.scoop`、`tests/fixtures/**/**/*inline*.scoop`、`tests/fixtures/**/**/*not_null*.scoop`、`tests/fixtures/**/**/*cast*.scoop`。
  3. 记录哪些旧语法 fixture 应机械迁移为新语法，哪些应保留为 negative fixture。
  4. 记录 overload 三个已知 bug 的最小代码样例：concrete overload `f(Int)` / `f(Bool)` lowering 不应串扰；arity overload `g(Int)` / `g(Int, Int)` 应各自 materialize callable version；generic + concrete 同名时 `h(Int)` 应按 specificity 胜出。
- 必须遵从的约束：
  - 本任务不改 compiler 语义，不新增会让全量 fixture 失败的 pass fixture。
  - 如果新增临时 inventory 文件，必须在本任务完成记录写明位置；否则直接把清单写入本条完成记录即可。
- 验证：
  1. `python3 tools/spec_fixtures.py check`
  2. `python3 tools/run_fixtures.py`
  3. 对完成记录中的 glob / 文件清单做人工抽样复核。
- 完成条件：
  - 后续任务能从本条完成记录直接知道旧 surface 和 fixture 迁移范围。
- 依赖：无
- 完成记录：
  - 改动范围：
    - 更新 `TODO.md` 索引与本条任务标题为 `[DONE]`。
    - 直接在本完成记录中写入迁移清单；未新增独立 inventory 文件，也未改 compiler / sysroot / fixture 行为。
    - 按用户要求同步更新 `memory/claude_plan.md` 作为本轮执行进度记录。
  - 旧 surface / sysroot / fixture 命中摘要：
    - `perform`：
      - 活跃 spec / split spec 仍有旧语法与说明：`SCOOP_FULL_SPEC.md`、`docs/spec/language_spec-part1.md`、`part2.md`、`part3.md`、`part4.md`。
      - Parser / token 仍接受旧 keyword：`crates/scoopc_ast/src/syntax/lexer.rs` 将 `"perform"` lex 成 `Keyword::Perform`；`crates/scoopc_ast/src/parser/expr.rs::try_parse_expr_prefix` 将 `perform E.op(...)` 当作 effect-op call 语法糖。
      - 后端与 effect pipeline 中有大量 “perform” 术语 / dump / fixture 命名，后续删除语法时需要区分内部 effect lowering 术语与用户可见旧语法。
      - fixture 重点：`tests/fixtures/**/**/*perform*.scoop`，尤其 `tests/fixtures/{hir,mir,mir_lowered,effect_facts,effect_lowered}/handle_perform.scoop` 与 `tests/fixtures/run-pass/effect_*perform*.scoop`。
    - Handler `with`：
      - `SCOOP_FULL_SPEC.md` 与 split spec 中仍有 `handle { ... } with { ... }` 示例。
      - Parser 仍固定要求 `with`：`crates/scoopc_ast/src/parser/expr.rs::parse_handle_expr` 在 handle body 后调用 `expect_keyword(Keyword::With)`。
      - Lexer 只有 `with` keyword；尚无 handler 目标 keyword `on`。
      - fixture 重点：`tests/fixtures/parse/handle_expr_minimal.scoop`、`handle_immediate_resume_removed.scoop`、`handle_expr_arm_recovery_two_errors.scoop`、`handle_arm_explicit_type_args_basic.scoop`。
      - 注意不要误删 value / enum `with` update：`tests/fixtures/parse/with_update_expr.scoop` 与后续 `with` update fixtures 不是 handler 旧语法。
    - Tuple `._0` / `._1`：
      - Spec 仍写 `t._0` / `t._1`：`SCOOP_FULL_SPEC.md`、`docs/spec/language_spec-part2.md`。
      - Parser member access 只接受 identifier，numeric member `t.0` 当前是 parse negative；typecheck / lowering / codegen 注释和逻辑仍按 `_0` / `_1` 识别。
      - fixture 重点：`tests/fixtures/run-pass/tuple_access_basic.scoop`、`tuple_access_print_sum.scoop`、`higher_order_aggregate_return_closure_tuple.scoop`、`with_update_tuple_nested_single_eval_basic.scoop`、`tests/fixtures/parse/tuple_access_numeric_member_not_allowed_fail.scoop`、`tests/fixtures/typecheck/with_update_tuple_nested_path_ok.scoop`、`with_update_tuple_overlapping_paths_is_error.scoop`。
    - f-string `{...}` / `{{` / `}}`：
      - Spec 仍使用单 `{expr}` 插值与 `{{` / `}}` literal brace escape：`SCOOP_FULL_SPEC.md`、`docs/spec/language_spec-part1.md`、`part3.md`、`part4.md`。
      - Parser `split_interpolated_string_parts` 以单 `{` 开始插值，并把 `{{` / `}}` 当作 literal brace escape；目标是仅 `${...}` 开始插值，普通 `{` / `}` 为文本。
      - fixture 重点：`tests/fixtures/parse/f_string_interpolation.scoop`、`tests/fixtures/codegen/f_string_interpolation.scoop`、`tests/fixtures/run-pass/fstring_desugar_basic.scoop`、`tests/fixtures/typecheck/fstring_interpolation_non_tostring_is_error.scoop`，以及多处 `runtime_gc` / `run_pass_cone` 中的 f-string 正例。
    - `@Inline` / `annotation class Inline`：
      - Spec 仍有 `@Inline` 专节、built-in annotation 表项和示例：`SCOOP_FULL_SPEC.md`、`docs/spec/language_spec-part1.md`、`part3.md`、`part5.md`。
      - Sysroot 仍定义 `annotation class Inline`：`sysroot/lib/scoop.core/src/core.scoop`。
      - Typecheck 仍识别 builtin annotation：`crates/scoopc_hir/src/typecheck/builtin_annotations.rs` 的 `BuiltinAnnotationKind::Inline` 与 `builtin_annotation_kind`，以及 `annotations.rs` 的 inline 专用检查。
      - fixture / golden 重点：`tests/fixtures/typecheck/inline_annotation_fun_ok.scoop`、`inline_annotation_invalid_target_is_error.scoop`、`return_in_inline_annotation_lambda_arg_is_error.scoop`、`tests/fixtures/parse/inline_modifier_removed.scoop`，以及 `tests/fixtures/effect_lowered/*.effectlowered` 中由 sysroot annotation class 产生的 `scoop.core.Inline` golden。
    - `AnyRef` / `AnyValue`：
      - Spec / sysroot 仍把二者建模为 sealed marker interface：`SCOOP_FULL_SPEC.md`、`sysroot/lib/scoop.core/src/core.scoop`。
      - Typecheck 仍有 marker 常量与 sealed-marker side table：`crates/scoopc_hir/src/typecheck/type_env.rs`、`where_clause.rs`、`interfaces.rs`。
      - Runtime / MIR / codegen 侧仍有 marker 依赖：`crates/scoopc_mir/src/rtti/type_desc.rs`、`crates/scoopc/src/pipeline/llvm_codegen_stage.rs`。
      - Sysroot unsafe atomic refs 仍以 `where T: AnyRef` 表达：`sysroot/lib/scoop.unsafe/src/unsafe.scoop`。
      - fixture 重点：`tests/fixtures/typecheck/sealed_interface_*`，尤其 marker 只能作 bound、禁止作参数 / 返回 / type argument / cast / supertype，以及 `sealed_interface_bounds_accept_ok.scoop`。
    - 隐式 public sysroot / API declarations：
      - `crates/scoopc_hir/src/resolve/mod.rs::visibility_from_modifiers` 当前默认 `Visibility::Public`。
      - `sysroot/lib/scoop.core/src/core.scoop` 与 `sysroot/lib/scoop.unsafe/src/unsafe.scoop` 大量 top-level API 没有显式 `public`，包括 builtins、`Any` / `Hashable` / scalar types、`String`、`Array` / `MutableArray`、annotations、GC / reflection structs、numeric structs、unsafe `Ptr` / `FunPtr` / atomic helpers。
      - Cone export / visibility 只导出 public：`crates/scoopc_cone/src/scoopir/export.rs`、`crates/scoopc_cone/src/visibility.rs`；默认改 internal 前必须先补 sysroot / fixture / cross-package API 的显式 `public`。
    - 缺少 `operator` modifier 的 operator-like declarations：
      - Sysroot numeric / char / bool / array / pointer API 大量使用 `plus`、`minus`、`times`、`div`、`rem`、`unaryPlus`、`unaryMinus`、`not`、`compareTo`、`get`、`set`，但 parser / AST 尚无 `operator` modifier surface。
      - 典型位置：`sysroot/lib/scoop.core/src/core.scoop` 的 `Bool.not`、`Char.plus/minus/compareTo`、`Float32/Float64` 和整数族 arithmetic / comparison、`Array.get` / `MutableArray.set`；`sysroot/lib/scoop.unsafe/src/unsafe.scoop` 的 `Ptr.plus/minus` 与 `FunPtr.invoke`。
      - fixture 重点：`tests/fixtures/run-pass/operator_overload_struct_basic.scoop`、`tests/fixtures/typecheck/operator_overload_plus_minus_ok.scoop`、`operator_overload_*_missing_is_error.scoop`。
  - 需要优先关注的 fixture 文件 / glob：
    - Handler parse：`tests/fixtures/parse/*handle*.scoop`（当前命中 `handle_expr_minimal.scoop`、`handle_immediate_resume_removed.scoop`、`handle_expr_arm_recovery_two_errors.scoop`、`handle_arm_explicit_type_args_basic.scoop`）。
    - f-string parse：`tests/fixtures/parse/*f_string*.scoop`（当前命中 `f_string_interpolation.scoop`）。
    - `with` parse：`tests/fixtures/parse/*with*.scoop`（当前命中 `with_update_expr.scoop`；保留为 value / enum `with` update 正例）。
    - `perform`：`tests/fixtures/**/**/*perform*.scoop`（handle pipeline、run-pass effect escape / resume / indirect perform 系列）。
    - overload：`tests/fixtures/**/**/*overload*.scoop`（`resolve/overload_*`、`typecheck/call_overload_*`、`infer/overload_resolution_*`、constructor / extension / operator overload 相关 fixtures）。
    - vararg：`tests/fixtures/**/**/*vararg*.scoop`（`vararg_call_ok.scoop`、spread bridge / missing bridge / non-array error fixtures）。
    - inline：`tests/fixtures/**/**/*inline*.scoop`（`inline_modifier_removed.scoop`、`inline_annotation_*`、inline lambda return fixtures）。
    - not-null：`tests/fixtures/**/**/*not_null*.scoop`（`not_null_assert_*`、`safe_call_not_null_assert.scoop`、`elvis_not_null.scoop`）。
    - cast：`tests/fixtures/**/**/*cast*.scoop`（`cast_as_*`、runtime typecheck cast, smart-cast, unsafe invalid-cast, sealed-marker cast negative fixtures）。
  - 旧语法 fixture 迁移 / negative 分类：
    - 机械迁移为新语法的正例：
      - `perform Effect.op(...)` 正例改为普通 qualified effect op call `Effect.op(...)`。
      - `handle { ... } with { ... }` 正例改为目标 handler keyword `handle { ... } on { ... }`。
      - tuple 读取 / nested path 正例从 `t._0` / `t._1` 与 field path `_0` / `_1` 迁移到 numeric segment `.0` / `.1`。
      - f-string 正例从 `f"{expr}"` 迁移到 `f"${expr}"`；旧 `{{` / `}}` escape 正例改成目标 literal-brace 表达方式。
      - operator overload 正例在 parser surface 可用后给参与 operator-positioned call 的 callable 加 `operator`。
      - `AnyRef` / `AnyValue` bound 正例迁移到 `ref` / `value` bound kind。
      - 依赖跨包可见性的 sysroot / fixture API 在默认 internal 前补显式 `public`。
    - 保留或新增为 negative 的旧 surface：
      - `perform` keyword 本身。
      - handler 专用 `with` keyword（不包括 value / enum `with` update）。
      - tuple `._0` / `._1` 旧 member access。
      - 旧 f-string `{expr}` 插值语义；后续应验证它不再求值为插值，或在需要表达旧插值意图时作为 negative / diagnostic fixture。
      - `@Inline` positive surface；旧 `inline` keyword removed diagnostic fixture 可保留，但不再提示改写为 active `@Inline` surface。
      - `AnyRef` / `AnyValue` 作为类型、supertype、type argument、cast target 等 marker-type 用法；P3 marker 删除后这些应保持前端 reject 或迁移为新的 bound-kind negative。
  - overload 三个已知 bug 的最小代码样例：
    - Concrete overload lowering 不应串扰：
      ```scoop
      package overload_concrete_bug

      import scoop.core.*

      fun f(x: Int): Int {
          return x + 1
      }

      fun f(x: Bool): Bool {
          return !x
      }

      fun main(): Int {
          return f(10)
      }
      ```
      当前基线：typecheck 放过，codegen / lowering 可能把同名 `f` 的参数类型串到 Bool 版本。目标：`f(Int)` 与 `f(Bool)` 独立 lower，main 返回 11。
    - Arity overload 应各自 materialize callable version：
      ```scoop
      package overload_arity_bug

      import scoop.core.*

      fun g(x: Int): Int {
          return x + 1
      }

      fun g(x: Int, y: Int): Int {
          return x + y
      }

      fun main(): Int {
          return g(10) + g(2, 3)
      }
      ```
      当前基线：可能报缺少 `overload_arity_bug.g` published callable version。目标：按 arity 分别绑定并 lower，main 返回 16。
    - Generic + concrete 同名时 concrete 按 specificity 胜出：
      ```scoop
      package overload_gvc_ok

      import scoop.core.*

      fun h<T>(x: T): T {
          return x
      }

      fun h(x: Int): Int {
          return x + 100
      }

      fun main(): Int {
          return h(10)
      }
      ```
      当前基线：typecheck 可能报 `scoop::typecheck::ambiguous_overload`。目标：`h(Int)` 比 `h<T>(T)` 更 specific，main 返回 110。
  - 验证结果：
    - `python3 tools/spec_fixtures.py check`：通过，输出 `spec fixtures: ok (1)`。
    - `python3 tools/run_fixtures.py`：通过，输出 `fixtures: ok (1533)`。
    - 对必查 glob / 文件清单做了 `rg` / `glob` 抽样复核，覆盖 spec、sysroot、fixtures、parser、typecheck、lowering、cone visibility 入口。
    - 完成验证后仅更新 `TODO.md` / `TODO-1.md` / `memory/claude_plan.md` 文档记录，未改编译输出；无需重新运行 suite。
  - 与 `PLAN.md` / 设计文档对应闭合：
    - 闭合 `PLAN.md` P0 第一项：冻结旧 surface、sysroot 默认 public 假设、fixture 迁移范围与 overload bug baseline。
    - 对应 `SPEC_FIX.md` 的 B4、C2、B2、B6、B1、C4、C5、A3 后续迁移入口。
    - 对应 `OVERLOAD_RESOLUTION.md` §1 三个当前偏离样例，供 P0-T02 / P5 继续固化为正式 regression。

### [DONE] P0-T01R：Review 旧 surface / sysroot / fixture 迁移清单

- 参考：
  - P0-T01 完成记录
  - [`PLAN.md`](./PLAN.md) §5 / P0
- 目标：
  - 独立复核 P0-T01 的 inventory 是否足以支撑后续任务，避免漏掉旧 surface 或关键 fixture。
- 必须检查的文件/位置：
  - P0-T01 完成记录中的所有文件和 glob
  - `TODO.md` 与 `TODO-1.md` 中 P0-T01 状态/完成记录
- 必须实现的内容：
  1. 抽样复查 P0-T01 列出的旧 surface 命中，确认分类准确。
  2. 反向检查至少一遍 spec、sysroot、fixtures、parser/typecheck/lowering 入口，确认没有明显漏项。
  3. 确认 P0-T01 没有引入会破坏全量 suite 的 positive fixture。
  4. 如发现漏项，直接补充清单或新增后续任务；若影响阶段边界，先更新 `PLAN.md`。
- 必须遵从的约束：
  - Review 不得只看文档格式；必须复核 inventory 可执行性。
  - 如果 P0-T01 未完成目标，阻塞 P0-T02。
- 验证：
  1. `python3 tools/spec_fixtures.py check`
  2. `python3 tools/run_fixtures.py`
- 完成条件：
  - 迁移清单完整、可执行，后续任务无需重新全仓搜索。
- 依赖：P0-T01
- 完成记录：
  - 改动范围：
    - 更新 `TODO.md` 索引与本条任务标题为 `[DONE]`，并填写 review 完成记录。
    - 未修改 compiler / sysroot / fixture / `PLAN.md`；本轮只补充 review 结论与 `memory/claude_plan.md` 进度记录。
  - 核心决策：
    - 独立抽样复查确认 P0-T01 对 `perform`、handler `with`、tuple `._0` / `._1`、f-string `{...}` / `{{` / `}}`、`@Inline`、`AnyRef` / `AnyValue`、隐式 public、缺少 `operator` 的 operator-like declarations 的分类准确，足以支撑后续迁移任务。
    - 反向检查覆盖 spec、split spec、sysroot、fixtures、parser/token、typecheck、HIR/MIR lowering、cone public export / visibility 入口；未发现需要新增 prerequisite task 的漏项。
    - Reviewed commit `b6cf70ca` 只修改 `TODO.md`、`TODO-1.md`、`memory/claude_plan.md`，没有新增 positive fixture 或 compiler behavior change，因此没有引入会破坏全量 suite 的 pass fixture。
    - Review 补充给后续迁移任务的执行提示：handler `with` 迁移不应只看 `tests/fixtures/parse/*handle*.scoop`，还应覆盖 `tests/fixtures/**/*handle*.scoop` 中的 run-pass / typecheck / HIR / MIR / effect goldens；tuple `.0` 迁移应使用内容搜索 `\._[0-9]+` 与 `with { _[0-9]... }`，因为不少命中不在 `*tuple*` 文件名下，也包括 Rust test snippet；f-string `${...}` 迁移同样应使用 `f"...{` / `{{` / `}}` 内容搜索覆盖 run-pass、runtime_gc、run_pass_cone 与 goldens。
    - `Keyword::On` / handler `on` 入口当前不存在，`parse_handle_expr` 仍固定 `expect_keyword(Keyword::With)`；P2-T02 需要同时新增 lexer/parser surface 并保留 value / enum `with` update。
  - 验证结果：
    - `cargo fmt --all --check`：通过。
    - `cargo clippy --all-targets -- -D warnings`：通过。
    - `python3 tools/spec_fixtures.py check`：通过，输出 `spec fixtures: ok (1)`。
    - `python3 tools/run_fixtures.py`：通过，输出 `fixtures: ok (1533)`。
    - 人工抽样 / 反向复查命令覆盖了 P0-T01 记录中的 required glob 与额外内容搜索：`perform`、handler `with` / `Keyword::With`、tuple `._N`、f-string brace interpolation、`@Inline`、`AnyRef` / `AnyValue`、visibility export、operator-like sysroot declarations。
  - 与 `PLAN.md` / 设计文档对应闭合：
    - 闭合 P0-T01 的独立 review gate；迁移清单可作为 P0-T02 与后续 P1-P6 的执行输入。
    - 未改变阶段边界、依赖结构或完成条件，因此无需更新 `PLAN.md`。

### [DONE] P0-T02：建立 overload bug 与 diagnostics 基线样例

- 参考：
  - [`PLAN.md`](./PLAN.md) §5 / P0
  - [`OVERLOAD_RESOLUTION.md`](./OVERLOAD_RESOLUTION.md) §1、§10
- 目标：
  - 在正式改 overload resolution 前，固定当前必须修复的样例和诊断要求。
  - 避免 P5 agent 再重新从设计文档复制测试程序。
- 必须检查的文件/位置：
  - `tests/fixtures/typecheck/**`
  - `tests/fixtures/run-pass/**`
  - `tests/fixtures/build/**`
  - `tools/run_fixtures.py::{PHASE_DIRS,run_one_file,parse_expectations}`
  - `tools/audit_user_visible_failure_policy.py::FRONTEND_REJECT_FORBIDDEN_TERMS`
- 必须实现的内容：
  1. 确认 fixture runner 是否已有 expected-fail / negative 机制足以承载“当前未修复但目标明确”的 overload 样例。
  2. 若 runner 可表达当前失败，新增或更新 targeted fixtures，样例内容直接来自 `OVERLOAD_RESOLUTION.md`：`overload_concrete_bug`、`overload_arity_bug`、`overload_gvc_ok`。
  3. 若 runner 不适合承载当前失败，不要加入会破坏全量 suite 的 fixture；把完整样例与预期结果写入本条完成记录，并在 P5-T04 正式加入 run-pass。
  4. 记录 overload diagnostics 的 audit 要求：`ambiguous_overload`、`no_applicable_overload`、`conflicting_overloads` 必须列候选位置，且不含 `backend` / `LLVM` / `UnsupportedMainBody` / `codegen`。
- 必须遵从的约束：
  - 不得通过删除或弱化现有 overload fixture 来让 P0 通过。
  - 不得在 P0 修 overload 算法；这里只做基线样例和测试落点确认。
- 验证：
  1. `python3 tools/run_fixtures.py`
  2. 如果新增 fixture，定向运行对应 fixture 所在 phase。
  3. 如果未新增 fixture，在完成记录中写明原因和 P5 应新增的具体文件名建议。
- 完成条件：
  - P5 agent 不需要重新整理 overload bug reproduction；样例、预期和落点已固定。
- 依赖：P0-T01R
- 完成记录：
  - 改动范围：
    - 新增 P0 baseline run-pass fixtures：
      - `tests/fixtures/run-pass/overload_concrete_bug.scoop`
      - `tests/fixtures/run-pass/overload_arity_bug.scoop`
      - `tests/fixtures/run-pass/overload_gvc_ok.scoop`
    - 每个 fixture 都保留 `OVERLOAD_RESOLUTION.md` §1 对应最小程序形状，并以 `EXPECT: pass` + 最终 `EXPECT-EXIT` 固定目标行为；同时加 `IGNORE-UNTIL-FIX: P5-T04 ...`，避免把当前 backend / codegen / typecheck 失败接受为最终期望。
    - 更新 `TODO-5.md` 的 P5-T04 要求：后续不再重新复制样例，而是移除上述三个 fixture 的 `IGNORE-UNTIL-FIX` 并使其通过。
  - 核心决策：
    - `tools/run_fixtures.py` 已支持 negative fixture 的 `EXPECT: fail`，也支持 current-failing target-pass fixture 的 `IGNORE-UNTIL-FIX`；P0-T02 采用后者承载三个目标应通过的 overload 样例。
    - 当前失败已定向复核：`overload_concrete_bug` 仍在后端前的 MIR/materialization 路径失败，`overload_arity_bug` 仍暴露缺少同名不同 arity callable version，`overload_gvc_ok` 仍报 `scoop::typecheck::ambiguous_overload`。这些都是 P5-T04 selected callable identity 贯通的输入，不在 P0 修算法。
    - Diagnostics audit baseline 固定为 `OVERLOAD_RESOLUTION.md` §10：`ambiguous_overload`、`no_applicable_overload`、`conflicting_overloads` 必须列候选 file/line/col 与原因，且用户可见文本不得包含 `backend` / `LLVM` / `UnsupportedMainBody` / `codegen`；现有 `FRONTEND_REJECT_FORBIDDEN_TERMS` 已覆盖 `backend`、`LLVM`、`codegen` 与 `Unsupported*`，P5-T05 已显式负责 overload-specific audit 扩展。
  - 验证结果：
    - `python3 tools/run_fixtures.py tests/fixtures/run-pass/overload_concrete_bug.scoop && python3 tools/run_fixtures.py tests/fixtures/run-pass/overload_arity_bug.scoop && python3 tools/run_fixtures.py tests/fixtures/run-pass/overload_gvc_ok.scoop`：通过，三个 fixture 均按 `IGNORE-UNTIL-FIX` skip。
    - `target/debug/scoopc check-source --phase parse --input ...`：三个新增 fixture 均通过 parse。
    - `target/debug/scoopc check-source --phase typecheck --input ...`：`overload_concrete_bug` 与 `overload_arity_bug` 通过 typecheck；`overload_gvc_ok` 的当前 `ambiguous_overload` 失败由定向 `scoop run` 复核确认。
    - `cargo fmt --all --check`：通过。
    - `cargo clippy --all-targets -- -D warnings`：通过。
    - `cargo test --all --all-targets`：通过。
    - `python3 tools/run_fixtures.py`：通过，输出 `fixtures: ok (1536)`。
  - 与 `PLAN.md` / 设计文档对应闭合：
    - 闭合 `PLAN.md` P0 对最小 overload regression / bug baseline 的要求，三个 §1 已知偏离样例已有固定 fixture 落点。
    - 对应 `OVERLOAD_RESOLUTION.md` §1 的 concrete、arity、generic+concrete bug baseline，以及 §10 的 overload diagnostics audit 要求；未改变阶段边界，因此无需更新 `PLAN.md`。

### [DONE] P0-T02R：Review overload bug 与 diagnostics 基线

- 参考：
  - P0-T02 完成记录
  - [`OVERLOAD_RESOLUTION.md`](./OVERLOAD_RESOLUTION.md) §1、§10
- 目标：
  - 复核 overload baseline 是否准确覆盖已知 codegen bug 和诊断要求。
- 必须检查的文件/位置：
  - P0-T02 新增或记录的 fixture / 样例
  - `tests/fixtures/**/**/*overload*.scoop`
  - `tools/run_fixtures.py`
  - `tools/audit_user_visible_failure_policy.py`
- 必须实现的内容：
  1. 确认三个已知 bug 样例完整、预期明确、不会破坏当前全量 suite。
  2. 确认 P5-T04 可以直接使用 P0-T02 的样例作为 run-pass regression。
  3. 确认 diagnostics audit 要求足够明确，包含候选位置和 forbidden terms。
  4. 如基线样例无法执行，记录阻塞原因和替代落点。
- 必须遵从的约束：
  - 不得把当前 backend/codegen 报错接受为最终期望。
- 验证：
  1. `python3 tools/run_fixtures.py`
  2. 定向运行或人工复核 P0-T02 记录的样例。
- 完成条件：
  - overload bug baseline 可直接驱动 P5 实现和回归。
- 依赖：P0-T02
- 完成记录：
  - 改动范围：
    - 更新 `TODO.md` 索引与本条任务标题为 `[DONE]`，并填写 review 完成记录。
    - 未修改 compiler / sysroot / fixture / `PLAN.md`；本轮只补充 review 结论与 `memory/claude_plan.md` 进度记录。
  - 核心决策：
    - 三个 P0-T02 baseline fixture 均保留 `OVERLOAD_RESOLUTION.md` §1 的最小程序形状，并用 `EXPECT: pass` + `EXPECT-EXIT` 固定最终目标行为：`overload_concrete_bug` 返回 11、`overload_arity_bug` 返回 16、`overload_gvc_ok` 返回 110。
    - `IGNORE-UNTIL-FIX: P5-T04 ...` 由 fixture runner 在执行前直接 skip，只用于承载当前未修复但目标应通过的 run-pass regression；没有把当前 backend/codegen/typecheck 失败接受为最终期望。
    - `TODO-5.md` 的 P5-T04 已明确要求移除三个 baseline fixture 的 `IGNORE-UNTIL-FIX` 并让它们通过，因此 P5 可直接使用 P0-T02 样例作为 selected callable identity regression。
    - Diagnostics baseline 与 `OVERLOAD_RESOLUTION.md` §10 对齐：overload reject 需要列候选 file/line/col、完整 signature、不可用或不可比原因，且不得泄露 `backend`、`LLVM`、`UnsupportedMainBody`、`codegen` 等内部术语；P5-T05 已把这些要求写成后续实现与 audit 任务。
    - 当前实现与既有 fixtures 仍使用 `no_matching_overload` / `overload_conflict` 等旧 diagnostic code 名称；这属于 P5-T05 已排期的 overload diagnostics audit 范围，不阻塞 P0-T02R。
  - 验证结果：
    - `cargo fmt --all --check`：通过。
    - `cargo clippy --all-targets -- -D warnings`：通过。
    - `python3 tools/run_fixtures.py tests/fixtures/run-pass/overload_concrete_bug.scoop`：通过，fixture 按 `IGNORE-UNTIL-FIX` skip。
    - `python3 tools/run_fixtures.py tests/fixtures/run-pass/overload_arity_bug.scoop`：通过，fixture 按 `IGNORE-UNTIL-FIX` skip。
    - `python3 tools/run_fixtures.py tests/fixtures/run-pass/overload_gvc_ok.scoop`：通过，fixture 按 `IGNORE-UNTIL-FIX` skip。
    - `target/debug/scoopc check-source --phase parse --input ...`：三个 baseline fixture 均通过 parse。
    - `python3 tools/run_fixtures.py`：通过，输出 `fixtures: ok (1536)`。
  - 与 `PLAN.md` / 设计文档对应闭合：
    - 闭合 P0-T02 的独立 review gate；overload bug baseline 可作为 P5 selected callable identity 修复和 diagnostics audit 的直接输入。
    - 未改变阶段边界、依赖结构或完成条件，因此无需更新 `PLAN.md`。

## P1：纯 spec / low-risk cleanup 与 `@Inline` 删除

### [DONE] P1-T01：更新纯 spec 决议：`Nothing`、cone/package、value type `with`

- 参考：
  - [`PLAN.md`](./PLAN.md) §5 / P1
  - [`SPEC_FIX.md`](./SPEC_FIX.md) A1、A2、D1
- 目标：
  - 先关闭无需 compiler 语义改动的 spec drift。
- 必须修改的文件/位置：
  - `SCOOP_FULL_SPEC.md`
  - `docs/spec/language_spec-part*.md`
- 必须实现的内容：
  1. 在 type hierarchy 章节写入 `Nothing`：bottom type、subtype of every type、no inhabitants、only for non-returning functions、outside reference/value split。
  2. 在 cone/package 章节开头明确 cone 是 distribution / build unit，`.cone` 是 binary archive format，package 是 cone 内 source-level namespace。
  3. 在 value type / `with` 相关章节确认 value type immutable、struct 不引入 `var` field、`with` 保留为 update mechanism，C1 只会改 enum mismatch failure mode。
  4. 如 split spec 是手工维护，同步改 `docs/spec/language_spec-part*.md`；如有生成流程，在完成记录中写明实际同步方式。
- 必须遵从的约束：
  - 本任务不改 compiler 行为。
  - 不得在本任务顺手改 `perform`、handler `with`、f-string 等会影响 fixtures 的 spec code blocks；这些留给 P2/P6。
- 验证：
  1. `python3 tools/spec_fixtures.py check`
  2. 人工复核 `SCOOP_FULL_SPEC.md` 中 `Nothing`、cone/package、value type `with` 的新表述。
- 完成条件：
  - A1、A2、D1 在活跃 spec 中闭合。
- 依赖：P0-T02R
- 完成记录：
  - 改动范围：
    - 更新 `SCOOP_FULL_SPEC.md` §2.1、§2.3.1、§2.6、§13.1/§13.2 的纯 spec prose，写入 `Nothing`、cone/package 分层和 value type `with` 保留决议。
    - 手工同步 split spec：`docs/spec/language_spec-part1.md` 的 Cone / `package` 语言边界，和 `docs/spec/language_spec-part2.md` 的 `Nothing`、struct `val` 字段、`with` copy-update 表述。
    - 按用户要求同步更新 `memory/claude_plan.md` 作为本轮执行进度记录。
    - 未修改 compiler、sysroot、fixtures、fixture-bearing spec code block 或 `PLAN.md`。
  - 核心决策：
    - `Nothing` 已明确为 bottom / uninhabited type：是所有类型的子类型、无运行期值或表示、仅用于永不正常返回的表达式或函数，并且不属于 reference / value split。
    - Cone 已明确为 distribution / build unit 与依赖、可见性、产物归档边界；`.cone` 是二进制归档格式；source-level `package` 是 Cone 内 namespace，源文件仍使用 `package foo.bar`。
    - Value type 决议保持 immutability：struct 不引入 `var` 字段或 mutating method，`with` 保留为构造修改副本的更新机制；未提前修改 enum mismatched-variant failure mode。
    - Split spec 当前为手工维护，本任务通过直接编辑对应分卷同步，没有生成流程需要执行。
  - 验证结果：
    - `python3 tools/spec_fixtures.py check`：通过，输出 `spec fixtures: ok (1)`。
    - `git diff --check`：通过，无 whitespace error。
    - 人工复核 `SCOOP_FULL_SPEC.md` 与 `docs/spec/language_spec-part1.md` / `language_spec-part2.md` 中 `Nothing`、cone/package、value type `with` 的新表述；未改动 `perform`、handler `with`、f-string 等后续任务负责的 fixture-affecting code block。
    - 本任务仅修改 markdown/progress 文档，未改编译输出；`cargo fmt`、`cargo clippy`、`cargo test` 和全量 fixture suite 按任务验证要求跳过。
  - 与 `PLAN.md` / 设计文档对应闭合：
    - 闭合 `SPEC_FIX.md` A1、A2、D1。
    - 闭合 `PLAN.md` P1 中纯 spec 部分：§2.1 `Nothing`、§13 cone/package 层级、§2.6 value type immutability + `with` 保留。
    - 未改变阶段边界、依赖结构或完成条件，因此无需更新 `PLAN.md`。

### [DONE] P1-T01R：Review 纯 spec 决议更新

- 参考：
  - P1-T01 完成记录
  - [`SPEC_FIX.md`](./SPEC_FIX.md) A1、A2、D1
- 目标：
  - 复核 P1-T01 的 spec-only 更新是否准确、无无关 churn。
- 必须检查的文件/位置：
  - `SCOOP_FULL_SPEC.md`
  - `docs/spec/language_spec-part*.md`
  - P1-T01 diff
- 必须实现的内容：
  1. 确认 `Nothing` 描述符合 bottom type 决议。
  2. 确认 cone/package wording 没有与现有 source `package` 混淆。
  3. 确认 value type `with` 记录没有提前改变 C1 panic 语义或引入 struct `var`。
  4. 确认未修改 P2/P3 才应处理的 code blocks。
- 必须遵从的约束：
  - Review 发现 spec 决议偏离时必须先修正，再进入 P1-T02。
- 验证：
  1. `python3 tools/spec_fixtures.py check`
- 完成条件：
  - P1-T01 spec 更新准确且不影响 compiler/fixture baseline。
- 依赖：P1-T01
- 完成记录：
  - 改动范围：
    - 更新 `TODO.md` 索引与本条任务标题为 `[DONE]`，并填写 review 完成记录。
    - 复核 P1-T01 commit `697ba21e` 对 `SCOOP_FULL_SPEC.md`、`docs/spec/language_spec-part1.md`、`docs/spec/language_spec-part2.md` 的 spec-only diff。
    - 未修改 compiler、sysroot、fixtures、fixture-bearing spec code block 或 `PLAN.md`；本轮只补充 review 结论与 `memory/claude_plan.md` 进度记录。
  - 核心决策：
    - `Nothing` 的新表述符合 A1：bottom / uninhabited、是所有类型子类型、无运行期值或表示、仅用于永不正常返回的表达式或函数，并且明确位于 reference / value split 之外。
    - Cone/package wording 符合 A2：cone 是 distribution / build unit 与 `.cone` 归档边界；source-level `package` 是 cone 内 namespace，源文件仍使用 `package foo.bar`，没有把二者混为同一概念。
    - Value type `with` 表述符合 D1：value type 保持 immutable，struct 直接字段仍必须是 `val`，不引入 struct `var` 或 mutating method；P1-T01 没有提前改变 C1 enum mismatched-variant panic 语义。
    - P1-T01 diff 只改 spec prose 和 cone archive metadata label，没有迁移 `perform`、handler `with`、f-string、tuple access、`@Inline` 或其他 P2/P3 才应处理的 language-surface code blocks。
  - 验证结果：
    - `python3 tools/spec_fixtures.py check`：通过，输出 `spec fixtures: ok (1)`。
    - `git diff --check`：通过，无 whitespace error。
    - 本 review 只修改 markdown/progress 文档，未改编译输出；无需运行 `cargo fmt`、`cargo clippy`、`cargo test` 或全量 fixture suite。
  - 与 `PLAN.md` / 设计文档对应闭合：
    - 闭合 P1-T01 的独立 review gate；`SPEC_FIX.md` A1、A2、D1 在活跃 spec 与 split spec 中的表述准确，且不影响 compiler / fixture baseline。
    - 未改变阶段边界、依赖结构或完成条件，因此无需更新 `PLAN.md`。

### [DONE] P1-T02：删除 `@Inline` annotation surface

- 参考：
  - [`PLAN.md`](./PLAN.md) §5 / P1
  - [`SPEC_FIX.md`](./SPEC_FIX.md) B1
- 目标：
  - 删除无语义保证的 `@Inline`，避免后续 spec / sysroot / typecheck 继续维护 dead surface。
- 必须修改的文件/位置：
  - `SCOOP_FULL_SPEC.md`
  - `docs/spec/language_spec-part*.md`
  - `sysroot/lib/scoop.core/src/core.scoop`
  - `crates/scoopc_hir/src/typecheck/builtin_annotations.rs::{BuiltinAnnotationKind,builtin_annotation_kind}`
  - `crates/scoopc_hir/src/typecheck/annotations.rs::{check_inline_annotation_uses,check_builtin_inline_annotation}`
  - `crates/scoopc_ast/src/parser/decls.rs::parse_decl_prefix`
  - `tests/fixtures/**/**/*inline*.scoop`
- 必须实现的内容：
  1. 从 spec 的 annotation 章节和 built-in annotation 表删除 `@Inline`。
  2. 从 sysroot 删除 `annotation class Inline`。
  3. 从 `BuiltinAnnotationKind` 和 `builtin_annotation_kind` 删除 `Inline` 分支。
  4. 删除或改写 `check_inline_annotation_uses` / `check_builtin_inline_annotation` 的专用逻辑；如果函数只为 Inline 存在，应删除调用点。
  5. 更新 `@Inline` 正例 fixtures 为 negative fixture 或删除过时用例；保留 `inline` keyword removed diagnostic 的 parser/typecheck 覆盖。
- 必须遵从的约束：
  - 不得把 `inline` 改成新的 optimization hint。
  - 不得影响 compiler 自身 inliner heuristic。
- 验证：
  1. `python3 tools/spec_fixtures.py sync`
  2. `python3 tools/spec_fixtures.py check`
  3. `python3 tools/run_fixtures.py`
  4. targeted search：活跃 spec、sysroot、typecheck 不再出现 `annotation class Inline` / `BuiltinAnnotationKind::Inline`。
- 完成条件：
  - `@Inline` 不再是语言 surface；只有旧 keyword negative diagnostic 可保留。
- 依赖：P1-T01R
- 完成记录：
  - 改动范围：
    - 更新 `TODO.md` 索引与本条任务标题为 `[DONE]`，并填写完成记录。
    - 从 `SCOOP_FULL_SPEC.md` 和 split spec 删除 `@Inline` 函数章节、annotation overview 示例与 built-in annotation 表项；保留 lowercase `inline` 作为 removed keyword diagnostic，而不是可替代的 annotation hint。
    - 从 `sysroot/lib/scoop.core/src/core.scoop` 删除 `annotation class Inline`，并同步更新 HIR / effect-lowered goldens 中由该 sysroot class 产生的 nominal / vtable / class init / layout count。
    - 从 `BuiltinAnnotationKind` / `builtin_annotation_kind` 删除 `Inline` 分支；删除 `check_builtin_inline_annotation`，并将局部 / 表达式注解检查入口从 `check_inline_annotation_uses` 改写为 `check_local_annotation_uses`。
    - 更新 parser removed-keyword help，不再建议 `inline fun ...` 改写为 `@Inline fun ...`。
    - 删除过时 `@Inline` positive / target-specific fixtures，新增 `tests/fixtures/typecheck/inline_annotation_removed_is_error.scoop` 验证裸 `@Inline` 不再解析为内建或 sysroot annotation。
  - 核心决策：
    - `@Inline` 不再是语言、sysroot 或 typecheck 内建 surface；compiler inliner 仍保留在 MIR / LIR 优化路径中，未绑定到用户可见 annotation。
    - 旧 lowercase `inline` keyword 继续在 parser 中保留为前端错误，以保证旧 surface 有稳定 diagnostic，但 diagnostic help 只要求删除修饰符。
    - `@Inline` fixture 选择保留一个 negative typecheck 用例，避免无声回归为 active builtin annotation；其余旧 positive / non-local-return 相关用例删除，non-local return 行为继续由 `return_in_non_inline_lambda_arg_is_error.scoop` 覆盖。
  - 验证结果：
    - `cargo fmt --all`：通过。
    - `python3 tools/spec_fixtures.py sync`：通过，输出 `spec fixtures: ok (1)`。
    - `python3 tools/spec_fixtures.py check`：通过，输出 `spec fixtures: ok (1)`。
    - `cargo clippy --all-targets -- -D warnings`：通过。
    - `cargo test --all --all-targets`：通过。
    - `python3 tools/run_fixtures.py`：通过，输出 `fixtures: ok (1534)`。
    - targeted search：`SCOOP_FULL_SPEC.md`、`docs/spec/`、`sysroot/lib/scoop.core/src/core.scoop`、`crates/scoopc_hir/src/typecheck/` 与 active fixtures 中不再出现 `annotation class Inline`、`BuiltinAnnotationKind::Inline`、`check_inline_annotation_uses`、`check_builtin_inline_annotation` 或 `scoop.core.Inline`。
  - 与 `PLAN.md` / 设计文档对应闭合：
    - 闭合 `SPEC_FIX.md` B1：删除 `@Inline`，由 compiler 自主 inliner heuristic 决定内联。
    - 闭合 `PLAN.md` P1 中 `@Inline` 删除 gate；未改变阶段边界、依赖结构或后续任务完成条件，因此无需更新 `PLAN.md`。

### [DONE] P1-T02R：Review `@Inline` 删除结果

- 参考：
  - P1-T02 完成记录
  - [`SPEC_FIX.md`](./SPEC_FIX.md) B1
- 目标：
  - 确认 `@Inline` 已从 language surface、sysroot、typecheck、fixtures 和 spec 中真正删除。
- 必须检查的文件/位置：
  - `SCOOP_FULL_SPEC.md`
  - `docs/spec/language_spec-part*.md`
  - `sysroot/lib/scoop.core/src/core.scoop`
  - `crates/scoopc_hir/src/typecheck/builtin_annotations.rs`
  - `crates/scoopc_hir/src/typecheck/annotations.rs`
  - `tests/fixtures/**/**/*inline*.scoop`
- 必须实现的内容：
  1. 搜索并分类所有 `Inline` / `@Inline` / `annotation class Inline` 命中。
  2. 确认 `inline` keyword removed diagnostic 仍然覆盖旧 keyword，但不再表示 annotation surface。
  3. 确认 spec fixture sync 后无 stale generated fixture。
  4. 确认 inliner heuristic 或 optimization pass 没有被误删。
- 必须遵从的约束：
  - 不得以保留 hidden annotation alias 的方式通过 review。
- 验证：
  1. `python3 tools/spec_fixtures.py check`
  2. `python3 tools/run_fixtures.py`
- 完成条件：
  - B1 完整闭合，进入 P2 前无 `@Inline` active positive surface。
- 依赖：P1-T02
- 完成记录：
  - 改动范围：
    - 执行 P1-T02R review audit，覆盖 active spec、split spec、sysroot、typecheck builtin annotation 识别、annotation checker、parser removed-keyword diagnostic、inline 相关 fixtures 与优化 pass 名称。
    - 修正 `tests/fixtures/umb_fix/B-04-function-signature/{neg_function_signature_missing_type,pos_function_signature_return}.scoop` 与 `tests/fixtures/umb_fix/_index.csv` 中 stale `#51-inline` / `#52-non-local-return` metadata，改为当前 spec 的 `#51-non-local-return` anchor。
    - 更新 `TODO.md` 索引与本条任务标题为 `[DONE]`，并填写完成记录。
  - 核心决策：
    - `@Inline` 不再是 active language / sysroot / typecheck positive surface；仓库仅保留 `inline_annotation_removed_is_error.scoop` 作为 unresolved annotation negative coverage。
    - lowercase `inline` 仍作为 parser removed-keyword diagnostic 保留，`inline_modifier_removed.scoop` 验证旧 keyword 会在前端报错，且不会重新指向 `@Inline` annotation。
    - compiler 自主优化路径仍存在：`SummaryDrivenInlining` MIR pass 与 `HigherOrderWrapperInlineDevirt` LIR pass 未被 P1-T02 删除，也不依赖用户可见 `@Inline`。
  - 验证结果：
    - targeted search：active spec / split spec / sysroot / typecheck / fixtures 中不再出现 `annotation class Inline`、`BuiltinAnnotationKind::Inline`、`check_inline_annotation_uses`、`check_builtin_inline_annotation` 或 `scoop.core.Inline`。
    - targeted search：active fixtures 中不再出现 stale `#51-inline` / `#52-non-local-return` spec anchors。
    - `cargo fmt --all`：通过。
    - `cargo clippy --all-targets -- -D warnings`：通过。
    - `python3 tools/spec_fixtures.py check`：通过，输出 `spec fixtures: ok (1)`。
    - `python3 tools/run_fixtures.py`：通过，输出 `fixtures: ok (1534)`。
  - 与 `PLAN.md` / 设计文档对应闭合：
    - 闭合 `SPEC_FIX.md` B1 的独立 review gate：`@Inline` 删除在 spec、sysroot、typecheck、fixtures 与 active metadata 中完整闭合，进入 P2 前无 `@Inline` active positive surface。
    - 本次仅修正 review 发现的 fixture metadata 与任务记录，未改变 phase/stage 边界、依赖结构或完成条件，因此无需更新 `PLAN.md`。
