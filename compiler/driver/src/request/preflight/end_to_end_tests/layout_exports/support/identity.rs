use super::*;

pub(in super::super) fn identities(
    hir: &hir::DependencyHirOutput,
    mir: &mir::ConeMirInput,
    lir: Option<&lir::ConeLirOutput>,
    core: &scoop_slib::PhysicalImportsReplayedCrossConeLayoutSections,
) -> (
    ValidatedIdentityGraph,
    lir::ConeLirFoundation,
    hir::CanonicalHirFoundation,
) {
    let hir = hir::CanonicalHirFoundation::from_type_semantics_output(hir).unwrap();
    let provider = mir.module().cone;
    let mir = mir.foundation();
    let lir_foundation = lir.map(|lir| lir.foundation().as_canonical());
    let mut pending = PendingIdentityValidation::new();
    pending.register_authority(provider).unwrap();
    pending.register_authority(ConeIdentity::CORE).unwrap();
    hir.register_identities(&mut pending).unwrap();
    mir.register_identities(&mut pending).unwrap();
    if let Some(foundation) = &lir_foundation {
        foundation.register_identities(&mut pending).unwrap();
    }
    pending
        .register_external_graph_authorities(core.identity_graph())
        .unwrap();
    (
        pending.finish().unwrap(),
        core.lir_foundation().clone(),
        core.hir_foundation().clone(),
    )
}
