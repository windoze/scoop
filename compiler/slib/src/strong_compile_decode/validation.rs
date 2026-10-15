//! Shared foundation and production relation validation for strong profiles.

use super::*;

pub(crate) fn validate_cross_cone_profile_foundations(
    graph: &mut ValidatedGraphArtifact<'_>,
    identities: &mut ValidatedIdentityGraph,
    hir: DecodedHirFoundation,
    mir: DecodedMirFoundation,
    lir: DecodedLirFoundation,
    dependencies: &[&scoop_hir::CanonicalHirFoundation],
) -> Result<CanonicalFoundationSet, StrongProfileFoundationError> {
    let coordinate = graph.coordinate().clone();
    let producer = graph.identity();

    let hir = hir
        .validate_with_dependency_sources(&coordinate, identities, dependencies)
        .map_err(StrongProfileFoundationError::HirStructure)?;
    let mir = mir
        .validate(identities)
        .map_err(StrongProfileFoundationError::MirStructure)?;
    let lir = lir
        .validate(producer, identities)
        .map_err(StrongProfileFoundationError::LirStructure)?;
    let hir = hir.into_canonical();
    let mir = mir.into_canonical();
    let lir = ConeLirFoundation::from_validated(lir);
    if graph.envelope.manifest().compatibility().artifact_profile()
        != &crate::ArtifactCapabilityProfile::CROSS_CONE_GENERIC.id()
    {
        hir.require_strong()
            .map_err(StrongProfileFoundationError::HirOdr)?;
        mir.require_strong()
            .map_err(StrongProfileFoundationError::MirOdr)?;
        lir.require_strong()
            .map_err(StrongProfileFoundationError::LirOdr)?;
    }
    Ok(CanonicalFoundationSet {
        hir: Rc::new(hir),
        mir: Rc::new(mir),
        lir,
    })
}
