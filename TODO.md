# TODO（MIR Closure：refactor MIR gap 收口）

> 生成时间：2026-05-06  
> 计划基线：[`PLAN.md`](./PLAN.md)  
> 差距基线：[`PIPELINE_GAPS.md`](./PIPELINE_GAPS.md)  
> 格式参考：[`TODO-effect-refactor.md`](./TODO-effect-refactor.md)、[`TODO-P3.md`](./TODO-P3.md)  
> 前置条件：effect-refactor 新路径已经存在；本阶段只收口新路径上的 MIR gaps，legacy 路径保持现状。  
> 顺序约束：严格按当前文件中的条目顺序推进；不得跨条目并行实现。  
> 本阶段目标：refactor MIR stage 和 materialized MIR handoff 不再向后输出任何 `Todo(...)` 或等价 placeholder。所有 spec 支持且 frontend 接收的 surface 都必须有完整 MIR 表达；当前不支持的 surface 必须在进入 MIR 前被 parser/frontend 清晰拒绝。

## 任务索引

| ID | 阶段 | 标题 |
| --- | --- | --- |
| `MIR-T00` | M0 | 建立 MIR placeholder inventory 与 gap ownership map |
| `MIR-T01` | M1 | 落地 refactor production MIR strict verifier |
| `MIR-T02` | M1 | 落地 materialized MIR strict verifier 与 no-param gate |
| `MIR-T03` | M2 | 收口 parser/frontend/HIR placeholder 入口 |
| `MIR-T04` | M2 | 完成 comptime、splice field、class literal、with-update 的 MIR 前置闭包 |
| `MIR-T05` | M3 | 建立完整 MIR program item graph 与 top-level roots |
| `MIR-T06` | M4 | 建立 unified place/lvalue contract 并清理 assignment Todo |
| `MIR-T07` | M5 | 收口 call/ctor/default/named/intrinsic typed call-site contract |
| `MIR-T08` | M5 | 收口 dispatch/resume/perform/handle site contract |
| `MIR-T09` | M6 | 收口 runtime value primitives 的 MIR 表达 |
| `MIR-T10` | M6 | 收口 aggregate/array/enum/closure transport 的 MIR contract |
| `MIR-T11` | M7 | 收口 generic root、effect-row args 与 materialization substitution |
| `MIR-T12` | M8 | 建立 MIR-only 验证矩阵并完成阶段退出审计 |

## 全局约束

- 本文件所有任务只修 refactor 新路径。
- 不允许把 legacy path 的旧 fallback 改成“部分 refactor aware”的混合实现；共享逻辑必须是完全中立 API，否则在 refactor stage 附近单独实现。
- 每个任务完成时都必须保证 refactor production MIR 不新增任何 placeholder。
- 不允许新增 `Todo(...)` reason 后再“稍后处理”；必须先更新 inventory、指定 owner task 和 disposition。
- 不允许让 P4/P5/P6 回 AST/HIR 私有 side table 补语义；本阶段必须把 semantic source of truth 固定在 MIR stage output / materialized MIR / MIR metadata 上。
- 验证以定向命令为准，不运行 full fixtures。
- 明确禁止在本阶段要求通过：`cargo test --all`、`cargo run -p scoop -- test`、P7/P8 GC/full regression 矩阵。

## [DONE] MIR-T00：建立 MIR placeholder inventory 与 gap ownership map

- 参考：
  - [`PLAN.md`](./PLAN.md) §2/M0
  - [`PIPELINE_GAPS.md`](./PIPELINE_GAPS.md) §1、§2、§8
- 目标：
  - 把所有会进入 refactor MIR 或 materialized MIR 的 placeholder/fallback 变成可执行 inventory。
  - 为每个 gap 指定处理策略和 owner task。

- 必须实现的内容：
  1. 新增 refactor MIR placeholder inventory 测试或等价模块。
     - 推荐位置：`crates/scoopc/src/mir/placeholder_inventory.rs` 或 refactor pipeline 专属测试模块。
     - inventory 扫描 `crates/scoopc/src/mir/**`、`crates/scoopc/src/effect_refactor_pipeline/**`、必要的 HIR handoff 入口。
  2. inventory 至少识别以下构造：
     - `Item::Todo`
     - `StatementKind::Todo`
     - `Rvalue::Todo`
     - `TerminatorKind::Todo`
     - `UnwindAction::Todo`
     - materializer 对上述结构的 no-op rewrite
  3. 为每条 entry 记录：
     - placeholder surface
     - reason string
     - 所属 `PIPELINE_GAPS.md` 编号
     - disposition：`ImplementInMir` / `ImplementBeforeMir` / `RejectBeforeMir` / `LegacyOnly`
     - owner task id
     - 是否必须从 refactor production path 消除
  4. inventory 必须覆盖当前已知 MIR reason：
     - `top-level val`
     - `assign lhs lowering pending`
     - `assign place contract missing`
     - `call callee lowering pending`
     - `ctor call lowering pending`
     - `resume lowering requires canonical callee shape`
     - `dispatch callee lowering pending`
     - `refactor perform contract missing`
     - `refactor handle contract missing`
     - `missing expr`
     - `boxed var decl init pending`
     - `break not in loop`
     - `continue not in loop`
  5. 若 inventory 扫描发现新增 placeholder，测试必须失败并要求先更新本文件。

- 必须遵从的约束：
  - inventory 是本阶段的 gating asset，不是文档注释。
  - 不允许把 refactor production 必须消除的 entry 标成 `LegacyOnly`。

- 验证：
  1. 运行：`cargo test -p scoopc --no-default-features refactor_mir_placeholder_inventory`
  2. 额外搜索：`rg 'Todo\(|Item::Todo|StatementKind::Todo|Rvalue::Todo|TerminatorKind::Todo|UnwindAction::Todo' crates/scoopc/src`
  3. 搜索结果必须全部可追溯到 inventory 或 legacy-only bucket。

- 完成条件：
  - 所有 known MIR gaps 都有 owner task。
  - 后续任务能用 inventory 判断是否真正消除了对应 gap。
- 依赖：无

- 完成记录（2026-05-06）：
  - 新增 `crates/scoopc/src/mir/placeholder_inventory.rs`，以 `refactor_mir_placeholder_inventory` 单测固化 refactor MIR placeholder inventory。
  - inventory 覆盖 MIR item/statement/rvalue/terminator/unwind placeholder surface、HIR handoff placeholder passthrough、materializer Todo no-op rewrite，并为每项记录 `PIPELINE_GAPS.md` 映射、disposition、owner task、production elimination 要求和处理策略。
  - `crates/scoopc/src/mir/mod.rs` 已以 `#[cfg(test)]` 接入 inventory 模块；扫描器会在新增 placeholder 构造或 materializer no-op 变化时要求先更新 inventory。
  - 验证通过：`cargo test -p scoopc --no-default-features refactor_mir_placeholder_inventory`。
  - 搜索审计已执行：`rg 'Todo\(|Item::Todo|StatementKind::Todo|Rvalue::Todo|TerminatorKind::Todo|UnwindAction::Todo' crates/scoopc/src`；命中已归入 inventory、HIR inventory、现有 verifier/preflight/summary/pass/codegen consumer、materializer no-op 或 legacy/effect-lowered consumer。
  - 额外 lint 通过：`cargo clippy -p scoopc --no-default-features --all-targets -- -D warnings`。

## [DONE] MIR-T01：落地 refactor production MIR strict verifier

- 参考：
  - [`PLAN.md`](./PLAN.md) §2/M1
  - [`PIPELINE_GAPS.md`](./PIPELINE_GAPS.md) §2.1、§2.3、§2.4
  - 当前实现：`crates/scoopc/src/mir/mod.rs::Body::validate_refactor_direct_style`
- 目标：
  - 让 refactor MIR stage 在 stage boundary 拒绝任何 executable placeholder。
  - 区分 dump/debug verifier 与 production verifier。

- 必须实现的内容：
  1. 新增 strict production verifier。
     - 推荐 API：`MirFile::validate_refactor_production(...)` 或 `validate_refactor_production_mir(...)`。
     - 它必须调用现有 CFG/site verifier，再追加 no-placeholder 规则。
  2. verifier 必须拒绝：
     - `Item::Todo`
     - 所有 body 内的 `StatementKind::Todo`
     - 所有 `Rvalue::Todo`
     - 所有 `TerminatorKind::Todo`
     - 所有 `UnwindAction::Todo`
     - 非 `Unit` 函数的 `Return { value: None }`
  3. verifier 必须校验 effect-sensitive site metadata 完整性：
     - `Rvalue::Call` 有 `site_id` 和完整 `CallKind` contract。
     - `CallKind::Resume` 有 continuation/resume/answer/out/runtime-error metadata。
     - `TerminatorKind::Perform` 有 site id、op identity、payload tuple metadata、resume target、unwind action。
     - `TerminatorKind::Handle` 有 site id、metadata、arms、body/arm/finally/exit target。
  4. verifier error 必须包含 body FQN、block id、span 或 best-effort span、placeholder reason/category。
  5. `effect_refactor_pipeline::mir_stage::run(...)` 必须调用 strict verifier。
  6. `dump-mir --effect-pipeline refactor` 可以继续走同一 stage；如果需要 dump legacy/incomplete MIR，必须通过独立 debug-only entry，不得复用 production stage。

- 必须遵从的约束：
  - 不得只扩展 `is_forbidden_refactor_effect_todo(...)` 白名单/黑名单。
  - production verifier 默认拒绝所有 Todo，不允许普通 Todo 继续通过。
  - 不能依赖 LLVM codegen 报错作为 verifier。

- 验证：
  1. 新增单元测试，推荐前缀：`refactor_mir_no_todo_*`。
  2. 覆盖每种 Todo surface 的最小负例。
  3. 覆盖非 Unit `Return { value: None }` 负例。
  4. 运行：`cargo test -p scoopc --no-default-features refactor_mir_no_todo`

- 完成条件：
  - refactor MIR stage 无法产出含 Todo 的 successful output。
  - 负例 diagnostics 不再晚到 LLVM/raw MIR codegen。
- 依赖：`MIR-T00`

- 完成记录（2026-05-06）：
  - 新增 `MirFile::validate_refactor_production(...)`，在现有 direct-style CFG/site verifier 之后执行 production-only no-placeholder、非 `Unit` 空返回、effect-sensitive site metadata 完整性检查。
  - strict verifier 现在拒绝 `Item::Todo`、`StatementKind::Todo`、`Rvalue::Todo`、`TerminatorKind::Todo`、`UnwindAction::Todo`，并在诊断中携带 body FQN、block、span、placeholder category/reason 或 site metadata 缺失原因。
  - `effect_refactor_pipeline::mir_stage::run(...)` 已改为调用 production verifier；debug/body-level `validate_refactor_direct_style()` 保留为 direct-style 结构 verifier。
  - 新增 `refactor_mir_no_todo_*` 单测，覆盖所有 Todo surface、非 `Unit` `Return { value: None }`、resume runtime-error metadata 缺失，以及 stage validator 拒绝 item Todo。
  - 验证通过：`cargo test -p scoopc --no-default-features refactor_mir_no_todo`。
  - 额外回归通过：`cargo test -p scoopc --no-default-features refactor_mir_placeholder_inventory`。
  - 额外 lint 通过：`cargo clippy -p scoopc --no-default-features --all-targets -- -D warnings`。

## MIR-T02：落地 materialized MIR strict verifier 与 no-param gate

- 参考：
  - [`PLAN.md`](./PLAN.md) §2/M1、§2/M7
  - [`PIPELINE_GAPS.md`](./PIPELINE_GAPS.md) §2.2、§2.5、§2.7、§2.8
  - 当前实现：`crates/scoopc/src/mir/materialize.rs`
- 目标：
  - materializer 不再透传 Todo。
  - materialized snapshot 不再携带裸 type/effect param 或 missing template/root。

- 必须实现的内容：
  1. 在 materializer rewrite 入口拒绝 `StatementKind::Todo`、`Rvalue::Todo`、`TerminatorKind::Todo`、`UnwindAction::Todo`。
  2. 在 materializer 输出后运行 strict materialized verifier。
  3. verifier 必须检查：
     - 无 `TypeKind::Param` 出现在普通 source value、frame slot、call arg、return、aggregate transport、closure env、top-level root。
     - effect-row generic args 已完成 substitution。
     - resume surface 上允许 erased carrier 的例外必须有显式 marker。
     - 所有 direct/generic call target 都能解析到 materialized root 或明确 external/intrinsic root。
  4. `MissingGenericTemplate`、`MissingMirRootForTemplate`、effect arg inference failure 必须携带 source call site diagnostic。
  5. 更新 materialized MIR dump/preflight，确保它和 direct MIR 使用同一 no-placeholder policy。

- 必须遵从的约束：
  - 不允许 materializer 对 Todo no-op。
  - 不允许把裸 param 留给 LLVM layout 报错。

- 验证：
  1. 新增单元测试，推荐前缀：`refactor_materialized_mir_no_todo_*`、`refactor_materialized_mir_no_param_*`。
  2. 运行：`cargo test -p scoopc --no-default-features refactor_materialized_mir`
  3. 负例覆盖 Todo template、missing root、裸 type param、effect-row arg 缺失。

- 完成条件：
  - materialized MIR successful output 一定 no Todo/no unresolved generic param。
  - 后续 P4/P5/P6 可以把 materialized snapshot 当 canonical input。
- 依赖：`MIR-T01`

## MIR-T03：收口 parser/frontend/HIR placeholder 入口

- 参考：
  - [`PLAN.md`](./PLAN.md) §2/M2
  - [`PIPELINE_GAPS.md`](./PIPELINE_GAPS.md) §1.1、§1.2、§1.3、§1.5、§7.4
  - 当前实现：`crates/scoopc/src/hir/lower/placeholder_inventory.rs`
- 目标：
  - refactor path 不再从 HIR 接收必须消除的 `ExprKind::Todo`、`StmtKind::Todo`、`Item::Todo`、`ExprKind::Missing`。
  - 延期 surface 在 parser/frontend 处被拒绝。

- 必须实现的内容：
  1. 扩展 HIR placeholder inventory，使它和 MIR inventory 共享 disposition 语义。
  2. parser/frontend 对 structured concurrency `spawn` / user-facing `join` 给出 deferred-feature diagnostic。
  3. parser recovery 的 `Missing` 在 refactor production path 中升级为 parse diagnostic，不允许进入 HIR/MIR。
  4. package-level `comptime if` 未被 trim 时必须 diagnostic，不允许生成 `Item::Todo(comptime_if_item)`。
  5. HIR stage output 增加 no-placeholder preflight，MIR stage 不再需要为 HIR-origin placeholder 兜底。

- 必须遵从的约束：
  - 不允许只在 MIR verifier 报 `HIR leaked Todo`，必须尽可能在 parser/frontend/HIR stage 给出 source-level diagnostic。
  - 不允许为了通过 inventory 删除 parser tests。

- 验证：
  1. 运行：`cargo test -p scoopc --no-default-features refactor_hir_placeholder_inventory`
  2. 运行：`cargo test -p scoopc --no-default-features refactor_hir_preflight`
  3. 新增 parser/frontend diagnostics fixtures，覆盖 `spawn`、`join`、parser recovery missing、untrimmed package comptime if。

- 完成条件：
  - refactor HIR successful output 不包含必须消除的 placeholder。
  - unsupported/deferred syntax 不能再走到 MIR。
- 依赖：`MIR-T00`

## MIR-T04：完成 comptime、splice field、class literal、with-update 的 MIR 前置闭包

- 参考：
  - [`PLAN.md`](./PLAN.md) §2/M2
  - [`PIPELINE_GAPS.md`](./PIPELINE_GAPS.md) §1.1、§1.2、§1.3、§1.12
- 目标：
  - spec-supported compile-time surface 在进入 MIR 前展开为普通 typed HIR/MIR 输入。
  - class literal 和 copy-update 不再以 fallback Todo 表示。

- 必须实现的内容：
  1. `comptime block/if/for`：
     - runtime HIR lowering 前必须完成 expansion/elimination。
     - 未能求值的条件、不可枚举的 comptime for、非法生成项都给 source diagnostic。
  2. splice field：
     - `value.[field]` 必须在 comptime/typecheck 阶段解析为 concrete member access。
     - 若 field 值不是 compile-time string/name，frontend diagnostic。
  3. class literal：
     - 选择并实现本阶段 policy。
     - 若 runtime class literal 支持，则 MIR 表达为 type metadata/string/type descriptor value primitive。
     - 若仅 annotation/comptime 支持，则 runtime 用法 frontend diagnostic。
  4. `with` copy-update：
     - typed handoff 必须发布 aggregate kind、base type、field path、value type、enum variant payload mapping。
     - HIR lowering 不再因缺 aggregate maps 返回 `Todo("with_update")`。
  5. 将 HIR preflight 中相关 `HirOnly` 样本升级为 MIR smoke。

- 必须遵从的约束：
  - 不允许 comptime/splice/class literal 的合法样本只停在 HIR 验证。
  - 不允许 with-update unsupported aggregate 晚到 MIR Todo；必须 frontend diagnostic 或具体 MIR contract。

- 验证：
  1. 运行：`cargo test -p scoopc --no-default-features refactor_hir_comptime`
  2. 运行：`cargo test -p scoopc --no-default-features refactor_mir_comptime_splice`
  3. 定向命令：`cargo run -p scoop --no-default-features -- --effect-pipeline refactor dump-mir tests/fixtures/comptime/splice_field_access_v0_basic.scoop`
  4. 新增/更新 `mir_refactor` fixtures 覆盖 comptime control-flow、splice field、class literal policy、tuple/struct/enum with-update。

- 完成条件：
  - `comptime_*`、`splice_field`、`class_lit`、`with_update` 不再能泄漏到 refactor MIR Todo。
- 依赖：`MIR-T03`

## MIR-T05：建立完整 MIR program item graph 与 top-level roots

- 参考：
  - [`PLAN.md`](./PLAN.md) §2/M3
  - [`PIPELINE_GAPS.md`](./PIPELINE_GAPS.md) §1.4、§1.5、§2.5、§6.4
- 目标：
  - MIR file 不再只是函数列表。
  - top-level values、object/type metadata、extern globals 和 initializer roots 都能从 MIR stage output 查询。

- 必须实现的内容：
  1. 设计并实现 non-executable declaration 与 executable initializer 的 MIR 表达。
  2. top-level `val` / const-like immutable value：
     - 生成 initializer body 或 MIR value root。
     - 发布 dependency order、hidden ordinary effects、runtime-vs-const split。
  3. `typealias`、`type`、`object`：
     - 发射 resolved alias metadata、nominal metadata、object init metadata。
     - member fun 和 object/member init root 进入 canonical root index。
  4. `@Extern` global variable：
     - MIR metadata 包含 external symbol name、linkage、TLS flag、initializer absence、unsafe access contract。
  5. refactor stage output 增加查询 API：
     - callable roots
     - initializer roots
     - global/extern roots
     - nominal/object/typealias metadata roots

- 必须遵从的约束：
  - 不允许 `hir::Item::Val` 继续 lower 成 `Item::Todo`。
  - 不允许 P4/P5 继续回 HIR side table 才能发现 object/type/global metadata。

- 验证：
  1. 运行：`cargo test -p scoopc --no-default-features refactor_mir_item_graph`
  2. 新增 `mir_refactor/top_level_roots.scoop`，覆盖 top-level val、typealias、type、object、extern global。
  3. 运行：`cargo run -p scoop --no-default-features -- --effect-pipeline refactor dump-mir tests/fixtures/mir_refactor/top_level_roots.scoop`

- 完成条件：
  - refactor MIR `File` 不含 top-level declaration Todo。
  - materialized root index 能找到所有 generic/non-generic callable 和 initializer template。
- 依赖：`MIR-T02`

## MIR-T06：建立 unified place/lvalue contract 并清理 assignment Todo

- 参考：
  - [`PLAN.md`](./PLAN.md) §2/M4
  - [`PIPELINE_GAPS.md`](./PIPELINE_GAPS.md) §1.6、§7.5
- 目标：
  - assignment lowering 只消费 typed place contract。
  - 所有 typecheck 接收的 assignable place 都能 lower 成 MIR place/store。

- 必须实现的内容：
  1. 定义 MIR place model 或等价 typed store contract。
  2. 覆盖：
     - local / boxed local
     - top-level var / extern global
     - member field / property
     - tuple/struct field path
     - enum payload path
     - index place
  3. refactor typed HIR 为每个 assignment 发布 authoritative place contract。
  4. MIR lowering 删除 HIR expr shape fallback；缺 contract 直接 diagnostic。
  5. 修复 boxed mutable local 无 initializer 的 MIR 表达；不能再输出 `boxed var decl init pending`。
  6. 非法 `break` / `continue` 在 frontend control-flow check 报错，不得生成 MIR Todo。

- 必须遵从的约束：
  - 不允许 `assign lhs lowering pending`、`assign place contract missing` 等 reason 继续存在于 refactor production path。
  - 不允许新增只覆盖 local/member 的第二套 partial place helper。

- 验证：
  1. 运行：`cargo test -p scoopc --no-default-features refactor_mir_place_contract`
  2. 新增 `mir_refactor/assignment_places.scoop` 覆盖所有 supported place。
  3. 负例 diagnostics 覆盖 unsupported assign syntax、非法 break/continue。

- 完成条件：
  - assignment/store lowering 不再产生 Todo。
  - Store metadata 足以支撑 later-stage member/global/index/continuation provenance。
- 依赖：`MIR-T05`

## MIR-T07：收口 call/ctor/default/named/intrinsic typed call-site contract

- 参考：
  - [`PLAN.md`](./PLAN.md) §2/M5
  - [`PIPELINE_GAPS.md`](./PIPELINE_GAPS.md) §1.7、§2.6、§3.9、§3.10、§3.7、§6.3
- 目标：
  - 所有 call-like expression 都由 typecheck 发布的 call-site contract 驱动。
  - MIR 不再猜 callee provenance，不再因为 named/default/generic/ctor/intrinsic 缺绑定而占位。

- 必须实现的内容：
  1. typed handoff 为每个 call site 发布：
     - selected callable/ctor/intrinsic identity
     - receiver binding
     - ordered complete args
     - named/default/vararg binding
     - generic type args
     - effect-row args
     - hidden ordinary effects
  2. MIR `CallKind` 从 binding 生成：
     - `Direct`
     - `Closure`
     - `FunValue`
     - intrinsic value primitive/call
     - class/object/enum constructor
  3. top-level function reference 明确 normalized policy：
     - 要么成为 function value/closure object。
     - 要么成为可 codegen 的 symbol value，并带完整 metadata。
  4. class ctor MIR 携带 selected ctor 和 complete bound args；不把 named/default 留给 backend。
  5. runtime fallback intrinsics：
     - `sizeOf<T>()`
     - `nameOf<T>()`
     - `getPlatform()`
     必须在 MIR 中有明确 intrinsic representation 或 frontend reject。
  6. 删除 `call callee lowering pending`、`ctor call lowering pending`、`sizeOf intrinsic requires one positional arg` 的 refactor reachable path。

- 必须遵从的约束：
  - 不允许基于 `ValueOrigin::UnknownCallable` 作为 authoritative semantics。
  - 不允许后端再补 named/default args。

- 验证：
  1. 运行：`cargo test -p scoopc --no-default-features refactor_mir_call_contract`
  2. 新增 `mir_refactor/call_contracts.scoop` 覆盖 direct、function value、closure、member, generic, named/default, ctor, intrinsic。
  3. 运行对应 `dump-mir --effect-pipeline refactor`。

- 完成条件：
  - call/ctor/intrinsic 相关 Todo reason 不再可能出现在 refactor production MIR。
  - materializer 可以从 call metadata 构造完整 instance key。
- 依赖：`MIR-T06`

## MIR-T08：收口 dispatch/resume/perform/handle site contract

- 参考：
  - [`PLAN.md`](./PLAN.md) §2/M5
  - [`PIPELINE_GAPS.md`](./PIPELINE_GAPS.md) §1.8、§1.9、§1.10、§1.11、§6.5
- 目标：
  - effect/control-sensitive site 的 source of truth 全部在 MIR metadata 或 MIR-attached side table。
  - MIR lowering 不再从字符串 FQN、callee shape、span fallback 恢复语义。

- 必须实现的内容：
  1. dynamic dispatch：
     - typed handoff 发布 structured owner/member binding、dispatch kind、receiver type、selected default method fallback。
     - MIR lowering 删除 `callee_fqn.rsplit_once('.')` 作为 refactor semantic recovery 的路径。
  2. continuation resume：
     - typed handoff 发布 receiver expression route、resume tuple、answer type、out effects、runtime error effect、suspends outward。
     - MIR lowering 支持别名/function value/wrapper/extension 等合法 callee shape，只要 typed contract 指明这是 resume site。
  3. perform：
     - typed handoff 必须发布 concrete op、payload tuple、arg mapping、result type、resume target semantics。
     - 缺 contract 是 stage error，不生成 Todo rvalue/terminator。
  4. handle：
     - typed handoff 必须发布 result/body/finally type、handled op cases、payload binders、continuation binders、arm targets contract。
     - 缺 contract 是 stage error。
  5. strict verifier 追加 site metadata completeness checks。

- 必须遵从的约束：
  - 不允许 `dispatch callee lowering pending`、`resume lowering requires canonical callee shape`、`refactor perform contract missing`、`refactor handle contract missing` 出现在 successful MIR。
  - 不允许 P4 再回 P2 side table 解释 site 语义。

- 验证：
  1. 运行：`cargo test -p scoopc --no-default-features refactor_mir_effect_site_contract`
  2. 复用/新增 `mir_refactor/dispatch_and_resume_call.scoop`、`handle_perform.scoop`、`handle_finally_boundary.scoop`。
  3. 添加负例：删除/伪造 typed contract 时 MIR stage diagnostic 清晰。

- 完成条件：
  - effect/control-sensitive site 的 MIR metadata 可独立驱动 P4 facts。
- 依赖：`MIR-T07`

## MIR-T09：收口 runtime value primitives 的 MIR 表达

- 参考：
  - [`PLAN.md`](./PLAN.md) §2/M6
  - [`PIPELINE_GAPS.md`](./PIPELINE_GAPS.md) §3.4、§3.5、§3.8、§6.1、§6.2、§7.2
- 目标：
  - typecheck/cast/not-null/pattern 等 runtime value surface 在 MIR 中语义完整。
  - later codegen 缺口不再伪装成 MIR 缺 metadata。

- 必须实现的内容：
  1. `is` / `!is`：
     - `Rvalue::TypeCheck` 携带 source value type、target type、runtime descriptor key、static-foldability、parameterized matching contract。
  2. `as` / `as?`：
     - `Rvalue::Cast` 携带 target type、failure behavior、`Raise<RuntimeError.ClassCastFailed>` effect、`Option<T>` result contract。
  3. `!!`：
     - MIR 显式表达 nullable match、success payload、failure raise。
     - `Nothing`/raise arm result coercion 在 MIR type contract 中可验证。
  4. pattern `is Type`：
     - pattern metadata 区分 static value test 与 runtime ref/interface/class test。
  5. function type runtime cast / effectful function type cast：
     - 若当前不支持，frontend diagnostic。
     - 不允许 parser/typecheck 接收后 MIR 占位。

- 必须遵从的约束：
  - 不要求本任务实现 LLVM lowering。
  - 不允许 MIR 用 default value 表示 cast/typecheck/perform result。

- 验证：
  1. 运行：`cargo test -p scoopc --no-default-features refactor_mir_value_primitives`
  2. 新增 `mir_refactor/runtime_typecheck_cast.scoop`、`mir_refactor/not_null_assert.scoop`、`mir_refactor/pattern_is_type.scoop`。
  3. 负例覆盖 unsupported function type cast。

- 完成条件：
  - runtime type/cast/not-null/pattern surface 在 MIR no-placeholder 且 metadata 完整。
- 依赖：`MIR-T08`

## MIR-T10：收口 aggregate/array/enum/closure transport 的 MIR contract

- 参考：
  - [`PLAN.md`](./PLAN.md) §2/M6
  - [`PIPELINE_GAPS.md`](./PIPELINE_GAPS.md) §3.11、§3.12、§4.1、§4.2、§4.3、§4.4、§4.5、§5.5
- 目标：
  - aggregate/array/enum/closure/effect payload 在 MIR 中都有完整 transport intent。
  - 后续 stage 不需要从 HIR 或 LLVM type lowering 反推 composite value 语义。

- 必须实现的内容：
  1. closure env：
     - MIR env type 支持 arbitrary source type。
     - mutable capture 明确通过 capture box contract 表达。
     - capture field list、trace/copy/drop requirements 可查询。
  2. aggregate boxing：
     - MIR 发布 tuple/struct/value-type boxing intent，用于 `Any`/`Ref`/effect payload/closure capture/array element。
  3. enum payload：
     - Unit field、大整数 payload、nested enum/tuple/struct payload 的 layout intent 不丢失。
     - inline/boxed choice 可以留给 later layout，但 MIR 必须发布 payload schema。
  4. array element transport：
     - MIR array get/set/build contract 包含 element type、copy/trace requirements、composite element support。
  5. effect/function-value adapter surface：
     - MIR call metadata 标明 callee may-outward-effect、aggregate return、adapter/boundary need。
  6. `StoreMember` continuation route：
     - ambiguous route 必须在 MIR stage 或 effect solver handoff 前拆解/诊断，不留给 backend。

- 必须遵从的约束：
  - 不要求本任务实现 final LLVM physical layout。
  - 不允许 MIR 只用 `u64`/ref 双轨隐式代表 composite transport。

- 验证：
  1. 运行：`cargo test -p scoopc --no-default-features refactor_mir_aggregate_transport`
  2. 新增 `mir_refactor/aggregate_transport.scoop`，覆盖 tuple/struct/enum/array/closure capture/effect payload。
  3. 负例覆盖 ambiguous continuation route。

- 完成条件：
  - composite source values 在 MIR 中有 explicit transport contract。
  - 后续 backend gaps 可以明确归类为 layout/codegen 未实现，而不是 MIR 信息缺失。
- 依赖：`MIR-T09`

## MIR-T11：收口 generic root、effect-row args 与 materialization substitution

- 参考：
  - [`PLAN.md`](./PLAN.md) §2/M7
  - [`PIPELINE_GAPS.md`](./PIPELINE_GAPS.md) §2.5、§2.6、§2.7、§2.8、§7.3
- 目标：
  - 所有 generic callable 都有 canonical MIR template 和 materialized instance。
  - type args/effect args 完整进入 instance key。

- 必须实现的内容：
  1. root index 覆盖：
     - top-level generic function
     - member generic function
     - extension generic function
     - generic constructor
     - object/member side-table callable
  2. `InstanceKey` 扩展或核验：
     - type args
     - effect-row args
     - receiver/owner identity
     - callable version
  3. materializer substitution 覆盖所有 MIR metadata：
     - call kind
     - dispatch
     - resume
     - perform
     - handle
     - cast/typecheck
     - aggregate transport
     - closure env
     - top-level roots
  4. 明确 erased carrier exception：
     - 只有标记过的 resume surface 可以保留 erased generic carrier。
     - 普通 source value/frame/call/return 不允许裸 param。
  5. effect-row use-site type args 若仍不支持，frontend diagnostic；若支持，则 materializer 必须消费。

- 必须遵从的约束：
  - 不允许 materializer fallback inference 失败后返回 `None` 并让后端再报错。
  - 不允许 generic member/extension root 只存在于 HIR side table。

- 验证：
  1. 运行：`cargo test -p scoopc --no-default-features refactor_mir_materialize_generics`
  2. 新增 `mir_refactor/generic_materialization.scoop` 覆盖 generic function/member/extension/ctor/effect-row call。
  3. 负例覆盖 missing root、missing template、unsubstituted type/effect param。

- 完成条件：
  - materialized MIR snapshot 完整替换所有 generic/effect params。
  - P4/P5/P6 不再需要 generic HIR template side table 补语义。
- 依赖：`MIR-T10`

## MIR-T12：建立 MIR-only 验证矩阵并完成阶段退出审计

- 参考：
  - [`PLAN.md`](./PLAN.md) §2/M8、§4
  - [`PIPELINE_GAPS.md`](./PIPELINE_GAPS.md) §9
- 目标：
  - 用 MIR-only 验证证明本阶段完成。
  - 明确 later-stage gaps 与 MIR gaps 的边界。

- 必须实现的内容：
  1. 建立或更新 `tests/fixtures/mir_refactor/**` 矩阵，覆盖所有 owner task 的代表样本。
  2. 将 `effect_refactor_pipeline::hir_preflight` 中所有合法 `HirOnly` entry 升级为 MIR smoke，除非该 surface 已改为 frontend reject 负例。
  3. 增加 `dump-mir` golden 或 stable snapshot，确保 CLI、fixture runner、Rust tests 共用同一 formatter/stage helper。
  4. 建立 diagnostics fixture 集，覆盖本阶段决定拒绝的 syntax/surface。
  5. 写阶段退出审计记录，逐项核对：
     - `PIPELINE_GAPS.md` §1 每个 HIR/MIR lowering gap。
     - `PIPELINE_GAPS.md` §2 每个 handoff/materialization gap。
     - §3-§6 中需要 MIR contract 的 backend gap 是否已有 MIR 表达或 frontend reject。
  6. 更新本文件任务状态或完成记录，确保每个 task 有验证命令和结果。

- 必须遵从的约束：
  - 不运行 full fixture suite。
  - 不因为 later-stage LLVM/runtime 缺口降低 MIR golden 或删除样本。
  - 如果某样本 MIR 已完整但后端仍失败，记录为 later-stage gap，而不是回退 MIR 表达。

- 验证：
  1. 运行：`cargo test -p scoopc --no-default-features refactor_hir_preflight`
  2. 运行：`cargo test -p scoopc --no-default-features refactor_mir_no_todo`
  3. 运行：`cargo test -p scoopc --no-default-features refactor_materialized_mir`
  4. 运行：`cargo run -p scoop --no-default-features -- --effect-pipeline refactor test --fixtures tests/fixtures/mir_refactor`
  5. 按需逐个运行新增 diagnostics fixtures。

- 完成条件：
  - refactor direct-style MIR 和 materialized MIR 对验证矩阵 no Todo/no unresolved generic param。
  - 所有 unsupported/deferred surface 均有 frontend diagnostic fixture。
  - `PIPELINE_GAPS.md` 中 MIR-stage scope 的 gap 已关闭或重分类为 later-stage backend/runtime gap。
  - 可以进入下一阶段，不再担心 Todo placeholder 流入 P4/P5/P6。
- 依赖：`MIR-T11`
