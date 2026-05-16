//! ValScope, dump-support AST loading, lower_for_dump / lower_typed_for_dump entry points.

#![allow(dead_code)]

use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ValScope {
    TopLevel,
    Local,
}

pub(crate) fn load_dump_support_asts(
    session: &Session,
    entry_source: &SourceFile,
) -> Result<Vec<(SourceFile, ast::File)>, HirLowerError> {
    let support_sources = crate::frontend::load_default_support_sources(session.options())
        .map_err(|err| HirLowerError::Frontend {
            message: format!("加载 dump support sources 失败：{err}"),
        })?;

    let mut out = Vec::new();
    for support_source in support_sources {
        if support_source.path() == entry_source.path() {
            continue;
        }
        if support_source
            .path()
            .file_name()
            .is_some_and(|name| name == "print.scoop")
        {
            continue;
        }
        if session
            .sysroot()
            .files
            .iter()
            .any(|file| file.source.path() == support_source.path())
        {
            continue;
        }
        let ast = parse_file(&support_source)?;
        out.push((support_source, ast));
    }
    Ok(out)
}

/// 为 `scoop dump-hir` 生成 HIR（最小实现）。
///
/// 流程：
/// 1) parse 源文件为 AST；
/// 2) 构建 sysroot + 当前文件的 `Index`；
/// 3) 运行 resolver（headers + bodies）把绑定结果写回 AST；
/// 4) 在一个新的 `TypeStore` 中 intern builtin types，收集 struct 布局信息，并把 AST 降为 HIR
///    （未覆盖节点用 `Any` 占位）。
pub fn lower_for_dump(session: &Session, source: &SourceFile) -> Result<LoweredHir, HirLowerError> {
    let mut ast = parse_file(source)?;
    let mut support_asts = load_dump_support_asts(session, source)?;
    {
        let support_sources = support_asts
            .iter()
            .map(|(source, _)| source.clone())
            .collect::<Vec<_>>();
        let mut sources = support_sources.iter().collect::<Vec<_>>();
        sources.push(source);
        let mut files = support_asts
            .iter_mut()
            .map(|(_, ast)| ast)
            .collect::<Vec<_>>();
        files.push(&mut ast);
        crate::comptime::trim_package_level_comptime_ifs_in_compilation_unit(
            session.sysroot(),
            &sources,
            &mut files,
        )?;
    }

    let index = {
        // 注意：`check_file_bodies` 需要 `&mut ast`，因此这里把构建 index 的临时借用放在独立作用域中，
        // 避免把 `&ast` 存到更长生命周期的容器里导致借用冲突。
        let mut pairs: Vec<(&SourceFile, &ast::File)> = Vec::new();
        for f in &session.sysroot().files {
            pairs.push((&f.source, &f.ast));
        }
        for (support_source, support_ast) in &support_asts {
            pairs.push((support_source, support_ast));
        }
        pairs.push((source, &ast));
        Index::build(&pairs)?
    };

    let mut support_headers = Vec::with_capacity(support_asts.len());
    for (support_source, support_ast) in &support_asts {
        support_headers.push(crate::resolve::check_file_headers(
            support_source,
            support_ast,
            &index,
        )?);
    }
    for ((support_source, support_ast), headers) in
        support_asts.iter_mut().zip(support_headers.iter())
    {
        crate::resolve::check_file_bodies(support_source, support_ast, &index, headers)?;
    }
    let headers = crate::resolve::check_file_headers(source, &ast, &index)?;
    crate::resolve::check_file_bodies(source, &mut ast, &index, &headers)?;

    let mut pairs: Vec<(&SourceFile, &ast::File)> = Vec::new();
    for f in &session.sysroot().files {
        pairs.push((&f.source, &f.ast));
    }
    for (support_source, support_ast) in &support_asts {
        pairs.push((support_source, support_ast));
    }
    pairs.push((source, &ast));
    let type_kinds = collect_type_decl_kinds(&pairs);
    let nominal_variances = collect_nominal_variances(&pairs);
    let direct_supertypes = collect_direct_supertypes(&pairs, &index);
    let known_receiver_subclasses =
        crate::devirtualize::collect_known_receiver_subclasses(&direct_supertypes);
    let delegated_properties = collect_delegated_properties(&pairs);
    let default_arg_structs = collect_default_arg_structs(&pairs);
    let computed_property_accessors = collect_computed_property_accessor_fqns(&pairs);
    let class_vtables = crate::vtable::collect_class_vtables(&pairs, &index)?;
    let (interfaces, class_itables) =
        crate::itable::collect_interfaces_and_class_itables(&pairs, &index, &class_vtables)?;
    let continuation_resume_call_sites = ast
        .continuation_resume_call_sites()
        .into_iter()
        .map(|span| CallSite::new(source.path().to_path_buf(), span))
        .collect();
    let non_pure_continuation_resume_call_sites = ast
        .non_pure_continuation_resume_call_sites()
        .into_iter()
        .map(|span| CallSite::new(source.path().to_path_buf(), span))
        .collect();

    let mut types = TypeStore::new();
    let builtins = types.intern_builtins();
    let stable_cone_key = StableConeKey::for_virtual_source_path(source.path());
    let generic_template_symbol_suffixes =
        util::collect_generic_template_symbol_suffixes_with_stable_cone_key(
            &stable_cone_key,
            &index,
            &pairs,
        );

    // 先降 HIR（保持 fixtures 中 `TypeId` 分配顺序稳定），再补充 struct 布局索引供后端使用。
    let pkg_prefix = package_prefix(source, ast.package.as_ref());
    let (
        file,
        member_funs,
        mut ctor_call_sites,
        mut dispatch_call_sites,
        effect_op_call_sites,
        handle_payload_tuple_tys,
        mut with_update_contracts,
        mut assign_place_contracts,
        top_level_vars,
        extern_globals,
        top_level_consts,
        top_level_immutable_values,
        when_pat_binding_tys,
    ) = {
        let mut ctx = HirLowering::new(
            source,
            &ast,
            &index,
            &mut types,
            HirLoweringSetup {
                typecheck_types: None,
                type_kinds: &type_kinds,
                delegated_properties: &delegated_properties,
                compilation_unit: &pairs,
                default_arg_structs: default_arg_structs.clone(),
                computed_property_getters: &computed_property_accessors.getters,
                computed_property_setters: &computed_property_accessors.setters,
                builtins,
                generic_template_symbol_suffixes: &generic_template_symbol_suffixes,
                known_receiver_subclasses: &known_receiver_subclasses,
                class_vtables: &class_vtables,
                interfaces: &interfaces,
                class_itables: &class_itables,
                materialize_direct_call_targets: false,
                devirtualize_dispatch_calls: false,
                runtime_comptime_plan: None,
            },
        );
        let file = ctx.lower_file();
        if let Some(err) = ctx.take_stage_error() {
            return Err(err.into());
        }
        let member_funs = ctx.collect_member_funs(&pkg_prefix);
        if let Some(err) = ctx.take_stage_error() {
            return Err(err.into());
        }
        ctx.record_missing_assign_place_contracts_in_file(&file);
        ctx.record_missing_assign_place_contracts_in_funs(&member_funs);
        let ctor_call_sites = std::mem::take(&mut ctx.ctor_call_sites);
        let dispatch_call_sites = std::mem::take(&mut ctx.dispatch_call_sites);
        let effect_op_call_sites = std::mem::take(&mut ctx.effect_op_call_sites);
        let handle_payload_tuple_tys = std::mem::take(&mut ctx.handle_payload_tuple_tys);
        let with_update_contracts = std::mem::take(&mut ctx.with_update_contracts);
        let assign_place_contracts = std::mem::take(&mut ctx.assign_place_contracts);
        let top_level_vars = std::mem::take(&mut ctx.top_level_vars);
        let extern_globals = std::mem::take(&mut ctx.extern_globals);
        let top_level_consts = std::mem::take(&mut ctx.top_level_consts);
        let top_level_immutable_values = std::mem::take(&mut ctx.top_level_immutable_values);
        let when_pat_binding_tys = std::mem::take(&mut ctx.when_pat_binding_tys);
        (
            file,
            member_funs,
            ctor_call_sites,
            dispatch_call_sites,
            effect_op_call_sites,
            handle_payload_tuple_tys,
            with_update_contracts,
            assign_place_contracts,
            top_level_vars,
            extern_globals,
            top_level_consts,
            top_level_immutable_values,
            when_pat_binding_tys,
        )
    };

    // T4016T2：sysroot/task.scoop 这类“实现文件依赖同编译单元里的声明元数据”的路径，
    // 需要从整个 compilation unit 收集 object/class side tables，而不是只看当前 lowering 的文件。
    let (
        object_inits,
        class_inits,
        side_table_ctor_call_sites,
        side_table_dispatch_call_sites,
        side_table_with_update_contracts,
        side_table_assign_place_contracts,
    ) = collect_compilation_unit_object_and_class_inits(
        &pairs,
        CompilationUnitInitCollectionInputs {
            index: &index,
            type_kinds: &type_kinds,
            known_receiver_subclasses: &known_receiver_subclasses,
            class_vtables: &class_vtables,
            interfaces: &interfaces,
            class_itables: &class_itables,
            typecheck_types: None,
            materialize_direct_call_targets: false,
            devirtualize_dispatch_calls: false,
            builtins,
        },
        &mut types,
    )?;
    ctor_call_sites.extend(side_table_ctor_call_sites);
    dispatch_call_sites.extend(side_table_dispatch_call_sites);
    with_update_contracts.extend(side_table_with_update_contracts);
    assign_place_contracts.extend(side_table_assign_place_contracts);

    // T1006：收集 `@Extern` 外部函数的符号名与 ABI（side table；不影响 dump-hir 输出）。
    let extern_funs = collect_extern_funs(source, &ast);
    let extern_libs = collect_extern_libs(&pairs);

    // T0811：早期 LLVM codegen 需要知道 struct 的字段顺序与字段类型，用于生成字段 GEP 索引。
    let mut struct_layouts = collect_struct_layouts(&pairs, &index, &mut types);
    // T0813：早期 LLVM codegen 需要知道 enum 的 variant tag 与 payload 字段类型，用于生成判别与解构。
    let mut enum_layouts = collect_enum_layouts(&pairs, &index, &mut types);
    // T0124：泛型 struct/enum 的具体实例化布局。
    struct_layouts.extend(collect_generic_struct_instantiation_layouts(
        &pairs, &index, &mut types,
    ));
    enum_layouts.extend(collect_generic_enum_instantiation_layouts(
        &pairs, &index, &mut types,
    ));
    // T0125：泛型 class 的具体实例化 ClassInit。
    let class_inits = {
        let mut ci = class_inits;
        ci.extend(collect_generic_class_instantiation_inits(
            &pairs, &mut types, &ci,
        ));
        ci
    };
    // T0126：为泛型 class 的具体实例化生成单态化的成员方法 FunDecl。
    let monomorphized_member_funs = collect_generic_member_fun_instantiations(
        &pairs,
        &index,
        &type_kinds,
        None,
        &mut types,
        builtins,
    );
    let mut member_funs = member_funs;
    member_funs.extend(monomorphized_member_funs);
    struct_layouts.extend(collect_generic_struct_instantiation_layouts(
        &pairs, &index, &mut types,
    ));
    enum_layouts.extend(collect_generic_enum_instantiation_layouts(
        &pairs, &index, &mut types,
    ));
    let class_inits = {
        let mut ci = class_inits;
        ci.extend(collect_generic_class_instantiation_inits(
            &pairs, &mut types, &ci,
        ));
        ci
    };
    let mut top_level_fun_call_sites = collect_top_level_fun_call_sites(&[(source, &ast)]);
    top_level_fun_call_sites.extend(collect_synthetic_named_intrinsic_call_sites(
        &index,
        &member_funs,
    ));
    let call_arg_bindings = collect_call_arg_bindings(&[(source, &ast)]);
    let stable_type_param_keys =
        collect_stable_type_param_keys(&[(source, &ast)], &stable_cone_key);
    Ok(LoweredHir {
        file,
        stable_cone_key,
        stable_type_param_keys,
        member_funs,
        materialized_mir: None,
        types,
        struct_layouts,
        enum_layouts,
        extern_funs,
        extern_globals,
        extern_libs,
        top_level_vars,
        top_level_consts,
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

/// 为需要 typed HIR 事实的调试入口生成 HIR。
///
/// 与 `lower_for_dump` 的区别：
/// - 这里会运行 typecheck（annotations / type refs / exprs）并消费 AST side tables；
/// - 用于 `dump-mir` / MIR fixtures 这类需要 `Continuation.resume`、late-bound member
///   resolution、effect payload binding 等 typed 事实的路径；
/// - 但仍停留在 generic HIR/MIR template 边界：不会在这里额外 materialize
///   standalone generic fun 或 owner-specialized member fun 的 `::<...>` 实例。
pub fn lower_typed_for_dump(
    session: &Session,
    source: &SourceFile,
) -> Result<LoweredHir, HirLowerError> {
    let mut ast = parse_file(source)?;
    let mut support_asts = load_dump_support_asts(session, source)?;
    {
        let support_sources = support_asts
            .iter()
            .map(|(source, _)| source.clone())
            .collect::<Vec<_>>();
        let mut sources = support_sources.iter().collect::<Vec<_>>();
        sources.push(source);
        let mut files = support_asts
            .iter_mut()
            .map(|(_, ast)| ast)
            .collect::<Vec<_>>();
        files.push(&mut ast);
        crate::comptime::trim_package_level_comptime_ifs_in_compilation_unit(
            session.sysroot(),
            &sources,
            &mut files,
        )?;
    }

    for (support_source, support_ast) in &support_asts {
        crate::typecheck::check_file_headers(support_source, support_ast)?;
        crate::typecheck::check_file_struct_decls(support_source, support_ast)?;
    }
    crate::typecheck::check_file_headers(source, &ast)?;
    crate::typecheck::check_file_struct_decls(source, &ast)?;

    let index = {
        let mut compilation_unit: Vec<(&SourceFile, &ast::File)> = Vec::new();
        for file in &session.sysroot().files {
            compilation_unit.push((&file.source, &file.ast));
        }
        for (support_source, support_ast) in &support_asts {
            compilation_unit.push((support_source, support_ast));
        }
        compilation_unit.push((source, &ast));
        Index::build(&compilation_unit)?
    };

    let mut support_headers = Vec::with_capacity(support_asts.len());
    for (support_source, support_ast) in &support_asts {
        support_headers.push(crate::resolve::check_file_headers(
            support_source,
            support_ast,
            &index,
        )?);
    }
    for ((support_source, support_ast), headers) in
        support_asts.iter_mut().zip(support_headers.iter())
    {
        crate::resolve::check_file_bodies(support_source, support_ast, &index, headers)?;
    }
    let headers = crate::resolve::check_file_headers(source, &ast, &index)?;
    crate::resolve::check_file_bodies(source, &mut ast, &index, &headers)?;

    let mut env = crate::typecheck::TypeEnv::from_sysroot(session.sysroot(), &index)?;
    for (support_source, support_ast) in &support_asts {
        env.extend_from_file(support_source, support_ast, &index)?;
    }
    env.extend_from_file(source, &ast, &index)?;

    let mut typecheck_types = TypeStore::new();
    let builtins = typecheck_types.intern_builtins();
    crate::typecheck::check_file_annotations(
        source,
        &ast,
        &index,
        &headers.imports,
        &env,
        &mut typecheck_types,
        builtins,
    )?;
    crate::typecheck::check_file_properties(source, &ast, &index, &env)?;
    crate::typecheck::check_file_type_refs(
        source,
        &ast,
        &index,
        &headers.imports,
        &env,
        &mut typecheck_types,
        builtins,
    )?;
    crate::typecheck::check_file_exprs(
        source,
        &ast,
        &index,
        &headers.imports,
        &env,
        &mut typecheck_types,
        builtins,
    )?;

    let mut compilation_unit: Vec<(&SourceFile, &ast::File)> = Vec::new();
    for file in &session.sysroot().files {
        compilation_unit.push((&file.source, &file.ast));
    }
    for (support_source, support_ast) in &support_asts {
        compilation_unit.push((support_source, support_ast));
    }
    compilation_unit.push((source, &ast));
    let files_to_lower = [(source, &ast)];
    let runtime_comptime_plan = crate::comptime::plan_runtime_comptime_in_file(
        source,
        &ast,
        &compilation_unit,
        &typecheck_types,
    )?;
    let mut runtime_comptime_plans = HashMap::new();
    if !runtime_comptime_plan.is_empty() {
        runtime_comptime_plans.insert(source.path().to_path_buf(), runtime_comptime_plan);
    }

    lower_for_compilation_unit_multi_files_internal(
        &index,
        &compilation_unit,
        &files_to_lower,
        &[],
        Some(&env),
        &typecheck_types,
        CompilationUnitLoweringOptions::generic_template_only(
            StableConeKey::for_virtual_source_path(source.path()),
        )
        .with_runtime_comptime_plans(&runtime_comptime_plans),
    )
}
