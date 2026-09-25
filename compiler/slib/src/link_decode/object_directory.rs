//! Exact LinkObject directory coverage against the replayed member plan.

use super::*;
use scoop_wire::{BudgetMeter, encode_canonical_temporary_with_meter};

pub(crate) fn validate<'input>(
    graph: &mut ValidatedGraphArtifact<'input>,
    plan: &crate::PlannedLinkObjectMemberSetV1,
) -> Result<
    (
        Vec<ScoopLirObjectCandidateV1<'input>>,
        Vec<GeneratedCBridgeObjectCandidateV1<'input>>,
    ),
    StrongLinkMaterializationError,
> {
    let path = WirePath::root().field(1);
    let (manifest, meter) = graph.envelope.manifest_and_meter();
    let count = plan
        .scoop_lir_members()
        .len()
        .saturating_add(plan.generated_bridge_members().len());
    let mut expected = Vec::new();
    meter.try_reserve_exact(
        &mut expected,
        count as u64,
        std::mem::size_of::<SlibMemberId>() as u64,
        &path,
    )?;
    meter.charge_work(
        (count as u64).saturating_mul(1 + u64::from(count.max(1).ilog2())),
        &path,
    )?;
    expected.extend(
        plan.scoop_lir_members()
            .iter()
            .map(|member| member.member_id()),
    );
    expected.extend(
        plan.generated_bridge_members()
            .iter()
            .map(|member| member.member_id()),
    );
    expected.sort_unstable();
    meter.check_table_entries(manifest.members().len() as u64, &path)?;
    for member in manifest.members() {
        meter.charge_work(1 + u64::from(count.max(1).ilog2()), &path)?;
        if matches!(member.role(), SlibMemberRole::LinkObject { .. })
            && expected.binary_search(&member.id()).is_err()
        {
            return Err(StrongLinkMaterializationError::UnexpectedObjectMember(
                member.id(),
            ));
        }
    }
    let mut scoop = Vec::new();
    let mut bridges = Vec::new();
    reserve(&mut scoop, plan.scoop_lir_members().len(), meter, &path)?;
    reserve(
        &mut bridges,
        plan.generated_bridge_members().len(),
        meter,
        &path,
    )?;
    for member in plan.scoop_lir_members() {
        let payload = required_payload(
            graph,
            member.member_id(),
            member.stable_key(),
            member.role(),
        )?;
        scoop.push(ScoopLirObjectCandidateV1::new(member.member_id(), payload));
    }
    for member in plan.generated_bridge_members() {
        let payload = required_payload(
            graph,
            member.member_id(),
            member.stable_key(),
            member.role(),
        )?;
        bridges.push(GeneratedCBridgeObjectCandidateV1::new(
            member.member_id(),
            payload,
        ));
    }
    Ok((scoop, bridges))
}

fn required_payload<'input>(
    graph: &mut ValidatedGraphArtifact<'input>,
    member: SlibMemberId,
    stable_key: &crate::MemberStableKey,
    role: &SlibMemberRole,
) -> Result<&'input [u8], StrongLinkMaterializationError> {
    let path = WirePath::root().field(1);
    let (manifest, meter) = graph.envelope.manifest_and_meter();
    meter.charge_work(
        1 + u64::from(manifest.members().len().max(1).ilog2()),
        &path,
    )?;
    let record = manifest
        .members()
        .binary_search_by_key(&member, SlibMemberRecord::id)
        .map(|index| &manifest.members()[index])
        .map_err(|_| StrongLinkMaterializationError::MissingObjectMember(member))?;
    let actual = encode_canonical_temporary_with_meter(record.stable_key(), meter, &path)?;
    let expected = encode_canonical_temporary_with_meter(stable_key, meter, &path)?;
    meter.charge_work(1 + actual.len().min(expected.len()) as u64, &path)?;
    if actual != expected || record.role() != role {
        return Err(StrongLinkMaterializationError::ObjectRecordMismatch(member));
    }
    meter.charge_work(
        1 + u64::from(manifest.members().len().max(1).ilog2()),
        &path,
    )?;
    graph
        .envelope
        .member(member)
        .ok_or(StrongLinkMaterializationError::MissingObjectPayload(member))
}

fn reserve<T>(
    values: &mut Vec<T>,
    count: usize,
    meter: &mut BudgetMeter,
    path: &WirePath,
) -> Result<(), WireError> {
    meter.try_reserve_exact(values, count as u64, std::mem::size_of::<T>() as u64, path)
}
