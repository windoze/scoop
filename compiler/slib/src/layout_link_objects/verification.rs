use super::*;

pub(crate) fn replay<'input>(
    link: &crate::DecodedCrossConeLayoutLinkOnlySections,
    graph: &mut crate::ValidatedGraphArtifact<'input>,
    foundation: &lir::OdrFreeLirFoundation,
    strong: &lir::ReplayedStrongProductionSectionV2,
    profile: &lir::CBridgeToolchainProfileV1,
) -> Result<ReplayedLayoutLinkObjectContentsV1<'input>, crate::SharedLirPhysicalError> {
    let partition = crate::link_decode::materializations::producer_units(
        foundation,
        strong.generated_bridge_plan(),
        graph.envelope.meter_mut(),
    )?;
    let wire = link.link_identity_closure_wire();
    let plan = wire
        .replay_materializations(&partition, graph.envelope.meter_mut())
        .map_err(crate::StrongLinkMaterializationError::Closure)?;
    let (scoop, generated) = crate::link_decode::object_directory::validate(graph, &plan)?;
    let target = graph.target_selection().target();
    let meter = graph.envelope.meter_mut();
    let verify = || -> Result<_, LayoutLinkObjectContentsError> {
        let costs = resources::ReplayCosts::new(&scoop, &generated, meter)?;
        let production = link.production_manifest_wire().replay_c_bridge_production(
            strong.generated_bridge_plan(),
            profile,
            meter,
        )?;
        costs.bridge_envelopes(&generated, strong.generated_bridge_plan(), meter)?;
        let bridges = verify_c_bridge_production_envelopes_v1(
            strong.generated_bridge_plan().clone(),
            production,
            profile,
            &plan,
            &generated,
        )?;
        let sites =
            wire.replay_digest_patch_inputs(&plan, strong.digest_finalization_plan(), meter)?;
        costs.normalization(&scoop, sites.len(), meter)?;
        let objects = normalize_final_scoop_lir_objects_v1(&plan, &scoop, &sites)?;
        let candidates = objects.candidates();
        resources::symbol_plan(strong, meter)?;
        let symbols =
            PlannedStrongObjectSymbolSetV1::new(target, strong.canonical_definitions(), &plan)?;
        costs.strong(&symbols, meter)?;
        let builtins = verify_builtin_object_strong_relocations_v1(
            &plan,
            &symbols,
            &candidates,
            bridges,
            &generated,
        )?;
        resources::digest_plan(strong, meter)?;
        costs.digest_sites(&sites, meter)?;
        let patches = verify_scoop_lir_digest_patch_sites_v1(
            builtins,
            foundation,
            strong.digest_finalization_plan().clone(),
            &candidates,
            &sites,
        )?;
        wire.replay_object_projections(&plan, &patches, meter)?;
        costs.stackmaps(
            &candidates,
            patches.builtins(),
            strong.safepoint_registrations().registrations().len(),
            meter,
        )?;
        let stackmaps = verify_scoop_lir_stackmaps_v1(
            patches.builtins().clone(),
            strong.safepoint_semantics(),
            &candidates,
        )?;
        registrations::replay(
            objects, generated, patches, stackmaps, strong, &costs, meter,
        )
    };
    verify().map_err(crate::SharedLirPhysicalError::from)
}
