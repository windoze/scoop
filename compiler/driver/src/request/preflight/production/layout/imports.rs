//! Physical imports follow actual MIR materializations and initialization uses.

use super::*;

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
