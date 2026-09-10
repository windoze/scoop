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
