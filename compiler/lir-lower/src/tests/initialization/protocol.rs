use super::*;

#[test]
fn initialization_function_uses_the_ordinary_callable_abi_export() {
    let mut builder = Builder::new();
    let function = builder.user_fn("exported", Arena::new(), Vec::new());
    let cycle_function =
        builder.user_fn("__scoopThrowInitializationCycle", Arena::new(), Vec::new());
    let mut module = builder.finish_with_types(function, ConeIdentity::CORE, vec![]);
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
    let selected = mir::SelectedExternalMirSet::empty(module.cone);
    let mir_output = mir::DependencyMirOutput::try_new(module, selected).unwrap();
    let foundation = mir_output.strong_foundation().unwrap();
    let strong = mir::StrongCallableBridgeSurfaceV1::from_foundation(&foundation);
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
    let input = mir::ConeMirInput::try_new(mir_output, production, Vec::new()).unwrap();

    let output = crate::lower(
        &input,
        &[],
        &lir::SelectedExternalLirSet::empty(ConeIdentity::CORE),
        lir::LirTargetProfile::DARWIN_AARCH64,
    )
    .unwrap();
    let target = scoop_identity::StrongCallableDefinitionOwner::Function(cycle_definition);
    let export = mir::ParamFreeMirCallableExportV1::try_new(
        scoop_identity::DependencyCallableDeclarationId::Function(cycle_definition),
        target,
        exact.clone(),
        scoop_mir::GcEffect::Managed,
    )
    .unwrap();
    let mir_bridge = mir::CrossConeMirBridgeSectionV1::try_new(
        ConeIdentity::CORE,
        input.foundation(),
        vec![export],
        Vec::new(),
    )
    .unwrap();
    let lir_bridge = crate::lower_cross_cone_bridge_section(&input, &mir_bridge, &output).unwrap();
    let callable = lir_bridge.exports()[0].callable_abi();
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
    let selected_lir = lir::SelectedExternalLirSet::try_from_callables(
        ConeIdentity::SINGLE_FILE,
        vec![lir_callable],
    )
    .unwrap();

    assert!(matches!(
        crate::lower(
            &ordinary_input,
            &[],
            &selected_lir,
            lir::LirTargetProfile::DARWIN_AARCH64,
        ),
        Err(LirLoweringError::MissingDependencyLayoutSelection { .. })
    ));
    let output = crate::lower(
        &ordinary_input,
        &[runtime_string],
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
    assert_eq!(
        external.legacy_declaration(),
        Some(scoop_identity::DependencyCallableDeclarationId::Function(
            match target {
                scoop_identity::StrongCallableDefinitionOwner::Function(function) => function,
                _ => panic!("source function"),
            }
        ))
    );
}
