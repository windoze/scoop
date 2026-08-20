//! Match the actual object directory to the completed object contents.
use super::*;

#[derive(Clone)]
pub(super) struct ExpectedFinalLinkObjectMemberV1 {
    pub(super) member: SlibMemberId,
    pub(super) stable_key: MemberStableKey,
    pub(super) role: SlibMemberRole,
    pub(super) byte_length: u64,
    pub(super) content_digest: Digest256,
}

pub(super) fn expected_final_members<D, C, I>(
    final_objects: &VerifiedEntryPatchSetV1<D, C, I>,
) -> Result<Vec<ExpectedFinalLinkObjectMemberV1>, CodeLinkObjectMemberValidationError>
where
    D: scoop_lir::StrongDescriptorReference,
    C: Clone,
{
    let builtins = final_objects.entry().patch_sites().builtins();
    let member_plan = builtins.member_plan();
    let scoop_objects = final_objects
        .objects()
        .iter()
        .map(|object| (object.member(), object))
        .collect::<BTreeMap<_, _>>();
    let generated_objects = builtins
        .c_bridge_production()
        .members()
        .iter()
        .map(|object| (object.plan().member_id(), object))
        .collect::<BTreeMap<_, _>>();
    let mut expected = Vec::with_capacity(
        member_plan.scoop_lir_members().len() + member_plan.generated_bridge_members().len(),
    );
    for member in member_plan.scoop_lir_members() {
        let object = scoop_objects.get(&member.member_id()).ok_or(
            CodeLinkObjectMemberValidationError::FinalProofMissingMember(member.member_id()),
        )?;
        let envelope = object.envelope().sections().envelope();
        expected.push(ExpectedFinalLinkObjectMemberV1 {
            member: member.member_id(),
            stable_key: member.stable_key().clone(),
            role: member.role().clone(),
            byte_length: envelope.byte_length(),
            content_digest: envelope.content_digest(),
        });
    }
    for member in member_plan.generated_bridge_members() {
        let object = generated_objects.get(&member.member_id()).ok_or(
            CodeLinkObjectMemberValidationError::FinalProofMissingMember(member.member_id()),
        )?;
        let envelope = object.envelope().sections().envelope();
        expected.push(ExpectedFinalLinkObjectMemberV1 {
            member: member.member_id(),
            stable_key: member.stable_key().clone(),
            role: member.role().clone(),
            byte_length: envelope.byte_length(),
            content_digest: envelope.content_digest(),
        });
    }
    expected.sort_unstable_by_key(|member| member.member);
    if let Some(pair) = expected
        .windows(2)
        .find(|pair| pair[0].member == pair[1].member)
    {
        return Err(CodeLinkObjectMemberValidationError::FinalProofDuplicateMember(pair[0].member));
    }
    Ok(expected)
}

pub(super) fn verify_directory_records(
    expected: &[ExpectedFinalLinkObjectMemberV1],
    directory: &[SlibMemberRecord],
) -> Result<CodeLinkObjectMemberSetV1, CodeLinkObjectMemberValidationError> {
    let mut seen = BTreeSet::new();
    let mut actual = BTreeMap::new();
    for record in directory {
        if !seen.insert(record.id()) {
            return Err(CodeLinkObjectMemberValidationError::DuplicateDirectoryMember(record.id()));
        }
        match record.role() {
            SlibMemberRole::LinkObject { .. } => {
                actual.insert(record.id(), record);
            }
            SlibMemberRole::ExtensionBlob {
                capability,
                requirement: crate::ExtensionRequirement::Link,
            } => {
                return Err(
                    CodeLinkObjectMemberValidationError::UnsupportedLinkExtension {
                        member: record.id(),
                        capability: capability.clone(),
                    },
                );
            }
            SlibMemberRole::HirMetadata
            | SlibMemberRole::MirMetadata
            | SlibMemberRole::LirMetadata
            | SlibMemberRole::DiagnosticAttachment { .. }
            | SlibMemberRole::ExtensionBlob {
                requirement: crate::ExtensionRequirement::Optional,
                ..
            } => {}
        }
    }

    let expected_ids = expected
        .iter()
        .map(|member| member.member)
        .collect::<BTreeSet<_>>();
    let actual_ids = actual.keys().copied().collect::<BTreeSet<_>>();
    if let Some(member) = expected_ids.difference(&actual_ids).next() {
        return Err(CodeLinkObjectMemberValidationError::MissingDirectoryMember(
            *member,
        ));
    }
    let mut verified = Vec::with_capacity(expected.len());
    for member in actual_ids.difference(&expected_ids) {
        let record = actual[member];
        if !crate::is_native_link_object(record) {
            return Err(CodeLinkObjectMemberValidationError::UnexpectedDirectoryMember(*member));
        }
        verified.push(VerifiedCodeLinkObjectMemberV1 {
            member: *member,
            fingerprint: record
                .as_link_member()
                .expect("LinkObject record")
                .fingerprint()
                .map_err(CodeLinkObjectMemberValidationError::Hash)?,
        });
    }
    for expected in expected {
        let record = actual[&expected.member];
        if record.stable_key() != &expected.stable_key || record.role() != &expected.role {
            return Err(CodeLinkObjectMemberValidationError::MemberIdentityMismatch(
                expected.member,
            ));
        }
        if record.byte_length() != expected.byte_length
            || record.sha256() != expected.content_digest
        {
            return Err(CodeLinkObjectMemberValidationError::MemberContentMismatch(
                expected.member,
            ));
        }
        let fingerprint = record
            .as_link_member()
            .expect("a LinkObject directory record always has a link member view")
            .fingerprint()
            .map_err(CodeLinkObjectMemberValidationError::Hash)?;
        verified.push(VerifiedCodeLinkObjectMemberV1 {
            member: expected.member,
            fingerprint,
        });
    }
    verified.sort_unstable_by_key(|member| member.member);
    Ok(CodeLinkObjectMemberSetV1 { members: verified })
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CodeLinkObjectMemberValidationError {
    FinalProofMissingMember(SlibMemberId),
    FinalProofDuplicateMember(SlibMemberId),
    DuplicateDirectoryMember(SlibMemberId),
    MissingDirectoryMember(SlibMemberId),
    UnexpectedDirectoryMember(SlibMemberId),
    UnsupportedLinkExtension {
        member: SlibMemberId,
        capability: CapabilityId,
    },
    MemberIdentityMismatch(SlibMemberId),
    MemberContentMismatch(SlibMemberId),
    Hash(HashError),
}

impl fmt::Display for CodeLinkObjectMemberValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Self::UnsupportedLinkExtension { member, capability } = self {
            return write!(
                formatter,
                "member {member}: unsupported Link-required extension {}/{}/{}",
                capability.namespace(),
                capability.name(),
                capability.major_version()
            );
        }
        write!(formatter, "invalid code link-object member set: {self:?}")
    }
}

impl std::error::Error for CodeLinkObjectMemberValidationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Hash(source) => Some(source),
            _ => None,
        }
    }
}
