//! Physical imports follow actual MIR materializations and initialization uses.

use super::*;

pub(super) fn select<'a>(
    input: &mir::SingleConeStrongMirInput,
    section: &mir::CrossConeMirTypeBridgeSectionV1<'_>,
    dependencies: &[&'a slib::PhysicalImportsReplayedCrossConeLayoutSections],
    target: lir::LirTargetProfile,
) -> Result<lir::StrongProductionDependencySelectionV2<'a>, Error> {
    let mut roots = Vec::new();
    let mut physical = Vec::new();
    for shape in input
        .materialization()
        .dependency_generated_nominal_shapes()
    {
        roots.push(lir::LayoutAbiDependencyV1::new(
            shape.provider(),
            lir::LayoutAbiSemanticTargetV1::ShapeSupport(shape.source()),
        ));
        physical.push((
            shape.provider(),
            lir::ExternalStrongShapeSubjectV1::TypeDescriptor(shape.exact()),
        ));
    }
    for source in input.module().meta.source_exact_types.iter() {
        if !matches!(
            source.ty(),
            mir::Type::String | mir::Type::Class(_) | mir::Type::Interface(_)
        ) {
            continue;
        }
        let mir::SourceExactTypeOwner::Cone(provider) = source.owner() else {
            continue;
        };
        if provider == input.module().cone {
            continue;
        }
        let exact = source.identity_record().id();
        roots.push(lir::LayoutAbiDependencyV1::new(
            provider,
            lir::LayoutAbiSemanticTargetV1::Descriptor(exact),
        ));
        physical.push((
            provider,
            lir::ExternalStrongShapeSubjectV1::TypeDescriptor(exact),
        ));
        if source.ty() == &mir::Type::String && !input.materialization().strings().is_empty() {
            physical.push((
                provider,
                lir::ExternalStrongShapeSubjectV1::TypeRegistration(exact),
            ));
        }
    }
    physical.extend(section.initialization_uses().records().iter().map(|usage| {
        (
            usage.provider(),
            lir::ExternalStrongShapeSubjectV1::InitializationDescriptor(usage.dependency_unit()),
        )
    }));
    roots.sort_unstable();
    roots.dedup();
    physical.sort_unstable();
    physical.dedup();
    let providers = dependencies
        .iter()
        .map(|dependency| {
            let exports = dependency.lir_exports();
            lir::ShapeLinkProviderV1::try_new(lir::ShapeLinkProviderPartsV1 {
                foundation: dependency.lir_foundation(),
                production: dependency.lir_strong_production(),
                ordinary: dependency.lir_cross_cone_bridge(),
                layouts: exports.layouts(),
                callables: exports.callables(),
                descriptors: exports.descriptors(),
                dispatch: exports.dispatch(),
            })
        })
        .collect::<Result<Vec<_>, _>>()
        .map_err(Error::Shape)?;
    let imports = physical
        .into_iter()
        .map(|(identity, subject)| {
            let provider = providers
                .iter()
                .find(|provider| provider.provider() == identity)
                .ok_or(lir::ShapeLinkError::Provider)?;
            lir::ExternalShapeLinkImportV1::replay(provider, subject, input.module().cone)
        })
        .collect::<Result<Vec<_>, _>>()
        .map_err(Error::Shape)?;
    lir::StrongProductionDependencySelectionV2::try_new(
        input.module().cone,
        target,
        &dependencies
            .iter()
            .map(|d| d.lir_exports())
            .collect::<Vec<_>>(),
        imports,
        &roots,
    )
    .map_err(Error::Selection)
}

pub(super) fn initialization(
    section: &mir::CrossConeMirTypeBridgeSectionV1<'_>,
    dependencies: &[&slib::PhysicalImportsReplayedCrossConeLayoutSections],
    selected: &lir::StrongProductionDependencySelectionV2<'_>,
) -> Result<Vec<lir::StrongExternalInitializationUseV2>, Error> {
    let definitions = section
        .initialization_uses()
        .records()
        .iter()
        .map(|usage| {
            dependencies
                .iter()
                .find(|dependency| dependency.identity() == usage.provider())
                .and_then(|dependency| {
                    lir::StrongInitializationUnitDefinitionRefV2::from_registrations(
                        dependency
                            .lir_strong_production()
                            .initialization_registrations(),
                        usage.dependency_unit(),
                    )
                })
                .ok_or(
                    scoop_lir_lower::StrongProductionV2ProjectionError::MissingDefinition {
                        provider: usage.provider(),
                        unit: usage.dependency_unit(),
                    },
                )
        })
        .collect::<Result<Vec<_>, _>>()
        .map_err(Error::Initialization)?;
    let mut definitions = definitions;
    definitions.sort_unstable_by_key(|definition| (definition.provider(), definition.unit()));
    definitions.dedup();
    scoop_lir_lower::project_external_initialization_uses_v2(section, &definitions, selected)
        .map_err(Error::Initialization)
}
