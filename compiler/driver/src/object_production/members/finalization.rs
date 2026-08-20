//! Final member assembly shared by both strong production versions.

use super::*;

pub(in crate::object_production) fn final_members(
    member_plan: &PlannedLinkObjectMemberSetV1,
    generated_c_bridge_members: &[PlannedGeneratedCBridgeObjectInputV1],
    objects: &[scoop_slib::VerifiedEntryPatchedScoopLirObjectV1],
) -> Result<Vec<SlibMember>, BuiltinObjectProductionError> {
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
        let member = SlibMember::new(
            producer,
            plan.stable_key().clone(),
            plan.role().clone(),
            object.bytes().to_vec(),
        )
        .map_err(BuiltinObjectProductionError::FinalMember)?;
        require_final_member_id(&member, object.member())?;
        members.push(member);
    }
    for object in generated_c_bridge_members {
        let member = SlibMember::new(
            producer,
            object.plan.stable_key().clone(),
            object.plan.role().clone(),
            object.bytes.clone(),
        )
        .map_err(BuiltinObjectProductionError::FinalMember)?;
        require_final_member_id(&member, object.plan.member_id())?;
        members.push(member);
    }
    members.sort_unstable_by_key(|member| member.record().id());
    Ok(members)
}

fn require_final_member_id(
    member: &SlibMember,
    expected: SlibMemberId,
) -> Result<(), BuiltinObjectProductionError> {
    let actual = member.record().id();
    if actual == expected {
        Ok(())
    } else {
        Err(BuiltinObjectProductionError::FinalMemberIdMismatch { expected, actual })
    }
}
