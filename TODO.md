# TODO：HIR Completeness 新路径收口

> 生成时间：2026-05-06
> 计划基线：[`PLAN.md`](./PLAN.md)
> Gap 基线：[`PIPELINE_GAPS.md`](./PIPELINE_GAPS.md)
> 格式参考：[`TODO-effect-refactor.md`](./TODO-effect-refactor.md) 与 `TODO-Px.md` 执行任务文件
> 前置条件：effect-refactor 新路径入口已存在，`dump-hir --effect-pipeline refactor` 可进入 refactor typed HIR stage。
> 顺序约束：严格按当前文件顺序推进；不得跨条目并行实现。
> 本阶段目标：关闭所有现有 HIR gap，使 refactor typed HIR handoff 不再包含 `Todo(...)` 或等价 placeholder；支持的 spec surface 必须完整进入 HIR，不支持或延期的 surface 必须在 parser/typecheck/comptime/HIR stage 给清晰诊断。

## 全局约束

- 本任务列表只覆盖 effect-refactor 新路径。
- 旧 legacy path 可以保持现状，不得为了本阶段去修 legacy 行为。
- 不得在旧 HIR/MIR 业务函数中加入 `if refactor { ... } else { ... }` 式混线逻辑。
- 若共享实现不能做到完全中立单一 API，则为 refactor 新路径建立独立 stage/helper。
- 每个任务完成时必须更新或新增定向测试；不要求运行 full fixtures。
- 不得把 MIR/LLVM 后端缺口作为保留 HIR Todo 的理由。
- `dump-hir --effect-pipeline refactor` 对 unsupported input 应失败并打印明确诊断，而不是输出含 Todo 的 HIR。
- 所有 negative fixture 的错误消息必须能定位到用户源码 span，并说明该 surface 是延期、非法上下文、缺 typed metadata，还是当前 unsupported spec subset。
- 每个任务如果发现新的 HIR placeholder reason，必须追加到 HIR completeness verifier 的清单或直接消除。

## 任务索引

| ID | 标题 |
| --- | --- |
| `HIR-T00` | 审计并冻结 refactor HIR placeholder inventory |
| `HIR-T01` | 建立 refactor HIR no-Todo verifier 与 stage error 通道 |
| `HIR-T02` | 在 parser 拒绝纯语法延期/非法 surface |
| `HIR-T03` | 收口 comptime block/if/for 与 package-level comptime if |
| `HIR-T05` | 为 typealias/type/object/extension property 建立 HIR declaration graph |
| `HIR-T04` | 收口 splice field `value.[field]` |
| `HIR-T06` | 收口 array literal、named/default/spread args 与 call arg canonicalization |
| `HIR-T07` | 发布 callable callee provenance 与 dispatch/ctor/intrinsic HIR contract |
| `HIR-T08` | 收口 class literal 与 reflection/platform intrinsic HIR contract |
| `HIR-T09` | 收口 `with` copy-update aggregate metadata |
| `HIR-T10` | 建立 assignment LHS / HIR place contract |
| `HIR-T11` | 收口 custom iterator for-loop 与 remaining debug fallbacks |
| `HIR-T12` | 建立 top-level init/storage/object metadata handoff |
| `HIR-T13` | 建立 HIR -> next-stage preflight，阻止 HIR gap 流入 MIR |
| `HIR-T14` | 冻结 HIR completeness 验证矩阵与阶段完成记录 |

## [DONE] HIR-T00：审计并冻结 refactor HIR placeholder inventory

- 参考：
  - [`PLAN.md`](./PLAN.md) §2/H0
  - [`PIPELINE_GAPS.md`](./PIPELINE_GAPS.md) §1.1-§1.12
- 目标：
  - 找出所有可能在 refactor HIR handoff 中出现的 `Todo(...)` / `Missing` / fallback sentinel。
  - 建立一份可执行 inventory，后续任务逐项清零。

- 必须实现的内容：
  1. 搜索并记录 `crates/scoopc/src/hir/**` 中所有 `ExprKind::Todo`、`StmtKind::Todo`、`Item::Todo` 构造点。
  2. 搜索 refactor HIR stage 输出中是否还会携带来自 legacy `LoweredHir` 的 dump-only fallback。
  3. 把每个 placeholder reason 分类为：parser 应拒绝、typecheck/comptime 应诊断、HIR 应实现、HIR handoff contract 应补齐、legacy-only 可忽略。
  4. 在代码注释或测试 fixture 中固定 reason 清单，避免后续新增未分类 reason。
  5. 明确哪些 reason 属于本阶段必须消除：`comptime_*`、`splice_field`、`class_lit`、`typealias`、`type`、`object`、`array_lit`、`spread_arg`、`named_arg`、`structured_concurrency_*`、`assign`、`with_update`、`missing_stmt`、`for_custom_iterator`、`extension_property_no_getter`。

- 必须遵从的约束：
  - 不修改 legacy output 以追求 inventory 通过。
  - 不把 “后端还不支持” 写成 HIR placeholder 的保留理由。

- 验证：
  - `cargo test -p scoopc --no-default-features refactor_hir_placeholder_inventory`
  - 额外搜索：`rg "ExprKind::Todo|StmtKind::Todo|Item::Todo" crates/scoopc/src/hir`

- 完成条件：
  - 当前所有 HIR placeholder 构造点都有处理策略。
  - 可进入 `HIR-T01`。
- 完成记录（2026-05-06）：
  - 新增 `crates/scoopc/src/hir/lower/placeholder_inventory.rs`，以可执行测试冻结 `src/hir/**` 中的 `ExprKind::Todo`、`StmtKind::Todo`、`Item::Todo` 与当前 `ExprKind::Missing` sentinel 清单。
  - 每个 reason 已分类到 parser 拒绝、typecheck/comptime 诊断、HIR 实现或 HIR handoff contract，并绑定后续 owner task；明确当前 refactor HIR handoff 没有 legacy `lower_for_dump` dump-only fallback。
  - 为保持验证无警告，将仅 LLVM test 使用的 `LateLoweredSurfaceResumeDispatchInventoryEntry::new` gate 到 `all(test, feature = "llvm")`。
  - 已运行：`cargo test -p scoopc --no-default-features refactor_hir_placeholder_inventory`、`rg "ExprKind::Todo|StmtKind::Todo|Item::Todo" crates/scoopc/src/hir`、`cargo clippy -p scoopc --no-default-features --all-targets -- -D warnings`。
- 依赖：无

## [DONE] HIR-T01：建立 refactor HIR no-Todo verifier 与 stage error 通道

- 参考：
  - [`PLAN.md`](./PLAN.md) §2/H0, §2/H2
- 目标：
  - 让 refactor typed HIR stage 成功输出的前提变成“无 Todo、无 Missing、无缺 contract”。
  - 把 HIR lowering 失败改成结构化 diagnostic，而不是 placeholder。

- 必须实现的内容：
  1. 新增 `RefactorHirCompletenessVerifier` 或等价 API。
  2. verifier 遍历 `hir::File`、member fun side table、object init/top-level init roots、typed HIR effect/call/place/copy-update side tables。
  3. 遇到 `Item::Todo`、`StmtKind::Todo`、`ExprKind::Todo`、`ExprKind::Missing`，或由 parser recovery 传入的 missing statement sentinel 时返回 `HirStageError`。
  4. `HirStageError` 必须包含 source path、span、reason、所属 item/function。
  5. refactor `dump-hir` 和所有 refactor HIR stage helper 默认执行 verifier。
  6. legacy `dump-hir` 可继续走旧行为。

- 必须遵从的约束：
  - verifier 不能只扫描顶层 `File.items`，必须扫描所有下游可达 HIR body。
  - 不允许通过跳过 side table/body 的方式绕过 verifier。

- 验证：
  - `cargo test -p scoopc --no-default-features refactor_hir_no_todo`
  - `cargo test -p scoop --no-default-features dump_hir`
  - 构造一个测试输入触发每类 current placeholder，确认 refactor HIR stage 失败并给出 diagnostic。

- 完成条件：
  - refactor HIR stage 不可能成功产出含 Todo handoff。
  - 可进入 `HIR-T02`。
- 完成记录（2026-05-06）：
  - 新增 `crates/scoopc/src/effect_refactor_pipeline/hir_completeness.rs`，实现 `RefactorHirCompletenessVerifier`，默认扫描 refactor typed HIR `File.items`、member fun side table、top-level init roots、object init roots、class init/ctor roots中的所有 HIR body。
  - 新增结构化 `HirStageError` 并接入 `HirLowerError::Stage`，错误携带 source path、span、reason 与所属 item/function；`TypedHirStageOutput::new`、refactor `hir_stage::run`、refactor LLVM handoff 入口默认执行 verifier，legacy wrapper 保留 unchecked 构造以维持 legacy 行为。
  - 新增 no-Todo 单测覆盖当前 placeholder inventory 中的 `Item::Todo`、`StmtKind::Todo`、`ExprKind::Todo`、`ExprKind::Missing` reason，并覆盖 member fun、top-level init、object init、class init roots；新增真实 `typealias` 输入验证 stage diagnostic。
  - 调整 `dump-hir` parity 测试 fixture，避免继续用会被 no-Todo gate 正确拒绝的 declaration-placeholder run-pass 输入证明成功路径。
  - 已运行：`cargo test -p scoopc --no-default-features refactor_hir_no_todo`、`cargo test -p scoopc --no-default-features refactor_typed_hir`、`cargo test -p scoop --no-default-features dump_hir`、`cargo clippy -p scoopc -p scoop --no-default-features --all-targets -- -D warnings`。
- 依赖：`HIR-T00`

## [DONE] HIR-T02：在 parser 拒绝纯语法延期/非法 surface

- 参考：
  - [`PLAN.md`](./PLAN.md) §2/H1
  - [`PIPELINE_GAPS.md`](./PIPELINE_GAPS.md) §7.4
- 目标：
  - 不让纯语法即可判定为延期或非法上下文的 AST 进入 HIR。

- 必须实现的内容：
  1. parser 对 user-facing `spawn` 给出 deferred feature diagnostic。
  2. parser 对 user-facing `join` 给出 deferred feature diagnostic。
  3. parser 禁止 assignment expression 出现在表达式上下文；assignment 仅保留 statement form。
  4. parser 禁止 `*arg` / spread arg 出现在 call argument list 之外。
  5. parser 禁止 named arg 出现在 call argument list 之外。
  6. parser diagnostics 必须说明当前 surface 不能进入 HIR，避免用户看到后端 unsupported。

- 必须遵从的约束：
  - 不拒绝 `comptime` 和 `value.[field]`，它们是本阶段必须支持的 spec surface。
  - 不破坏已有合法 call arg 语法。

- 验证：
  - 新增 parse/typecheck negative fixtures：`spawn_deferred_is_error`、`join_deferred_is_error`、`assignment_expression_is_error`、`spread_arg_outside_call_is_error`、`named_arg_outside_call_is_error`。
  - `cargo test -p scoopc --no-default-features parser_hir_surface_gate`
  - `cargo run -p scoop --no-default-features -- --effect-pipeline refactor test --fixtures <新增 fixtures>`

- 完成条件：
  - 上述 surface 不再能触达 refactor HIR lowering。
  - 可进入 `HIR-T03`。
- 完成记录（2026-05-06）：
  - parser 新增 HIR surface gate diagnostic：`spawn { ... }` / `join expr` 现在以 `scoop::parse::structured_concurrency_deferred` 在 parser 阶段拒绝，并说明不能进入 HIR。
  - 表达式入口默认拒绝 assignment expression，assignment 仅保留 block statement form；call 参数列表内的 named/spread args 继续合法。
  - call 外 spread arg 与数组等普通表达式列表中的 named arg 现在分别以 `scoop::parse::spread_arg_outside_call` / `scoop::parse::named_arg_outside_call` 拒绝。
  - 注解参数 `name = value` 改为结构化 `AnnotationArg.name`，避免继续把合法注解参数建模为 assignment expression。
  - 新增 parse negative fixtures：`spawn_deferred_is_error`、`join_deferred_is_error`、`assignment_expression_is_error`、`spread_arg_outside_call_is_error`、`named_arg_outside_call_is_error`；更新既有 spawn/join typecheck fixtures 为 parser diagnostic 期望。
  - 已运行：`cargo test -p scoopc --no-default-features parser_hir_surface_gate`、`cargo run -p scoop --no-default-features -- --effect-pipeline refactor test --fixtures tests/fixtures/parse`、受影响 typecheck fixtures（spawn/join/experimental annotation 系列）、`cargo clippy -p scoopc -p scoop --no-default-features --all-targets -- -D warnings`。
- 依赖：`HIR-T01`

## [DONE] HIR-T03：收口 comptime block/if/for 与 package-level comptime if

- 参考：
  - [`PLAN.md`](./PLAN.md) §2/H3
  - [`PIPELINE_GAPS.md`](./PIPELINE_GAPS.md) §1.1, §1.5
- 目标：
  - spec 支持的 comptime control flow 在进入 runtime HIR 前完成展开/裁剪。

- 必须实现的内容：
  1. 在 refactor HIR stage 前建立 comptime expansion pass，输入 typed/resolved AST 或等价 typed surface。
  2. `comptime if` 求值为 true/false 后只保留选中 branch。
  3. `comptime for` 遍历 compile-time collection 并展开为普通 statements/items。
  4. `comptime block` 的 compile-time declarations / generated code 能进入后续 HIR lowering 输入。
  5. package-level `comptime if` 展开为普通 top-level items。
  6. expansion 失败时给出 source diagnostic，不生成 `comptime_*` Todo。
  7. HIR verifier 增加断言：runtime HIR 中不得出现任何 comptime placeholder。

- 必须遵从的约束：
  - 不把 comptime control flow lower 成 runtime `if` / loop，除非 spec 语义明确要求。
  - 不让后续 MIR/effect facts 再回 AST 做 expansion。

- 验证：
  - `cargo test -p scoopc --no-default-features refactor_hir_comptime`
  - `cargo run -p scoop --no-default-features -- --effect-pipeline refactor dump-hir tests/fixtures/comptime/splice_field_access_v0_basic.scoop`
  - 新增 HIR fixtures 覆盖 function body `comptime if/for` 与 package-level `comptime if`。

- 完成条件：
  - `comptime_block`、`comptime_if`、`comptime_for`、`comptime_if_item` 不再出现在 refactor HIR handoff。
  - 可进入 `HIR-T05`。
- 完成记录（2026-05-06）：
  - 新增 `RuntimeComptimePlan` 与 `plan_runtime_comptime_in_file`，在 refactor typed HIR lowering 前基于已 resolved/typechecked AST 求值语句级 `comptime if` 条件与 `comptime for` 迭代集合。
  - refactor typed HIR lowering 现在消费该计划：`comptime block` 直接内联生成语句，`comptime if` 只 lower 选中分支，`comptime for` 按编译期迭代值展开 body；基础编译期 loop binder 可 lower 为 HIR 合成 literal，并为 unroll body 内局部声明重映射 synthetic span，避免重复 SymbolId。
  - typecheck 为 `comptime for` body 注入 binder 类型（覆盖 `IntProgression`、`Array`/`MutableArray`、`ComptimeList` 与 tuple 的当前可推导元素类型），使展开体中的 binder 引用能进入 typed HIR。
  - 新增单测 `refactor_hir_comptime_expands_block_if_for_and_package_if`，覆盖 function body `comptime block/if/for`、package-level `comptime if`、以及 `comptime for` binder literal lowering；新增 HIR fixture `tests/fixtures/hir/refactor_comptime_control_flow.{scoop,hir}`。
  - 已运行：`cargo test -p scoopc --no-default-features refactor_hir_comptime`、`cargo run -p scoop --no-default-features -- --effect-pipeline refactor test --fixtures tests/fixtures/hir/refactor_comptime_control_flow.scoop`、`cargo test -p scoopc --no-default-features refactor_hir_no_todo`、`cargo test -p scoop --no-default-features dump_hir`、`cargo clippy -p scoopc -p scoop --no-default-features --all-targets -- -D warnings`。
  - 已尝试任务列出的 `cargo run -p scoop --no-default-features -- --effect-pipeline refactor dump-hir tests/fixtures/comptime/splice_field_access_v0_basic.scoop`；当前仍在进入 HIR 前以既有 `scoop::typecheck::missing_type_annotation`（顶层 `P`）诊断失败，未生成 comptime placeholder。该 fixture 的 type declaration/splice HIR 收口仍归后续 `HIR-T05`/`HIR-T04`。
- 依赖：`HIR-T02`

## [DONE] HIR-T05：为 typealias/type/object/extension property 建立 HIR declaration graph

- 参考：
  - [`PLAN.md`](./PLAN.md) §2/H4
  - [`PIPELINE_GAPS.md`](./PIPELINE_GAPS.md) §1.5
- 目标：
  - HIR file 能完整描述当前前端接受的 declaration，不再用 `Item::Todo` 占位。

- 必须实现的内容：
  1. 扩展 HIR `Item` 或新增 declaration side table，表达 typealias resolved form。
  2. 表达 class/struct/enum/interface type declaration 的 nominal identity、kind、params、fields、constructors、member funcs、interfaces。
  3. 表达 object declaration 的 singleton identity、member declarations、initializer root。
  4. 表达 extension property getter/setter contract。
  5. 缺 getter 且被读取的 extension property 在 typecheck/HIR stage 诊断，不再 `extension_property_no_getter`。
  6. 更新 refactor HIR stable dump，展示 declaration graph 的关键字段。

- 必须遵从的约束：
  - 不要求 legacy HIR dump 改成新 declaration graph。
  - 不把 type/object 的语义只留在 resolver/typecheck 私有表中。

- 验证：
  - `cargo test -p scoopc --no-default-features refactor_hir_decls`
  - 新增/更新 HIR fixtures 覆盖 typealias、class、struct、enum、interface、object、extension property。
  - `cargo run -p scoop --no-default-features -- --effect-pipeline refactor dump-hir <上述 fixtures>`

- 完成条件：
  - `Item::Todo("typealias" / "type" / "object" / "extension_property_no_getter")` 不再可达 refactor HIR。
  - 可进入 `HIR-T04`。
- 调整记录（2026-05-06）：
  - `HIR-T04` 的指定 `dump-hir` 验证 fixture 含本地 `struct Point`；在 refactor no-Todo verifier 下会先被 `Item::Todo(type)` 拒绝。
  - 因此 declaration graph 是 `HIR-T04` 的具体 prerequisite，先完成本任务，再回到 splice field HIR lowering。
- 完成记录（2026-05-06）：
  - `hir::File` 新增 `decls` declaration graph，并以自定义 Debug 保持无声明文件的既有 dump 形状；新增 `Decl`、`TypeAliasDecl`、`NominalDecl`、`ObjectDecl`、`ExtensionPropertyDecl` 及字段/构造器/成员/访问器 contract 结构。
  - 新增 `crates/scoopc/src/hir/lower/decls.rs`，refactor typed HIR lowering 现在为 typealias、class/struct/enum/interface、object 与 extension property 生成结构化 declaration graph；extension property getter 继续合成为可调用 HIR function，缺 getter 不再生成 `Item::Todo(extension_property_no_getter)`。
  - refactor typed HIR 入口现在执行 property declaration check；读取缺 getter 的 extension property 会在进入 HIR 前得到 `scoop::typecheck::extension_property_getter_required` 诊断，而不是落入 HIR placeholder 或通用后端 unsupported。
  - 已移除 `Item::Todo("typealias" / "type" / "object" / "extension_property_no_getter")` 构造点，并同步 placeholder inventory 与 no-Todo verifier 测试期望。
  - 新增 `tests/fixtures/hir/refactor_decl_graph.scoop/.hir` 覆盖 typealias、interface、class、struct、enum、object、extension property；同步受 declaration graph 输出影响且可通过当前 no-Todo gate 的既有 HIR golden。
  - 已运行：`cargo test -p scoopc --no-default-features refactor_hir_decls`、`cargo test -p scoopc --no-default-features refactor_hir_placeholder_inventory`、`cargo test -p scoopc --no-default-features refactor_hir_no_todo`、更新后的 HIR snapshot fixtures、`cargo run -q -p scoop --no-default-features -- --effect-pipeline refactor dump-hir tests/fixtures/hir/refactor_decl_graph.scoop`、`cargo test -p scoop --no-default-features dump_hir`、`cargo clippy -p scoopc -p scoop --no-default-features --all-targets -- -D warnings`。
- 依赖：`HIR-T03`

## [DONE] HIR-T04：收口 splice field `value.[field]`

- 参考：
  - [`PLAN.md`](./PLAN.md) §2/H3
  - [`PIPELINE_GAPS.md`](./PIPELINE_GAPS.md) §1.2
- 目标：
  - `value.[field]` 在 HIR 中变成具体 typed field/member access。

- 必须实现的内容：
  1. typecheck/comptime 发布 splice field contract：receiver type、field name、field owner、field type、是否 mutable/place。
  2. HIR lowering 直接消费该 contract，构造普通 `MemberAccess` 或 HIR place。
  3. 对 string literal field、comptime variable field、reflection loop field 都建立覆盖。
  4. field 无法静态解析时，diagnostic 要说明 `.[field]` 要求 compile-time known field name。
  5. HIR verifier 禁止 `splice_field` reason。

- 必须遵从的约束：
  - MIR 不能再从 `SpliceField` AST shape 恢复 field。
  - 不能把 unresolved splice field 降成 dynamic reflection fallback，除非 spec 明确支持且 HIR contract 已定义。

- 验证：
  - `cargo test -p scoopc --no-default-features refactor_hir_splice_field`
  - `cargo run -p scoop --no-default-features -- --effect-pipeline refactor test --fixtures tests/fixtures/typecheck/splice_field_access_string_lit_ok.scoop`
  - `cargo run -p scoop --no-default-features -- --effect-pipeline refactor dump-hir tests/fixtures/comptime/splice_field_access_v0_basic.scoop`
  - 新增 unresolved splice negative fixture。

- 完成条件：
  - `ExprKind::Todo("splice_field")` 不再可达 refactor HIR。
  - 可进入 `HIR-T06`。
- 阻塞记录（2026-05-06）：
  - 指定验证 `cargo run -p scoop --no-default-features -- --effect-pipeline refactor dump-hir tests/fixtures/comptime/splice_field_access_v0_basic.scoop` 当前先失败于 `scoop::typecheck::missing_type_annotation`（顶层 `P`）；即使补齐注解，同类本地 `struct` splice fixture 也会被 refactor HIR no-Todo verifier 以 `Item::Todo(type)` 拒绝。
  - 该失败属于 `HIR-T05` declaration graph prerequisite，不能通过削弱 splice fixture、跳过 verifier 或保留 `splice_field` placeholder 绕过。
- 完成记录（2026-05-06）：
  - `ast::File` 新增 splice field contract side table；typecheck 对字符串字面量与 `FieldMeta { name: "..." }` 发布 receiver type、owner FQN、field FQN、field type 与 mutability contract。
  - typecheck 现在拒绝无法静态解析的 `value.[field]`，新增 `scoop::typecheck::splice_field_name_not_static` 诊断，错误定位到 field 表达式并说明需要编译期已知字段名。
  - 语句 typecheck 跟踪 `comptime for` binder；HIR lowering 可消费 runtime comptime expansion 的 FieldMeta/string 常量值，把 reflection loop 中的 `p.[field]` 展开为普通 resolved `MemberAccess`。
  - HIR lowering 不再构造 `ExprKind::Todo("splice_field")`，placeholder inventory 已移除该构造点；refactor HIR stable dump 中 splice field 现在显示为普通 `MemberAccess`。
  - 为指定 comptime dump fixture 补齐既有 typecheck 规则要求的顶层 `const val P: Point` 注解；未改变 splice field 覆盖形状。
  - 新增/更新覆盖：静态 string literal、FieldMeta descriptor、reflection loop FieldMeta、non-static field negative、unknown field negative。
  - 已运行：`cargo test -p scoopc --no-default-features refactor_hir_splice_field`、`cargo test -p scoopc --no-default-features refactor_hir_placeholder_inventory`、`cargo test -p scoopc --no-default-features refactor_hir_no_todo`、`cargo run -p scoop --no-default-features -- --effect-pipeline refactor test --fixtures tests/fixtures/typecheck/splice_field_access_string_lit_ok.scoop`、`cargo run -p scoop --no-default-features -- --effect-pipeline refactor test --fixtures tests/fixtures/typecheck/splice_field_non_static_name_is_error.scoop`、`cargo run -p scoop --no-default-features -- --effect-pipeline refactor test --fixtures tests/fixtures/typecheck/splice_field_unknown_field_is_error.scoop`、`cargo run -p scoop --no-default-features -- --effect-pipeline refactor dump-hir tests/fixtures/comptime/splice_field_access_v0_basic.scoop`、`cargo test -p scoop --no-default-features dump_hir`、`cargo clippy -p scoopc -p scoop --no-default-features --all-targets -- -D warnings`。
- 依赖：`HIR-T03`, `HIR-T05`

## [DONE] HIR-T06：收口 array literal、named/default/spread args 与 call arg canonicalization

- 参考：
  - [`PLAN.md`](./PLAN.md) §2/H5
  - [`PIPELINE_GAPS.md`](./PIPELINE_GAPS.md) §3.9, §3.10
- 目标：
  - HIR call sites 输出完整 ordered args，不让后端补 named/default/spread。
  - array literal 无法推断时提前诊断。

- 必须实现的内容：
  1. 空 array literal 无 expected element type 时，typecheck/HIR stage 报错，不生成 `array_lit` Todo。
  2. 非空 array literal 统一记录 target kind、element type、result type。
  3. named args 在 typecheck 后写回 ordered arg mapping。
  4. default args 在 HIR 中补齐为显式 expression 或 default thunk invocation contract。
  5. spread args 仅在 vararg context 展开或构造 vararg array；非 vararg context 诊断。
  6. class ctor、member、extension、generic function call 全部使用同一 arg binding contract。
  7. 更新 HIR dump 显示 canonical arg order 与 default/spread 来源。

- 必须遵从的约束：
  - 不让 LLVM/raw MIR codegen 成为 named/default args 的唯一补齐点。
  - 不把 `NamedArg` / `SpreadArg` raw syntax 作为普通 HIR expression 输出。

- 验证：
  - `cargo test -p scoopc --no-default-features refactor_hir_call_args`
  - fixtures 覆盖 empty array ok/error、function named/default、ctor named/default、member/extension/generic default、vararg/spread ok/error。

- 完成条件：
  - `array_lit`、`named_arg`、`spread_arg` placeholder 不再可达 refactor HIR。
  - 可进入 `HIR-T07`。
- 完成记录（2026-05-06）：
  - `ast::File` 新增 typechecked call-arg binding side table，typecheck 现在为 direct function、member、extension 与 ctor 调用发布 ordered argument contract，覆盖 receiver、explicit、default 与 vararg/spread slots。
  - refactor HIR lowering 消费该 contract：命名实参按形参顺序输出为 positional HIR args，默认参数以显式 block/局部绑定补齐，vararg/spread 构造为显式 array slot，effect-op args 也按 typechecked mapping 输出为 positional payload。
  - array literal fallback 不再生成 `ExprKind::Todo("array_lit")`；typed empty/non-empty arrays 继续降为 builder block，call 外 named/spread 语法壳在 HIR 中剥离而不生成 placeholder。
  - placeholder inventory 已移除 `array_lit`、`named_arg`、`spread_arg` reason；新增 `refactor_hir_call_args` 单测覆盖 generic default、class ctor/member/extension default、array literal 与 vararg spread canonicalization。
  - 新增 HIR fixture `tests/fixtures/hir/refactor_call_args.scoop/.hir`，覆盖 generic named/default、typed empty/non-empty array literal 与 vararg spread 的 canonical HIR dump。
  - 已运行：`cargo test -p scoopc --no-default-features refactor_hir_call_args`、`cargo test -p scoopc --no-default-features refactor_hir_placeholder_inventory`、`cargo test -p scoopc --no-default-features refactor_hir_no_todo`、`cargo run -p scoop --no-default-features -- --effect-pipeline refactor test --fixtures tests/fixtures/hir/refactor_call_args.scoop`、相关 array/named/default/vararg typecheck fixtures、`cargo test -p scoop --no-default-features dump_hir`、`cargo clippy -p scoopc -p scoop --no-default-features --all-targets -- -D warnings`。
- 依赖：`HIR-T04`

## [DONE] HIR-T07：发布 callable callee provenance 与 dispatch/ctor/intrinsic HIR contract

- 参考：
  - [`PLAN.md`](./PLAN.md) §2/H5
  - [`PIPELINE_GAPS.md`](./PIPELINE_GAPS.md) §1.7-§1.11, §3.7, §3.10
- 目标：
  - MIR lowering 不再从 callee 表达式形状、字符串 FQN 或成员名猜 call kind。

- 必须实现的内容：
  1. 在 typed HIR side table 中为每个 call site 发布 `CallSiteContract`。
  2. contract 至少覆盖 direct top-level、member direct、extension、constructor、closure、fun value、fun ptr、virtual、interface、intrinsic、effect op、continuation resume。
  3. dispatch contract 使用结构化 owner/member binding，不使用 `rsplit_once('.')` 作为语义来源。
  4. ctor contract 包含 selected ctor、owner type、complete ordered args、default/named mapping。
  5. continuation resume、perform、handle contract 继续以 stable site id/source key 发布，缺失即 HIR stage error。
  6. GC/reflection/platform intrinsics 若 HIR 接受，必须标明 intrinsic kind 与 args contract。

- 必须遵从的约束：
  - refactor MIR stage 不得再依赖 legacy span/name guess 作为 authoritative source。
  - 不在 HIR 阶段引入 StepSchema 或 late-lowered ABI。

- 验证：
  - `cargo test -p scoopc --no-default-features refactor_hir_call_contracts`
  - `dump-hir --effect-pipeline refactor` fixtures 覆盖 direct/fun-value/closure/virtual/interface/ctor/resume/perform/handle/intrinsic call。

- 完成条件：
  - 所有 call-like HIR site 都有下游可消费 provenance。
  - 可进入 `HIR-T08`。
- 完成记录（2026-05-06）：
  - `TypedHirEffectContracts` 新增结构化 `call_site_contracts`，覆盖 direct top-level、member direct、extension、constructor、closure、fun value、FunPtr、virtual/interface dispatch、intrinsic、effect-op 与 `Continuation.resume`；stable dump 现在展示各调用点 kind、target、receiver/owner/member、ctor arg mapping、payload/typed effect contract 等关键字段。
  - HIR lowering 产物新增 canonical call-arg binding side table；typed HIR call contract 可消费 typecheck 写回的 receiver/default/vararg 参数槽信息，并避免把 named/default/spread 语义留给后端猜测。
  - refactor MIR dispatch lowering 现在从 typed HIR call contract 读取 owner/member/receiver dispatch metadata，不再在 refactor path 对 callee FQN 做 `rsplit_once('.')` 作为 authoritative dispatch 来源；legacy fallback 保持原行为。
  - 新增 `refactor_hir_call_contracts_record_callable_provenance` 单测与 `tests/fixtures/typecheck/refactor_hir_call_contracts_surface_ok.scoop`，覆盖 direct、member、extension、ctor、closure、fun value、FunPtr、virtual、interface、resume、perform/handle 与 reflection intrinsic。
  - 已运行：`cargo test -p scoopc --no-default-features refactor_hir_call_contracts`、`cargo test -p scoopc --no-default-features refactor_typed_hir`、`cargo test -p scoopc --no-default-features refactor_hir_no_todo`、`cargo test -p scoopc --no-default-features dispatch_and_resume`、`cargo test -p scoop --no-default-features dump_hir`、`cargo run -p scoop --no-default-features -- --effect-pipeline refactor test --fixtures tests/fixtures/typecheck/refactor_hir_call_contracts_surface_ok.scoop`、`cargo run -p scoop --no-default-features -- --effect-pipeline refactor dump-hir tests/fixtures/typecheck/refactor_hir_call_contracts_surface_ok.scoop`、`cargo clippy -p scoopc -p scoop --no-default-features --all-targets -- -D warnings`。
- 依赖：`HIR-T06`

## [DONE] HIR-T08：收口 class literal 与 reflection/platform intrinsic HIR contract

- 参考：
  - [`PLAN.md`](./PLAN.md) §2/H6
  - [`PIPELINE_GAPS.md`](./PIPELINE_GAPS.md) §1.3, §6.3
- 目标：
  - class literal 不再是 HIR Todo；runtime reflection fallback 有明确 HIR contract 或明确诊断。

- 必须实现的内容：
  1. 定义 HIR `ClassLiteral` / `TypeMetadataLiteral` 或等价 value primitive。
  2. annotation/comptime context 按 v0 规则折叠为类型名字符串或 metadata constant。
  3. runtime context 若允许，输出 source type、metadata kind、result type。
  4. runtime context 若不允许，typecheck/HIR stage 诊断，不生成 Todo。
  5. `nameOf<T>()`、`sizeOf<T>()`、`getPlatform()` 的 HIR intrinsic contract 必须明确 allowed context 与 fallback behavior。
  6. HIR dump 显示 class literal/intrinsic contract。

- 必须遵从的约束：
  - 不以 LLVM lowering 尚未实现为理由删除 HIR contract。
  - 不让 annotation-only 消费路径绕过 refactor HIR verifier。

- 验证：
  - `cargo test -p scoopc --no-default-features refactor_hir_class_literal`
  - `cargo run -p scoop --no-default-features -- --effect-pipeline refactor test --fixtures tests/fixtures/typecheck/annotation_args_const_expr_array_enum_classlit_ok.scoop`
  - `cargo run -p scoop --no-default-features -- --effect-pipeline refactor test --fixtures tests/fixtures/typecheck/reflection_runtime_fallback_v0.scoop`
  - `cargo run -p scoop --no-default-features -- --effect-pipeline refactor test --fixtures tests/fixtures/typecheck/get_platform_runtime_ok.scoop`

- 完成条件：
  - `ExprKind::Todo("class_lit")` 不再可达 refactor HIR。
  - 可进入 `HIR-T09`。
- 完成记录（2026-05-06）：
  - HIR 新增 `ClassLiteral` / `ClassLiteralExpr` value primitive，v0 runtime value 明确为类型名字符串，同时保留 source type、source FQN、metadata kind 与 result type，refactor HIR dump 可直接展示该 contract。
  - 普通表达式 typecheck 现在接受 `Type::class` 并验证类型引用；HIR lowering 不再生成 `ExprKind::Todo("class_lit")`，placeholder inventory 与 no-Todo verifier 期望已同步移除该 reason。
  - typed HIR intrinsic contract 现在为 `nameOf<T>()`、`sizeOf<T>()`、`getPlatform()` 显示 `intrinsic_allowed_context` 与 `intrinsic_runtime_fallback`，区分 reflection normal runtime call 与 platform query fallback。
  - 新增 `refactor_hir_class_literal_and_intrinsic_contracts` 单测覆盖 runtime class literal HIR contract，以及 `nameOf<T>()`、`sizeOf<T>()`、`getPlatform()` 的 intrinsic contract dump。
  - 已运行：`cargo test -p scoopc --no-default-features refactor_hir_class_literal`、`cargo test -p scoopc --no-default-features refactor_hir_placeholder_inventory`、`cargo test -p scoopc --no-default-features refactor_hir_no_todo`、`cargo test -p scoopc --no-default-features refactor_hir_call_contracts`、`cargo test -p scoop --no-default-features dump_hir`、三个指定 typecheck fixtures、`cargo run -p scoop --no-default-features -- --effect-pipeline refactor dump-hir tests/fixtures/typecheck/reflection_runtime_fallback_v0.scoop`、`cargo clippy -p scoopc -p scoop --no-default-features --all-targets -- -D warnings`。
- 依赖：`HIR-T07`

## [DONE] HIR-T09：收口 `with` copy-update aggregate metadata

- 参考：
  - [`PLAN.md`](./PLAN.md) §2/H7
  - [`PIPELINE_GAPS.md`](./PIPELINE_GAPS.md) §1.12
- 目标：
  - copy-update 在 HIR 中有完整 aggregate/update contract，缺 metadata 不再 fallback Todo。

- 必须实现的内容：
  1. typecheck 发布 `WithUpdateContract`，包含 base type、aggregate kind、field/variant path、result type。
  2. HIR lowering 强制消费该 contract。
  3. struct、tuple、enum copy-update 都 lower 成稳定 HIR representation。
  4. nested update path 明确 evaluation order 与 temporary/result binding。
  5. unsupported aggregate kind 诊断。
  6. HIR verifier 禁止 `with_update` reason。

- 必须遵从的约束：
  - 不允许缺 map 时降成 Any/Todo 再让 MIR/codegen 爆炸。
  - 不为某个 aggregate shape 新开 codegen-only 特判绕过 HIR contract。

- 验证：
  - `cargo test -p scoopc --no-default-features refactor_hir_with_update`
  - 定向 fixtures 覆盖 `with_update_*` struct/tuple/enum/nested path 与 unsupported aggregate error。

- 完成条件：
  - `ExprKind::Todo("with_update")` 不再可达 refactor HIR。
  - 可进入 `HIR-T10`。
- 完成记录（2026-05-06）：
  - `ast::File` 新增 typechecked `WithUpdateContract` side table，typecheck 现在为每个 `base with { ... }` 发布 base/result 类型、路径 aggregate 形状、struct/tuple/enum field/variant path 与 update value/target 类型。
  - refactor HIR lowering 改为强制消费该 contract：base 先绑定到 `$with_base`，update value 按源码顺序绑定到 `__with_update_value*` 临时，再重建 struct/tuple/enum；缺失或不一致的 metadata 走 HIR verifier 可定位 failure，不再生成 `ExprKind::Todo("with_update")`。
  - typed HIR stable dump 新增 `with_update_contracts` section，展示 copy-update aggregate/update contract；placeholder inventory 已移除 `with_update` 构造点。
  - 新增 `refactor_hir_with_update_publishes_copy_update_contracts` 单测覆盖 struct、tuple、enum 与 nested path contract；新增 negative fixture `with_update_base_not_supported_is_error.scoop` 覆盖 unsupported aggregate base 诊断；同步 `with_update_expr` AST golden。
  - 已运行：`cargo test -p scoopc --no-default-features with_update`、`cargo test -p scoopc --no-default-features refactor_hir_placeholder_inventory`、`cargo test -p scoopc --no-default-features refactor_hir_no_todo`、`cargo test -p scoopc --no-default-features refactor_typed_hir`、`cargo run -p scoop --no-default-features -- --effect-pipeline refactor test --fixtures tests/fixtures/typecheck`、`cargo run -p scoop --no-default-features -- --effect-pipeline refactor test --fixtures tests/fixtures/parse/with_update_expr.scoop`、`cargo test -p scoop --no-default-features dump_hir`、`cargo clippy -p scoopc -p scoop --no-default-features --all-targets -- -D warnings`。
- 依赖：`HIR-T08`

## [DONE] HIR-T10：建立 assignment LHS / HIR place contract

- 参考：
  - [`PLAN.md`](./PLAN.md) §2/H7
  - [`PIPELINE_GAPS.md`](./PIPELINE_GAPS.md) §1.6
- 目标：
  - 所有 refactor HIR assignment statement 都携带 typed place，不让 MIR 再产生 `assign lhs lowering pending`。

- 必须实现的内容：
  1. 定义 HIR `Place` 或 `AssignPlaceContract` side table。
  2. 覆盖当前 typecheck 接受的 LHS：local var、top-level var、member field、index/property setter、safe-member setter。
  3. 每个 place contract 包含 owner/binding、value type、mutability、write barrier/unsafe requirement 所需信息。
  4. 未支持 place shape 由 typecheck/HIR stage 诊断。
  5. assignment expression 在 `HIR-T02` 已 parser 拒绝；此任务只处理 statement assignment。
  6. HIR dump 显示 place kind。

- 必须遵从的约束：
  - 不让 MIR 从 arbitrary expression tree 恢复 lvalue。
  - 不把 unsupported setter/property shape 降成普通 member store。

- 验证：
  - `cargo test -p scoopc --no-default-features refactor_hir_places`
  - fixtures 覆盖 local/global/field/index/property/safe-member assignment 与 unsupported LHS error。

- 完成条件：
  - refactor MIR lowering 可只消费 place contract 生成 store。
  - 可进入 `HIR-T11`。
- 完成记录（2026-05-06）：
  - `ast::File` 新增 assignment place side table；typecheck 现在为通过前端校验的 assignment statement 发布 local var、top-level `@ThreadLocal/@Global var` 与 mutable member field 的 typed place contract，包含 binding、place/value type、mutability、storage-slot write-barrier requirement 与 unsafe requirement 位。
  - HIR lowering 将 typecheck contract 转换为 HIR-level `AssignPlaceContract`，保留 HIR `SymbolId`、top-level/member identity、receiver type 与 member binding；对 lowering 产生的合成 assignment 也补齐同类 contract，避免 for/desugar/debug fallback 形成无 contract HIR assignment。
  - refactor HIR completeness verifier 现在要求每个 `StmtKind::Assign` 都有 place contract；typed HIR stable dump 新增 `assign_place_contracts` section，并同步 typed HIR snapshot。
  - refactor MIR lowering 在 typed handoff 模式下消费 assignment place contract 生成 local store、`StoreTopLevelVar` 与 `StoreMember`，不再把 refactor assignment place 语义建立在 arbitrary LHS expression tree 猜测上；legacy MIR fallback 保持原行为。
  - 当前 parser/typecheck 尚不接受 index assignment 或 safe-member assignment 作为成功 LHS；这些 unsupported LHS 仍在进入 HIR 前诊断，未作为可达 refactor HIR place contract 形态处理。
  - 已移除 HIR `ExprKind::Todo("assign")` 构造点，并同步 placeholder inventory。
  - 已运行：`cargo test -p scoopc --no-default-features refactor_hir_places`、`cargo test -p scoopc --no-default-features refactor_hir_no_todo`、`cargo test -p scoopc --no-default-features refactor_hir_placeholder_inventory`、`cargo test -p scoopc --no-default-features refactor_typed_hir`、`cargo test -p scoop --no-default-features dump_hir`、`cargo clippy -p scoopc -p scoop --no-default-features --all-targets -- -D warnings`。
- 依赖：`HIR-T09`

## [DONE] HIR-T11：收口 custom iterator for-loop 与 remaining debug fallbacks

- 参考：
  - [`PLAN.md`](./PLAN.md) §2/H2
- 目标：
  - 清理 `missing_stmt`、`for_custom_iterator` 等非 spec 语义 placeholder。

- 必须实现的内容：
  1. parser 不应让 `ast::StmtKind::Missing` 进入成功 parse；若 recovery 产生 missing，refactor HIR stage 诊断。
  2. custom iterator for-loop 必须强制依赖 typecheck 写回的 iterator/next contract。
  3. contract 缺失时 HIR stage error，不能降成 `for_custom_iterator` Todo。
  4. 若 custom iterator 当前 spec/typecheck 已支持，则 HIR lowering 必须展开为 while/when 或专用 HIR loop primitive。
  5. 清查其它 debug fallback reason，逐一改为实现或诊断。

- 必须遵从的约束：
  - dump-only recovery 可以留在 legacy 路径，但 refactor production handoff 不能接受。

- 验证：
  - `cargo test -p scoopc --no-default-features refactor_hir_for_loop`
  - custom iterator ok/error fixtures。
  - parser recovery negative fixture 确认 refactor HIR 不产出 missing stmt。

- 完成条件：
  - `missing_stmt`、`for_custom_iterator` 等 debug fallback 不再可达 refactor HIR。
  - 可进入 `HIR-T12`。
- 完成记录（2026-05-06）：
  - parser 未知语句 fallback 现在返回 parse diagnostic；`ast::StmtKind::Missing` 仅保留为错误恢复 sentinel，不再能出现在成功 parse 结果中并进入 refactor HIR。
  - HIR lowering 对意外 `StmtKind::Missing`、缺失 `resolved_for_info` 或缺失 custom iterator metadata 记录结构化 `HirStageError`，不再构造 `StmtKind::Todo("missing_stmt")` / `StmtKind::Todo("for_custom_iterator")`。
  - custom iterator for-loop 继续消费 typecheck 写回的 iterator/next contract 并展开为显式 `while` + `when` HIR；合成 `iterator()` / `next()` 调用改用不同 synthetic span，typed call contract 可分别记录 interface dispatch provenance。
  - placeholder inventory 已移除 `missing_stmt` 与 `for_custom_iterator` 构造点；新增 parser recovery negative fixture `parser_recovery_missing_stmt_is_error.scoop` 与 `refactor_hir_for_loop` 定向单测覆盖 custom iterator 成功 lowering 和缺 contract stage error。
  - 已运行：`cargo test -p scoopc --no-default-features refactor_hir_for_loop`、`cargo test -p scoopc --no-default-features refactor_hir_placeholder_inventory`、`cargo test -p scoopc --no-default-features refactor_hir_no_todo`、`cargo run -p scoop --no-default-features -- --effect-pipeline refactor test --fixtures tests/fixtures/parse`、custom iterator ok/error typecheck fixtures、`cargo test -p scoop --no-default-features dump_hir`、`cargo run -p scoop --no-default-features -- --effect-pipeline refactor dump-hir tests/fixtures/typecheck/for_loop_iter_protocol_ok.scoop`、`cargo clippy -p scoopc -p scoop --no-default-features --all-targets -- -D warnings`。
- 依赖：`HIR-T10`

## [DONE] HIR-T12：建立 top-level init/storage/object metadata handoff

- 参考：
  - [`PLAN.md`](./PLAN.md) §2/H8
  - [`PIPELINE_GAPS.md`](./PIPELINE_GAPS.md) §1.4, §6.4
- 目标：
  - HIR program handoff 足以让下一阶段构建 top-level MIR roots，不再生成 top-level item Todo。

- 必须实现的内容：
  1. 为 top-level `val` / `var` 发布 initializer body contract。
  2. 区分 const value、runtime immutable value、runtime mutable global。
  3. object singleton initializer 进入 HIR init root graph。
  4. type metadata / alias metadata 若需要 runtime init，进入 graph。
  5. `@Extern` global variable contract 包含 external symbol name、linkage kind、TLS/global、initializer absence、unsafe access requirement。
  6. 发布 dependency ordering 所需 facts。
  7. 更新 HIR dump 显示 init/storage roots。

- 必须遵从的约束：
  - 不要求本任务实现 LLVM external global lowering。
  - 不允许下一阶段只能回读 AST/HIR expr 私有字段补 init 语义。

- 验证：
  - `cargo test -p scoopc --no-default-features refactor_hir_top_level_init`
  - fixtures 覆盖 top-level val/var、const val、object init、extern global。

- 完成条件：
  - HIR handoff 已完整发布 top-level init/storage/object metadata。
  - 可进入 `HIR-T13`。
- 完成记录（2026-05-06）：
  - `LoweredHir` 新增 `extern_globals` side table，HIR lowering 现在为 `@Extern` 顶层 `val/var` 发布外部符号名、external linkage、TLS/global storage、initializer absence 与 unsafe access requirement；`@Extern` 顶层值不再被误归入 runtime top-level init root。
  - refactor typed HIR contract 新增 `top_level_init_roots` 与 `extern_global_contracts`，覆盖 `const val`、runtime immutable `val`、runtime mutable `@Global/@ThreadLocal var`、当前 lowering 文件中的 object singleton init root，以及每个 root 的直接 dependency ordering facts。
  - `dump-hir --effect-pipeline refactor` 的 typed contract section 现在显示 init/storage roots 与 extern global contract；新增 `tests/fixtures/hir/refactor_top_level_init.scoop/.hir` 覆盖 top-level val/var、const val、object init 与 extern global。
  - 已运行：`cargo test -p scoopc --no-default-features refactor_hir_top_level_init`、`cargo run -p scoop --no-default-features -- --effect-pipeline refactor test --fixtures tests/fixtures/hir/refactor_top_level_init.scoop`、`cargo run -q -p scoop --no-default-features -- --effect-pipeline refactor dump-hir tests/fixtures/hir/refactor_top_level_init.scoop`、`cargo test -p scoopc --no-default-features refactor_hir_no_todo`、`cargo test -p scoopc --no-default-features refactor_typed_hir`、`cargo test -p scoop --no-default-features dump_hir`、`cargo clippy -p scoopc -p scoop --no-default-features --all-targets -- -D warnings`。
- 依赖：`HIR-T11`

## HIR-T13：建立 HIR -> next-stage preflight，阻止 HIR gap 流入 MIR

- 参考：
  - [`PLAN.md`](./PLAN.md) §2/H9
  - [`PIPELINE_GAPS.md`](./PIPELINE_GAPS.md) §2.1-§2.3
- 目标：
  - 在不要求 full fixtures 和 LLVM 的前提下，证明 HIR gap 不再流向下一阶段。

- 必须实现的内容：
  1. 新增 `refactor_hir_preflight` 测试入口或 internal API。
  2. preflight 对所有 HIR completeness fixtures 执行 refactor typed HIR stage + no-Todo verifier。
  3. preflight 检查 side tables 覆盖 call/resume/perform/handle/place/copy-update/top-level init roots。
  4. 对少量代表性样本运行 refactor direct-style MIR stage，只检查没有 HIR-origin Todo 或 missing contract。
  5. preflight 不能把后续已知 LLVM/codegen failure 计入本阶段 blocker。
  6. 若 MIR stage 因后续非 HIR gap 失败，测试应明确标注 skip/known-later-stage，而不是降低 HIR gate。

- 必须遵从的约束：
  - 不运行 `cargo run -p scoop -- test` 全量矩阵。
  - 不用 legacy path 证明 refactor HIR 完整。

- 验证：
  - `cargo test -p scoopc --no-default-features refactor_hir_preflight`
  - `cargo run -p scoop --no-default-features -- --effect-pipeline refactor dump-hir <HIR completeness fixture set>`
  - 少量 `dump-mir --effect-pipeline refactor` smoke。

- 完成条件：
  - preflight 能阻止 HIR-origin Todo / missing contract 进入下一阶段。
  - 可进入 `HIR-T14`。
- 依赖：`HIR-T12`

## HIR-T14：冻结 HIR completeness 验证矩阵与阶段完成记录

- 参考：
  - [`PLAN.md`](./PLAN.md) §4
- 目标：
  - 固化本阶段完成标准和后续阶段可依赖的 HIR handoff contract。

- 必须实现的内容：
  1. 在 `PLAN.md` 或专门 handoff 文档中记录最终 HIR invariants。
  2. 更新 `TODO.md` 每个任务完成记录，列出实际变更、测试和剩余非 HIR gap。
  3. 整理 HIR completeness fixture set，并说明为何不跑 full fixtures。
  4. 搜索确认 refactor HIR stage 可达路径中没有 `Todo(...)` reason。
  5. 明确后续 MIR/codegen gap 不属于 HIR stage blocker 的清单。

- 必须遵从的约束：
  - 不把“后续阶段仍失败”写成 HIR 阶段未完成，除非失败根因是 HIR contract 缺失。
  - 不删除 legacy path，也不要求 legacy no-Todo。

- 验证：
  - `cargo test -p scoopc --no-default-features refactor_hir_no_todo refactor_hir_preflight`
  - `cargo test -p scoop --no-default-features dump_hir`
  - `rg "Todo\(" crates/scoopc/src/hir crates/scoopc/src/effect_refactor_pipeline`，并确认命中要么 legacy-only、测试、verifier 禁用清单，要么不可达 refactor production handoff。

- 完成条件：
  - 可以明确宣布 HIR stage complete。
  - 后续阶段可以把 refactor typed HIR handoff 当作完整输入。
- 依赖：`HIR-T13`
