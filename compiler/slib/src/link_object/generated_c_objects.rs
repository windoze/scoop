//! Actual generated-C object sections and the record used by Code fingerprints.

use super::{
    GeneratedCBridgeObjectEnvelopeValidationError, ValidatedBuiltinObjectSectionInventoryV1,
    read_generated_c_object_sections_v1,
};
use crate::SlibMemberId;
use scoop_lir::{CBridgeProductionSetV1, CBridgeToolchainProfileV1};

#[derive(Clone, Copy, Debug)]
pub struct GeneratedCBridgeObjectCandidateV1<'bytes> {
    member: SlibMemberId,
    bytes: &'bytes [u8],
}
impl<'bytes> GeneratedCBridgeObjectCandidateV1<'bytes> {
    pub const fn new(member: SlibMemberId, bytes: &'bytes [u8]) -> Self {
        Self { member, bytes }
    }
    pub const fn member(self) -> SlibMemberId {
        self.member
    }
    pub const fn bytes(self) -> &'bytes [u8] {
        self.bytes
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GeneratedCObjectMemberV1 {
    member: SlibMemberId,
    sections: ValidatedBuiltinObjectSectionInventoryV1,
}
impl GeneratedCObjectMemberV1 {
    pub const fn member(&self) -> SlibMemberId {
        self.member
    }
    pub const fn sections(&self) -> &ValidatedBuiltinObjectSectionInventoryV1 {
        &self.sections
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GeneratedCObjectSetV1 {
    production: CBridgeProductionSetV1,
    members: Vec<GeneratedCObjectMemberV1>,
}
impl GeneratedCObjectSetV1 {
    pub const fn production(&self) -> &CBridgeProductionSetV1 {
        &self.production
    }
    pub fn members(&self) -> &[GeneratedCObjectMemberV1] {
        &self.members
    }
}

pub fn read_generated_c_objects_v1(
    production: CBridgeProductionSetV1,
    profile: &CBridgeToolchainProfileV1,
    objects: &[GeneratedCBridgeObjectCandidateV1<'_>],
) -> Result<GeneratedCObjectSetV1, GeneratedCObjectReadError> {
    let members = objects
        .iter()
        .map(|object| {
            let sections =
                read_generated_c_object_sections_v1(object.bytes, profile.contract().target().id())
                    .map_err(|source| GeneratedCObjectReadError {
                        member: object.member,
                        source,
                    })?;
            Ok(GeneratedCObjectMemberV1 {
                member: object.member,
                sections,
            })
        })
        .collect::<Result<_, _>>()?;
    Ok(GeneratedCObjectSetV1 {
        production,
        members,
    })
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GeneratedCObjectReadError {
    pub member: SlibMemberId,
    pub source: GeneratedCBridgeObjectEnvelopeValidationError,
}
impl std::fmt::Display for GeneratedCObjectReadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "cannot read generated-C member {}: {}",
            self.member, self.source
        )
    }
}
impl std::error::Error for GeneratedCObjectReadError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.source)
    }
}

#[cfg(test)]
pub(super) mod tests;
