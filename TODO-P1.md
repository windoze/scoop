# TODO（P1：AST / surface contract 冻结）

> 生成时间：2026-05-02  
> 设计基线：[`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md)  
> 计划基线：[`PLAN.md`](./PLAN.md)  
> 前置条件：`TODO-P0.md` 已完整完成，且新旧主线可通过 `scoop` / `scoopc` 的显式 CLI 参数并存。  
> 顺序约束：严格按当前文件中的条目顺序推进；不得跨条目并行实现。  
> 本阶段目标：冻结 AST / surface contract，让 refactor 新路径在 AST 阶段明确保持普通调用语法，不引入 continuation/resume 特殊节点或特殊 keyword；同时把“哪些 sugar 只允许在 P2 typed 阶段处理”的边界固化下来。

## 全局约束

- [`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md) 是本阶段唯一设计基线；实现过程中如需改变主张，必须先回写该文档，再继续写代码。
- [`PLAN.md`](./PLAN.md) 与 [`TODO-P0.md`](./TODO-P0.md) 是本阶段执行前提；P1 不得重新开启 P0 已经定下来的 CLI / dispatcher / 共享边界讨论。
- 本阶段只处理 AST / parser / surface contract，不得提前进入 HIR 语义实现。
  - 明确禁止：在 P1 中实现 `Continuation<...>` 的 typed 语义、`resume` 的 type rule、effect row typed 传播、runtime error 的 typed lowering；这些属于 P2。
- 本阶段必须保持 AST 层普通调用模型。
  - 禁止新增 `ResumeExpr`、`ContinuationResumeExpr`、`ZeroArgUnitCallExpr`、或等价的 AST 专用节点；
  - 禁止为 continuation/resume 新增 surface keyword；
  - 禁止在 AST 阶段执行 type-dependent desugar。
- 本阶段必须严格遵守 [`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md) §5.3.1 已定下来的 surface 语义：
  - continuation 交互使用普通方法调用 `k.resume(...)`
  - `ResumeTuple = ()` 时允许 `k.resume()`，其语义只是 `k.resume(())` 的语法糖
  - 更一般地，单一 `Unit` 参数调用允许 `f()` 作为 `f(())` 的语法糖
- 但在 P1，这些规则只能被固定为**surface/AST contract**，不能提前在 AST 中做 typed rewrite。
  - `k.resume()` 在 AST 中必须仍然表现为“零参数普通调用”；
  - `k.resume(())` 必须仍然表现为“一参数调用，其参数是 `UnitLit`”；
  - `f()` 与 `f(())` 在 AST 中必须保留为两种不同形状，typed desugar 留给 P2。
- 本阶段不做 full regression。
  - 只做 parser / AST / dump-ast / parse fixtures 的定向验证；
  - 不执行 HIR/MIR/LLVM 相关全集测试。
- 所有需要触发新路径的验证都必须通过 P0 建立的 CLI 参数进入，不允许通过修改默认值或内部测试入口偷渡到 refactor 路径。

## P1-T01：建立 refactor AST stage 专用入口与阶段输出类型

- 参考：
  - [`PLAN.md`](./PLAN.md) §2/P1
  - [`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md) §4.10, §4.11, §5.3.1
- 目标：
  - 在新路径中把 AST 阶段从 P0 的“整条链 dispatcher 委托”收窄成一个明确的 stage entry；
  - 为后续 P2 建立一个稳定的 AST-stage 输出类型，明确记录“P1 不做 typed desugar”的 handoff contract。

- 必须实现的内容：
  1. 在新路径模块下新增 AST stage 文件/模块。
     - 推荐位置：`crates/scoopc/src/effect_refactor_pipeline/ast_stage.rs` 或等价位置；
     - 要求：该模块属于 refactor 新路径的阶段入口，而不是旧 parser 模块本身。
  2. 定义一个 AST stage 输出类型。
     - 名称可自行决定，但必须至少承载：
       - 输入 `SourceFile`（或可追溯到源文件的稳定引用）
       - 解析出的 `ast::File`
       - 供后续阶段使用的最小 handoff 元信息（若有）
     - 要求：这个输出类型必须在文档注释中明确写出本阶段 invariants：
       - 只保留普通 `Call` / `MemberAccess` 等 AST 语义
       - 不执行 type-dependent desugar
       - `resume()` / `f()` 之类零参数表面语法在 AST 中仍保留零参数调用形状
  3. 让 refactor pipeline 的 `dump-ast` 路径不再只是“整个 pipeline 委托 legacy”，而是显式进入新的 AST stage 入口，再把结果交给 dump 命令输出。
  4. 该 AST stage 内允许复用现有 parser 作为共享中立模块，但必须通过单一 API 调用。
     - 若需要共享 parser 功能，只能通过 `Session::parse(...)` 或等价中立 API 进入；
     - 禁止把 pipeline mode 传播到 parser 业务逻辑中。

- 必须遵从的约束：
  - 禁止为了实现 AST stage 而修改 parser，使其了解 `legacy` / `refactor` 路线差异。
  - 禁止在 AST stage 输出中夹带“后续 typed 阶段应如何解释 effect/continuation”这种仍需 HIR/typecheck 才能得出的语义结论。
  - 禁止让 `dump-ast` 的 refactor 路径仍直接调用 legacy 的整条 command implementation 而不经过新 stage。

- 验证：
  1. 新增/更新 unit tests，覆盖：
     - refactor AST stage 输出类型可构造
     - refactor `dump-ast` 路径确实进入 AST stage，而不是整条命令直接落回 legacy
  2. 运行：
      - `cargo test -p scoopc --no-default-features ast_stage`
      - `cargo run -p scoop --no-default-features -- --effect-pipeline refactor dump-ast tests/fixtures/parse/hello.scoop`
      - `cargo run -p scoop --no-default-features -- --effect-pipeline legacy dump-ast tests/fixtures/parse/hello.scoop`
  3. 验证 `legacy` 与 `refactor` 对上述最小输入输出一致。

- 完成条件：
  - refactor 新路径已拥有明确的 AST stage 入口与输出类型；
  - `dump-ast` 在 refactor 路径下不再只是整条链偷跳 legacy command；
  - P2 可以直接消费这个 AST stage 输出，而不必重新定义 handoff 结构。
- 依赖：`TODO-P0.md` 最后一项 review 完成
- 完成记录：
  - 2026-05-02：新增 `crates/scoopc/src/effect_refactor_pipeline/ast_stage.rs`，为 refactor 新路径建立独立 AST stage，并定义 `AstStageOutput<'a>` 作为后续阶段可消费的稳定 handoff 结构。
  - `AstStageOutput<'a>` 现在稳定承载输入 `SourceFile` 引用与解析后的 `ast::File`，并在文档注释中明确固定 P1 invariants：AST 只保留普通 `Call` / `MemberAccess` 形状、不做 type-dependent desugar、`k.resume()` / `f()` 继续保留零参数调用形状、`k.resume(())` / `f(())` 继续保留显式 `UnitLit` 参数。
  - `crates/scoopc/src/effect_refactor_pipeline/mod.rs` 新增 `load_ast_stage_output_for_dump(...)`；refactor 模式下 `dump-ast` 已显式进入 AST stage，再把 stage 输出交给 driver 渲染，而不再只是整条命令路径统一委托 legacy。
  - parser 仍通过中立共享 API `Session::parse(...)` 复用；本次改动未把 pipeline selector 注入 `parser/` 业务逻辑。
  - 新增/更新测试：`crates/scoopc/src/effect_refactor_pipeline/ast_stage.rs`、`crates/scoopc/src/effect_refactor_pipeline/mod.rs`、`crates/scoop/src/commands/dump_ast.rs`。
  - 验证通过：`cargo test -p scoopc --no-default-features ast_stage`、`cargo test -p scoopc --no-default-features effect_refactor_pipeline`、`cargo test -p scoop --no-default-features dump_ast_command_uses_refactor_ast_dispatcher`、`cargo run -p scoop --no-default-features -- --effect-pipeline refactor dump-ast tests/fixtures/parse/hello.scoop`、`cargo run -p scoop --no-default-features -- --effect-pipeline legacy dump-ast tests/fixtures/parse/hello.scoop`、`cargo clippy -p scoop -p scoopc --all-targets --no-default-features -- -D warnings`。

## P1-T01R：Review AST stage 入口与 handoff 类型，确认 parser 仍是中立共享模块

- 参考：
  - [`PLAN.md`](./PLAN.md) §0，§2/P1
  - [`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md) §4.11, §5.3.1
- 重点：
  - AST stage 是否已经是 refactor 新路径上的显式阶段，而不是隐式落回 legacy；
  - parser 是否继续作为共享中立模块存在，没有渗入 pipeline mode；
  - AST stage 输出类型是否已经把“typed desugar 留给 P2”写成明确 contract。
- 必须检查的文件/位置：
  - 新增的 AST stage 模块
  - `crates/scoopc/src/session/mod.rs`
  - `crates/scoop/src/commands/dump_ast.rs` 或等价 dispatch 入口
  - 任何新加的 parser 共享 API

- 验证：
  - 重新运行 P1-T01 的所有测试与命令；
  - 额外搜索一遍 parser 目录，确认没有把 pipeline mode 注入 parser 业务逻辑：
    - `rg "EffectPipelineMode|effect_pipeline|legacy|refactor" crates/scoopc/src/parser crates/scoopc/src/ast`
  - 若命中结果来自测试或注释，需在完成记录中解释；若命中业务代码，必须修复。

- 完成条件：
  - review 能明确证明 parser 仍是中立共享模块；
  - 可进入 P1-T02。
- 依赖：P1-T01
- 完成记录：
  - 2026-05-02：完成 `P1-T01R` review，未发现需要在 `P1-T02` 前补入的新前置缺陷；最近一次提交 `[P1-T01] Introduce refactor AST stage output` 也未显式留下与本 review 直接相关的未完成事项。
  - AST stage 复核结论：`crates/scoopc/src/effect_refactor_pipeline/ast_stage.rs` 中的 `AstStageOutput<'a>` 已稳定承载 `SourceFile` 引用与 `ast::File`，并在文档注释中明确写出 P1 handoff invariants：AST 仅保留普通 `Call` / `MemberAccess` 形状、不做 type-dependent desugar、`k.resume()` / `f()` 仍保留零参数调用、`k.resume(())` / `f(())` 仍保留显式 `UnitLit` 参数。
  - 路由复核结论：`crates/scoopc/src/effect_refactor_pipeline/refactor.rs` 仅在 `StageKind::Ast` 边界通过 `ast_stage::run(session, source)` 进入新 AST stage；`crates/scoopc/src/effect_refactor_pipeline/mod.rs` 的 `load_ast_stage_output_for_dump(...)` 在 refactor 模式下显式调用该 stage，而 `crates/scoop/src/commands/dump_ast.rs` 的生产路径已统一经 `load_ast_for_dump(...)` 进入该 wrapper，不再整条命令偷跳 legacy。
  - parser 中立性复核结论：`crates/scoopc/src/session/mod.rs` 仍只通过中立共享 API `Session::parse(&SourceFile)` 复用 parser；`crates/scoopc/src/parser/mod.rs` 继续暴露 `parse_file(source: &SourceFile) -> Result<ast::File, ParseError>`，未引入 pipeline selector 参数。
  - 搜索摘要：执行 `rg -n "EffectPipelineMode|effect_pipeline|legacy|refactor" crates/scoopc/src/parser crates/scoopc/src/ast` 输出为 0 命中，说明 parser / AST 业务代码中没有渗入 pipeline mode 或 legacy/refactor 路线分叉。
  - 复验通过：`cargo test -p scoopc --no-default-features ast_stage`、`cargo test -p scoopc --no-default-features effect_refactor_pipeline`、`cargo test -p scoop --no-default-features dump_ast_command_uses_refactor_ast_dispatcher`、`diff -u <(cargo run -q -p scoop --no-default-features -- --effect-pipeline legacy dump-ast tests/fixtures/parse/hello.scoop) <(cargo run -q -p scoop --no-default-features -- --effect-pipeline refactor dump-ast tests/fixtures/parse/hello.scoop)>`、`cargo clippy -p scoop -p scoopc --all-targets --no-default-features -- -D warnings`。

## P1-T02：锁定 continuation/resume 与单一 `Unit` 参数调用的 AST 形状

- 参考：
  - [`PLAN.md`](./PLAN.md) §2/P1
  - [`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md) §5.3.1（尤其是 `k.resume(...)` / `resume()` / `f() == f(())` 部分）
- 目标：
  - 用 parse fixtures 和 parser 单元测试把 P1 关心的 surface 语法锁死；
  - 明确“continuation/resume 没有特殊 AST 节点，仍只是普通 member-call / call”。

- 必须实现的内容：
  1. 新增最小 parse fixtures，推荐至少包括：
     - `tests/fixtures/parse/continuation_resume_member_call_basic.scoop`
       - 场景：`k.resume(x)`
       - 目标：锁定“resume 是普通 member call + call”
     - `tests/fixtures/parse/continuation_resume_unit_call_basic.scoop`
       - 场景：`k.resume()`
       - 目标：锁定“零参数 member call，不在 AST 中自动补 `UnitLit`”
     - `tests/fixtures/parse/unit_single_param_zero_arg_call_basic.scoop`
       - 场景：同一文件中同时出现 `f()` 与 `f(())`
       - 目标：锁定“AST 保留这两种形状的区别，typed desugar 留给 P2”
  2. 为以上 fixtures 生成 `.ast` snapshot。
  3. 在 `crates/scoopc/src/parser/tests.rs` 或等价位置新增结构性断言，至少检查：
     - `k.resume(x)` 的 AST 为：
       - `ExprKind::Call { ... }`
       - 其 `callee` 为 `ExprKind::MemberAccess { member: resume, ... }`
       - `args.len() == 1`
     - `k.resume()` 的 AST 为：
       - `ExprKind::Call { ... }`
       - `callee` 为 `ExprKind::MemberAccess`
       - `args.len() == 0`
     - `f(())` 的 AST 为：
       - `ExprKind::Call { ... }`
       - `args.len() == 1`
       - 唯一参数为 `ExprKind::UnitLit`
     - `f()` 的 AST 为：
       - `ExprKind::Call { ... }`
       - `args.len() == 0`
  4. 若当前 parser / AST debug 输出中还没有足够稳定的信息区分这些形状，则先补充 Debug 输出稳定性，使 snapshot 能可靠锁定这些差异。

- 必须遵从的约束：
  - 禁止为了让 fixture 通过而在 AST 中新增 `ResumeExpr` / `ZeroArgUnitCallExpr` / 等价特例节点。
  - 禁止在 parser 中根据标识符名字 `resume` 做特殊处理。
  - 禁止在 parser 中因为看到 `()` 就自动改写成显式 `UnitLit` 参数调用；`f()` 与 `f(())` 必须在 AST 形状上区分开。

- 验证：
  1. 运行定向 parser 测试：
      - `cargo test -p scoopc --no-default-features parser::tests`
  2. 运行定向 parse fixtures（legacy + refactor 两种路径都跑）：
      - `cargo run -p scoop --no-default-features -- --effect-pipeline legacy test --fixtures tests/fixtures/parse/continuation_resume_member_call_basic.scoop`
      - `cargo run -p scoop --no-default-features -- --effect-pipeline refactor test --fixtures tests/fixtures/parse/continuation_resume_member_call_basic.scoop`
      - `cargo run -p scoop --no-default-features -- --effect-pipeline legacy test --fixtures tests/fixtures/parse/continuation_resume_unit_call_basic.scoop`
      - `cargo run -p scoop --no-default-features -- --effect-pipeline refactor test --fixtures tests/fixtures/parse/continuation_resume_unit_call_basic.scoop`
      - `cargo run -p scoop --no-default-features -- --effect-pipeline legacy test --fixtures tests/fixtures/parse/unit_single_param_zero_arg_call_basic.scoop`
      - `cargo run -p scoop --no-default-features -- --effect-pipeline refactor test --fixtures tests/fixtures/parse/unit_single_param_zero_arg_call_basic.scoop`
  3. 额外 smoke：
      - `cargo run -p scoop --no-default-features -- --effect-pipeline refactor dump-ast tests/fixtures/parse/continuation_resume_unit_call_basic.scoop`
      - `cargo run -p scoop --no-default-features -- --effect-pipeline refactor dump-ast tests/fixtures/parse/unit_single_param_zero_arg_call_basic.scoop`

- 完成条件：
  - continuation/resume 与 `Unit` zero-arg sugar 的 AST 形状已经被 fixtures + 单元测试锁定；
  - 后续 P2 若要做 typed desugar，已经有清晰的 AST 输入合同可依赖。
- 依赖：P1-T01R
- 完成记录：
  - 2026-05-02：完成 `P1-T02`，通过 parse fixtures 与 parser 结构断言把 `k.resume(x)`、`k.resume()`、`f()` / `f(())` 的 AST 形状固定为普通 `Call` / `MemberAccess` 合同，未引入任何 `ResumeExpr`、`ZeroArgUnitCallExpr` 或等价 AST 特例节点。
  - 新增 parse fixtures 与 `.ast` golden：`tests/fixtures/parse/continuation_resume_member_call_basic.{scoop,ast}`、`tests/fixtures/parse/continuation_resume_unit_call_basic.{scoop,ast}`、`tests/fixtures/parse/unit_single_param_zero_arg_call_basic.{scoop,ast}`；其中 `k.resume(x)` 保持为 `Call { callee: MemberAccess, args.len() == 1 }`，`k.resume()` 保持为 `Call { callee: MemberAccess, args: [] }`，`f()` 与 `f(())` 分别保留为零参数调用与单个 `UnitLit` 参数调用。
  - `crates/scoopc/src/parser/tests.rs` 新增结构断言 `parse_resume_member_call_as_plain_call_shape`、`parse_zero_arg_resume_member_call_without_unit_desugar`、`parse_zero_arg_and_explicit_unit_calls_as_distinct_shapes`，直接检查 `callee` 形状、实参数量与 `UnitLit` 保留情况。
  - 验证通过：`cargo test -p scoopc --no-default-features parser::tests`、`cargo run -q -p scoop --no-default-features -- --effect-pipeline legacy test --fixtures tests/fixtures/parse/continuation_resume_member_call_basic.scoop`、`cargo run -q -p scoop --no-default-features -- --effect-pipeline refactor test --fixtures tests/fixtures/parse/continuation_resume_member_call_basic.scoop`、`cargo run -q -p scoop --no-default-features -- --effect-pipeline legacy test --fixtures tests/fixtures/parse/continuation_resume_unit_call_basic.scoop`、`cargo run -q -p scoop --no-default-features -- --effect-pipeline refactor test --fixtures tests/fixtures/parse/continuation_resume_unit_call_basic.scoop`、`cargo run -q -p scoop --no-default-features -- --effect-pipeline legacy test --fixtures tests/fixtures/parse/unit_single_param_zero_arg_call_basic.scoop`、`cargo run -q -p scoop --no-default-features -- --effect-pipeline refactor test --fixtures tests/fixtures/parse/unit_single_param_zero_arg_call_basic.scoop`、`cargo run -q -p scoop --no-default-features -- --effect-pipeline refactor dump-ast tests/fixtures/parse/continuation_resume_unit_call_basic.scoop`、`cargo run -q -p scoop --no-default-features -- --effect-pipeline refactor dump-ast tests/fixtures/parse/unit_single_param_zero_arg_call_basic.scoop`、`cargo clippy -p scoop -p scoopc --all-targets --no-default-features -- -D warnings`。

## P1-T02R：Review surface parse contract，确认 continuation / `Unit` sugar 仍是普通调用语法

- 参考：
  - [`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md) §5.3.1
  - [`PLAN.md`](./PLAN.md) §2/P1
- 重点：
  - `k.resume(...)` / `k.resume()` 是否都只是普通 `Call + MemberAccess` 形状；
  - `f()` 与 `f(())` 是否在 AST 中仍可区分；
  - parser 是否仍然没有任何“看到 `resume` 就特殊处理”的逻辑。
- 必须检查的文件/位置：
  - 新增的 parse fixtures 及 `.ast`
  - `crates/scoopc/src/parser/tests.rs`
  - `crates/scoopc/src/parser/expr.rs`
  - `crates/scoopc/src/ast/mod.rs`

- 验证：
  - 重新运行 P1-T02 的全部测试与命令；
  - 额外搜索：
    - `rg "ResumeExpr|ContinuationResume|ZeroArgUnit|resume\(" crates/scoopc/src/ast crates/scoopc/src/parser`
  - 允许匹配：注释、测试字符串、fixture 名称；
  - 不允许匹配：新引入的 AST 特例节点或 parser 关键分支。

- 完成条件：
  - review 能明确说明：P1 已经把 surface contract 锁成“普通调用语法”，而没有引入新的语法种类；
  - 可进入 P1-T03。
- 依赖：P1-T02
- 完成记录：
  - 2026-05-02：完成 `P1-T02R` review，未发现需要在 `P1-T03` 前补入的新前置缺陷；最近一次提交 `[P1-T02] Lock resume and Unit call AST shapes` 也未显式留下与本 review 直接相关的未完成事项。
  - fixture / AST 复核结论：`tests/fixtures/parse/continuation_resume_member_call_basic.{scoop,ast}`、`continuation_resume_unit_call_basic.{scoop,ast}`、`unit_single_param_zero_arg_call_basic.{scoop,ast}` 已稳定锁定四种形状：`k.resume(x)` 仍是 `Call { callee: MemberAccess, args.len() == 1 }`，`k.resume()` 仍是 `Call { callee: MemberAccess, args: [] }`，`f()` 与 `f(())` 继续分别保持零参数调用与单个 `UnitLit` 参数调用。
  - parser / AST 复核结论：`crates/scoopc/src/parser/tests.rs` 中 `parse_resume_member_call_as_plain_call_shape`、`parse_zero_arg_resume_member_call_without_unit_desugar`、`parse_zero_arg_and_explicit_unit_calls_as_distinct_shapes` 直接断言 `callee` 形状、实参数量与 `UnitLit` 保留情况；`crates/scoopc/src/ast/mod.rs` 的 `ExprKind` 仍只用普通 `MemberAccess` / `Call` / `UnitLit` 组合表达这些 surface，没有新增 `ResumeExpr`、`ContinuationResume`、`ZeroArgUnit*` 等 AST 特例节点；`crates/scoopc/src/parser/expr.rs` 的普通 postfix 路径仍统一通过 `parse_member_access_expr(...)` 与 `parse_call_expr(...)` 解析 `k.resume(...)` / `k.resume()`。
  - 搜索摘要：执行 `rg "ResumeExpr|ContinuationResume|ZeroArgUnit|resume\(" crates/scoopc/src/ast crates/scoopc/src/parser` 后，命中仅来自 `parser/tests.rs` 的结构断言、`ast/mod.rs` 中关于 typed side table 的注释，以及 `parser/mod.rs` 里旧 `-> resume { ... }` 已移除语法的迁移诊断帮助文本；额外复核 `crates/scoopc/src/parser/expr.rs` 中唯一的 `peek_ident_text("resume")` 分支仅用于对已删除的 handler-arm 旧语法报 `HandleImmediateResumeRemoved`，并不参与 `k.resume(...)` / `k.resume()` 的普通 member-call 解析，因此不构成新的 surface 特判。
  - 复验通过：`cargo test -p scoopc --no-default-features parser::tests`、`cargo run -q -p scoop --no-default-features -- --effect-pipeline legacy test --fixtures tests/fixtures/parse/continuation_resume_member_call_basic.scoop`、`cargo run -q -p scoop --no-default-features -- --effect-pipeline refactor test --fixtures tests/fixtures/parse/continuation_resume_member_call_basic.scoop`、`cargo run -q -p scoop --no-default-features -- --effect-pipeline legacy test --fixtures tests/fixtures/parse/continuation_resume_unit_call_basic.scoop`、`cargo run -q -p scoop --no-default-features -- --effect-pipeline refactor test --fixtures tests/fixtures/parse/continuation_resume_unit_call_basic.scoop`、`cargo run -q -p scoop --no-default-features -- --effect-pipeline legacy test --fixtures tests/fixtures/parse/unit_single_param_zero_arg_call_basic.scoop`、`cargo run -q -p scoop --no-default-features -- --effect-pipeline refactor test --fixtures tests/fixtures/parse/unit_single_param_zero_arg_call_basic.scoop`、`cargo run -q -p scoop --no-default-features -- --effect-pipeline refactor dump-ast tests/fixtures/parse/continuation_resume_unit_call_basic.scoop`、`cargo run -q -p scoop --no-default-features -- --effect-pipeline refactor dump-ast tests/fixtures/parse/unit_single_param_zero_arg_call_basic.scoop`、`cargo clippy -p scoop -p scoopc --all-targets --no-default-features -- -D warnings`。

## P1-T03：建立 AST -> HIR handoff contract，并锁定 refactor AST stage parity

- 参考：
  - [`PLAN.md`](./PLAN.md) §2/P1
  - [`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md) §4.10, §4.11, §5.3.1
- 目标：
  - 把 P1 阶段的最终输出明确成“可交给 P2 的 AST handoff contract”；
  - 同时建立 refactor AST stage 相对 legacy 的 parity 验证，证明 P1 只是冻结 surface contract，不改变默认 AST 语义。

- 必须实现的内容：
  1. 在 refactor AST stage 模块中，增加一段明确的 handoff contract 文档或等价的结构化注释，至少写清：
     - `k.resume(...)` / `k.resume()` / `f()` 都在 AST 中保持普通调用形状；
     - AST 不做 typed desugar；
     - `f()` 与 `f(())` 的等价性只在 P2 typed 阶段解释；
     - `Continuation` 的 typed 含义、runtime error 语义、effect row 传播都不属于 P1。
  2. 新增一组 refactor AST stage parity 测试，推荐命名 `refactor_ast_stage_parity_*`，比较同一输入在：
     - `legacy` pipeline
     - `refactor` pipeline
     下的 AST/debug output 完全一致。
     - 样本至少覆盖：
       - `tests/fixtures/parse/continuation_resume_member_call_basic.scoop`
       - `tests/fixtures/parse/continuation_resume_unit_call_basic.scoop`
       - `tests/fixtures/parse/unit_single_param_zero_arg_call_basic.scoop`
       - `tests/fixtures/parse/handle_expr_minimal.scoop`
  3. 如果需要，为 `dump-ast` 增加一个测试辅助层，允许在 Rust 测试中直接调用 legacy/refactor 两条 AST stage 并比较输出；
     - 但该辅助层不能成为生产入口；生产入口仍然必须是 CLI -> session -> dispatcher。

- 必须遵从的约束：
  - 不允许在 P1-T03 中偷偷引入 P2 的 typed desugar。
  - parity 的目标是证明“P1 新 AST stage 与 legacy AST 语义一致”，不是开始让 refactor AST 先行改变输出。
  - handoff contract 必须落在仓库中的代码或文档实体里，不能只存在于 TODO 描述中。

- 验证：
  1. 运行新增的 AST parity 自动化测试；
  2. 通过 CLI 再做一次 smoke：
      - `cargo run -p scoop --no-default-features -- --effect-pipeline legacy dump-ast tests/fixtures/parse/continuation_resume_member_call_basic.scoop`
      - `cargo run -p scoop --no-default-features -- --effect-pipeline refactor dump-ast tests/fixtures/parse/continuation_resume_member_call_basic.scoop`
      - `cargo run -p scoop --no-default-features -- --effect-pipeline legacy dump-ast tests/fixtures/parse/unit_single_param_zero_arg_call_basic.scoop`
      - `cargo run -p scoop --no-default-features -- --effect-pipeline refactor dump-ast tests/fixtures/parse/unit_single_param_zero_arg_call_basic.scoop`
   3. 运行：
      - `cargo test -p scoop --no-default-features refactor_ast_stage_parity`
      - `cargo test -p scoopc --no-default-features parser::tests`
  4. 不执行 full regression。

- 完成条件：
  - P1 产物已经形成一个清晰的 AST handoff contract，可直接交给 P2 typed 阶段使用；
  - new-path AST stage 在选定样本上与 legacy 完全 parity；
  - 本阶段可以结束并进入 P2。
- 依赖：P1-T02R
- 完成记录：
  - 2026-05-02：完成 `P1-T03`，将 refactor AST stage 的输出注释升级为显式 AST -> typed handoff contract，并补齐 refactor AST stage 相对 legacy 的自动化 parity 验证。
  - `crates/scoopc/src/effect_refactor_pipeline/ast_stage.rs` 现在明确写出本阶段 contract：AST 只保留普通 `Call` / `MemberAccess` 形状、不做 type-dependent desugar、`k.resume()` 与一般 `f()` 继续保留零参数调用、`k.resume(())` / `f(())` 继续保留显式 `UnitLit` 参数，且 `k.resume()` <=> `k.resume(())`、`f()` <=> `f(())` 的等价性只允许在 P2 typed 阶段解释；`Continuation` typed 含义、runtime error 传播与 effect row 解释均不属于 P1。
  - `crates/scoop/src/commands/parity.rs` 新增 4 个 `refactor_ast_stage_parity_*` 自动化测试，覆盖 `tests/fixtures/parse/handle_expr_minimal.scoop`、`continuation_resume_member_call_basic.scoop`、`continuation_resume_unit_call_basic.scoop`、`unit_single_param_zero_arg_call_basic.scoop`，统一通过 CLI -> session -> dispatcher 路径比较 `legacy` / `refactor` 的 `dump-ast` 输出，锁定 AST stage parity。
  - 本任务未修改 `TODO.md` 或 `PLAN.md`：任务顺序、索引与阶段计划保持不变。
  - 验证通过：`cargo test -p scoop --no-default-features refactor_ast_stage_parity`、`cargo test -p scoopc --no-default-features parser::tests`、`cargo run -p scoop --no-default-features -- --effect-pipeline legacy dump-ast tests/fixtures/parse/continuation_resume_member_call_basic.scoop`、`cargo run -p scoop --no-default-features -- --effect-pipeline refactor dump-ast tests/fixtures/parse/continuation_resume_member_call_basic.scoop`、`cargo run -p scoop --no-default-features -- --effect-pipeline legacy dump-ast tests/fixtures/parse/unit_single_param_zero_arg_call_basic.scoop`、`cargo run -p scoop --no-default-features -- --effect-pipeline refactor dump-ast tests/fixtures/parse/unit_single_param_zero_arg_call_basic.scoop`、`cargo clippy -p scoop -p scoopc --all-targets --no-default-features -- -D warnings`。

## P1-T03R：Review P1 阶段退出条件，确认可以进入 HIR / typecheck 新路径

- 参考：
  - [`PLAN.md`](./PLAN.md) §2/P1，§3
  - [`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md) §5.3.1
- 重点：
  - AST stage 是否已经成为 refactor 路径上的独立阶段，而不是继续隐式借用 legacy 整条链；
  - continuation/resume 与 `Unit` zero-arg sugar 的 surface contract 是否已经被 fixture 和测试完整锁定；
  - P2 需要的 typed desugar handoff 是否已经明确写清楚；
  - refactor AST stage 与 legacy 的 parity 是否已经建立。

- 验证：
  - 重新运行 P1-T01 ~ P1-T03 的所有定向测试与 smoke 命令；
  - 不再额外执行 `cargo test -p scoop` / `cargo test -p scoopc` 全 crate 测试；保持本阶段只做定向验证。

- 完成条件：
  - review 能明确说明：P1 已经完成“冻结 AST / surface contract”这一阶段目标；
  - P2 可以在不重新讨论 surface 语法的前提下直接进入 HIR / typecheck 新路径实现。
- 依赖：P1-T03
- 完成记录：
  - （执行时填写）
