use super::*;

#[test]
fn static_function_adapter_keeps_its_complete_structural_identity() {
    let mut h = Harness::new();
    let unit = h.unit;
    let int = h.int;
    let any = h.any();
    let source = function_type(&mut h, vec![any], unit);
    let target = function_type(&mut h, vec![int], unit);
    let main = empty_main(&mut h);
    let executable = h.finish(main);
    let entry = executable.entry();
    let mut source_module = executable.into_module();
    let coercion = source_module
        .function_coercions
        .alloc(hir::FunctionCoercion {
            source: source.0,
            target: target.0,
        });
    let hir::FunctionKind::User(body) = &mut source_module.functions[main].kind else {
        panic!("main is a user function")
    };
    let source_local = body.locals.alloc(local("source", source.1));
    let adapted = body.locals.alloc(local("adapted", target.1));
    body.statements.push(val_decl(
        adapted,
        expr(
            hir::ExprKind::FunctionCoercion {
                source: Box::new(local_ref(source_local, source.1)),
                coercion,
                target_type: target.0,
            },
            target.1,
        ),
    ));

    let export = legacy_executable(source_module, entry);
    let concrete = scoop_hir_lower::concretize_legacy_export(&export);
    let target_function = scoop_hir::concrete::FunctionTypeId::from_raw(target.0.into_raw());
    let target_exact = concrete.module().exact_type_identities
        [concrete.module().function_types[target_function].canonical_type]
        .id();
    let mut module = super::super::lower(&concrete);
    let (adapter_id, adapter) = module
        .meta
        .closure_adapters
        .iter()
        .next()
        .expect("the coercion creates one static function adapter");
    assert_eq!(module.meta.closure_adapters.len(), 1);
    let exact = module
        .meta
        .generated_exact_types
        .get(mir::GeneratedExactTypeLocation::Closure(adapter.class()))
        .expect("the adapter environment has one generated exact type");
    assert_eq!(
        exact.exact_record().key(),
        &scoop_identity::ExactTypeKey::Nominal(adapter.identity().environment_record().id())
    );
    assert!(matches!(
        adapter.identity().environment_record().key(),
        scoop_identity::GeneratedNominalKey::CallableAdapterEnvironment {
            key: scoop_identity::CallableAdapterEnvironmentKey::Static { .. },
        }
    ));
    assert!(matches!(
        adapter.identity().callable_record().key(),
        scoop_identity::GeneratedCallableKey::FunctionAdapter { .. }
    ));
    assert!(matches!(
        adapter.identity().odr_group_record().key(),
        scoop_identity::SpecializationKey::StructuralType { exact_type }
            if *exact_type == target_exact
    ));
    assert_eq!(
        adapter.identity().environment_member_record().key().role(),
        scoop_identity::OdrMemberRole::GeneratedNominal
    );
    assert_eq!(
        adapter.identity().callable_member_record().key().role(),
        scoop_identity::OdrMemberRole::CallableBody
    );
    assert_eq!(
        adapter.identity().callable_signature_record().signature(),
        match adapter.identity().callable_record().key() {
            scoop_identity::GeneratedCallableKey::FunctionAdapter { target, .. } => target,
            _ => unreachable!("the adapter has a static callable key"),
        }
    );
    assert_eq!(module.validate(), Ok(()));

    let class = adapter.class();
    module.closure_classes[class].captures[0].ty = mir::Type::Any;
    assert_eq!(
        module.validate(),
        Err(mir::MirValidationError {
            location: mir::MirValidationLocation::FunctionAdapter {
                adapter: adapter_id,
            },
            kind: mir::MirValidationErrorKind::InvalidFunctionAdapter {
                reason: "the adapter class has an invalid source capture",
            },
        })
    );
}

#[test]
fn checked_function_cast_keeps_its_complete_dynamic_adapter_identity() {
    let mut h = Harness::new();
    h.exception("ClassCastException");
    let unit = h.unit;
    let int = h.int;
    let any = h.any();
    let target = function_type(&mut h, vec![int], unit);
    let mut locals = Arena::new();
    let erased = locals.alloc(local("erased", any));
    let adapted = locals.alloc(local("adapted", target.1));
    let main = h.user_fn(
        "main",
        hir::Body {
            locals,
            statements: vec![val_decl(
                adapted,
                expr(
                    hir::ExprKind::Cast {
                        operand: Box::new(local_ref(erased, any)),
                        optional: false,
                    },
                    target.1,
                ),
            )],
        },
    );

    let mut module = lower(&h.finish(main));
    let (adapter_id, adapter) = module
        .meta
        .dynamic_closure_adapters
        .iter()
        .next()
        .expect("the checked cast creates one dynamic function adapter");
    assert_eq!(module.meta.dynamic_closure_adapters.len(), 1);
    assert!(matches!(
        adapter.identity().environment_record().key(),
        scoop_identity::GeneratedNominalKey::CallableAdapterEnvironment {
            key: scoop_identity::CallableAdapterEnvironmentKey::Dynamic { .. },
        }
    ));
    assert!(matches!(
        adapter.identity().callable_record().key(),
        scoop_identity::GeneratedCallableKey::DynamicFunctionAdapter { .. }
    ));
    assert!(matches!(
        adapter.identity().odr_group_record().key(),
        scoop_identity::SpecializationKey::StructuralType { .. }
    ));
    assert_eq!(
        adapter.identity().environment_member_record().key().role(),
        scoop_identity::OdrMemberRole::GeneratedNominal
    );
    assert_eq!(
        adapter.identity().callable_member_record().key().role(),
        scoop_identity::OdrMemberRole::CallableBody
    );
    assert_eq!(module.validate(), Ok(()));

    let class = adapter.class();
    module.closure_classes[class].captures[0].ty = mir::Type::Function(adapter.target());
    assert_eq!(
        module.validate(),
        Err(mir::MirValidationError {
            location: mir::MirValidationLocation::DynamicFunctionAdapter {
                adapter: adapter_id,
            },
            kind: mir::MirValidationErrorKind::InvalidFunctionAdapter {
                reason: "the adapter class has an invalid source capture",
            },
        })
    );
}

#[test]
fn signature_changing_closure_dispatch_keeps_its_generated_bridge_identity() {
    let mut h = Harness::new();
    h.exception("ClassCastException");
    let unit = h.unit;
    let int = h.int;
    let any = h.any();
    let source_type = function_type(&mut h, vec![any], unit);
    let target_type = function_type(&mut h, vec![int], unit);

    let mut target_locals = Arena::new();
    let value = target_locals.alloc(local("value", any));
    let target = h.user_fn_full(
        "consumeAny",
        Vec::new(),
        vec![param("value", any, value)],
        unit,
        hir::Body {
            locals: target_locals,
            statements: Vec::new(),
        },
    );
    let main = empty_main(&mut h);
    let executable = h.finish(main);
    let entry = executable.entry();
    let mut source = executable.into_module();
    let reference = source.callable_references.alloc(hir::CallableReference {
        definition_root: hir::LexicalDefinitionRoot::Function(main),
        definition_path: scoop_identity::StructuralDefinitionPath::from_first(
            scoop_identity::StructuralPathSegment::new(
                scoop_identity::StructuralDefinitionSiteRole::CallableConversion,
                0,
            ),
            [],
        ),
        target: hir::CallableReferenceTarget::Named(hir::Callable::Function(target)),
        function_type: source_type.0,
        owner_type_param_count: 0,
        captures: Vec::new(),
        span: SPAN,
    });
    let coercion = source.function_coercions.alloc(hir::FunctionCoercion {
        source: source_type.0,
        target: target_type.0,
    });
    let hir::FunctionKind::User(body) = &mut source.functions[main].kind else {
        panic!("main is a user function")
    };
    let adapted = body.locals.alloc(local("adapted", target_type.1));
    body.statements.push(val_decl(
        adapted,
        expr(
            hir::ExprKind::FunctionCoercion {
                source: Box::new(expr(
                    hir::ExprKind::CallableReference(reference),
                    source_type.1,
                )),
                coercion,
                target_type: target_type.0,
            },
            target_type.1,
        ),
    ));
    let erased = body.locals.alloc(local("erased", any));
    let checked = body.locals.alloc(local("checked", target_type.1));
    body.statements.push(val_decl(
        checked,
        expr(
            hir::ExprKind::Cast {
                operand: Box::new(local_ref(erased, any)),
                optional: false,
            },
            target_type.1,
        ),
    ));

    let export = legacy_executable(source, entry);
    let concrete = scoop_hir_lower::concretize_legacy_export(&export);
    let module = super::super::lower(&concrete);
    let bridge = module
        .meta
        .function_bridges
        .first()
        .expect("the signature-changing source closure creates one bridge");
    assert_eq!(module.meta.function_bridges.len(), 1);
    let environment = module
        .meta
        .closure_environments
        .iter()
        .find(|environment| environment.class() == bridge.class())
        .expect("the bridge belongs to the callable-reference environment");
    assert_eq!(
        bridge.identity().environment_record(),
        environment.identity().generated_type_record()
    );
    assert!(matches!(
        bridge.identity().callable_record().key(),
        scoop_identity::GeneratedCallableKey::FunctionBridge {
            environment: found,
            target,
        } if *found == environment.identity().generated_type_record().id()
            && target == bridge.identity().signature_record().signature()
    ));
    assert_eq!(module.validate(), Ok(()));
}

fn function_type(
    harness: &mut Harness,
    parameters: Vec<hir::TypeId>,
    result: hir::TypeId,
) -> (hir::FunctionTypeId, hir::TypeId) {
    let canonical_type = hir::TypeId::from_raw(
        u32::try_from(harness.types.len())
            .expect("fixture type id fits in u32")
            .into(),
    );
    let function = harness.function_types.alloc(hir::FunctionType {
        canonical_type,
        is_suspend: false,
        parameter_types: parameters,
        return_type: result,
    });
    assert_eq!(
        harness.types.alloc(hir::Type::Function(function)),
        canonical_type
    );
    (function, canonical_type)
}
