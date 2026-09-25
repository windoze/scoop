use super::*;
use scoop_identity::ValidatedIdentityGraph;

pub(super) fn physical<'a>(
    layout: lir::DependencyResolvedCrossConeLayoutAbiSectionV1,
    strong: &lir::ReplayedStrongProductionSectionV2,
    mir: &mir::DependencyResolvedCrossConeMirTypeBridgeSectionV1,
    dependencies: &[&'a PhysicalImportsReplayedCrossConeLayoutSections<'_, 'a>],
    identities: &mut ValidatedIdentityGraph,
) -> Result<lir::PhysicalImportsReplayedLayoutAbiSectionV1<'a>, SharedLirPhysicalError> {
    let mut providers = Vec::new();
    scoop_wire::allocation::try_reserve(&mut providers, dependencies.len(), &WirePath::root())?;
    for dependency in dependencies {
        let exports = dependency.layout.exports();
        let production = dependency.strong.replay_layout_exports(exports)?;
        providers.push(lir::ShapeLinkProviderV1::from_replayed(
            dependency.prepared.lir_foundation(),
            &dependency.ordinary,
            production,
        )?);
    }
    let support = support::SharedSupport {
        consumer: mir,
        dependencies,
    };
    Ok(layout.replay_physical_imports::<Infallible>(
        strong.canonical_definitions(),
        &providers,
        &support,
        identities,
    )?)
}
