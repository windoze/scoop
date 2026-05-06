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
| `MIR-T05` | M3 | 建立完整 MIR program item graph 与 top-level roots |
| `MIR-T04` | M2 | 完成 comptime、splice field、class literal、with-update 的 MIR 前置闭包 |
| `MIR-T06` | M4 | 建立 unified place/lvalue contract 并清理 assignment Todo |
| `MIR-T07` | M5 | 收口 call/ctor/default/named/intrinsic typed call-site contract |
| `MIR-T07R` | M5R | Review MIR-T07 typed call-site contract |
| `MIR-T08` | M5 | 收口 dispatch/resume/perform/handle site contract |
| `MIR-T08R` | M5R | Review MIR-T08 dispatch/resume/perform/handle contract |
| `MIR-T09` | M6 | 收口 runtime value primitives 的 MIR 表达 |
| `MIR-T09R` | M6R | Review MIR-T09 runtime value primitives |
| `MIR-T10` | M6 | 收口 aggregate/array/enum/closure transport 的 MIR contract |
| `MIR-T10R` | M6R | Review MIR-T10 composite transport contract |
| `MIR-T11` | M7 | 收口 generic root、effect-row args 与 materialization substitution |
| `MIR-T11R` | M7R | Review MIR-T11 generic materialization contract |
| `MIR-T12` | M8 | 建立 codegen routing / ABI handoff 守卫 |
| `MIR-T12R` | M8R | Review MIR-T12 codegen handoff guard |
| `MIR-T13` | M8 | 收口 remaining MIR-facing frontend/runtime policy gates |
| `MIR-T13R` | M8R | Review MIR-T13 policy gates |
| `MIR-T14` | M8 | 建立 MIR-only 验证矩阵并完成阶段退出审计 |
| `MIR-T14R` | M8R | Review MIR-T14 phase exit audit |

## 全局约束

- 本文件所有任务只修 refactor 新路径。
- 不允许把 legacy path 的旧 fallback 改成“部分 refactor aware”的混合实现；共享逻辑必须是完全中立 API，否则在 refactor stage 附近单独实现。
- 每个任务完成时都必须保证 refactor production MIR 不新增任何 placeholder。
- 不允许新增 `Todo(...)` reason 后再“稍后处理”；必须先更新 inventory、指定 owner task 和 disposition。
- 不允许让 P4/P5/P6 回 AST/HIR 私有 side table 补语义；本阶段必须把 semantic source of truth 固定在 MIR stage output / materialized MIR / MIR metadata 上。
- 后续涉及 ABI routing、function-value adapter 或 boundary contract 的未完成任务，必须消费 effect facts 中的 `resolved_outward_cases` / `impl_plan` / `CallableAbiKind` 决定 ABI：`impl_plan = NoOutward` 或 `resolved_outward_cases = ∅` 的 body 对外发布 plain ABI；只有 `CallableAbiKind::EffectStep` body 或独立 effect-typed adapter publication 使用 Step/effect ABI。
- 本文件新增的 codegen-facing MIR 任务只发布/校验 handoff contract 与 routing policy；LLVM/runtime 实现任务归 [`TODO-pipeline-gaps-codegen.md`](./TODO-pipeline-gaps-codegen.md)。
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

## [DONE] MIR-T02：落地 materialized MIR strict verifier 与 no-param gate

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

- 完成记录（2026-05-06）：
  - `MaterializedMir::validate_refactor_materialized()` 已作为 materializer 输出边界的 strict verifier 接入，覆盖 raw materialized file 与 pass-view canonical callable bodies。
  - materializer rewrite 入口现在拒绝 `StatementKind::Todo`、`Rvalue::Todo`、`TerminatorKind::Todo`、`UnwindAction::Todo`，不再 no-op 透传 placeholder。
  - materialized verifier 现在检查 instance type/effect args、函数签名、参数、frame slot、return/call arg operand、aggregate/closure/effect metadata、resume/perform/handle metadata 中的 unresolved `TypeKind::Param` / effect-row param，并拒绝未物化的 generic direct call target。
  - `MissingGenericTemplate`、`MissingMirRootForTemplate`、type/effect arg arity error 增加 call-site 字段；site-bound missing template 会携带 source call site。
  - 更新 MIR placeholder inventory：移除 materializer no-op owner entry，并改为断言 materializer 不得恢复 Todo no-op rewrite。
  - 新增 `refactor_materialized_mir_*` 单测，覆盖 Todo template、missing root、裸 type param、effect-row arg 缺失。
  - 验证通过：`cargo test -p scoopc --no-default-features refactor_materialized_mir`。
  - 额外回归通过：`cargo test -p scoopc --no-default-features refactor_mir_placeholder_inventory`。
  - 额外回归通过：`cargo test -p scoopc --no-default-features refactor_mir_no_todo`。
  - 额外 lint 通过：`cargo clippy -p scoopc --no-default-features --all-targets -- -D warnings`。

## [DONE] MIR-T03：收口 parser/frontend/HIR placeholder 入口

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

- 完成记录（2026-05-06）：
  - HIR placeholder inventory 已改用与 MIR inventory 一致的 disposition 语义；`structured_concurrency_spawn_deferred`、`structured_concurrency_join_deferred`、`comptime_if_item`、`missing_expr` 不再作为 HIR/MIR placeholder inventory entry 保留。
  - parser/frontend 现在对 user-facing `spawn` / `join` 给出 `scoop::parse::structured_concurrency_deferred` diagnostic，相关 parse/typecheck fixtures 已覆盖。
  - HIR lowering 对 parser recovery `Missing`、assignment-expression fallback、缺失 splice/with-update contract、未裁剪 package-level `comptime if` 改为记录 HIR stage diagnostic，不再构造 `ExprKind::Missing` 或 `Item::Todo(comptime_if_item)`。
  - refactor typed HIR stage 通过 `RefactorHirCompletenessVerifier` 在 `TypedHirStageOutput::new(...)` 边界拒绝 `Item::Todo`、`StmtKind::Todo`、`ExprKind::Todo`、`ExprKind::Missing`；HIR preflight 的 MIR smoke 使用 test-only unvalidated lowering 只审计 HIR-origin fallback，不再被后续 `MIR-T05` 的 top-level val strict MIR gap 阻塞。
  - 新增 `tests/fixtures/resolve/package_level_comptime_if_untrimmed_is_error.scoop`，覆盖 package-level `comptime if` 无法常量裁剪时的 frontend diagnostic。
  - 验证通过：`cargo test -p scoopc --no-default-features refactor_hir_placeholder_inventory`。
  - 验证通过：`cargo test -p scoopc --no-default-features refactor_hir_preflight`。
  - 额外回归通过：`cargo test -p scoopc --no-default-features refactor_hir_no_todo`。
  - 额外回归通过：`cargo test -p scoopc --no-default-features parser_hir_surface_gate`。
  - diagnostics fixtures 通过：`parse/spawn_deferred_is_error.scoop`、`parse/join_deferred_is_error.scoop`、`parse/parser_recovery_missing_stmt_is_error.scoop`、`typecheck/spawn_deferred_is_error.scoop`、`typecheck/join_deferred_is_error.scoop`、`resolve/package_level_comptime_if_untrimmed_is_error.scoop`。
  - 额外 lint 通过：`cargo clippy -p scoopc --no-default-features --all-targets -- -D warnings`。

## [DONE] MIR-T05：建立完整 MIR program item graph 与 top-level roots

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

- 排序记录（2026-05-06）：
  - `MIR-T04` 的指定验证命令 `cargo run -p scoop --no-default-features -- --effect-pipeline refactor dump-mir tests/fixtures/comptime/splice_field_access_v0_basic.scoop` 当前被 `top-level val` MIR item placeholder 阻塞。
  - 该 blocker 属于本任务范围，因此本任务前移为 `MIR-T04` 的前置任务；不得通过替换 fixture、跳过指定命令或放宽 strict verifier 绕过。

- 完成记录（2026-05-06）：
  - 新增 MIR-owned `InitializerRoot`、`ExternGlobalRoot`、`MetadataRoot` item graph 表达，覆盖 top-level const/runtime val/var/object singleton initializer roots、extern global contract、typealias/nominal/object/extension-property metadata roots。
  - refactor MIR lowering 现在从 typed HIR handoff 发布 declaration graph、top-level initializer roots、extern global roots，并在 refactor path 上跳过旧的 `Item::Todo { kind: "top-level val" }` 降级路径。
  - `RefactorMirStageOutput` 新增 callable/initializer/global/metadata root 查询面；后续阶段可从 MIR stage output 查询 top-level values、extern globals、type/object/typealias metadata 和 object/member callable roots。
  - materialized MIR verifier/known-root 收集已识别新的 MIR root item，避免后续恢复 top-level root 时只认 callable roots。
  - 新增 `tests/fixtures/mir_refactor/top_level_roots.scoop` 和 `refactor_mir_item_graph_publishes_top_level_roots` 单测，覆盖 top-level val、typealias、type、object、extern global。
  - 验证通过：`cargo test -p scoopc --no-default-features refactor_mir_item_graph`。
  - 验证通过：`cargo run -p scoop --no-default-features -- --effect-pipeline refactor dump-mir tests/fixtures/mir_refactor/top_level_roots.scoop`。
  - 额外回归通过：`cargo test -p scoopc --no-default-features refactor_mir_placeholder_inventory`。
  - 额外回归通过：`cargo test -p scoopc --no-default-features refactor_mir_no_todo`。
  - 额外回归通过：`cargo test -p scoopc --no-default-features refactor_materialized_mir`。
  - 额外 lint 通过：`cargo clippy -p scoopc --no-default-features --all-targets -- -D warnings`。
  - `MIR-T04` 原 blocker 验证已通过：`cargo run -p scoop --no-default-features -- --effect-pipeline refactor dump-mir tests/fixtures/comptime/splice_field_access_v0_basic.scoop`。

## [DONE] MIR-T04：完成 comptime、splice field、class literal、with-update 的 MIR 前置闭包

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
- 依赖：`MIR-T03`、`MIR-T05`

- 阻塞记录（2026-05-06）：
  - 指定 splice field `dump-mir` 验证目前会被 `Item::Todo { kind: "top-level val" }` 拒绝，不能通过换 fixture 或降低 verifier 绕过。
  - 已将 `MIR-T05` 前移为本任务前置任务；完成 top-level roots 后再继续本任务。

- 完成记录（2026-05-06）：
  - runtime class literal 已按本阶段 policy 降为 `Rvalue::TypeMetadataLiteral`，保留 source type / FQN，并以 `String` type-name primitive 支持 MIR/codegen/materialization consumers。
  - `refactor_hir_preflight` 中 comptime control-flow、splice field、runtime class literal、struct/tuple/enum with-update 合法样本已从 `HirOnly` 升级为 MIR smoke。
  - 新增 `tests/fixtures/mir_refactor/comptime_splice_class_with_update.scoop`，覆盖 package/runtime comptime expansion、splice field concrete member access、runtime class literal、struct/tuple/enum with-update MIR transport。
  - MIR placeholder inventory 已移除 `class literal MIR lowering pending`；`MIR-T04` fixture 断言输出不含 `Todo`，并包含 `TypeMetadataLiteral`、`MemberAccess`、`StructLit`、`MakeTuple`、`EnumVariant`。
  - 验证通过：`cargo test -p scoopc --no-default-features refactor_hir_comptime`。
  - 验证通过：`cargo test -p scoopc --no-default-features refactor_mir_comptime_splice`。
  - 验证通过：`cargo test -p scoopc --no-default-features refactor_hir_preflight`。
  - 验证通过：`cargo run -p scoop --no-default-features -- --effect-pipeline refactor dump-mir tests/fixtures/comptime/splice_field_access_v0_basic.scoop`。
  - 验证通过：`cargo run -p scoop --no-default-features -- --effect-pipeline refactor dump-mir tests/fixtures/mir_refactor/comptime_splice_class_with_update.scoop`。
  - 额外回归通过：`cargo test -p scoopc --no-default-features refactor_mir_placeholder_inventory`。
  - 额外回归通过：`cargo test -p scoopc --no-default-features refactor_mir_no_todo`。
  - 额外回归通过：`cargo test -p scoopc --no-default-features refactor_materialized_mir`。
  - 额外 lint 通过：`cargo clippy -p scoopc --no-default-features --all-targets -- -D warnings`。

## [DONE] MIR-T06：建立 unified place/lvalue contract 并清理 assignment Todo

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

- 完成记录（2026-05-06）：
  - refactor MIR assignment lowering 现在只消费 typed HIR assignment place contract；缺失 contract、缺失 local symbol、member contract 与 LHS route 不一致等情况均作为前置 HIR invariant，不再构造 refactor-reachable assignment/place Todo。
  - captured mutable local without initializer 已在 frontend/typecheck 以 source diagnostic 拒绝；MIR lowering 不再生成 `Rvalue::Todo("boxed var decl init pending")`。
  - `break` / `continue` outside loop 继续由 typecheck control-flow diagnostic 拒绝；MIR lowering 不再保留对应 terminator Todo 构造。
  - MIR placeholder inventory 已移除 refactor-reachable `MIR-T06` placeholder entries；仅 legacy non-refactor assignment fallback 仍以 `LegacyOnly` bucket 追踪 `assign lhs missing local` / `assign lhs lowering pending`。
  - 新增 `tests/fixtures/mir_refactor/assignment_places.scoop`，覆盖 local、boxed local、top-level var、extern global、direct/nested member store 的 refactor MIR store contract。
  - 新增 diagnostics fixtures：`parse/assignment_call_lhs_is_error.scoop`、`typecheck/local_var_missing_initializer_is_error.scoop`，并复用既有 `break_not_in_loop_is_error.scoop` / `continue_not_in_loop_is_error.scoop`。
  - 验证通过：`cargo test -p scoopc --no-default-features refactor_mir_place_contract`。
  - 验证通过：`cargo run -p scoop --no-default-features -- --effect-pipeline refactor dump-mir tests/fixtures/mir_refactor/assignment_places.scoop`。
  - 验证通过：`cargo test -p scoopc --no-default-features refactor_mir_placeholder_inventory`。
  - 验证通过：`cargo test -p scoopc --no-default-features refactor_mir_no_todo`。
  - 验证通过：`cargo test -p scoopc --no-default-features refactor_hir_preflight`。
  - diagnostics fixtures 通过：`parse/assignment_call_lhs_is_error.scoop`、`typecheck/local_var_missing_initializer_is_error.scoop`、`typecheck/break_not_in_loop_is_error.scoop`、`typecheck/continue_not_in_loop_is_error.scoop`。
  - 额外 lint 通过：`cargo clippy -p scoopc --no-default-features --all-targets -- -D warnings`。

## [DONE] MIR-T07：收口 call/ctor/default/named/intrinsic typed call-site contract

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

- 完成记录（2026-05-06）：
  - refactor MIR call lowering 现在优先消费 typed HIR `TypedCallSiteContract`，覆盖 direct top-level、member direct、extension、constructor、closure、function value、FunPtr、virtual/interface dispatch 和 intrinsic call sites；旧 callee-shape fallback 仅保留给 legacy non-refactor path。
  - `nameOf<T>()` 降为 `Rvalue::TypeMetadataLiteral`，`sizeOf<T>()` / `sizeOf(value)` 降为 `Rvalue::SizeOf`，`getPlatform()` 以 typed intrinsic contract 驱动 direct intrinsic call，不再经过 `sizeOf intrinsic requires one positional arg` 或普通 callee fallback Todo。
  - constructor lowering 现在从 selected constructor contract 生成 `Rvalue::ClassCtor`，并消费 canonical ordered args；named/default/extension receiver args 在 HIR canonicalization 后以 positional MIR `CallArg` 进入调用节点。
  - MIR placeholder inventory 已将 `call callee lowering pending`、`ctor call lowering pending`、`sizeOf intrinsic requires one positional arg` 归入 legacy-only bucket；refactor production no-Todo verifier 继续拒绝这些 reason 泄漏。
  - 新增 `tests/fixtures/mir_refactor/call_contracts.scoop` 和 `refactor_mir_call_contract_lowers_typed_call_sites`，覆盖 direct、generic、named/default、extension、object member、class ctor、top-level function reference/function value、closure、`nameOf`、`sizeOf`、`getPlatform`。
  - `reflection_runtime_fallback_v0.scoop` 与 `get_platform_runtime_ok.scoop` 的 HIR preflight 已升级为 MIR smoke。
  - 验证通过：`cargo test -p scoopc --no-default-features refactor_mir_call_contract`。
  - 验证通过：`cargo run -p scoop --no-default-features -- --effect-pipeline refactor dump-mir tests/fixtures/mir_refactor/call_contracts.scoop`。
  - 验证通过：`cargo test -p scoopc --no-default-features refactor_mir_placeholder_inventory`。
  - 验证通过：`cargo test -p scoopc --no-default-features refactor_mir_no_todo`。
  - 验证通过：`cargo test -p scoopc --no-default-features refactor_hir_preflight`。
  - 额外回归通过：`cargo test -p scoopc --no-default-features refactor_hir_call_contracts_record_callable_provenance`。
  - 额外 lint 通过：`cargo clippy -p scoopc --no-default-features --all-targets -- -D warnings`。

## [DONE] MIR-T07R：Review MIR-T07 typed call-site contract

- 参考：
  - `MIR-T07`
  - [`PLAN.md`](./PLAN.md) §2/M5
  - [`PIPELINE_GAPS.md`](./PIPELINE_GAPS.md) §1.7、§2.6、§3.7、§3.9、§3.10、§6.3
- 重点：
  - typed handoff 是否完整发布 selected callable/ctor/intrinsic、receiver、complete args、named/default/vararg binding、generic/effect-row args、hidden ordinary effects。
  - MIR lowering 是否不再从 HIR call syntax、FQN 字符串、span fallback 或 backend guess 恢复调用语义。
  - class ctor、top-level function reference、runtime fallback intrinsic 是否都有 no-placeholder contract。
- 验证：
  1. 重跑 `MIR-T07` 的全部验证命令。
  2. 抽查 `mir_refactor/call_contracts.scoop` 的 MIR dump/golden，确认 metadata 足够 codegen 消费。
  3. 搜索 `call callee lowering pending`、`ctor call lowering pending`、`sizeOf intrinsic requires one positional arg`，确认 refactor production path 不再命中。
- 完成条件：
  - Review 结论明确说明 `MIR-T07` 已正确实现；若发现缺口，`MIR-T07R` 保持未完成并把修复归回 `MIR-T07`。
- 依赖：`MIR-T07`

- 完成记录（2026-05-06）：
  - Review 结论：`MIR-T07` typed call-site contract 已按任务要求正确实现；未发现需要归回 `MIR-T07` 的阻塞缺口。
  - 已审查 typed HIR `TypedCallSiteContract` 收集、`MirLoweringFacts::from_refactor_typed_handoff(...)` handoff、refactor MIR call lowering、fixture 断言与 placeholder inventory bucket。
  - `call_contracts.scoop` 的 MIR dump 抽查确认 direct/generic/named-default/extension/object-member/class-ctor/top-level function reference/function value/immediate closure/`nameOf`/`sizeOf`/`getPlatform` 均有 no-placeholder MIR 表达，call args 已为 canonical positional args。
  - 搜索审计确认 `call callee lowering pending`、`ctor call lowering pending`、`sizeOf intrinsic requires one positional arg` 的源码命中仅在 legacy fallback、inventory/preflight 审计或测试断言中；refactor production dump 未命中这些 reason。
  - 验证通过：`cargo test -p scoopc --no-default-features refactor_mir_call_contract`。
  - 验证通过：`cargo run -p scoop --no-default-features -- --effect-pipeline refactor dump-mir tests/fixtures/mir_refactor/call_contracts.scoop`。
  - 额外回归通过：`cargo test -p scoopc --no-default-features refactor_hir_call_contracts_record_callable_provenance`。
  - 额外回归通过：`cargo test -p scoopc --no-default-features refactor_hir_preflight`。
  - 额外回归通过：`cargo test -p scoopc --no-default-features refactor_mir_no_todo`。
  - 额外 lint 通过：`cargo clippy -p scoopc --no-default-features --all-targets -- -D warnings`。

## [DONE] MIR-T08：收口 dispatch/resume/perform/handle site contract

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
- 依赖：`MIR-T07R`

- 完成记录（2026-05-06）：
  - typed HIR continuation resume contract 现在发布 receiver route 与 payload arg indices；refactor MIR resume lowering 消费该 route，不再从 canonical callee shape 恢复 continuation receiver/payload。
  - typed perform contract 与 MIR `PerformMetadata` 现在发布 `result_ty`；materialized MIR verifier/substitution 覆盖 perform result type。
  - refactor perform/handle 缺 contract 路径不再构造 `Rvalue::Todo` / `TerminatorKind::Todo`，改由 strict production verifier 以 site metadata diagnostic 拒绝 forged/missing contract。
  - MIR placeholder inventory 已将 resume/dispatch 旧 placeholder 归入 `LegacyOnly`，并移除 refactor perform/handle contract-missing placeholder entries；搜索确认 perform/handle contract-missing Todo 构造已不存在。
  - 新增/重命名 `refactor_mir_effect_site_contract*` 定向测试，覆盖 dispatch、resume、perform、handle metadata，以及缺失 perform/handle typed contract 的 stage error 负例。
  - 验证通过：`cargo test -p scoopc --no-default-features refactor_mir_effect_site_contract`。
  - 验证通过：`cargo run -p scoop --no-default-features -- --effect-pipeline refactor dump-mir tests/fixtures/mir_refactor/dispatch_and_resume_call.scoop`。
  - 验证通过：`cargo run -p scoop --no-default-features -- --effect-pipeline refactor dump-mir tests/fixtures/mir_refactor/handle_perform.scoop`。
  - 验证通过：`cargo run -p scoop --no-default-features -- --effect-pipeline refactor dump-mir tests/fixtures/mir_refactor/handle_finally_boundary.scoop`。
  - 额外回归通过：`cargo test -p scoopc --no-default-features refactor_hir_preflight`。
  - 额外回归通过：`cargo test -p scoopc --no-default-features refactor_materialized_mir`。
  - 额外回归通过：`cargo test -p scoopc --no-default-features refactor_mir_no_todo`。
  - 额外回归通过：`cargo test -p scoopc --no-default-features refactor_mir_call_contract`。
  - 额外回归通过：`cargo test -p scoopc --no-default-features refactor_mir_placeholder_inventory`。
  - 额外 lint 通过：`cargo clippy -p scoopc --no-default-features --all-targets -- -D warnings`。

## [DONE] MIR-T08R：Review MIR-T08 dispatch/resume/perform/handle contract

- 参考：
  - `MIR-T08`
  - [`PLAN.md`](./PLAN.md) §2/M5
  - [`PIPELINE_GAPS.md`](./PIPELINE_GAPS.md) §1.8、§1.9、§1.10、§1.11、§6.5
- 重点：
  - dispatch、resume、perform、handle site 是否都有 stable `SiteId`、source span、typed semantic identity 和 payload/result metadata。
  - continuation resume 是否支持别名、function value、wrapper、extension 等合法 callee shape，只依赖 typed contract。
  - successful MIR 中是否仍残留 canonical-shape-only placeholder 或 contract-missing diagnostic。
- 验证：
  1. 重跑 `MIR-T08` 的全部验证命令。
  2. 抽查 `mir_refactor/dispatch_and_resume_call.scoop`、`handle_perform.scoop`、`handle_finally_boundary.scoop` 的 MIR dump。
  3. 搜索 `resume lowering requires canonical callee shape`、`refactor perform contract missing`、`refactor handle contract missing`，确认 refactor production path 不再命中。
- 完成条件：
  - Review 结论明确说明 `MIR-T08` 已正确实现；若发现缺口，`MIR-T08R` 保持未完成并把修复归回 `MIR-T08`。
- 依赖：`MIR-T08`

- 完成记录（2026-05-06）：
  - Review 结论：`MIR-T08` dispatch/resume/perform/handle site contract 已按任务要求正确实现；未发现需要归回 `MIR-T08` 的阻塞缺口。
  - 已审查 typed HIR `ContinuationResumeSiteContract` / `PerformSiteContract` / `HandleSiteContract` 收集、`MirLoweringFacts::with_refactor_typed_contracts(...)` handoff、refactor MIR dispatch/resume/perform/handle lowering、strict production site metadata verifier、placeholder inventory 和相关 fixtures。
  - `dispatch_and_resume_call.scoop` dump 抽查确认 virtual/interface dispatch 使用 structured owner/member/receiver metadata，resume 使用 typed continuation/resume/answer/out/runtime-error metadata，不依赖 legacy canonical-shape placeholder。
  - `handle_perform.scoop` 与 `handle_finally_boundary.scoop` dump 抽查确认 perform/handle 均有 stable site id、operation identity、payload/result metadata、resume/cleanup/exit targets 和 explicit unwind/finally boundary。
  - 搜索审计确认 `refactor perform contract missing`、`refactor handle contract missing` 不再有源码构造；`resume lowering requires canonical callee shape`、`dispatch callee lowering pending` 源码命中仅保留在 legacy non-refactor fallback、inventory/preflight/verifier/docs 中，refactor production path 不命中。
  - 验证通过：`cargo test -p scoopc --no-default-features refactor_mir_effect_site_contract`。
  - 验证通过：`cargo run -p scoop --no-default-features -- --effect-pipeline refactor dump-mir tests/fixtures/mir_refactor/dispatch_and_resume_call.scoop`。
  - 验证通过：`cargo run -p scoop --no-default-features -- --effect-pipeline refactor dump-mir tests/fixtures/mir_refactor/handle_perform.scoop`。
  - 验证通过：`cargo run -p scoop --no-default-features -- --effect-pipeline refactor dump-mir tests/fixtures/mir_refactor/handle_finally_boundary.scoop`。
  - 额外回归通过：`cargo test -p scoopc --no-default-features refactor_hir_preflight`。
  - 额外回归通过：`cargo test -p scoopc --no-default-features refactor_materialized_mir`。
  - 额外回归通过：`cargo test -p scoopc --no-default-features refactor_mir_no_todo`。
  - 额外回归通过：`cargo test -p scoopc --no-default-features refactor_mir_call_contract`。
  - 额外回归通过：`cargo test -p scoopc --no-default-features refactor_mir_placeholder_inventory`。
  - 额外 lint 通过：`cargo clippy -p scoopc --no-default-features --all-targets -- -D warnings`。

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
- 依赖：`MIR-T08R`

## MIR-T09R：Review MIR-T09 runtime value primitives

- 参考：
  - `MIR-T09`
  - [`PLAN.md`](./PLAN.md) §2/M6
  - [`PIPELINE_GAPS.md`](./PIPELINE_GAPS.md) §3.4、§3.5、§3.8、§6.1、§6.2、§7.2
- 重点：
  - `is` / `!is` / `as` / `as?` / `!!` / pattern `is Type` 的 MIR metadata 是否足以表达 runtime descriptor、failure behavior、ordinary `Raise<RuntimeError>` 和 `Option<T>` result。
  - function type runtime cast 是否固定为 frontend/typecheck diagnostic，而不是留下 MIR 或 codegen placeholder。
  - cast/typecheck/not-null failure 是否没有 default value、panic-only path 或 late unsupported。
- 验证：
  1. 重跑 `MIR-T09` 的全部验证命令。
  2. 抽查 `mir_refactor/runtime_typecheck_cast.scoop`、`not_null_assert.scoop`、`pattern_is_type.scoop`。
  3. 负例确认 unsupported function type cast 不进入 MIR。
- 完成条件：
  - Review 结论明确说明 `MIR-T09` 已正确实现；若发现缺口，`MIR-T09R` 保持未完成并把修复归回 `MIR-T09`。
- 依赖：`MIR-T09`

## MIR-T10：收口 aggregate/array/enum/closure transport 的 MIR contract

- 参考：
  - [`PLAN.md`](./PLAN.md) §2/M6
  - [`PIPELINE_GAPS.md`](./PIPELINE_GAPS.md) §3.11、§3.12、§3.13、§4.1、§4.2、§4.3、§4.4、§4.5、§5.5
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
     - MIR call metadata 标明可 materialize 的 callee `resolved_outward_cases` / `impl_plan` / `CallableAbiKind` facts、aggregate return、adapter/boundary need；`NoOutward` plain body 不得要求 Step/effect body ABI。
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
- 依赖：`MIR-T09R`

## MIR-T10R：Review MIR-T10 composite transport contract

- 参考：
  - `MIR-T10`
  - [`PLAN.md`](./PLAN.md) §2/M6
  - [`PIPELINE_GAPS.md`](./PIPELINE_GAPS.md) §3.11、§3.12、§3.13、§4.1、§4.2、§4.3、§4.4、§4.5、§5.5
- 重点：
  - closure env、value boxing、enum payload、array element、effect/function-value adapter surface 和 `StoreMember` continuation route 是否都有 source value transport contract。
  - GC trace/copy/drop、boxed representation、aggregate return、adapter/boundary need 是否能从 MIR/effect facts materialize。
  - ambiguous continuation route 是否已在 MIR/effect solver handoff 前拆解或诊断。
- 验证：
  1. 重跑 `MIR-T10` 的全部验证命令。
  2. 抽查 `mir_refactor/aggregate_transport.scoop` 的 tuple/struct/enum/array/closure capture/effect payload 样本。
  3. 检查 materialized MIR 中 aggregate/closure/effect metadata 没有裸 type param 或 source-shape fallback。
- 完成条件：
  - Review 结论明确说明 `MIR-T10` 已正确实现；若发现缺口，`MIR-T10R` 保持未完成并把修复归回 `MIR-T10`。
- 依赖：`MIR-T10`

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
- 依赖：`MIR-T10R`

## MIR-T11R：Review MIR-T11 generic materialization contract

- 参考：
  - `MIR-T11`
  - [`PLAN.md`](./PLAN.md) §2/M7
  - [`PIPELINE_GAPS.md`](./PIPELINE_GAPS.md) §2.5、§2.6、§2.7、§2.8、§7.3
- 重点：
  - generic root index 是否覆盖 top-level/member/extension/ctor/object side-table callable。
  - `InstanceKey` / materialization key 是否包含 type args、effect-row args、receiver/owner identity、callable version，并与 spec 的 runtime-erased effect-row 语义不冲突。
  - substitution 是否覆盖 call/dispatch/resume/perform/handle/cast/typecheck/aggregate/closure/top-level roots 等 MIR metadata。
- 验证：
  1. 重跑 `MIR-T11` 的全部验证命令。
  2. 抽查 `mir_refactor/generic_materialization.scoop` 的 materialized MIR。
  3. 负例确认 Todo template、missing root、裸 type param、effect-row arg 缺失均被 verifier 拒绝。
- 完成条件：
  - Review 结论明确说明 `MIR-T11` 已正确实现；若发现缺口，`MIR-T11R` 保持未完成并把修复归回 `MIR-T11`。
- 依赖：`MIR-T11`

## MIR-T12：建立 codegen routing / ABI handoff 守卫

- 参考：
  - [`PLAN.md`](./PLAN.md) §2/M8
  - [`PIPELINE_GAPS.md`](./PIPELINE_GAPS.md) §3.1、§3.2、§3.3、§3.6、§5.1、§5.2、§5.4
  - [`TODO-pipeline-gaps-codegen.md`](./TODO-pipeline-gaps-codegen.md) CG-T01、CG-T05、CG-T06
- 目标：
  - 防止 direct-style effect/control MIR 或 unsupported call kind 误入 raw MIR LLVM path。
  - 让 ABI handoff 明确表达 plain vs EffectStep，且只由 P5/P6 发布的 `resolved_outward_cases`、`impl_plan` 和 `CallableAbiKind` 决定。
  - 把 unsupported source classification 和 ABI 漂移前移到 MIR/effect-lowered handoff verifier，而不是 LLVM body emission 才失败。

- 必须实现的内容：
  1. 为每个 materialized callable 发布 codegen routing facts：
     - 是否含 `Handle`、`ResumeUnwind`、`Perform`、`PerformResult`、`Virtual`、`Interface`、`Resume` call kind。
     - 允许的 backend route：plain raw MIR、plain local-control handoff、EffectStep lowering、或 frontend reject。
     - route 选择理由和 source span / body FQN。
  2. strict handoff verifier 必须拒绝：
     - raw MIR route 中仍有 raw backend 不支持的 effect/control terminator 或 call kind。
     - `PerformResult` 没有 resume payload injection / binding contract 却进入 raw value emission。
     - plain ABI body 中残留未局部化的 `Perform` / `Handle` / `ResumeUnwind`。
     - `impl_plan = NoOutward` 或 `resolved_outward_cases = ∅` 的 body 发布 EffectStep body ABI，或 `CallableAbiKind::EffectStep` 缺 boundary/adapter contract。
  3. ABI publication 必须坚持 effect facts 规则：
     - `impl_plan = NoOutward` 或 `resolved_outward_cases = ∅` 的 body 发布 plain ABI。
     - 非空 `resolved_outward_cases` 的 body 才能按 `CallableAbiKind::EffectStep` 发布 EffectStep body 或 effect boundary。
     - effect-typed adapter 是独立 publication，可以把 plain body 返回值包装为 `Step_F::Complete`，但不改变 plain body ABI。
     - `main(args: Array<String>) / Pure!` 继续使用 plain argv ABI；不得为 `NoOutward` plain body 引入 Step argv ABI。
  4. late-lowered source classifications：
     - `Unsupported` 默认在 verifier fail-fast。
     - 若存在 intentional elide/skip，必须带 explicit reason、source span 和 owner task，不能作为 production success path 静默通过。
  5. `dump-mir` / materialized MIR / effect-lowered preflight 要能显示 routing facts 与 ABI kind，供 codegen TODO 接续实现。

- 必须遵从的约束：
  - 本任务不实现 LLVM lowering；只发布/校验 MIR-to-codegen handoff contract。
  - 不允许 backend 回 HIR、span guess、legacy handler-stack 或 old callable wrapper 补语义。
  - 不允许用 complete-only `Step_F` 代表 `NoOutward` plain body。

- 验证：
  1. 运行：`cargo test -p scoopc --no-default-features refactor_mir_codegen_routing_contract`
  2. 运行：`cargo test -p scoopc --no-default-features refactor_materialized_mir_codegen_route_verifier`
  3. 新增/更新 `mir_refactor/codegen_routing_contracts.scoop`，覆盖 raw-safe plain、plain-local effect/control、EffectStep boundary、dynamic dispatch、resume、perform-result。
  4. 负例覆盖 `NoOutward` body 发布 EffectStep、raw route 含 unsupported terminator/call kind、unsupported source classification 被 verifier 拒绝。

- 完成条件：
  - raw MIR / refactor LLVM route selection 已由 materialized facts 与 ABI handoff 驱动。
  - `PIPELINE_GAPS.md` §3.1、§3.2、§3.3、§3.6、§5.1、§5.2、§5.4 的 MIR-facing 部分有明确 verifier 或 publication。
  - 剩余实现工作可无歧义转交 codegen task。
- 依赖：`MIR-T11R`

## MIR-T12R：Review MIR-T12 codegen handoff guard

- 参考：
  - `MIR-T12`
  - [`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md) §3.3、§3.5、§5.4、§5.6、§8
  - [`TODO-pipeline-gaps-codegen.md`](./TODO-pipeline-gaps-codegen.md) CG-T01、CG-T05、CG-T06
- 重点：
  - routing facts 是否足以阻止 direct-style effect/control MIR 或 unsupported call kind 误入 raw MIR LLVM path。
  - ABI handoff 是否只消费 `resolved_outward_cases`、`impl_plan`、`CallableAbiKind`，并正确区分 plain body、plain-local handoff、EffectStep body、effect-typed adapter。
  - `Unsupported` source classification、residual effect/control terminator、Step argv ABI 漂移是否都在 verifier 阶段 fail-fast。
- 验证：
  1. 重跑 `MIR-T12` 的全部验证命令。
  2. 抽查 `mir_refactor/codegen_routing_contracts.scoop` 的 materialized MIR / effect-lowered output。
  3. 负例确认 `NoOutward` body 发布 EffectStep、raw route 含 unsupported terminator/call kind、unsupported source classification 均被拒绝。
- 完成条件：
  - Review 结论明确说明 `MIR-T12` 已正确实现；若发现缺口，`MIR-T12R` 保持未完成并把修复归回 `MIR-T12`。
- 依赖：`MIR-T12`

## MIR-T13：收口 remaining MIR-facing frontend/runtime policy gates

- 参考：
  - [`PLAN.md`](./PLAN.md) §2/M8
  - [`PIPELINE_GAPS.md`](./PIPELINE_GAPS.md) §5.3、§5.6、§7.1、§7.6
  - [`TODO-pipeline-gaps-codegen.md`](./TODO-pipeline-gaps-codegen.md) CG-T06、CG-T07
- 目标：
  - 对仍会影响 MIR/codegen coverage 的 frontend/runtime policy gap 给出明确 contract 或早期 diagnostic。
  - 避免 `ResumeUnwind`、cross-thread effect propagation、or-pattern binder、GC pin/handle intrinsic 在后端变成 late fatal 或 shape guess。

- 必须实现的内容：
  1. `ResumeUnwind` / cleanup / finally pending completion：
     - MIR 或 late-lowered handoff 必须发布 unwind payload carrier、cleanup continuation、pending completion、origin/resume-state provenance。
     - 若某路径暂不支持，必须在 verifier 阶段给出 contract-missing diagnostic，不允许 LLVM body emission 才发现。
  2. cross-thread resume 后 non-complete Step：
     - 若语言当前不支持跨线程继续向外传播 effect，type/effect checker 或 MIR handoff 必须拒绝该 surface。
     - 若允许，则必须发布 cross-thread effect propagation contract，交给 codegen/runtime task 实现。
  3. or-pattern binder：
     - 维持 frontend reject 时，diagnostic fixture 必须覆盖且说明不能进入 HIR/MIR。
     - 若改为支持，HIR/MIR pattern contract 必须发布 binder identity、scope、dominance 与 payload type，不允许后端推断。
  4. GC pin/handle intrinsic surface：
     - 若当前支持，MIR intrinsic metadata 必须包含 root lifetime、pin/unpin pairing、unsafe requirement、trace/copy constraints。
     - 若当前延期，parser/typecheck 诊断必须在进入 MIR 前触发。
  5. 更新 MIR preflight denylist/allowlist，确保上述 policy gap 要么有 MIR contract，要么有 frontend diagnostic fixture。

- 必须遵从的约束：
  - 不把 runtime/codegen 尚未实现写成 MIR `Todo` 或 late LLVM unsupported。
  - 不通过缩小 fixture、legacy selector 或 hidden fallback 绕过 policy 决策。

- 验证：
  1. 运行：`cargo test -p scoopc --no-default-features refactor_mir_policy_gates`
  2. 新增 diagnostics fixtures 覆盖 or-pattern binder、unsupported cross-thread outward propagation、unsupported GC pin/handle surface。
  3. 新增 MIR/effect-lowered smoke 覆盖已支持的 `ResumeUnwind` / cleanup contract 样本。

- 完成条件：
  - `PIPELINE_GAPS.md` §5.3、§5.6、§7.1、§7.6 的 MIR-facing 部分已归类为 published contract 或 frontend diagnostic。
  - codegen/runtime 后续任务不需要猜测这些 surface 的 source semantics。
- 依赖：`MIR-T12R`

## MIR-T13R：Review MIR-T13 policy gates

- 参考：
  - `MIR-T13`
  - [`SCOOP_FULL_SPEC.md`](./SCOOP_FULL_SPEC.md) §4、§5、§15.10
  - [`EFFECT_REFACTOR.md`](./EFFECT_REFACTOR.md) §5.6、§8
- 重点：
  - `ResumeUnwind` / cleanup / finally pending completion 是否发布 enough handoff，或以明确 contract-missing diagnostic 拒绝暂不支持路径。
  - cross-thread resume、or-pattern binder、GC pin/handle intrinsic surface 是否已归类为 spec-compatible contract 或 frontend diagnostic，不再晚到 backend 猜 shape。
  - 所有 diagnostics fixture 是否说明 surface 为什么不能进入 HIR/MIR。
- 验证：
  1. 重跑 `MIR-T13` 的全部验证命令。
  2. 抽查新增 diagnostics fixtures 与 MIR/effect-lowered smoke。
  3. 检查 preflight denylist/allowlist 不把 runtime/codegen 缺口写成 MIR `Todo`。
- 完成条件：
  - Review 结论明确说明 `MIR-T13` 已正确实现；若发现缺口，`MIR-T13R` 保持未完成并把修复归回 `MIR-T13`。
- 依赖：`MIR-T13`

## MIR-T14：建立 MIR-only 验证矩阵并完成阶段退出审计

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
     - §3-§7 中需要 MIR contract、routing policy 或 frontend reject 的 gap 是否已有明确 owner。
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
  - `PIPELINE_GAPS.md` 中 MIR-stage scope 的 gap 已关闭或重分类为 later-stage backend/runtime gap，并链接到 codegen TODO owner。
  - 可以进入下一阶段，不再担心 Todo placeholder 流入 P4/P5/P6。
- 依赖：`MIR-T13R`

## MIR-T14R：Review MIR-T14 phase exit audit

- 参考：
  - `MIR-T14`
  - [`PLAN.md`](./PLAN.md) §4
  - [`PIPELINE_GAPS.md`](./PIPELINE_GAPS.md) §9
- 重点：
  - MIR-only 验证矩阵是否覆盖所有未完成 owner task 的代表样本和本阶段拒绝的 diagnostics surface。
  - 阶段退出审计是否逐项核对 `PIPELINE_GAPS.md` §1-§2，并为 §3-§7 中 MIR-facing contract / routing policy / frontend reject gap 标明 owner。
  - later-stage LLVM/runtime gap 是否已链接到 codegen TODO，而不是通过降低 MIR golden 或删除样本绕过。
- 验证：
  1. 重跑 `MIR-T14` 的全部验证命令。
  2. 复查阶段退出审计记录与本文件任务完成记录。
  3. 抽查新增/更新的 `tests/fixtures/mir_refactor/**` 和 diagnostics fixtures。
- 完成条件：
  - Review 结论明确说明 `MIR-T14` 已正确实现，MIR 阶段可以交接到 codegen 阶段；若发现缺口，`MIR-T14R` 保持未完成并把修复归回 `MIR-T14`。
- 依赖：`MIR-T14`
