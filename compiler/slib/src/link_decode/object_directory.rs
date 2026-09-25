//! Exact LinkObject directory coverage against the replayed member plan.

use super::*;
use scoop_wire::encode_canonical_temporary;

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
    let manifest = graph.envelope.manifest();
    let count = plan
        .scoop_lir_members()
        .len()
        .saturating_add(plan.generated_bridge_members().len());
    let mut expected = Vec::new();
    scoop_wire::allocation::try_reserve_count(&mut expected, count as u64, &path)?;

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

    for member in manifest.members() {
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
    reserve(&mut scoop, plan.scoop_lir_members().len(), &path)?;
    reserve(&mut bridges, plan.generated_bridge_members().len(), &path)?;
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
    let manifest = graph.envelope.manifest();

    let record = manifest
        .members()
        .binary_search_by_key(&member, SlibMemberRecord::id)
        .map(|index| &manifest.members()[index])
        .map_err(|_| StrongLinkMaterializationError::MissingObjectMember(member))?;
    let actual = encode_canonical_temporary(record.stable_key(), &path)?;
    let expected = encode_canonical_temporary(stable_key, &path)?;

    if actual != expected || record.role() != role {
        return Err(StrongLinkMaterializationError::ObjectRecordMismatch(member));
    }

    graph
        .envelope
        .member(member)
        .ok_or(StrongLinkMaterializationError::MissingObjectPayload(member))
}

fn reserve<T>(values: &mut Vec<T>, count: usize, path: &WirePath) -> Result<(), WireError> {
    scoop_wire::allocation::try_reserve_count(values, count as u64, path)
}
