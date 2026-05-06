# Scoop：MIR Gap 收口计划

> 生成时间：2026-05-06  
> 差距基线：[`PIPELINE_GAPS.md`](./PIPELINE_GAPS.md)  
> 格式参考：[`PLAN-effect-refactor.md`](./PLAN-effect-refactor.md)  
> 本轮主题：第一阶段只收口 refactor 新路径上的 MIR gaps，让 HIR -> direct-style MIR -> materialized MIR handoff 成为后续 facts / late lowering / codegen 可以信任的完整输入。

## 0. 工作原则

- 本阶段只修新 effect-refactor 路径。legacy HIR/MIR/codegen 路径可保持现状，后续按 deprecation 计划整体删除。
- 本阶段的目标不是让所有后端都通过，而是让 MIR stage 自身闭包：任何 refactor production MIR 输出都不得包含 `Item::Todo`、`StatementKind::Todo`、`Rvalue::Todo`、`TerminatorKind::Todo`、`UnwindAction::Todo` 或等价占位。
- 所有 spec 已支持且 parser/typecheck 接收的 surface，都必须在 refactor MIR 中有明确表示、metadata 或 materialized contract。
- 任何当前不准备支持的 surface，必须在进入 refactor HIR/MIR 前被 parser 或更早的 frontend gate 拒绝，并给出清晰诊断；不得通过 HIR `Todo(...)` 或 MIR `Todo(...)` 继续向后流。
- 若拒绝条件依赖类型或解析结果，必须在 resolver/typecheck/comptime handoff 阶段 fail fast；但 MIR stage 仍必须把它视为输入不合法，而不是生成占位。
- 本阶段不解决 later-stage LLVM/runtime gaps，但必须把 later-stage 需要的 MIR 语义合同发布完整。例如 runtime cast、dynamic dispatch、aggregate transport、closure env、generic/effect args 可以仍由后续 codegen 实现，但 MIR 不能缺失身份、类型、payload、layout intent 或 source span。
- production verifier 和 dump/debug verifier 必须分离。debug/legacy dump 可以继续容忍历史占位；refactor production MIR stage 与 materialized snapshot 必须 strict no-placeholder。
- 新实现不得在旧 `mir::*` 业务函数中加入 pipeline 分支。若共享代码不能成为完全中立的单一 API，应在 refactor stage 附近建立独立 wrapper、strict verifier 或复制实现。
- MIR stage 输出必须对后续阶段语义闭包。P4/P5/P6 只能消费 refactor MIR stage output、materialized MIR snapshot、MIR metadata/facts 以及 target/session config；不得回看 AST/HIR 私有 side table 来补语义。
- 本阶段不要求 full fixtures，因为后续 codegen/runtime 仍有缺口。验证以 parser/typecheck diagnostics、HIR/MIR preflight、`dump-mir` golden、MIR unit tests、materialization unit tests 和少量定向 fixture smoke 为主。

## 1. 顺序总览

1. M0：MIR gap inventory 与 strict gate 设计冻结。
2. M1：refactor production MIR verifier 与 diagnostic 边界落地。
3. M2：frontend/HIR placeholder 入口收口，保证 unsupported surface 不进入 MIR。
4. M3：program item graph 与 top-level roots MIR 化。
5. M4：统一 place/lvalue 与 statement lowering 收口。
6. M5：call/ctor/dispatch/resume/perform/handle typed contract 收口。
7. M6：MIR value primitive 与 spec-supported runtime surface 收口。
8. M7：generic/materialized MIR 完整性收口。
9. M8：MIR-only 验证矩阵与阶段退出审计。

执行顺序调整（2026-05-06）：`MIR-T04` 的指定 splice-field `dump-mir` 验证依赖 top-level `const val`/`val` 不再生成 MIR item placeholder；因此 `MIR-T05` 先于 `MIR-T04` 执行，完成 top-level roots 后再关闭剩余 M2 surface。

## 2. 分阶段计划

### M0. MIR gap inventory 与 strict gate 设计冻结

参考：[`PIPELINE_GAPS.md`](./PIPELINE_GAPS.md) §1、§2、§8。

目标：

- 把当前所有可能进入 refactor MIR 的 placeholder、fallback、late diagnostic 明确归类。
- 明确哪些 gap 必须在 MIR stage 实现，哪些必须在 frontend 拒绝，哪些只需要 MIR 发布后续 stage 所需 contract。
- 在写代码前固定 strict production MIR 的定义，避免继续用“dump-only MIR”标准衡量新路径。

实现：

- 建立 refactor MIR placeholder inventory，覆盖：
  - HIR-origin placeholders：`comptime_*`、`splice_field`、`class_lit`、`with_update`、`structured_concurrency_*`、`missing_expr`。
  - MIR-origin placeholders：top-level `Item::Todo`、assign place fallback、call/ctor/dispatch/resume fallback、perform/handle contract missing、boxed var init pending、`break/continue not in loop`。
  - materialization placeholders：materializer 当前 no-op 透传的 statement/terminator/rvalue Todo。
- 为每个 inventory entry 指定 disposition：
  - `ImplementInMir`：合法 surface 必须 lower 成 MIR。
  - `ImplementBeforeMir`：合法 surface 必须在 comptime/HIR typed handoff 展开成普通 MIR 输入。
  - `RejectBeforeMir`：延期或不支持 surface 必须由 parser/frontend diagnostic 拒绝。
  - `LegacyOnly`：只允许旧路径保留，不得进入 refactor production path。
- 将 inventory 与 `PIPELINE_GAPS.md` 的 gap 编号建立映射，后续 TODO 以该映射作为审计基线。

阶段输出：

- 一份可执行的 placeholder inventory 或等价测试。
- 一份 strict production MIR gate 规则，供 M1 实现。

验证：

- `cargo test -p scoopc --no-default-features refactor_mir_placeholder_inventory`
- 搜索 `Todo("`、`Item::Todo`、`StatementKind::Todo`、`Rvalue::Todo`、`TerminatorKind::Todo`、`UnwindAction::Todo` 的新增位置必须先更新 inventory。
- 不执行 full fixtures。

完成条件：

- 所有已知 MIR gaps 都有唯一 owner task 和处理策略。
- 后续任务不得再把新增 placeholder 当成临时实现塞进 refactor production path。

### M1. refactor production MIR verifier 与 diagnostic 边界落地

参考：[`PIPELINE_GAPS.md`](./PIPELINE_GAPS.md) §2.1、§2.2、§2.3、§2.4。

目标：

- 把“无 Todo / 无隐式缺值 / 无裸 generic param”的要求前移到 MIR stage 和 materialized snapshot。
- 让 refactor MIR stage 在边界处 fail fast，带 source span、body FQN、placeholder category 和建议修复阶段。

实现：

- 新增或扩展 strict verifier，例如 `validate_refactor_production_mir(...)`。
- verifier 必须拒绝：
  - `Item::Todo`。
  - 任意 executable body 中的 `StatementKind::Todo`、`Rvalue::Todo`、`TerminatorKind::Todo`、`UnwindAction::Todo`。
  - 非 `Unit` 函数的 `Return { value: None }`。
  - 缺 source span / site id / typed metadata 的 effect-sensitive `Call`、`Perform`、`Handle`、`Resume`。
  - materialized MIR 中未替换的 `TypeKind::Param`、effect-row param 或 missing template root。
- 保留现有 `validate_refactor_direct_style()` 作为 CFG/site 形状校验，但 production stage 必须在其后追加 strict no-placeholder 校验。
- 将 `effect_refactor_pipeline::mir_stage::run(...)` 接到 strict verifier。
- 将 materializer 输出接到同一 strict verifier 或 materialized 专用 verifier。
- 所有 verifier error 必须映射到 clear diagnostic，不允许继续让 LLVM raw codegen 报 `pass MIR ... todo`。

阶段输出：

- refactor production MIR stage 的 no-placeholder gate。
- materialized MIR snapshot 的 no-placeholder/no-param gate。
- 可被 CLI、unit tests、preflight 复用的诊断格式。

验证：

- `cargo test -p scoopc --no-default-features refactor_mir_no_todo`
- `cargo test -p scoopc --no-default-features refactor_materialized_mir_no_todo`
- 构造最小负例验证每类 Todo、`Return None`、裸 type param 都在 MIR stage 或 materializer stage fail fast。
- 不执行 `cargo run -p scoop -- test` 全量。

完成条件：

- refactor stage 不再可能产出含 placeholder 的 production MIR。
- materialized MIR 不再把模板缺口或占位克隆到下游。

### M2. frontend/HIR placeholder 入口收口

参考：[`PIPELINE_GAPS.md`](./PIPELINE_GAPS.md) §1.1、§1.2、§1.3、§1.5、§1.12、§7.4。

目标：

- 清理所有会从 AST/HIR 漏到 MIR 的 placeholder 来源。
- 对 spec 支持的 surface，在 comptime/HIR typed handoff 完成展开或发布 contract。
- 对延期 surface，在 parser/frontend 拒绝，确保不进入 refactor MIR。

实现：

- `comptime block/if/for` 和 package-level `comptime if`：
  - 在 runtime HIR 进入 MIR 前完成 expansion/elimination。
  - 未能静态求值时给出 comptime diagnostic，不生成 HIR/MIR Todo。
- `value.[field]` splice field：
  - 在 comptime/typecheck 阶段把字段名解析为 concrete member access。
  - 无法静态解析时 fail fast。
- class literal：
  - 若 runtime `T::class` 属于本阶段支持 surface，则 lower 为明确的 MIR value primitive 或 type metadata constant。
  - 若只允许 annotation/comptime 消费，则 parser/frontend 对 runtime class literal 报错，不生成 `class_lit` Todo。
- `with` copy-update：
  - 强制 typed handoff 发布 aggregate kind、base value、update path、field/value types、enum/tuple/struct path。
  - HIR fallback `with_update` 在 refactor path 禁止。
- structured concurrency `spawn` / user-facing `join`：
  - 按当前 spec deferred surface，在 parser/frontend feature gate 给出明确 diagnostic。
  - `async`/`await` 已支持的 sugar 继续 lower 到 effect contract，不受影响。
- parser recovery `Missing`：
  - refactor production path 把 parser recovery 当成 parse error，而不是 `missing expr` MIR value。

阶段输出：

- refactor typed HIR stage 不再含必须消除的 placeholder。
- HIR preflight 中原先 `HirOnly` 的合法样本逐步升级为 MIR smoke。

验证：

- `cargo test -p scoopc --no-default-features refactor_hir_placeholder_inventory`
- `cargo test -p scoopc --no-default-features refactor_hir_preflight`
- 定向 `dump-mir --effect-pipeline refactor` 覆盖 comptime、splice field、class literal、with update。
- Parser/frontend 负例覆盖 `spawn`、`join`、未解析 splice、runtime class literal 禁用形态。

完成条件：

- refactor HIR 输入不再通过 `Todo(...)` 表达“以后再说”。
- 所有合法 HIR completeness fixtures 至少能进入 direct-style MIR smoke，除非该 surface 被 parser/frontend 明确拒绝。

### M3. program item graph 与 top-level roots MIR 化

参考：[`PIPELINE_GAPS.md`](./PIPELINE_GAPS.md) §1.4、§1.5、§2.5、§6.4。

目标：

- 让 MIR file 成为完整 program IR，而不是只包含函数 body。
- 消除 top-level `val`、typealias、type/object declaration 通过 `Item::Todo` 或 HIR side table 隐式存在的问题。

实现：

- 为 top-level immutable value / const / runtime initializer 设计 MIR item 或 synthetic initializer root。
- 建立 top-level init dependency ordering 和 hidden ordinary effect contract。
- 为 object init、companion/member init、type metadata、typealias resolved form 发布 MIR-level declaration item 或 metadata item。
- 为 `@Extern` global variable 发布 MIR extern storage model：symbol name、linkage、TLS flag、initializer absence、unsafe access contract。
- 确保 member fun、extension fun、generic callable、object/member initializer 都进入 canonical MIR root index。

阶段输出：

- `MirFile` 不再用 `Item::Todo` 表示任何 refactor production declaration。
- P4/P5 可以从 MIR stage output 查询所有 callable root、initializer root、extern/global root 和 metadata root。

验证：

- `cargo test -p scoopc --no-default-features refactor_mir_item_graph`
- `dump-mir --effect-pipeline refactor` 覆盖 top-level val、object init、typealias/type/object declaration、extern global。
- materialized root index 单测覆盖 generic member/extension/object roots。

完成条件：

- 后续阶段无需回 HIR side table 才能发现 top-level init、object init、extern global 或 generic callable template。

### M4. 统一 place/lvalue 与 statement lowering 收口

参考：[`PIPELINE_GAPS.md`](./PIPELINE_GAPS.md) §1.6、§7.5。

目标：

- 把 assignment LHS 从临时 shape matching 改为 typed place contract。
- 所有 typecheck 接受的 assignable place 都 lower 成 MIR place/store，不再有 `assign lhs lowering pending` 等 fallback。

实现：

- 定义 unified MIR place model，至少覆盖 local、boxed local、top-level var、member field/property、tuple/struct field、enum payload path、index place、future-safe deref/pin handle 的拒绝边界。
- refactor typed HIR 必须为每个 assignment 发布 authoritative place contract。
- MIR lowering 只消费 place contract，不再从 HIR expr shape 猜 LHS。
- 对 parser 语法允许但当前不支持的 assignable syntax，parser/frontend 给出明确 unsupported diagnostic。
- 修正 `boxed var decl init pending`：无 initializer 的 boxed mutable local 要么有明确 default/init contract，要么由 frontend 拒绝。
- `break not in loop`、`continue not in loop` 应作为 parser/frontend control-flow diagnostic，不得进入 MIR terminator Todo。

阶段输出：

- refactor MIR statement lowering 不再含 assignment/place fallback Todo。
- Store statements 携带完整 value type、receiver type、member/index/path metadata 和 continuation route provenance。

验证：

- `cargo test -p scoopc --no-default-features refactor_mir_place_contract`
- `dump-mir --effect-pipeline refactor` 覆盖 local/global/member/index/tuple/struct/enum path assignment。
- 负例覆盖 unsupported LHS、非法 break/continue、缺 initializer boxed var。

完成条件：

- assignment 相关占位全部从 refactor production MIR 中消失。

### M5. call/ctor/dispatch/resume/perform/handle typed contract 收口

参考：[`PIPELINE_GAPS.md`](./PIPELINE_GAPS.md) §1.7、§1.8、§1.9、§1.10、§1.11、§2.6、§3.9、§3.10、§6.3、§6.5。

目标：

- 让所有 call-like syntax 由 typed call-site contract 驱动 MIR lowering。
- 消除 callee provenance、ctor binding、dispatch owner/member、resume shape、perform/handle contract missing 等 fallback。

实现：

- typecheck/refactor HIR handoff 为每个 call site 发布 bound argument mapping：positional/named/default/vararg、receiver、selected overload、selected ctor、generic type/effect args。
- MIR `CallKind` 必须从 authoritative binding 得出：direct、closure、fun value、virtual、interface、resume、intrinsic、constructor。
- constructor MIR 必须携带 selected ctor、ordered complete args、hidden effects、default/named args 展开结果。
- dynamic dispatch metadata 必须来自 structured owner/member binding，不允许 `rsplit_once('.')` 恢复语义。
- `Continuation.resume` 必须由 typed resume site contract 发布 receiver、resume tuple、answer、out effects、runtime error effect；MIR 不再要求 canonical callee shape。
- `perform` / `handle` 缺 typed contract 时直接 MIR stage diagnostic，不生成 Todo terminator/rvalue。
- effect-row generic call 的 effect args 必须进入 call-site instance key 和 materialization key。
- runtime fallback intrinsics `sizeOf<T>()`、`nameOf<T>()`、`getPlatform()` 在 MIR 中有明确 intrinsic rvalue/call metadata；若某 intrinsic 不支持 runtime fallback，则 frontend 拒绝。
- interface default method dispatch 的 selected implementation/default slot 必须在 MIR call metadata 中可见。

阶段输出：

- call-like MIR 不再依赖 `ValueOrigin::UnknownCallable` 的 guess 才能选择语义。
- P4/P5/P6 可以仅凭 MIR call/site metadata 做 facts、late lowering 和 backend routing。

验证：

- `cargo test -p scoopc --no-default-features refactor_mir_call_contract`
- `dump-mir --effect-pipeline refactor` 覆盖 direct/fun-value/closure/member/default/named/generic/ctor/virtual/interface/resume/perform/handle/intrinsic。
- materialization 单测覆盖 effect-row generic direct-call instance key。

完成条件：

- `call callee lowering pending`、`ctor call lowering pending`、`dispatch callee lowering pending`、`resume lowering requires canonical callee shape`、`refactor perform/handle contract missing` 不再可能出现在 refactor production MIR。

### M6. MIR value primitive 与 spec-supported runtime surface 收口

参考：[`PIPELINE_GAPS.md`](./PIPELINE_GAPS.md) §3.4、§3.5、§3.7、§3.8、§3.11、§3.12、§4、§6.1、§6.2。

目标：

- 让 spec/typecheck 已支持的 runtime value surface 在 MIR 中有完整、typed、可 materialize 的表示。
- 不要求本阶段完成 LLVM lowering，但 MIR 表达不能丢语义或用默认值占位。

实现：

- `is` / `!is` / `as` / `as?`：
  - MIR `Rvalue::TypeCheck` / `Rvalue::Cast` 必须携带 runtime type descriptor key、target type、failure effect、`as?` Option result contract。
  - parameterized class/interface matching 所需 type args/itable query contract 必须在 MIR metadata 中可查询。
- `!!` 非空断言：
  - MIR 表达 `Some -> value`、`None -> Raise<RuntimeError.NullAssertionFailed>` 的 control/effect contract。
  - `Nothing`/raise arm coercion 在 MIR type contract 中明确。
- pattern `is Type`：
  - MIR pattern metadata 区分静态可折叠 value type、runtime ref/interface/class type test、unsupported function type cast 的 frontend reject。
- function value / closure / top-level function ref：
  - MIR 明确 function reference normalization，是 closure/function value object 还是 symbol value。
  - closure env type 支持 arbitrary source type，并发布 capture boxing/mutable capture contract。
- aggregate transport：
  - MIR 层发布 tuple/struct/enum boxing intent、array element layout intent、copy/drop/trace requirements、effect payload/resume payload carrier shape。
  - enum payload、Unit field、大整数 payload、nested aggregate payload 在 MIR layout intent 中不丢信息。
- Array get/set、copy-update、aggregate literal 在 MIR 中不退化为 HIR builder guess；每个 element/value path 有 source type 和 transport contract。

阶段输出：

- 后续 codegen 可能仍缺实现，但它缺的是 backend lowering，不是 MIR 信息。
- raw/refactor backend 不再需要回 HIR 迁移 runtime cast、not-null、aggregate/closure/env 语义。

验证：

- `cargo test -p scoopc --no-default-features refactor_mir_value_primitives`
- `dump-mir --effect-pipeline refactor` 覆盖 typecheck/cast/not-null/pattern/closure/aggregate/array/enum payload。
- 不运行对应 run-pass 全量；只验证 MIR shape、metadata 和 no-placeholder。

完成条件：

- spec-supported runtime value surface 在 MIR 中语义完整。
- 不支持的 function type cast/effectful function type cast 等 surface 在 frontend 被明确拒绝。

### M7. generic/materialized MIR 完整性收口

参考：[`PIPELINE_GAPS.md`](./PIPELINE_GAPS.md) §2.2、§2.5、§2.6、§2.7、§2.8。

目标：

- 让 materialized MIR snapshot 可以作为后续 facts/late lowering/codegen 的 canonical monomorphic input。
- 消除 missing template、missing root、effect arg inference failure、裸 type/effect param 漏出等问题。

实现：

- 所有 generic callable，包括 top-level、member、extension、constructor、object/member side-table root，都必须有 canonical template key。
- `InstanceKey` 包含 type args、effect-row args、callable version、receiver/owner identity。
- materializer 重写所有 MIR metadata：call kind、dispatch metadata、perform/handle/resume metadata、cast/typecheck target、aggregate transport、closure env、top-level roots。
- 对允许 erased carrier 的 resume surface 明确标记；其它 source value/frame slot/call arg/return surface 禁止裸 `TypeKind::Param`。
- materializer 遇到 Todo 或不完整 substitution 立即 error，不再 no-op。
- materialized snapshot 运行 strict verifier。

阶段输出：

- `MaterializedMir` 对 refactor production path no Todo、no unresolved generic param、no missing root。
- P4/P5 只消费 materialized snapshot，不再绕回 generic HIR/template side table。

验证：

- `cargo test -p scoopc --no-default-features refactor_mir_materialize_generics`
- 定向 fixtures 覆盖 generic function/member/extension/ctor、effect-row generic call、generic effect payload、resume erased-carrier exception。
- 负例覆盖 missing template、missing root、裸 param leak。

完成条件：

- materialized MIR snapshot 的完整性由 stage gate 强制保证。

### M8. MIR-only 验证矩阵与阶段退出审计

参考：[`PIPELINE_GAPS.md`](./PIPELINE_GAPS.md) §9。

目标：

- 用 MIR-only 验证证明第一阶段完成，而不被 later-stage LLVM/runtime gaps 阻塞。
- 将所有 HIR/MIR gap 的代表样本纳入 refactor MIR preflight 或 golden。

实现：

- 建立 `mir_refactor` fixture/golden 矩阵，覆盖：
  - comptime expansion、splice field、class literal policy、with update。
  - top-level val/object/typealias/extern global roots。
  - assignment place、control-flow、cleanup/finally。
  - call/default/named/generic/ctor/dispatch/resume/perform/handle/intrinsics。
  - typecheck/cast/not-null/pattern/closure/aggregate/array/enum payload。
  - generic materialization/effect-row args/no naked params。
- 将 `effect_refactor_pipeline::hir_preflight` 从“代表性 MIR smoke”升级为“所有合法 HIR completeness fixtures 都必须通过 MIR no-placeholder smoke”。
- 为 parser/frontend reject 负例建立 diagnostics fixtures。
- 建立阶段退出 review，逐项核对 `PIPELINE_GAPS.md` §1/§2 中每个 gap 的状态。

验证：

- `cargo test -p scoopc --no-default-features refactor_hir_preflight`
- `cargo test -p scoopc --no-default-features refactor_mir_no_todo`
- `cargo test -p scoopc --no-default-features refactor_mir_materialize`
- `cargo run -p scoop --no-default-features -- --effect-pipeline refactor test --fixtures tests/fixtures/mir_refactor`
- 若某单个 fixture 需要 CLI smoke，逐个运行，不执行 full fixture suite。
- 明确不要求本阶段运行：
  - `cargo test --all`
  - `cargo run -p scoop -- test`
  - P7/P8 GC/full regression 矩阵

完成条件：

- refactor MIR stage 对所有合法代表样本产出 no-placeholder direct-style MIR。
- materialized MIR snapshot 对所有需要单态化的代表样本 no-placeholder/no-param。
- 所有 unsupported/deferred surface 由 parser/frontend diagnostics 捕获。
- `PIPELINE_GAPS.md` 中 MIR stage 相关 gap 已有测试或明确转为 later-stage backend gap。

## 3. 阶段切换门槛

- M0 未完成前，不允许新增 MIR lowering 逻辑绕过 placeholder inventory。
- M1 未完成前，不进入大规模 feature 收口；否则新实现仍可能被旧 verifier 放行成占位。
- M2 未完成前，不宣称 MIR no-placeholder，因为 HIR 仍可能泄露 placeholder。
- M3 未完成前，后续 facts/late lowering 不得假定 MIR file 是完整 program graph。
- M4 未完成前，不继续扩展 assignment-dependent features。
- M5 未完成前，P4/P5 不得依赖 call/site facts 完整。
- M6 未完成前，后续 backend 不得把 runtime value primitive 缺口误判为 codegen-only 问题。
- M7 未完成前，不把 materialized MIR snapshot 作为 production handoff。
- M8 未完成前，本阶段不算完成。

## 4. 完成标准

本阶段完成时，必须能够明确陈述以下结论全部成立：

1. refactor production MIR stage 输出中不存在 `Todo(...)` 或等价 placeholder。
2. materialized MIR snapshot 中不存在 `Todo(...)`、missing template/root、裸 type/effect param 或非 Unit implicit return。
3. 所有 parser/typecheck 接收的 spec-supported surface 都能进入 MIR，并携带后续阶段所需的 typed contract。
4. 所有延期或当前不支持的 surface 在进入 MIR 前被 parser/frontend diagnostic 拒绝。
5. top-level values、object/type metadata、extern globals、generic callable roots 都能从 MIR stage output 查询。
6. assignment place、call/ctor/dispatch/resume/perform/handle 都由 typed contract 驱动，不再靠 HIR shape/string/span fallback。
7. runtime cast/typecheck/not-null/pattern/function value/closure/aggregate/array/enum payload 在 MIR 中语义完整，即使 LLVM/runtime 实现仍属于后续阶段。
8. 验证矩阵只依赖 MIR/unit/dump/materialization 定向测试，不要求 full fixture suite。
