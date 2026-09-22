use super::*;

#[test]
fn core_lowering_publishes_initialization_protocol_abi() {
    let mut builder = Builder::new();
    let function = builder.user_fn("exported", Arena::new(), Vec::new());
    let cycle_function =
        builder.user_fn("__scoopThrowInitializationCycle", Arena::new(), Vec::new());
    let mut module = builder.finish(function);
    module.cone = scoop_identity::ConeIdentity::CORE;
    module.output = mir::MirOutput::Library;
    let mir::CallableSignatureSubject::Strong(cycle_implementation) = module
        .meta
        .callable_signature_subject(cycle_function)
        .expect("test cycle function has a strong callable subject")
    else {
        panic!("test cycle function cannot have ODR ownership")
    };
    let scoop_identity::CallableOwner::Function(cycle_definition) = cycle_implementation else {
        panic!("test cycle function has a function owner")
    };
    let foundation = mir::OdrFreeMirFoundation::from_module(&module).unwrap();
    let strong = mir::StrongCallableBridgeSurfaceV1::from_odr_free_foundation(&foundation);
    let exact = strong
        .bridges()
        .iter()
        .find(|bridge| bridge.implementation() == cycle_implementation)
        .unwrap()
        .signature()
        .clone();
    let production = mir::CoreBootstrapBridgeSectionV1::try_new(
        ConeIdentity::CORE,
        mir::EntryMirBridgeBranchV1::Library,
        strong.with_initialization_cycle(cycle_definition).unwrap(),
    )
    .unwrap();
    let input = mir::SingleConeStrongMirInput::try_new(
        module,
        foundation,
        production,
        Vec::new(),
        mir::StrongExternalCallableInput::Unused,
    )
    .unwrap();

    let output = crate::lower(
        &input,
        crate::RuntimeStringDescriptor::Local,
        &lir::SelectedExternalLirSet::empty(ConeIdentity::CORE),
        lir::LirTargetProfile::DARWIN_AARCH64,
    )
    .unwrap();
    let Some(callable) = output.initialization_cycle_abi() else {
        panic!("the initialization role must publish its LIR ABI")
    };
    assert_eq!(callable.abi_signature().signature(), &exact);
    assert_eq!(
        callable.root_plan(),
        lir::ExternalCallableRootPlan::ManagedStatepoint
    );
    assert_eq!(
        callable.expected_symbol(),
        output.module().functions[1].callable_body.symbol_request()
    );
}

mod fixture;
mod selection;

#[test]
fn ordinary_lowering_materializes_and_calls_the_initialization_protocol() {
    let fixture::ImportedInitialization {
        input: ordinary_input,
        runtime_string,
        callable: lir_callable,
    } = fixture::imported_initialization();
    let target = lir_callable.bridge().target();
    let string_exact = runtime_string.target();
    let selected_lir = lir::SelectedExternalLirSet::empty(ConeIdentity::SINGLE_FILE)
        .with_initialization_cycle(lir_callable)
        .unwrap();

    assert!(matches!(
        crate::lower(
            &ordinary_input,
            crate::RuntimeStringDescriptor::Local,
            &selected_lir,
            lir::LirTargetProfile::DARWIN_AARCH64
        ),
        Err(crate::StrongLirLoweringError::RuntimeStringDescriptorOwnership { .. })
    ));
    let output = crate::lower(
        &ordinary_input,
        crate::RuntimeStringDescriptor::External(runtime_string),
        &selected_lir,
        lir::LirTargetProfile::DARWIN_AARCH64,
    )
    .unwrap();
    let module = output.module();
    assert_eq!(module.meta.external_callables.len(), 1);
    assert_eq!(module.meta.external_type_descriptors.len(), 1);
    let string_descriptor = &module
        .meta
        .external_type_descriptors
        .iter()
        .next()
        .unwrap()
        .1;
    assert_eq!(string_descriptor.target(), string_exact);
    assert!(matches!(
        module.meta.well_known_type_descriptors.string,
        lir::TypeDescriptorRef::External(_)
    ));
    assert!(
        module
            .meta
            .type_descriptors
            .iter()
            .all(|(_, descriptor)| descriptor.identity.exact_type() != string_exact)
    );
    assert!(
        module.meta.layouts.iter().all(|(_, layout)| {
            layout.identity.layout_record().key().exact_type() != string_exact
        })
    );
    let external = &module.meta.external_callables.iter().next().unwrap().1;
    assert_eq!(external.signature().logical_argument_count(), 1);
    assert_eq!(external.target(), target);
    assert_eq!(
        external.root_plan(),
        lir::ExternalCallableRootPlan::ManagedStatepoint
    );
    let call = module.functions[0].blocks[module.functions[0].entry]
        .instructions
        .iter()
        .find_map(|instruction| match instruction {
            lir::Instruction::Call { site } => Some(site),
            _ => None,
        })
        .expect("ordinary caller emits the imported core call");
    assert!(matches!(
        call.destination(&module.functions[0].call_targets),
        lir::CallDestination::External(_)
    ));
    let production = output
        .build_production_section(
            scoop_identity::ConeCoordinate::reserved_single_file(),
            lir::EntryProductionSourceV1::Library,
        )
        .unwrap();
    assert_eq!(production.external_bridges().bridges().len(), 2);
    assert!(production.external_bridges().bridges().contains(
        &lir::StrongExternalLirBridgeV1::TypeDescriptor(runtime_string)
    ));
    assert_eq!(
        production
            .external_bridges()
            .bridges()
            .iter()
            .filter(|bridge| matches!(bridge, lir::StrongExternalLirBridgeV1::Callable(_)))
            .count(),
        1
    );
}
