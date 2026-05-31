//! Public materialization entry points used by callers (dump-ir path, typechecked-input path).

use super::*;

/// 为 `dump-ir` / tests 生成 monomorphic MIR instances。
pub fn materialize_for_dump(
    session: &Session,
    source: &SourceFile,
) -> MaterializeResult<MaterializedMir> {
    materialize_for_dump_with_opt_level(session, source, OptLevel::O2)
}

/// 为 `dump-ir` / tests 生成 monomorphic MIR instances，并显式指定 MIR pass 优化等级。
pub fn materialize_for_dump_with_opt_level(
    session: &Session,
    source: &SourceFile,
    opt_level: OptLevel,
) -> MaterializeResult<MaterializedMir> {
    let DumpMaterializationInputs {
        prepared_files,
        index,
        env,
        typecheck_types,
        monomorph_requests,
    } = collect_dump_materialization_inputs(session, source)?;
    let compilation_unit = prepared_files
        .iter()
        .map(|file| (&file.source, &file.ast))
        .collect::<Vec<_>>();
    super::super::materialize_compilation_unit_from_typechecked_inputs_with_opt_level(
        &compilation_unit,
        &[source.path().to_path_buf()],
        &index,
        Some(&env),
        &typecheck_types,
        &monomorph_requests,
        opt_level,
    )
}

/// 基于既有 typechecked compilation-unit facts 执行 generic MIR template -> monomorphic
/// instance materialization。
///
/// 说明：
/// - 该入口直接复用调用方已经准备好的 `Index` / `TypeEnv` / `TypeStore` /
///   `MonomorphRequest` 与 AST side tables，不重新跑 parse/resolve/typecheck；
/// - dump/debug 路径目前通过它做包装，后续 build/frontend 主路径也将复用同一层。
pub(crate) fn materialize_compilation_unit_from_typechecked_inputs(
    compilation_unit: &[(&SourceFile, &ast::File)],
    index: &Index,
    type_env: Option<&TypeEnv>,
    typecheck_types: &TypeStore,
    monomorph_requests: &[MonomorphRequest],
    options: super::super::MaterializeCompilationUnitOptions<'_>,
) -> MaterializeResult<MaterializedMir> {
    let super::super::MaterializeCompilationUnitOptions {
        stable_cone_key,
        source_cones,
        request_source_paths,
        request_root_mode,
        opt_level,
    } = options;
    let template_catalog = collect_generic_template_infos_with_source_cones(
        &stable_cone_key,
        source_cones,
        index,
        compilation_unit,
    );
    let monomorph_requests =
        stabilize_monomorph_requests(typecheck_types, monomorph_requests, &template_catalog)?;
    let callable_body_infos = collect_callable_body_infos(compilation_unit);
    // materialized callee 可能定义在 helper/sysroot 等“非请求源文件”中，因此 generic
    // template lowering 与 site binding 收集都必须覆盖完整 compilation unit；调用方只需通过
    // `monomorph_requests` 决定初始请求种子，而不是把 template 提供者排除在外。
    let (top_level_fun_value_refs, top_level_fun_call_bindings) =
        collect_site_instance_bindings(compilation_unit);
    let mut lowered_hir = crate::hir::lower_generic_for_compilation_unit_multi_files_with_type_env(
        stable_cone_key.clone(),
        index,
        compilation_unit,
        compilation_unit,
        type_env,
        typecheck_types,
    )?;
    lowered_hir.source_cones.extend(
        source_cones
            .iter()
            .map(|(path, info)| (path.clone(), info.clone())),
    );
    let request_root_fun_keys =
        collect_request_root_fun_keys(&lowered_hir, request_source_paths, index, request_root_mode);
    let request_sources = request_source_paths.iter().cloned().collect::<HashSet<_>>();
    let callable_signatures = collect_callable_signature_infos(&lowered_hir);
    let member_value_tys = collect_member_value_type_infos_from_hir_decls(&lowered_hir.file.decls);
    let lowered_top_level_fun_call_bindings =
        collect_lowered_top_level_fun_call_bindings(&lowered_hir);
    let ctor_call_sites = lowered_hir.ctor_call_sites.clone();
    let top_level_vars = lowered_hir.top_level_vars.clone();
    let top_level_immutable_values = lowered_hir.top_level_immutable_values.clone();
    let object_inits = lowered_hir.object_inits.clone();
    let class_inits = lowered_hir.class_inits.clone();
    let hir_direct_instance_keys_by_fun = collect_hir_direct_call_instance_requests(
        &mut lowered_hir,
        typecheck_types,
        &top_level_fun_call_bindings,
    );
    let known_receiver_subclasses =
        super::super::collect_known_receiver_subclasses(&lowered_hir.direct_supertypes);
    let direct_subclasses =
        collect_direct_subclasses_from_supertypes(&lowered_hir.direct_supertypes);
    let class_vtables = lowered_hir.class_vtables.clone();
    let interfaces = lowered_hir.interfaces.clone();
    let class_itables = lowered_hir.class_itables.clone();
    let enum_layouts = lowered_hir.enum_layouts.clone();
    let extern_funs = lowered_hir.extern_funs.clone();
    let native_callable_funs = lowered_hir.native_callable_funs.clone();
    let builtins = lowered_hir.types.intern_builtins();
    let default_contract_source_path = request_source_paths
        .first()
        .map(PathBuf::as_path)
        .or_else(|| compilation_unit.first().map(|(source, _)| source.path()))
        .ok_or_else(|| {
            materialize_err(MirMaterializeError::Frontend {
                message: "materialize compilation unit must contain at least one source file"
                    .to_string(),
            })
        })?;
    let hir_facts = scoopc_hir::stage::build_hir_facts_from_lowered_hir(
        &lowered_hir,
        default_contract_source_path,
    )
    .map_err(crate::hir::HirLowerError::from)?;
    let facts = super::super::MirLoweringFacts::from_hir_facts(&lowered_hir, &hir_facts);
    let generic_file = super::super::lower_hir_file_for_dump_with_facts(
        builtins,
        &mut lowered_hir.types,
        &lowered_hir.file,
        &lowered_hir.member_funs,
        &facts,
    );
    let types = lowered_hir.types;

    materialize_generic_mir(
        generic_file,
        types,
        builtins,
        MaterializeRequestSet {
            monomorph_requests: &monomorph_requests,
            hir_direct_instance_keys_by_fun,
            construction_inputs: MaterializerConstructionInputs {
                stable_cone_key,
                typecheck_types,
                template_infos: template_catalog,
                callable_body_infos,
                callable_signatures,
                known_receiver_subclasses,
                direct_subclasses,
                class_vtables,
                interfaces,
                class_itables,
                enum_layouts,
                extern_funs,
                native_callable_funs,
                top_level_fun_value_refs,
                top_level_fun_call_bindings,
                lowered_top_level_fun_call_bindings,
                ctor_call_sites,
                top_level_vars,
                top_level_immutable_values,
                object_inits,
                class_inits,
                member_value_tys,
                request_sources,
                request_root_mode,
                request_root_fun_keys,
            },
        },
        opt_level,
    )
}

pub(super) fn stabilize_monomorph_requests(
    typecheck_types: &TypeStore,
    monomorph_requests: &[MonomorphRequest],
    template_catalog: &[GenericTemplateInfo],
) -> MaterializeResult<Vec<MonomorphRequest>> {
    let templates_by_request = template_catalog
        .iter()
        .map(|info| {
            (
                info.request_lookup_key.clone(),
                info.stable_template_key.clone(),
            )
        })
        .collect::<HashMap<_, _>>();
    monomorph_requests
        .iter()
        .map(|request| {
            let key = &request.key;
            if key.type_args.is_empty() && key.eff_args.is_empty() {
                return Ok(request.clone());
            }
            if !instance_request_is_concrete(typecheck_types, &key.type_args, &key.eff_args) {
                return Ok(request.clone());
            }
            let lookup_key = (
                key.symbol.fqn.clone(),
                key.symbol.decl_file.clone(),
                key.symbol.decl_span,
            );
            let Some(stable_template_key) = templates_by_request.get(&lookup_key).cloned() else {
                return Err(materialize_err(
                    MirMaterializeError::MissingGenericTemplate {
                        fqn: key.symbol.fqn.clone(),
                        file: key.symbol.decl_file.display().to_string(),
                        span: key.symbol.decl_span,
                        call_file: Some(request.request_source_path.display().to_string()),
                        call_site: Some(request.call_span),
                    },
                ));
            };
            let stable_instance_key = StableInstanceKey::from_type_arguments(
                stable_template_key.clone(),
                typecheck_types,
                &key.type_args,
                &key.eff_args,
                &NoTypeParamResolver,
            )
            .map_err(|err| {
                frontend_err(format!(
                    "无法为 monomorph request `{}` 构造 stable instance key: {err}",
                    key.symbol.fqn
                ))
            })?;
            let mut request = request.clone();
            request.key = request
                .key
                .with_stable_identity(stable_template_key, stable_instance_key);
            Ok(request)
        })
        .collect()
}
