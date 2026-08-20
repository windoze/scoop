//! Ordinary native object members; they have no Scoop Strong or ODR records.

use scoop_identity::{CapabilityId, ConeIdentity, NormalizedSourcePath};

use crate::{
    LogicalMemberKey, MemberStableKey, SlibMember, SlibMemberRecord, SlibMemberRecordError,
    SlibMemberRole,
};

pub fn native_link_object_capability() -> CapabilityId {
    CapabilityId::new("org.scoop-lang.link-object", "native", 1)
        .expect("built-in native object capability is valid")
}

pub fn is_native_link_object(record: &SlibMemberRecord) -> bool {
    matches!(record.role(), SlibMemberRole::LinkObject { verifier_capability, .. }
        if verifier_capability == &native_link_object_capability())
}

pub fn native_link_object_member(
    cone: ConeIdentity,
    target: scoop_lir::LirTargetProfile,
    source: &NormalizedSourcePath,
    bytes: Vec<u8>,
) -> Result<SlibMember, SlibMemberRecordError> {
    SlibMember::new(
        cone,
        MemberStableKey::LinkObject {
            verifier_capability: native_link_object_capability(),
            logical_key: LogicalMemberKey::new(source.as_str().as_bytes().to_vec())
                .expect("normalized source paths are nonempty"),
        },
        super::link_object_role(target, native_link_object_capability()),
        bytes,
    )
}

pub struct NativeLinkObject {
    pub(crate) member: crate::SlibMemberId,
    pub(crate) source: NormalizedSourcePath,
    pub(crate) bytes: std::sync::Arc<[u8]>,
}

impl NativeLinkObject {
    pub fn member(&self) -> crate::SlibMemberId {
        self.member
    }
    pub fn source(&self) -> &NormalizedSourcePath {
        &self.source
    }
    pub fn bytes(&self) -> &std::sync::Arc<[u8]> {
        &self.bytes
    }
}
