//! lower_for_compilation_unit family + internal helpers (generic instance MIR collection, stable-cone key, options).

#![allow(dead_code)]

use super::*;

pub(crate) fn generic_template_symbol_suffixes_for_compilation_unit(
    stable_cone_key: &StableConeKey,
    index: &Index,
    compilation_unit: &[(&SourceFile, &ast::File)],
) -> util::GenericTemplateSymbolSuffixIndex {
    util::collect_generic_template_symbol_suffixes_with_stable_cone_key(
        stable_cone_key,
        index,
        compilation_unit,
    )
}

/// 在“给定编译单元（多个源文件）”的上下文中，为其中一个文件生成 HIR。
///
/// 用途：
/// - `.cone` 打包时导出 `api.scoopir`（TODO T1104）需要跨文件可见的类型 kind 信息；
/// - 后续多包编译/链接流程也会复用类似的“多文件 lowering”入口。
///
/// 约定：
/// - `file` 必须已经过 resolver（至少 `check_file_headers + check_file_bodies`），
///   以保证标识符绑定信息已写回 AST；
/// - `compilation_unit` 应包含 sysroot + 当前 cone 的全部源文件（稳定排序可保证输出更可回归）。
pub fn lower_for_compilation_unit(
    source: &SourceFile,
    file: &ast::File,
    index: &Index,
    compilation_unit: &[(&SourceFile, &ast::File)],
) -> Result<LoweredHir, HirLowerError> {
    lower_for_compilation_unit_with_stable_cone_key(
        StableConeKey::for_virtual_source_path(source.path()),
        source,
        file,
        index,
        compilation_unit,
    )
}

pub fn lower_for_compilation_unit_with_stable_cone_key(
    stable_cone_key: StableConeKey,
    source: &SourceFile,
    file: &ast::File,
    index: &Index,
    compilation_unit: &[(&SourceFile, &ast::File)],
) -> Result<LoweredHir, HirLowerError> {
    let type_kinds = collect_type_decl_kinds(compilation_unit);
    let nominal_variances = collect_nominal_variances(compilation_unit);
    let direct_supertypes = collect_direct_supertypes(compilation_unit, index);
    let known_receiver_subclasses =
        crate::devirtualize::collect_known_receiver_subclasses(&direct_supertypes);
    let delegated_properties = collect_delegated_properties(compilation_unit);
    let default_arg_structs = collect_default_arg_structs(compilation_unit);
    let computed_property_accessors = collect_computed_property_accessor_fqns(compilation_unit);
    let class_vtables = crate::vtable::collect_class_vtables(compilation_unit, index)?;
    let (interfaces, class_itables) = crate::itable::collect_interfaces_and_class_itables(
        compilation_unit,
        index,
        &class_vtables,
    )?;
    let continuation_resume_call_sites = file
        .continuation_resume_call_sites()
        .into_iter()
        .map(|span| CallSite::new(source.path().to_path_buf(), span))
        .collect();
    let non_pure_continuation_resume_call_sites = file
        .non_pure_continuation_resume_call_sites()
        .into_iter()
        .map(|span| CallSite::new(source.path().to_path_buf(), span))
        .collect();

    let mut types = TypeStore::new();
    let builtins = types.intern_builtins();
    let generic_template_symbol_suffixes =
        util::collect_generic_template_symbol_suffixes_with_stable_cone_key(
            &stable_cone_key,
            index,
            compilation_unit,
        );

    // 先降 HIR（保持 `TypeId` 分配顺序稳定），再补充 side tables（layout/extern/object init）。
    let pkg_prefix = package_prefix(source, file.package.as_ref());
    let (
        file_hir,
        member_funs,
        mut ctor_call_sites,
        mut dispatch_call_sites,
        effect_op_call_sites,
        handle_payload_tuple_tys,
        mut with_update_contracts,
        mut assign_place_contracts,
        top_level_vars,
        extern_globals,
        top_level_immutable_values,
        when_pat_binding_tys,
    ) = {
        let mut ctx = HirLowering::new(
            source,
            file,
            index,
            &mut types,
            HirLoweringSetup {
                typecheck_types: None,
                type_kinds: &type_kinds,
                delegated_properties: &delegated_properties,
                compilation_unit,
                default_arg_structs: default_arg_structs.clone(),
                computed_property_getters: &computed_property_accessors.getters,
                computed_property_setters: &computed_property_accessors.setters,
                builtins,
                generic_template_symbol_suffixes: &generic_template_symbol_suffixes,
                known_receiver_subclasses: &known_receiver_subclasses,
                class_vtables: &class_vtables,
                interfaces: &interfaces,
                class_itables: &class_itables,
                materialize_direct_call_targets: true,
                devirtualize_dispatch_calls: false,
            },
        );
        let file_hir = ctx.lower_file();
        if let Some(err) = ctx.take_stage_error() {
            return Err(err.into());
        }
        let member_funs = ctx.collect_member_funs(&pkg_prefix);
        if let Some(err) = ctx.take_stage_error() {
            return Err(err.into());
        }
        ctx.record_missing_assign_place_contracts_in_file(&file_hir);
        ctx.record_missing_assign_place_contracts_in_funs(&member_funs);
        let ctor_call_sites = std::mem::take(&mut ctx.ctor_call_sites);
        let dispatch_call_sites = std::mem::take(&mut ctx.dispatch_call_sites);
        let effect_op_call_sites = std::mem::take(&mut ctx.effect_op_call_sites);
        let handle_payload_tuple_tys = std::mem::take(&mut ctx.handle_payload_tuple_tys);
        let with_update_contracts = std::mem::take(&mut ctx.with_update_contracts);
        let assign_place_contracts = std::mem::take(&mut ctx.assign_place_contracts);
        let top_level_vars = std::mem::take(&mut ctx.top_level_vars);
        let extern_globals = std::mem::take(&mut ctx.extern_globals);
        let top_level_immutable_values = std::mem::take(&mut ctx.top_level_immutable_values);
        let when_pat_binding_tys = std::mem::take(&mut ctx.when_pat_binding_tys);
        (
            file_hir,
            member_funs,
            ctor_call_sites,
            dispatch_call_sites,
            effect_op_call_sites,
            handle_payload_tuple_tys,
            with_update_contracts,
            assign_place_contracts,
            top_level_vars,
            extern_globals,
            top_level_immutable_values,
            when_pat_binding_tys,
        )
    };

    let (
        object_inits,
        class_inits,
        side_table_ctor_call_sites,
        side_table_dispatch_call_sites,
        side_table_with_update_contracts,
        side_table_assign_place_contracts,
    ) = collect_compilation_unit_object_and_class_inits(
        compilation_unit,
        CompilationUnitInitCollectionInputs {
            index,
            type_kinds: &type_kinds,
            known_receiver_subclasses: &known_receiver_subclasses,
            class_vtables: &class_vtables,
            interfaces: &interfaces,
            class_itables: &class_itables,
            typecheck_types: None,
            materialize_direct_call_targets: true,
            devirtualize_dispatch_calls: false,
            builtins,
        },
        &mut types,
    )?;
    ctor_call_sites.extend(side_table_ctor_call_sites);
    dispatch_call_sites.extend(side_table_dispatch_call_sites);
    with_update_contracts.extend(side_table_with_update_contracts);
    assign_place_contracts.extend(side_table_assign_place_contracts);
    let extern_funs = collect_extern_funs(source, file);
    let native_callable_funs = collect_native_callable_funs(source, file);
    let extern_libs = collect_extern_libs(compilation_unit);
    let mut struct_layouts = collect_struct_layouts(compilation_unit, index, &mut types);
    let mut enum_layouts = collect_enum_layouts(compilation_unit, index, &mut types);
    // T0124：泛型 struct/enum 的具体实例化布局。
    struct_layouts.extend(collect_generic_struct_instantiation_layouts(
        compilation_unit,
        index,
        &mut types,
    ));
    enum_layouts.extend(collect_generic_enum_instantiation_layouts(
        compilation_unit,
        index,
        &mut types,
    ));
    // T0125：泛型 class 的具体实例化 ClassInit。
    let class_inits = {
        let mut ci = class_inits;
        ci.extend(collect_generic_class_instantiation_inits(
            compilation_unit,
            &mut types,
            &ci,
        ));
        ci
    };
    let mut top_level_fun_call_sites = collect_top_level_fun_call_sites(&[(source, file)]);
    top_level_fun_call_sites.extend(collect_synthetic_named_intrinsic_call_sites_for_file(
        index,
        &file_hir,
        &member_funs,
    ));
    let call_arg_bindings = collect_call_arg_bindings(&[(source, file)]);
    let stable_type_param_keys = collect_stable_type_param_keys(compilation_unit, &stable_cone_key);
    let no_source_cone_overrides = HashMap::new();
    let source_cones = source_cones_for_lowering(
        compilation_unit,
        index,
        &stable_cone_key,
        &no_source_cone_overrides,
    );

    Ok(LoweredHir {
        file: file_hir,
        stable_cone_key,
        source_cones,
        stable_type_param_keys,
        member_funs,
        materialized_mir: None,
        types,
        struct_layouts,
        enum_layouts,
        extern_funs,
        native_callable_funs,
        extern_globals,
        extern_libs,
        top_level_vars,
        top_level_immutable_values,
        top_level_fun_call_sites,
        call_arg_bindings,
        with_update_contracts,
        assign_place_contracts,
        object_inits,
        class_inits,
        class_vtables,
        interfaces,
        class_itables,
        ctor_call_sites,
        dispatch_call_sites,
        effect_op_call_sites,
        handle_payload_tuple_tys,
        continuation_resume_call_sites,
        non_pure_continuation_resume_call_sites,
        when_pat_binding_tys,
        nominal_kinds: type_kinds,
        nominal_variances,
        direct_supertypes,
        builtins,
    })
}

/// 在”给定编译单元（多个源文件）”的上下文中，把多个文件一起 lowering 为一个 `LoweredHir`。
///
/// 用途（T1315a）：
/// - `scoop build/run` 注入 sysroot support sources 后，需要让这些文件里的顶层函数在后端可见；
/// - T0150c：HIR 继续保留本地 span，但不再在 lowering 时 eager parse Int/String 字面量；
///   后续阶段通过“声明所属源文件 + 本地 span”回查原文。
pub fn lower_for_compilation_unit_multi_files(
    entry_source: &SourceFile,
    index: &Index,
    compilation_unit: &[(&SourceFile, &ast::File)],
    files_to_lower: &[(&SourceFile, &ast::File)],
    monomorph_keys: &[crate::monomorph::MonomorphKey],
    typecheck_types: &TypeStore,
) -> Result<LoweredHir, HirLowerError> {
    let stable_cone_key = virtual_stable_cone_key_for_sources(Some(entry_source), compilation_unit);
    lower_for_compilation_unit_multi_files_internal(
        index,
        compilation_unit,
        files_to_lower,
        monomorph_keys,
        None,
        typecheck_types,
        CompilationUnitLoweringOptions::direct_lowered_hir(stable_cone_key),
    )
}

pub fn lower_for_compilation_unit_multi_files_with_type_env(
    index: &Index,
    compilation_unit: &[(&SourceFile, &ast::File)],
    files_to_lower: &[(&SourceFile, &ast::File)],
    monomorph_keys: &[crate::monomorph::MonomorphKey],
    type_env: Option<&crate::typecheck::TypeEnv>,
    typecheck_types: &TypeStore,
) -> Result<LoweredHir, HirLowerError> {
    let stable_cone_key = virtual_stable_cone_key_for_sources(
        files_to_lower.first().map(|(source, _)| *source),
        compilation_unit,
    );
    lower_for_compilation_unit_multi_files_internal(
        index,
        compilation_unit,
        files_to_lower,
        monomorph_keys,
        type_env,
        typecheck_types,
        CompilationUnitLoweringOptions::direct_lowered_hir(stable_cone_key),
    )
}

/// 为 build / single-file LLVM frontend 生成“由 MIR instance collection 决定实例集合”的 HIR 兼容输入。
///
/// 说明：
/// - 该入口先复用 typechecked compilation-unit facts 做 MIR materialization；
/// - 再只按 MIR 产出的 `InstanceKey` 集合生成当前 LLVM codegen 仍需要的 monomorphic HIR fun/member；
/// - 因而实例发现职责归属于 MIR，而不是继续由 HIR 自己扫描 `MonomorphKey` / `TypeStore`。
pub fn lower_for_compilation_unit_multi_files_via_mir_instance_collection(
    index: &Index,
    compilation_unit: &[(&SourceFile, &ast::File)],
    files_to_lower: &[(&SourceFile, &ast::File)],
    monomorph_requests: &[crate::monomorph::MonomorphRequest],
    type_env: Option<&crate::typecheck::TypeEnv>,
    typecheck_types: &TypeStore,
) -> Result<LoweredHir, Box<crate::mir::MirMaterializeError>> {
    lower_for_compilation_unit_multi_files_via_mir_instance_collection_with_opt_level(
        index,
        compilation_unit,
        files_to_lower,
        monomorph_requests,
        type_env,
        typecheck_types,
        crate::opt::OptLevel::O0,
    )
}

pub fn lower_for_compilation_unit_multi_files_via_mir_instance_collection_with_opt_level(
    index: &Index,
    compilation_unit: &[(&SourceFile, &ast::File)],
    files_to_lower: &[(&SourceFile, &ast::File)],
    monomorph_requests: &[crate::monomorph::MonomorphRequest],
    type_env: Option<&crate::typecheck::TypeEnv>,
    typecheck_types: &TypeStore,
    opt_level: crate::opt::OptLevel,
) -> Result<LoweredHir, Box<crate::mir::MirMaterializeError>> {
    let request_source_paths = files_to_lower
        .iter()
        .map(|(source, _)| source.path().to_path_buf())
        .collect::<Vec<_>>();
    let source_cones = HashMap::new();
    lower_for_compilation_unit_multi_files_via_mir_instance_collection_with_request_sources(
        index,
        compilation_unit,
        files_to_lower,
        monomorph_requests,
        type_env,
        typecheck_types,
        MirInstanceCollectionOptions {
            stable_cone_key: virtual_stable_cone_key_for_sources(
                files_to_lower.first().map(|(source, _)| *source),
                compilation_unit,
            ),
            source_cones: &source_cones,
            request_source_paths: &request_source_paths,
            request_root_mode: crate::mir::MaterializeRequestRootMode::RequestSources,
            opt_level,
        },
    )
}

pub struct MirInstanceCollectionOptions<'a> {
    pub stable_cone_key: StableConeKey,
    pub source_cones: &'a HashMap<std::path::PathBuf, crate::cone::SourceConeInfo>,
    pub request_source_paths: &'a [std::path::PathBuf],
    pub request_root_mode: crate::mir::MaterializeRequestRootMode<'a>,
    pub opt_level: crate::opt::OptLevel,
}

/// 为 build / frontend 生成“由 MIR instance collection 决定实例集合”的 HIR 兼容输入，
/// 但允许把“参与 lowering 的文件集合”和“允许贡献实例请求的 request roots”显式分离。
///
/// 说明：
/// - `files_to_lower` 仍决定哪些文件的顶层声明 / body 会进入 HIR 兼容输出；
/// - `request_source_paths` 只决定哪些源文件可以贡献 monomorphization 请求与 request-root
///   可达扫描；
/// - `request_root_mode` 决定 request roots 是整个 request source 集合，还是 production
///   executable 的 entry-main / export entry points；
/// - 这样 frontend 就能把 sysroot support sources 留在 lowering / fun_index 中，
///   同时避免把这些支持文件里未被入口触达的 generic 调用错误提升为实例收集种子。
/// - 返回值中的 `LoweredHir` 仍承载当前 LLVM codegen 所需的 HIR 兼容输入，但会额外挂住
///   `LoweredHir::materialized_mir()`，作为 production 主路径保留的 canonical materialized
///   MIR / summary 产物。
pub fn lower_for_compilation_unit_multi_files_via_mir_instance_collection_with_request_sources(
    index: &Index,
    compilation_unit: &[(&SourceFile, &ast::File)],
    files_to_lower: &[(&SourceFile, &ast::File)],
    monomorph_requests: &[crate::monomorph::MonomorphRequest],
    type_env: Option<&crate::typecheck::TypeEnv>,
    typecheck_types: &TypeStore,
    options: MirInstanceCollectionOptions<'_>,
) -> Result<LoweredHir, Box<crate::mir::MirMaterializeError>> {
    let MirInstanceCollectionOptions {
        stable_cone_key,
        source_cones,
        request_source_paths,
        request_root_mode,
        opt_level,
    } = options;
    let materialized =
        crate::mir::materialize_compilation_unit_from_typechecked_inputs_with_options(
            compilation_unit,
            index,
            type_env,
            typecheck_types,
            monomorph_requests,
            crate::mir::MaterializeCompilationUnitOptions {
                stable_cone_key: stable_cone_key.clone(),
                source_cones,
                request_source_paths,
                request_root_mode,
                opt_level,
            },
        )?;
    let mut lowered = lower_for_compilation_unit_multi_files_internal(
        index,
        compilation_unit,
        files_to_lower,
        &[],
        type_env,
        typecheck_types,
        CompilationUnitLoweringOptions::explicit_mir_instances(
            stable_cone_key,
            &materialized.instance_keys,
            &materialized.types,
            true,
        )
        .with_source_cones(source_cones),
    )?;
    for ty in materialized.types.iter_ids() {
        let _ = lowered.types.re_intern_from(&materialized.types, ty);
    }
    lowered.materialized_mir = Some(materialized);
    Ok(lowered)
}

/// 为 typed dump / MIR materializer 构造“只保留 generic template”的多文件 HIR。
///
/// 该入口会复用 resolver/typecheck 事实，但显式关闭 HIR lowering 中遗留的 generic
/// `::<...>` 实例物化路径，使实例身份只在后续 MIR 层建立。
pub(crate) fn lower_generic_for_compilation_unit_multi_files_with_type_env(
    stable_cone_key: StableConeKey,
    index: &Index,
    compilation_unit: &[(&SourceFile, &ast::File)],
    files_to_lower: &[(&SourceFile, &ast::File)],
    type_env: Option<&crate::typecheck::TypeEnv>,
    typecheck_types: &TypeStore,
) -> Result<LoweredHir, HirLowerError> {
    lower_for_compilation_unit_multi_files_internal(
        index,
        compilation_unit,
        files_to_lower,
        &[],
        type_env,
        typecheck_types,
        CompilationUnitLoweringOptions::generic_template_only(stable_cone_key),
    )
}

pub(crate) enum CompilationUnitInstanceMode<'a> {
    DirectLoweredHir,
    ExplicitMirInstances {
        instance_keys: &'a [crate::mir::InstanceKey],
        instance_types: &'a TypeStore,
    },
    GenericTemplateOnly,
}

pub(crate) struct CompilationUnitLoweringOptions<'a> {
    pub(crate) stable_cone_key: StableConeKey,
    pub(crate) source_cones: HashMap<std::path::PathBuf, crate::cone::SourceConeInfo>,
    pub(crate) instance_mode: CompilationUnitInstanceMode<'a>,
    pub(crate) devirtualize_dispatch_calls: bool,
}

impl<'a> CompilationUnitLoweringOptions<'a> {
    pub(crate) fn direct_lowered_hir(stable_cone_key: StableConeKey) -> Self {
        Self {
            stable_cone_key,
            source_cones: HashMap::new(),
            instance_mode: CompilationUnitInstanceMode::DirectLoweredHir,
            devirtualize_dispatch_calls: false,
        }
    }

    pub(crate) fn explicit_mir_instances(
        stable_cone_key: StableConeKey,
        instance_keys: &'a [crate::mir::InstanceKey],
        instance_types: &'a TypeStore,
        devirtualize_dispatch_calls: bool,
    ) -> Self {
        Self {
            stable_cone_key,
            source_cones: HashMap::new(),
            instance_mode: CompilationUnitInstanceMode::ExplicitMirInstances {
                instance_keys,
                instance_types,
            },
            devirtualize_dispatch_calls,
        }
    }

    pub(crate) fn generic_template_only(stable_cone_key: StableConeKey) -> Self {
        Self {
            stable_cone_key,
            source_cones: HashMap::new(),
            instance_mode: CompilationUnitInstanceMode::GenericTemplateOnly,
            devirtualize_dispatch_calls: false,
        }
    }

    pub(crate) fn with_source_cones(
        mut self,
        source_cones: &HashMap<std::path::PathBuf, crate::cone::SourceConeInfo>,
    ) -> Self {
        self.source_cones = source_cones.clone();
        self
    }

    pub(crate) fn materialize_direct_call_targets(&self) -> bool {
        !matches!(
            self.instance_mode,
            CompilationUnitInstanceMode::GenericTemplateOnly
        )
    }
}

pub(crate) fn virtual_stable_cone_key_for_sources(
    primary_source: Option<&SourceFile>,
    compilation_unit: &[(&SourceFile, &ast::File)],
) -> StableConeKey {
    primary_source
        .map(|source| StableConeKey::for_virtual_source_path(source.path()))
        .or_else(|| {
            compilation_unit
                .first()
                .map(|(source, _)| StableConeKey::for_virtual_source_path(source.path()))
        })
        .unwrap_or_else(|| StableConeKey::new("virtual-cone", "0.0.0"))
}

fn source_cones_for_lowering(
    compilation_unit: &[(&SourceFile, &ast::File)],
    index: &Index,
    fallback_stable_cone_key: &StableConeKey,
    overrides: &HashMap<std::path::PathBuf, crate::cone::SourceConeInfo>,
) -> HashMap<std::path::PathBuf, crate::cone::SourceConeInfo> {
    let mut out = HashMap::new();
    for (source, _) in compilation_unit {
        let path = source.path().to_path_buf();
        let info = overrides.get(&path).cloned().unwrap_or_else(|| {
            let cone = index.cone_info_of_source(source);
            crate::cone::SourceConeInfo {
                id: cone.id,
                kind: cone.kind,
                stable_key: fallback_stable_cone_key.clone(),
                trust: if source.is_trusted_syslib() {
                    crate::cone::SourceConeTrust::TrustedSyslib
                } else {
                    crate::cone::SourceConeTrust::Untrusted
                },
            }
        });
        out.insert(path, info);
    }
    out
}

pub(crate) fn lower_for_compilation_unit_multi_files_internal<'a>(
    index: &Index,
    compilation_unit: &[(&SourceFile, &ast::File)],
    files_to_lower: &[(&SourceFile, &ast::File)],
    monomorph_keys: &[crate::monomorph::MonomorphKey],
    type_env: Option<&crate::typecheck::TypeEnv>,
    typecheck_types: &TypeStore,
    options: CompilationUnitLoweringOptions<'a>,
) -> Result<LoweredHir, HirLowerError> {
    let materialize_direct_call_targets = options.materialize_direct_call_targets();
    let CompilationUnitLoweringOptions {
        stable_cone_key,
        source_cones: source_cone_overrides,
        instance_mode,
        devirtualize_dispatch_calls,
    } = options;
    let type_kinds = collect_type_decl_kinds(compilation_unit);
    let nominal_variances = collect_nominal_variances(compilation_unit);
    let direct_supertypes = collect_direct_supertypes(compilation_unit, index);
    let known_receiver_subclasses =
        crate::devirtualize::collect_known_receiver_subclasses(&direct_supertypes);
    let delegated_properties = collect_delegated_properties(compilation_unit);
    let default_arg_structs = collect_default_arg_structs(compilation_unit);
    let computed_property_accessors = collect_computed_property_accessor_fqns(compilation_unit);
    let class_vtables = crate::vtable::collect_class_vtables(compilation_unit, index)?;
    let (interfaces, class_itables) = match type_env {
        Some(env) => crate::itable::collect_runtime_interfaces_and_class_itables_with_env(
            compilation_unit,
            index,
            &class_vtables,
            env,
            typecheck_types,
        )?,
        None => crate::itable::collect_runtime_interfaces_and_class_itables(
            compilation_unit,
            index,
            &class_vtables,
            typecheck_types,
        )?,
    };

    let mut types = TypeStore::new();
    let builtins = types.intern_builtins();
    let generic_template_symbol_suffixes =
        util::collect_generic_template_symbol_suffixes_with_stable_cone_key(
            &stable_cone_key,
            index,
            compilation_unit,
        );

    let mut decls: Vec<Decl> = Vec::new();
    let mut items: Vec<Item> = Vec::new();
    let mut member_funs: Vec<FunDecl> = Vec::new();
    let mut ctor_call_sites: CtorCallSiteIndex = HashMap::new();
    let mut dispatch_call_sites: crate::hir::DispatchCallSiteIndex = HashMap::new();
    let mut effect_op_call_sites: crate::hir::EffectOpCallSiteIndex = HashMap::new();
    let mut handle_payload_tuple_tys: crate::hir::HandlePayloadTupleSiteIndex = HashMap::new();
    let mut with_update_contracts: WithUpdateSiteIndex = HashMap::new();
    let mut assign_place_contracts: AssignPlaceSiteIndex = HashMap::new();
    let mut continuation_resume_call_sites: ContinuationResumeCallSiteIndex =
        ContinuationResumeCallSiteIndex::new();
    let mut non_pure_continuation_resume_call_sites: NonPureContinuationResumeCallSiteIndex =
        NonPureContinuationResumeCallSiteIndex::new();
    let mut top_level_vars: crate::hir::TopLevelVarIndex = HashMap::new();
    let mut extern_globals: crate::hir::ExternGlobalIndex = HashMap::new();
    let mut top_level_immutable_values: crate::hir::TopLevelImmutableValueIndex = HashMap::new();
    let mut when_pat_binding_tys: crate::hir::WhenPatBindingTypeIndex = HashMap::new();

    for (source, file) in files_to_lower {
        let (
            file_hir,
            file_member_funs,
            file_ctor_call_sites,
            file_dispatch_call_sites,
            file_effect_op_call_sites,
            file_handle_payload_tuple_tys,
            file_with_update_contracts,
            file_assign_place_contracts,
            file_top_level_vars,
            file_extern_globals,
            file_top_level_immutable_values,
            file_when_pat_binding_tys,
        ) = {
            let mut ctx = HirLowering::new(
                source,
                file,
                index,
                &mut types,
                HirLoweringSetup {
                    typecheck_types: Some(typecheck_types),
                    type_kinds: &type_kinds,
                    delegated_properties: &delegated_properties,
                    compilation_unit,
                    default_arg_structs: default_arg_structs.clone(),
                    computed_property_getters: &computed_property_accessors.getters,
                    computed_property_setters: &computed_property_accessors.setters,
                    builtins,
                    generic_template_symbol_suffixes: &generic_template_symbol_suffixes,
                    known_receiver_subclasses: &known_receiver_subclasses,
                    class_vtables: &class_vtables,
                    interfaces: &interfaces,
                    class_itables: &class_itables,
                    materialize_direct_call_targets,
                    devirtualize_dispatch_calls,
                },
            );
            let file_hir = ctx.lower_file();
            if let Some(err) = ctx.take_stage_error() {
                return Err(err.into());
            }
            // 字面量已不再依赖“仅入口文件可切片”的旧路径，因此这里可以稳定收集所有文件的 member_funs。
            let pkg_prefix = package_prefix(source, file.package.as_ref());
            let file_member_funs = ctx.collect_member_funs(&pkg_prefix);
            if let Some(err) = ctx.take_stage_error() {
                return Err(err.into());
            }
            ctx.record_missing_assign_place_contracts_in_file(&file_hir);
            ctx.record_missing_assign_place_contracts_in_funs(&file_member_funs);
            let ctor_call_sites = std::mem::take(&mut ctx.ctor_call_sites);
            let dispatch_call_sites = std::mem::take(&mut ctx.dispatch_call_sites);
            let effect_op_call_sites = std::mem::take(&mut ctx.effect_op_call_sites);
            let handle_payload_tuple_tys = std::mem::take(&mut ctx.handle_payload_tuple_tys);
            let file_with_update_contracts = std::mem::take(&mut ctx.with_update_contracts);
            let file_assign_place_contracts = std::mem::take(&mut ctx.assign_place_contracts);
            let file_top_level_vars = std::mem::take(&mut ctx.top_level_vars);
            let file_extern_globals = std::mem::take(&mut ctx.extern_globals);
            let file_top_level_immutable_values =
                std::mem::take(&mut ctx.top_level_immutable_values);
            let file_when_pat_binding_tys = std::mem::take(&mut ctx.when_pat_binding_tys);
            (
                file_hir,
                file_member_funs,
                ctor_call_sites,
                dispatch_call_sites,
                effect_op_call_sites,
                handle_payload_tuple_tys,
                file_with_update_contracts,
                file_assign_place_contracts,
                file_top_level_vars,
                file_extern_globals,
                file_top_level_immutable_values,
                file_when_pat_binding_tys,
            )
        };

        ctor_call_sites.extend(file_ctor_call_sites);
        dispatch_call_sites.extend(file_dispatch_call_sites);
        effect_op_call_sites.extend(file_effect_op_call_sites);
        handle_payload_tuple_tys.extend(file_handle_payload_tuple_tys);
        with_update_contracts.extend(file_with_update_contracts);
        assign_place_contracts.extend(file_assign_place_contracts);
        continuation_resume_call_sites.extend(
            file.continuation_resume_call_sites()
                .into_iter()
                .map(|span| CallSite::new(source.path().to_path_buf(), span)),
        );
        non_pure_continuation_resume_call_sites.extend(
            file.non_pure_continuation_resume_call_sites()
                .into_iter()
                .map(|span| CallSite::new(source.path().to_path_buf(), span)),
        );

        top_level_vars.extend(file_top_level_vars);
        extern_globals.extend(file_extern_globals);
        top_level_immutable_values.extend(file_top_level_immutable_values);
        when_pat_binding_tys.extend(file_when_pat_binding_tys);
        member_funs.extend(file_member_funs);
        decls.extend(file_hir.decls);
        items.extend(file_hir.items);
    }

    // side tables：保持与 `lower_for_compilation_unit` 一致的收集逻辑（先降 HIR，再补充 layout/init/extern）。
    let extern_funs = files_to_lower
        .iter()
        .flat_map(|(source, file)| collect_extern_funs(source, file))
        .collect();
    let native_callable_funs = files_to_lower
        .iter()
        .flat_map(|(source, file)| collect_native_callable_funs(source, file))
        .collect();
    let extern_libs = collect_extern_libs(compilation_unit);
    let mut struct_layouts = collect_struct_layouts(compilation_unit, index, &mut types);
    let mut enum_layouts = collect_enum_layouts(compilation_unit, index, &mut types);
    // T0124：泛型 struct/enum 的具体实例化布局。
    struct_layouts.extend(collect_generic_struct_instantiation_layouts(
        compilation_unit,
        index,
        &mut types,
    ));
    enum_layouts.extend(collect_generic_enum_instantiation_layouts(
        compilation_unit,
        index,
        &mut types,
    ));

    let (
        object_inits,
        mut class_inits,
        side_table_ctor_call_sites,
        side_table_dispatch_call_sites,
        side_table_with_update_contracts,
        side_table_assign_place_contracts,
    ) = collect_compilation_unit_object_and_class_inits(
        compilation_unit,
        CompilationUnitInitCollectionInputs {
            index,
            type_kinds: &type_kinds,
            known_receiver_subclasses: &known_receiver_subclasses,
            class_vtables: &class_vtables,
            interfaces: &interfaces,
            class_itables: &class_itables,
            typecheck_types: Some(typecheck_types),
            materialize_direct_call_targets,
            devirtualize_dispatch_calls,
            builtins,
        },
        &mut types,
    )?;
    ctor_call_sites.extend(side_table_ctor_call_sites);
    dispatch_call_sites.extend(side_table_dispatch_call_sites);
    with_update_contracts.extend(side_table_with_update_contracts);
    assign_place_contracts.extend(side_table_assign_place_contracts);
    // T0125：泛型 class 的具体实例化 ClassInit（第一遍：处理文件中已有的泛型 class 实例化类型）。
    class_inits.extend(collect_generic_class_instantiation_inits(
        compilation_unit,
        &mut types,
        &class_inits,
    ));

    match instance_mode {
        CompilationUnitInstanceMode::DirectLoweredHir => {
            // T0127：为泛型独立函数的具体实例化生成单态化的 FunDecl。
            // 注意：必须在 class member monomorphization 之前运行，因为独立函数的单态化
            // 可能在 TypeStore 中创建新的泛型 class 实例化类型（例如 `Printer<Greeter>`），
            // 这些类型需要被后续的 class member monomorphization 发现。
            let monomorphized_funs =
                collect_generic_fun_instantiations(GenericFunInstantiationInputs {
                    compilation_unit,
                    monomorph_keys,
                    index,
                    type_kinds: &type_kinds,
                    types: &mut types,
                    builtins,
                    typecheck_types,
                    initial_items: &items,
                    initial_member_funs: &member_funs,
                    stable_cone_key: &stable_cone_key,
                });
            items.extend(monomorphized_funs.into_iter().map(Item::Fun));

            // T0130：第二遍 class 实例化 —— standalone fun monomorphization 可能在 TypeStore 中
            // 创建了新的泛型 class 实例化类型（例如 `Printer<Greeter>`），这里补充收集。
            class_inits.extend(collect_generic_class_instantiation_inits(
                compilation_unit,
                &mut types,
                &class_inits,
            ));

            // T0126：为泛型 class 的具体实例化生成单态化的成员方法 FunDecl。
            member_funs.extend(collect_generic_member_fun_instantiations(
                compilation_unit,
                index,
                &type_kinds,
                Some(typecheck_types),
                &mut types,
                builtins,
            ));
            struct_layouts.extend(collect_generic_struct_instantiation_layouts(
                compilation_unit,
                index,
                &mut types,
            ));
            enum_layouts.extend(collect_generic_enum_instantiation_layouts(
                compilation_unit,
                index,
                &mut types,
            ));
            class_inits.extend(collect_generic_class_instantiation_inits(
                compilation_unit,
                &mut types,
                &class_inits,
            ));
        }
        CompilationUnitInstanceMode::ExplicitMirInstances {
            instance_keys,
            instance_types,
        } => {
            let monomorphic_funs = collect_generic_fun_instantiations_from_instance_keys(
                ExplicitGenericFunInstantiationInputs {
                    compilation_unit,
                    instance_keys,
                    instance_types,
                    index,
                    type_kinds: &type_kinds,
                    types: &mut types,
                    builtins,
                    typecheck_types,
                    stable_cone_key: &stable_cone_key,
                },
            )?;
            items.extend(monomorphic_funs.into_iter().map(Item::Fun));

            class_inits.extend(collect_generic_class_instantiation_inits(
                compilation_unit,
                &mut types,
                &class_inits,
            ));

            member_funs.extend(
                collect_generic_member_fun_instantiations_from_instance_keys(
                    ExplicitGenericMemberInstantiationInputs {
                        compilation_unit,
                        instance_keys,
                        instance_types,
                        index,
                        type_kinds: &type_kinds,
                        types: &mut types,
                        builtins,
                        typecheck_types: Some(typecheck_types),
                        stable_cone_key: &stable_cone_key,
                    },
                )?,
            );
            let wanted_dispatch_member_fqns = class_itables
                .values()
                .flat_map(|entries| {
                    entries.iter().flat_map(|entry| {
                        entry
                            .method_impl_fqns
                            .iter()
                            .filter(|fqn| fqn.contains("::<"))
                            .cloned()
                    })
                })
                .collect::<HashSet<_>>();
            if !wanted_dispatch_member_fqns.is_empty() {
                let existing_member_fqns = member_funs
                    .iter()
                    .map(|fun| fun.fqn.clone())
                    .collect::<HashSet<_>>();
                member_funs.extend(
                    collect_generic_member_fun_instantiations(
                        compilation_unit,
                        index,
                        &type_kinds,
                        Some(typecheck_types),
                        &mut types,
                        builtins,
                    )
                    .into_iter()
                    .filter(|fun| wanted_dispatch_member_fqns.contains(&fun.fqn))
                    .filter(|fun| !existing_member_fqns.contains(&fun.fqn)),
                );
            }
            struct_layouts.extend(collect_generic_struct_instantiation_layouts(
                compilation_unit,
                index,
                &mut types,
            ));
            enum_layouts.extend(collect_generic_enum_instantiation_layouts(
                compilation_unit,
                index,
                &mut types,
            ));
            class_inits.extend(collect_generic_class_instantiation_inits(
                compilation_unit,
                &mut types,
                &class_inits,
            ));
        }
        CompilationUnitInstanceMode::GenericTemplateOnly => {}
    }

    let mut top_level_fun_call_sites = collect_top_level_fun_call_sites_with_type_remap(
        files_to_lower,
        Some(typecheck_types),
        &mut types,
    );
    let file_hir = File { decls, items };
    top_level_fun_call_sites.extend(collect_synthetic_named_intrinsic_call_sites_for_file(
        index,
        &file_hir,
        &member_funs,
    ));
    let call_arg_bindings = collect_call_arg_bindings(files_to_lower);
    let stable_type_param_keys = collect_stable_type_param_keys(compilation_unit, &stable_cone_key);
    let source_cones = source_cones_for_lowering(
        compilation_unit,
        index,
        &stable_cone_key,
        &source_cone_overrides,
    );

    Ok(LoweredHir {
        file: file_hir,
        stable_cone_key,
        source_cones,
        stable_type_param_keys,
        member_funs,
        materialized_mir: None,
        types,
        struct_layouts,
        enum_layouts,
        extern_funs,
        native_callable_funs,
        extern_globals,
        extern_libs,
        top_level_vars,
        top_level_immutable_values,
        top_level_fun_call_sites,
        call_arg_bindings,
        with_update_contracts,
        assign_place_contracts,
        object_inits,
        class_inits,
        class_vtables,
        interfaces,
        class_itables,
        ctor_call_sites,
        dispatch_call_sites,
        effect_op_call_sites,
        handle_payload_tuple_tys,
        continuation_resume_call_sites,
        non_pure_continuation_resume_call_sites,
        when_pat_binding_tys,
        nominal_kinds: type_kinds,
        nominal_variances,
        direct_supertypes,
        builtins,
    })
}
