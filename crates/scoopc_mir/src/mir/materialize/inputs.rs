//! Materialization request inputs and the helpers that gather them from typechecked sources or pre-interned MIR pass artifacts.

use super::*;

pub(super) struct PreparedDumpFile {
    pub(super) source: SourceFile,
    pub(super) ast: ast::File,
    pub(super) extend_type_env: bool,
    pub(super) collect_monomorph_keys: bool,
}

pub(super) struct DumpMaterializationInputs {
    pub(super) prepared_files: Vec<PreparedDumpFile>,
    pub(super) index: Index,
    pub(super) env: TypeEnv,
    pub(super) typecheck_types: TypeStore,
    pub(super) monomorph_requests: Vec<MonomorphRequest>,
}

pub(super) type SourceSiteKey = (PathBuf, Span);

#[derive(Clone)]
pub(super) struct RequestRootFunKey {
    pub(super) source_path: PathBuf,
    pub(super) fqn: String,
    pub(super) span: Span,
}

#[derive(Clone)]
pub(super) struct CallableBodyInfo {
    pub(super) request_lookup_key: RequestTemplateKey,
    pub(super) source_path: PathBuf,
    pub(super) fqn: String,
    pub(super) body_span: Span,
}

#[derive(Clone)]
pub(super) struct CallableSignatureParam {
    pub(super) name: String,
    pub(super) ty: TypeId,
}

#[derive(Clone)]
pub(super) struct CallableSignatureInfo {
    pub(super) template: TemplateKey,
    pub(super) stable_template_key: Option<StableTemplateKey>,
    pub(super) fun_ty: TypeId,
    pub(super) return_ty: TypeId,
    pub(super) params: Vec<CallableSignatureParam>,
    pub(super) has_generic_params_or_effect_param: bool,
}

pub(super) struct MaterializerConstructionInputs<'a> {
    pub(super) stable_cone_key: StableConeKey,
    pub(super) typecheck_types: &'a TypeStore,
    pub(super) template_infos: Vec<GenericTemplateInfo>,
    pub(super) callable_body_infos: Vec<CallableBodyInfo>,
    pub(super) callable_signatures: Vec<CallableSignatureInfo>,
    pub(super) callable_effects:
        Vec<scoopc_hir::hir_facts::source_sites::CallableSourceEffectFacts>,
    pub(super) call_site_instance_facts:
        Vec<scoopc_hir::hir_facts::source_sites::CallSiteInstanceFact>,
    pub(super) template_site_binding_facts:
        Vec<scoopc_hir::hir_facts::source_sites::TemplateSiteBindingFact>,
    pub(super) known_receiver_subclasses: super::super::KnownReceiverSubclassIndex,
    pub(super) direct_subclasses: HashMap<String, BTreeSet<String>>,
    pub(super) class_vtables: crate::vtable::ClassVtableIndex,
    pub(super) interfaces: crate::itable::InterfaceIndex,
    pub(super) class_itables: crate::itable::ClassItableIndex,
    pub(super) enum_layouts: crate::hir::EnumLayoutIndex,
    pub(super) extern_funs: crate::hir::ExternFunIndex,
    pub(super) native_callable_funs: crate::hir::NativeCallableFunIndex,
    pub(super) lowered_top_level_fun_call_bindings:
        HashMap<SourceSiteKey, ast::TopLevelFunCallBinding>,
    pub(super) ctor_call_sites: crate::hir::CtorCallSiteIndex,
    pub(super) top_level_vars: crate::hir::TopLevelVarIndex,
    pub(super) top_level_immutable_values: crate::hir::TopLevelImmutableValueIndex,
    pub(super) object_inits: crate::hir::ObjectInitIndex,
    pub(super) class_inits: crate::hir::ClassInitIndex,
    pub(super) member_value_tys: HashMap<String, MemberValueTypeInfo>,
    pub(super) request_sources: HashSet<PathBuf>,
    pub(super) request_root_mode: super::super::MaterializeRequestRootMode<'a>,
    pub(super) request_root_fun_keys: Vec<RequestRootFunKey>,
}

pub(super) struct MaterializeRequestSet<'a> {
    pub(super) monomorph_requests: &'a [MonomorphRequest],
    pub(super) hir_direct_instance_keys_by_fun: HashMap<(PathBuf, Span), Vec<InstanceKey>>,
    pub(super) construction_inputs: MaterializerConstructionInputs<'a>,
}

pub(super) fn collect_request_root_fun_keys(
    lowered_hir: &crate::hir::LoweredHir,
    request_source_paths: &[PathBuf],
    index: &Index,
    request_root_mode: super::super::MaterializeRequestRootMode<'_>,
) -> Vec<RequestRootFunKey> {
    let request_sources = request_source_paths
        .iter()
        .cloned()
        .collect::<HashSet<PathBuf>>();
    let mut out = Vec::new();

    match request_root_mode {
        super::super::MaterializeRequestRootMode::RequestSources => {
            for item in &lowered_hir.file.items {
                let crate::hir::Item::Fun(fun) = item else {
                    continue;
                };
                if request_sources.contains(&fun.source_path) {
                    out.push(RequestRootFunKey {
                        source_path: fun.source_path.clone(),
                        fqn: fun.fqn.clone(),
                        span: fun.span,
                    });
                }
            }

            for fun in &lowered_hir.member_funs {
                if request_sources.contains(&fun.source_path) {
                    out.push(RequestRootFunKey {
                        source_path: fun.source_path.clone(),
                        fqn: fun.fqn.clone(),
                        span: fun.span,
                    });
                }
            }
        }
        super::super::MaterializeRequestRootMode::EntryMain { fqn } => {
            for item in &lowered_hir.file.items {
                let crate::hir::Item::Fun(fun) = item else {
                    continue;
                };
                if !request_sources.contains(&fun.source_path) {
                    continue;
                }
                let is_entry_main = fqn.map_or(fun.name == "main", |entry| fun.fqn == entry);
                if is_entry_main || index.is_export_entry_point(&fun.fqn) {
                    out.push(RequestRootFunKey {
                        source_path: fun.source_path.clone(),
                        fqn: fun.fqn.clone(),
                        span: fun.span,
                    });
                }
            }

            for fun in &lowered_hir.member_funs {
                if request_sources.contains(&fun.source_path) {
                    out.push(RequestRootFunKey {
                        source_path: fun.source_path.clone(),
                        fqn: fun.fqn.clone(),
                        span: fun.span,
                    });
                }
            }
        }
    }

    // Native object symbols emitted for `@CallingConvention` bodies may be called
    // only from cone-local native objects, so ordinary source reachability cannot
    // discover them. Treat every annotated body as a materialization root.
    for item in &lowered_hir.file.items {
        let crate::hir::Item::Fun(fun) = item else {
            continue;
        };
        if !lowered_hir.native_callable_funs.contains_key(&fun.fqn) {
            continue;
        }
        if out
            .iter()
            .any(|root| root.source_path == fun.source_path && root.span == fun.span)
        {
            continue;
        }
        out.push(RequestRootFunKey {
            source_path: fun.source_path.clone(),
            fqn: fun.fqn.clone(),
            span: fun.span,
        });
    }

    for fun in &lowered_hir.member_funs {
        if !lowered_hir.native_callable_funs.contains_key(&fun.fqn) {
            continue;
        }
        if out
            .iter()
            .any(|root| root.source_path == fun.source_path && root.span == fun.span)
        {
            continue;
        }
        out.push(RequestRootFunKey {
            source_path: fun.source_path.clone(),
            fqn: fun.fqn.clone(),
            span: fun.span,
        });
    }

    out
}

pub(super) fn collect_direct_subclasses_from_supertypes(
    direct_supertypes: &crate::hir::DirectSupertypesIndex,
) -> HashMap<String, BTreeSet<String>> {
    let mut out = HashMap::<String, BTreeSet<String>>::new();
    for (child, supers) in direct_supertypes {
        for super_fqn in supers {
            out.entry(super_fqn.clone())
                .or_default()
                .insert(child.clone());
        }
    }
    out
}

pub(super) fn collect_callable_signature_infos(
    lowered_hir: &crate::hir::LoweredHir,
) -> Vec<CallableSignatureInfo> {
    lowered_hir
        .file
        .items
        .iter()
        .filter_map(|item| match item {
            crate::hir::Item::Fun(fun) => Some(fun),
            _ => None,
        })
        .chain(lowered_hir.member_funs.iter())
        .map(|fun| {
            let source_stable_cone_key = lowered_hir
                .source_cones
                .get(&fun.source_path)
                .map(|info| info.stable_key.clone())
                .unwrap_or_else(|| lowered_hir.stable_cone_key.clone());
            let mut type_param_names = Vec::new();
            for param in &fun.params {
                collect_type_param_names_in_type(
                    &lowered_hir.types,
                    param.ty,
                    &mut type_param_names,
                );
            }
            collect_type_param_names_in_type(
                &lowered_hir.types,
                fun.return_ty,
                &mut type_param_names,
            );
            let has_effect_param = function_type_has_effect_param(&lowered_hir.types, fun.ty);
            let stable_template_key = canonical_callable_signature_key(
                &lowered_hir.types,
                fun.ty,
                0,
                0,
                0,
                &NoTypeParamResolver,
            )
            .ok()
            .map(|signature_key| {
                StableTemplateKey::new(StableDefKey::new(
                    source_stable_cone_key,
                    StableDefNamespace::Fun,
                    &fun.fqn,
                    "non_generic_callable",
                    Some(signature_key),
                ))
            });
            CallableSignatureInfo {
                template: TemplateKey {
                    fqn: fun.fqn.clone(),
                    source_path: fun.source_path.clone(),
                    decl_span: fun.span,
                },
                stable_template_key,
                fun_ty: fun.ty,
                return_ty: fun.return_ty,
                params: fun
                    .params
                    .iter()
                    .map(|param| CallableSignatureParam {
                        name: param.name.clone(),
                        ty: param.ty,
                    })
                    .collect(),
                has_generic_params_or_effect_param: !type_param_names.is_empty()
                    || has_effect_param,
            }
        })
        .collect()
}

pub(super) fn collect_dump_materialization_inputs(
    session: &Session,
    source: &SourceFile,
) -> MaterializeResult<DumpMaterializationInputs> {
    let support_sources = load_dump_support_sources(session)?
        .into_iter()
        .filter(|source| {
            !source
                .path()
                .file_name()
                .is_some_and(|name| name == "core.scoop" || name == "lang_string.scoop")
        })
        .collect::<Vec<_>>();
    let mut prepared_files =
        Vec::with_capacity(session.sysroot().files.len() + support_sources.len() + 1);
    for file in session.sysroot().index_files() {
        if support_sources
            .iter()
            .any(|support_source| support_source.path() == file.source.path())
        {
            continue;
        }
        prepared_files.push(PreparedDumpFile {
            source: file.source.clone(),
            ast: file.ast.clone(),
            extend_type_env: false,
            collect_monomorph_keys: false,
        });
    }

    for support_source in support_sources {
        if support_source.path() == source.path() {
            continue;
        }
        let ast = parse_file(&support_source)?;
        prepared_files.push(PreparedDumpFile {
            source: support_source,
            ast,
            extend_type_env: true,
            collect_monomorph_keys: false,
        });
    }

    let entry_source = source.clone();
    let entry_ast = parse_file(&entry_source)?;
    prepared_files.push(PreparedDumpFile {
        source: entry_source,
        ast: entry_ast,
        extend_type_env: true,
        collect_monomorph_keys: true,
    });

    for file in &prepared_files {
        typecheck::check_file_headers(&file.source, &file.ast)?;
        typecheck::check_file_struct_decls(&file.source, &file.ast)?;
    }

    let index = {
        let mut pairs: Vec<(&SourceFile, &ast::File)> = Vec::with_capacity(prepared_files.len());
        for file in &prepared_files {
            pairs.push((&file.source, &file.ast));
        }
        Index::build(&pairs)?
    };

    let mut resolved_headers = Vec::with_capacity(prepared_files.len());
    for file in &prepared_files {
        resolved_headers.push(crate::resolve::check_file_headers(
            &file.source,
            &file.ast,
            &index,
        )?);
    }
    for (file, headers) in prepared_files.iter_mut().zip(resolved_headers.iter()) {
        crate::resolve::check_file_bodies(&file.source, &mut file.ast, &index, headers)?;
    }

    let mut env = TypeEnv::from_sysroot(session.sysroot(), &index)?;
    for file in &prepared_files {
        if file.extend_type_env {
            env.extend_from_file(&file.source, &file.ast, &index)?;
        }
    }

    let mut types = TypeStore::new();
    let builtins = types.intern_builtins();
    let mut monomorph_requests = Vec::new();
    for (file, headers) in prepared_files.iter().zip(resolved_headers.iter()) {
        typecheck::check_file_annotations(
            &file.source,
            &file.ast,
            &index,
            &headers.imports,
            &env,
            &mut types,
            builtins,
        )?;
        typecheck::check_file_type_refs(
            &file.source,
            &file.ast,
            &index,
            &headers.imports,
            &env,
            &mut types,
            builtins,
        )?;

        if file.collect_monomorph_keys {
            monomorph_requests.extend(typecheck::check_file_exprs_with_monomorph_requests(
                &file.source,
                &file.ast,
                &index,
                &headers.imports,
                &env,
                &mut types,
                builtins,
            )?);
        } else {
            typecheck::check_file_exprs(
                &file.source,
                &file.ast,
                &index,
                &headers.imports,
                &env,
                &mut types,
                builtins,
            )?;
        }
    }

    Ok(DumpMaterializationInputs {
        prepared_files,
        index,
        env,
        typecheck_types: types,
        monomorph_requests,
    })
}

#[cfg(test)]
pub(super) fn collect_site_instance_bindings(
    files_to_lower: &[(&SourceFile, &ast::File)],
) -> (
    HashMap<SourceSiteKey, ast::TopLevelFunValueRef>,
    HashMap<SourceSiteKey, ast::TopLevelFunCallBinding>,
) {
    let mut top_level_fun_value_refs = HashMap::new();
    let mut top_level_fun_call_bindings = HashMap::new();
    for (source, file) in files_to_lower {
        let source_path = source.path().to_path_buf();
        for (span, binding) in file.top_level_fun_value_refs() {
            top_level_fun_value_refs.insert((source_path.clone(), span), binding);
        }
        for (span, binding) in file.top_level_fun_call_bindings() {
            top_level_fun_call_bindings.insert((source_path.clone(), span), binding);
        }
    }
    (top_level_fun_value_refs, top_level_fun_call_bindings)
}

pub(super) fn collect_lowered_top_level_fun_call_bindings(
    lowered_hir: &crate::hir::LoweredHir,
) -> HashMap<SourceSiteKey, ast::TopLevelFunCallBinding> {
    lowered_hir
        .top_level_fun_call_sites
        .iter()
        .map(|(site, binding)| ((site.source_path.clone(), site.span), binding.clone()))
        .collect()
}

pub(super) type RequestTemplateKey = (String, PathBuf, Span);
