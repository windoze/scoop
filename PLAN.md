# Scoop：HIR Completeness 收口计划

> 生成时间：2026-05-06
> 现状基线：[`PIPELINE_GAPS.md`](./PIPELINE_GAPS.md)
> 格式参考：[`PLAN-effect-refactor.md`](./PLAN-effect-refactor.md)
> 本轮主题：只在 effect-refactor 新路径上收口 HIR 阶段，使 typed HIR 成为后续 MIR / effect facts / late lowering 可直接消费的完整语义 handoff；旧 legacy 路径可以保持现状，后续整体废弃。

## 0. 工作原则

- 本阶段只修 effect-refactor 新路径。
- 旧 legacy HIR / MIR / codegen 路径不作为本计划的完成对象。
- 若某段现有实现无法以“完全不知道新旧线路”的中立 API 共享，就在 refactor 路径建立独立入口或复制实现，不在旧业务函数中塞 pipeline 分支。
- HIR 阶段输出必须是 production handoff，不再是“尽量保结构、不 panic”的 dump-only IR。
- refactor typed HIR 输出中不得出现任何会流向下一阶段的占位节点：`hir::Item::Todo`、`hir::StmtKind::Todo`、`hir::ExprKind::Todo`，以及等价的 placeholder / fallback sentinel。
- 任何当前语言 surface 已经被 spec 或 fixture 证明应支持的特性，必须在 HIR 中有完整表达或在 typed HIR 前完成规范化。
- 任何本阶段不打算支持的 surface，必须在进入 HIR production 输出前被拒绝。
- 纯语法层即可判定为延期或不支持的 surface，必须由 parser 给出明确错误，例如 structured concurrency `spawn` / user-facing `join`。
- 需要 type information、resolver binding 或 comptime 结果才能判定的 surface，必须由 typecheck / comptime / refactor HIR stage 给出 source diagnostic；但不得生成 HIR Todo。
- HIR stage 只能产出 HIR-level 和 typed contract，不提前修 MIR / LLVM codegen 缺口。
- 本阶段验证不要求运行 full fixtures，因为后续 MIR、late lowering、LLVM 仍有大量缺口。
- 本阶段验证集中在 parser/typecheck/HIR dump/refactor HIR stage verifier/少量 MIR preflight，不执行 `cargo run -p scoop -- test` 全量矩阵。
- 完成标准不是“目标 fixture 能跑到后端”，而是“refactor HIR handoff 对支持 surface 已语义闭包，且不再把 Todo 或缺 contract 推给下一阶段”。

## 1. HIR Gap 范围

本阶段覆盖 `PIPELINE_GAPS.md` 中会在 HIR 阶段直接留下缺口，或会因为 HIR typed contract 不完整而让 MIR lowering 只能生成 Todo 的项目。

直接 HIR Todo 来源：

- `comptime` block / if / for statement：当前降为 `StmtKind::Todo("comptime_*")`。
- package-level `comptime if`：当前降为 `Item::Todo("comptime_if_item")`。
- splice field `value.[field]`：当前降为 `ExprKind::Todo("splice_field")`。
- class literal：当前降为 `ExprKind::Todo("class_lit")`。
- typealias / type / object item：当前降为 `Item::Todo("typealias" / "type" / "object")`。
- array literal fallback：缺 expected type 且无法从元素推断时当前降为 `ExprKind::Todo("array_lit")`。
- spread / named arg 逃逸到普通表达式上下文：当前降为 `ExprKind::Todo("spread_arg" / "named_arg")`。
- structured concurrency `spawn` / user-facing `join`：当前降为 `ExprKind::Todo("structured_concurrency_*_deferred")`。
- assignment expression 逃逸到表达式 lowering：当前降为 `ExprKind::Todo("assign")`。
- `with` copy-update fallback：多处缺 metadata 或 unsupported aggregate 时降为 `ExprKind::Todo("with_update")`。
- missing statement / custom iterator fallback / extension property without getter 等 debug fallback：当前可能降为 `StmtKind::Todo("missing_stmt")`、`StmtKind::Todo("for_custom_iterator")`、`Item::Todo("extension_property_no_getter")`。

必须在 HIR handoff 补齐的 MIR-facing contract：

- top-level `val` / object init / type metadata / alias resolved form，避免下一阶段只能生成 `mir::Item::Todo` 或回读 HIR side table。
- callable callee provenance，包括 direct、closure、fun value、virtual、interface、constructor、extension/member、intrinsic call。
- default / named / spread argument binding，要求 HIR 输出完整 ordered args 或明确诊断。
- dispatch owner/member 结构化 binding，禁止 MIR 用字符串 FQN 拆解恢复 owner/member。
- `Continuation.resume`、`perform`、`handle` site contract，必须由 typed HIR 以 stable site key 发布。
- assignment LHS / lvalue place contract，至少覆盖当前 spec/typecheck 支持的 local、top-level、field、index、safe-member/property setter 等 surface；未支持者提前诊断。
- copy-update aggregate metadata，覆盖 struct、tuple、enum、nested path；unsupported aggregate 在 typed HIR 前诊断。
- runtime class literal / annotation class literal 的 allowed context 与 HIR 表示。

明确不作为本阶段完成对象的后端缺口：

- raw MIR LLVM codegen 对 `Handle` / `ResumeUnwind` / `TypeCheck` / `Cast` / dynamic call kind 的支持。
- aggregate boxing、array composite element、closure env layout、Step ABI、runtime helper 和 LLVM lowering。
- P7/P8 full regression 与 legacy 删除。

这些后续缺口不能成为 HIR 阶段继续输出 Todo 的理由；HIR 必须要么完整表达，要么提前拒绝。

## 2. 分阶段计划

### H0. 建立 HIR Completeness 守门与现状清单

目标：

- 给 refactor HIR stage 建立明确的“no placeholder”生产级 invariant。
- 把所有现有 HIR Todo 来源转换为可跟踪、可测试的 gap 清单。
- 防止后续任务修一个 gap 时继续从别处泄漏 Todo。

实现：

- 在 refactor typed HIR stage 输出后增加专用 verifier。
- verifier 必须遍历 `hir::File`、所有 lowered member fun、object init、top-level initializer、typed HIR side tables 中可达的表达式和语句。
- verifier 遇到 `Item::Todo`、`StmtKind::Todo`、`ExprKind::Todo`、`Missing` 或等价 placeholder 时返回 source diagnostic。
- verifier 的错误信息要包含 placeholder kind、source path、span、所属 item/function。
- verifier 只在 refactor production HIR handoff 强制执行；legacy dump-only 路径可以不改。
- 建立 Rust 单测扫描当前 HIR enum，确保未来新增 Todo-like variant 时必须显式决定是否允许进入 refactor handoff。

阶段输出：

- `RefactorHirCompletenessVerifier` 或等价 API。
- 一份代码内可维护的 HIR placeholder reason 清单。
- refactor HIR stage 默认启用 verifier。

验证：

- `cargo test -p scoopc --no-default-features refactor_hir_no_todo`
- 对每个当前 Todo reason 建一个最小 negative 或 pending fixture，确认不是静默流入 HIR handoff。
- 不运行 full fixtures。

完成条件：

- refactor HIR stage 不可能成功产出包含 HIR Todo 的 handoff。
- 现有 HIR Todo 来源全部被后续任务覆盖或明确映射到前端诊断。

### H1. Parser Surface Gate 与延期语法拒绝

目标：

- 对纯语法即可判断本阶段不支持的 surface，在 parser 阶段拒绝。
- 避免 AST 中长期保留“已解析但后面必然 Todo”的节点。

实现：

- 禁止 structured concurrency user surface `spawn` / `join` 进入 refactor HIR。
- 若 spec 仍标记该 surface 为 deferred，parser 对 `spawn` / user-facing `join` 给出明确错误：功能延期、当前请使用已有 async/await 或 runtime API。
- 禁止 assignment expression 出现在表达式上下文；parser 只允许 assignment 作为 statement。
- 禁止 spread / named arg 出现在 call argument list 之外；parser 在表达式上下文给出明确错误。
- 对 `comptime` item/stmt 保持可解析，因为它们属于 spec 已支持 surface，后续 H2/H3 必须实现，不得 parser 拒绝。
- 对 `value.[field]` 保持可解析，因为它属于 spec 已支持 surface，后续 H3 必须实现或在需要 type/comptime 时诊断。

阶段输出：

- 一组 parser diagnostics，覆盖延期或语法位置非法的 surface。
- AST -> HIR 前不再存在可纯语法判定的“必然 Todo”节点。

验证：

- 新增 parse fixtures：`spawn_is_deferred_error`、`join_is_deferred_error`、`assignment_expression_is_error`、`spread_arg_outside_call_is_error`、`named_arg_outside_call_is_error`。
- `cargo test -p scoopc --no-default-features parser_hir_surface_gate`
- `cargo run -p scoop --no-default-features -- --effect-pipeline refactor test --fixtures <上述 parse/typecheck fixture>`

完成条件：

- 纯语法非法或延期 surface 不再能进入 refactor typed HIR。

### H2. Typed HIR Stage 诊断通道与 no-Todo Handoff

目标：

- 让 HIR lowering 函数不再用 Todo 表达“metadata 缺失”或“暂不支持”。
- 所有需要 typecheck/comptime 信息才能判断的失败都变成 stage diagnostic。

实现：

- refactor HIR lowering API 从“总能返回 `hir::File`”调整为“返回 `Result<TypedHirStageOutput, HirStageError>`”。
- 建立 `HirStageErrorKind`，至少覆盖：missing typed metadata、unsupported aggregate、unsupported lvalue、invalid class literal context、unresolved splice field、array literal inference failure、missing custom iterator metadata、extension property without getter。
- 旧 HIR dump 路径若仍需要 Todo，可保留旧 API；refactor production stage 禁止调用 dump-only lowerer。
- typed HIR stage 对 parser/typecheck/comptime 已经诊断过的错误不重复制造 Todo；只负责把错误向 CLI/fixture runner 转成稳定 diagnostic。
- `dump-hir --effect-pipeline refactor` 若遇到不完整 surface，应失败并打印诊断，不生成含 Todo dump。

阶段输出：

- refactor HIR stage 的结构化错误类型。
- no-Todo handoff verifier 接入所有 refactor HIR 入口。
- HIR dump / test fixture 对错误诊断有稳定 golden。

验证：

- `cargo test -p scoopc --no-default-features refactor_hir_stage_errors`
- `cargo test -p scoop --no-default-features dump_hir`
- 针对每类 `HirStageErrorKind` 新增最小 fixture。

完成条件：

- refactor HIR lowering 不再需要通过 Todo 传递错误或延期语义。

### H3. Comptime Expansion 与 Splice Field Lowering

目标：

- 完整支持 spec 中的 `comptime block`、`comptime if`、`comptime for` 与 `value.[field]`。
- HIR runtime body 中不保留 comptime 控制流或 splice 占位。

实现：

- 在 typed HIR 前或 typed HIR stage 内建立 comptime expansion pass。
- `comptime if` 必须在编译期求值并只保留选中分支。
- `comptime for` 必须在编译期展开为普通 HIR statements/items。
- `comptime block` 必须执行其 compile-time side effect，并将 runtime 产物写回 AST/HIR lowering 输入。
- package-level `comptime if` 必须展开为普通 item 列表。
- `value.[field]` 必须在 comptime / typecheck 阶段解析为具体 field binding，再 lower 为普通 `MemberAccess` 或对应 HIR place。
- 对无法静态解析的 splice field，给出 source diagnostic，说明 field 必须是 compile-time known string / field symbol。
- 更新 comptime reflection side tables，使 HIR lowering 不从源码字符串猜 field。
- 涉及本地 `struct` / `class` declaration 的 splice field HIR 验证必须先依赖 H4 declaration graph，避免 refactor no-Todo verifier 在进入 splice lowering 断言前被 `Item::Todo(type)` 阻断。

阶段输出：

- refactor HIR 输入中 runtime body 已不含 comptime control AST。
- HIR 中 `SpliceField` 被规范化为具体 member/field access。
- package-level comptime expansion 后只剩普通 `Item`。

验证：

- `cargo test -p scoopc --no-default-features refactor_hir_comptime`
- 定向 fixtures 覆盖：`comptime if`、`comptime for`、package-level `comptime if`、reflection field loop、`value.["name"]` / `value.[field]`。
- 只运行上述 parse/typecheck/comptime/HIR dump fixtures，不运行 full fixtures。

完成条件：

- refactor HIR 中不再出现 `comptime_block`、`comptime_if`、`comptime_for`、`comptime_if_item`、`splice_field` 占位。

### H4. Declaration HIR：typealias / type / object / extension property

目标：

- HIR file 本身能完整表达当前前端接受的非 executable declaration。
- 不再把 typealias、type、object 当成 `Item::Todo`，也不要求下游回看 AST 才知道声明图。
- 该阶段是本地类型 splice field HIR 验证的 prerequisite；完成后再收口 `value.[field]` 的 HIR lowering。

实现：

- 扩展 HIR item model，新增或等价表达：`TypeAlias`、`TypeDecl`、`ObjectDecl`、`MetadataDecl`。
- typealias HIR 必须保存 alias FQN、type params、resolved target type、source span。
- type/object HIR 必须保存 nominal identity、kind、type params、fields/constructors/member funcs/properties、interfaces/supertypes、annotations 中 HIR stage 需要的信息。
- object HIR 必须发布 singleton value identity 和 object initializer contract。
- extension property 必须在 HIR 中表达 getter/setter contract；如果当前 spec surface 要求 getter 才可读而缺 getter，typecheck/HIR stage 给清晰诊断，不再 `extension_property_no_getter`。
- member funcs 可继续按现有 side table 降成 callable bodies，但 HIR item graph 必须能找到其 owner/member relation。
- 保持 legacy path 不变；refactor HIR formatter 为新增 declaration 提供稳定输出。

阶段输出：

- 无 `Item::Todo("typealias" / "type" / "object" / "extension_property_no_getter")`。
- HIR declaration graph 足以让 MIR/facts 后续按结构化 owner/member 查询。

验证：

- `cargo test -p scoopc --no-default-features refactor_hir_decls`
- `dump-hir --effect-pipeline refactor` 定向覆盖 typealias、class、struct、enum、interface、object、extension property。
- 不跑 full fixtures。

完成条件：

- 所有当前 parser/typecheck 接受的 declaration 都有非 Todo HIR 表示或明确诊断。

### H5. Literal / Argument / Call Canonicalization

目标：

- HIR 负责把 call-like surface 规范化为下游无需重新绑定的形式。
- array literal、named/default/spread args 不再以 fallback Todo 或 raw syntax 逃逸。

实现：

- array literal 必须由 expected type 或元素类型推断得到 `Array<T>` / `MutableArray<T>` / tuple-like target；空数组若没有 expected type，typed HIR stage 诊断。
- named/default args 在 typecheck 后形成完整 ordered arg list，HIR Call 不再保留需要后端补齐的缺省参数。
- default arg value 若需要 thunk 或 late evaluation，HIR contract 必须显式标出 default source 与 evaluation order。
- spread args 若当前 spec 支持，typecheck 输出 expanded arg binding；HIR lower 成普通 ordered args 或显式 vararg array construction。
- 若 spread surface 当前只允许 vararg context，则 parser/typecheck 在非 vararg context 报错。
- callable callee provenance 必须在 typed HIR side table 中结构化发布：direct FQN、member owner/name、extension target、closure/fun value、virtual/interface dispatch、constructor、intrinsic。
- HIR 不再依赖 callee expression shape 让 MIR 猜 `CallKind`。

阶段输出：

- 无 `array_lit`、`spread_arg`、`named_arg` HIR Todo。
- 每个 call site 都有完整 arg binding 与 callee provenance。

验证：

- `cargo test -p scoopc --no-default-features refactor_hir_call_contracts`
- 定向 fixtures 覆盖 empty array with expected type、empty array without expected type error、named/default args、vararg/spread、member/extension/generic/ctor calls。

完成条件：

- MIR lowering 可只消费 HIR call contract，不再靠表达式形状猜 callee 或补参数。

### H6. Class Literal 与 Runtime Reflection Contract

目标：

- 明确 `String::class` / `T::class` 等 class literal 的 HIR contract。
- annotation/comptime 与 runtime fallback 不再混用 Todo。

实现：

- 定义 HIR `ClassLiteral` 或 `TypeMetadataLiteral` 表示，包含 source type、runtime metadata kind、fallback value type。
- annotation/comptime context 中按当前 v0 语义可折叠为类型名字符串或 metadata constant。
- runtime context 若 spec 支持，则 HIR 输出明确的 metadata/string/value primitive contract，供后续 MIR/codegen 实现。
- runtime context 若本阶段决定暂不支持，则 typecheck/HIR stage 给 diagnostic，不能生成 Todo。
- 将 `nameOf<T>()`、`sizeOf<T>()`、`getPlatform()` 等 reflection/platform intrinsic 的 runtime fallback allowed context 写入 typed HIR side table；本阶段不实现 LLVM lowering，但不能让 HIR 语义丢失。

阶段输出：

- 无 `class_lit` HIR Todo。
- class literal / reflection intrinsic 有明确 allowed context 和 HIR representation。

验证：

- `cargo test -p scoopc --no-default-features refactor_hir_class_literal`
- 定向 fixtures 覆盖 annotation arg、comptime reflection、runtime fallback allowed 或 error。

完成条件：

- class literal 不再是 HIR placeholder；后续阶段看到的是明确 HIR primitive 或明确前端错误。

### H7. Copy-Update 与 Lvalue Place Contract

目标：

- 完整支持当前 typecheck 接受的 `with` copy-update 和 assignment LHS。
- 对未支持的 aggregate/place shape 提前诊断，不让 MIR 生成 assign/copy-update Todo。

实现：

- `with` copy-update lowering 必须强制消费 typecheck 发布的 aggregate metadata。
- struct、tuple、enum copy-update 和 nested path 必须 lower 成稳定 HIR representation 或 canonical constructor/update primitive。
- 缺 aggregate map、field map、variant payload map 时，refactor HIR stage fail-fast。
- unsupported aggregate kind 由 typecheck/HIR diagnostic 拒绝。
- 建立 HIR `Place` 或 typed side table，覆盖 assignment LHS 当前支持范围：local var、top-level var、field/member、index/property setter、safe-member setter；不支持者提前诊断。
- assignment 作为 statement lower 成 `StmtKind::Assign` + typed place contract；assignment expression 在 H1 已被 parser 拒绝。

阶段输出：

- 无 `with_update`、`assign` HIR Todo。
- 每个 assignment statement 都有下游可消费的 typed place contract。

验证：

- `cargo test -p scoopc --no-default-features refactor_hir_places`
- 定向 fixtures 覆盖 with-update struct/tuple/enum/nested path、unsupported aggregate error、local/global/field/index assignment。

完成条件：

- HIR/MIR 之间不再通过 `assign lhs lowering pending` 或 `with_update` 占位传递 place/update 缺口。

### H8. Top-Level Initialization 与 HIR Program Handoff

目标：

- HIR handoff 能完整描述 top-level value、const/runtime split、object initializer、metadata initializer。
- 下一阶段不再把 top-level `val` 或 object init 降为 MIR Todo，也不需要回读 AST。

实现：

- typed HIR stage 为每个 top-level `val` / `var` / const value 发布 initializer body contract。
- 区分 compile-time constant、runtime immutable value、runtime mutable global、extern global。
- object singleton initializer 作为明确 HIR init root 发布，包含 dependency ordering 所需信息。
- type metadata / alias metadata 若需要 runtime init，也进入 HIR program init graph。
- `@Extern` global variable 若 typecheck 已接受，HIR storage model 必须表达 extern symbol name、TLS/global、initializer absence、unsafe access requirement。
- HIR handoff 包含 top-level dependency graph 或至少包含后续 MIR init ordering 所需的结构化 facts。

阶段输出：

- HIR program handoff 中有完整 top-level init roots。
- top-level val/object/type metadata 不再依赖下游回读 HIR expr 或 AST 临时表。

验证：

- `cargo test -p scoopc --no-default-features refactor_hir_top_level_init`
- 定向 fixtures 覆盖 top-level val、object init、const value、extern global declaration。

完成条件：

- 下一阶段可从 HIR handoff 构建 MIR roots，不需要 `mir::Item::Todo("top-level val")`。

### H9. Refactor HIR -> MIR Preflight Guard

目标：

- 在 HIR 阶段完成后，以轻量 preflight 证明不会把 HIR gap 推到 refactor MIR lowering。
- 不要求 MIR/LLVM 后续能力完整，只验证 HIR handoff 足够且无 Todo 进入下一阶段。

实现：

- 新增 `--effect-pipeline refactor dump-hir` / stage helper 的 no-Todo verification。
- 新增可选 `hir-to-mir-preflight` 单测或 internal API，只对 HIR handoff 做结构检查，不跑后端。
- preflight 检查 HIR side tables 是否覆盖所有 call/effect-sensitive sites、place/update sites、declaration/init roots。
- 若调用 refactor MIR lowerer，只允许跑到 direct-style MIR stage verifier；遇到后续已知 MIR/codegen gap 不作为本阶段失败，除非是 HIR contract 缺失导致的 Todo。

阶段输出：

- HIR completeness matrix。
- HIR -> next-stage preflight 断言：没有 HIR Todo、没有 missing typed contract、没有 fallback reason。

验证：

- `cargo test -p scoopc --no-default-features refactor_hir_preflight`
- 针对 HIR gap fixture 逐个运行 `dump-hir --effect-pipeline refactor`。
- 对少量样本运行 `dump-mir --effect-pipeline refactor`，只确认没有 HIR-origin Todo。

完成条件：

- HIR 阶段可独立宣布完成，不依赖 full run-pass 或 LLVM。

## 3. 阶段切换门槛

- H0 未完成前，不开始批量替换具体 Todo，避免修复后仍无守门。
- H1 未完成前，不继续依赖 HIR stage 处理纯语法延期 surface。
- H2 未完成前，不允许把 HIR lowering 的错误继续编码成 Todo。
- H3 未完成前，不进入依赖 comptime/splice 结果的泛型/reflection HIR 闭包。
- H4 未完成前，不进入 top-level init graph 和 MIR root 收口。
- H5 未完成前，不进入 call/callee provenance 的 MIR preflight。
- H6 未完成前，不允许 class literal/runtime reflection fallback 继续流入下游。
- H7 未完成前，不允许 assignment/copy-update 相关 fixture 进入 MIR preflight。
- H8 未完成前，不允许声称 HIR program graph 完整。
- H9 未完成前，本阶段不算结束。

## 4. 完成标准

最终 HIR handoff contract 与验证矩阵冻结在 [`HIR_COMPLETENESS_HANDOFF.md`](./HIR_COMPLETENESS_HANDOFF.md)。该文档是本阶段完成记录的一部分，并明确 refactor typed HIR 可提供给后续 MIR/effect/late-lowering 阶段依赖的 invariant。

本阶段完成时，必须能够明确陈述以下结论全部成立：

1. `dump-hir --effect-pipeline refactor` 和 refactor typed HIR stage 不再产出任何 `Item::Todo`、`StmtKind::Todo`、`ExprKind::Todo` 或等价 placeholder。
2. spec/fixture 已支持的 HIR surface 都有 HIR 表达或在 HIR 前完成规范化。
3. parser 已拒绝纯语法可判定的延期或非法 surface，并给出明确错误消息。
4. 需要 type/comptime 信息才能判断的失败由 typecheck/comptime/refactor HIR stage 给出 source diagnostic，而不是生成 Todo。
5. comptime block/if/for 与 package-level comptime if 已在 HIR 前展开或裁剪。
6. splice field 已解析为普通 typed member/place access，无法静态解析时提前诊断。
7. typealias/type/object/extension property 在 HIR declaration graph 中有非 Todo 表达。
8. array literal、named/default/spread args、callable callee provenance 已在 typed HIR handoff 中闭合。
9. class literal 与 reflection intrinsic fallback 有明确 HIR contract 或明确诊断。
10. copy-update 与 assignment LHS 有 typed aggregate/place contract，unsupported shape 不再流向 MIR。
11. top-level val/object/type metadata/extern global 有 HIR-level init/storage contract。
12. refactor HIR -> next-stage preflight 能证明没有 HIR-origin Todo 或 missing contract 流向后续阶段。
13. 本阶段验证只依赖定向 parser/typecheck/HIR/preflight 测试，不要求后续 MIR/LLVM/full fixture 全部通过。
