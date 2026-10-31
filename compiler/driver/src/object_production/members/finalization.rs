//! Final member assembly shared by both strong production versions.

use super::*;

pub(in crate::object_production) fn final_members(
    member_plan: &PlannedLinkObjectMemberSetV1,
    generated_c_bridge_members: &[PlannedGeneratedCBridgeObjectInputV1],
    objects: &[scoop_slib::FinalScoopLirObjectV1],
) -> Result<Vec<UncompressedObjectMember>, BuiltinObjectProductionError> {
    let producer = member_plan.producer();
    let mut members = Vec::with_capacity(objects.len() + generated_c_bridge_members.len());
    for object in objects {
        let plan = member_plan
            .scoop_lir_members()
            .iter()
            .find(|plan| plan.member_id() == object.member())
            .ok_or(BuiltinObjectProductionError::MissingFinalMemberPlan(
                object.member(),
            ))?;
        let member = UncompressedObjectMember::new(
            producer,
            plan.stable_key().clone(),
            scoop_slib::SlibMemberRole::LinkObject,
            scoop_slib::remove_production_end_symbols(object.bytes())
                .map_err(BuiltinObjectProductionError::ProductionSymbols)?,
        )
        .map_err(BuiltinObjectProductionError::FinalMember)?;
        require_final_member_id(&member, object.member())?;
        members.push(member);
    }
    for object in generated_c_bridge_members {
        let member = UncompressedObjectMember::new(
            producer,
            object.plan.stable_key().clone(),
            scoop_slib::SlibMemberRole::LinkObject,
            scoop_slib::remove_production_end_symbols(&object.bytes)
                .map_err(BuiltinObjectProductionError::ProductionSymbols)?,
        )
        .map_err(BuiltinObjectProductionError::FinalMember)?;
        require_final_member_id(&member, object.plan.member_id())?;
        members.push(member);
    }
    members.sort_unstable_by_key(|member| member.content().id());
    Ok(members)
}

fn require_final_member_id(
    member: &UncompressedObjectMember,
    expected: SlibMemberId,
) -> Result<(), BuiltinObjectProductionError> {
    let actual = member.content().id();
    if actual == expected {
        Ok(())
    } else {
        Err(BuiltinObjectProductionError::FinalMemberIdMismatch { expected, actual })
    }
}
