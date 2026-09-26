use super::*;

pub(in super::super) fn identities(
    hir: &hir::DependencyHirOutput,
    mir: &mir::SingleConeStrongMirInput,
    lir: Option<&lir::SingleConeStrongLirOutput>,
    core: &scoop_slib::DecodedCrossConeHirFrontSections<'_>,
) -> (
    ValidatedIdentityGraph,
    lir::OdrFreeLirFoundation,
    hir::OdrFreeHirFoundation,
) {
    let mut pending = PendingIdentityValidation::new();
    pending.register_authority(ConeIdentity::CORE).unwrap();
    core.hir_foundation_wire()
        .register_identities(&mut pending)
        .unwrap();
    core.mir_foundation_wire()
        .register_identities(&mut pending)
        .unwrap();
    core.lir_foundation_wire()
        .register_identities(&mut pending)
        .unwrap();
    core.hir_foundation_wire()
        .resolve_identities(&mut pending)
        .unwrap();
    core.mir_foundation_wire()
        .resolve_identities(&mut pending)
        .unwrap();
    core.lir_foundation_wire()
        .resolve_identities(&mut pending)
        .unwrap();
    let mut core_graph = pending.finish().unwrap();
    let core_hir: hir::DecodedHirFoundation = decoded(core.hir_foundation_wire());
    let core_hir = hir::OdrFreeHirFoundation::from_validated(
        core_hir
            .validate(core.coordinate(), &mut core_graph)
            .unwrap(),
    )
    .unwrap();
    let core_lir: lir::DecodedLirFoundation = decoded(core.lir_foundation_wire());
    let core_lir = lir::OdrFreeLirFoundation::from_validated(
        core_lir
            .validate(ConeIdentity::CORE, &mut core_graph)
            .unwrap(),
    )
    .unwrap();
    let hir = hir::CanonicalHirFoundation::from_type_semantics_output(hir).unwrap();
    let provider = mir.module().cone;
    let mir = mir.foundation().as_canonical();
    let lir_foundation: Option<lir::DecodedLirFoundation> =
        lir.map(|lir| decoded(lir.foundation().as_canonical()));
    let mut pending = PendingIdentityValidation::new();
    pending.register_authority(provider).unwrap();
    pending.register_authority(ConeIdentity::CORE).unwrap();
    hir.register_identities(&mut pending).unwrap();
    mir.register_identities(&mut pending).unwrap();
    if let Some(foundation) = &lir_foundation {
        foundation.register_identities(&mut pending).unwrap();
    }
    pending
        .register_external_graph_authorities(&core_graph)
        .unwrap();
    if let Some(foundation) = &lir_foundation {
        foundation.resolve_identities(&mut pending).unwrap();
    }
    (pending.finish().unwrap(), core_lir, core_hir)
}
