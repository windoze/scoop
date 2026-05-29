# Scoop：Spec Fix + Overload Resolution 落地计划

> 生成时间：2026-05-27  
> 设计基线：[`SPEC_FIX.md`](./SPEC_FIX.md)、[`OVERLOAD_RESOLUTION.md`](./OVERLOAD_RESOLUTION.md)  
> 格式参考：[`docs/archive/plans/PLAN-managed-abi.md`](./docs/archive/plans/PLAN-managed-abi.md)  
> 当前状态：两份设计文档仍是 pending / design-only；`SCOOP_FULL_SPEC.md`、sysroot、compiler 与 fixtures 仍混有旧 surface，例如 `perform`、`handle ... with`、tuple `._0`、f-string `{...}`、`AnyRef` / `AnyValue` sealed marker、默认 public visibility，以及不完整 overload resolution。  
> 行号说明：下文以当前文件路径和符号名为准；后续若行号漂移，优先按文件路径、符号名和 fixture 名定位。

## 0. 工作原则

- [`SPEC_FIX.md`](./SPEC_FIX.md) 与 [`OVERLOAD_RESOLUTION.md`](./OVERLOAD_RESOLUTION.md) 是本轮设计基线。若实现中发现必须改变其中任何语言决议，必须先回写对应设计文档，再继续改代码。
- 当前活跃计划文档是根目录 [`PLAN.md`](./PLAN.md)；`docs/archive/plans/**` 仅作历史和格式参考，不再回写。
- 本轮不是“补几个特判让当前样例过”，而是把语法、spec、typecheck、lowering、call binding、codegen materialization 和 fixtures 收口到同一套可审计 contract。
- 不做无根据的兼容层。旧 `perform`、旧 handler `with`、tuple `._0`、旧 f-string `{...}` 插值等 surface 在切换阶段完成后应成为前端错误，而不是长期 soft alias。
- 所有用户可见 reject 必须发生在 parser/typecheck 侧；overload 相关错误不得泄露 `backend`、`LLVM`、`UnsupportedMainBody` 等内部术语。
- overload resolution 的 source of truth 必须是 typecheck 选出的唯一 callable binding / overload identity；lowering 与 codegen 不得再用 bare FQN 或同名函数 map 猜测目标。
- `!!`、`as`、refutable `val` pattern、enum `with` variant mismatch 这类“断言失败”统一走 `panic(...)`，不得给公开函数签名注入 `Raise<RuntimeError>`。
- spec 与实现必须一起闭环：`SCOOP_FULL_SPEC.md`、`docs/spec/language_spec-part*.md`、sysroot、fixtures、`tools/spec_fixtures.py check` 和 `tools/run_fixtures.py` 都是验收面。

## 1. 当前判断

- `SPEC_FIX.md` 的 13 项调整尚未应用到 `SCOOP_FULL_SPEC.md` 或 compiler；其中 A1/A2/D1 是纯 spec，A3/B/C 项需要 parser、typecheck、lowering、sysroot、cone exporter 或 fixtures 配套。
- `OVERLOAD_RESOLUTION.md` 已定义目标规则，但当前 overload 仍存在 concrete overload lowering 串扰、arity overload callable version 缺失、generic + concrete 同名报歧义等偏离。
- parser/lexer 当前已识别 `perform`、`handle`、`with`、`inline` 等旧 token；还没有 `on` handler keyword、`operator` modifier、tuple `.0` member access、`${...}` f-string 插值、`<T: ref>` / `<T: value>` bound surface。
- typecheck 当前默认 visibility 是 `public`；sysroot API 大量依赖隐式 public。改成默认 `internal` 前，必须先给导出 surface 补显式 `public`。
- `AnyRef` / `AnyValue` 目前通过 sysroot sealed marker 和 typecheck sealed-marker side table 表达；目标是替换成 bound constraint kind，不再作为类型出现。
- `!!` 和 runtime `as` failure 当前仍会记录 / lower 到 `Raise<RuntimeError>`；目标是 panic 路径，且 effect graph 不再记录这条边。
- `@Inline` 已在 parser 层对 `inline` modifier 有移除诊断，但 sysroot annotation 与 builtin annotation 识别仍存在，需要彻底删除。
- operator expression 当前按方法名如 `plus` / `compareTo` 参与 resolution，尚未检查 `operator` modifier。
- overload definition-time 检查已有一部分重复签名、返回类型冲突、默认参数歧义逻辑，但还缺 generic bound shape、vararg overlap、virtual generic method、完整 override 边界与高质量候选诊断。

## 2. Gap 覆盖矩阵

| Gap | 当前状态 | 本轮动作 | 归属阶段 |
|---|---|---|---|
| A1 `Nothing` type hierarchy | spec 使用但 lattice 未列 | 写入 spec，明确 bottom type / no inhabitants / outside ref-value split | P1 |
| A2 Cone vs package wording | spec 两个名字混用 | 重写 §13 开头，明确 cone=distribution/build unit，package=source namespace | P1 |
| D1 value type `with` 路线 | 仅设计记录 | 写入 spec，确认 value type immutable + `with` 保留 | P1 |
| B1 删除 `@Inline` | sysroot/typecheck 仍识别 annotation | 删除 sysroot annotation、builtin annotation 识别、fixtures 与 spec 表述 | P1 |
| B4 删除 `perform` | parser 仍接受 prefix keyword | 机械改写 effect op 调用为普通 qualified call，旧 keyword 变 parse/typecheck error | P2 |
| C2 handler `with` -> `on` | parser 固定 `handle ... with` | 新增 handler `on` surface，旧 handler `with` 报清晰错误 | P2 |
| B2 tuple field `._0` -> `.0` | parser/typecheck 按 ident `_0` | lexer/parser/typecheck/lowering 改为 numeric member segment | P2 |
| B6 f-string `{...}` -> `${...}` | split 逻辑按单 `{` 插值 | `${` 才开启插值，`{` / `}` 变 literal，无 `$x` shorthand | P2 |
| A3 `operator` required | AST 无 modifier，operator 按名字捕获 | 新增 modifier 并在 operator-positioned calls 筛选 | P2-P3 |
| B3 `!!` / `as` failure panic | typecheck/lowering 记录 `Raise<RuntimeError>` | failure arm 改为 `panic(...)`，effect graph 不再记录 Raise edge | P3 |
| C1 enum `with` mismatch panic | mismatch arm 保留原值 | SLIR/HIR/MIR lowering mismatch arm 改为 panic | P3 |
| C3 refutable `val` pattern | typecheck reject variant pattern | 允许 refutable pattern，mismatch fallback panic | P3 |
| B5 closure `var` capture forbidden | closure capture 可记录 mutable capture | typecheck 捕获外层 `var` 时诊断并提示 `RefCell` / snapshot | P3 |
| C4 sealed marker -> `ref` / `value` bound | `AnyRef` / `AnyValue` 是 sysroot sealed marker | parser/typecheck 引入 bound constraint kind，移除 marker 类型 | P2-P3 |
| C5 default visibility internal | `visibility_from_modifiers` 默认 public | 默认改 internal，sysroot/exported API 显式 public，cone API 只导出 public | P3 |
| overload definition-time rules | 仅部分重复签名检查 | effective signature、generic shape、vararg overlap、virtual generic、override 边界 | P4 |
| overload call-site rules | specificity / binding 不完整 | 五阶段 resolution、effective type specificity、diagnostics、effect-after-resolution | P5 |
| overloaded callable codegen | bare FQN / symbol disambiguation 不稳 | materialization/codegen 使用 typechecked overload identity，修复 concrete/arity bug | P5 |

## 3. 代码入口总表

| 主题 | 入口文件 / 符号 | 当前问题 | 目标状态 |
|---|---|---|---|
| lexer / token | `crates/scoopc_ast/src/syntax/token.rs`、`lexer.rs::lex_ident_or_keyword`、`lexer.rs::lex_number_literal`、`string_literal.rs` | 仍识别旧 keyword / f-string；`.` 后 number 可能吞成 float | 支持 handler `on`、`operator`、numeric member segment、`${...}` f-string；旧 surface 明确拒绝 |
| expression parser | `crates/scoopc_ast/src/parser/expr.rs::{try_parse_expr_prefix,parse_handle_expr,parse_member_access_expr,parse_field_path,split_interpolated_string_parts}` | `perform` prefix、handler `with`、member ident `_0`、单 `{` 插值 | 目标语法唯一入口，旧语法给定位诊断 |
| decl / generic parser | `crates/scoopc_ast/src/parser/decls.rs::{parse_decl_prefix,parse_type_param_list,parse_where_clause_opt}`、`ast/mod.rs::Modifier` | 无 `operator` modifier；inline bounds / `ref` / `value` bound 不完整 | modifier 与 bound surface 进入 AST，`ref`/`value` 仅可在 bound position |
| resolve / visibility | `crates/scoopc_hir/src/resolve/mod.rs::{ModifierSet,Visibility,visibility_from_modifiers,FunSig,FunOverload,ConstructorOverload}` | 默认 public；overload identity 与 visibility 规则未完整承载 | 默认 internal；callable signature / overload identity 可被 typecheck/lowering 复用 |
| overload definition check | `crates/scoopc_hir/src/typecheck/overloads.rs`、`headers.rs`、`inheritance.rs`、`interfaces.rs` | 重复签名检查有限，generic/vararg/virtual method 规则缺失 | definition-time reject 全部前端化，并输出候选位置 |
| call resolution | `crates/scoopc_hir/src/typecheck/expr/call/{dispatch.rs,member_call.rs,ctor.rs,args.rs,generic.rs,value_call.rs}` | specificity 与 binding 不符合目标规则 | 实现 local/member/extension/top-level/imported 层叠、visibility、applicability、specificity、ambiguity |
| operator / effect op | `typecheck/expr/ops.rs`、`typecheck/expr/call/effect_op.rs` | operator 按名字捕获；effect op 仍可从 `perform` 语法进入 | operator 需 modifier；effect op 是普通 qualified call 的 resolution 结果 |
| pattern / closure capture | `typecheck/val_pat.rs`、`hir/lower/util/closures.rs::compute_closure_captures` | refutable val reject；captured var 未诊断 | refutable val mismatch panic；captured var 前端错误 |
| cast / not-null / with lowering | `hir/lower/expr/{main_lower.rs,members.rs,canonical_call.rs}`、`scoopc_mir/src/mir/lower/fn_lowering_expr.rs` | `!!` / `as` / enum `with` failure 仍走 Raise 或 preserve original | failure arm 统一生成 panic，不污染 effect row |
| MIR / materialization / codegen | `scoopc_mir/src/mir/materialize/hir_calls.rs`、`scoopc_mir/src/mir/callables.rs`、`scoopc_codegen_llvm/src/llvm/codegen/mir_body/{call.rs,cast.rs}` | overloaded callable 可能按同名 FQN 混淆 | selected overload identity 贯穿 materialization 和 LLVM declaration/call |
| sysroot / cone | `sysroot/lib/scoop.core/src/core.scoop`、`sysroot/lib/scoop.unsafe/src/unsafe.scoop`、`crates/scoopc_cone/src/scoopir/export.rs`、`crates/scoopc_cone/src/visibility.rs` | `AnyRef` / `AnyValue` / `Inline` 仍存在；默认 public 假设多 | public API 显式 public；sealed marker 删除；cone 只导出 public |
| spec / fixtures | `SCOOP_FULL_SPEC.md`、`docs/spec/language_spec-part*.md`、`tools/spec_fixtures.py`、`tools/run_fixtures.py`、`tests/fixtures/**` | spec 与 fixtures 使用旧 surface | spec / generated fixtures / handwritten fixtures 与实现同步 |

## 4. 顺序总览

1. P0：冻结当前偏离、建立迁移清单与最小回归矩阵。
2. P1：先落纯 spec / low-risk cleanup，删除 `@Inline` 并建立 spec 同步基线。
3. P2：收敛 parser / AST 语法 surface，完成旧语法到新语法的机械迁移。
4. P3：落地 SPEC_FIX 的 type/effect/lowering 语义变化，修正 sysroot、visibility 与 cone export。
5. P4：实现 overload definition-time 规则，先把“无论如何调用都不合法”的声明前端拒绝。
6. P5：实现 overload call-site 五阶段 resolution，并让 selected callable identity 贯穿 MIR/codegen。
7. P6：全量 spec / fixtures / docs 收尾与 regression matrix。

依赖说明：

- P0 必须先于所有实现阶段，因为当前仓库同时存在旧 surface 与已部分迁移逻辑；不先冻结清单，后续容易漏改 fixture 或误判 design drift。
- P1 可先做，因为 A1/A2/D1 是文档收口，`@Inline` 删除是孤立低风险项；但若任何 spec code block 改动导致 generated fixtures 失败，必须把对应 compiler change 提前到同一 PR/任务中完成。
- P2 必须先于 P3，因为 refutable pattern、operator modifier、`ref`/`value` bound、tuple `.0` 与 f-string `${...}` 都需要 AST 表达稳定后才能做语义。
- P3 必须先于 P5，因为 overload call-site phase 依赖新的 visibility 默认、bound constraint kind、operator gate 与 `Nothing` subtype 规则。
- P4 必须先于 P5，因为 call-site resolution 假设同一 overload set 已经过 definition-time pruning。
- P6 之前不算完成；只让少量 fixtures 通过但 spec / sysroot / docs 仍使用旧 surface，不代表本轮闭环。

## 5. 分阶段计划

### P0. 冻结当前偏离、建立迁移清单与最小回归矩阵

参考：
- [`SPEC_FIX.md`](./SPEC_FIX.md) 全文
- [`OVERLOAD_RESOLUTION.md`](./OVERLOAD_RESOLUTION.md) §1、§2、§10
- `tests/fixtures/**`
- `tools/spec_fixtures.py`
- `tools/run_fixtures.py`

目标：

- 把旧 surface、已知 overload bug、sysroot 默认 public 假设、spec doctest 生成面先列清楚。
- 建立后续阶段可复用的最小 targeted regression，避免每个 agent 重新搜索。

必须实现的内容：

1. 对活跃源码、sysroot、spec 与 fixture 做迁移清单，至少覆盖：
   - `perform`；
   - `handle { ... } with { ... }`；
   - tuple `._0` / `._1`；
   - f-string `{...}` / `{{` / `}}`；
   - `@Inline` / `annotation class Inline`；
   - `AnyRef` / `AnyValue`；
   - 依赖默认 public 的 sysroot 与 cross-package fixtures；
   - `*overload*.scoop`、`*vararg*.scoop`、`*operator*.scoop`。
2. 新增或确认最小 overload bug fixtures：
   - concrete overload `f(Int)` / `f(Bool)` 互不串扰；
   - arity overload `g(Int)` / `g(Int, Int)` 均可 materialize；
   - generic + concrete 同名时 concrete 按 specificity 胜出；
   - ambiguous / no-applicable diagnostics 列候选位置且不含 backend 术语。
3. 明确哪些旧 syntax fixture 是“要机械迁移”，哪些应改成 negative fixture。
4. 记录 P0 期间仍允许的 design drift，后续阶段必须逐项关闭而不是长期跳过。

必须遵从的约束：

- P0 不改变语言行为；只做 inventory、targeted fixtures 和记录。
- 不得删除旧 fixture 来掩盖当前 bug；需要迁移的 fixture 必须有新 surface 等价版本或明确 negative 版本。

阶段输出：

- 一份旧 surface / sysroot / fixture 迁移清单。
- 一组最小 overload regression fixtures。
- 后续 P1-P6 可引用的 bug baseline。

验证：

1. `python3 tools/spec_fixtures.py check`
2. `python3 tools/run_fixtures.py`
3. targeted fixture：`tests/fixtures/**/**/*overload*.scoop`

完成条件：

- 后续实现阶段不需要重新判读哪些旧行为是待删除 surface，哪些是当前必须保持的 baseline。

### P1. 纯 spec / low-risk cleanup 与 `@Inline` 删除

参考：
- [`SPEC_FIX.md`](./SPEC_FIX.md) A1、A2、B1、D1
- `SCOOP_FULL_SPEC.md`
- `docs/spec/language_spec-part*.md`
- `sysroot/lib/scoop.core/src/core.scoop`
- `crates/scoopc_hir/src/typecheck/{builtin_annotations.rs,annotations.rs}`

目标：

- 先处理不依赖深层 lowering 的 spec 完整性和 annotation cleanup。
- 删除 `@Inline` 这个无语义保证的 surface，避免后续 spec/fixtures 继续引用。

必须实现的内容：

1. 更新 `SCOOP_FULL_SPEC.md` 与 split spec：
   - §2.1 增加 `Nothing`：bottom type、无 inhabitant、非 ref/value；
   - §13 开头明确 cone 与 package 的层级关系；
   - §2.6 decision record：保留 value type immutability + `with`，无 struct `var` 字段；
   - 删除 `@Inline` 章节与 built-in annotation 表项。
2. 删除 sysroot 中 `annotation class Inline`。
3. 删除 typecheck 对 `@Inline` 的 builtin annotation recognition 和专用 diagnostic 路径。
4. 更新或删除所有引用 `@Inline` 的 fixtures / doctest blocks。

必须遵从的约束：

- 不得把 `inline` keyword 改成新的优化 hint；compiler 是否 inlining 仍由自身 heuristic 决定。
- 如果 `SCOOP_FULL_SPEC.md` 中的 fixture code block 变更，必须同步运行 `tools/spec_fixtures.py sync` 并检查生成结果。

阶段输出：

- spec 中 `Nothing` / cone-package / value-type decision 已明确。
- `@Inline` 从 sysroot、typecheck、fixtures、spec 中消失。

验证：

1. `python3 tools/spec_fixtures.py sync`
2. `python3 tools/spec_fixtures.py check`
3. `python3 tools/run_fixtures.py`
4. targeted search：活跃代码与 spec 不再出现 `@Inline` / `annotation class Inline`。

完成条件：

- `@Inline` 不再是语言 surface，且 spec-only 决议不会继续漂移。

### P2. Parser / AST 语法 surface 收敛

参考：
- [`SPEC_FIX.md`](./SPEC_FIX.md) A3、B2、B4、B6、C2、C4
- `crates/scoopc_ast/src/syntax/{token.rs,lexer.rs,string_literal.rs}`
- `crates/scoopc_ast/src/parser/{expr.rs,decls.rs,pattern.rs,stmt.rs}`
- `crates/scoopc_ast/src/ast/mod.rs`

目标：

- 让 parser/AST 只表达目标语言 surface，为 P3 typecheck/lowering 与 P5 overload resolution 提供稳定输入。

必须实现的内容：

1. `perform` 删除：
   - 从 expression prefix parser 移除 `perform expr`；
   - effect operation 继续通过普通 qualified call `Effect.op(args)` 解析；
   - 旧 `perform` fixture 改成 negative parse/typecheck fixture。
2. handler keyword 改为 `on`：
   - `handle { body } on { ... } finally { ... }` 成为唯一正例；
   - `handle ... with ...` 报清晰 diagnostic；
   - `try/catch` desugaring 文档目标改成 `handle ... on ...`。
3. tuple/member numeric segment：
   - lexer 在 `.` 后的 numeric run 不把 `1.2` 贪成 float；
   - parser member access / with path 接受 `.0` / `.1` numeric segment；
   - 旧 `._0` / `._1` fixtures 迁移或变 negative。
4. f-string `${...}`：
   - f-string 内仅 `${` 开启插值；
   - `{` / `}` 是 literal，无需 `{{` / `}}`；
   - 不支持 `$x` shorthand，并添加 negative fixture。
5. `operator` modifier：
   - lexer/parser/AST 增加 `operator` modifier；
   - 先只进入 AST，不在 P2 改 resolution 行为。
6. inline generic bounds 与 `ref` / `value` bound surface：
   - `parse_type_param_list` 支持 `<T: Bound>` 形式，与现有 `where T: Bound` 收敛为同一 internal constraint 表达；
   - `ref` / `value` 仅在 bound position 合法，不能作为普通类型、type argument、`is` / `as` target 或 supertype；
   - 如实现选择 context keyword，不得污染普通 identifier 解析。

必须遵从的约束：

- 旧语法只允许作为 negative fixture 留存，不得作为长期 alias。
- `operator` modifier 的 parser 接入不得让普通函数自动获得 operator 能力；真正语义 gate 在 P3。
- `.0` 解析必须覆盖 chained case：`x.1.2` 应按 member segment 解析，而不是 float literal。

阶段输出：

- AST 能表达本轮所有新 surface。
- 旧 surface 在 parser/typecheck 层有清晰 reject。
- fixture 语法已机械迁移到目标写法。

验证：

1. `python3 tools/run_fixtures.py`
2. targeted parse fixtures：`tests/fixtures/parse/*handle*`、`*f_string*`、`*with*`、`*tuple*`、`*operator*`
3. targeted typecheck fixtures：`perform` 旧语法 negative、`ref` / `value` 非 bound position negative

完成条件：

- parser 不再阻塞 P3 的语义改动，且旧语法不会静默通过。

### P3. SPEC_FIX type/effect/lowering 语义落地

参考：
- [`SPEC_FIX.md`](./SPEC_FIX.md) A3、B3、B5、C1、C3、C4、C5
- `crates/scoopc_hir/src/typecheck/expr/{ops.rs,member.rs,infer.rs}`
- `crates/scoopc_hir/src/typecheck/{val_pat.rs,type_env.rs,lower.rs,where_clause.rs}`
- `crates/scoopc_hir/src/hir/lower/expr/{members.rs,canonical_call.rs,main_lower.rs}`
- `crates/scoopc_mir/src/mir/lower/{fn_lowering_expr.rs,fn_lowering_call.rs}`
- `crates/scoopc_cone/src/{scoopir/export.rs,visibility.rs}`
- `sysroot/lib/scoop.core/src/core.scoop`

目标：

- 把 SPEC_FIX 中会改变类型、effect、lowering、sysroot、visibility 的语言语义一次性收口。

必须实现的内容：

1. operator modifier gate：
   - operator-positioned call 只考虑带 `operator` modifier 的候选；
   - 普通 named call `x.plus(y)` 不要求 `operator`；
   - diagnostics 指出“候选存在但未声明 operator”与“没有候选”的区别。
2. `!!` 和 `as` failure 改为 panic：
   - typecheck 不再记录 `Raise<RuntimeError>` edge；
   - HIR/MIR lowering failure arm 生成 `scoop.core.panic(...)` 或等价 panic primitive；
   - `as?` 不变，仍返回 `Option<T>`；
   - `RuntimeError` 若仍被其它 surface 使用，保留；若仅剩旧路径使用，删除或降级为 panic message tag。
3. enum `with` mismatch panic：
   - lowering 已有 variant check 的 mismatch arm 改为 panic；
   - 不再 silent preserve original value。
4. refutable `val` pattern：
   - `val Some(x) = e` 允许通过 typecheck；
   - mismatch fallback lower 到 panic；
   - 保持 tuple / struct irrefutable pattern 现有行为。
5. closure 捕获 `var` 禁止：
   - 在 closure capture analysis 或 typecheck 阶段检测引用外层 mutable binding；
   - 报错提示 `RefCell<T>`、显式 `val snapshot = ...` 或 higher-order accumulation；
   - `val` capture 不变。
6. `AnyRef` / `AnyValue` sealed marker 替换为 `ref` / `value` bound constraint kind：
   - 删除 sysroot marker declarations；
   - type lowering 中把 `T: ref` / `T: value` 表示为内部 kind constraint；
   - 拒绝把 `ref` / `value` 当类型使用；
   - 更新 `Atomic<T>`、`AtomicValue<T>` 等 sysroot bounds；
   - 移除或收缩 `SealedMarkerInfo`、`ANY_REF_MARKER_FQN`、`ANY_VALUE_MARKER_FQN` 相关逻辑。
7. 默认 visibility 改为 `internal`：
   - `visibility_from_modifiers` 无 modifier 返回 `Internal`；
   - sysroot 与需要导出的 fixture/API 显式加 `public`；
   - `.cone` exporter 继续只导出 `public` declarations；
   - `internal` 语义明确为同一 cone 可见。

必须遵从的约束：

- panic 路径不得通过 `Raise.raise` 间接表达；否则 effect row 仍会被污染。
- `ref` / `value` 是 bound keyword，不是新类型；不得在 runtime metadata 或 supertype list 中生成它们。
- 默认 visibility 改动必须和 sysroot public annotation 同步提交，否则会造成大量 downstream / fixture 误报。

阶段输出：

- SPEC_FIX 的非 overload 语义改动已在 compiler/sysroot/cone 中落地。
- 断言失败统一 panic，不再影响 effect row。
- 默认 internal visibility 与 public export contract 生效。

验证：

1. `python3 tools/run_fixtures.py`
2. `cargo test --all --all-targets`
3. targeted fixtures：`not_null_assert*`、`runtime_typecheck_cast*`、`*with_update*`、`*val_pattern*`、`*closure*capture*`、`*visibility*`、`*anyref*` / `*ref_value_bound*`
4. `python3 tools/spec_fixtures.py check`

完成条件：

- SPEC_FIX 中除 overload 相关后续验证外，compiler-visible 语义都已关闭，不再留下旧 Raise / sealed marker / public default 路径。

### P4. Overload definition-time 规则落地

参考：
- [`OVERLOAD_RESOLUTION.md`](./OVERLOAD_RESOLUTION.md) §3、§4、§7、§8、§9、§10、§12
- `crates/scoopc_hir/src/typecheck/overloads.rs`
- `crates/scoopc_hir/src/typecheck/{headers.rs,inheritance.rs,interfaces.rs,override_effects.rs}`
- `crates/scoopc_hir/src/resolve/mod.rs::{FunSig,FunOverload,ConstructorOverload,ParamSig}`

目标：

- 在声明处理阶段拒绝“无论怎么调用都不可能合法”的 overload set，给 P5 call-site resolution 一个干净输入。

必须实现的内容：

1. effective signature / signature equivalence：
   - signature 只含参数类型与 arity，不含 return type 与 effect row；
   - type alias 透明；
   - type parameter alpha-equivalence 等价；
   - 参数位 effective type 完全相同视为 conflicting overloads。
2. generic overload shape 规则：
   - 允许同 shape、仅 bound 不同的 generic overload；
   - 允许 concrete 与 generic 同 shape 混合，concrete 视为更紧 bound；
   - reject 参数 shape 不同、TP 一致性约束等当前不支持形式，错误码 `generic_overload_shape_mismatch`；
   - bound 不可比的同 shape overload 不在定义点 reject，留给 call-site ambiguity。
3. vararg 与非 vararg 重叠：
   - 若 vararg 可 cover 非 vararg arity 且对应类型兼容，定义点 reject，错误码 `vararg_overlaps_non_vararg`；
   - 不允许依赖 spread operator 在 call-site 手动消歧。
4. override / overload 边界：
   - 父类 method 非 `open`，子类同 signature reject；
   - 父类 method `open`，子类同 signature 缺 `override` reject；
   - `override` 但父类无匹配 signature reject；
   - 子类同名但 signature 不同是新增 overload；
   - effect row 不参与 override target 匹配。
5. 虚方法不可方法级 generic：
   - `open fun`、`abstract fun`、`override fun`、interface method 引入方法级 TP 时 reject；
   - 类级/interface 级 TP 出现在 method signature 中仍合法。
6. constructor overload：
   - constructor 与 fun 共用 signature equivalence、generic shape、vararg overlap 与 diagnostics；
   - ctor 级 TP 与 class 级 TP 区分清楚。
7. diagnostics：
   - `conflicting_overloads`、`generic_overload_shape_mismatch`、`vararg_overlaps_non_vararg`、override 错误必须列出相关候选位置与 signature；
   - 错误文本不含 backend/LLVM 术语。

必须遵从的约束：

- 不得在 P4 实现 call-site “猜一个能用的” workaround；P4 只负责定义点集合合法性。
- 返回类型和 effect row 绝不能参与 overload signature。
- 虚 generic method 不允许通过 monomorphization 后动态 vtable slot 逃逸。

阶段输出：

- 合法 overload set 的 definition-time contract 固定。
- 不合法 overload set 在 typecheck 前端稳定报错。

验证：

1. `python3 tools/run_fixtures.py`
2. 新增/更新 typecheck fixtures：
   - return/effect-only overload conflict；
   - TP alpha-equivalent conflict；
   - generic shape mismatch；
   - bound-incomparable overload 定义点通过；
   - vararg overlap reject；
   - virtual method generic reject；
   - override boundary 四类样例。
3. `cargo test --all --all-targets`

完成条件：

- P5 看到的 overload set 都满足 design 文档的定义点前提。

### P5. Overload call-site resolution 与 callable identity 贯通

参考：
- [`OVERLOAD_RESOLUTION.md`](./OVERLOAD_RESOLUTION.md) §5、§6、§7、§8、§9、§10、§11
- `crates/scoopc_hir/src/typecheck/expr/call/{dispatch.rs,member_call.rs,ctor.rs,args.rs,generic.rs,value_call.rs}`
- `crates/scoopc_hir/src/typecheck/expr/ops.rs`
- `crates/scoopc_hir/src/hir/lower/expr/{typechecked.rs,canonical_call.rs}`
- `crates/scoopc_mir/src/mir/materialize/hir_calls.rs`
- `crates/scoopc_mir/src/mir/callables.rs`
- `crates/scoopc_codegen_llvm/src/llvm/codegen/mir_body/call.rs`

目标：

- 实现 call-site 五阶段 overload resolution，并让 typecheck 选出的唯一 callable identity 贯穿 HIR/MIR/materialization/codegen，修复当前 concrete / arity / generic-concrete overload bug。

必须实现的内容：

1. Phase A 候选收集：
   - local → member → extension → top-level → imported；
   - 外层找到任何同名候选即停止下沉；
   - member overload set 包含继承、override 替换和子类新增 overload。
2. Phase B visibility：
   - visibility 在 applicability 前筛选；
   - `private` / `protected` / `internal` / `public` 按当前 spec 语义判断；
   - 不可见候选不得影响 specificity。
3. Phase C applicability：
   - arity、named/default/vararg mapping 使用统一 args mapper；
   - 实参类型必须是形参类型 subtype；
   - 不引入隐式 widening；
   - `Nothing` 是所有类型 subtype；
   - function type 按参数逆变、返回协变、effect row 子集处理；
   - tuple/struct variance 按 spec 处理。
4. Phase D specificity：
   - A 更具体 iff 每个参数位 `A.eff_i <: B.eff_i` 且至少一处 strict；
   - member receiver 算第 0 参数位；
   - concrete param effective type 是自身；
   - method-level TP effective type 是 declared bound，无 bound 为 `Any`；
   - 多重 bound 用 intersection effective type；
   - 复合类型中 TP 替换为 declared bound；
   - 不使用 inferred substitution 做 specialization。
5. Phase E ambiguity：
   - 无唯一最具体时报 `ambiguous_overload`；
   - 列出所有适用候选、位置、effective type 来源和不可比原因。
6. effect row / return type 校验顺序：
   - overload resolution 先选唯一候选；
   - effect row 校验在选定之后进行；
   - effect 不匹配不得回退选择其它 overload。
7. lambda / function type overload：
   - 不加额外 reject；
   - 按普通 subtype specificity 处理。
8. operator desugar 与 overload：
   - `a + b` 等先 desugar 到 operator call，再走同一 overload algorithm；
   - P3 的 `operator` modifier gate 继续生效。
9. callable identity 贯通：
   - `TopLevelFunCallBinding` / member binding / ctor binding 中保留选中的 overload id、signature、type args、owner；
   - HIR lowering 不再用 bare FQN 重新查同名函数；
   - MIR materialization 以 binding identity 选择 template / callable version；
   - LLVM declaration/call 不再因同名 overload 混淆参数类型或 arity。
10. diagnostics：
   - `no_applicable_overload` 列出所有同名候选与每个不适用原因；
   - `ambiguous_overload` 列出所有适用候选与 cross-incomparable 位置；
   - candidate location 必填。

必须遵从的约束：

- 不得让 backend/codegen 继续承担 overload disambiguation。
- 不得为了修复 generic + concrete case 而引入 inferred-substitution specialization；specialization 只看 declared bound。
- 不得把 effect row 或 return type 纳入 signature 或 specificity。

阶段输出：

- 完整 call-site overload resolution。
- selected callable identity 贯穿 typecheck → HIR → MIR → LLVM。
- 当前三类 overload bug 变成 run-pass / typecheck regression。

验证：

1. `python3 tools/run_fixtures.py`
2. targeted fixtures：
   - concrete overload `f(Int)` / `f(Bool)`；
   - arity overload；
   - generic + concrete specificity；
   - bound-based specialization；
   - incomparable bound ambiguity；
   - member receiver specificity；
   - static resolution + dynamic dispatch；
   - constructor overload；
   - lambda/function type overload；
   - effect row mismatch after selection。
3. `cargo test --all --all-targets`
4. audit：overload diagnostics 中不出现 `backend`、`LLVM`、`UnsupportedMainBody`。

完成条件：

- overload 行为符合 design 文档，且 codegen 不再因同名 callable 混淆失败。

### P6. Spec / fixtures / docs 全量收尾与回归矩阵

参考：
- [`SPEC_FIX.md`](./SPEC_FIX.md) summary table
- [`OVERLOAD_RESOLUTION.md`](./OVERLOAD_RESOLUTION.md) §12
- `SCOOP_FULL_SPEC.md`
- `docs/spec/language_spec-part*.md`
- `tools/spec_fixtures.py`
- `tools/run_fixtures.py`
- `tests/fixtures/**`

目标：

- 把 P1-P5 的实现结果完整反映到 spec、fixtures、sysroot 注释和 regression matrix 中。
- 确保后续 agent 不需要重新解释旧 surface 是否仍被支持。

必须实现的内容：

1. 更新 `SCOOP_FULL_SPEC.md` 所有相关章节：
   - type hierarchy / `Nothing`；
   - tuple field `.0`；
   - `val` refutable pattern；
   - `!!` / `as` panic；
   - effect op plain qualified call；
   - handler `on`；
   - closure capture；
   - f-string `${...}`；
   - `ref` / `value` bound；
   - default `internal` visibility；
   - overload resolution rules；
   - `operator` modifier requirement；
   - delete `@Inline` / `AnyRef` / `AnyValue` old prose。
2. 同步 split spec `docs/spec/language_spec-part*.md` 或明确其生成/同步流程。
3. 运行 `tools/spec_fixtures.py sync` 并检查生成的 `tests/fixtures/spec_doctest/**`。
4. 机械更新 handwritten fixtures：
   - `perform` → plain effect op call；
   - handler `with` → `on`；
   - `._0` → `.0`；
   - f-string `{expr}` → `${expr}`；
   - sysroot/API export 加显式 `public`；
   - `AnyRef` / `AnyValue` bounds 改为 `ref` / `value`；
   - operator functions 加 `operator` 或改成普通 named call。
5. 增加 final audit：
   - 活跃 spec / sysroot / fixtures 不再出现旧 surface，除 negative fixtures 或历史文档外；
   - overload 错误诊断满足候选位置与原因要求；
   - `.cone` API export 只包含 public declarations。
6. 全量验证：
   - `cargo fmt`；
   - `cargo test --all --all-targets`；
   - `python3 tools/spec_fixtures.py check`；
   - `python3 tools/run_fixtures.py`；
   - LLVM/backend targeted tests 如当前环境启用默认 LLVM feature。

必须遵从的约束：

- P6 不能把未完成行为简单记成 future work；剩余项必须是明确超出本轮两份设计文档的 v2+ 扩展。
- negative fixtures 中保留旧语法时，文件名和 expected diagnostic 必须明确这是旧 surface 被拒绝。

阶段输出：

- spec、sysroot、fixtures、compiler 行为一致。
- 本轮设计文档中的 pending 项全部有实现或明确的回写决议。
- 完整 regression matrix 通过。

验证：

1. `cargo fmt`
2. `cargo test --all --all-targets`
3. `python3 tools/spec_fixtures.py check`
4. `python3 tools/run_fixtures.py`

完成条件：

- `SPEC_FIX.md` 与 `OVERLOAD_RESOLUTION.md` 的目标行为成为活跃 spec 和 compiler 的实际 contract。
- 旧 surface 只存在于 archive、design history 或明确 negative fixture 中。

## 6. 预期收口状态

- `SCOOP_FULL_SPEC.md` 与 compiler 对 `Nothing`、cone/package、value immutability、tuple `.0`、f-string `${...}`、handler `on`、plain effect op call、`operator` modifier、`ref` / `value` bound、default internal visibility 的描述一致。
- `@Inline`、`AnyRef`、`AnyValue`、`perform`、handler `with`、tuple `._0`、旧 f-string `{...}` 插值不再是正向语言 surface。
- `!!`、`as` failure、refutable `val` mismatch、enum `with` variant mismatch 都 panic，不再通过 `Raise<RuntimeError>` 污染 effect rows。
- closure 捕获外层 `var` 在前端报错，用户必须显式选择 `RefCell`、snapshot 或 higher-order accumulation。
- overload definition-time 与 call-site resolution 按 `OVERLOAD_RESOLUTION.md` 的五阶段模型工作，diagnostics 列出候选位置和不可比原因。
- selected overload identity 从 typecheck 贯穿到 HIR/MIR/materialization/codegen，同名函数不再在 lowering/codegen 阶段互相串扰。
- `.cone` API export 只包含显式 `public` declarations；未标注 declarations 默认为 cone-internal。
- fixtures 与 spec doctests 覆盖语法迁移、SPEC_FIX 语义变更、overload 正例/负例和关键 codegen bug。
