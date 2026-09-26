use scoop_identity::ConeCoordinate;
use scoop_wire::{decode_canonical, encode};

use crate::*;

mod callables;
mod descriptors;
mod fixture;
mod initialization_fixture;
mod lir_fixture;
mod physical_replay;
use fixture::{Provider, consumer_layout_section};
use initialization_fixture::attach_eager_initialization;
use lir_fixture::{consumer_module, pointer_result_signature};

const TARGET: LirTargetProfile = LirTargetProfile::DARWIN_AARCH64;

#[test]
fn pending_selection_drives_dependency_descriptor_dispatch_and_initialization_production() {
    exercise_dependency_production(Provider::new());
}

#[test]
fn core_shapes_use_the_same_dependency_descriptor_production_and_wire_replay() {
    exercise_dependency_production(Provider::for_coordinate(ConeCoordinate::reserved_core()));
}

fn exercise_dependency_production(provider: Provider) {
    let consumer_coordinate = ConeCoordinate::new("test", "consumer", "1.0.0").unwrap();
    let mut consumer = consumer_module(&consumer_coordinate);
    let consumer_exact = consumer.meta.exact_types[0].id();
    let consumer_unit = attach_eager_initialization(&mut consumer, "consumerValue", consumer_exact);
    let consumer_foundation = OdrFreeLirFoundation::from_module(&consumer).unwrap();
    let consumer_definitions =
        StrongObjectSymbolSurfaceV1::from_odr_free_foundation(&consumer_foundation).unwrap();
    let provider_view = provider.shape_link_provider();
    let descriptor_import = ExternalShapeLinkImportV1::replay(
        &provider_view,
        ExternalStrongShapeSubjectV1::TypeDescriptor(provider.exact),
        consumer.cone,
        &consumer_definitions,
        &NoShapeLinkSupportV1,
    )
    .unwrap();
    let callable_import = ExternalShapeLinkImportV1::replay(
        &provider_view,
        ExternalStrongShapeSubjectV1::Callable(provider.callable),
        consumer.cone,
        &consumer_definitions,
        &NoShapeLinkSupportV1,
    )
    .unwrap();
    let initialization_import = ExternalShapeLinkImportV1::replay(
        &provider_view,
        ExternalStrongShapeSubjectV1::InitializationDescriptor(provider.initialization_unit),
        consumer.cone,
        &consumer_definitions,
        &provider.initialization_support(),
    )
    .unwrap();
    let terminal = provider.layout_section();
    let dependencies = [&terminal];
    let source = ProductionSelectionSource::new(
        vec![
            LayoutAbiDependencyV1::new(
                provider.identity,
                LayoutAbiSemanticTargetV1::Descriptor(provider.exact),
            ),
            LayoutAbiDependencyV1::new(
                provider.identity,
                LayoutAbiSemanticTargetV1::Callable(provider.callable),
            ),
        ],
        vec![
            (
                provider.identity,
                ExternalStrongShapeSubjectV1::TypeDescriptor(provider.exact),
            ),
            (
                provider.identity,
                ExternalStrongShapeSubjectV1::Callable(provider.callable),
            ),
            (
                provider.identity,
                ExternalStrongShapeSubjectV1::InitializationDescriptor(
                    provider.initialization_unit,
                ),
            ),
        ],
    );
    let selected = StrongProductionDependencySelectionV2::try_new(
        consumer.cone,
        TARGET,
        &dependencies,
        vec![descriptor_import, callable_import, initialization_import],
        &source,
    )
    .unwrap();

    let descriptor = selected
        .materialize_type_descriptor(provider.identity, provider.exact)
        .unwrap();
    let callable = selected
        .materialize_dispatch_callable(
            provider.identity,
            provider.callable,
            pointer_result_signature(),
            &consumer.enums,
        )
        .unwrap();
    let descriptor_id = consumer.meta.external_type_descriptors.alloc(descriptor);
    let callable_id = consumer.meta.external_callables.alloc(callable);
    let local = consumer.meta.type_descriptors.iter_mut().next().unwrap().1;
    local.parent = Some(TypeDescriptorRef::External(descriptor_id));
    local.vtable = VtableRecord::new(
        &local.identity,
        vec![DispatchEntry {
            callable: CallableRef::External(callable_id),
        }],
    )
    .unwrap();
    let initialization_definition = provider.initialization_definition();
    let initialization_use = StrongExternalInitializationUseV2::try_new(
        consumer_unit,
        initialization_definition,
        &selected,
    )
    .unwrap();

    let output = SingleConeStrongLirOutput::try_new(consumer, Vec::new(), None).unwrap();
    let pending = output
        .build_production_section_v2(
            consumer_coordinate.clone(),
            &[provider.identity],
            EntryProductionSourceV1::Library,
            &selected,
            &[initialization_use],
        )
        .unwrap();
    let complete = consumer_layout_section(
        &consumer_coordinate,
        &output,
        pending.registration_production(),
        &provider,
        &dependencies,
        selected.physical_imports().records().to_vec(),
    );
    let produced = pending.validate_layout_abi(&complete).unwrap();
    let semantic = produced.type_registrations().registrations()[0].semantic();

    assert!(matches!(
        semantic.parent(),
        Some(StrongTypeDescriptorRefV2::DependencyExternal { provider: found, exact })
            if found == provider.identity && exact == provider.exact
    ));
    let initialization_semantic = produced
        .initialization_registrations()
        .registrations()
        .iter()
        .find(|registration| registration.semantic().unit() == consumer_unit)
        .unwrap()
        .semantic();
    assert!(matches!(
        initialization_semantic.dependencies(),
        [dependency]
            if matches!(
                dependency.kind(),
                StrongInitializationDependencyKindV2::DependencyExternalUnit {
                    provider: found,
                    unit_ref,
                } if found == provider.identity && unit_ref.unit() == provider.initialization_unit
            )
    ));
    assert!(matches!(
        semantic.vtable().slots(),
        [StrongTypeDispatchCallableRefV2::DependencyExternal { provider: found, body }]
            if *found == provider.identity && *body == provider.callable_body
    ));
    let section = produced;
    let bytes = encode(&section).unwrap();
    let decoded: DecodedStrongProductionSectionV2 = decode_canonical(&bytes).unwrap();
    let definitions = [
        StrongShapeDefinitionRefV1::from_foundation(
            ExternalStrongShapeSubjectV1::TypeDescriptor(provider.exact),
            provider.output.foundation(),
        )
        .unwrap(),
        StrongShapeDefinitionRefV1::from_foundation(
            ExternalStrongShapeSubjectV1::Callable(provider.callable),
            provider.output.foundation(),
        )
        .unwrap(),
    ];
    let replayed = decoded
        .replay(
            consumer_coordinate.clone(),
            &[provider.identity],
            TARGET,
            output.foundation(),
            section.external_bridges().clone(),
            EntryProductionSourceV1::Library,
            &[],
            None,
            &StrongTypeReferenceDefinitionsV2::new(output.foundation().producer(), &definitions)
                .unwrap(),
            &StrongInitializationDefinitionCatalogV2::new(
                output.foundation().producer(),
                &[initialization_definition],
            )
            .unwrap(),
        )
        .unwrap();
    physical_replay::check_join(&provider, &replayed, &complete);
    let validated = replayed.validate_layout_abi(&complete).unwrap();
    assert_eq!(
        validated.type_registrations(),
        section.registration_production().types()
    );
    assert_eq!(
        validated.initialization_registrations(),
        section.registration_production().initialization_units()
    );
}

#[test]
fn pending_selection_rejects_an_uncommitted_terminal_callable() {
    let provider = Provider::new();
    let consumer_coordinate = ConeCoordinate::new("test", "narrow-consumer", "1.0.0").unwrap();
    let consumer = consumer_module(&consumer_coordinate);
    let consumer_foundation = OdrFreeLirFoundation::from_module(&consumer).unwrap();
    let consumer_definitions =
        StrongObjectSymbolSurfaceV1::from_odr_free_foundation(&consumer_foundation).unwrap();
    let descriptor_import = ExternalShapeLinkImportV1::replay(
        &provider.shape_link_provider(),
        ExternalStrongShapeSubjectV1::TypeDescriptor(provider.exact),
        consumer.cone,
        &consumer_definitions,
        &NoShapeLinkSupportV1,
    )
    .unwrap();
    let terminal = provider.layout_section();
    let dependencies = [&terminal];
    let source = ProductionSelectionSource::new(
        vec![LayoutAbiDependencyV1::new(
            provider.identity,
            LayoutAbiSemanticTargetV1::Descriptor(provider.exact),
        )],
        vec![(
            provider.identity,
            ExternalStrongShapeSubjectV1::TypeDescriptor(provider.exact),
        )],
    );
    let selected = StrongProductionDependencySelectionV2::try_new(
        consumer.cone,
        TARGET,
        &dependencies,
        vec![descriptor_import],
        &source,
    )
    .unwrap();

    assert!(matches!(
        selected.materialize_dispatch_callable(
            provider.identity,
            provider.callable,
            pointer_result_signature(),
            &consumer.enums,

        ),
        Err(LayoutExternalMaterializationError::MissingCallable {
            provider: found,
            target,
        }) if found == provider.identity && target == provider.callable
    ));
}

struct ProductionSelectionSource {
    roots: Vec<LayoutAbiDependencyV1>,
    imports: Vec<(ConeIdentity, ExternalStrongShapeSubjectV1)>,
}

impl ProductionSelectionSource {
    fn new(
        mut roots: Vec<LayoutAbiDependencyV1>,
        mut imports: Vec<(ConeIdentity, ExternalStrongShapeSubjectV1)>,
    ) -> Self {
        roots.sort_unstable();
        imports.sort_unstable();
        Self { roots, imports }
    }
}

impl LayoutAbiSectionSourceAuthorityV1<()> for ProductionSelectionSource {
    fn validate_local_exports(&self, _: &LayoutAbiExportConstituentsV1) -> Result<(), ()> {
        Ok(())
    }

    fn committed_semantic_roots(&self) -> Result<&[LayoutAbiDependencyV1], ()> {
        Ok(&self.roots)
    }

    fn validate_physical_imports(&self, imports: &[ExternalShapeLinkImportV1]) -> Result<(), ()> {
        let actual = imports
            .iter()
            .map(|record| (record.provider(), record.subject()))
            .collect::<Vec<_>>();
        (actual == self.imports).then_some(()).ok_or(())
    }
}
