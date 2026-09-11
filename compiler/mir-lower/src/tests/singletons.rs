use super::*;

#[test]
fn singleton_identity_chain_survives_concretization_and_mir_lowering() {
    let mut harness = Harness::new();
    let backing = harness.class("Registry", hir::ClassModifier::Final, &[], None, &[]);
    let backing_ty = harness.class_ty(backing);
    let main = empty_main(&mut harness);
    let ensure = harness.user_fn(
        "ensureRegistry",
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
            link_stem: nominal_link_stem("Registry.object"),
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
                link_name: "Registry".to_string(),
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
            stable_key: "singleton:Registry".to_string(),
            display_name: "object:Registry".to_string(),
            schedule: hir::InitializationSchedule::LazyAccess,
            kind: hir::InitializationUnitKind::LazySingleton {
                value,
                published_root,
            },
            initializer: main,
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

    let source = legacy_executable(source, entry);
    let expected_identity = source.initialization_unit_identities[initialization].clone();
    let expected_singleton_owner = source.nominal_identities[object]
        .concrete_type_id()
        .expect("test object has a concrete nominal identity");
    let module = lower(&source);
    let declaration = &module.objects[mir::ObjectId::from_raw(0_u32.into())];
    let singleton = &module.singleton_values[mir::SingletonValueId::from_raw(0_u32.into())];
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
    assert_eq!(
        module.globals[root.global].symbol,
        "scoop.singleton.Registry"
    );
    assert_eq!(
        module.globals[root.global].storage_owner,
        mir::StaticStorageOwner::SingletonPublishedRoot(expected_singleton_owner)
    );
    assert_eq!(
        module.globals[module.initialization_failure_roots
            [module.initialization_units[singleton.initialization].failure_root]
            .global]
            .storage_owner,
        mir::StaticStorageOwner::InitializationFailureRoot(expected_identity.id())
    );
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

    let dump = mir::dump(&module);
    assert!(dump.contains("$init.state: machine<initialization-outcome>"));
    assert!(dump.contains("Binary MachineEq(InitializationOutcome)"));
    assert!(dump.contains("MachineScalarLiteral InitializationOutcome(RunInitializer)"));
    assert!(dump.contains("MachineScalarLiteral InitializationOutcome(Ready)"));
    assert!(dump.contains("MachineScalarLiteral InitializationOutcome(Failed)"));
    assert!(!dump.contains("$init.state: Int"));
}
