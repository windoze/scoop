use super::*;

pub(crate) fn prepare(
    emitted: EmittedConeObjectSetV2,
    generated: &EmittedGeneratedCBridgeObjectSetV1,
) -> Result<PreparedLayoutObjects, BuiltinObjectProductionError> {
    let production = emitted.production();
    if production.generated_bridge_plan() != generated.sources().plan() {
        return Err(BuiltinObjectProductionError::GeneratedBridgePlanMismatch);
    }
    let bindings = plan_codegen_objects(
        emitted.target_selection().target(),
        emitted.partition().producer_units(),
        emitted.members(),
        generated,
    )?;
    let candidates = bindings
        .scoop_lir_members
        .iter()
        .map(|member| ScoopLirObjectCandidateV1::new(member.plan.member_id(), &member.bytes))
        .collect::<Vec<_>>();
    let bridges = bindings
        .generated_c_bridge_members
        .iter()
        .map(|member| {
            GeneratedCBridgeObjectCandidateV1::new(member.plan.member_id(), &member.bytes)
        })
        .collect::<Vec<_>>();
    let bridge_proof = verify_c_bridge_production_envelopes_v1(
        production.generated_bridge_plan().clone(),
        generated.production().clone(),
        generated.profile(),
        &bindings.member_plan,
        &bridges,
    )
    .map_err(BuiltinObjectProductionError::CBridgeEnvelopes)?;
    let symbols = PlannedStrongObjectSymbolSetV1::new(
        emitted.target(),
        production.canonical_definitions(),
        &bindings.member_plan,
    )
    .map_err(BuiltinObjectProductionError::StrongSymbolPlan)?;
    let relocations = verify_builtin_object_strong_relocations_v1(
        &bindings.member_plan,
        &symbols,
        &candidates,
        bridge_proof,
        &bridges,
    )
    .map_err(BuiltinObjectProductionError::StrongRelocations)?;
    let patch_sites = verify_scoop_lir_digest_patch_sites_v1(
        relocations,
        emitted.foundation(),
        production.digest_finalization_plan().clone(),
        &candidates,
        &bindings.digest_patches,
    )
    .map_err(BuiltinObjectProductionError::DigestPatchSites)?;
    let stackmaps = verify_scoop_lir_stackmaps_v1(
        patch_sites.builtins().clone(),
        production.safepoint_semantics(),
        &candidates,
    )
    .map_err(BuiltinObjectProductionError::Stackmaps)?;
    Ok(PreparedLayoutObjects {
        target_selection: emitted.target_selection(),
        foundation: emitted.foundation().clone(),
        production: emitted.into_production(),
        c_bridge_profile: generated.profile().clone(),
        patch_sites,
        stackmaps,
        bindings,
    })
}
