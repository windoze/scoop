use super::*;
use crate::mir::{
    BasicBlock, ClassCtorCallMetadata, CtorMetadata, LocalSourceKind, MirLoweringFacts,
    MirTransportKind, RuntimeTypeDescriptorKind, RuntimeTypeStaticFold, SiteId,
    StoredContinuationRoutePublication, lower_hir_file_for_dump_with_facts,
};
use crate::session::Session;
use crate::source::SourceFile;
use crate::ty::{NominalType, RefTypeKind, TypeKind, TypeParamType};
use scoopc_mir_facts::pipeline::MirPassKind;

const SYNTHETIC_STATEMENT_TODO_REASON: &str = "synthetic statement todo";

/// 构造“完整编译单元 facts + 仅部分文件贡献实例请求”的最小测试输入。
fn prepare_typechecked_compilation_unit_inputs(
    session: &Session,
    files: Vec<SourceFile>,
    request_file_indices: &[usize],
) -> (
    Vec<(SourceFile, ast::File)>,
    Index,
    TypeEnv,
    TypeStore,
    Vec<MonomorphRequest>,
) {
    let mut files = files
        .into_iter()
        .map(|source| {
            let ast = parse_file(&source).unwrap();
            (source, ast)
        })
        .collect::<Vec<_>>();

    for (source, ast) in &files {
        typecheck::check_file_headers(source, ast).unwrap();
        typecheck::check_file_struct_decls(source, ast).unwrap();
    }

    let index = {
        let mut unit: Vec<(&SourceFile, &ast::File)> =
            Vec::with_capacity(session.sysroot().files.len() + files.len());
        for file in session.sysroot().index_files() {
            unit.push((&file.source, &file.ast));
        }
        for (source, ast) in &files {
            unit.push((source, ast));
        }
        Index::build(&unit).unwrap()
    };

    let mut resolved_headers = Vec::with_capacity(files.len());
    for (source, ast) in &files {
        resolved_headers.push(crate::resolve::check_file_headers(source, ast, &index).unwrap());
    }
    for ((source, ast), headers) in files.iter_mut().zip(resolved_headers.iter()) {
        crate::resolve::check_file_bodies(source, ast, &index, headers).unwrap();
    }

    let mut env = TypeEnv::from_sysroot(session.sysroot(), &index).unwrap();
    for (source, ast) in &files {
        env.extend_from_file(source, ast, &index).unwrap();
    }

    let mut types = TypeStore::new();
    let builtins = types.intern_builtins();
    let mut monomorph_requests = Vec::new();
    for (file_index, ((source, ast), headers)) in
        files.iter().zip(resolved_headers.iter()).enumerate()
    {
        typecheck::check_file_annotations(
            source,
            ast,
            &index,
            &headers.imports,
            &env,
            &mut types,
            builtins,
        )
        .unwrap();
        typecheck::check_file_type_refs(
            source,
            ast,
            &index,
            &headers.imports,
            &env,
            &mut types,
            builtins,
        )
        .unwrap();

        if request_file_indices.contains(&file_index) {
            monomorph_requests.extend(
                typecheck::check_file_exprs_with_monomorph_requests(
                    source,
                    ast,
                    &index,
                    &headers.imports,
                    &env,
                    &mut types,
                    builtins,
                )
                .unwrap(),
            );
        } else {
            typecheck::check_file_exprs(
                source,
                ast,
                &index,
                &headers.imports,
                &env,
                &mut types,
                builtins,
            )
            .unwrap();
        }
    }

    (files, index, env, types, monomorph_requests)
}

fn test_span() -> Span {
    Span::new(10, 20)
}

fn test_source_path() -> PathBuf {
    PathBuf::from("<mem>/materialized_mir.scoop")
}

fn mir_fixture(name: &str) -> SourceFile {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/mir_lowered")
        .join(name);
    SourceFile::load(&path)
        .unwrap_or_else(|error| panic!("failed to load MIR fixture {}: {error}", path.display()))
}

fn type_arg_names(materialized: &MaterializedMir, key: &InstanceKey) -> Vec<String> {
    key.type_args
        .iter()
        .map(|&ty| materialized.types.display(ty).to_string())
        .collect()
}

fn effect_arg_names(materialized: &MaterializedMir, key: &InstanceKey) -> Vec<String> {
    key.eff_args
        .iter()
        .map(|row| {
            if row.is_pure() {
                "Pure".to_string()
            } else {
                row.terms
                    .iter()
                    .map(|&ty| materialized.types.display(ty).to_string())
                    .collect::<Vec<_>>()
                    .join(" + ")
            }
        })
        .collect()
}

fn direct_call_fqns(fun: &FunDecl) -> Vec<String> {
    let Some(body) = &fun.body else {
        return Vec::new();
    };
    body.blocks
        .iter()
        .flat_map(|block| block.stmts.iter())
        .filter_map(|stmt| match &stmt.kind {
            StatementKind::Assign {
                value:
                    Rvalue::Call {
                        kind: CallKind::Direct { callee_fqn },
                        ..
                    },
                ..
            } => Some(callee_fqn.clone()),
            _ => None,
        })
        .collect()
}

fn has_class_ctor_for_type(
    materialized: &MaterializedMir,
    fun: &FunDecl,
    expected_ty: &str,
) -> bool {
    let Some(body) = &fun.body else {
        return false;
    };
    body.blocks.iter().any(|block| {
        block.stmts.iter().any(|stmt| {
            let StatementKind::Assign { target, value } = &stmt.kind else {
                return false;
            };
            let Rvalue::ClassCtor { class_fqn, .. } = value else {
                return false;
            };
            class_fqn == "mir_lowered.generic_materialization.Holder"
                && body
                    .locals
                    .get(target.as_u32() as usize)
                    .is_some_and(|local| {
                        materialized.types.display(local.ty).to_string() == expected_ty
                    })
        })
    })
}

fn unit_return_body() -> Body {
    let mut body = Body::new_empty();
    let bb = body.push_block(BasicBlock {
        is_cleanup: false,
        stmts: Vec::new(),
        terminator: Terminator {
            span: test_span(),
            kind: TerminatorKind::Return { value: None },
            unwind: UnwindAction::NoUnwind,
        },
    });
    body.start = bb;
    body
}

fn body_with_statement_todo() -> Body {
    let mut body = unit_return_body();
    body.blocks[0].stmts.push(Statement {
        span: test_span(),
        kind: StatementKind::Todo(SYNTHETIC_STATEMENT_TODO_REASON.to_string()),
    });
    body
}

fn body_with_rvalue_todo(unit_ty: TypeId) -> Body {
    let mut body = Body::new_empty();
    let local = body.push_local(LocalDecl {
        span: test_span(),
        name: Some("tmp".to_string()),
        ty: unit_ty,
        source: LocalSourceKind::CompilerTemporary,
    });
    let bb = body.push_block(BasicBlock {
        is_cleanup: false,
        stmts: vec![Statement {
            span: test_span(),
            kind: StatementKind::Assign {
                target: local,
                value: Rvalue::Todo("missing expr".to_string()),
            },
        }],
        terminator: Terminator {
            span: test_span(),
            kind: TerminatorKind::Return { value: None },
            unwind: UnwindAction::NoUnwind,
        },
    });
    body.start = bb;
    body
}

fn body_with_terminator_todo() -> Body {
    let mut body = Body::new_empty();
    let bb = body.push_block(BasicBlock {
        is_cleanup: false,
        stmts: Vec::new(),
        terminator: Terminator {
            span: test_span(),
            kind: TerminatorKind::Todo("unterminated".to_string()),
            unwind: UnwindAction::NoUnwind,
        },
    });
    body.start = bb;
    body
}

fn body_with_unwind_todo() -> Body {
    let mut body = Body::new_empty();
    let bb = body.push_block(BasicBlock {
        is_cleanup: false,
        stmts: Vec::new(),
        terminator: Terminator {
            span: test_span(),
            kind: TerminatorKind::Return { value: None },
            unwind: UnwindAction::Todo("perform unwind pending".to_string()),
        },
    });
    body.start = bb;
    body
}

fn generic_template_key_with_source_path(source_path: PathBuf) -> TemplateKey {
    TemplateKey {
        fqn: "fixtures.materialize.id".to_string(),
        source_path,
        decl_span: test_span(),
    }
}

fn generic_template_key() -> TemplateKey {
    generic_template_key_with_source_path(test_source_path())
}

fn test_stable_cone_key() -> StableConeKey {
    StableConeKey::new("fixtures-materialize", "0.1.0")
}

fn test_stable_template_key(template: &TemplateKey, signature_key: &str) -> StableTemplateKey {
    stable_template_key_for_template(
        &test_stable_cone_key(),
        &template.fqn,
        StableDefNamespace::Fun,
        "generic_fun",
        signature_key,
    )
}

fn generic_materializer_for_body_with_template(
    body: Body,
    eff_param_name: Option<String>,
    template: TemplateKey,
) -> (MirInstanceMaterializer, InstanceKey) {
    let mut types = TypeStore::new();
    let builtins = types.intern_builtins();
    let fun = FunDecl {
        span: template.decl_span,
        fqn: template.fqn.clone(),
        name: "id".to_string(),
        ty: builtins.unit,
        params: Vec::new(),
        return_ty: builtins.unit,
        body: Some(body),
    };
    let typecheck_types = TypeStore::new();
    let materializer = MirInstanceMaterializer::new(
        File {
            items: vec![Item::Fun(fun)],
        },
        types,
        builtins,
        MaterializerConstructionInputs {
            stable_cone_key: test_stable_cone_key(),
            typecheck_types: &typecheck_types,
            template_infos: vec![GenericTemplateInfo {
                request_lookup_key: (
                    template.fqn.clone(),
                    template.source_path.clone(),
                    template.decl_span,
                ),
                template: template.clone(),
                stable_template_key: test_stable_template_key(&template, "fun||id||Unit"),
                type_param_names: Vec::new(),
                eff_param_name: eff_param_name.clone(),
                signature_key: "fun||id||Unit".to_string(),
                has_body: true,
            }],
            callable_body_infos: Vec::new(),
            callable_signatures: vec![CallableSignatureInfo {
                template: template.clone(),
                stable_template_key: Some(test_stable_template_key(&template, "fun||id||Unit")),
                fun_ty: builtins.unit,
                return_ty: builtins.unit,
                params: Vec::new(),
                has_generic_params_or_effect_param: false,
            }],
            known_receiver_subclasses: HashSet::new(),
            direct_subclasses: HashMap::new(),
            class_vtables: HashMap::new(),
            interfaces: HashMap::new(),
            class_itables: HashMap::new(),
            enum_layouts: HashMap::new(),
            extern_funs: HashMap::new(),
            native_callable_funs: HashMap::new(),
            top_level_fun_value_refs: HashMap::new(),
            top_level_fun_call_bindings: HashMap::new(),
            lowered_top_level_fun_call_bindings: HashMap::new(),
            ctor_call_sites: HashMap::new(),
            top_level_vars: HashMap::new(),
            top_level_immutable_values: HashMap::new(),
            object_inits: HashMap::new(),
            class_inits: HashMap::new(),
            member_value_tys: HashMap::new(),
            request_sources: HashSet::new(),
            request_root_mode: crate::mir::MaterializeRequestRootMode::RequestSources,
            request_root_fun_keys: Vec::new(),
        },
        OptLevel::O0,
    )
    .unwrap();
    let instance = InstanceKey {
        template,
        type_args: Vec::new(),
        eff_args: Vec::new(),
    };
    (materializer, instance)
}

fn generic_materializer_for_body(
    body: Body,
    eff_param_name: Option<String>,
) -> (MirInstanceMaterializer, InstanceKey) {
    generic_materializer_for_body_with_template(body, eff_param_name, generic_template_key())
}

fn materialized_for_test(file: File, types: TypeStore) -> MaterializedMir {
    let instance_keys = Vec::new();
    let callable_families = MaterializedCallableFamilies::from_inputs(Vec::new());
    let summaries = MaterializedMirSummaries::default();
    let pass_artifacts = MaterializedMirPassArtifacts::from_initial_publication(
        &file,
        &summaries,
        &callable_families,
        &instance_keys,
    );
    MaterializedMir {
        file,
        types,
        instance_keys,
        summaries,
        backend_contracts: MaterializedBackendContracts::default(),
        top_level_value_tys: HashMap::new(),
        stable_cone_key: StableConeKey::new("tests", "0.0.0"),
        stable_instance_keys: HashMap::new(),
        stable_template_keys: HashMap::new(),
        nongeneric_callable_stable_template_keys: HashMap::new(),
        opt_level: OptLevel::O0,
        callable_families,
        pass_artifacts,
        dispatch_devirtualization_facts: DispatchDevirtualizationFacts::default(),
        caller_side_pass_candidates: Vec::new(),
        source_callable_signatures: Vec::new(),
    }
}

fn fun_has_dispatch_call(fun: &FunDecl) -> bool {
    let Some(body) = fun.body.as_ref() else {
        return false;
    };
    body.blocks.iter().any(|block| {
        block.stmts.iter().any(|stmt| {
            matches!(
                &stmt.kind,
                StatementKind::Assign {
                    value: Rvalue::Call {
                        kind: CallKind::Virtual { .. } | CallKind::Interface { .. },
                        ..
                    },
                    ..
                }
            )
        })
    })
}

#[test]
fn instance_display_fqn_and_exported_symbol_use_separate_identity_surfaces() {
    let template_a = generic_template_key_with_source_path(PathBuf::from(
        "/tmp/root-a/fixtures/materialize_id.scoop",
    ));
    let template_b = generic_template_key_with_source_path(PathBuf::from(
        "/tmp/root-b/fixtures/materialize_id.scoop",
    ));
    let (materializer_a, mut instance_a) =
        generic_materializer_for_body_with_template(unit_return_body(), None, template_a);
    let (materializer_b, mut instance_b) =
        generic_materializer_for_body_with_template(unit_return_body(), None, template_b);

    instance_a.type_args.push(materializer_a.builtins.int);
    instance_b.type_args.push(materializer_b.builtins.int);

    let display_a = materializer_a.instance_display_fqn(&instance_a);
    let display_b = materializer_b.instance_display_fqn(&instance_b);
    let exported_a = materializer_a.instance_exported_fun_symbol(&instance_a);
    let exported_b = materializer_b.instance_exported_fun_symbol(&instance_b);

    assert_eq!(display_a, "fixtures.materialize.id::<Int>");
    assert_eq!(display_a, display_b);
    assert!(exported_a.starts_with("__scoop_abi0_fun__fixtures_materialize_id__h"));
    assert_eq!(exported_a, exported_b);
    assert!(!exported_a.contains("::<"));
    assert!(!exported_a.contains("Int"));
}

#[test]
fn materialized_overloaded_generic_instances_publish_distinct_path_stable_exported_symbols() {
    let sess = Session::new().unwrap();
    let program = r#"
package fixtures.materialize

fun <T> pick(x: T): T { return x }
fun <T> pick(x: T, y: T): T { return y }

object Box {
fun <T> pick(x: T): T { return x }
fun <T> pick(x: T, y: T): T { return y }
}

fun main(): Int {
val a: Int = pick(1)
val b: Int = pick(1, 2)
val c: Int = Box.pick(3)
val d: Int = Box.pick(3, 4)
return a + b + c + d
}
"#;

    let collect_symbols = |source: &SourceFile| {
        let materialized = materialize_for_dump(&sess, source)
            .expect("overloaded generic fixture 应可 materialize");
        let pass_view = materialized.pass_view();
        let top_level = pass_view
            .instances()
            .filter(|family| {
                family.key().template.fqn == "fixtures.materialize.pick"
                    && !family.key().type_args.is_empty()
            })
            .map(|family| {
                materialized
                    .instance_exported_fun_symbol(family.key())
                    .expect("materialized generic overload instance 应发布 stable exported symbol")
            })
            .collect::<std::collections::BTreeSet<_>>();
        let member = pass_view
            .instances()
            .filter(|family| {
                family.key().template.fqn == "fixtures.materialize.Box.pick"
                    && !family.key().type_args.is_empty()
            })
            .map(|family| {
                materialized
                    .instance_exported_fun_symbol(family.key())
                    .expect("materialized generic member overload instance 应发布 stable exported symbol")
            })
            .collect::<std::collections::BTreeSet<_>>();
        (top_level, member)
    };

    let source_a = SourceFile::new_virtual(
        "/tmp/root-a/fixtures/materialize_overload_exported_identity.scoop",
        program,
    );
    let source_b = SourceFile::new_virtual(
        "/tmp/root-b/fixtures/materialize_overload_exported_identity.scoop",
        program,
    );

    let (top_level_a, member_a) = collect_symbols(&source_a);
    let (top_level_b, member_b) = collect_symbols(&source_b);

    assert_eq!(
        top_level_a.len(),
        2,
        "两个 top-level generic overload 应发布两个 distinct exported symbol：{top_level_a:#?}"
    );
    assert_eq!(
        member_a.len(),
        2,
        "两个 generic member overload 应发布两个 distinct exported symbol：{member_a:#?}"
    );
    assert!(
        top_level_a
            .iter()
            .all(|symbol| symbol.starts_with("__scoop_abi0_fun__fixtures_materialize_pick__h")),
        "top-level generic overload exported symbol 应统一走 AbiMangler namespace：{top_level_a:#?}"
    );
    assert!(
        member_a
            .iter()
            .all(|symbol| symbol.starts_with("__scoop_abi0_fun__fixtures_materialize_Box_pick__h")),
        "generic member overload exported symbol 应统一走 AbiMangler namespace：{member_a:#?}"
    );
    assert_eq!(
        top_level_a, top_level_b,
        "不同源码根路径下的 top-level generic overload exported symbol 应保持稳定：{top_level_a:#?} vs {top_level_b:#?}"
    );
    assert_eq!(
        member_a, member_b,
        "不同源码根路径下的 generic member overload exported symbol 应保持稳定：{member_a:#?} vs {member_b:#?}"
    );
}

#[test]
fn materialized_mir_no_todo_rejects_statement_template() {
    let (materializer, instance) = generic_materializer_for_body(body_with_statement_todo(), None);

    let err = materializer.run(vec![instance]).unwrap_err();
    assert!(matches!(
        *err,
        MirMaterializeError::MaterializedTodo {
            category: MirPlaceholderCategory::Statement,
            reason,
            ..
        } if reason == SYNTHETIC_STATEMENT_TODO_REASON
    ));
}

#[test]
fn materialized_mir_no_todo_rejects_rvalue_template() {
    let mut types = TypeStore::new();
    let builtins = types.intern_builtins();
    let (materializer, instance) =
        generic_materializer_for_body(body_with_rvalue_todo(builtins.unit), None);

    let err = materializer.run(vec![instance]).unwrap_err();
    assert!(matches!(
        *err,
        MirMaterializeError::MaterializedTodo {
            category: MirPlaceholderCategory::Rvalue,
            reason,
            ..
        } if reason == "missing expr"
    ));
}

#[test]
fn materialized_mir_no_todo_rejects_terminator_template() {
    let (materializer, instance) = generic_materializer_for_body(body_with_terminator_todo(), None);

    let err = materializer.run(vec![instance]).unwrap_err();
    assert!(matches!(
        *err,
        MirMaterializeError::MaterializedTodo {
            category: MirPlaceholderCategory::Terminator,
            reason,
            ..
        } if reason == "unterminated"
    ));
}

#[test]
fn materialized_mir_no_todo_rejects_unwind_template() {
    let (materializer, instance) = generic_materializer_for_body(body_with_unwind_todo(), None);

    let err = materializer.run(vec![instance]).unwrap_err();
    assert!(matches!(
        *err,
        MirMaterializeError::MaterializedTodo {
            category: MirPlaceholderCategory::UnwindAction,
            reason,
            ..
        } if reason == "perform unwind pending"
    ));
}

#[test]
fn mir_no_return_none_materialized_rejects_non_unit_empty_return() {
    let mut types = TypeStore::new();
    let builtins = types.intern_builtins();
    let file = File {
        items: vec![Item::Fun(FunDecl {
            span: test_span(),
            fqn: "fixtures.materialize.main".to_string(),
            name: "main".to_string(),
            ty: builtins.int,
            params: Vec::new(),
            return_ty: builtins.int,
            body: Some(unit_return_body()),
        })],
    };
    let materialized = materialized_for_test(file, types);

    let err = materialized.validate_materialized().unwrap_err();
    assert!(matches!(
        *err,
        MirMaterializeError::MaterializedMirValidation {
            error: crate::mir::MirValidationError::ProductionMissingReturnValue {
                return_ty,
                ..
            },
            ..
        } if return_ty == builtins.int
    ));
}

#[test]
fn materialized_mir_mir_materialize_generics_rejects_frame_slot_type_param() {
    let mut types = TypeStore::new();
    let builtins = types.intern_builtins();
    let param_ty = types.ty_param(TypeParamType {
        name: "T".to_string(),
        decl_file: test_source_path(),
        decl_span: test_span(),
    });
    let mut body = unit_return_body();
    body.push_local(LocalDecl {
        span: test_span(),
        name: Some("x".to_string()),
        ty: param_ty,
        source: LocalSourceKind::SourceLocal,
    });
    let file = File {
        items: vec![Item::Fun(FunDecl {
            span: test_span(),
            fqn: "fixtures.materialize.main".to_string(),
            name: "main".to_string(),
            ty: builtins.unit,
            params: Vec::new(),
            return_ty: builtins.unit,
            body: Some(body),
        })],
    };
    let materialized = materialized_for_test(file, types);

    let err = materialized.validate_materialized().unwrap_err();
    assert!(matches!(
        *err,
        MirMaterializeError::MaterializedUnresolvedGenericParam {
            surface: "frame slot",
            ..
        }
    ));
}

#[test]
fn materialized_mir_contract_rejects_extern_global_initializer() {
    let mut types = TypeStore::new();
    let builtins = types.intern_builtins();
    let file = File {
        items: vec![Item::ExternGlobal(ExternGlobalRoot {
            span: test_span(),
            fqn: "fixtures.materialize.NativeCounter".to_string(),
            source_path: test_source_path(),
            ty: builtins.int,
            mutable: true,
            symbol: "scoop_test_native_counter".to_string(),
            linkage: crate::hir::ExternGlobalLinkage::External,
            storage: crate::hir::TopLevelVarStorage::Global,
            initializer_absent: false,
            unsafe_required: true,
        })],
    };
    let materialized = materialized_for_test(file, types);

    let err = materialized.validate_materialized().unwrap_err();
    assert!(matches!(
        *err,
        MirMaterializeError::MaterializedMirValidation {
            error: crate::mir::MirValidationError::TypeContract {
                surface: "extern global initializer",
                detail: "extern global roots must not publish an initializer",
                ..
            },
            ..
        }
    ));
}

#[test]
fn materialized_mir_contract_rejects_immutable_extern_global_store() {
    let mut types = TypeStore::new();
    let builtins = types.intern_builtins();
    let mut body = Body::new_empty();
    let value = body.push_local(LocalDecl {
        span: test_span(),
        name: Some("value".to_string()),
        ty: builtins.int,
        source: LocalSourceKind::CompilerTemporary,
    });
    let bb = body.push_block(BasicBlock {
        is_cleanup: false,
        stmts: vec![Statement {
            span: test_span(),
            kind: StatementKind::StoreTopLevelVar {
                fqn: "fixtures.materialize.NativeCounter".to_string(),
                value: Operand::Local(value),
                value_ty: builtins.int,
            },
        }],
        terminator: Terminator {
            span: test_span(),
            kind: TerminatorKind::Return { value: None },
            unwind: UnwindAction::NoUnwind,
        },
    });
    body.start = bb;
    let file = File {
        items: vec![
            Item::ExternGlobal(ExternGlobalRoot {
                span: test_span(),
                fqn: "fixtures.materialize.NativeCounter".to_string(),
                source_path: test_source_path(),
                ty: builtins.int,
                mutable: false,
                symbol: "scoop_test_native_counter".to_string(),
                linkage: crate::hir::ExternGlobalLinkage::External,
                storage: crate::hir::TopLevelVarStorage::Global,
                initializer_absent: true,
                unsafe_required: true,
            }),
            Item::Fun(FunDecl {
                span: test_span(),
                fqn: "fixtures.materialize.main".to_string(),
                name: "main".to_string(),
                ty: builtins.unit,
                params: Vec::new(),
                return_ty: builtins.unit,
                body: Some(body),
            }),
        ],
    };
    let materialized = materialized_for_test(file, types);

    let err = materialized.validate_materialized().unwrap_err();
    assert!(matches!(
        *err,
        MirMaterializeError::MaterializedMirValidation {
            error: crate::mir::MirValidationError::TypeContract {
                surface: "top-level store target",
                detail: "top-level store target is immutable",
                ..
            },
            ..
        }
    ));
}

#[test]
fn materialized_mir_type_contract_rejects_missing_operand_local() {
    let mut types = TypeStore::new();
    let builtins = types.intern_builtins();
    let mut body = Body::new_empty();
    let target = body.push_local(LocalDecl {
        span: test_span(),
        name: Some("target".to_string()),
        ty: builtins.unit,
        source: LocalSourceKind::CompilerTemporary,
    });
    let bb = body.push_block(BasicBlock {
        is_cleanup: false,
        stmts: vec![Statement {
            span: test_span(),
            kind: StatementKind::Assign {
                target,
                value: Rvalue::Use(Operand::Local(LocalId::from_raw(42))),
            },
        }],
        terminator: Terminator {
            span: test_span(),
            kind: TerminatorKind::Return { value: None },
            unwind: UnwindAction::NoUnwind,
        },
    });
    body.start = bb;
    let file = File {
        items: vec![Item::Fun(FunDecl {
            span: test_span(),
            fqn: "fixtures.materialize.main".to_string(),
            name: "main".to_string(),
            ty: builtins.unit,
            params: Vec::new(),
            return_ty: builtins.unit,
            body: Some(body),
        })],
    };
    let materialized = materialized_for_test(file, types);

    let err = materialized.validate_materialized().unwrap_err();
    assert!(matches!(
        *err,
        MirMaterializeError::MaterializedMirValidation {
            error: crate::mir::MirValidationError::TypeContract {
                surface: "source value",
                detail: "local reference is outside the body local table",
                ..
            },
            ..
        }
    ));
}

#[test]
fn materialized_mir_type_contract_rejects_param_local_type_drift() {
    let mut types = TypeStore::new();
    let builtins = types.intern_builtins();
    let mut body = unit_return_body();
    let local = body.push_local(LocalDecl {
        span: test_span(),
        name: Some("x".to_string()),
        ty: builtins.int,
        source: LocalSourceKind::SourceLocal,
    });
    let file = File {
        items: vec![Item::Fun(FunDecl {
            span: test_span(),
            fqn: "fixtures.materialize.main".to_string(),
            name: "main".to_string(),
            ty: builtins.unit,
            params: vec![Param {
                span: test_span(),
                name: "x".to_string(),
                ty: builtins.bool_,
                local,
            }],
            return_ty: builtins.unit,
            body: Some(body),
        })],
    };
    let materialized = materialized_for_test(file, types);

    let err = materialized.validate_materialized().unwrap_err();
    assert!(matches!(
        *err,
        MirMaterializeError::MaterializedMirValidation {
            error: crate::mir::MirValidationError::TypeContract {
                surface: "parameter type",
                detail: "parameter type and parameter local type disagree",
                ..
            },
            ..
        }
    ));
}

#[test]
fn materialized_mir_type_contract_rejects_return_value_outside_local_table() {
    let mut types = TypeStore::new();
    let builtins = types.intern_builtins();
    let mut body = Body::new_empty();
    let bb = body.push_block(BasicBlock {
        is_cleanup: false,
        stmts: Vec::new(),
        terminator: Terminator {
            span: test_span(),
            kind: TerminatorKind::Return {
                value: Some(Operand::Local(LocalId::from_raw(0))),
            },
            unwind: UnwindAction::NoUnwind,
        },
    });
    body.start = bb;
    let file = File {
        items: vec![Item::Fun(FunDecl {
            span: test_span(),
            fqn: "fixtures.materialize.main".to_string(),
            name: "main".to_string(),
            ty: builtins.unit,
            params: Vec::new(),
            return_ty: builtins.unit,
            body: Some(body),
        })],
    };
    let materialized = materialized_for_test(file, types);

    let err = materialized.validate_materialized().unwrap_err();
    assert!(matches!(
        *err,
        MirMaterializeError::MaterializedMirValidation {
            error: crate::mir::MirValidationError::TypeContract {
                surface: "return value",
                detail: "local reference is outside the body local table",
                ..
            },
            ..
        }
    ));
}

#[test]
fn materialized_mir_cfg_contract_rejects_non_bool_branch_condition() {
    let mut types = TypeStore::new();
    let builtins = types.intern_builtins();
    let mut body = Body::new_empty();
    let cond = body.push_local(LocalDecl {
        span: test_span(),
        name: Some("cond".to_string()),
        ty: builtins.int,
        source: LocalSourceKind::SourceLocal,
    });
    let bb = body.push_block(BasicBlock {
        is_cleanup: false,
        stmts: Vec::new(),
        terminator: Terminator {
            span: test_span(),
            kind: TerminatorKind::CondBr {
                cond: Operand::Local(cond),
                then_target: BasicBlockId::from_raw(0),
                else_target: BasicBlockId::from_raw(0),
            },
            unwind: UnwindAction::NoUnwind,
        },
    });
    body.start = bb;
    let file = File {
        items: vec![Item::Fun(FunDecl {
            span: test_span(),
            fqn: "fixtures.materialize.main".to_string(),
            name: "main".to_string(),
            ty: builtins.unit,
            params: Vec::new(),
            return_ty: builtins.unit,
            body: Some(body),
        })],
    };
    let materialized = materialized_for_test(file, types);

    let err = materialized.validate_materialized().unwrap_err();
    assert!(matches!(
        *err,
        MirMaterializeError::MaterializedMirValidation {
            error: crate::mir::MirValidationError::TypeContract {
                surface: "branch condition",
                detail: "branch condition operand must have Bool type",
                ..
            },
            ..
        }
    ));
}

#[test]
fn materialized_mir_cfg_contract_rejects_residual_interpolated_string() {
    let mut types = TypeStore::new();
    let builtins = types.intern_builtins();
    let mut body = Body::new_empty();
    let target = body.push_local(LocalDecl {
        span: test_span(),
        name: Some("message".to_string()),
        ty: builtins.string,
        source: LocalSourceKind::CompilerTemporary,
    });
    let bb = body.push_block(BasicBlock {
        is_cleanup: false,
        stmts: vec![Statement {
            span: test_span(),
            kind: StatementKind::Assign {
                target,
                value: Rvalue::InterpolatedString {
                    raw: false,
                    parts: Vec::new(),
                },
            },
        }],
        terminator: Terminator {
            span: test_span(),
            kind: TerminatorKind::Return { value: None },
            unwind: UnwindAction::NoUnwind,
        },
    });
    body.start = bb;
    let file = File {
        items: vec![Item::Fun(FunDecl {
            span: test_span(),
            fqn: "fixtures.materialize.main".to_string(),
            name: "main".to_string(),
            ty: builtins.unit,
            params: Vec::new(),
            return_ty: builtins.unit,
            body: Some(body),
        })],
    };
    let materialized = materialized_for_test(file, types);

    let err = materialized.validate_materialized().unwrap_err();
    assert!(matches!(
        *err,
        MirMaterializeError::MaterializedMirValidation {
            error: crate::mir::MirValidationError::TypeContract {
                surface: "interpolated string",
                detail: "interpolated strings must be desugared before MIR codegen",
                ..
            },
            ..
        }
    ));
}

#[test]
fn materialized_mir_aggregate_schema_rejects_tuple_arity_drift() {
    let mut types = TypeStore::new();
    let builtins = types.intern_builtins();
    let tuple_ty = types.ty_tuple(vec![builtins.int, builtins.bool_]);
    let mut body = Body::new_empty();
    let source = body.push_local(LocalDecl {
        span: test_span(),
        name: Some("source".to_string()),
        ty: builtins.int,
        source: LocalSourceKind::SourceLocal,
    });
    let target = body.push_local(LocalDecl {
        span: test_span(),
        name: Some("target".to_string()),
        ty: tuple_ty,
        source: LocalSourceKind::CompilerTemporary,
    });
    let bb = body.push_block(BasicBlock {
        is_cleanup: false,
        stmts: vec![Statement {
            span: test_span(),
            kind: StatementKind::Assign {
                target,
                value: Rvalue::MakeTuple {
                    elements: vec![Operand::Local(source)],
                    transport: AggregateTransportMetadata {
                        aggregate_ty: tuple_ty,
                        kind: crate::mir::AggregateTransportKind::Tuple,
                        fields: vec![AggregateTransportField {
                            index: 0,
                            name: None,
                            ty: builtins.int,
                            transport: ValueTransportMetadata::plain(
                                builtins.int,
                                crate::mir::MirTransportKind::Scalar,
                            ),
                        }],
                    },
                },
            },
        }],
        terminator: Terminator {
            span: test_span(),
            kind: TerminatorKind::Return { value: None },
            unwind: UnwindAction::NoUnwind,
        },
    });
    body.start = bb;
    let file = File {
        items: vec![Item::Fun(FunDecl {
            span: test_span(),
            fqn: "fixtures.materialize.main".to_string(),
            name: "main".to_string(),
            ty: builtins.unit,
            params: Vec::new(),
            return_ty: builtins.unit,
            body: Some(body),
        })],
    };
    let materialized = materialized_for_test(file, types);

    let err = materialized.validate_materialized().unwrap_err();
    assert!(matches!(
        *err,
        MirMaterializeError::MaterializedMirValidation {
            error: crate::mir::MirValidationError::ProductionTransportMetadata {
                transport: "tuple aggregate transport",
                detail: "tuple aggregate field count does not match tuple type",
                ..
            },
            ..
        }
    ));
}

#[test]
fn materialized_mir_pattern_schema_rejects_tuple_arity_drift() {
    let mut types = TypeStore::new();
    let builtins = types.intern_builtins();
    let tuple_ty = types.ty_tuple(vec![builtins.int]);
    let mut body = Body::new_empty();
    let subject = body.push_local(LocalDecl {
        span: test_span(),
        name: Some("subject".to_string()),
        ty: tuple_ty,
        source: LocalSourceKind::SourceLocal,
    });
    let target = body.push_local(LocalDecl {
        span: test_span(),
        name: Some("matched".to_string()),
        ty: builtins.bool_,
        source: LocalSourceKind::CompilerTemporary,
    });
    let bb = body.push_block(BasicBlock {
        is_cleanup: false,
        stmts: vec![Statement {
            span: test_span(),
            kind: StatementKind::Assign {
                target,
                value: Rvalue::PatternMatch {
                    subject: Operand::Local(subject),
                    pattern: Pattern::Tuple {
                        elements: vec![Pattern::Wildcard, Pattern::Wildcard],
                    },
                },
            },
        }],
        terminator: Terminator {
            span: test_span(),
            kind: TerminatorKind::Return { value: None },
            unwind: UnwindAction::NoUnwind,
        },
    });
    body.start = bb;
    let file = File {
        items: vec![Item::Fun(FunDecl {
            span: test_span(),
            fqn: "fixtures.materialize.main".to_string(),
            name: "main".to_string(),
            ty: builtins.unit,
            params: Vec::new(),
            return_ty: builtins.unit,
            body: Some(body),
        })],
    };
    let materialized = materialized_for_test(file, types);

    let err = materialized.validate_materialized().unwrap_err();
    assert!(matches!(
        *err,
        MirMaterializeError::MaterializedMirValidation {
            error: crate::mir::MirValidationError::TypeContract {
                surface: "pattern tuple arity",
                detail: "tuple pattern arity does not match subject type",
                ..
            },
            ..
        }
    ));
}

#[test]
fn materialized_mir_pattern_extract_rejects_result_type_drift() {
    let mut types = TypeStore::new();
    let builtins = types.intern_builtins();
    let tuple_ty = types.ty_tuple(vec![builtins.int]);
    let mut body = Body::new_empty();
    let subject = body.push_local(LocalDecl {
        span: test_span(),
        name: Some("subject".to_string()),
        ty: tuple_ty,
        source: LocalSourceKind::SourceLocal,
    });
    let target = body.push_local(LocalDecl {
        span: test_span(),
        name: Some("extracted".to_string()),
        ty: builtins.bool_,
        source: LocalSourceKind::CompilerTemporary,
    });
    let bb = body.push_block(BasicBlock {
        is_cleanup: false,
        stmts: vec![Statement {
            span: test_span(),
            kind: StatementKind::Assign {
                target,
                value: Rvalue::PatternExtract {
                    subject: Operand::Local(subject),
                    path: vec![crate::mir::PatternBindingStep::TupleIndex(0)],
                },
            },
        }],
        terminator: Terminator {
            span: test_span(),
            kind: TerminatorKind::Return { value: None },
            unwind: UnwindAction::NoUnwind,
        },
    });
    body.start = bb;
    let file = File {
        items: vec![Item::Fun(FunDecl {
            span: test_span(),
            fqn: "fixtures.materialize.main".to_string(),
            name: "main".to_string(),
            ty: builtins.unit,
            params: Vec::new(),
            return_ty: builtins.unit,
            body: Some(body),
        })],
    };
    let materialized = materialized_for_test(file, types);

    let err = materialized.validate_materialized().unwrap_err();
    assert!(matches!(
        *err,
        MirMaterializeError::MaterializedMirValidation {
            error: crate::mir::MirValidationError::TypeContract {
                surface: "pattern extract result",
                detail: "pattern extract result type does not match assignment target",
                ..
            },
            ..
        }
    ));
}

#[test]
fn materialized_mir_call_abi_rejects_direct_call_arity_drift() {
    let mut types = TypeStore::new();
    let builtins = types.intern_builtins();
    let mut caller_body = Body::new_empty();
    let arg = caller_body.push_local(LocalDecl {
        span: test_span(),
        name: Some("arg".to_string()),
        ty: builtins.int,
        source: LocalSourceKind::SourceLocal,
    });
    let target = caller_body.push_local(LocalDecl {
        span: test_span(),
        name: Some("result".to_string()),
        ty: builtins.int,
        source: LocalSourceKind::CompilerTemporary,
    });
    let bb = caller_body.push_block(BasicBlock {
        is_cleanup: false,
        stmts: vec![Statement {
            span: test_span(),
            kind: StatementKind::Assign {
                target,
                value: Rvalue::Call {
                    site_id: SiteId::from_raw(0),
                    kind: CallKind::Direct {
                        callee_fqn: "fixtures.materialize.callee".to_string(),
                    },
                    args: vec![CallArg {
                        span: test_span(),
                        name: None,
                        value: Operand::Local(arg),
                    }],
                    transport: CallTransportMetadata::plain_no_outward(
                        builtins.int,
                        MirTransportKind::Scalar,
                    ),
                },
            },
        }],
        terminator: Terminator {
            span: test_span(),
            kind: TerminatorKind::Return { value: None },
            unwind: UnwindAction::NoUnwind,
        },
    });
    caller_body.start = bb;
    let file = File {
        items: vec![
            Item::Fun(FunDecl {
                span: test_span(),
                fqn: "fixtures.materialize.callee".to_string(),
                name: "callee".to_string(),
                ty: builtins.int,
                params: vec![
                    Param {
                        span: test_span(),
                        name: "x".to_string(),
                        ty: builtins.int,
                        local: LocalId::from_raw(0),
                    },
                    Param {
                        span: test_span(),
                        name: "y".to_string(),
                        ty: builtins.int,
                        local: LocalId::from_raw(1),
                    },
                ],
                return_ty: builtins.int,
                body: None,
            }),
            Item::Fun(FunDecl {
                span: test_span(),
                fqn: "fixtures.materialize.main".to_string(),
                name: "main".to_string(),
                ty: builtins.unit,
                params: Vec::new(),
                return_ty: builtins.unit,
                body: Some(caller_body),
            }),
        ],
    };
    let materialized = materialized_for_test(file, types);

    let err = materialized.validate_materialized().unwrap_err();
    assert!(matches!(
        *err,
        MirMaterializeError::MaterializedMirValidation {
            error: crate::mir::MirValidationError::TypeContract {
                surface: "call arguments",
                detail: "call arguments do not bind exactly to callee parameters",
                ..
            },
            ..
        }
    ));
}

#[test]
fn materialized_mir_class_ctor_contract_rejects_selected_ctor_drift() {
    let mut types = TypeStore::new();
    let builtins = types.intern_builtins();
    let class_fqn = "fixtures.materialize.Box";
    let class_ty = types.intern(TypeKind::Ref(RefTypeKind::Nominal(NominalType {
        fqn: class_fqn.to_string(),
        args: Vec::new(),
        eff: None,
    })));
    let mut body = Body::new_empty();
    let target = body.push_local(LocalDecl {
        span: test_span(),
        name: Some("box".to_string()),
        ty: class_ty,
        source: LocalSourceKind::CompilerTemporary,
    });
    let bb = body.push_block(BasicBlock {
        is_cleanup: false,
        stmts: vec![Statement {
            span: test_span(),
            kind: StatementKind::Assign {
                target,
                value: Rvalue::ClassCtor {
                    site_id: SiteId::from_raw(0),
                    class_fqn: class_fqn.to_string(),
                    ctor: ClassCtorCallMetadata {
                        selected_ctor_span: Some(Span::new(30, 40)),
                        ordered_param_count: 0,
                    },
                    args: Vec::new(),
                    hidden_effects: EffectRow::pure(),
                },
            },
        }],
        terminator: Terminator {
            span: test_span(),
            kind: TerminatorKind::Return { value: None },
            unwind: UnwindAction::NoUnwind,
        },
    });
    body.start = bb;
    let file = File {
        items: vec![
            Item::Metadata(MetadataRoot::Nominal(NominalMetadata {
                span: test_span(),
                fqn: class_fqn.to_string(),
                name: "Box".to_string(),
                kind: ast::TypeKind::Class,
                is_interior_mutable: false,
                type_params: Vec::new(),
                supertypes: Vec::new(),
                interfaces: Vec::new(),
                constructors: vec![CtorMetadata {
                    span: Span::new(50, 60),
                    kind: crate::hir::ClassCtorKind::Primary,
                    params: Vec::new(),
                    delegation: None,
                }],
                members: Vec::new(),
            })),
            Item::Fun(FunDecl {
                span: test_span(),
                fqn: "fixtures.materialize.main".to_string(),
                name: "main".to_string(),
                ty: builtins.unit,
                params: Vec::new(),
                return_ty: builtins.unit,
                body: Some(body),
            }),
        ],
    };
    let materialized = materialized_for_test(file, types);

    let err = materialized.validate_materialized().unwrap_err();
    assert!(matches!(
        *err,
        MirMaterializeError::MaterializedMirValidation {
            error: crate::mir::MirValidationError::TypeContract {
                surface: "class constructor target",
                detail: "selected class constructor is not declared",
                ..
            },
            ..
        }
    ));
}

#[test]
fn materialized_mir_class_ctor_contract_rejects_result_nominal_mismatch() {
    let mut types = TypeStore::new();
    let builtins = types.intern_builtins();
    let class_fqn = "fixtures.materialize.Box";
    let other_ty = types.intern(TypeKind::Ref(RefTypeKind::Nominal(NominalType {
        fqn: "fixtures.materialize.Other".to_string(),
        args: Vec::new(),
        eff: None,
    })));
    let mut body = Body::new_empty();
    let target = body.push_local(LocalDecl {
        span: test_span(),
        name: Some("box".to_string()),
        ty: other_ty,
        source: LocalSourceKind::CompilerTemporary,
    });
    let bb = body.push_block(BasicBlock {
        is_cleanup: false,
        stmts: vec![Statement {
            span: test_span(),
            kind: StatementKind::Assign {
                target,
                value: Rvalue::ClassCtor {
                    site_id: SiteId::from_raw(0),
                    class_fqn: class_fqn.to_string(),
                    ctor: ClassCtorCallMetadata {
                        selected_ctor_span: None,
                        ordered_param_count: 0,
                    },
                    args: Vec::new(),
                    hidden_effects: EffectRow::pure(),
                },
            },
        }],
        terminator: Terminator {
            span: test_span(),
            kind: TerminatorKind::Return { value: None },
            unwind: UnwindAction::NoUnwind,
        },
    });
    body.start = bb;
    let file = File {
        items: vec![
            Item::Metadata(MetadataRoot::Nominal(NominalMetadata {
                span: test_span(),
                fqn: class_fqn.to_string(),
                name: "Box".to_string(),
                kind: ast::TypeKind::Class,
                is_interior_mutable: false,
                type_params: Vec::new(),
                supertypes: Vec::new(),
                interfaces: Vec::new(),
                constructors: Vec::new(),
                members: Vec::new(),
            })),
            Item::Fun(FunDecl {
                span: test_span(),
                fqn: "fixtures.materialize.main".to_string(),
                name: "main".to_string(),
                ty: builtins.unit,
                params: Vec::new(),
                return_ty: builtins.unit,
                body: Some(body),
            }),
        ],
    };
    let materialized = materialized_for_test(file, types);

    let err = materialized.validate_materialized().unwrap_err();
    assert!(matches!(
        *err,
        MirMaterializeError::MaterializedMirValidation {
            error: crate::mir::MirValidationError::TypeContract {
                surface: "class constructor result",
                detail: "class constructor result target and class metadata disagree",
                ..
            },
            ..
        }
    ));
}

#[test]
fn materialized_mir_member_contract_rejects_unresolved_target() {
    let mut types = TypeStore::new();
    let builtins = types.intern_builtins();
    let mut body = Body::new_empty();
    let receiver = body.push_local(LocalDecl {
        span: test_span(),
        name: Some("receiver".to_string()),
        ty: builtins.int,
        source: LocalSourceKind::SourceLocal,
    });
    let target = body.push_local(LocalDecl {
        span: test_span(),
        name: Some("value".to_string()),
        ty: builtins.int,
        source: LocalSourceKind::CompilerTemporary,
    });
    let bb = body.push_block(BasicBlock {
        is_cleanup: false,
        stmts: vec![Statement {
            span: test_span(),
            kind: StatementKind::Assign {
                target,
                value: Rvalue::MemberAccess {
                    site_id: None,
                    receiver: Operand::Local(receiver),
                    member: MemberAccessMetadata {
                        name: "missing".to_string(),
                        receiver_ty: builtins.int,
                        resolved: None,
                        hidden_effects: EffectRow::pure(),
                    },
                },
            },
        }],
        terminator: Terminator {
            span: test_span(),
            kind: TerminatorKind::Return { value: None },
            unwind: UnwindAction::NoUnwind,
        },
    });
    body.start = bb;
    let file = File {
        items: vec![Item::Fun(FunDecl {
            span: test_span(),
            fqn: "fixtures.materialize.main".to_string(),
            name: "main".to_string(),
            ty: builtins.unit,
            params: Vec::new(),
            return_ty: builtins.unit,
            body: Some(body),
        })],
    };
    let materialized = materialized_for_test(file, types);

    let err = materialized.validate_materialized().unwrap_err();
    assert!(matches!(
        *err,
        MirMaterializeError::MaterializedMirValidation {
            error: crate::mir::MirValidationError::TypeContract {
                surface: "member target",
                detail: "member access target must be resolved before MIR codegen",
                ..
            },
            ..
        }
    ));
}

#[test]
fn materialized_mir_member_store_contract_rejects_non_place_receiver() {
    let mut types = TypeStore::new();
    let builtins = types.intern_builtins();
    let class_fqn = "fixtures.materialize.Box";
    let class_ty = types.intern(TypeKind::Ref(RefTypeKind::Nominal(NominalType {
        fqn: class_fqn.to_string(),
        args: Vec::new(),
        eff: None,
    })));
    let mut body = Body::new_empty();
    let value = body.push_local(LocalDecl {
        span: test_span(),
        name: Some("value".to_string()),
        ty: builtins.int,
        source: LocalSourceKind::CompilerTemporary,
    });
    let bb = body.push_block(BasicBlock {
        is_cleanup: false,
        stmts: vec![Statement {
            span: test_span(),
            kind: StatementKind::StoreMember {
                receiver: Operand::Const(ConstValue::Unit),
                member: MemberAccessMetadata {
                    name: "value".to_string(),
                    receiver_ty: class_ty,
                    resolved: Some(MemberTarget::Value {
                        fqn: format!("{class_fqn}.value"),
                    }),
                    hidden_effects: EffectRow::pure(),
                },
                value: Operand::Local(value),
                value_ty: builtins.int,
                continuation_route: StoredContinuationRoutePublication::None,
            },
        }],
        terminator: Terminator {
            span: test_span(),
            kind: TerminatorKind::Return { value: None },
            unwind: UnwindAction::NoUnwind,
        },
    });
    body.start = bb;
    let file = File {
        items: vec![
            Item::Metadata(MetadataRoot::Nominal(NominalMetadata {
                span: test_span(),
                fqn: class_fqn.to_string(),
                name: "Box".to_string(),
                kind: ast::TypeKind::Class,
                is_interior_mutable: false,
                type_params: Vec::new(),
                supertypes: Vec::new(),
                interfaces: Vec::new(),
                constructors: Vec::new(),
                members: vec![DeclMemberMetadata::Field(FieldMetadata {
                    span: test_span(),
                    fqn: format!("{class_fqn}.value"),
                    name: "value".to_string(),
                    mutable: true,
                    ty: builtins.int,
                    origin: crate::hir::FieldOrigin::PrimaryCtorParam,
                })],
            })),
            Item::Fun(FunDecl {
                span: test_span(),
                fqn: "fixtures.materialize.main".to_string(),
                name: "main".to_string(),
                ty: builtins.unit,
                params: Vec::new(),
                return_ty: builtins.unit,
                body: Some(body),
            }),
        ],
    };
    let materialized = materialized_for_test(file, types);

    let err = materialized.validate_materialized().unwrap_err();
    assert!(matches!(
        *err,
        MirMaterializeError::MaterializedMirValidation {
            error: crate::mir::MirValidationError::TypeContract {
                surface: "member store receiver",
                detail: "member store receiver must be a local place",
                ..
            },
            ..
        }
    ));
}

#[test]
fn materialized_mir_member_store_contract_rejects_value_type_receiver() {
    let mut types = TypeStore::new();
    let builtins = types.intern_builtins();
    let class_fqn = "fixtures.materialize.Box";
    let mut body = Body::new_empty();
    let receiver = body.push_local(LocalDecl {
        span: test_span(),
        name: Some("receiver".to_string()),
        ty: builtins.int,
        source: LocalSourceKind::SourceLocal,
    });
    let value = body.push_local(LocalDecl {
        span: test_span(),
        name: Some("value".to_string()),
        ty: builtins.int,
        source: LocalSourceKind::CompilerTemporary,
    });
    let bb = body.push_block(BasicBlock {
        is_cleanup: false,
        stmts: vec![Statement {
            span: test_span(),
            kind: StatementKind::StoreMember {
                receiver: Operand::Local(receiver),
                member: MemberAccessMetadata {
                    name: "value".to_string(),
                    receiver_ty: builtins.int,
                    resolved: Some(MemberTarget::Value {
                        fqn: format!("{class_fqn}.value"),
                    }),
                    hidden_effects: EffectRow::pure(),
                },
                value: Operand::Local(value),
                value_ty: builtins.int,
                continuation_route: StoredContinuationRoutePublication::None,
            },
        }],
        terminator: Terminator {
            span: test_span(),
            kind: TerminatorKind::Return { value: None },
            unwind: UnwindAction::NoUnwind,
        },
    });
    body.start = bb;
    let file = File {
        items: vec![
            Item::Metadata(MetadataRoot::Nominal(NominalMetadata {
                span: test_span(),
                fqn: class_fqn.to_string(),
                name: "Box".to_string(),
                kind: ast::TypeKind::Class,
                is_interior_mutable: false,
                type_params: Vec::new(),
                supertypes: Vec::new(),
                interfaces: Vec::new(),
                constructors: Vec::new(),
                members: vec![DeclMemberMetadata::Field(FieldMetadata {
                    span: test_span(),
                    fqn: format!("{class_fqn}.value"),
                    name: "value".to_string(),
                    mutable: true,
                    ty: builtins.int,
                    origin: crate::hir::FieldOrigin::PrimaryCtorParam,
                })],
            })),
            Item::Fun(FunDecl {
                span: test_span(),
                fqn: "fixtures.materialize.main".to_string(),
                name: "main".to_string(),
                ty: builtins.unit,
                params: Vec::new(),
                return_ty: builtins.unit,
                body: Some(body),
            }),
        ],
    };
    let materialized = materialized_for_test(file, types);

    let err = materialized.validate_materialized().unwrap_err();
    assert!(matches!(
        *err,
        MirMaterializeError::MaterializedMirValidation {
            error: crate::mir::MirValidationError::TypeContract {
                surface: "member store receiver",
                detail: "member store receiver must be a reference nominal type",
                ..
            },
            ..
        }
    ));
}

#[test]
fn materialized_mir_member_store_contract_accepts_source_store_field_type_ids() {
    let mut types = TypeStore::new();
    let builtins = types.intern_builtins();
    let class_fqn = "fixtures.materialize.Box";
    let class_ty = types.intern(TypeKind::Ref(RefTypeKind::Nominal(NominalType {
        fqn: class_fqn.to_string(),
        args: Vec::new(),
        eff: None,
    })));
    let mut body = Body::new_empty();
    let receiver = body.push_local(LocalDecl {
        span: test_span(),
        name: Some("receiver".to_string()),
        ty: class_ty,
        source: LocalSourceKind::SourceLocal,
    });
    let value = body.push_local(LocalDecl {
        span: test_span(),
        name: Some("value".to_string()),
        ty: builtins.bool_,
        source: LocalSourceKind::CompilerTemporary,
    });
    let bb = body.push_block(BasicBlock {
        is_cleanup: false,
        stmts: vec![Statement {
            span: test_span(),
            kind: StatementKind::StoreMember {
                receiver: Operand::Local(receiver),
                member: MemberAccessMetadata {
                    name: "value".to_string(),
                    receiver_ty: class_ty,
                    resolved: Some(MemberTarget::Value {
                        fqn: format!("{class_fqn}.value"),
                    }),
                    hidden_effects: EffectRow::pure(),
                },
                value: Operand::Local(value),
                value_ty: builtins.bool_,
                continuation_route: StoredContinuationRoutePublication::None,
            },
        }],
        terminator: Terminator {
            span: test_span(),
            kind: TerminatorKind::Return { value: None },
            unwind: UnwindAction::NoUnwind,
        },
    });
    body.start = bb;
    let file = File {
        items: vec![
            Item::Metadata(MetadataRoot::Nominal(NominalMetadata {
                span: test_span(),
                fqn: class_fqn.to_string(),
                name: "Box".to_string(),
                kind: ast::TypeKind::Class,
                is_interior_mutable: false,
                type_params: Vec::new(),
                supertypes: Vec::new(),
                interfaces: Vec::new(),
                constructors: Vec::new(),
                members: vec![DeclMemberMetadata::Field(FieldMetadata {
                    span: test_span(),
                    fqn: format!("{class_fqn}.value"),
                    name: "value".to_string(),
                    mutable: true,
                    ty: builtins.int,
                    origin: crate::hir::FieldOrigin::PrimaryCtorParam,
                })],
            })),
            Item::Fun(FunDecl {
                span: test_span(),
                fqn: "fixtures.materialize.main".to_string(),
                name: "main".to_string(),
                ty: builtins.unit,
                params: Vec::new(),
                return_ty: builtins.unit,
                body: Some(body),
            }),
        ],
    };
    let materialized = materialized_for_test(file, types);

    materialized.validate_materialized().unwrap();
}

#[test]
fn materialized_mir_enum_contract_rejects_unknown_variant_payload() {
    let mut types = TypeStore::new();
    let builtins = types.intern_builtins();
    let option_int = types.ty_option(builtins.int);
    let mut body = Body::new_empty();
    let target = body.push_local(LocalDecl {
        span: test_span(),
        name: Some("opt".to_string()),
        ty: option_int,
        source: LocalSourceKind::CompilerTemporary,
    });
    let bb = body.push_block(BasicBlock {
        is_cleanup: false,
        stmts: vec![Statement {
            span: test_span(),
            kind: StatementKind::Assign {
                target,
                value: Rvalue::EnumVariant {
                    enum_ty: option_int,
                    variant_name: "Missing".to_string(),
                    args: Vec::new(),
                    payload: AggregateTransportMetadata {
                        aggregate_ty: option_int,
                        kind: crate::mir::AggregateTransportKind::EnumPayload,
                        fields: Vec::new(),
                    },
                },
            },
        }],
        terminator: Terminator {
            span: test_span(),
            kind: TerminatorKind::Return { value: None },
            unwind: UnwindAction::NoUnwind,
        },
    });
    body.start = bb;
    let file = File {
        items: vec![Item::Fun(FunDecl {
            span: test_span(),
            fqn: "fixtures.materialize.main".to_string(),
            name: "main".to_string(),
            ty: builtins.unit,
            params: Vec::new(),
            return_ty: builtins.unit,
            body: Some(body),
        })],
    };
    let materialized = materialized_for_test(file, types);

    let err = materialized.validate_materialized().unwrap_err();
    assert!(matches!(
        *err,
        MirMaterializeError::MaterializedMirValidation {
            error: crate::mir::MirValidationError::ProductionTransportMetadata {
                transport: "enum payload transport",
                detail: "enum payload aggregate variant is not declared",
                ..
            },
            ..
        }
    ));
}

#[test]
fn materialized_mir_typecheck_contract_rejects_non_bool_result() {
    let mut types = TypeStore::new();
    let builtins = types.intern_builtins();
    let mut body = Body::new_empty();
    let source = body.push_local(LocalDecl {
        span: test_span(),
        name: Some("source".to_string()),
        ty: builtins.any,
        source: LocalSourceKind::SourceLocal,
    });
    let target = body.push_local(LocalDecl {
        span: test_span(),
        name: Some("result".to_string()),
        ty: builtins.int,
        source: LocalSourceKind::CompilerTemporary,
    });
    let bb = body.push_block(BasicBlock {
        is_cleanup: false,
        stmts: vec![Statement {
            span: test_span(),
            kind: StatementKind::Assign {
                target,
                value: Rvalue::TypeCheck {
                    value: Operand::Local(source),
                    op: ast::TypeCheckOp::Is,
                    test_ty: builtins.string,
                    metadata: RuntimeTypeTestMetadata {
                        source_ty: builtins.any,
                        target_ty: builtins.string,
                        descriptor: RuntimeTypeDescriptorKey {
                            ty: builtins.string,
                            kind: RuntimeTypeDescriptorKind::String,
                        },
                        static_fold: RuntimeTypeStaticFold::Dynamic,
                        parameterized: RuntimeTypeParameterizedMatch::None,
                    },
                },
            },
        }],
        terminator: Terminator {
            span: test_span(),
            kind: TerminatorKind::Return { value: None },
            unwind: UnwindAction::NoUnwind,
        },
    });
    body.start = bb;
    let file = File {
        items: vec![Item::Fun(FunDecl {
            span: test_span(),
            fqn: "fixtures.materialize.main".to_string(),
            name: "main".to_string(),
            ty: builtins.unit,
            params: Vec::new(),
            return_ty: builtins.unit,
            body: Some(body),
        })],
    };
    let materialized = materialized_for_test(file, types);

    let err = materialized.validate_materialized().unwrap_err();
    assert!(matches!(
        *err,
        MirMaterializeError::MaterializedMirValidation {
            error: crate::mir::MirValidationError::TypeContract {
                surface: "typecheck result",
                detail: "typecheck result target must have Bool type",
                ..
            },
            ..
        }
    ));
}

#[test]
fn materialized_mir_cast_contract_rejects_asq_option_payload_drift() {
    let mut types = TypeStore::new();
    let builtins = types.intern_builtins();
    let option_int = types.ty_option(builtins.int);
    let mut body = Body::new_empty();
    let source = body.push_local(LocalDecl {
        span: test_span(),
        name: Some("source".to_string()),
        ty: builtins.any,
        source: LocalSourceKind::SourceLocal,
    });
    let target = body.push_local(LocalDecl {
        span: test_span(),
        name: Some("result".to_string()),
        ty: option_int,
        source: LocalSourceKind::CompilerTemporary,
    });
    let bb = body.push_block(BasicBlock {
        is_cleanup: false,
        stmts: vec![Statement {
            span: test_span(),
            kind: StatementKind::Assign {
                target,
                value: Rvalue::Cast {
                    value: Operand::Local(source),
                    op: ast::CastOp::AsQ,
                    target_ty: builtins.string,
                    metadata: RuntimeCastMetadata {
                        test: RuntimeTypeTestMetadata {
                            source_ty: builtins.any,
                            target_ty: builtins.string,
                            descriptor: RuntimeTypeDescriptorKey {
                                ty: builtins.string,
                                kind: RuntimeTypeDescriptorKind::String,
                            },
                            static_fold: RuntimeTypeStaticFold::Dynamic,
                            parameterized: RuntimeTypeParameterizedMatch::None,
                        },
                        failure: RuntimeCastFailure::ReturnNone,
                        result: RuntimeCastResult::Option {
                            option_ty: option_int,
                            some_ty: builtins.string,
                        },
                    },
                },
            },
        }],
        terminator: Terminator {
            span: test_span(),
            kind: TerminatorKind::Return { value: None },
            unwind: UnwindAction::NoUnwind,
        },
    });
    body.start = bb;
    let file = File {
        items: vec![Item::Fun(FunDecl {
            span: test_span(),
            fqn: "fixtures.materialize.main".to_string(),
            name: "main".to_string(),
            ty: builtins.unit,
            params: Vec::new(),
            return_ty: builtins.unit,
            body: Some(body),
        })],
    };
    let materialized = materialized_for_test(file, types);

    let err = materialized.validate_materialized().unwrap_err();
    assert!(matches!(
        *err,
        MirMaterializeError::MaterializedMirValidation {
            error: crate::mir::MirValidationError::ProductionRuntimeValueMetadata {
                primitive: "cast",
                detail: "`as?` Option payload type must match some type",
                ..
            },
            ..
        }
    ));
}

#[test]
fn materialized_mir_mir_materialize_generics_missing_root_reports_template_span() {
    let mut types = TypeStore::new();
    let builtins = types.intern_builtins();
    let typecheck_types = TypeStore::new();
    let template = generic_template_key();

    let err = match MirInstanceMaterializer::new(
        File { items: Vec::new() },
        types,
        builtins,
        MaterializerConstructionInputs {
            stable_cone_key: test_stable_cone_key(),
            typecheck_types: &typecheck_types,
            template_infos: vec![GenericTemplateInfo {
                request_lookup_key: (
                    template.fqn.clone(),
                    template.source_path.clone(),
                    template.decl_span,
                ),
                template: template.clone(),
                stable_template_key: test_stable_template_key(&template, "fun||id||Unit"),
                type_param_names: Vec::new(),
                eff_param_name: None,
                signature_key: "fun||id||Unit".to_string(),
                has_body: true,
            }],
            callable_body_infos: Vec::new(),
            callable_signatures: Vec::new(),
            known_receiver_subclasses: HashSet::new(),
            direct_subclasses: HashMap::new(),
            class_vtables: HashMap::new(),
            interfaces: HashMap::new(),
            class_itables: HashMap::new(),
            enum_layouts: HashMap::new(),
            extern_funs: HashMap::new(),
            native_callable_funs: HashMap::new(),
            top_level_fun_value_refs: HashMap::new(),
            top_level_fun_call_bindings: HashMap::new(),
            lowered_top_level_fun_call_bindings: HashMap::new(),
            ctor_call_sites: HashMap::new(),
            top_level_vars: HashMap::new(),
            top_level_immutable_values: HashMap::new(),
            object_inits: HashMap::new(),
            class_inits: HashMap::new(),
            member_value_tys: HashMap::new(),
            request_sources: HashSet::new(),
            request_root_mode: crate::mir::MaterializeRequestRootMode::RequestSources,
            request_root_fun_keys: Vec::new(),
        },
        OptLevel::O0,
    ) {
        Ok(_) => panic!("missing generic MIR root should be rejected"),
        Err(err) => err,
    };

    assert!(matches!(
        *err,
        MirMaterializeError::MissingMirRootForTemplate {
            fqn,
            span,
            call_site: None,
            ..
        } if fqn == "fixtures.materialize.id" && span == test_span()
    ));
}

#[test]
fn mir_materialize_generics_missing_template_reports_call_site() {
    let mut types = TypeStore::new();
    let builtins = types.intern_builtins();
    let mut typecheck_types = TypeStore::new();
    let typecheck_builtins = typecheck_types.intern_builtins();
    let template = generic_template_key();
    let fun = FunDecl {
        span: template.decl_span,
        fqn: template.fqn.clone(),
        name: "id".to_string(),
        ty: builtins.unit,
        params: Vec::new(),
        return_ty: builtins.unit,
        body: Some(unit_return_body()),
    };
    let call_site = Span::new(30, 40);
    let err = match MirInstanceMaterializer::new(
        File {
            items: vec![Item::Fun(fun)],
        },
        types,
        builtins,
        MaterializerConstructionInputs {
            stable_cone_key: test_stable_cone_key(),
            typecheck_types: &typecheck_types,
            template_infos: vec![GenericTemplateInfo {
                request_lookup_key: (
                    template.fqn.clone(),
                    template.source_path.clone(),
                    template.decl_span,
                ),
                template: template.clone(),
                stable_template_key: test_stable_template_key(&template, "fun||id||Unit"),
                type_param_names: Vec::new(),
                eff_param_name: None,
                signature_key: "fun||id||Unit".to_string(),
                has_body: true,
            }],
            callable_body_infos: Vec::new(),
            callable_signatures: vec![CallableSignatureInfo {
                stable_template_key: Some(test_stable_template_key(&template, "fun||id||Unit")),
                template,
                fun_ty: builtins.unit,
                return_ty: builtins.unit,
                params: Vec::new(),
                has_generic_params_or_effect_param: false,
            }],
            known_receiver_subclasses: HashSet::new(),
            direct_subclasses: HashMap::new(),
            class_vtables: HashMap::new(),
            interfaces: HashMap::new(),
            class_itables: HashMap::new(),
            enum_layouts: HashMap::new(),
            extern_funs: HashMap::new(),
            native_callable_funs: HashMap::new(),
            top_level_fun_value_refs: HashMap::new(),
            top_level_fun_call_bindings: HashMap::from([(
                (test_source_path(), call_site),
                ast::TopLevelFunCallBinding {
                    fqn: "fixtures.materialize.missing".to_string(),
                    decl_file: test_source_path(),
                    decl_span: test_span(),
                    is_intrinsic: false,
                    intrinsic_entry_name: None,
                    param_tys: Vec::new(),
                    return_ty: None,
                    type_args: vec![typecheck_builtins.int],
                    eff_args: Vec::new(),
                    types_are_hir: false,
                },
            )]),
            lowered_top_level_fun_call_bindings: HashMap::new(),
            ctor_call_sites: HashMap::new(),
            top_level_vars: HashMap::new(),
            top_level_immutable_values: HashMap::new(),
            object_inits: HashMap::new(),
            class_inits: HashMap::new(),
            member_value_tys: HashMap::new(),
            request_sources: HashSet::new(),
            request_root_mode: crate::mir::MaterializeRequestRootMode::RequestSources,
            request_root_fun_keys: Vec::new(),
        },
        OptLevel::O0,
    ) {
        Ok(_) => panic!("missing site template should be rejected"),
        Err(err) => err,
    };

    assert!(matches!(
        *err,
        MirMaterializeError::MissingGenericTemplate {
            fqn,
            call_site: Some(span),
            ..
        } if fqn == "fixtures.materialize.missing" && span == call_site
    ));
}

#[test]
#[ignore = "owner-eff/effect-row materialization is restored in Fact refactor batch 4 T4-04"]
fn materialized_mir_mir_materialize_generics_rejects_missing_effect_row_arg() {
    let (materializer, instance) =
        generic_materializer_for_body(unit_return_body(), Some("E".to_string()));

    let err = materializer.run(vec![instance]).unwrap_err();
    assert!(matches!(
        *err,
        MirMaterializeError::EffectArgArityMismatch {
            fqn,
            expected: 1,
            found: 0,
            ..
        } if fqn == "fixtures.materialize.id"
    ));
}

#[test]
fn materializer_filters_initial_monomorph_requests_by_call_site_source() {
    let sess = Session::new().unwrap();
    let main = SourceFile::new_virtual(
        "<mem>/request_source_main.scoop",
        r#"
package fixtures.materialize

fun main() {}
"#,
    );
    let support = SourceFile::new_virtual(
        "<mem>/request_source_support.scoop",
        r#"
package fixtures.materialize

fun <T> id(x: T): T {
return x
}

fun support(): Int {
return id<Int>(1)
}
"#,
    );
    let (files, index, env, types, monomorph_requests) =
        prepare_typechecked_compilation_unit_inputs(&sess, vec![main, support], &[0, 1]);
    let main_path = files[0].0.path().to_path_buf();
    let support_path = files[1].0.path().to_path_buf();
    assert!(
        monomorph_requests.iter().any(|request| {
            request.key.symbol.fqn == "fixtures.materialize.id"
                && request.request_source_path == support_path
        }),
        "test setup 应故意收集 support source 中的 id<Int> request"
    );

    let compilation_unit = files
        .iter()
        .map(|(source, ast)| (source, ast))
        .collect::<Vec<_>>();

    let main_only = crate::mir::materialize_compilation_unit_from_typechecked_inputs(
        &compilation_unit,
        std::slice::from_ref(&main_path),
        &index,
        Some(&env),
        &types,
        &monomorph_requests,
    )
    .unwrap();
    assert!(
        main_only
            .instance_keys
            .iter()
            .all(|key| key.template.fqn != "fixtures.materialize.id"),
        "support source 中收集到的 id<Int> request 不应在 main-only request roots 下成为 initial seed"
    );

    let support_roots = crate::mir::materialize_compilation_unit_from_typechecked_inputs(
        &compilation_unit,
        std::slice::from_ref(&support_path),
        &index,
        Some(&env),
        &types,
        &monomorph_requests,
    )
    .unwrap();
    assert!(
        support_roots
            .instance_keys
            .iter()
            .any(|key| key.template.fqn == "fixtures.materialize.id"),
        "同一个 request 来自 request source 时仍应正常进入 initial seeds"
    );
}

#[test]
fn generic_mir_template_for_dump_stays_free_of_hir_level_instances() {
    let sess = Session::new().unwrap();
    let source = SourceFile::new_virtual(
        "<mem>/materialize_generic_template_boundary.scoop",
        r#"
package fixtures.materialize

class Box<T>(val value: T) {
fun get(): T {
    return value
}
}

fun id<T>(x: T): T {
return x
}

fun entry(): Int {
val box: Box<Int> = Box(1)
val a = id(1)
return a + box.get()
}
"#,
    );

    let inputs = collect_dump_materialization_inputs(&sess, &source).unwrap();
    let compilation_unit = inputs
        .prepared_files
        .iter()
        .map(|file| (&file.source, &file.ast))
        .collect::<Vec<_>>();
    let stable_cone_key = StableConeKey::for_virtual_source_path(source.path());
    let mut lowered_hir = crate::hir::lower_generic_for_compilation_unit_multi_files_with_type_env(
        stable_cone_key,
        &inputs.index,
        &compilation_unit,
        &compilation_unit,
        Some(&inputs.env),
        &inputs.typecheck_types,
    )
    .unwrap();

    let hir_fun_fqns: Vec<&str> = lowered_hir
        .file
        .items
        .iter()
        .filter_map(|item| match item {
            crate::hir::Item::Fun(fun) => Some(fun.fqn.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert!(hir_fun_fqns.contains(&"fixtures.materialize.id"));
    assert!(hir_fun_fqns.contains(&"fixtures.materialize.entry"));
    assert!(
        hir_fun_fqns.iter().all(|fqn| !fqn.contains("::<")),
        "generic typed HIR 不应预先混入 standalone generic HIR instances: {hir_fun_fqns:?}"
    );

    let hir_member_fqns: Vec<&str> = lowered_hir
        .member_funs
        .iter()
        .map(|fun| fun.fqn.as_str())
        .collect::<Vec<_>>();
    assert!(hir_member_fqns.contains(&"fixtures.materialize.Box.get"));
    assert!(
        hir_member_fqns.iter().all(|fqn| !fqn.contains("::<")),
        "generic typed HIR 不应预先混入 owner-specialized member instances: {hir_member_fqns:?}"
    );

    let builtins = lowered_hir.types.intern_builtins();
    let hir_facts =
        scoopc_hir::stage::build_hir_facts_from_lowered_hir(&lowered_hir, source.path()).unwrap();
    let facts = MirLoweringFacts::from_hir_facts(&lowered_hir, &hir_facts);
    let generic_file = lower_hir_file_for_dump_with_facts(
        builtins,
        &mut lowered_hir.types,
        &lowered_hir.file,
        &lowered_hir.member_funs,
        &facts,
    );

    let mir_fun_fqns: Vec<&str> = generic_file
        .items
        .iter()
        .filter_map(|item| match item {
            Item::Fun(fun) => Some(fun.fqn.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert!(mir_fun_fqns.contains(&"fixtures.materialize.id"));
    assert!(mir_fun_fqns.contains(&"fixtures.materialize.Box.get"));
    assert!(
        mir_fun_fqns.iter().all(|fqn| !fqn.contains("::<")),
        "generic MIR template 不应在 materializer 之前混入 monomorphic roots: {mir_fun_fqns:?}"
    );
}

#[test]
fn materialize_for_dump_dedups_repeated_instance_requests() {
    let sess = Session::new().unwrap();
    let source = SourceFile::new_virtual(
        "<mem>/materialize_instance_dedup.scoop",
        r#"
package fixtures.materialize

fun id<T>(x: T): T {
return x
}

fun entry(): Int {
val a = id(1)
val b = id(2)
return a + b
}
"#,
    );

    let materialized = materialize_for_dump_with_opt_level(&sess, &source, OptLevel::O0).unwrap();
    let id_instances = materialized
        .instance_keys
        .iter()
        .filter(|key| key.template.fqn == "fixtures.materialize.id")
        .collect::<Vec<_>>();
    assert_eq!(
        id_instances.len(),
        1,
        "重复请求同一 generic instance 时应只保留一个 InstanceKey"
    );
    assert_eq!(
        materialized
            .file
            .items
            .iter()
            .filter(|item| matches!(
                item,
                Item::Fun(fun) if fun.fqn == "fixtures.materialize.id::<Int>"
            ))
            .count(),
        1,
        "per-InstanceKey cache 应确保同一实例只 materialize 一次"
    );
}

#[test]
fn typechecked_compilation_unit_materialization_distinguishes_same_type_args_with_different_effect_rows()
 {
    let sess = Session::new().unwrap();
    let source = SourceFile::new_virtual(
        "<mem>/materialize_compilation_unit_effect_rows.scoop",
        r#"
package fixtures.materialize

effect Boom {
fun ping(): Int
}

effect Zap {
fun pong(): Int
}

fun <T, eff E = Pure> wrap(x: T): T / E {
return x
}

fun entry(): Unit / (Boom + Zap) {
val a = wrap<Int, eff Boom>(1)
val b = wrap<Int, eff Zap>(2)
}
"#,
    );

    let inputs = collect_dump_materialization_inputs(&sess, &source).unwrap();
    let compilation_unit = inputs
        .prepared_files
        .iter()
        .map(|file| (&file.source, &file.ast))
        .collect::<Vec<_>>();
    let materialized = crate::mir::materialize_compilation_unit_from_typechecked_inputs(
        &compilation_unit,
        &[source.path().to_path_buf()],
        &inputs.index,
        Some(&inputs.env),
        &inputs.typecheck_types,
        &inputs.monomorph_requests,
    )
    .unwrap();

    let wrap_keys = materialized
        .instance_keys
        .iter()
        .filter(|key| key.template.fqn == "fixtures.materialize.wrap")
        .collect::<Vec<_>>();
    assert_eq!(wrap_keys.len(), 2);
    assert!(wrap_keys.iter().all(|key| key.type_args.len() == 1));
    assert!(wrap_keys.iter().all(|key| key.eff_args.len() == 1));
    assert!(
        materialized.file.items.iter().any(|item| matches!(
            item,
            Item::Fun(fun)
                if fun.fqn == "fixtures.materialize.wrap::<Int, eff fixtures.materialize.Boom>"
        )),
        "编译单元 materialization 应保留 Boom effect-row 实例"
    );
    assert!(
        materialized.file.items.iter().any(|item| matches!(
            item,
            Item::Fun(fun)
                if fun.fqn == "fixtures.materialize.wrap::<Int, eff fixtures.materialize.Zap>"
        )),
        "编译单元 materialization 应保留 Zap effect-row 实例"
    );
}

#[test]
fn mir_materialize_generics_covers_roots_effect_rows_and_call_rewrites() {
    let sess = Session::new().unwrap();
    let source = mir_fixture("generic_materialization.scoop");
    let materialized = materialize_for_dump_with_opt_level(&sess, &source, OptLevel::O0).unwrap();
    let boom = "mir_lowered.generic_materialization.Boom".to_string();

    let key = |template_fqn: &str| {
        materialized
            .instance_keys
            .iter()
            .find(|key| key.template.fqn == template_fqn)
            .unwrap_or_else(|| panic!("missing materialized instance for {template_fqn}"))
    };

    let top = key("mir_lowered.generic_materialization.top");
    assert_eq!(type_arg_names(&materialized, top), vec!["Int"]);
    assert_eq!(effect_arg_names(&materialized, top), vec![boom.clone()]);

    let capture = key("mir_lowered.generic_materialization.capture");
    assert_eq!(type_arg_names(&materialized, capture), vec!["Int"]);
    assert_eq!(effect_arg_names(&materialized, capture), vec![boom.clone()]);

    let pair = key("mir_lowered.generic_materialization.Box.pair");
    assert_eq!(type_arg_names(&materialized, pair), vec!["Int", "String"]);
    assert_eq!(effect_arg_names(&materialized, pair), vec![boom.clone()]);

    let extension = key("mir_lowered.generic_materialization.effectExt");
    assert!(extension.type_args.is_empty());
    assert_eq!(
        effect_arg_names(&materialized, extension),
        vec![boom.clone()]
    );

    let object_member = key("mir_lowered.generic_materialization.Tools.choose");
    assert_eq!(type_arg_names(&materialized, object_member), vec!["String"]);
    assert!(object_member.eff_args.is_empty());

    let fun_fqns = materialized
        .file
        .items
        .iter()
        .filter_map(|item| match item {
            Item::Fun(fun) => Some(fun.fqn.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>();
    for expected in [
        "mir_lowered.generic_materialization.top::<Int, eff mir_lowered.generic_materialization.Boom>",
        "mir_lowered.generic_materialization.capture::<Int, eff mir_lowered.generic_materialization.Boom>",
        "mir_lowered.generic_materialization.Box.pair::<Int, String, eff mir_lowered.generic_materialization.Boom>",
        "mir_lowered.generic_materialization.effectExt::<eff mir_lowered.generic_materialization.Boom>",
        "mir_lowered.generic_materialization.Tools.choose::<String>",
    ] {
        assert!(
            fun_fqns.contains(&expected),
            "missing materialized callable `{expected}` in {fun_fqns:#?}"
        );
    }

    let pass_view = materialized.pass_view();
    let entry = pass_view
        .callable("mir_lowered.generic_materialization.entry")
        .expect("request-root entry should be visible in materialized pass view");
    let direct_calls = direct_call_fqns(entry);
    for expected in [
        "mir_lowered.generic_materialization.Box.pair::<Int, String, eff mir_lowered.generic_materialization.Boom>",
        "mir_lowered.generic_materialization.Tools.choose::<String>",
        "mir_lowered.generic_materialization.effectExt::<eff mir_lowered.generic_materialization.Boom>",
        "mir_lowered.generic_materialization.capture::<Int, eff mir_lowered.generic_materialization.Boom>",
        "mir_lowered.generic_materialization.top::<Int, eff mir_lowered.generic_materialization.Boom>",
    ] {
        assert!(
            direct_calls.iter().any(|fqn| fqn == expected),
            "request-root call target `{expected}` should be rewritten to concrete materialized root; calls={direct_calls:#?}"
        );
    }
    for template in [
        "mir_lowered.generic_materialization.Box.pair",
        "mir_lowered.generic_materialization.Tools.choose",
        "mir_lowered.generic_materialization.effectExt",
        "mir_lowered.generic_materialization.capture",
        "mir_lowered.generic_materialization.top",
    ] {
        assert!(
            !direct_calls.iter().any(|fqn| fqn == template),
            "materialized pass view must not leave generic template direct-call target `{template}` in entry; calls={direct_calls:#?}"
        );
    }
    assert!(
        has_class_ctor_for_type(
            &materialized,
            entry,
            "mir_lowered.generic_materialization.Holder<Int>",
        ),
        "generic constructor surface should keep the concrete owner type in materialized pass view"
    );
}

#[test]
fn typechecked_compilation_unit_materialization_keeps_cross_file_effect_roots_when_request_sources_are_subset()
 {
    let sess = Session::new().unwrap();
    let helper_source = SourceFile::new_virtual(
        "<mem>/materialize_cross_file_helper.scoop",
        r#"
package fixtures.materialize

effect Boom {
fun ping(): Unit
}

fun <eff E = Pure> id(x: Int): Int / E {
return x
}

fun <eff E = Pure> wrap(x: Int): Int / E {
return id<eff E>(x)
}
"#,
    );
    let main_source = SourceFile::new_virtual(
        "<mem>/materialize_cross_file_main.scoop",
        r#"
package fixtures.materialize

fun entry(): Int / Boom {
return wrap<eff Boom>(1)
}
"#,
    );
    let main_source_path = main_source.path().to_path_buf();

    let (files, index, env, types, monomorph_requests) =
        prepare_typechecked_compilation_unit_inputs(&sess, vec![helper_source, main_source], &[1]);
    let compilation_unit = files
        .iter()
        .map(|(source, ast)| (source, ast))
        .collect::<Vec<_>>();

    let materialized = crate::mir::materialize_compilation_unit_from_typechecked_inputs(
        &compilation_unit,
        &[main_source_path],
        &index,
        Some(&env),
        &types,
        &monomorph_requests,
    )
    .unwrap();

    let wrap_keys = materialized
        .instance_keys
        .iter()
        .filter(|key| key.template.fqn == "fixtures.materialize.wrap")
        .collect::<Vec<_>>();
    let id_keys = materialized
        .instance_keys
        .iter()
        .filter(|key| key.template.fqn == "fixtures.materialize.id")
        .collect::<Vec<_>>();
    assert_eq!(wrap_keys.len(), 1);
    assert_eq!(id_keys.len(), 1);
    assert_eq!(wrap_keys[0].eff_args.len(), 1);
    assert_eq!(id_keys[0].eff_args.len(), 1);
    assert!(
        materialized.file.items.iter().any(|item| matches!(
            item,
            Item::Fun(fun)
                if fun.fqn == "fixtures.materialize.wrap::<eff fixtures.materialize.Boom>"
        )),
        "跨文件 helper 中定义的 wrap 应在编译单元 materialization 中保留 concrete root"
    );
    assert!(
        materialized.file.items.iter().any(|item| matches!(
            item,
            Item::Fun(fun)
                if fun.fqn == "fixtures.materialize.id::<eff fixtures.materialize.Boom>"
        )),
        "跨文件 helper 中嵌套调用的 id 应通过 helper 文件内的 site binding 继续 materialize"
    );
}

#[test]
fn typechecked_compilation_unit_materialization_skips_unreachable_generic_requests_from_non_request_sources()
 {
    let sess = Session::new().unwrap();
    let helper_source = SourceFile::new_virtual(
        "<mem>/materialize_unreachable_helper.scoop",
        r#"
package fixtures.materialize

fun <T> id(x: T): T {
return x
}

fun helperOnly(): Int {
return id(1)
}
"#,
    );
    let main_source = SourceFile::new_virtual(
        "<mem>/materialize_unreachable_main.scoop",
        r#"
package fixtures.materialize

fun entry(): Int {
return 0
}
"#,
    );
    let (files, index, env, types, monomorph_requests) =
        prepare_typechecked_compilation_unit_inputs(
            &sess,
            vec![helper_source, main_source.clone()],
            &[1],
        );
    let compilation_unit = files
        .iter()
        .map(|(source, ast)| (source, ast))
        .collect::<Vec<_>>();
    let request_source_paths = vec![main_source.path().to_path_buf()];
    let source_cones = HashMap::new();
    let stable_cone_key = StableConeKey::for_virtual_source_path(main_source.path());
    let materialized =
        crate::mir::materialize_compilation_unit_from_typechecked_inputs_with_options(
            &compilation_unit,
            &index,
            Some(&env),
            &types,
            &monomorph_requests,
            crate::mir::MaterializeCompilationUnitOptions {
                stable_cone_key: stable_cone_key.clone(),
                source_cones: &source_cones,
                request_source_paths: &request_source_paths,
                request_root_mode: crate::mir::MaterializeRequestRootMode::RequestSources,
                opt_level: OptLevel::O2,
            },
        )
        .unwrap();
    let lowered = crate::hir::lower_for_compilation_unit_multi_files_with_explicit_mir_instances(
        &index,
        &compilation_unit,
        &compilation_unit,
        Some(&env),
        &types,
        crate::hir::ExplicitMirInstanceLoweringOptions {
            stable_cone_key,
            source_cones: &source_cones,
            instance_keys: &materialized.instance_keys,
            instance_types: &materialized.types,
        },
    )
    .unwrap();

    assert!(
        lowered.file.items.iter().any(|item| matches!(
            item,
            crate::hir::Item::Fun(fun) if fun.fqn == "fixtures.materialize.helperOnly"
        )),
        "support source 仍应参与 lowering，保证 helper 实现体继续进入 HIR 兼容输出"
    );
    assert!(
        lowered.file.items.iter().all(|item| !matches!(
            item,
            crate::hir::Item::Fun(fun) if fun.fqn == "fixtures.materialize.id::<Int>"
        )),
        "未被 request-root 路径触达的 helper-only generic 实例不应被物化进 HIR 兼容输出"
    );
}

#[test]
fn request_root_scan_ignores_generic_calls_in_unreachable_mir_blocks() {
    let sess = Session::new().unwrap();
    let source = SourceFile::new_virtual(
        "<mem>/materialize_unreachable_mir_block.scoop",
        r#"
package fixtures.materialize

fun <T> id(x: T): T {
return x
}

fun main(): Int {
return 0
}
"#,
    );
    let source_path = source.path().to_path_buf();

    let (files, index, env, typecheck_types, monomorph_requests) =
        prepare_typechecked_compilation_unit_inputs(&sess, vec![source.clone()], &[0]);
    assert!(
        monomorph_requests
            .iter()
            .all(|request| request.key.symbol.fqn != "fixtures.materialize.id"),
        "test setup 不应通过源代码本身收集 id<T> request"
    );

    let compilation_unit = files
        .iter()
        .map(|(source, ast)| (source, ast))
        .collect::<Vec<_>>();
    let stable_cone_key = StableConeKey::for_virtual_source_path(&source_path);
    let template_infos =
        collect_generic_template_infos(&stable_cone_key, &index, &compilation_unit);
    let callable_body_infos = collect_callable_body_infos(&compilation_unit);
    let (top_level_fun_value_refs, top_level_fun_call_bindings) =
        collect_site_instance_bindings(&compilation_unit);
    let mut lowered_hir = crate::hir::lower_generic_for_compilation_unit_multi_files_with_type_env(
        stable_cone_key.clone(),
        &index,
        &compilation_unit,
        &compilation_unit,
        Some(&env),
        &typecheck_types,
    )
    .unwrap();
    let request_root_fun_keys = collect_request_root_fun_keys(
        &lowered_hir,
        std::slice::from_ref(&source_path),
        &index,
        crate::mir::MaterializeRequestRootMode::EntryMain { fqn: None },
    );
    assert_eq!(
        request_root_fun_keys
            .iter()
            .map(|key| key.fqn.as_str())
            .collect::<Vec<_>>(),
        vec!["fixtures.materialize.main"],
        "entry-main 模式下测试应只从 main 扫描 request roots"
    );
    let request_sources = [source_path.clone()].into_iter().collect::<HashSet<_>>();
    let callable_signatures = collect_callable_signature_infos(&lowered_hir);
    let hir_direct_instance_keys_by_fun = collect_hir_direct_call_instance_requests(
        &mut lowered_hir,
        &typecheck_types,
        &top_level_fun_call_bindings,
    );
    assert!(
        hir_direct_instance_keys_by_fun
            .values()
            .flatten()
            .all(|key| key.template.fqn != "fixtures.materialize.id"),
        "test setup 不应通过 HIR fallback 预先发现 id<T> 实例"
    );
    let known_receiver_subclasses =
        crate::mir::collect_known_receiver_subclasses(&lowered_hir.direct_supertypes);
    let direct_subclasses =
        collect_direct_subclasses_from_supertypes(&lowered_hir.direct_supertypes);
    let class_vtables = lowered_hir.class_vtables.clone();
    let interfaces = lowered_hir.interfaces.clone();
    let class_itables = lowered_hir.class_itables.clone();
    let builtins = lowered_hir.types.intern_builtins();
    let hir_facts =
        scoopc_hir::stage::build_hir_facts_from_lowered_hir(&lowered_hir, source_path.as_path())
            .unwrap();
    let facts = MirLoweringFacts::from_hir_facts(&lowered_hir, &hir_facts);
    let mut generic_file = lower_hir_file_for_dump_with_facts(
        builtins,
        &mut lowered_hir.types,
        &lowered_hir.file,
        &lowered_hir.member_funs,
        &facts,
    );
    append_unreachable_id_call_to_main(&mut generic_file, builtins);
    let top_level_vars = lowered_hir.top_level_vars.clone();
    let top_level_immutable_values = lowered_hir.top_level_immutable_values.clone();
    let object_inits = lowered_hir.object_inits.clone();
    let class_inits = lowered_hir.class_inits.clone();
    let enum_layouts = lowered_hir.enum_layouts.clone();
    let extern_funs = lowered_hir.extern_funs.clone();
    let native_callable_funs = lowered_hir.native_callable_funs.clone();
    let lowered_top_level_fun_call_bindings =
        collect_lowered_top_level_fun_call_bindings(&lowered_hir);
    let ctor_call_sites = lowered_hir.ctor_call_sites.clone();
    let member_value_tys = collect_member_value_type_infos_from_hir_decls(&lowered_hir.file.decls);
    let types = lowered_hir.types;

    let mut materializer = MirInstanceMaterializer::new(
        generic_file,
        types,
        builtins,
        MaterializerConstructionInputs {
            stable_cone_key,
            typecheck_types: &typecheck_types,
            template_infos,
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
            request_root_mode: crate::mir::MaterializeRequestRootMode::EntryMain { fqn: None },
            request_root_fun_keys,
        },
        OptLevel::O0,
    )
    .unwrap();
    materializer.hir_direct_instance_keys_by_fun = hir_direct_instance_keys_by_fun;
    let initial_requests = materializer
        .seed_requests(&typecheck_types, &monomorph_requests)
        .unwrap();
    let initial_id_keys = initial_requests
        .iter()
        .filter(|key| key.template.fqn == "fixtures.materialize.id")
        .collect::<Vec<_>>();
    assert!(
        initial_id_keys.is_empty(),
        "MIR 不可达 block 中的 id<Int> direct-call 不应进入 initial requests：{initial_id_keys:#?}"
    );
    assert!(
        initial_requests.is_empty(),
        "test setup 不应产生任何 initial requests：{initial_requests:#?}"
    );
    let materialized = materializer.run(initial_requests).unwrap();

    let id_keys = materialized
        .instance_keys
        .iter()
        .filter(|key| key.template.fqn == "fixtures.materialize.id")
        .collect::<Vec<_>>();
    assert!(
        id_keys.is_empty(),
        "MIR 不可达 block 中的 id<Int> direct-call 不应产生额外实例：{id_keys:#?}"
    );
    assert!(
        materialized.file.items.iter().all(|item| !matches!(
            item,
            Item::Fun(fun) if fun.fqn == "fixtures.materialize.id::<Int>"
        )),
        "MIR 不可达 block 中的 id<Int> direct-call 不应物化为 callable body"
    );
}

fn append_unreachable_id_call_to_main(generic_file: &mut File, builtins: BuiltinTypes) {
    let main_fun = generic_file
        .items
        .iter_mut()
        .find_map(|item| match item {
            Item::Fun(fun) if fun.fqn == "fixtures.materialize.main" => Some(fun),
            _ => None,
        })
        .expect("test setup should contain fixtures.materialize.main");
    let body = main_fun.body.as_mut().expect("main should have MIR body");
    let call_span = Span::new(10_000, 10_010);
    let result = body.push_local(LocalDecl {
        span: call_span,
        name: Some("unreachable_id_result".to_string()),
        ty: builtins.int,
        source: LocalSourceKind::SourceLocal,
    });
    let unreachable_block = body.push_block(crate::mir::BasicBlock {
        is_cleanup: false,
        stmts: vec![Statement {
            span: call_span,
            kind: StatementKind::Assign {
                target: result,
                value: Rvalue::Call {
                    site_id: crate::mir::SiteId::from_raw(0),
                    kind: CallKind::Direct {
                        callee_fqn: "fixtures.materialize.id".to_string(),
                    },
                    args: vec![CallArg {
                        span: call_span,
                        name: None,
                        value: Operand::Const(ConstValue::Int),
                    }],
                    transport: CallTransportMetadata::plain_no_outward(
                        builtins.int,
                        crate::mir::MirTransportKind::Unknown,
                    ),
                },
            },
        }],
        terminator: Terminator {
            span: call_span,
            kind: TerminatorKind::Return {
                value: Some(Operand::Local(result)),
            },
            unwind: crate::mir::UnwindAction::NoUnwind,
        },
    });

    assert!(
        body.unreachable_blocks()
            .unwrap()
            .contains(&unreachable_block),
        "test setup 应追加一个结构上不可达的 MIR block"
    );
}

#[test]
fn typechecked_compilation_unit_materialization_handles_owner_specialized_effect_generic_member_calls()
 {
    let sess = Session::new().unwrap();
    let source = SourceFile::new_virtual(
        "<mem>/materialize_owner_specialized_effect_member.scoop",
        r#"
package fixtures.materialize

effect Boom {
fun ping(): Unit
}

class Box<T>(val value: T) {
fun <eff E = Pure> forward(): T / E {
    return value
}
}

fun <eff E = Pure> wrap(box: Box<Int>): Int / E {
return box.forward<eff E>()
}

fun entry(): Int / Boom {
return wrap<eff Boom>(Box(1))
}
"#,
    );

    let (files, index, env, types, monomorph_requests) =
        prepare_typechecked_compilation_unit_inputs(&sess, vec![source.clone()], &[0]);
    let compilation_unit = files
        .iter()
        .map(|(source, ast)| (source, ast))
        .collect::<Vec<_>>();

    let materialized = crate::mir::materialize_compilation_unit_from_typechecked_inputs(
        &compilation_unit,
        &[source.path().to_path_buf()],
        &index,
        Some(&env),
        &types,
        &monomorph_requests,
    )
    .unwrap();

    let forward_keys = materialized
        .instance_keys
        .iter()
        .filter(|key| key.template.fqn == "fixtures.materialize.Box.forward")
        .collect::<Vec<_>>();
    assert_eq!(forward_keys.len(), 1);
    assert_eq!(forward_keys[0].type_args.len(), 1);
    assert_eq!(forward_keys[0].eff_args.len(), 1);
    assert!(
        materialized.file.items.iter().any(|item| matches!(
            item,
            Item::Fun(fun)
                if fun.fqn
                    == "fixtures.materialize.Box.forward::<Int, eff fixtures.materialize.Boom>"
        )),
        "generic owner + effect-generic member direct-call 应产出同时携带 owner args 与 eff_args 的 concrete MIR root"
    );
}

#[test]
fn typechecked_compilation_unit_materialization_seeds_owner_specialized_getter_from_request_roots()
{
    let sess = Session::new().unwrap();
    let source = SourceFile::new_virtual(
        "<mem>/materialize_owner_specialized_getter.scoop",
        r#"
package fixtures.materialize

struct Box<T>(val value: T) {
val doubled: T
    get() = this.value
}

fun entry(): Int {
val box: Box<Int> = Box(1)
val unused: Box<String> = Box("x")
return box.doubled
}
"#,
    );

    let (files, index, env, types, monomorph_requests) =
        prepare_typechecked_compilation_unit_inputs(&sess, vec![source.clone()], &[0]);
    let compilation_unit = files
        .iter()
        .map(|(source, ast)| (source, ast))
        .collect::<Vec<_>>();

    let materialized = crate::mir::materialize_compilation_unit_from_typechecked_inputs(
        &compilation_unit,
        &[source.path().to_path_buf()],
        &index,
        Some(&env),
        &types,
        &monomorph_requests,
    )
    .unwrap();

    let getter_keys = materialized
        .instance_keys
        .iter()
        .filter(|key| key.template.fqn == "fixtures.materialize.Box.doubled")
        .collect::<Vec<_>>();
    assert_eq!(getter_keys.len(), 1);
    assert_eq!(getter_keys[0].type_args.len(), 1);
    assert!(
        materialized.file.items.iter().any(|item| matches!(
            item,
            Item::Fun(fun) if fun.fqn == "fixtures.materialize.Box.doubled::<Int>"
        )),
        "generic owner getter 应从请求根非调用式访问进入 materialization"
    );
    assert!(
        !materialized.file.items.iter().any(|item| matches!(
            item,
            Item::Fun(fun) if fun.fqn == "fixtures.materialize.Box.doubled::<String>"
        )),
        "请求根扫描应保持 call-site driven，不应因为 `Box<String>` 出现在 TypeStore 中就 eager materialize 未调用 getter"
    );
}

#[test]
fn typechecked_compilation_unit_materialization_reaches_owner_specialized_getter_through_cross_file_non_generic_helper()
 {
    let sess = Session::new().unwrap();
    let helper = SourceFile::new_virtual(
        "<mem>/materialize_owner_specialized_getter_helper.scoop",
        r#"
package fixtures.materialize

struct Box<T>(val value: T) {
val doubled: T
    get() = this.value
}

fun helper(box: Box<Int>): Int {
return box.doubled
}
"#,
    );
    let main = SourceFile::new_virtual(
        "<mem>/materialize_owner_specialized_getter_main.scoop",
        r#"
package fixtures.materialize

fun entry(): Int {
return helper(Box(1))
}
"#,
    );

    let (files, index, env, types, monomorph_requests) =
        prepare_typechecked_compilation_unit_inputs(&sess, vec![helper, main.clone()], &[1]);
    let compilation_unit = files
        .iter()
        .map(|(source, ast)| (source, ast))
        .collect::<Vec<_>>();

    let materialized = crate::mir::materialize_compilation_unit_from_typechecked_inputs(
        &compilation_unit,
        &[main.path().to_path_buf()],
        &index,
        Some(&env),
        &types,
        &monomorph_requests,
    )
    .unwrap();

    let getter_keys = materialized
        .instance_keys
        .iter()
        .filter(|key| key.template.fqn == "fixtures.materialize.Box.doubled")
        .collect::<Vec<_>>();
    assert_eq!(getter_keys.len(), 1);
    assert!(
        materialized.file.items.iter().any(|item| matches!(
            item,
            Item::Fun(fun) if fun.fqn == "fixtures.materialize.Box.doubled::<Int>"
        )),
        "跨文件非泛型 helper 中触发的 owner-specialized getter 应继续进入 MIR materialization"
    );
}

#[test]
fn typechecked_compilation_unit_materialization_reaches_owner_specialized_getter_through_non_generic_helper_called_by_generic_instance()
 {
    let sess = Session::new().unwrap();
    let helper = SourceFile::new_virtual(
        "<mem>/materialize_owner_specialized_getter_helper_via_generic_instance.scoop",
        r#"
package fixtures.materialize

struct Box<T>(val value: T) {
val doubled: T
    get() = this.value
}

fun helper(box: Box<Int>): Int {
return box.doubled
}
"#,
    );
    let main = SourceFile::new_virtual(
        "<mem>/materialize_owner_specialized_getter_generic_instance_main.scoop",
        r#"
package fixtures.materialize

fun <eff E = Pure> wrap(box: Box<Int>): Int / E {
return helper(box)
}

fun entry(): Int {
return wrap(Box(1))
}
"#,
    );

    let (files, index, env, types, monomorph_requests) =
        prepare_typechecked_compilation_unit_inputs(&sess, vec![helper, main.clone()], &[1]);
    let compilation_unit = files
        .iter()
        .map(|(source, ast)| (source, ast))
        .collect::<Vec<_>>();

    let materialized = crate::mir::materialize_compilation_unit_from_typechecked_inputs(
        &compilation_unit,
        &[main.path().to_path_buf()],
        &index,
        Some(&env),
        &types,
        &monomorph_requests,
    )
    .unwrap();

    let getter_keys = materialized
        .instance_keys
        .iter()
        .filter(|key| key.template.fqn == "fixtures.materialize.Box.doubled")
        .collect::<Vec<_>>();
    assert_eq!(getter_keys.len(), 1);
    assert!(
        materialized.file.items.iter().any(|item| matches!(
            item,
            Item::Fun(fun) if fun.fqn == "fixtures.materialize.Box.doubled::<Int>"
        )),
        "generic instance 经由非泛型 helper 可达的 owner-specialized getter 应继续进入 MIR materialization"
    );
}

#[test]
fn dump_materialization_inputs_keep_eff_args_for_extension_direct_call_binding() {
    let sess = Session::new().unwrap();
    let source = SourceFile::new_virtual(
        "<mem>/materialize_extension_binding_effect.scoop",
        r#"
package fixtures.materialize

effect Boom {
fun ping(): Unit
}

fun <eff E = Pure> Int.forward(): Int / E {
return this
}

fun <eff E = Pure> wrap(x: Int): Int / E {
return x.forward<eff E>()
}

fun entry(): Int / Boom {
return wrap<eff Boom>(1)
}
"#,
    );

    let inputs = collect_dump_materialization_inputs(&sess, &source).unwrap();
    let compilation_unit = inputs
        .prepared_files
        .iter()
        .map(|file| (&file.source, &file.ast))
        .collect::<Vec<_>>();
    let (_value_refs, call_bindings) = collect_site_instance_bindings(&compilation_unit);
    let bindings = call_bindings
        .values()
        .filter(|binding| binding.fqn == "fixtures.materialize.forward")
        .collect::<Vec<_>>();
    assert_eq!(bindings.len(), 1);
    let binding = bindings[0];
    assert_eq!(binding.decl_file, source.path().to_path_buf());
    assert!(binding.decl_span.start < binding.decl_span.end);
    assert!(binding.type_args.is_empty());
    assert_eq!(binding.eff_args.len(), 1);
    assert!(
        !binding.eff_args[0].is_pure(),
        "extension direct-call 的 TopLevelFunCallBinding 不应退回 Pure"
    );

    let keys = inputs
        .monomorph_requests
        .iter()
        .filter(|request| request.key.symbol.fqn == "fixtures.materialize.forward")
        .collect::<Vec<_>>();
    assert_eq!(keys.len(), 1);
    assert!(keys[0].key.type_args.is_empty());
    assert_eq!(keys[0].key.eff_args.len(), 1);
    assert!(
        !keys[0].key.eff_args[0].is_pure(),
        "extension direct-call 的 monomorph key 应保留非 Pure 的 eff_args"
    );
}

#[test]
fn dump_materialization_inputs_keep_eff_args_for_member_direct_call_binding() {
    let sess = Session::new().unwrap();
    let source = SourceFile::new_virtual(
        "<mem>/materialize_member_binding_effect.scoop",
        r#"
package fixtures.materialize

effect Boom {
fun ping(): Unit
}

class Box() {
fun <eff E = Pure> forward(): Int / E {
    return 1
}
}

fun <eff E = Pure> wrap(box: Box): Int / E {
return box.forward<eff E>()
}

fun entry(): Int / Boom {
return wrap<eff Boom>(Box())
}
"#,
    );

    let inputs = collect_dump_materialization_inputs(&sess, &source).unwrap();
    let compilation_unit = inputs
        .prepared_files
        .iter()
        .map(|file| (&file.source, &file.ast))
        .collect::<Vec<_>>();
    let (_value_refs, call_bindings) = collect_site_instance_bindings(&compilation_unit);
    let bindings = call_bindings
        .values()
        .filter(|binding| binding.fqn == "fixtures.materialize.Box.forward")
        .collect::<Vec<_>>();
    assert_eq!(bindings.len(), 1);
    let binding = bindings[0];
    assert_eq!(binding.decl_file, source.path().to_path_buf());
    assert!(binding.decl_span.start < binding.decl_span.end);
    assert!(binding.type_args.is_empty());
    assert_eq!(binding.eff_args.len(), 1);
    assert!(
        !binding.eff_args[0].is_pure(),
        "成员 direct-call 的 TopLevelFunCallBinding 不应退回 Pure"
    );

    let keys = inputs
        .monomorph_requests
        .iter()
        .filter(|request| request.key.symbol.fqn == "fixtures.materialize.Box.forward")
        .collect::<Vec<_>>();
    assert_eq!(keys.len(), 1);
    assert!(keys[0].key.type_args.is_empty());
    assert_eq!(keys[0].key.eff_args.len(), 1);
    assert!(
        !keys[0].key.eff_args[0].is_pure(),
        "成员 direct-call 的 monomorph key 应保留非 Pure 的 eff_args"
    );
}

#[test]
fn dump_materialization_inputs_keep_eff_args_for_member_direct_call_binding_from_lambda() {
    let sess = Session::new().unwrap();
    let source = SourceFile::new_virtual(
        "<mem>/materialize_member_lambda_binding_effect.scoop",
        r#"
package fixtures.materialize

effect Boom {
fun ping(): Unit
}

class Box() {
fun <eff E = Pure> lift(f: () -> Int / E): Int / E {
    return f()
}
}

fun entry(): Int / Boom {
val box: Box = Box()
return box.lift({
    Boom.ping()
    1
})
}
"#,
    );

    let inputs = collect_dump_materialization_inputs(&sess, &source).unwrap();
    let compilation_unit = inputs
        .prepared_files
        .iter()
        .map(|file| (&file.source, &file.ast))
        .collect::<Vec<_>>();
    let (_value_refs, call_bindings) = collect_site_instance_bindings(&compilation_unit);
    let bindings = call_bindings
        .values()
        .filter(|binding| binding.fqn == "fixtures.materialize.Box.lift")
        .collect::<Vec<_>>();
    assert_eq!(bindings.len(), 1);
    let binding = bindings[0];
    assert_eq!(binding.decl_file, source.path().to_path_buf());
    assert!(binding.decl_span.start < binding.decl_span.end);
    assert!(binding.type_args.is_empty());
    assert_eq!(binding.eff_args.len(), 1);
    assert!(
        !binding.eff_args[0].is_pure(),
        "lambda-derived 成员 direct-call binding 应保留非 Pure eff_args"
    );

    let keys = inputs
        .monomorph_requests
        .iter()
        .filter(|request| request.key.symbol.fqn == "fixtures.materialize.Box.lift")
        .collect::<Vec<_>>();
    assert_eq!(keys.len(), 1);
    assert!(keys[0].key.type_args.is_empty());
    assert_eq!(keys[0].key.eff_args.len(), 1);
    assert!(
        !keys[0].key.eff_args[0].is_pure(),
        "lambda-derived 成员 direct-call monomorph key 应保留非 Pure eff_args"
    );
}

#[test]
fn dump_materialization_inputs_keep_owner_type_args_and_eff_args_for_operator_overload_binding() {
    let sess = Session::new().unwrap();
    let source = SourceFile::new_virtual(
        "<mem>/materialize_operator_overload_binding_effect.scoop",
        r#"
package fixtures.materialize

effect Boom {
fun ping(): Unit
}

struct Box<T>(val value: Int) {
operator fun <eff E = Boom> plus(other: Box<T>): Box<T> / Boom {
    Boom.ping()
    return Box { value: this.value + other.value }
}
}

fun entry(): Box<Int> / Boom {
val lhs: Box<Int> = Box { value: 1 }
val rhs: Box<Int> = Box { value: 2 }
return lhs + rhs
}
"#,
    );

    let mut inputs = collect_dump_materialization_inputs(&sess, &source).unwrap();
    let compilation_unit = inputs
        .prepared_files
        .iter()
        .map(|file| (&file.source, &file.ast))
        .collect::<Vec<_>>();
    let (_value_refs, call_bindings) = collect_site_instance_bindings(&compilation_unit);
    let builtins = inputs.typecheck_types.intern_builtins();

    let bindings = call_bindings
        .values()
        .filter(|binding| binding.fqn == "fixtures.materialize.Box.plus")
        .collect::<Vec<_>>();
    assert_eq!(bindings.len(), 1);
    let binding = bindings[0];
    assert_eq!(binding.decl_file, source.path().to_path_buf());
    assert!(binding.decl_span.start < binding.decl_span.end);
    assert_eq!(binding.type_args.len(), 1);
    assert_eq!(
        binding.type_args[0], builtins.int,
        "operator-overload binding 应保留 owner specialization 的 Int type arg"
    );
    assert_eq!(binding.eff_args.len(), 1);
    assert!(
        !binding.eff_args[0].is_pure(),
        "operator-overload binding 不应把默认 `Boom` eff_arg 退回 Pure"
    );

    let keys = inputs
        .monomorph_requests
        .iter()
        .filter(|request| request.key.symbol.fqn == "fixtures.materialize.Box.plus")
        .collect::<Vec<_>>();
    assert_eq!(keys.len(), 1);
    assert_eq!(keys[0].key.type_args.len(), 1);
    assert_eq!(
        keys[0].key.type_args[0], builtins.int,
        "operator-overload monomorph key 应保留 owner specialization 的 Int type arg"
    );
    assert_eq!(keys[0].key.eff_args.len(), 1);
    assert!(
        !keys[0].key.eff_args[0].is_pure(),
        "operator-overload monomorph key 不应把默认 `Boom` eff_arg 退回 Pure"
    );
}

#[test]
fn dump_materialization_inputs_keep_owner_type_args_and_eff_args_for_compare_to_binding() {
    let sess = Session::new().unwrap();
    let source = SourceFile::new_virtual(
        "<mem>/materialize_compare_to_binding_effect.scoop",
        r#"
package fixtures.materialize

effect Boom {
fun ping(): Unit
}

struct Box<T>(val value: Int) {
operator fun <eff E = Boom> compareTo(other: Box<T>): Int / Boom {
    Boom.ping()
    return this.value - other.value
}
}

fun entry(): Bool / Boom {
val lhs: Box<Int> = Box { value: 1 }
val rhs: Box<Int> = Box { value: 2 }
return lhs < rhs
}
"#,
    );

    let mut inputs = collect_dump_materialization_inputs(&sess, &source).unwrap();
    let compilation_unit = inputs
        .prepared_files
        .iter()
        .map(|file| (&file.source, &file.ast))
        .collect::<Vec<_>>();
    let (_value_refs, call_bindings) = collect_site_instance_bindings(&compilation_unit);
    let builtins = inputs.typecheck_types.intern_builtins();

    let bindings = call_bindings
        .values()
        .filter(|binding| binding.fqn == "fixtures.materialize.Box.compareTo")
        .collect::<Vec<_>>();
    assert_eq!(bindings.len(), 1);
    let binding = bindings[0];
    assert_eq!(binding.decl_file, source.path().to_path_buf());
    assert!(binding.decl_span.start < binding.decl_span.end);
    assert_eq!(binding.type_args.len(), 1);
    assert_eq!(
        binding.type_args[0], builtins.int,
        "compareTo binding 应保留 owner specialization 的 Int type arg"
    );
    assert_eq!(binding.eff_args.len(), 1);
    assert!(
        !binding.eff_args[0].is_pure(),
        "compareTo binding 不应把默认 `Boom` eff_arg 退回 Pure"
    );

    let keys = inputs
        .monomorph_requests
        .iter()
        .filter(|request| request.key.symbol.fqn == "fixtures.materialize.Box.compareTo")
        .collect::<Vec<_>>();
    assert_eq!(keys.len(), 1);
    assert_eq!(keys[0].key.type_args.len(), 1);
    assert_eq!(
        keys[0].key.type_args[0], builtins.int,
        "compareTo monomorph key 应保留 owner specialization 的 Int type arg"
    );
    assert_eq!(keys[0].key.eff_args.len(), 1);
    assert!(
        !keys[0].key.eff_args[0].is_pure(),
        "compareTo monomorph key 不应把默认 `Boom` eff_arg 退回 Pure"
    );
}

#[test]
fn dump_materialization_inputs_keep_precise_type_args_for_object_member_call_results() {
    let sess = Session::new().unwrap();
    let source = SourceFile::new_virtual(
        "<mem>/materialize_object_member_call_binding.scoop",
        r#"
package fixtures.materialize

import scoop.core.*

object Helper {
fun run(seed: Int): Int {
    println(seed)
    return seed + 1
}
}

fun main(): Int {
val result: Int = Helper.run(41)
println(result)
return 0
}
"#,
    );

    let mut inputs = collect_dump_materialization_inputs(&sess, &source).unwrap();
    let compilation_unit = inputs
        .prepared_files
        .iter()
        .map(|file| (&file.source, &file.ast))
        .collect::<Vec<_>>();
    let (_value_refs, call_bindings) = collect_site_instance_bindings(&compilation_unit);
    let builtins = inputs.typecheck_types.intern_builtins();

    let println_type_args = call_bindings
        .iter()
        .filter(|((site_path, _), binding)| {
            *site_path == source.path() && binding.fqn == "scoop.core.println"
        })
        .map(|(_, binding)| {
            assert_eq!(binding.type_args.len(), 1);
            binding.type_args[0]
        })
        .collect::<Vec<_>>();
    assert_eq!(
        println_type_args.len(),
        2,
        "object member call 场景中应记录 2 个用户 println 调用"
    );
    assert!(
        println_type_args.iter().all(|&ty| ty == builtins.int),
        "object member call 场景中的 println binding 不应退回 Any：{println_type_args:?}"
    );

    let println_monomorph_type_args = inputs
        .monomorph_requests
        .iter()
        .filter(|request| request.key.symbol.fqn == "scoop.core.println")
        .map(|request| {
            assert_eq!(request.key.type_args.len(), 1);
            request.key.type_args[0]
        })
        .collect::<Vec<_>>();
    assert!(
        !println_monomorph_type_args.is_empty(),
        "object member call 场景中至少应保留 request-root 上的 println monomorph key"
    );
    assert!(
        println_monomorph_type_args
            .iter()
            .all(|&ty| ty == builtins.int),
        "object member call 场景中的 println monomorph key 不应退回 Any：{println_monomorph_type_args:?}"
    );

    let materialized = crate::mir::materialize_compilation_unit_from_typechecked_inputs(
        &compilation_unit,
        &[source.path().to_path_buf()],
        &inputs.index,
        Some(&inputs.env),
        &inputs.typecheck_types,
        &inputs.monomorph_requests,
    )
    .unwrap();
    let materialized_printlns = materialized
        .file
        .items
        .iter()
        .filter_map(|item| match item {
            Item::Fun(fun) if fun.fqn.starts_with("scoop.core.println::<") => Some(fun.fqn.clone()),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert!(
        materialized_printlns
            .iter()
            .filter(|fqn| *fqn == "scoop.core.println::<Int>")
            .count()
            >= 1,
        "object member call 场景中应 materialize 出 println::<Int>：{materialized_printlns:#?}"
    );
    assert!(
        !materialized_printlns
            .iter()
            .any(|fqn| fqn == "scoop.core.println::<Any>"),
        "object member call 场景中不应 materialize 出 println::<Any>：{materialized_printlns:#?}"
    );
}

#[test]
fn dump_materialization_inputs_keep_precise_type_args_for_chained_member_access_call_args() {
    let sess = Session::new().unwrap();
    let source = SourceFile::new_virtual(
        "<mem>/materialize_chained_member_access_binding.scoop",
        r#"
package fixtures.materialize

import scoop.core.*

struct Tag(val label: String, val score: Int)

class Node(val name: String, val tag: Tag, val value: Int)

class Holder(val node: Node)

fun makeHolder(): Holder {
val node: Node = Node("root", Tag { label: "alpha", score: 7 }, 42)
return Holder(node)
}

fun main() {
val holder: Holder = makeHolder()
val label: String = holder.node.tag.label
println(label)
println(holder.node.tag.label)
println(holder.node.tag.score)
}
"#,
    );

    let mut inputs = collect_dump_materialization_inputs(&sess, &source).unwrap();
    let compilation_unit = inputs
        .prepared_files
        .iter()
        .map(|file| (&file.source, &file.ast))
        .collect::<Vec<_>>();
    let (_value_refs, call_bindings) = collect_site_instance_bindings(&compilation_unit);
    let builtins = inputs.typecheck_types.intern_builtins();

    let println_type_args = call_bindings
        .iter()
        .filter(|((site_path, _), binding)| {
            *site_path == source.path() && binding.fqn == "scoop.core.println"
        })
        .map(|(_, binding)| {
            assert_eq!(binding.type_args.len(), 1);
            binding.type_args[0]
        })
        .collect::<Vec<_>>();

    assert_eq!(println_type_args.len(), 3);
    assert!(
        !println_type_args.contains(&builtins.any),
        "链式成员访问作为实参时，println 不应退回到 `Any` 实例"
    );
    assert_eq!(
        println_type_args
            .iter()
            .filter(|&&ty| ty == builtins.string)
            .count(),
        2,
        "label 与 holder.node.tag.label 都应绑定到 println::<String>"
    );
    assert_eq!(
        println_type_args
            .iter()
            .filter(|&&ty| ty == builtins.int)
            .count(),
        1,
        "holder.node.tag.score 应绑定到 println::<Int>"
    );

    let println_monomorph_type_args = inputs
        .monomorph_requests
        .iter()
        .filter(|request| request.key.symbol.fqn == "scoop.core.println")
        .map(|request| {
            assert_eq!(request.key.type_args.len(), 1);
            request.key.type_args[0]
        })
        .collect::<Vec<_>>();
    assert!(
        !println_monomorph_type_args.contains(&builtins.any),
        "链式成员访问作为实参时，println 的 monomorph key 不应退回到 `Any`"
    );

    let stable_cone_key = StableConeKey::for_virtual_source_path(source.path());
    let template_catalog =
        collect_generic_template_infos(&stable_cone_key, &inputs.index, &compilation_unit);
    let callable_body_infos = collect_callable_body_infos(&compilation_unit);
    let (top_level_fun_value_refs, top_level_fun_call_bindings) =
        collect_site_instance_bindings(&compilation_unit);
    let mut lowered_hir = crate::hir::lower_generic_for_compilation_unit_multi_files_with_type_env(
        stable_cone_key.clone(),
        &inputs.index,
        &compilation_unit,
        &compilation_unit,
        Some(&inputs.env),
        &inputs.typecheck_types,
    )
    .unwrap();
    let request_root_fun_keys = collect_request_root_fun_keys(
        &lowered_hir,
        &[source.path().to_path_buf()],
        &inputs.index,
        crate::mir::MaterializeRequestRootMode::RequestSources,
    );
    let request_sources = [source.path().to_path_buf()]
        .into_iter()
        .collect::<HashSet<_>>();
    let callable_signatures = collect_callable_signature_infos(&lowered_hir);
    let hir_direct_instance_keys_by_fun = collect_hir_direct_call_instance_requests(
        &mut lowered_hir,
        &inputs.typecheck_types,
        &call_bindings,
    );
    let hir_direct_instance_keys = hir_direct_instance_keys_by_fun
        .values()
        .flatten()
        .cloned()
        .collect::<Vec<_>>();
    let hir_direct_println_requests = hir_direct_instance_keys
        .iter()
        .filter(|key| key.template.fqn == "scoop.core.println")
        .map(|key| {
            (
                key.template.source_path.clone(),
                key.template.decl_span,
                key.type_args.clone(),
                key.eff_args.clone(),
            )
        })
        .collect::<Vec<_>>();
    let known_receiver_subclasses =
        crate::mir::collect_known_receiver_subclasses(&lowered_hir.direct_supertypes);
    let direct_subclasses =
        collect_direct_subclasses_from_supertypes(&lowered_hir.direct_supertypes);
    let class_vtables = lowered_hir.class_vtables.clone();
    let interfaces = lowered_hir.interfaces.clone();
    let class_itables = lowered_hir.class_itables.clone();
    let builtins = lowered_hir.types.intern_builtins();
    let hir_facts =
        scoopc_hir::stage::build_hir_facts_from_lowered_hir(&lowered_hir, source.path()).unwrap();
    let facts = MirLoweringFacts::from_hir_facts(&lowered_hir, &hir_facts);
    let generic_file = lower_hir_file_for_dump_with_facts(
        builtins,
        &mut lowered_hir.types,
        &lowered_hir.file,
        &lowered_hir.member_funs,
        &facts,
    );
    let top_level_vars = lowered_hir.top_level_vars.clone();
    let top_level_immutable_values = lowered_hir.top_level_immutable_values.clone();
    let object_inits = lowered_hir.object_inits.clone();
    let class_inits = lowered_hir.class_inits.clone();
    let enum_layouts = lowered_hir.enum_layouts.clone();
    let extern_funs = lowered_hir.extern_funs.clone();
    let native_callable_funs = lowered_hir.native_callable_funs.clone();
    let lowered_top_level_fun_call_bindings =
        collect_lowered_top_level_fun_call_bindings(&lowered_hir);
    let ctor_call_sites = lowered_hir.ctor_call_sites.clone();
    let member_value_tys = collect_member_value_type_infos_from_hir_decls(&lowered_hir.file.decls);
    let types = lowered_hir.types;
    let mut materializer = MirInstanceMaterializer::new(
        generic_file,
        types,
        builtins,
        MaterializerConstructionInputs {
            stable_cone_key,
            typecheck_types: &inputs.typecheck_types,
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
            request_root_mode: crate::mir::MaterializeRequestRootMode::RequestSources,
            request_root_fun_keys,
        },
        OptLevel::O2,
    )
    .unwrap();
    materializer.hir_direct_instance_keys_by_fun = hir_direct_instance_keys_by_fun;
    let request_root_println_bindings = materializer
        .request_root_funs
        .iter()
        .flat_map(|reachable_fun| {
            reachable_fun
                .fun
                .body
                .iter()
                .flat_map(|body| body.blocks.iter())
                .flat_map(|block| block.stmts.iter())
                .filter_map(|stmt| match &stmt.kind {
                    StatementKind::Assign {
                        value:
                            Rvalue::Call {
                                kind: CallKind::Direct { callee_fqn },
                                ..
                            },
                        ..
                    } if callee_fqn == "scoop.core.println" => Some((
                        reachable_fun.source_path.clone(),
                        stmt.span,
                        materializer
                            .lookup_site_instance_binding(&reachable_fun.source_path, stmt.span)
                            .map(|binding| binding.type_args.clone()),
                    )),
                    _ => None,
                })
        })
        .collect::<Vec<_>>();
    assert_eq!(
        request_root_println_bindings.len(),
        3,
        "request root 中应恰好看到 3 个用户 println 调用：{request_root_println_bindings:#?}"
    );
    assert!(
        request_root_println_bindings
            .iter()
            .all(|(_, _, binding)| binding.is_some()),
        "request root 的 println 调用必须全部命中 site binding：{request_root_println_bindings:#?}"
    );
    assert!(
        request_root_println_bindings.iter().all(|(_, _, binding)| {
            binding
                .as_ref()
                .is_some_and(|type_args| !type_args.contains(&builtins.any))
        }),
        "request root 的 println 调用命中的 binding 不应含 Any：{request_root_println_bindings:#?}"
    );
    let mut reachable_generic_calls = Vec::new();
    let mut visited_non_generic = std::collections::HashSet::new();
    let mut stack = materializer.request_root_funs.clone();
    while let Some(reachable_fun) = stack.pop() {
        let scan_key = (reachable_fun.source_path.clone(), reachable_fun.fun.span);
        if !visited_non_generic.insert(scan_key) {
            continue;
        }
        let Some(body) = &reachable_fun.fun.body else {
            continue;
        };
        for block in &body.blocks {
            for stmt in &block.stmts {
                let StatementKind::Assign {
                    value:
                        Rvalue::Call {
                            kind: CallKind::Direct { callee_fqn },
                            args,
                            ..
                        },
                    ..
                } = &stmt.kind
                else {
                    continue;
                };
                if let Some(instance_key) =
                    materializer.infer_direct_call_instance(DirectCallInferenceInput {
                        template_source_path: &reachable_fun.source_path,
                        call_span: stmt.span,
                        callee_fqn,
                        args,
                        result_ty: None,
                        locals: &body.locals,
                        substitution: &InstanceSubstitution::default(),
                    })
                {
                    reachable_generic_calls.push((
                        reachable_fun.fun.fqn.clone(),
                        reachable_fun.source_path.clone(),
                        stmt.span,
                        materializer.instance_display_fqn(&instance_key),
                    ));
                    continue;
                }
                if let Some(reachable_callee) = materializer.resolve_non_generic_direct_callee(
                    &reachable_fun.source_path,
                    stmt.span,
                    callee_fqn,
                    args,
                    &body.locals,
                ) {
                    stack.push(reachable_callee);
                }
            }
        }
    }
    let reachable_println_calls = reachable_generic_calls
        .iter()
        .filter(|(_, _, _, instance_fqn)| instance_fqn.starts_with("scoop.core.println::<"))
        .collect::<Vec<_>>();
    assert!(
        !reachable_println_calls
            .iter()
            .any(|(_, _, _, instance_fqn)| instance_fqn == "scoop.core.println::<Any>"),
        "request-root 可达扫描不应推导出 println::<Any>：{reachable_println_calls:#?}"
    );
    let mut initial_requests = materializer
        .seed_requests(&inputs.typecheck_types, &inputs.monomorph_requests)
        .unwrap();
    let initial_println_requests = initial_requests
        .iter()
        .filter(|key| key.template.fqn == "scoop.core.println")
        .map(|key| {
            (
                key.template.source_path.clone(),
                key.template.decl_span,
                materializer.instance_display_fqn(key),
            )
        })
        .collect::<Vec<_>>();
    initial_requests.extend(hir_direct_instance_keys);
    assert!(
        !initial_requests.iter().any(|key| {
            key.template.fqn == "scoop.core.println" && key.type_args == vec![builtins.any]
        }),
        "精确 monomorph key 与 call binding 存在时，seed_requests 不应额外加入 println::<Any>：seed={initial_println_requests:#?}, hir={hir_direct_println_requests:#?}"
    );

    let materialized = materializer.run(initial_requests).unwrap();
    let materialized_printlns = materialized
        .file
        .items
        .iter()
        .filter_map(|item| match item {
            Item::Fun(fun) if fun.fqn.starts_with("scoop.core.println::<") => Some(fun.fqn.clone()),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert!(
        !materialized_printlns
            .iter()
            .any(|fqn| fqn == "scoop.core.println::<Any>"),
        "精确 call binding 存在时，materialize 后不应额外产出 println::<Any>：{materialized_printlns:#?}"
    );
}

#[test]
fn materialize_request_root_rewrites_nested_array_println_call_sites() {
    let sess = Session::new().unwrap();
    let source = SourceFile::new_virtual(
        "<mem>/materialize_nested_array_println.scoop",
        r#"
package fixtures.materialize

import scoop.core.*

fun main(): Int {
val xs = [1, 2, 3]
println(xs.size())

val nested = [[1, 2], [3, 4]]
println(nested.size())
return 0
}
"#,
    );
    let inputs = collect_dump_materialization_inputs(&sess, &source).unwrap();
    let compilation_unit = inputs
        .prepared_files
        .iter()
        .map(|file| (&file.source, &file.ast))
        .collect::<Vec<_>>();
    let materialized = crate::mir::materialize_compilation_unit_from_typechecked_inputs(
        &compilation_unit,
        &[source.path().to_path_buf()],
        &inputs.index,
        Some(&inputs.env),
        &inputs.typecheck_types,
        &inputs.monomorph_requests,
    )
    .unwrap();
    let main = materialized
        .pass_view()
        .callable("fixtures.materialize.main")
        .expect("pass-visible main body should be published");
    let direct_calls = direct_call_fqns(main);
    let generic_println_count = direct_calls
        .iter()
        .filter(|fqn| fqn.as_str() == "scoop.core.println")
        .count();
    let concrete_println_count = direct_calls
        .iter()
        .filter(|fqn| fqn.starts_with("scoop.core.println::<"))
        .count();

    assert_eq!(
        generic_println_count, 0,
        "request-root main should rewrite every println call to a concrete instance"
    );
    assert_eq!(
        concrete_println_count, 2,
        "both outer println calls should remain as concrete println instances"
    );
}

#[test]
fn materialize_for_dump_handles_type_body_generic_member_fun_roots() {
    let sess = Session::new().unwrap();
    let source = SourceFile::new_virtual(
        "<mem>/materialize_member_root_generic.scoop",
        r#"
package fixtures.materialize

effect Boom {
fun ping(): Unit
}

class Box() {
fun <eff E = Pure> forward(): Int / E {
    return 1
}
}

fun <eff E = Pure> wrap(box: Box): Int / E {
return box.forward<eff E>()
}

fun entry(): Int / Boom {
return wrap<eff Boom>(Box())
}
"#,
    );

    let materialized = materialize_for_dump(&sess, &source).unwrap();
    let forward_instances = materialized
        .instance_keys
        .iter()
        .filter(|key| key.template.fqn == "fixtures.materialize.Box.forward")
        .collect::<Vec<_>>();
    assert_eq!(forward_instances.len(), 1);
    assert!(forward_instances[0].type_args.is_empty());
    assert_eq!(forward_instances[0].eff_args.len(), 1);
    assert!(
        !forward_instances[0].eff_args[0].is_pure(),
        "type-body generic member fun 的实例 key 应保留非 Pure eff_args"
    );
    assert!(
        materialized.file.items.iter().any(|item| matches!(
            item,
            Item::Fun(fun)
                if fun.fqn.starts_with("fixtures.materialize.Box.forward::<")
                    && fun.fqn.contains("eff fixtures.materialize.Boom")
        )),
        "materialize_for_dump 应产出 Box.forward 的 concrete MIR root"
    );
}

#[test]
fn materialize_for_dump_devirtualizes_exact_receiver_in_mir_pass() {
    let sess = Session::new().unwrap();
    let source = SourceFile::new_virtual(
        "<mem>/materialize_dispatch_devirtualization.scoop",
        r#"
package fixtures.materialize

class Base() {
open fun value(): Int {
    return 1
}
}

class Leaf(): Base() {
override fun value(): Int {
    return 2
}
}

fun main(): Int {
val leaf = Leaf()
return leaf.value()
}
"#,
    );

    let materialized = materialize_for_dump(&sess, &source).unwrap();
    let raw_main = materialized
        .caller_side_pass_candidate_bodies()
        .iter()
        .find(|fun| fun.fqn == "fixtures.materialize.main")
        .expect("raw materialized main should be available for debug validation");
    assert!(
        fun_has_dispatch_call(raw_main),
        "raw materialized MIR should preserve dynamic dispatch before the pass layer"
    );
    let pass_main = materialized
        .pass_view()
        .callable("fixtures.materialize.main")
        .expect("main should be published in the canonical pass view");
    assert!(
        !fun_has_dispatch_call(pass_main),
        "pass-visible main should no longer contain virtual/interface dispatch after rewrite"
    );
    assert!(
        materialized
            .pass_artifacts()
            .pipeline_runs()
            .iter()
            .any(|run| {
                run.pass == MirPassKind::Devirtualization && run.enabled && run.changed_bodies > 0
            })
    );
}

#[test]
fn materialize_for_dump_publishes_generic_interface_dispatch_member_instances() {
    let sess = Session::new().unwrap();
    let source = SourceFile::new_virtual(
        "<mem>/materialize_generic_interface_dispatch.scoop",
        r#"
package fixtures.materialize

import scoop.core.*

interface IFace {
fun m(): Int
}

class Box<T>(val value: T, val code: Int) : IFace {
fun m(): Int {
    return this.code
}
}

fun read(it: IFace): Int {
return it.m()
}

fun main(): Int {
val ints: IFace = Box(9, 41)
val texts: IFace = Box("hi", 7)
return read(ints) + read(texts)
}
"#,
    );

    let inputs = collect_dump_materialization_inputs(&sess, &source).unwrap();
    let compilation_unit = inputs
        .prepared_files
        .iter()
        .map(|file| (&file.source, &file.ast))
        .collect::<Vec<_>>();
    let stable_cone_key = StableConeKey::for_virtual_source_path(source.path());
    let template_catalog =
        collect_generic_template_infos(&stable_cone_key, &inputs.index, &compilation_unit);
    let callable_body_infos = collect_callable_body_infos(&compilation_unit);
    let (top_level_fun_value_refs, top_level_fun_call_bindings) =
        collect_site_instance_bindings(&compilation_unit);
    let mut lowered_hir = crate::hir::lower_generic_for_compilation_unit_multi_files_with_type_env(
        stable_cone_key.clone(),
        &inputs.index,
        &compilation_unit,
        &compilation_unit,
        Some(&inputs.env),
        &inputs.typecheck_types,
    )
    .unwrap();
    let request_root_fun_keys = collect_request_root_fun_keys(
        &lowered_hir,
        &[source.path().to_path_buf()],
        &inputs.index,
        crate::mir::MaterializeRequestRootMode::RequestSources,
    );
    let request_sources = [source.path().to_path_buf()]
        .into_iter()
        .collect::<HashSet<_>>();
    let callable_signatures = collect_callable_signature_infos(&lowered_hir);
    let hir_direct_instance_keys_by_fun = collect_hir_direct_call_instance_requests(
        &mut lowered_hir,
        &inputs.typecheck_types,
        &top_level_fun_call_bindings,
    );
    let known_receiver_subclasses =
        crate::mir::collect_known_receiver_subclasses(&lowered_hir.direct_supertypes);
    let direct_subclasses =
        collect_direct_subclasses_from_supertypes(&lowered_hir.direct_supertypes);
    let class_vtables = lowered_hir.class_vtables.clone();
    let interfaces = lowered_hir.interfaces.clone();
    let class_itables = lowered_hir.class_itables.clone();
    let builtins = lowered_hir.types.intern_builtins();
    let hir_facts =
        scoopc_hir::stage::build_hir_facts_from_lowered_hir(&lowered_hir, source.path()).unwrap();
    let facts = MirLoweringFacts::from_hir_facts(&lowered_hir, &hir_facts);
    let generic_file = lower_hir_file_for_dump_with_facts(
        builtins,
        &mut lowered_hir.types,
        &lowered_hir.file,
        &lowered_hir.member_funs,
        &facts,
    );
    let top_level_vars = lowered_hir.top_level_vars.clone();
    let top_level_immutable_values = lowered_hir.top_level_immutable_values.clone();
    let object_inits = lowered_hir.object_inits.clone();
    let class_inits = lowered_hir.class_inits.clone();
    let enum_layouts = lowered_hir.enum_layouts.clone();
    let extern_funs = lowered_hir.extern_funs.clone();
    let native_callable_funs = lowered_hir.native_callable_funs.clone();
    let lowered_top_level_fun_call_bindings =
        collect_lowered_top_level_fun_call_bindings(&lowered_hir);
    let ctor_call_sites = lowered_hir.ctor_call_sites.clone();
    let member_value_tys = collect_member_value_type_infos_from_hir_decls(&lowered_hir.file.decls);
    let types = lowered_hir.types;
    let mut materializer = MirInstanceMaterializer::new(
        generic_file,
        types,
        builtins,
        MaterializerConstructionInputs {
            stable_cone_key,
            typecheck_types: &inputs.typecheck_types,
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
            request_root_mode: crate::mir::MaterializeRequestRootMode::RequestSources,
            request_root_fun_keys,
        },
        OptLevel::O2,
    )
    .unwrap();
    materializer.hir_direct_instance_keys_by_fun = hir_direct_instance_keys_by_fun;

    let mut dispatch_candidates = materializer
        .request_root_funs
        .iter()
        .find_map(|reachable_fun| {
            let body = reachable_fun.fun.body.as_ref()?;
            body.blocks.iter().find_map(|block| {
                block.stmts.iter().find_map(|stmt| {
                    let StatementKind::Assign {
                        value:
                            Rvalue::Call {
                                kind: CallKind::Interface { dispatch, .. },
                                args,
                                ..
                            },
                        ..
                    } = &stmt.kind
                    else {
                        return None;
                    };
                    Some(materializer.interface_dispatch_candidate_fqns(
                        dispatch.receiver_ty,
                        &dispatch.owner_fqn,
                        &dispatch.member_name,
                        args.len(),
                    ))
                })
            })
        })
        .unwrap_or_default();
    dispatch_candidates.sort();
    assert!(
        dispatch_candidates.contains(&"fixtures.materialize.Box.m::<Int>".to_string())
            && dispatch_candidates.contains(&"fixtures.materialize.Box.m::<String>".to_string()),
        "interface dispatch candidate set 应至少包含 generic owner-specialized Box.m targets：{dispatch_candidates:#?}"
    );

    let mut resolved_instances = dispatch_candidates
        .iter()
        .filter_map(|candidate| materializer.explicit_dispatch_candidate_instance(candidate))
        .map(|instance| materializer.instance_display_fqn(&instance))
        .collect::<Vec<_>>();
    resolved_instances.sort();
    assert_eq!(
        resolved_instances,
        vec![
            "fixtures.materialize.Box.m::<Int>".to_string(),
            "fixtures.materialize.Box.m::<String>".to_string(),
        ],
        "explicit dispatch candidate 必须解析出 concrete Box.m instances；当前索引键 = {:#?}",
        materializer
            .explicit_dispatch_candidate_instances
            .keys()
            .cloned()
            .collect::<Vec<_>>()
    );

    let initial_requests = materializer
        .seed_requests(&inputs.typecheck_types, &inputs.monomorph_requests)
        .unwrap();
    let mut seeded_fqns = initial_requests
        .iter()
        .filter(|key| key.template.fqn == "fixtures.materialize.Box.m")
        .map(|key| materializer.instance_display_fqn(key))
        .collect::<Vec<_>>();
    seeded_fqns.sort();
    assert_eq!(
        seeded_fqns,
        vec![
            "fixtures.materialize.Box.m::<Int>".to_string(),
            "fixtures.materialize.Box.m::<String>".to_string(),
        ],
        "request-root reachable scan 应把 generic interface dispatch targets 编入初始实例请求"
    );

    let materialized = materializer.run(initial_requests).unwrap();
    let mut display_fqns = materialized
        .file
        .items
        .iter()
        .filter_map(|item| match item {
            Item::Fun(fun) if fun.fqn.starts_with("fixtures.materialize.Box.m::<") => {
                Some(fun.fqn.clone())
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    display_fqns.sort();
    assert_eq!(
        display_fqns,
        vec![
            "fixtures.materialize.Box.m::<Int>".to_string(),
            "fixtures.materialize.Box.m::<String>".to_string(),
        ],
        "generic interface dispatch 应发布 owner-specialized Box.m instances"
    );
}

#[test]
fn materialize_for_dump_distinguishes_companion_member_fun_effect_instances() {
    let sess = Session::new().unwrap();
    let source = SourceFile::new_virtual(
        "<mem>/materialize_companion_member_root_generic.scoop",
        r#"
package fixtures.materialize

effect Boom {
fun ping(): Unit
}

effect Zap {
fun pong(): Unit
}

class Box() {
companion object {
    fun <eff E = Pure> forward(): Int / E {
        return 1
    }
}
}

fun <eff E = Pure> wrap(): Int / E {
return Box.forward<eff E>()
}

fun use_boom(): Int / Boom {
return wrap<eff Boom>()
}

fun use_zap(): Int / Zap {
return wrap<eff Zap>()
}
"#,
    );

    let materialized = materialize_for_dump(&sess, &source).unwrap();
    let forward_instances = materialized
        .instance_keys
        .iter()
        .filter(|key| key.template.fqn == "fixtures.materialize.Box.Companion.forward")
        .collect::<Vec<_>>();
    assert_eq!(forward_instances.len(), 2);
    let mut effect_rows = forward_instances
        .iter()
        .map(|key| {
            assert!(key.type_args.is_empty());
            assert_eq!(key.eff_args.len(), 1);
            assert_eq!(key.eff_args[0].terms.len(), 1);
            materialized
                .types
                .display(key.eff_args[0].terms[0])
                .to_string()
        })
        .collect::<Vec<_>>();
    effect_rows.sort();
    assert_eq!(
        effect_rows,
        vec![
            "fixtures.materialize.Boom".to_string(),
            "fixtures.materialize.Zap".to_string()
        ]
    );
    assert!(materialized.file.items.iter().any(|item| matches!(
        item,
        Item::Fun(fun)
            if fun.fqn == "fixtures.materialize.Box.Companion.forward::<eff fixtures.materialize.Boom>"
    )));
    assert!(materialized.file.items.iter().any(|item| matches!(
        item,
        Item::Fun(fun)
            if fun.fqn == "fixtures.materialize.Box.Companion.forward::<eff fixtures.materialize.Zap>"
    )));
}

#[test]
fn materialize_for_dump_keeps_non_generic_overload_targets_path_stable() {
    let sess = Session::new().unwrap();
    let program = r#"
package fixtures.materialize

fun pick(x: Int): Int { return x }
fun pick(x: Int, y: Int): Int { return y }

fun main(): Int {
val a: Int = pick(1)
val b: Int = pick(1, 2)
return a + b
}
"#;

    let collect_targets = |source: &SourceFile| {
        let materialized = materialize_for_dump(&sess, source)
            .expect("non-generic overload fixture 应可 materialize");
        let pass_view = materialized.pass_view();
        let published = pass_view
            .instances()
            .map(|family| family.root_fqn().to_string())
            .filter(|fqn| fqn.starts_with("fixtures.materialize.pick$overload$"))
            .collect::<std::collections::BTreeSet<_>>();
        for target in &published {
            assert!(
                pass_view.callable(target).is_some(),
                "pass-view 应发布非泛型 overload callable `{target}` 的 canonical body"
            );
        }
        published
    };

    let source_a = SourceFile::new_virtual(
        "/tmp/root-a/fixtures/non_generic_overload_identity.scoop",
        program,
    );
    let source_b = SourceFile::new_virtual(
        "/tmp/root-b/fixtures/non_generic_overload_identity.scoop",
        program,
    );

    let targets_a = collect_targets(&source_a);
    let targets_b = collect_targets(&source_b);

    assert_eq!(
        targets_a.len(),
        2,
        "两个非泛型 overload 应发布两个 distinct callable target：{targets_a:#?}"
    );
    assert!(
        targets_a
            .iter()
            .all(|target| target.starts_with("fixtures.materialize.pick$overload$")),
        "非泛型 overload callable 应统一切到 overload-aware FQN：{targets_a:#?}"
    );
    assert_eq!(
        targets_a, targets_b,
        "不同源码根路径下的非泛型 overload target 应保持稳定：{targets_a:#?} vs {targets_b:#?}"
    );
}

#[test]
fn materialize_for_dump_keeps_mutable_array_push_transport_concrete() {
    let sess = Session::new().unwrap();
    let source = SourceFile::new_virtual(
        "/tmp/fixtures/mutable_array_push_transport.scoop",
        r#"
import scoop.core.*

fun main(): Int {
    val xs: MutableArray<Int> = mutableArrayNew<Int>(capacity = 2)
    xs.push(1)
    return 0
}
"#,
    );

    let materialized =
        materialize_for_dump(&sess, &source).expect("mutable array push fixture 应可 materialize");
    let pass_view = materialized.pass_view();
    let body = pass_view
        .callable("main")
        .and_then(|fun| fun.body.as_ref())
        .expect("应保留 main 的 materialized body");
    let transport = body
        .blocks
        .iter()
        .flat_map(|block| block.stmts.iter())
        .find_map(|stmt| {
            let StatementKind::Assign {
                value:
                    Rvalue::Call {
                        kind: CallKind::Direct { callee_fqn },
                        transport,
                        ..
                    },
                ..
            } = &stmt.kind
            else {
                return None;
            };
            callee_fqn
                .split("::<")
                .next()
                .is_some_and(|base| base == "scoop.core.push")
                .then_some(transport)
        })
        .expect("应找到 MutableArray.push call transport");
    let array = transport
        .array
        .as_ref()
        .expect("MutableArray.push 应发布 array transport metadata");

    assert!(
        !type_contains_param(&materialized.types, array.array_ty),
        "MutableArray.push array transport array type 应已具体化: {}",
        materialized.types.display(array.array_ty)
    );
    assert!(
        !type_contains_param(&materialized.types, array.element_ty),
        "MutableArray.push array transport element type 应已具体化: {}",
        materialized.types.display(array.element_ty)
    );
    let TypeKind::Ref(RefTypeKind::Nominal(array_nominal)) =
        materialized.types.kind(array.array_ty)
    else {
        panic!(
            "MutableArray.push receiver 应是 nominal mutable array，实际为 {:?}",
            materialized.types.kind(array.array_ty)
        );
    };
    assert!(
        array_nominal.fqn == "scoop.core.MutableArray",
        "MutableArray.push receiver 应保持 MutableArray，实际为 {}",
        array_nominal.fqn
    );
    assert_eq!(array_nominal.args.first().copied(), Some(array.element_ty));
    assert_eq!(
        materialized.types.display(array.element_ty).to_string(),
        "Int"
    );
    assert_eq!(array.element.source_ty, array.element_ty);
}
