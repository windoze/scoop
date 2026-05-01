# TODO（P2：HIR / typecheck 新路径落地）

> 生成时间：2026-05-02  
> 设计基线：[`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md)  
> 计划基线：[`PLAN.md`](./PLAN.md)  
> 前置条件：`TODO-P1.md` 已完整完成，refactor AST stage 已存在，且新旧主线可通过 `scoop` / `scoopc` 的显式 CLI 参数并存。  
> 顺序约束：严格按当前文件中的条目顺序推进；不得跨条目并行实现。  
> 本阶段目标：在新路径上建立 typed HIR / typecheck 阶段，让 `Continuation<ResumeTuple, Answer, Out>`、`resume(value): Answer / Out` 的 surface contract、`Unit` 零参 sugar、以及 runtime error 的普通 effect 语义都进入 HIR/typecheck 的显式 contract；同时让后续 P3 不再需要为这些语义回看 AST 或临时猜测。

## 全局约束

- [`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md) 是本阶段唯一设计基线；若实现过程中改变主张，必须先回写该文档，再继续实现。
- [`PLAN.md`](./PLAN.md) 与 [`TODO-P0.md`](./TODO-P0.md)、[`TODO-P1.md`](./TODO-P1.md) 是本阶段执行前提；P2 不得重开 P0/P1 已经收敛的 CLI / dispatcher / surface AST 形状讨论。
- 本阶段只处理 HIR / typecheck 新路径，不得提前进入 direct-style MIR 或 late lowering。
  - 明确禁止：在 P2 中实现 `StepSchema` / `ContinuationSchema` / `MaterializedEffectFacts` / `resolved_outward_cases` / LLVM lowering；这些属于 P3/P4/P6。
- 本阶段必须继续遵守 P0 的“共享模块 vs 复制实现”原则。
  - 如果现有 `typecheck` / `hir::lower_typed_for_dump` 逻辑可以通过中立单一 API 复用，则允许抽象共享；
  - 如果不能满足“完全不知道自己被哪条线调用”的条件，则必须把旧逻辑复制到 refactor 路线上，而不是在现有业务实现里加 pipeline 分支。
- 本阶段必须严格遵守 [`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md) §5.3.1 的 surface 规则：
  - `Continuation` 是源码可见的接口类型；
  - 交互语法只使用普通方法调用 `k.resume(...)`；
  - `ResumeTuple = ()` 时允许 `k.resume()` 作为 `k.resume(())` 的语法糖；
  - 一般性单一 `Unit` 参数调用允许 `f()` 作为 `f(())` 的语法糖；
  - 不新增 continuation/resume 的专用 keyword 或专用调用语法。
- 本阶段必须同时遵守 [`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md) §5.3.9：
  - `ContinuationAlreadyResumed` 等 runtime error 在语言语义上是普通 effect 分支；
  - 不能在 typecheck / HIR 里引入第二条隐藏的运行时错误通道。
- 本阶段不做 full regression。
  - 只做 HIR dump、typecheck fixtures、parser/HIR/typecheck 单元测试；
  - 不执行 MIR/LLVM/full suite。
- 所有需要走新路径的验证都必须通过 `--effect-pipeline refactor` 进入。

## P2-T01：建立 refactor typed HIR stage 入口，并让 `dump-hir` 新路径不再调用 legacy `lower_for_dump`

- 参考：
  - [`PLAN.md`](./PLAN.md) §2/P2
  - [`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md) §4.10, §4.11, §5.3.1, §5.4
- 目标：
  - 在 refactor 新路径上建立一个显式的 typed HIR stage；
  - 该 stage 的输出必须是“可交给 P3 的 typed HIR handoff”，而不是旧的 untyped / debug-only `lower_for_dump` 产物；
  - `dump-hir --effect-pipeline refactor` 必须走这条新 stage，而不是继续调用 legacy `hir::lower_for_dump(...)`。

- 必须实现的内容：
  1. 在新路径模块下新增 HIR/typecheck stage 文件/模块。
     - 推荐位置：`crates/scoopc/src/effect_refactor_pipeline/hir_stage.rs`。
     - 该模块必须属于 refactor pipeline 的阶段入口，而不是直接在 `hir::lower` 内加 pipeline 分支。
  2. 定义一个 refactor typed HIR 输出类型。
     - 名称可自定，但必须至少承载：
       - typed `hir::File`
       - 支撑该文件的 `TypeStore` / typed lowering 上下文引用或拥有权
       - 供后续阶段使用的 typed effect/continuation side tables 容器占位（本任务可以先为空壳，P2-T03/P2-T04 再补内容）
     - 该输出类型的注释中必须写明本阶段 invariants：
       - 已经过 resolver + typecheck；
       - `Continuation` / `resume` / `perform` / `handle` 的 typed contract 不再需要回 AST 猜测；
       - `dump-hir` refactor 路径应优先使用它，而不是 legacy `lower_for_dump`。
  3. 为 refactor HIR stage 提供一个明确入口函数。
     - 推荐语义：输入 `Session + SourceFile`，输出上面的 typed HIR 结果。
     - 若复用现有 `hir::lower_typed_for_dump(...)`，必须通过中立共享 API 进入；
     - 若现有 `hir::lower_typed_for_dump(...)` 不能满足共享要求，则复制逻辑到 refactor 路线，不允许在旧函数中混入 pipeline 分支。
  4. 修改 `crates/scoop/src/commands/dump_hir.rs` 与相关 dispatcher。
     - `legacy` 继续保持当前行为；
     - `refactor` 路径必须显式走新的 typed HIR stage。
  5. 若 `scoopc` 内有其它直接调用 `hir::lower_for_dump(...)` 的调试/测试辅助入口，且这些入口需要支持 refactor 路径，则必须为它们提供“调用 refactor typed stage”的新入口；
     - 禁止让 refactor 路径继续假装自己只是在跑旧 `lower_for_dump`。

- 必须遵从的约束：
  - 禁止在 `crates/scoopc/src/hir/lower/mod.rs` 的旧业务实现里加入 `if pipeline == Refactor` 分支。
  - 禁止让 `dump-hir` 的 refactor 路径直接复用 legacy command 的完整实现。
  - 禁止让 P2 的 typed HIR stage 继续输出“主要靠 `Any` / `Todo(...)` 维持不 panic”的旧 contract；对于本阶段触及的 continuation/effect surface，typed 结果必须是 explicit contract，而不是 fallback 占位。

- 验证：
  1. 新增/更新单元测试，推荐以 `refactor_typed_hir_stage_*` 命名，并至少覆盖：
     - refactor HIR stage 输出类型可构造；
     - `dump-hir` refactor 路径确实进入新 stage；
     - legacy `dump-hir` 路径仍保持旧行为。
  2. 运行：
     - `cargo test -p scoopc refactor_typed_hir_stage`
     - `cargo run -p scoop -- --effect-pipeline legacy dump-hir tests/fixtures/hir/minimal.scoop`
     - `cargo run -p scoop -- --effect-pipeline refactor dump-hir tests/fixtures/hir/minimal.scoop`
     - `cargo run -p scoop -- --effect-pipeline refactor dump-hir tests/fixtures/run-pass/continuation_resume_surface_named_tuple_and_unit_basic.scoop`
  3. 要求：
     - `legacy` 路径对现有 `tests/fixtures/hir/minimal.*` 的输出不变；
     - `refactor` 路径能产出 typed HIR 输出，而不是因缺路由落回 legacy 或直接失败。

- 完成条件：
  - refactor 新路径拥有明确的 typed HIR stage 与输出类型；
  - `dump-hir --effect-pipeline refactor` 已不再调用 legacy `lower_for_dump`。
- 依赖：`TODO-P1.md` 最后一项 review 完成
- 完成记录：
  - （执行时填写）

## P2-T01R：Review refactor typed HIR stage，确认新路径已从 legacy `lower_for_dump` 分离

- 参考：
  - [`PLAN.md`](./PLAN.md) §2/P2
  - [`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md) §4.11, §5.3.1
- 重点：
  - refactor `dump-hir` 是否真的走了新的 typed HIR stage；
  - parser / resolve / typecheck 若有共享，是否仍通过中立单一 API 复用；
  - 是否已经避免在旧 `hir::lower` 业务函数里加 pipeline 分支。
- 必须检查的文件/位置：
  - 新增的 `hir_stage` 模块
  - `crates/scoop/src/commands/dump_hir.rs`
  - `crates/scoopc/src/hir/lower/mod.rs`
  - 任何新加的 typed HIR 共享 API

- 验证：
  - 重新运行 P2-T01 的所有测试与命令；
  - 额外搜索：
    - `rg "EffectPipelineMode|refactor|legacy" crates/scoopc/src/hir crates/scoopc/src/typecheck`
  - 允许命中：新 dispatcher/stage 模块、测试、注释；
  - 不允许命中：旧 HIR lowering / typecheck 业务函数里的线路分支。

- 完成条件：
  - review 能明确说明：refactor typed HIR stage 已经是独立阶段，而不是 legacy `lower_for_dump` 的换壳；
  - 可进入 P2-T02。
- 依赖：P2-T01
- 完成记录：
  - （执行时填写）

## P2-T02：对齐 `Continuation` surface contract，并把单一 `Unit` 参数 sugar 落到 typed 阶段

- 参考：
  - [`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md) §5.3.1
  - [`PLAN.md`](./PLAN.md) §2/P2
  - 当前 sysroot 声明参考：`sysroot/core.scoop` 中 `Continuation` 相关注释与声明（当前位于 `Continuation` 注释与 `class Continuation...` 定义附近）
- 目标：
  - 让源码层 `Continuation` surface 与设计文档一致：它是编译器拥有的接口形态，而不是当前 sysroot 中的 legacy `class Continuation...` 表述；
  - 把 `k.resume()` / `f()` 这类“单一 `Unit` 参数零参写法”真正落到 typed 阶段做 canonicalization，而不是继续停留在注释或 AST 约定里。

- 必须实现的内容：
  1. 更新 `sysroot/core.scoop` 的 `Continuation` surface 声明与注释，使其与 [`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md) §5.3.1 一致。
     - 至少要完成：
       - 从当前 `class Continuation<...>` 改成 interface 语义的声明；
       - 注释里删除/改写旧的 builtin resume 特殊表述，例如：
         - 旧的“tuple payload 允许 `k.resume(a0, a1, ...)` / named args ...”
         - 旧的“兼容单 payload `k.resume(value)` 特判”
       - 新注释必须明确：源码层只保留普通方法调用 `k.resume(...)`，`resume()` 只是 `resume(())` 的 sugar。
     - 若语言当前还没有 `sealed` surface，则在 sysroot 中先使用普通 `interface`；“用户不能自己实现/伪造”这条约束由 P2-T03 的 typecheck 来保证。
  2. 在 refactor typed HIR / typecheck 路径中实现**一般性的**单一 `Unit` 参数 sugar 规则。
     - 规则必须是：
       - `f()` 只有在**选中的 callee**恰好接收一个 `Unit` 参数时，才 canonicalize 为 `f(())`；
       - 不能在 AST 阶段预先改写；
       - 不能仅仅因为函数名是 `resume` 才触发；
       - 这是对所有 callable 都成立的一般规则。
  3. 这个 sugar 的 candidate 选择规则必须明确：
     - 优先使用“普通零参数调用”已有的 exact-arity 解析；
     - 只有在 typed call 解析阶段确定当前候选是“单一 `Unit` 参数 callable”时，才插入显式 `Unit` 参数；
     - 禁止把 `f()` 一上来就无条件改写成 `f(())`，否则会破坏普通零参数函数与 overload 解析。
  4. 在 refactor typed HIR 输出中，canonical call 形状必须已经是：
     - `k.resume()` -> 一个显式 `UnitLit` 参数调用
     - `f()`（当选中 callee 是单一 `Unit` 参数）-> 一个显式 `UnitLit` 参数调用
     - `f(())` 保持为一个显式 `UnitLit` 参数调用
     - 这三者在 typed HIR 中应合流成同一种 canonical call shape。
  5. 若现有 typecheck/continuation resume 逻辑里仍有“tuple payload 展开成多个 positional args / named args”的特殊 resume 规则，则 refactor 路径必须停止依赖它。
     - 对 tuple resume payload，源码层应通过普通 tuple 值作为**单一实参**传递；
     - 例如需要 `(a, b)` 的地方，应当是 `k.resume((a, b))`，而不是 `k.resume(a, b)`。

- 必须遵从的约束：
  - 禁止在 parser / AST 层实现该 sugar；只能在 typed 阶段实现。
  - 禁止在 typed 阶段仅对 `resume` 名字做特殊零参处理；必须是一般 callable 规则。
  - 禁止保留“普通 callable 用一般规则、`resume` 再单独走一套多参数特殊规则”的双轨模式。

- 验证：
  1. 复用/新增 typecheck fixtures，至少覆盖：
     - `tests/fixtures/typecheck/continuation_resume_answer_expression_ok.scoop`
     - 新增 `tests/fixtures/typecheck/continuation_resume_unit_sugar_ok.scoop`
     - 新增 `tests/fixtures/typecheck/unit_single_param_zero_arg_call_ok.scoop`
     - 新增 `tests/fixtures/typecheck/continuation_resume_tuple_requires_single_tuple_arg_is_error.scoop`
  2. 新增/更新 HIR dump 样本，至少覆盖：
     - `tests/fixtures/hir/continuation_resume_surface_named_tuple_and_unit_basic.scoop`
       - 要求包含 `k.resume()`、`k.resume(())`、以及单一 `Unit` 参数普通函数调用
       - 要求 typed HIR canonical 输出中这些调用都落成一个显式 `UnitLit` 参数调用形状
  3. 运行：
     - `cargo test -p scoopc continuation_resume`
     - `cargo test -p scoopc unit_single_param_zero_arg`
     - `cargo run -p scoop -- --effect-pipeline refactor test --fixtures tests/fixtures/typecheck/continuation_resume_unit_sugar_ok.scoop`
     - `cargo run -p scoop -- --effect-pipeline refactor test --fixtures tests/fixtures/typecheck/unit_single_param_zero_arg_call_ok.scoop`
     - `cargo run -p scoop -- --effect-pipeline refactor test --fixtures tests/fixtures/typecheck/continuation_resume_tuple_requires_single_tuple_arg_is_error.scoop`
     - `cargo run -p scoop -- --effect-pipeline refactor dump-hir tests/fixtures/hir/continuation_resume_surface_named_tuple_and_unit_basic.scoop`

- 完成条件：
  - `Continuation` surface 声明与设计文档对齐；
  - 单一 `Unit` 参数 sugar 已在 typed 阶段 canonicalize；
  - `resume` 不再依赖 AST 特例或 variadic resume 特殊规则。
- 依赖：P2-T01R
- 完成记录：
  - （执行时填写）

## P2-T02R：Review `Continuation` surface 与 typed sugar，确认零参 sugar 没有污染 AST 和 parser

- 参考：
  - [`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md) §5.3.1
  - [`PLAN.md`](./PLAN.md) §2/P2
- 重点：
  - `sysroot/core.scoop` 中 `Continuation` 的 surface 声明与注释是否已经和设计文档一致；
  - `k.resume()` / `f()` 的 typed desugar 是否只发生在 typed 阶段；
  - refactor 路径是否已经禁止 tuple resume 的多参数特殊 surface。
- 必须检查的文件/位置：
  - `sysroot/core.scoop`
  - refactor HIR/typecheck stage 模块
  - 任何承载 typed call / typed desugar 的新共享 API
  - 现有 `typecheck/expr/call.rs` 中与 `Continuation.resume(...)` 相关的 legacy 逻辑

- 验证：
  - 重新运行 P2-T02 的全部测试与命令；
  - 额外搜索以下模式，确认 refactor 路径不再依赖 resume 多参数特例：
    - `rg "k\.resume\(a0|expanded_payload_param_names|legacy_value_expr|Continuation\.resume payload" crates/scoopc/src sysroot`
  - 若旧逻辑仍保留在 legacy 路径，必须在完成记录中明确说明“新路径未再调用它”；若新路径仍依赖这些特例，则必须修复。

- 完成条件：
  - review 能明确说明：typed sugar 已按一般 callable 规则收口，不会污染 AST/parser，也不会在新路径继续保留 resume 特例；
  - 可进入 P2-T03。
- 依赖：P2-T02
- 完成记录：
  - （执行时填写）

## P2-T03：落地 `Continuation` typed 语义、runtime error 的普通 effect 传播，以及 compiler-owned interface 约束

- 参考：
  - [`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md) §4.1, §5.3.1, §5.3.9
  - [`PLAN.md`](./PLAN.md) §2/P2
  - 当前实现参考：`crates/scoopc/src/typecheck/expr/call.rs` 中 `try_infer_continuation_resume_call_expr_type(...)` 一带
- 目标：
  - 让 refactor typed HIR / typecheck 真正理解 `Continuation<ResumeTuple, Answer, eff Out>` 的语义；
  - `resume` 的参数、返回值、required effects 都进入显式 typed contract；
  - `ContinuationAlreadyResumed` 等 runtime error 在语言层继续视作普通 `Raise<RuntimeError>` 分支，而不是第二条隐藏错误通道；
  - `Continuation` 虽然源码可见，但必须保持 compiler-owned：用户不能自己实现/伪造/运行时构造它。

- 必须实现的内容：
  1. 在 refactor typecheck 路径中，显式建模 `Continuation<ResumeTuple, Answer, eff Out>` 的 receiver contract。
     - `resume(value)` 的静态返回类型必须来自 `Answer`；
     - `resume(value)` 的实参类型必须来自 `ResumeTuple`；
     - safe-call `receiver?.resume(...)` 若当前新路径继续支持，应在返回类型上保持与普通 member call 一致的 `Option<Answer>` 包装。
  2. 显式建模 runtime error 的 ordinary effect 传播。
     - 当前阶段要求：`resume(...)` 的 required effects 必须显式包含 ordinary `Raise<RuntimeError>` 传播；
     - 这不是第二条隐藏错误通道，而是普通 effect row 贡献；
     - 若未来设计要把 `Out` 本身定义为“已包含 runtime error 的完整 row”，必须先回写 `EFFECT_REFACTOR.md`，再改实现；在本任务中，先以当前 typecheck 主线的 `Out + Raise<RuntimeError>` 规则为准，并保证它被显式记录，而不是暗箱处理。
  3. 把 `Continuation` 的 compiler-owned 约束落到 typecheck：
     - 用户不得自己 `class X : Continuation<...>` / `type Y : Continuation<...>` / 等价实现它；
     - 用户不得直接 runtime construct `Continuation<...>()` 或等价实例化它；
     - 若语言 surface 缺少“sealed interface”机制，则必须用 typecheck 显式拒绝这些使用方式。
  4. 把 refactor typed HIR 输出中的 continuation/effect surface contract 显式化。
     - 至少要能在 typed HIR stage 输出中得到：
       - 某个 `resume` site 的 `ResumeTuple`
       - `Answer`
       - `Out`
       - required effects 是否包含 `Raise<RuntimeError>`
     - 本任务不要求落地到最终 `MaterializedEffectFacts`；但必须确保 P3 不需要回 AST/typecheck 猜这些关系。

- 必须遵从的约束：
  - 禁止继续把 `Continuation.resume(...)` 当成“没有正式 surface contract 的内建魔法函数”。
  - 禁止通过特殊隐藏错误通道实现 runtime error；必须让 typecheck / typed HIR 能显式看见 `Raise<RuntimeError>`。
  - 禁止只在 codegen 阶段才拒绝“用户实现/构造 Continuation”；这条约束必须在 typecheck 阶段就成立。

- 验证：
  1. 复用并更新现有 typecheck fixtures：
     - `tests/fixtures/typecheck/continuation_resume_answer_expression_ok.scoop`
     - `tests/fixtures/typecheck/continuation_answer_type_mismatch_is_error.scoop`
     - `tests/fixtures/typecheck/continuation_resume_in_pure_main_after_handle_is_error.scoop`
     - `tests/fixtures/typecheck/continuation_resume_from_escape_binder_requires_step_effect.scoop`
  2. 新增 typecheck fixtures：
     - `tests/fixtures/typecheck/continuation_user_impl_is_error.scoop`
     - `tests/fixtures/typecheck/continuation_runtime_ctor_is_error.scoop`
     - `tests/fixtures/typecheck/continuation_resume_requires_runtime_error_effect_is_error.scoop`
       - 目标：锁定 `Raise<RuntimeError>` 仍是 ordinary required effect 的一部分
  3. 运行：
     - `cargo run -p scoop -- --effect-pipeline refactor test --fixtures tests/fixtures/typecheck/continuation_resume_answer_expression_ok.scoop`
     - `cargo run -p scoop -- --effect-pipeline refactor test --fixtures tests/fixtures/typecheck/continuation_answer_type_mismatch_is_error.scoop`
     - `cargo run -p scoop -- --effect-pipeline refactor test --fixtures tests/fixtures/typecheck/continuation_resume_in_pure_main_after_handle_is_error.scoop`
     - `cargo run -p scoop -- --effect-pipeline refactor test --fixtures tests/fixtures/typecheck/continuation_resume_from_escape_binder_requires_step_effect.scoop`
     - `cargo run -p scoop -- --effect-pipeline refactor test --fixtures tests/fixtures/typecheck/continuation_user_impl_is_error.scoop`
     - `cargo run -p scoop -- --effect-pipeline refactor test --fixtures tests/fixtures/typecheck/continuation_runtime_ctor_is_error.scoop`
     - `cargo run -p scoop -- --effect-pipeline refactor test --fixtures tests/fixtures/typecheck/continuation_resume_requires_runtime_error_effect_is_error.scoop`
  4. 新增/更新以 `refactor_continuation_typecheck_*` 命名的单元测试，并运行：
     - `cargo test -p scoopc refactor_continuation_typecheck`

- 完成条件：
  - refactor typecheck 已经对 `Continuation` / `resume` / runtime error ordinary effect 形成显式 contract；
  - 用户不能实现或构造 `Continuation`；
  - P3 不再需要回到 typecheck 推断 continuation surface 语义。
- 依赖：P2-T02R
- 完成记录：
  - （执行时填写）

## P2-T03R：Review continuation typed 语义，确认没有残留隐藏通道或 legacy 魔法

- 参考：
  - [`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md) §5.3.1, §5.3.9
  - [`PLAN.md`](./PLAN.md) §2/P2
- 重点：
  - `Continuation.resume(...)` 是否已具备显式的参数/返回/effect contract；
  - runtime error 是否确实以 ordinary `Raise<RuntimeError>` 的形式进入 typed 语义；
  - 是否已经拒绝用户实现/构造 `Continuation`。
- 必须检查的文件/位置：
  - refactor typed HIR stage 模块
  - refactor 专属 typecheck 逻辑
  - `sysroot/core.scoop`
  - 任何仍残留在 legacy `typecheck/expr/call.rs` 的 `Continuation.resume(...)` 特殊逻辑

- 验证：
  - 重新运行 P2-T03 的全部 fixtures 与单元测试；
  - 额外搜索：
    - `rg "ContinuationAlreadyResumed|Raise<RuntimeError>|Continuation<" crates/scoopc/src sysroot`
  - 要求：
    - 能清楚指出 refactor 路径在哪里显式处理了 runtime error ordinary effect 传播；
    - 若 legacy 路径仍保留旧实现，必须在完成记录中说明新路径已经不依赖它。

- 完成条件：
  - review 能明确说明：P2 typed 语义已经把 continuation/runtime error contract 说清楚，不再靠 legacy 特例或 codegen 兜底；
  - 可进入 P2-T04。
- 依赖：P2-T03
- 完成记录：
  - （执行时填写）

## P2-T04：输出 typed HIR effect/continuation side tables，并锁定 `dump-hir` / typecheck 验证矩阵

- 参考：
  - [`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md) §4.13.1, §4.13.3, §5.4 的早期对应物
  - [`PLAN.md`](./PLAN.md) §2/P2，§2/P3
  - 当前 HIR side table 参考：`crates/scoopc/src/hir/mod.rs` 中 `CallSite` / `EffectOpCallSiteIndex` / `ContinuationResumeCallSiteIndex` 等定义，以及 `hir/lower/mod.rs` 中对这些表的收集
- 目标：
  - 在 P2 产物中显式提供一份“typed HIR effect/continuation contract side tables”；
  - 让 P3 直接消费这些 side tables，而不需要再通过 span 回到 AST/typecheck 查询；
  - 用 refactor `dump-hir` 与 typecheck fixtures 把这些 typed contract 锁定下来。

- 必须实现的内容：
  1. 为 refactor typed HIR 输出增加一个 side-table 容器。
     - 名称可自定，例如 `TypedHirEffectContracts`；
     - 但必须显式挂在 P2 stage 输出上，而不是散落在 session 私有缓存里。
  2. 该 side-table 容器至少要包含：
     - per-function `allowed_row` / required effects contract
     - per-`resume` site contract
       - `ResumeTuple`
       - `Answer`
       - `Out`
       - runtime error ordinary effect 贡献
     - per-`perform` site contract
       - concrete op / performed effect / payload 类型
     - per-`handle` site contract
       - handle body result type
       - arm effect/result typed 关系
     - 若调用 site 在 P2 就已经能区分 direct/effect-op/continuation resume 等模式，则也应显式记录，避免 P3 再猜。
  3. 若现有 HIR side table（如 `ContinuationResumeCallSiteIndex`）只有“这个 span 是 resume site”这种布尔级信息，则在 refactor 路径必须把它升级为**结构化 contract**，而不只是集合标记。
  4. 为 refactor typed HIR 增加稳定的 debug/dump 输出能力。
     - 允许继续打印主体 `hir::File` 的 Debug；
     - 但必须另外提供一种方式看到上述 typed side tables，供测试与后续阶段验证；
     - 推荐：在 refactor `dump-hir` 输出中追加一个稳定的 side-table debug 区块，或为 Rust tests 提供稳定 formatter。
  5. 新增/更新 HIR 级 golden / snapshot 覆盖，至少覆盖：
     - continuation `resume` surface
     - runtime error ordinary effect 传播
     - `perform` / `handle` 的 typed contract

- 必须遵从的约束：
  - 禁止把这些 typed contract 只留在 `TypeLowering` / infer 的临时局部状态中，而不输出到 P2 产物。
  - 禁止用“后续 P3 可以再回 typecheck 算一次”为理由跳过 side-table 输出。
  - 禁止只输出布尔标记而不输出下游真正需要的结构化 typed contract。

- 验证：
  1. 新增/更新 HIR snapshot / unit tests，推荐命名：
     - `refactor_typed_hir_continuation_contract_dump_*`
     - `refactor_typed_hir_handle_contract_dump_*`
  2. 新增/更新 HIR 样本，推荐至少包括：
     - `tests/fixtures/hir/continuation_resume_surface_named_tuple_and_unit_basic.scoop`
     - `tests/fixtures/hir/continuation_runtime_error_surface_basic.scoop`
     - `tests/fixtures/hir/handle_perform.scoop`（可复用现有样本，但必须在 refactor typed dump 中看到新的 typed contract 信息）
  3. 运行：
     - `cargo test -p scoopc refactor_typed_hir`
     - `cargo run -p scoop -- --effect-pipeline refactor dump-hir tests/fixtures/hir/continuation_resume_surface_named_tuple_and_unit_basic.scoop`
     - `cargo run -p scoop -- --effect-pipeline refactor dump-hir tests/fixtures/hir/continuation_runtime_error_surface_basic.scoop`
     - `cargo run -p scoop -- --effect-pipeline refactor dump-hir tests/fixtures/hir/handle_perform.scoop`
  4. 保持 legacy 路径不变，额外抽样验证：
     - `cargo run -p scoop -- --effect-pipeline legacy dump-hir tests/fixtures/hir/handle_perform.scoop`

- 完成条件：
  - P2 产物已经包含后续 P3 可直接消费的 typed HIR effect/continuation side tables；
  - `dump-hir --effect-pipeline refactor` 已能稳定显示这些 contract；
  - 本阶段可结束并进入 P3。
- 依赖：P2-T03R
- 完成记录：
  - （执行时填写）

## P2-T04R：Review P2 阶段退出条件，确认 P3 不再需要回 AST/typecheck 猜语义

- 参考：
  - [`PLAN.md`](./PLAN.md) §2/P2，§3
  - [`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md) §4.13, §5.3.1, §5.3.9
- 重点：
  - refactor typed HIR stage 是否已经独立存在；
  - `Continuation` / `resume` / runtime error 的 typed contract 是否已完整显式化；
  - typed HIR side tables 是否已足够支撑 P3，不必回 AST/typecheck 猜语义；
  - `dump-hir` refactor 路径是否已经能稳定展示这些 contract。

- 验证：
  - 重新运行 P2-T01 ~ P2-T04 的所有定向测试与命令；
  - 再跑一次：
    - `cargo test -p scoop`
    - `cargo test -p scoopc`
  - 不执行 full regression。

- 完成条件：
  - review 能明确说明：P2 已经完成“typed HIR / typecheck 新路径落地”的阶段目标；
  - P3 可以在不重新讨论 continuation surface、runtime error 语义、或 AST sugar 的前提下直接进入 direct-style MIR 新路径实现。
- 依赖：P2-T04
- 完成记录：
  - （执行时填写）
