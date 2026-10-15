use super::*;
use scoop_identity::ValidatedIdentityGraph;

pub(super) fn physical(
    layout: lir::DependencyResolvedCrossConeLayoutAbiSectionV1,
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
                definitions: dependency.strong.canonical_definitions(),
                static_storages: dependency.strong.static_storage_registrations(),
                initialization: dependency.strong.initialization_registrations(),
                ordinary: &dependency.ordinary,
                layouts: exports.layouts(),
                callables: exports.callables(),
                descriptors: exports.descriptors(),
                dispatch: exports.dispatch(),
            },
        )?);
    }
    Ok(layout.replay_physical_imports(&providers, identities)?)
}
