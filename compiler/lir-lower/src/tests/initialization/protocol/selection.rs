use super::*;
use fixture::{ImportedInitialization, imported_initialization};

fn lower_selected(
    fixture: &ImportedInitialization,
    selected: &lir::SelectedExternalLirSet,
) -> Result<lir::SingleConeStrongLirOutput, crate::StrongLirLoweringError> {
    crate::lower(
        &fixture.input,
        crate::RuntimeStringDescriptor::External(fixture.runtime_string),
        selected,
        lir::LirTargetProfile::DARWIN_AARCH64,
    )
}

#[test]
fn initialization_selection_requires_its_explicit_role() {
    let fixture = imported_initialization();
    let ordinary = lir::SelectedExternalLirSet::try_from_callables(
        fixture.input.module().cone,
        vec![fixture.callable.clone()],
    )
    .unwrap();
    assert!(matches!(
        lower_selected(&fixture, &ordinary),
        Err(crate::StrongLirLoweringError::ExternalCallableMismatch { index: 0, .. })
    ));
    let empty = lir::SelectedExternalLirSet::empty(fixture.input.module().cone);
    assert!(matches!(
        lower_selected(&fixture, &empty),
        Err(crate::StrongLirLoweringError::ExternalCallableCountMismatch { mir: 1, lir: 0 })
    ));
    let foreign = lir::SelectedExternalLirSet::empty(ConeIdentity::CORE);
    assert!(matches!(
        lower_selected(&fixture, &foreign),
        Err(crate::StrongLirLoweringError::ForeignExternalLirSelection { .. })
    ));
}

#[test]
fn initialization_selection_uses_the_shared_gc_effect_check() {
    let fixture = imported_initialization();
    let bridge = fixture.callable.bridge();
    let canonical = bridge.abi_signature();
    let no_gc = scoop_identity::CanonicalScoopAbiFunctionSignature::new(
        canonical.signature().clone(),
        canonical.arguments().to_vec(),
        canonical.result(),
        scoop_identity::GcEffect::NoGc,
    )
    .unwrap();
    let callable = lir::SelectedDependencyLirCallableV1::new(
        ConeIdentity::CORE,
        bridge.declaration(),
        bridge.target(),
        no_gc,
        bridge.calling_convention(),
        lir::ExternalCallableRootPlan::NoGc,
    )
    .unwrap();
    let selected = lir::SelectedExternalLirSet::empty(fixture.input.module().cone)
        .with_initialization_cycle(callable)
        .unwrap();
    assert!(matches!(
        lower_selected(&fixture, &selected),
        Err(
            crate::StrongLirLoweringError::ExternalCallableGcEffectMismatch {
                mir: mir::GcEffect::Managed,
                lir: scoop_identity::GcEffect::NoGc,
                ..
            }
        )
    ));
}

#[test]
fn runtime_string_input_checks_the_actual_descriptor_and_provider() {
    let fixture = imported_initialization();
    let selected = lir::SelectedExternalLirSet::empty(fixture.input.module().cone)
        .with_initialization_cycle(fixture.callable.clone())
        .unwrap();
    let wrong_type =
        lir::ExternalTypeDescriptor::new(ConeIdentity::CORE, mir::core_unit_exact_type()).unwrap();
    assert!(matches!(
        crate::lower(
            &fixture.input,
            crate::RuntimeStringDescriptor::External(wrong_type),
            &selected,
            lir::LirTargetProfile::DARWIN_AARCH64
        ),
        Err(crate::StrongLirLoweringError::RuntimeStringExactMismatch { .. })
    ));
    let wrong_provider = lir::ExternalTypeDescriptor::new(
        ConeIdentity::SINGLE_FILE,
        fixture.runtime_string.target(),
    )
    .unwrap();
    assert!(matches!(
        crate::lower(
            &fixture.input,
            crate::RuntimeStringDescriptor::External(wrong_provider),
            &selected,
            lir::LirTargetProfile::DARWIN_AARCH64
        ),
        Err(crate::StrongLirLoweringError::RuntimeStringDescriptorOwnership { .. })
    ));
    let unrelated = scoop_identity::ConeCoordinate::new("test", "unrelated", "1.0.0")
        .unwrap()
        .identity()
        .unwrap();
    let wrong_provider =
        lir::ExternalTypeDescriptor::new(unrelated, fixture.runtime_string.target()).unwrap();
    assert!(matches!(
        crate::lower(&fixture.input, crate::RuntimeStringDescriptor::External(wrong_provider), &selected, lir::LirTargetProfile::DARWIN_AARCH64),
        Err(crate::StrongLirLoweringError::RuntimeStringProviderMismatch { expected: ConeIdentity::CORE, actual }) if actual == unrelated
    ));
}

#[test]
fn an_ordinary_provider_supplies_string_and_initialization_through_shared_records() {
    let provider = scoop_identity::ConeCoordinate::new("test", "services", "1.0.0")
        .unwrap()
        .identity()
        .unwrap();
    let fixture = fixture::imported_initialization_from(provider);
    let selected = lir::SelectedExternalLirSet::empty(fixture.input.module().cone)
        .with_initialization_cycle(fixture.callable.clone())
        .unwrap();
    let output = lower_selected(&fixture, &selected).unwrap();
    let module = output.module();
    let lir::TypeDescriptorRef::External(id) = module.meta.well_known_type_descriptors.string
    else {
        panic!("the selected String descriptor is external");
    };
    assert_eq!(
        module.meta.external_type_descriptors[id],
        fixture.runtime_string
    );
    assert_eq!(fixture.runtime_string.provider(), provider);
    assert_eq!(
        module
            .meta
            .external_callables
            .iter()
            .next()
            .unwrap()
            .1
            .provider(),
        provider
    );
    let production = output
        .build_production_section(
            scoop_identity::ConeCoordinate::reserved_single_file(),
            &[provider],
            lir::EntryProductionSourceV1::Library,
        )
        .unwrap();
    assert_eq!(production.image_plan().dependencies(), &[provider]);
    assert_eq!(production.external_bridges().bridges().len(), 2);
    assert!(
        production
            .external_bridges()
            .bridges()
            .iter()
            .all(|reference| reference.provider() == provider)
    );
}

#[test]
fn unused_mir_string_does_not_need_a_materialized_type_entry() {
    let mut builder = Builder::new();
    let entry = builder.user_fn("main", Arena::new(), Vec::new());
    let mut module = builder.finish(entry);
    let string = module
        .meta
        .source_exact_types
        .get(&mir::Type::String)
        .unwrap()
        .identity_record()
        .id();
    module.classes = Arena::new();
    module.meta.source_exact_types = mir::SourceExactTypeIdentities::checked(
        module
            .meta
            .source_exact_types
            .iter()
            .filter(|identity| identity.ty() != &mir::Type::String)
            .cloned()
            .collect(),
    )
    .unwrap();
    let input = crate::tests::seal_strong_input(module);
    let descriptor = lir::ExternalTypeDescriptor::new(ConeIdentity::CORE, string).unwrap();
    let output = crate::lower(
        &input,
        crate::RuntimeStringDescriptor::External(descriptor),
        &lir::SelectedExternalLirSet::empty(input.module().cone),
        lir::LirTargetProfile::DARWIN_AARCH64,
    )
    .unwrap();
    let lir::TypeDescriptorRef::External(id) =
        output.module().meta.well_known_type_descriptors.string
    else {
        panic!("the runtime role retains the selected external descriptor");
    };
    assert_eq!(
        output.module().meta.external_type_descriptors[id],
        descriptor
    );
}
