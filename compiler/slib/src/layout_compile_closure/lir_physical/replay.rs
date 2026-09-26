use super::*;
use scoop_identity::ValidatedIdentityGraph;

pub(super) fn physical(
    layout: lir::DependencyResolvedCrossConeLayoutAbiSectionV1,
    strong: &lir::StrongProductionSectionV2,
    mir: &mir::DependencyResolvedCrossConeMirTypeBridgeSectionV1,
    dependencies: &[&PhysicalImportsReplayedCrossConeLayoutSections],
    identities: &mut ValidatedIdentityGraph,
) -> Result<lir::PhysicalImportsReplayedLayoutAbiSectionV1, SharedLirPhysicalError> {
    let mut providers = Vec::new();
    scoop_wire::allocation::try_reserve(&mut providers, dependencies.len(), &WirePath::root())?;
    for dependency in dependencies {
        let exports = dependency.layout.exports();
        providers.push(lir::ShapeLinkProviderV1::try_new(
            lir::ShapeLinkProviderPartsV1 {
                foundation: dependency.lir_foundation(),
                production: &dependency.strong,
                ordinary: &dependency.ordinary,
                layouts: exports.layouts(),
                callables: exports.callables(),
                descriptors: exports.descriptors(),
                dispatch: exports.dispatch(),
            },
        )?);
    }
    let support = support::SharedSupport {
        consumer: mir,
        dependencies,
    };
    Ok(layout.replay_physical_imports(
        strong.canonical_definitions(),
        &providers,
        &support,
        identities,
    )?)
}
