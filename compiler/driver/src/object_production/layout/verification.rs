use super::*;

pub(crate) fn prepare(
    emitted: EmittedConeObjectSetV2,
    generated: &EmittedGeneratedCBridgeObjectSetV1,
) -> Result<PreparedLayoutObjects, BuiltinObjectProductionError> {
    let EmittedConeObjectSetV2 {
        target_selection,
        foundation,
        production,
        partition,
        members,
        ..
    } = emitted;
    let bindings = plan_codegen_objects(
        target_selection.target(),
        partition.producer_units(),
        members,
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
    let bridge_objects = read_generated_c_objects_v1(
        generated.production().clone(),
        generated.profile(),
        &bridges,
    )
    .map_err(BuiltinObjectProductionError::CBridgeEnvelopes)?;
    let symbols = PlannedStrongObjectSymbolSetV1::new(
        target_selection.target(),
        production.canonical_definitions(),
        &bindings.member_plan,
    )
    .map_err(BuiltinObjectProductionError::StrongSymbolPlan)?;
    let relocations = verify_builtin_object_strong_relocations_v1(
        &bindings.member_plan,
        &symbols,
        &candidates,
        bridge_objects,
        &bridges,
    )
    .map_err(BuiltinObjectProductionError::StrongRelocations)?;
    let builtins = relocations;
    let stackmaps = read_scoop_stackmaps_v1(
        builtins.clone(),
        production.safepoint_semantics(),
        &candidates,
    )
    .map_err(BuiltinObjectProductionError::Stackmaps)?;
    Ok(PreparedLayoutObjects {
        target_selection,
        foundation,
        production,
        builtins,
        stackmaps,
        bindings,
    })
}
