use scoop_hir::PublicNominalShapeRequirementsV1;
use scoop_identity::{ConeCoordinate, ConeIdentity, SourceDeclarationKey};
use scoop_lir as lir;
use scoop_wire::WirePath;

use super::{
    LirStrongProductionReplayedCrossConeLayoutSections, SharedLirStrongProductionError as Error,
};
use crate::layout_compile_decode::PreparedLayoutMirSemanticParts;

pub(super) fn replay(
    coordinate: ConeCoordinate,
    direct: &[ConeIdentity],
    target: lir::LirTargetProfile,
    strong: lir::InitializationAbiResolvedStrongProductionSectionV2,
    parts: PreparedLayoutMirSemanticParts<'_>,
    dependencies: &[&LirStrongProductionReplayedCrossConeLayoutSections<'_>],
) -> Result<lir::ReplayedStrongProductionSectionV2, Error> {
    let provider = parts.lir_foundation.producer();
    let external = strong.reconstruct_external_bridges(provider, parts.identities)?;
    super::dependencies::validate_external_bridges(&external, dependencies)?;
    let (type_definitions, initialization_definitions) =
        super::dependencies::definitions(provider, dependencies, &external)?;
    let roots = PublicNominalShapeRequirementsV1::from_shared_surface(
        provider,
        parts.hir_core.direct_public_surface(),
        parts.hir_foundation.as_canonical(),
        parts.hir_interface.nominal_interfaces(),
        parts.hir_interface.callable_interfaces(),
    )?;
    let path = WirePath::root();
    let mut sources = Vec::new();
    scoop_wire::allocation::try_reserve(&mut sources, roots.roots().len(), &path)?;
    for root in roots.roots() {
        let key = parts
            .identities
            .canonical_key::<_, SourceDeclarationKey>(root.source())?;

        sources.push(key.as_ref().clone());
    }
    let entry = match parts.mir_core.entry_bridge() {
        scoop_mir::EntryMirBridgeBranchV1::Library => lir::EntryProductionSourceV1::Library,
        scoop_mir::EntryMirBridgeBranchV1::Executable(bridge) => {
            lir::EntryProductionSourceV1::executable(bridge.source().clone())
        }
    };
    let initialization = strong
        .initialization_cycle_abi()
        .map(|abi| Box::new(abi.clone()));
    Ok(strong.replay(
        coordinate,
        direct,
        target,
        parts.lir_foundation,
        external,
        entry,
        &sources,
        initialization,
        &type_definitions,
        &initialization_definitions,
    )?)
}
