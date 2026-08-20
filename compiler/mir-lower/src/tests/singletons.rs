use super::*;

#[test]
fn singleton_identity_chain_survives_concretization_and_mir_lowering() {
    let mut harness = Harness::new();
    let backing = harness.class("Registry", hir::ClassModifier::Final, &[], None, &[]);
    harness.classes[backing].access.declared = hir::DeclaredVisibility::Private;
    let backing_ty = harness.class_ty(backing);
    let main = empty_main(&mut harness);
    let ensure = harness.user_fn(
        "ensureRegistry",
        hir::Body {
            locals: Arena::new(),
            statements: Vec::new(),
        },
    );
    harness.functions[ensure].kind = hir::FunctionKind::InitializationEnsure;
    let initializer = harness.user_fn(
        "initializeRegistry",
        hir::Body {
            locals: Arena::new(),
            statements: Vec::new(),
        },
    );
    let executable = harness.finish_with_initialization_core(main);
    let entry = executable.entry();
    let mut source = executable.into_module();

    let object = hir::ObjectId::from_raw(0_u32.into());
    let object_type = hir::ObjectTypeId::from_raw(0_u32.into());
    let value = hir::SingletonValueId::from_raw(0_u32.into());
    let published_root = hir::SingletonPublishedRootId::from_raw(0_u32.into());
    let initialization = hir::InitializationUnitId::from_raw(0_u32.into());
    let failure_root = source
        .initialization_failure_roots
        .alloc(hir::InitializationFailureRoot {
            unit: initialization,
        });
    assert_eq!(
        source.object_types.alloc(hir::ObjectType {
            declaration: object,
            representation: source.classes[backing].self_application,
            canonical_type: backing_ty,
        }),
        object_type
    );
    assert_eq!(
        source.objects.alloc(hir::ObjectDecl {
            name: "Registry".to_string(),
            owner: None,
            access: hir::NominalAccess::public(),
            object_type,
            singleton_value: value,
            kind: hir::ObjectKind::Standalone,
            backing_class: backing,
            span: SPAN,
        }),
        object
    );
    assert_eq!(
        source
            .singleton_published_roots
            .alloc(hir::SingletonPublishedRoot {
                value,
                ty: backing_ty,
            }),
        published_root
    );
    assert_eq!(
        source.singleton_values.alloc(hir::SingletonValue {
            declaration: object,
            object_type,
            published_root,
            initialization,
        }),
        value
    );
    assert_eq!(
        source.initialization_units.alloc(hir::InitializationUnit {
            display_name: "object:Registry".to_string(),
            schedule: hir::InitializationSchedule::LazyAccess,
            kind: hir::InitializationUnitKind::LazySingleton {
                value,
                published_root,
            },
            initializer,
            ensure,
            failure_root,
            dependencies: Vec::new(),
            span: SPAN,
        }),
        initialization
    );

    source.nominal_identities = crate::tests::harness_nominals::test_nominal_identities(
        &source.structs,
        &source.enums,
        &source.classes,
        &source.interfaces,
        &source.objects,
    );
    source.type_identities = rebuild_type_identities(&source);
    source.initialization_unit_identities = rebuild_initialization_unit_identities(&source);
    source.object_value_identities = hir::HirObjectValueIdentities::from_declarations(
        &source.objects,
        &source.object_types,
        &source.singleton_values,
        &source.nominal_identities,
    )
    .unwrap();
    let expected_value = source.object_value_identities[value].id();
    let hir::FunctionKind::User(body) = &mut source.functions[main].kind else {
        panic!("the fixture entry has a user body");
    };
    body.statements.push(expr_stmt(expr(
        hir::ExprKind::SingletonValue(hir::SingletonValueTarget::Local(value)),
        backing_ty,
    )));

    let source = executable_output(source, entry);
    let expected_identity = source.initialization_unit_identities[initialization].clone();
    let expected_singleton_owner = source.nominal_identities[object]
        .concrete_type_id()
        .expect("test object has a concrete nominal identity");
    let module = lower(&source);
    let declaration = &module.objects[mir::ObjectId::from_raw(0_u32.into())];
    let singleton = &module.singleton_values[mir::SingletonValueId::from_raw(0_u32.into())];
    assert_eq!(singleton.identity, expected_value);
    let root = &module.singleton_published_roots[singleton.published_root];
    assert_eq!(
        module.initialization_units[singleton.initialization].display_name,
        "object:Registry"
    );
    assert_eq!(
        module.initialization_units[singleton.initialization].identity,
        expected_identity
    );
    assert_eq!(declaration.name, "Registry");
    assert_eq!(declaration.object_type, singleton.object_type);
    assert_eq!(
        declaration.singleton_value,
        mir::SingletonValueId::from_raw(0_u32.into())
    );
    assert_eq!(module.globals[root.global].name, "$singleton$Registry");
    assert_eq!(
        module.globals[root.global].storage_owner,
        mir::StaticStorageOwner::SingletonPublishedRoot(expected_singleton_owner)
    );
    let failure_global = &module.globals[module.initialization_failure_roots
        [module.initialization_units[singleton.initialization].failure_root]
        .global];
    assert_eq!(
        failure_global.storage_owner,
        mir::StaticStorageOwner::InitializationFailureRoot(expected_identity.id())
    );
    assert_eq!(failure_global.ty, mir::Type::Any);
    assert!(matches!(
        module.initialization_units[singleton.initialization].kind,
        mir::InitializationUnitKind::LazySingleton {
            value: mir_value,
            published_root: mir_root,
        } if mir_value == mir::SingletonValueId::from_raw(0_u32.into())
            && mir_root == singleton.published_root
    ));

    let ensure = &module.functions[module.initialization_units[singleton.initialization].ensure];
    let state = ensure
        .body
        .locals
        .iter()
        .find_map(|(_, local)| (local.name == "$init.state").then_some(local))
        .expect("the generated ensure function has an initialization outcome local");
    assert_eq!(
        state.ty,
        mir::Type::MachineScalar(mir::MachineScalarKind::InitializationOutcome)
    );
    for name in ["$init.caught", "$init.failure"] {
        let local = ensure
            .body
            .locals
            .iter()
            .find_map(|(_, local)| (local.name == name).then_some(local))
            .unwrap_or_else(|| panic!("generated ensure function is missing {name}"));
        assert_eq!(local.ty, mir::Type::Any);
    }
    let mir::InitializationCycleThrower::Local(cycle_thrower) =
        module.initialization_units[singleton.initialization].cycle_thrower
    else {
        panic!("a core-defining fixture must use its local cycle thrower")
    };
    assert_eq!(
        module.functions[cycle_thrower].name,
        "__scoopThrowInitializationCycle"
    );

    let dump = mir::dump(&module);
    assert!(dump.contains("$init.state: machine<initialization-outcome>"));
    assert!(dump.contains("Binary MachineEq(InitializationOutcome)"));
    assert!(dump.contains("MachineScalarLiteral InitializationOutcome(RunInitializer)"));
    assert!(dump.contains("MachineScalarLiteral InitializationOutcome(Ready)"));
    assert!(dump.contains("MachineScalarLiteral InitializationOutcome(Failed)"));
    assert!(!dump.contains("$init.state: Int"));
    assert!(dump.contains("$init.failure: Any"));
    assert!(!dump.contains("IsInstance Any"));
}
