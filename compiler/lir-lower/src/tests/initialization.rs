use super::*;

mod protocol;
use scoop_identity::{
    InitializationCallableRole, PersistentGeneratedCallableId, PersistentInitializationUnitId,
};

fn initialization_function(
    module: &mut mir::Module,
    unit: PersistentInitializationUnitId,
    role: InitializationCallableRole,
) -> mir::FunctionId {
    let function = module.functions.alloc(mir::Function {
        gc_effect: mir::GcEffect::Managed,
        name: format!("{role:?}"),
        params: vec![],
        return_ty: mir::Type::Unit,
        body: mir::Body::unreachable(Arena::new()),
    });
    let generated =
        PersistentGeneratedCallableId::from_key(&GeneratedCallableKey::Initialization {
            unit,
            role,
        })
        .unwrap();
    let exact = module
        .meta
        .source_exact_types
        .get(&mir::Type::Unit)
        .unwrap()
        .identity_record()
        .id();
    let source = mir::SourceCallableMaterialization::new(
        function,
        CallableMaterialization::new(
            CallableTemplateOwner::Generated(generated),
            CallableMaterializationContext::NoSubstitution,
        ),
        ExactCallableSignature::new(Effect::Ordinary, None, vec![], exact),
        None,
    )
    .unwrap();
    let mut signatures: Vec<_> = module.meta.callable_signatures.iter().cloned().collect();
    signatures.push(source.signature_record().clone());
    module.meta.callable_signatures = mir::MirCallableSignatures::checked(signatures).unwrap();
    let mut sources: Vec<_> = module
        .meta
        .source_callable_materializations
        .iter()
        .cloned()
        .collect();
    sources.push(source);
    module.meta.source_callable_materializations =
        mir::SourceCallableMaterializations::checked(sources).unwrap();
    module.top_level.push(function);
    function
}

#[test]
fn initialization_display_name_survives_lir_lowering() {
    let mut builder = Builder::new();
    let main = builder.main(Arena::new(), Vec::new());
    let mut source = builder.finish(main);
    let unit_identity = initialization_unit_identity();
    let persistent_unit = unit_identity.id();
    let initializer = initialization_function(
        &mut source,
        persistent_unit,
        InitializationCallableRole::Initializer,
    );
    let ensure = initialization_function(
        &mut source,
        persistent_unit,
        InitializationCallableRole::Ensure,
    );
    let storage_owner = property_owner("value");
    let storage = source.globals.alloc(mir::Global {
        name: "value".to_string(),
        storage_owner: mir::StaticStorageOwner::PropertyBacking(storage_owner),
        ty: mir::Type::Unit,
        mutable: false,
        storage: mir::GlobalStorage::Managed {
            initial_state: mir::MirStaticInitialState::ZeroedForRuntimeUnit,
        },
    });
    let failure_global = source.globals.alloc(mir::Global {
        name: "$init.failure".to_string(),
        storage_owner: mir::StaticStorageOwner::InitializationFailureRoot(persistent_unit),
        ty: mir::Type::String,
        mutable: true,
        storage: mir::GlobalStorage::Managed {
            initial_state: mir::MirStaticInitialState::ZeroedForRuntimeUnit,
        },
    });
    let unit_id = mir::InitializationUnitId::from_raw(0_u32.into());
    let failure_root = source
        .initialization_failure_roots
        .alloc(mir::InitializationFailureRoot {
            global: failure_global,
        });
    let unit = source.initialization_units.alloc(mir::InitializationUnit {
        identity: unit_identity,
        display_name: "top-level:value".to_string(),
        schedule: mir::InitializationSchedule::EagerStartup,
        kind: mir::InitializationUnitKind::EagerTopLevel { storage },
        initializer,
        ensure,
        failure_root,
        dependencies: Vec::new(),
        cycle_thrower: mir::InitializationCycleThrower::Local(main),
    });
    assert_eq!(unit, unit_id);
    let expected_unit_identity = source.initialization_units[unit].identity.clone();

    let module = lower(source);
    let lowered_unit =
        &module.initialization_units[lir::InitializationUnitId::from_raw(unit_id.into_raw())];
    assert_eq!(lowered_unit.display_name, "top-level:value");
    assert_eq!(lowered_unit.identity, expected_unit_identity);
    assert_ne!(
        lowered_unit.initializer.declaration(),
        lowered_unit.ensure.declaration()
    );
    let lir::GlobalInit::Storage {
        identity: storage_identity,
        layout: storage_layout,
        ..
    } = &module.globals[lowered_unit.kind.storage()].init
    else {
        panic!("initialization storage must remain a storage global")
    };
    assert_eq!(
        storage_identity,
        &lir::StaticStorageIdentity::static_place_for_property(
            storage_owner,
            lir::MaterializationRoot::cone_owned(),
        )
        .unwrap()
    );
    assert_eq!(
        storage_layout.layout_record().key().representation(),
        scoop_identity::RepresentationRole::ManagedValue
    );
    assert_eq!(
        storage_layout.scan_record().key().layout(),
        storage_layout.layout_record().id()
    );
    let lir::GlobalInit::Storage {
        identity: failure_identity,
        layout: failure_layout,
        ..
    } = &module.globals[lowered_unit.failure_root].init
    else {
        panic!("initialization failure root must remain a storage global")
    };
    assert_eq!(
        failure_identity,
        &lir::StaticStorageIdentity::initialization_failure_root(
            persistent_unit,
            lir::MaterializationRoot::cone_owned(),
        )
        .unwrap()
    );
    assert_eq!(
        failure_layout.layout_record().key().representation(),
        scoop_identity::RepresentationRole::ManagedValue
    );
    assert_eq!(
        failure_layout.scan_record().key().layout(),
        failure_layout.layout_record().id()
    );
    let gateway_body =
        lir::CallableBodyIdentity::for_initialization_startup_gateway(expected_unit_identity.id())
            .unwrap()
            .id();
    let gateway = module
        .functions
        .iter()
        .find(|function| function.callable_body.id() == gateway_body)
        .expect("an eager initialization unit emits one startup gateway");
    assert_eq!(gateway.gc_effect, lir::GcEffect::Managed);
    assert!(matches!(
        gateway.signature.result(),
        lir::AbiReturn::Direct(_)
    ));
    let site = gateway.blocks[gateway.entry]
        .instructions
        .iter()
        .find_map(|instruction| match instruction {
            lir::Instruction::Invoke { site } => Some(site),
            _ => None,
        })
        .expect("the no-throw startup gateway invokes the unit ensure function");
    assert_eq!(
        site.destination(&gateway.call_targets),
        lir::CallDestination::Local(lowered_unit.ensure.declaration())
    );
}
