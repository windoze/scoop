# Autonomous execution plan

## Selected task
**P10-T04-b: 让 cached dependency cone artifact 在所有下游 stage 都正确可见** (TODO-7.md:1038)

P10-T04R 的 review 已经在前一轮中识别了根因并阻塞了 review 任务。本轮的目标是完整实现 P10-T04-b 描述的修复，而不是接着 review。

## Bug recap (from previous review)

- 复现：fixture `tests/fixtures/run_pass_cone/source_path_dependency_public_call`，cold build → cache hit → 编辑 consumer source（`echo "" >> src/main.scoop`）→ rebuild
- 错误：`call site 无法为 fixtures.run_pass_cone.source_path_dependency_public_call.lib.dependencyValue 构建 surface contract`
- 根因：`EffectFactsTypeContext::build` 等下游 stage 从 `compilation_sources` **独立重建** Index/TypeEnv；frontend 把 cached dep API 注入了 frontend 那一份 Index/TypeEnv，但下游 stage 用的是它们自己 rebuild 的版本，看不到 cached dep。
- 同源问题潜在还出现在 `scoopc_mir/mir/materialize/inputs.rs:338`、`scoopc_mir/rtti/mod.rs:217`、`scoopc_mir/rtti/type_desc.rs:372`、`scoopc_cone/scoopir/export.rs:204`、`scoopc_cone/annotations.rs:113`。

## Code surface map (from explore agent)

### Frontend artifact injection path
- `crates/scoopc_cone/src/consume.rs:226-240` — `inject_cone_artifact_frontend_import` 入口
- `crates/scoopc_cone/src/consume.rs:242-463` — `inject_frontend_import_payload` 当前注入 Index/TypeEnv 的 types/funs/extension funs/annotation classes/visibility/synthetic source；**没有注入 FileTypeContext，也没有注入 file_cones / file_cone_infos / cone_kinds 等 Index cone-mapping fields**
- `crates/scoopc_cone/src/consume.rs:465-553` — `inject_non_public_symbols_into_index`
- `crates/scoopc_cone/src/artifact.rs:225-239` — `ConeArtifactFrontendImport`：public_api / annotation_classes / symbol_visibility / pre_specialize
- `crates/scoopc_cone/src/artifact.rs:99-147` — `ConeArtifactManifest`：cone_name/version/compiler/schema_versions —— **缺 cone_kind**

### Stages that independently rebuild Index/TypeEnv
- `crates/scoopc_effect_facts_stage/src/effect_facts/builder.rs:2164-2235` — `EffectFactsTypeContext::build`：`session.build_top_level_index(&sources)` + `TypeEnv::from_sysroot(...)` + `extend_from_file()`
- `crates/scoopc_effect_facts_stage/src/effect_facts/builder.rs:245-284` — `surface_callable_contract`：`self.index.by_fqn.get(fqn)` 失败处
- `crates/scoopc_mir/src/mir/materialize/inputs.rs:271-399` — `prepare_materialize_inputs`：line 318-324 Index::build + line 338 TypeEnv::from_sysroot
- `crates/scoopc_mir/src/rtti/mod.rs:196-244` — `RttiBuilder::build`：line 209 Index::build + line 217 TypeEnv::from_sysroot
- `crates/scoopc_mir/src/rtti/type_desc.rs:372` — 类似
- `crates/scoopc_cone/src/scoopir/export.rs:112-254` — `export_public_api_for_cone_sources`：line 158-175 Index::build_with_cones + line 204 TypeEnv::from_sysroot
- `crates/scoopc_cone/src/annotations.rs:75-159` — `collect_cone_preserved_annotation_classes_for_cone_sources`：类似

### Pipeline plumbing
- `crates/scoopc/src/frontend.rs:628-852` — `run_frontend_with_artifact_cache`：line 672-692 cache-hit dep 加载到 `published_artifacts`；line 738-752 注入 frontend Index/TypeEnv；**没有把 cached dep payload 沿 stage 边界向后透传**
- `crates/scoopc/src/pipeline/llvm_codegen_stage.rs:331-380` — `run_lir_stage_from_lowered_hir`：line 348 source_map_compilation_sources；line 349-354 调用 `build_effect_facts_stage_output_with_compilation_sources`
- `crates/scoopc/src/pipeline/mod.rs:118-130` — `build_effect_facts_stage_output_with_compilation_sources` 当前签名只有 session/source/compilation_sources/mir_stage_output
- `crates/scoopc/src/pipeline/effect_facts_stage.rs:75-135` — `run_with_compilation_sources` 同样不接 cached dep

### Crate dependency direction
- `scoopc_cone -> scoopc_effect_facts_stage`（forward）→ effect_facts_stage 不能反向依赖 cone
- `scoopc_hir` 是 cone / mir / effect_facts_stage 共同的 base → 适合放中性 inject API

## Refactor design

### 1. 新中性 inject API：`scoopc_hir::cone_import` 模块
新建 `crates/scoopc_hir/src/cone_import.rs`：
- 中性数据类型（不依赖 scoopc_cone 的 ScoopIrFile schema）：
  - `pub struct CachedConeImport { cone_id, cone_kind, cone_name, cone_version, public_types, public_funs, annotation_classes, non_public_symbols, synthetic_decl_file }`
  - `CachedConePublicType { fqn, kind, type_params, alias_of }`（不直接复用 IrTypeDecl/IrFunDecl，避免引入 ScoopIrFile 依赖）
  - `CachedConePublicFun { fqn, kind, type_params, receiver, params, return_ty, effects, ... }`
  - `CachedConeAnnotationClass { fqn, targets, retention }`
  - `CachedConeNonPublicSymbol { kind, fqn, visibility }`
- 中性 inject helper：
  - `pub fn inject_cached_cone_imports(&mut Index, &mut TypeEnv, &[CachedConeImport]) -> Result<...>`
  - 内部覆盖：types / funs / extension funs / annotation classes / non-public visibility / synthetic source / **FileTypeContext**（pkg_prefix + 空 imports + ConeInfo { id, kind }）/ Index 的 file_cones, file_cone_infos, cone_kinds 等 cone-mapping
- 把现有 `SyntheticSourceBuilder`、`last_segment`、`ir_type_to_type_ref`、`ir_effect_row_to_effect_row_expr`、`inject_type_symbol_into_index`、`inject_non_public_symbols_into_index` 这些 helper 全部搬到 scoopc_hir（中性化或保留 scoopc_cone 端的转换）

注意：scoopc_hir 不直接消费 IrType。`ir_type_to_type_ref` 等转换器 stay in scoopc_cone（它们是 wire format → neutral/HIR-side 桥梁）。scoopc_hir 的中性类型应该用 `ast::TypeRef` / `ast::EffectRowExpr` 等已有的中性表达。

实际上更简单：scoopc_hir 的中性 payload 直接持有 ast::TypeRef（HIR-side 已经依赖 ast）。scoopc_cone 把 IrType 转换成 ast::TypeRef，填进中性 payload，再调用 scoopc_hir helper。

### 2. 持久化 cone_kind
- `ConeArtifactManifest` 加 `pub cone_kind: ConeKind` 字段
- `current()` 方法接收 ConeKind（变更签名，调用方 `build_frontend_import_for_typechecked_cone` 与 `ConeArtifact::new` 处提供）
- ConeKind 自身需要 `Serialize`/`Deserialize` —— 当前是 `#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]`，要加 serde derive
- `frontend_import` schema version bump（避免无 cone_kind 的旧 artifact 静默放行）—— `ensure_compatible` 现在已经只放行 current 版本，所以 bump version 自动拒绝旧 artifact，符合 P10-T02 的 coarse-grained 兼容策略

### 3. scoopc_cone 改造
- `inject_frontend_import_payload` 改为：
  1. 把 ScoopIrFile / ConeAnnotationClassesFile / ConeSymbolVisibilityFile 的内容转换成 scoopc_hir::cone_import::CachedConeImport
  2. 调用 `scoopc_hir::cone_import::inject_cached_cone_imports`
- `inject_cone_artifact_frontend_import` 改为读 ConeArtifactManifest 中的 cone_kind，连同 cone_id / name / version 一起转换 + 注入

### 4. Pipeline plumbing
- `frontend.rs::run_frontend_with_artifact_cache` 在 cache-hit dep 处构造 `Vec<CachedConeImport>` 并随 `FrontendOutput` 返回（新字段 `pub cached_cone_imports: Vec<CachedConeImport>`）
- `run_lir_stage_from_lowered_hir`、`build_effect_facts_stage_output_with_compilation_sources`、`build_effect_facts_stage_output`、`effect_facts_stage::run_with_compilation_sources` 加 `cached_cone_imports: &[CachedConeImport]` 参数
- `EffectFactsTypeContext::build` 签名扩展：build 完 env+index 后直接调用 `inject_cached_cone_imports`

### 5. 其它独立重建路径
- `scoopc_mir::mir::materialize::inputs.rs::prepare_materialize_inputs` 加 `cached_cone_imports` 参数；调用方（pipeline 中的 mir stage）从 frontend 输出取
- `scoopc_mir::rtti::RttiBuilder::build` 同样加参数
- `scoopc_mir::rtti::type_desc::*:372` 同样
- `scoopc_cone::scoopir::export.rs::export_public_api_for_cone_sources` —— 这是 export side（消费者本 cone 的 public API），不消费 dep API；但如果它在某个 dep 是 cache-hit 时被调用且需要看 dep symbol，需要扩展。先审计 caller。
- `scoopc_cone::annotations.rs::collect_cone_preserved_annotation_classes_for_cone_sources` 同上

实际上 export.rs / annotations.rs 是从 frontend 已 typechecked state 生成 artifact 的工具，它们不应该看 cached dep（因为本 cone artifact 不需要包含 dep symbol）。这条审计应该确认调用语义而不是盲目加注入。

### 6. Tests
- `crates/scoopc_hir/src/cone_import.rs` 单测：构造 fake CachedConeImport，注入空 Index/TypeEnv，断言 by_fqn / extension_funs / FileTypeContext 等都注入到位
- `crates/scoopc/src/frontend.rs` 单测：扩展现有 `dependency_frontend_cache_hit_uses_artifact_without_reading_source` 为：cache-hit dep + consumer-edit + 跑完整 codegen，断言 success
- 添加 `cached_dep_visible_to_effect_facts_stage` 集成测试
- mir/rtti unit-level regressions（即使当前 fixture 触发不到，也加单测覆盖，避免回归）

### 7. 实现顺序
1. **复现 bug**（先验证根因）
2. **scoopc_hir cone_import** 模块 + 中性类型 + helper（新增）
3. **artifact.rs cone_kind** + ConeKind serde derive
4. **scoopc_cone consume.rs** 重写为 wrapper + 转换器
5. **frontend.rs 聚合 + plumb**（FrontendOutput 加字段）
6. **effect_facts stage** plumb
7. **mir / rtti** plumb
8. **export.rs / annotations.rs** 审计（不一定改）
9. **测试**（包括新 regression）
10. 全套验证（fmt/clippy/test/scoop fixtures）
11. 更新 TODO.md / TODO-7.md，commit

## Progress log
- 2026-05-25: 选定 P10-T04-b。读完 TODO-7.md / claude_plan.md / 探索代码。设计完成。
- 准备复现 bug 验证根因
