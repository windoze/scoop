use super::*;

pub(crate) fn replay<'input>(
    link: &crate::DecodedCrossConeLayoutLinkOnlySections,
    graph: &mut crate::ValidatedGraphArtifact<'input>,
    foundation: &lir::ConeLirFoundation,
    strong: &lir::ConeProductionSectionV2,
    profile: &lir::CBridgeToolchainProfileV1,
) -> Result<ReplayedLayoutLinkObjectContentsV1, crate::SharedLirPhysicalError> {
    let partition = lir::ProducerUnitPartitionV1::from_foundation(foundation)
        .map_err(crate::StrongLinkMaterializationError::ProducerUnits)?;
    let wire = link.link_identity_closure_wire();
    let plan = wire
        .replay_materializations(graph.target_selection().target(), &partition)
        .map_err(crate::StrongLinkMaterializationError::Closure)?;
    let (scoop, generated) = crate::link_decode::object_directory::validate(graph, &plan)?;
    let target = graph.target_selection().target();
    let verify = || -> Result<_, LayoutLinkObjectContentsError> {
        let production = link
            .production_manifest_wire()
            .replay_c_bridge_production(strong.generated_bridge_plan(), profile)?;
        let bridges = verify_c_bridge_production_envelopes_v1(
            strong.generated_bridge_plan().clone(),
            production,
            profile,
            &plan,
            &generated,
        )?;
        let sites = wire.replay_digest_patch_inputs(&plan, strong.digest_finalization_plan())?;
        let objects = normalize_final_scoop_lir_objects_v1(&plan, &scoop, &sites)?;
        let candidates = objects.candidates();
        let symbols =
            PlannedStrongObjectSymbolSetV1::new(target, strong.canonical_definitions(), &plan)?;
        let builtins = verify_builtin_object_strong_relocations_v1(
            &plan,
            &symbols,
            &candidates,
            bridges,
            &generated,
        )?;
        let patches = verify_scoop_lir_digest_patch_sites_v1(
            builtins,
            foundation,
            strong.digest_finalization_plan().clone(),
            &candidates,
            &sites,
        )?;
        wire.replay_object_projections(&plan, &patches)?;
        let stackmaps = verify_scoop_lir_stackmaps_v1(
            patches.builtins().clone(),
            strong.safepoint_semantics(),
            &candidates,
        )?;
        registrations::replay(objects, generated, patches, stackmaps, strong)
    };
    verify().map_err(crate::SharedLirPhysicalError::from)
}
