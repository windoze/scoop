use super::super::machine_selection::Source;
use super::*;

pub(super) fn check_provider(input: scoop_mir_lower::MirTypeBridgeExportInputV1<'_>) {
    for property in input.public.property_interfaces().records() {
        if property.owner() != hir::PublicDeclarationOwnerV1::TopLevel
            || property.representation() != hir::PropertyRepresentationV1::RuntimeAccessor
        {
            continue;
        }
        let scoop_identity::PropertyOwner::Property(id) = property.declaration() else {
            panic!("a top-level property has a property identity")
        };
        let key = input
            .identities
            .canonical_key::<_, scoop_identity::SourceDeclarationKey>(id)
            .unwrap();
        for accessor in [
            Some(property.accessors().getter()),
            property.accessors().setter(),
        ]
        .into_iter()
        .flatten()
        {
            assert!(
                input
                    .ordinary
                    .export(
                        scoop_identity::DependencyCallableDeclarationId::PropertyAccessor(accessor)
                    )
                    .is_some(),
                "provider {:?} accessor {accessor} has no actual MIR export",
                key.name()
            );
        }
    }
}

pub(super) fn select<'a>(
    input: &mir::SingleConeStrongMirInput,
    output: &lir::SingleConeStrongLirOutput,
    mir: &mir::CrossConeMirTypeBridgeSectionV1<'_>,
    provider: &lir::ShapeLinkProviderV1<'a>,
    layout: &'a lir::LayoutAbiExportConstituentsV1,
    production: &'a lir::StrongProductionSectionV2,
) -> (
    lir::StrongProductionDependencySelectionV2<'a>,
    Vec<lir::StrongExternalInitializationUseV2>,
) {
    let uses = mir.initialization_uses().records();
    let mut source = Source::from_mir(input);
    source.physical.extend(uses.iter().map(|usage| {
        (
            usage.provider(),
            lir::ExternalStrongShapeSubjectV1::InitializationDescriptor(usage.dependency_unit()),
        )
    }));
    source.physical.sort_unstable();
    source.physical.dedup();
    let imports = source
        .physical
        .iter()
        .map(|(_, subject)| {
            lir::ExternalShapeLinkImportV1::replay(provider, *subject, output.module().cone)
                .unwrap()
        })
        .collect();
    let selected = lir::StrongProductionDependencySelectionV2::try_new(
        output.module().cone,
        output.module().meta.target_profile,
        &[layout],
        imports,
        &source.roots,
    )
    .unwrap();
    let definitions = production
        .initialization_registrations()
        .registrations()
        .iter()
        .map(|record| {
            lir::StrongInitializationUnitDefinitionRefV2::from_registrations(
                production.initialization_registrations(),
                record.semantic().unit(),
            )
            .unwrap()
        })
        .collect::<Vec<_>>();
    let projected =
        scoop_lir_lower::project_external_initialization_uses_v2(mir, &definitions, &selected)
            .unwrap();
    assert_eq!(projected.len(), uses.len());
    (selected, projected)
}
