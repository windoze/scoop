# TODO-2：Base crates + cone compilation unit model

> 生成时间：2026-05-21
> 计划基线：[`PLAN.md`](./PLAN.md) §4/P1
> 设计基线：[`PIPELINE_REFACTOR.md`](./PIPELINE_REFACTOR.md)
> 审计基线：[`PIPELINE-CLEANUP.md`](./PIPELINE-CLEANUP.md)
> 索引：[`TODO.md`](./TODO.md)
> 顺序约束：严格按本文件任务顺序推进；每个实现任务后必须执行紧随其后的 review 任务。
> 本包目标：建立 stage/fact crate 可共同依赖的基础 crate 层，并把 source cone graph 与 cone-level compilation unit 语义固定为 facade 和后续阶段的输入模型。

## 全局约束

- 本包只处理 `PLAN.md` 的 P1：基础 crate 壳层、`ProjectInput` / `ProjectContext` / `SourceConeGraph` 边界，以及 cone = compilation unit API；不提前拆 HIR/MIR/effect/LIR facts 或 backend 输入。
- 基础 crate 不得依赖任何 stage crate、fact crate 或 `scoopc` facade；允许的方向是 `scoopc` facade 依赖基础 crate，并在迁移期 re-export 旧模块路径。
- 基础 crate 之间必须保持无环依赖。目标依赖方向优先为：`scoopc_source` / `scoopc_types` / `scoopc_ids` 可依赖 `scoopc_span`，`scoopc_project_model` 可依赖 `scoopc_source` / `scoopc_ids` / 必要的 `scoopc_types`；若实现中必须调整该方向，需在当前任务完成记录中说明原因。
- 迁移期可以保留 `scoopc::{span, source, ty, stable_id, cone, frontend}` 的 re-export / adapter，但不得出现两套独立定义或两套 authoritative 查询面。
- “文件”不能被重新命名或伪装成正式 compilation unit；虚拟单文件输入只能建模为 synthetic consumer cone 中的单文件特例。
- cone 内文件顺序只允许作为稳定遍历、诊断顺序和 dump 稳定性输入；不得让任何任务把文件顺序升级为语义依赖。
- 每个任务完成后，在该任务的“完成记录”下写明改动范围、核心决策、验证命令和残余风险。

## 触碰面基线

本节是 `TODO-2-INIT` 的仓库搜索记录，后续 P1 任务应优先从这些位置开始，不再重复做开放式搜索。

### Workspace 与 facade

- `Cargo.toml` 当前 workspace 只有 `crates/scoop`、`crates/scoopc`、`crates/scoop_runtime`、`tools/scoop_tools`；P1 新基础 crate 需要显式加入 workspace。
- `crates/scoopc/Cargo.toml` 当前把 `miette`、`thiserror`、`tracing`、`toml`、`serde`、`sha2`、LLVM optional deps 都集中在 `scoopc`；迁移时要把基础 crate 所需依赖下放到对应 crate，避免基础 crate 反向依赖 `scoopc`。
- `crates/scoopc/src/lib.rs` 当前公开 `ast`、`cone`、`frontend`、`source`、`span`、`stable_id`、`ty` 等模块；迁移期 re-export 应从这里收口。

### `span` / `source`

- `crates/scoopc/src/span.rs` 只定义 `Span` 和 `From<Span> for miette::SourceSpan`，是最小可先迁移模块。
- `crates/scoopc/src/source.rs` 定义 `SourceOrigin`、`SourceTrust`、`SourceFile`、`SourceId`、`SourceMapSpan`、`SourceLocation`、`SourceMap`；依赖 `Span`、`miette`、路径与文件读取。
- 主要调用面包括 parser/syntax token、resolve/typecheck 诊断、session/sysroot 加载、frontend source map、cone graph source 加载、HIR/MIR/LIR/codegen tests 与 LLVM source-map helper。

### `types` / `ids` / `stable_id`

- `crates/scoopc/src/ty/mod.rs` 与 `crates/scoopc/src/ty/layout.rs` 定义 `TypeId`、`TypeStore`、`TypeKind`、`EffectRow`、builtin type universe 和 layout 查询；`TypeParamType` 当前直接携带 `crate::span::Span`。
- `TypeId` / `TypeStore` / `EffectRow` 被 typecheck、HIR stage、MIR lowering/materialization、effect facts、LIR、RTTI 和 LLVM codegen 广泛消费；迁移必须保持单一 type universe，不能引入等价 `TypeStore` 桥。
- `crates/scoopc/src/stable_id.rs` 定义 `StableHashScope`、`StableConeKey`、`StableDefKey`、`StableTemplateKey`、`StableInstanceKey`、canonical type/effect encoding 和 symbol mangler；当前依赖 `ConeManifest`、`Span` 和 `ty`，因此完整迁移要等 project model 与 types 边界稳定。
- `SiteId` 当前在 `crates/scoopc/src/mir/mod.rs`，`TemplateKey` / `InstanceKey` 当前在 `crates/scoopc/src/mir/materialize/mod.rs`；`TemplateKey` / `InstanceKey` 是 MIR materialization 内部键，不应在 P1 被误升级为跨阶段基础 ID。

### `cone` / `project_model`

- `crates/scoopc/src/cone/manifest.rs` 定义 `ConeKind`、`ConeSection`、`ConeDependencySpec`、`ConeManifest`、`ConeNativeBuildConfig` 等 project-level manifest 数据；当前 `ConeNativeBuildConfig` 依赖 `crate::opt::OptLevel`。
- `crates/scoopc/src/cone/package.rs` 负责 source package 文件系统加载、platform selector 应用和 main anchor 发现；这属于 project loading adapter，不应被后续 stage/fact crate 直接依赖。
- `crates/scoopc/src/cone/graph.rs` 定义 `SourceConeGraph`、`SourceConeNode`、`SourceConeInfo`、dependency edge、trust/role/kind，并在 `SourceConeGraph::from_nodes(...)` 中执行 dependency 校验和拓扑排序。
- `ConeId` / `ConeInfo` 当前定义在 `crates/scoopc/src/resolve/mod.rs`，但它们表达 project/cone membership，不应长期归属于 resolver stage。

### `frontend` / compilation unit API

- `crates/scoopc/src/frontend.rs` 定义 `ProjectInput`、`ProjectContext`、`FrontendOutput`、`run_project_frontend(...)`、`run_frontend(...)`、`build_source_map(...)`。
- `ProjectInput` 当前持有 `SourceConeGraph`、flatten 后的 `sources`、`source_cone_ids`、`source_cone_infos`、consumer cone、main anchor 和 manifest；这是 build closure 输入，不应被解释为单一 compilation unit。
- `run_frontend(...)` 当前把 sysroot/local dependency/consumer sources 一起 parse、index、resolve、typecheck；P1 不要求完成真正逐 cone 编译，但必须让 API 命名和数据模型明确区分 build graph、cone unit 和文件。
- `crates/scoopc/src/pipeline/ast_stage.rs` 当前 `AstStageOutput<'a>` 仍是单文件 handoff；P1 需要建立 cone-level AST/compilation-unit handoff，或至少提供 facade 可直接使用的 cone-level wrapper。
- HIR/MIR/codegen 入口当前会消费 `StableConeKey`、`source_cones`、`request_source_paths` 等 project metadata（例如 `frontend.rs` 的 HIR lowering入口、`hir/lower/main/compilation_unit.rs`、`mir/mod.rs`、`llvm/codegen/mod.rs`）；P1 需要把这些 metadata 的来源改为 base/project model，而不是阶段私有 side table。

## [DONE] TODO-2-INIT：初始化并细化本任务包

- 目标：
  - 分析 `PLAN.md` §4/P1、`PIPELINE_REFACTOR.md` 和当前代码中 `span` / `source` / `ty` / `stable_id` / `cone` / `frontend` 的真实依赖关系；
  - 生成本任务包的详细任务列表，覆盖基础 crate 壳层、迁移顺序、re-export 策略、cone compilation unit API 和验证门禁；
  - 更新 `TODO.md` 的具体任务索引，用新生成的任务替换或扩展 `TODO-2-INIT` 所在索引行。
- 必须实现的内容：
  1. 搜索并记录本包会触碰的主要类型、模块和调用点，避免后续任务重复做开放式仓库搜索。
  2. 把 P1 拆成数量适中的实现小阶段，每个阶段必须有明确目标、修改范围、验证命令和完成条件。
  3. 在每个实现小阶段后插入独立 review 任务，review 任务必须复审前一阶段是否满足 P1 约束。
  4. 同步更新 `TODO.md` 中的具体任务索引，确保任务 ID、状态和顺序与本文件一致。
- 完成条件：
  - `TODO-2.md` 不再只是范围说明，而是包含可直接执行的详细任务列表；
  - `TODO.md` 的具体任务索引已经同步反映 `TODO-2.md` 的新任务和 `[TODO]` 状态；
  - 本任务完成记录说明任务拆分依据和未展开的风险。
- 依赖：P0-T04R
- 完成记录：
  - 拆分依据：P1 的依赖顺序由当前代码依赖决定，先迁移最小自包含的 `Span`，再迁移依赖 span 的 source/type 基础设施，随后收口 stable id / cone id / project model，最后改 facade 的 cone compilation unit API。
  - 触碰面记录：已在本文件“触碰面基线”中记录 workspace、`span`、`source`、`ty`、`stable_id`、`cone`、`frontend`、`AstStageOutput`、`SourceConeGraph`、`ProjectInput` 和主要下游调用点。
  - 任务结构：新增 6 个实现阶段和 6 个 review 阶段：基础 crate 壳层、span/source 迁移、types/ids 迁移、project model/cone graph 迁移、cone compilation unit facade、P1 全包清场。
  - 核心决策：允许 `scoopc` 在迁移期保留旧模块路径作为 re-export / adapter，但不允许基础 crate 反向依赖 `scoopc` 或产生重复 authoritative 类型；`TemplateKey` / `InstanceKey` 等 MIR materialization 内部键不在 P1 强行外提。
  - 未展开风险：`stable_id.rs` 的完整外提同时涉及 type canonical encoding 和 `ConeManifest`，因此必须等 types/project model 迁移后完成；production frontend 可能仍在过渡期一次处理整个 build closure，但 P1 任务要求 API 和命名明确区分 build graph 与 cone-level compilation unit，不能继续把 flatten 后的 `sources` 描述为一个大 compilation unit。
  - 验证命令：文档任务仅需检查 markdown/TODO 一致性；本次执行使用 `git diff --check`。

## [DONE] P1-T01：建立基础 crate 壳层与依赖门禁

- 参考：
  - `PLAN.md` §1.2、§4/P1
  - `PIPELINE_REFACTOR.md` “crate 划分”“依赖规则”
  - 本文件“触碰面基线 / Workspace 与 facade”
- 目标：
  - 在 workspace 中加入基础 crate 壳层：`scoopc_span`、`scoopc_source`、`scoopc_types`、`scoopc_ids`、`scoopc_project_model`；
  - 固定基础 crate 的依赖方向、crate-level 文档和 facade re-export 策略；
  - 建立可重复运行的依赖门禁，防止基础 crate 依赖 stage/fact crate 或 `scoopc`。
- 必须检查和修改的主要位置：
  - `Cargo.toml`
  - `crates/scoopc/Cargo.toml`
  - `crates/scoopc/src/lib.rs`
  - 新增 `crates/scoopc_span/`、`crates/scoopc_source/`、`crates/scoopc_types/`、`crates/scoopc_ids/`、`crates/scoopc_project_model/`
  - 如需自动化门禁，优先放在 `tools/scoop_tools/` 或 `crates/scoopc/tests/` 中，避免引入外部脚本依赖。
- 必须实现的内容：
  1. 新建 5 个基础 crate 的最小可编译壳层，写明每个 crate 的职责和禁止依赖。
  2. 将 `scoopc` 改为依赖这些基础 crate，但暂不迁移业务类型定义。
  3. 选择一种可维护的依赖门禁：测试、工具命令或文档化 `cargo tree` 检查；门禁必须能发现基础 crate 反向依赖 `scoopc`。
  4. 在 `scoopc` facade 中预留迁移期 re-export 位置，但不得复制当前类型定义。
  5. 更新 root `README.md` 或已有架构说明中的 workspace/crate 概览，使新基础 crate 角色可见。
- 禁止事项：
  - 禁止把 HIR/MIR/effect/LIR/codegen 类型放入基础 crate。
  - 禁止创建只为通过依赖检查而空转、且后续任务无法承接的 crate 名称。
  - 禁止让基础 crate 依赖 `scoopc` facade、stage crate 或 fact crate。
- 验证：
  1. `cargo fmt`
  2. `cargo check --workspace --no-default-features`
  3. 运行本任务新增的依赖门禁。
  4. `cargo clippy --all-targets -- -D warnings`
- 完成条件：
  - 5 个基础 crate 都能独立编译；
  - `scoopc` 可以依赖并 re-export 基础 crate 壳层；
  - 依赖门禁能明确证明基础 crate 未反向依赖 stage/fact/facade。
- 依赖：TODO-2-INIT
- 完成记录：
  - 改动范围：新增 workspace 基础 crate 壳层 `scoopc_span`、`scoopc_source`、`scoopc_types`、`scoopc_ids`、`scoopc_project_model`；`scoopc` 增加对这些 crate 的 path 依赖，并通过 `scoopc::base::{span, source, types, ids, project_model}` 预留迁移期 facade anchor；`README.md` 增加 workspace/crate 概览；`scoop_tools` 增加 `dependency-gate` 命令。
  - 核心决策：P1-T01 不迁移业务类型，现有 `scoopc::{span, source, ty, stable_id, cone}` 仍是 authoritative 定义，后续 P1 迁移任务再把定义物理移动到基础 crate；基础 crate 壳层暂不引入互相依赖，允许方向由依赖门禁编码并在后续迁移时逐步落地。
  - 依赖门禁：`cargo run -p scoop_tools -- dependency-gate` 对 5 个基础 crate 分别运行 `cargo tree`，拒绝依赖 `scoopc` facade、driver/runtime/tool、目标 stage/fact crate 名称，以及违反 P1 基础 crate 方向的 later-base 依赖；单元测试覆盖 parser、允许方向、反向依赖 `scoopc` 和 later-base 依赖检测。
  - 验证命令：`cargo fmt`；`cargo check --workspace --no-default-features`；`cargo run -p scoop_tools -- dependency-gate`；`cargo clippy --all-targets -- -D warnings`；`cargo test -p scoop_tools dependency_gate`。
  - 残余风险：基础 crate 当前仍是壳层，实际 `Span` / source / type / ID / project model authoritative 定义尚未迁移；这是后续 P1-T02 到 P1-T04 的范围，P1-T01 未引入重复定义或桥接转换。

## [DONE] P1-T01R：Review 基础 crate 壳层与依赖门禁

- 参考：P1-T01。
- 重点：
  - workspace 是否包含所有 P1 基础 crate；
  - 基础 crate 是否只有 crate-level 文档和必要壳层，没有误放 stage/fact 类型；
  - 依赖门禁是否能防止基础 crate 依赖 `scoopc` 或后续阶段。
- 必须复查的范围：
  - `Cargo.toml`
  - `crates/scoopc*/Cargo.toml`
  - `crates/scoopc*/src/lib.rs`
  - 新增依赖门禁实现
  - `README.md` 或架构说明更新
- 验证：
  - 重新运行 P1-T01 的所有验证；
  - 额外检查 `cargo tree -p scoopc_span`、`cargo tree -p scoopc_source`、`cargo tree -p scoopc_types`、`cargo tree -p scoopc_ids`、`cargo tree -p scoopc_project_model` 的依赖方向。
- 完成条件：
  - review 结论明确写出：基础 crate 壳层和依赖门禁满足 P1 crate DAG 约束，或列出阻塞项并在本 review 内修复。
- 依赖：P1-T01
- 完成记录：
  - 复查范围：确认 root workspace 已包含 `scoopc_span`、`scoopc_source`、`scoopc_types`、`scoopc_ids`、`scoopc_project_model`；复查 5 个基础 crate 的 `Cargo.toml` / `src/lib.rs`、`scoopc` facade anchor、`README.md` crate 概览和 `tools/scoop_tools` 的 `dependency-gate` 实现。
  - review 结论：P1-T01 的基础 crate 壳层满足当前 P1 crate DAG 约束；基础 crate 当前只包含 crate-level 职责文档和 `#![forbid(unsafe_code)]`，没有误放 HIR/MIR/effect/LIR/codegen 或 fact 类型；`scoopc::base::{span, source, types, ids, project_model}` 只是迁移期 facade anchor，没有复制 authoritative 定义。
  - 依赖门禁结论：`dependency-gate` 从每个基础 crate 的 `cargo tree` 视角拒绝依赖 `scoopc` facade、driver/runtime/tool、列出的 stage/fact/backend crate 名称，以及违反 P1 基础 crate 方向的 later-base 依赖；额外 `cargo tree -p ...` 检查显示 5 个基础 crate 当前均无依赖。
  - 验证命令：`cargo fmt`；`cargo check --workspace --no-default-features`；`cargo run -p scoop_tools -- dependency-gate`；`cargo clippy --all-targets -- -D warnings`；`cargo test -p scoop_tools dependency_gate`；`cargo tree -p scoopc_span`；`cargo tree -p scoopc_source`；`cargo tree -p scoopc_types`；`cargo tree -p scoopc_ids`；`cargo tree -p scoopc_project_model`。
  - 残余风险：基础 crate 仍是壳层，实际 `Span` / source / type / ID / project model authoritative 定义尚未迁移；这是后续 `P1-T02` 到 `P1-T04` 的既定范围，本 review 未发现阻塞 `P1-T02` 的依赖或命名问题。

## [DONE] P1-T02：迁移 `span` 与 `source` 基础设施

- 参考：
  - 本文件“触碰面基线 / `span` / `source`”
  - `PIPELINE_REFACTOR.md` “base context 包括什么”
- 目标：
  - 将 `Span` 迁入 `scoopc_span`；
  - 将 `SourceFile`、`SourceId`、`SourceMap` 等 source 基础设施迁入 `scoopc_source`；
  - 在 `scoopc` 中保留旧路径 re-export，使后续迁移可以逐步更新 imports。
- 必须检查和修改的主要位置：
  - `crates/scoopc/src/span.rs`
  - `crates/scoopc/src/source.rs`
  - `crates/scoopc/src/lib.rs`
  - `crates/scoopc_span/src/lib.rs`
  - `crates/scoopc_source/src/lib.rs`
  - parser/syntax/source-map/session/frontend/cone/sysroot/typecheck/HIR/MIR/LIR/LLVM 中直接引用 `crate::span`、`crate::source` 的位置。
- 必须实现的内容：
  1. 将 `Span` 的定义和测试迁入 `scoopc_span`，保留 `miette::SourceSpan` 转换能力。
  2. 将 source 文件、source map 和 source span 相关定义与测试迁入 `scoopc_source`，并改为依赖 `scoopc_span`。
  3. 在 `scoopc` 中把 `span` / `source` 模块改为 re-export adapter，不保留重复定义。
  4. 更新必要 imports，使全仓只存在一套 authoritative `Span` / `SourceFile` / `SourceId` 类型。
  5. 保持现有 diagnostics、source map global span 和 UTF-8 boundary 行为不变。
- 禁止事项：
  - 禁止同时保留 `scoopc::span::Span` 和 `scoopc_span::Span` 两套不同类型。
  - 禁止在 `scoopc_source` 中引入 parser、AST、resolve、typecheck 或 sysroot 依赖。
  - 禁止借迁移改变 source loading、path canonicalization 或 diagnostic span 语义。
- 验证：
  1. `cargo fmt`
  2. `cargo test -p scoopc_span`
  3. `cargo test -p scoopc_source`
  4. `cargo test --all --all-targets --no-default-features`
  5. `cargo clippy --all-targets -- -D warnings`
  6. 搜索 `pub struct Span|pub struct SourceFile|pub struct SourceId|pub struct SourceMap`，确认 authoritative 定义只在基础 crate 中。
- 完成条件：
  - `Span` 和 source 基础设施已从 `scoopc` 物理定义迁到基础 crate；
  - 旧 `scoopc::span` / `scoopc::source` 路径只是 re-export；
  - 下游代码无需桥接或转换即可共享同一基础类型。
- 依赖：P1-T01R
- 完成记录：
  - 改动范围：`Span` authoritative 定义迁入 `crates/scoopc_span/src/lib.rs`，保留 `miette::SourceSpan` 转换能力并补充基础单元测试；`SourceOrigin`、`SourceTrust`、`SourceFile`、`SourceId`、`SourceMapSpan`、`SourceLocation`、`SourceMap` 及原 source map 行列/UTF-8 boundary 测试迁入 `crates/scoopc_source/src/lib.rs`。
  - 核心决策：`scoopc_source` 只依赖 `scoopc_span` 和 `miette`；`crates/scoopc/src/span.rs` 与 `crates/scoopc/src/source.rs` 只保留 re-export adapter，因此旧 `scoopc::{span, source}` 路径与新基础 crate 路径共享同一套类型，没有桥接转换或重复 authoritative 定义。
  - 依赖方向：本次新增基础依赖为 `scoopc_source -> scoopc_span`，符合 P1 base crate DAG；`dependency-gate` 报告 `scoopc_source` 的 base deps 仅为 `scoopc_span`。
  - 验证命令：`cargo fmt`；`cargo test -p scoopc_span`；`cargo test -p scoopc_source`；`cargo test --all --all-targets --no-default-features`（首次 10 分钟外层超时中断后以 30 分钟超时重跑通过）；`cargo clippy --all-targets -- -D warnings`；`cargo run -p scoop_tools -- dependency-gate`；`cargo tree -p scoopc_source`；搜索 `pub struct Span|pub struct SourceFile|pub struct SourceId|pub struct SourceMap`。
  - 残余风险：全仓调用点仍大量经由迁移期 `scoopc::span` / `scoopc::source` 路径引用基础类型；这是 P1 允许的 adapter 状态，后续清场任务可逐步改为直接依赖基础 crate。

## [DONE] P1-T02R：Review `span` / `source` 迁移结果

- 参考：P1-T02。
- 重点：
  - 是否还存在重复 `Span` / `SourceFile` / `SourceId` / `SourceMap` 定义；
  - `scoopc_source` 是否保持只依赖基础层；
  - diagnostics、source map 和 UTF-8 boundary tests 是否未退化。
- 必须复查的范围：
  - `crates/scoopc_span/`
  - `crates/scoopc_source/`
  - `crates/scoopc/src/span.rs`
  - `crates/scoopc/src/source.rs`
  - `crates/scoopc/src/lib.rs`
  - `crates/scoopc/src/frontend.rs`
  - `crates/scoopc/src/session/mod.rs`
  - `crates/scoopc/src/sysroot/`
- 验证：
  - 重新运行 P1-T02 的所有验证；
  - 额外检查 `cargo tree -p scoopc_source` 不含 `scoopc`、parser、resolve、typecheck、HIR/MIR/LIR/codegen 依赖。
- 完成条件：
  - review 结论明确写出：`span` / `source` 已成为基础 crate authoritative 类型，且迁移没有改变 source/diagnostic 行为。
- 依赖：P1-T02
- 完成记录：
  - 复查范围：复查 `crates/scoopc_span/`、`crates/scoopc_source/`、`crates/scoopc/src/span.rs`、`crates/scoopc/src/source.rs`、`crates/scoopc/src/lib.rs`、`crates/scoopc/src/frontend.rs`、`crates/scoopc/src/session/mod.rs` 与 `crates/scoopc/src/sysroot/mod.rs`；全仓搜索 `Span` / `SourceFile` / `SourceId` / `SourceMap` authoritative 定义。
  - review 结论：`Span` 的 authoritative 定义只在 `scoopc_span`；`SourceFile`、`SourceId`、`SourceMap` 及相关 source map 类型只在 `scoopc_source`；`scoopc::span` / `scoopc::source` 仅为迁移期 re-export adapter，没有桥接转换或重复类型。
  - 依赖方向结论：`scoopc_source` 只依赖 `scoopc_span` 和 `miette`，`cargo tree -p scoopc_source` 未出现 `scoopc`、parser、resolve、typecheck、HIR/MIR/LIR/codegen 或 fact/backend 依赖；`dependency-gate` 报告 `scoopc_source` 的 base deps 仅为 `scoopc_span`。
  - 行为结论：基础 crate 单元测试与全 workspace no-default-features 测试覆盖 diagnostics/source map/UTF-8 boundary 行为，未发现迁移导致的 source/diagnostic 行为退化。
  - 验证命令：`cargo fmt`；`cargo test -p scoopc_span`；`cargo test -p scoopc_source`；`cargo test --all --all-targets --no-default-features`；`cargo clippy --all-targets -- -D warnings`；`cargo tree -p scoopc_source`；`cargo run -p scoop_tools -- dependency-gate`；搜索 `pub struct Span|pub struct SourceFile|pub struct SourceId|pub struct SourceMap`。
  - 残余风险：全仓仍大量通过旧 `crate::span` / `crate::source` 路径引用基础类型，这是 P1 允许的迁移期 adapter 状态；后续 P1 清场任务可继续收敛直接依赖基础 crate 的调用面。

## [TODO] P1-T03：迁移 `types` 并建立 `ids` 基础身份层

- 参考：
  - 本文件“触碰面基线 / `types` / `ids` / `stable_id`”
  - `PIPELINE_REFACTOR.md` “fact crate 必须自包含”
- 目标：
  - 将 `TypeId`、`TypeStore`、`TypeKind`、`EffectRow`、builtin type universe 和 layout 基础设施迁入 `scoopc_types`；
  - 建立 `scoopc_ids` 的第一批跨阶段基础身份类型和 stable hash/key primitives；
  - 让 `scoopc::ty` / `scoopc::stable_id` 成为迁移期 facade，而不是基础定义的 owner。
- 必须检查和修改的主要位置：
  - `crates/scoopc/src/ty/mod.rs`
  - `crates/scoopc/src/ty/layout.rs`
  - `crates/scoopc/src/stable_id.rs`
  - `crates/scoopc/src/mir/mod.rs` 中的 `SiteId`
  - `crates/scoopc/src/mir/materialize/mod.rs` 中的 `TemplateKey` / `InstanceKey`
  - typecheck/HIR/MIR/effect_facts/effect_lowered/RTTI/LLVM 中的 `TypeId`、`TypeStore`、`EffectRow`、stable id imports。
- 必须实现的内容：
  1. 将 `ty` 模块和 layout 基础设施迁入 `scoopc_types`，并改为依赖 `scoopc_span`。
  2. 在 `scoopc` 中保留 `ty` re-export adapter，避免一次性重写所有 call sites。
  3. 将跨阶段会作为 facts key 的基础身份 primitives 放入 `scoopc_ids`；至少覆盖 stable hash/key traits 与 `SiteId`，并保留后续 `BodyVersionKey` 扩展点。
  4. 更新 `stable_id` 使其消费 `scoopc_types` / `scoopc_ids` 的基础类型；若 `StableConeKey::from_manifest(...)` 暂时仍留在 `scoopc` adapter，必须在完成记录中说明迁出条件。
  5. 明确保留 `TemplateKey` / `InstanceKey` 为 MIR materialization 内部键，直到 P3 MIR facts/output 任务正式处理。
- 禁止事项：
  - 禁止引入第二套 `TypeStore` 或 “equivalent type id” 转换层来绕过迁移。
  - 禁止让 `scoopc_types` 依赖 typecheck、HIR、MIR、effect facts、LIR 或 LLVM。
  - 禁止把 MIR-internal `TemplateKey` / `InstanceKey` 当作全阶段基础 ID 提前发布。
- 验证：
  1. `cargo fmt`
  2. `cargo test -p scoopc_types`
  3. `cargo test -p scoopc_ids`
  4. `cargo test --all --all-targets --no-default-features`
  5. `cargo clippy --all-targets -- -D warnings`
  6. 搜索 `pub struct TypeId|pub struct TypeStore|pub struct EffectRow|pub struct SiteId`，确认 authoritative 定义位置符合本任务决策。
- 完成条件：
  - type 基础设施已由 `scoopc_types` 提供；
  - `scoopc_ids` 已承载第一批跨阶段基础身份 primitives；
  - `stable_id` 不再直接依赖 `scoopc` 私有 type 定义。
- 依赖：P1-T02R
- 完成记录：
  - 待填写。

## [TODO] P1-T03R：Review `types` / `ids` 迁移结果

- 参考：P1-T03。
- 重点：
  - `TypeId` / `TypeStore` / `EffectRow` 是否只有一套 authoritative 定义；
  - `scoopc_ids` 是否没有误收 MIR-owned materialization internals；
  - `stable_id` 是否已经从基础类型角度解耦，而不是继续绑在 `scoopc` 私有模块上。
- 必须复查的范围：
  - `crates/scoopc_types/`
  - `crates/scoopc_ids/`
  - `crates/scoopc/src/ty.rs` 或 `crates/scoopc/src/ty/`
  - `crates/scoopc/src/stable_id.rs`
  - `crates/scoopc/src/mir/mod.rs`
  - `crates/scoopc/src/mir/materialize/mod.rs`
  - `crates/scoopc/src/llvm/codegen/ty.rs`
- 验证：
  - 重新运行 P1-T03 的所有验证；
  - 额外检查 `cargo tree -p scoopc_types` 和 `cargo tree -p scoopc_ids` 不含 stage/fact/backend crate。
- 完成条件：
  - review 结论明确写出：types/ids 基础层满足后续 fact crate 自包含要求，或列出阻塞项并在本 review 内修复。
- 依赖：P1-T03
- 完成记录：
  - 待填写。

## [TODO] P1-T04：迁移 project model、cone graph 与 resolver cone identity

- 参考：
  - `PLAN.md` §1.4、§4/P1
  - `PIPELINE_REFACTOR.md` “编译单元定义”“编译顺序模型”
  - 本文件“触碰面基线 / `cone` / `project_model`”
- 目标：
  - 将 stage-independent 的 cone/project 数据模型迁入 `scoopc_project_model`；
  - 将 `ConeId` / `ConeInfo` 从 resolver ownership 中移出；
  - 固定 `SourceConeGraph` 作为 build graph，并保留 dependency topo order 语义。
- 必须检查和修改的主要位置：
  - `crates/scoopc/src/cone/manifest.rs`
  - `crates/scoopc/src/cone/package.rs`
  - `crates/scoopc/src/cone/graph.rs`
  - `crates/scoopc/src/cone/mod.rs`
  - `crates/scoopc/src/resolve/mod.rs` 中的 `ConeId` / `ConeInfo`
  - `crates/scoopc/src/frontend.rs`
  - `crates/scoopc/src/typecheck/type_env.rs`
  - `crates/scoopc/src/hir/lower/**`、`crates/scoopc/src/mir/**`、`crates/scoopc/src/llvm/**` 中的 `SourceConeInfo` / `ConeId` 调用点。
- 必须实现的内容：
  1. 将 `ConeKind`、manifest 数据结构、source-cone role/trust/dependency edge/node/info、`SourceConeGraph` 的纯数据与 topo validation 迁入 `scoopc_project_model`。
  2. 将 `ConeId` / `ConeInfo` 迁出 resolver；resolver 只能消费 project model 发布的 cone identity。
  3. 处理 `ConeNativeBuildConfig::opt_level` 的基础层依赖：要么将 backend-neutral `OptLevel` 放入基础配置层，要么改成 project model 自己的中立表示，禁止 `scoopc_project_model` 依赖 `scoopc::opt`。
  4. 对 filesystem/sysroot package loading 做清晰切分：纯 project model 类型在基础 crate，涉及 sysroot/session/filesystem 策略的 loader 可以暂留 `scoopc::cone` adapter，但不得成为 stage/fact crate 依赖入口。
  5. 更新 `SourceConeInfo` / `StableConeKey` 的来源，使 HIR/MIR/codegen 获取 cone metadata 时不再依赖 resolver 私有类型。
- 禁止事项：
  - 禁止让 `scoopc_project_model` 依赖 resolver、typecheck、HIR、MIR、effect facts、LIR、LLVM 或 `scoopc` facade。
  - 禁止把 source package filesystem loader 伪装成 stage/fact crate 可依赖的基础模型。
  - 禁止为了减少修改而继续让 `ConeId` 的 authoritative 定义留在 resolver。
- 验证：
  1. `cargo fmt`
  2. `cargo test -p scoopc_project_model`
  3. `cargo test --all --all-targets --no-default-features`
  4. `cargo clippy --all-targets -- -D warnings`
  5. 搜索 `pub struct ConeId|pub struct ConeInfo|pub struct SourceConeGraph|pub struct SourceConeInfo`，确认 authoritative 定义位置符合本任务决策。
- 完成条件：
  - project/cone membership、graph topology 和 manifest 数据已由 `scoopc_project_model` 承载；
  - resolver 不再拥有 `ConeId` / `ConeInfo` 定义；
  - `SourceConeGraph` 仍保证 dependency cones 位于 consumer cone 之前。
- 依赖：P1-T03R
- 完成记录：
  - 待填写。

## [TODO] P1-T04R：Review project model 与 cone graph 迁移结果

- 参考：P1-T04。
- 重点：
  - project model 是否只承载 stage-independent 数据；
  - resolver/typecheck/HIR/MIR/codegen 是否消费同一套 cone identity；
  - source-cone graph topo order、cycle reject、consumer uniqueness tests 是否仍有效。
- 必须复查的范围：
  - `crates/scoopc_project_model/`
  - `crates/scoopc/src/cone/`
  - `crates/scoopc/src/resolve/mod.rs`
  - `crates/scoopc/src/frontend.rs`
  - `crates/scoopc/src/typecheck/type_env.rs`
  - `crates/scoopc/src/hir/lower/`
  - `crates/scoopc/src/mir/`
  - `crates/scoopc/src/llvm/`
- 验证：
  - 重新运行 P1-T04 的所有验证；
  - 额外运行覆盖 `SourceConeGraph` 的单元测试；
  - 检查 `cargo tree -p scoopc_project_model` 不含 stage/fact/backend/facade 依赖。
- 完成条件：
  - review 结论明确写出：project model 和 cone graph 迁移满足 P1 crate DAG 与 cone DAG 语义，或列出阻塞项并在本 review 内修复。
- 依赖：P1-T04
- 完成记录：
  - 待填写。

## [TODO] P1-T05：固定 cone-level compilation unit facade API

- 参考：
  - `PLAN.md` §1.4、§4/P1
  - `PIPELINE_REFACTOR.md` “cone 才是源码级编译单元”“同一 cone 内：不是文件拓扑，而是阶段屏障”
  - 本文件“触碰面基线 / `frontend` / compilation unit API”
- 目标：
  - 明确 `ProjectInput` / `ProjectContext` / `SourceConeGraph` 的职责边界；
  - 在 facade 层提供 cone-level compilation unit API；
  - 把 AST stage handoff 从单文件形状升级为可表达 cone unit 的形状，或提供等价的 cone-level wrapper。
- 必须检查和修改的主要位置：
  - `crates/scoopc/src/frontend.rs`
  - `crates/scoopc/src/pipeline/ast_stage.rs`
  - `crates/scoopc/src/pipeline/mod.rs`
  - `crates/scoopc/src/driver_cli.rs`
  - `crates/scoopc/src/session/mod.rs`
  - `crates/scoopc/src/hir/lower/main/compilation_unit.rs`
  - `crates/scoopc/src/mir/mod.rs`
  - `crates/scoopc/src/llvm/codegen/mod.rs`
- 必须实现的内容：
  1. 在 project model 或 frontend facade 中引入明确的 `CompilationUnit` / `CompilationUnitInput` / 等价类型，表示“一个 cone 的全部 Scoop 源文件 + 依赖 cone 已发布 API/facts”。
  2. 将 `ProjectInput` 的 flatten `sources` 语义改名或包装为 build closure source view，避免任何 public API 把它称为单一 compilation unit。
  3. 提供按 source-cone DAG 拓扑顺序遍历 compilation units 的 API；consumer cone 必须能被明确定位。
  4. 将 `AstStageOutput` 或 pipeline facade 扩展为 cone-level AST handoff；单文件 AST output 只能作为 worker/helper，不得作为正式 project compilation unit output。
  5. 更新 HIR/MIR/codegen 当前消费的 `source_cones`、`stable_cone_key`、`request_source_paths` 来源，使它们来自 cone unit / project model API。
  6. 增加测试覆盖：虚拟单文件输入是 synthetic consumer cone；显式 cone 多文件按稳定路径遍历但语义上同属一个 unit；依赖 cone 在 consumer 前。
- 禁止事项：
  - 禁止把现有 flatten `Vec<SourceFile>` 直接改名为 `CompilationUnit` 而不区分 cone。
  - 禁止引入“先按文件完整编译再编下一个文件”的语义。
  - 禁止为单文件 fixture 保留独立于 source cone graph 的长期 project API。
- 验证：
  1. `cargo fmt`
  2. `cargo test -p scoopc --no-default-features frontend`
  3. `cargo test -p scoopc --no-default-features pipeline`
  4. `cargo test --all --all-targets --no-default-features`
  5. `cargo run -p scoop -- test --fixtures tests/fixtures/build`
  6. `cargo clippy --all-targets -- -D warnings`
  7. 搜索 `single file compilation unit|single-source compilation unit|current compilation unit.*sources|AstStageOutput`，确认活跃注释/API 不再误导。
- 完成条件：
  - facade/API 明确表达 build graph 与 cone compilation unit 的区别；
  - AST stage 或 pipeline handoff 能表达 cone-level unit；
  - 生产路径即使内部仍有过渡 flatten worker，也不再把 whole build closure 暴露为一个 compilation unit。
- 依赖：P1-T04R
- 完成记录：
  - 待填写。

## [TODO] P1-T05R：Review cone-level compilation unit API

- 参考：P1-T05。
- 重点：
  - facade 是否明确区分 project/build graph、source cone、compilation unit 和 source file；
  - 单文件模式是否只是 synthetic consumer cone 特例；
  - AST/frontend/pipeline API 是否还把单文件或 whole-build flatten 误称为 compilation unit。
- 必须复查的范围：
  - `crates/scoopc/src/frontend.rs`
  - `crates/scoopc/src/pipeline/ast_stage.rs`
  - `crates/scoopc/src/pipeline/mod.rs`
  - `crates/scoopc/src/driver_cli.rs`
  - `crates/scoopc/src/session/mod.rs`
  - `crates/scoopc/src/hir/lower/main/compilation_unit.rs`
  - `crates/scoopc/src/mir/mod.rs`
  - `crates/scoopc/src/llvm/codegen/mod.rs`
- 验证：
  - 重新运行 P1-T05 的所有验证；
  - 额外抽查至少一个 virtual input test 和一个 explicit cone/local dependency test。
- 完成条件：
  - review 结论明确写出：cone = compilation unit 已在 facade/API 层成立，或列出阻塞项并在本 review 内修复。
- 依赖：P1-T05
- 完成记录：
  - 待填写。

## [TODO] P1-T06：P1 全包清场、文档同步与依赖审计

- 参考：
  - P1-T01 到 P1-T05R 的完成记录
  - `PLAN.md` §4/P1、§6
  - `PIPELINE_REFACTOR.md` crate DAG 与 compilation unit 规则
- 目标：
  - 对 P1 的基础 crate、re-export、project model 和 cone unit API 做全仓一致性清场；
  - 同步文档和 TODO 索引；
  - 为 `TODO-3.md` / P2 的 HIR barrier + `hir_facts` 留出干净起点。
- 必须实现的内容：
  1. 全仓搜索并分类处理旧基础模块 owner 和误导性 compilation unit 文字：`crate::span`、`crate::source`、`crate::ty`、`crate::stable_id`、`crate::resolve::ConeId`、`single file compilation unit`、`whole build compilation unit`、`AstStageOutput`。
  2. 确认 `scoopc` 中保留的旧路径都是 re-export/adapter，并记录每个 adapter 的保留原因。
  3. 检查基础 crate 的 `Cargo.toml` 依赖和 `cargo tree`，确认没有 stage/fact/backend/facade 反向依赖。
  4. 更新 `README.md` 和相关 active docs 中的 crate/layout/project model 描述；只有阶段计划实际变化时才更新 `PLAN.md`。
  5. 同步更新 `TODO.md` 和本文件中 P1 完成记录需要的索引状态。
- 禁止事项：
  - 禁止把 P2/P3 的 HIR/MIR facts 拆分工作提前塞进 P1 清场。
  - 禁止留下未解释的旧 owner 路径或重复定义。
  - 禁止为了通过搜索而改写归档文档历史。
- 验证：
  1. `cargo fmt`
  2. `cargo test --all --all-targets --no-default-features`
  3. `cargo run -p scoop -- test`
  4. `cargo run -p scoop_tools -- spec-fixtures check`
  5. `cargo clippy --all-targets -- -D warnings`
  6. 全仓搜索 P1 清场关键词，并在完成记录中附允许命中分类摘要。
- 完成条件：
  - 后续 stage/fact crate 都有明确基础依赖层；
  - 文档和代码不再把单文件或 whole-build flatten 当正式 compilation unit 定义；
  - P2 可以直接在 cone-level AST/HIR input model 上建立 `HIR + hir_facts` 边界。
- 依赖：P1-T05R
- 完成记录：
  - 待填写。

## [TODO] P1-T06R：Review P1 全包完成度

- 参考：P1-T01 到 P1-T06 的所有完成记录。
- 重点：
  - P1 是否真的满足 `PLAN.md` §4/P1 的完成标准；
  - 基础 crate 是否可被后续 stage/fact crate 直接依赖；
  - fact crate 禁止依赖 stage/fact 的前置条件是否已经具备；
  - cone = compilation unit 是否在 facade/API 和文档中成立。
- 必须复查的范围：
  - `Cargo.toml`
  - `crates/scoopc_span/`
  - `crates/scoopc_source/`
  - `crates/scoopc_types/`
  - `crates/scoopc_ids/`
  - `crates/scoopc_project_model/`
  - `crates/scoopc/src/lib.rs`
  - `crates/scoopc/src/frontend.rs`
  - `crates/scoopc/src/pipeline/ast_stage.rs`
  - `crates/scoopc/src/cone/`
  - `crates/scoopc/src/resolve/mod.rs`
  - `README.md`
  - `PLAN.md`
  - `PIPELINE_REFACTOR.md`
- 验证：
  - 重新运行 P1-T06 的所有验证；
  - 额外逐项检查基础 crate 的 `cargo tree`；
  - 抽查至少一个 multi-file same-cone fixture 和一个 local dependency cone fixture，确认 API 不依赖文件语义顺序。
- 完成条件：
  - review 结论明确写出：P1 完成，可以进入 `TODO-3.md` / P2；或列出阻塞项并在本 review 内修复。
- 依赖：P1-T06
- 完成记录：
  - 待填写。
