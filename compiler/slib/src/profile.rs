use std::fmt;

use scoop_identity::{ArtifactCapabilityProfileId, CapabilityId};
use scoop_wire::{Encoder, HashError, WireEncode, domain_separated_cbor_hash};

const ARTIFACT_PROFILE_DOMAIN: &str = "scoop-artifact-capability-profile-v1";

pub fn hir_identity_foundation_capability() -> CapabilityId {
    known_capability("org.scoop-lang.hir", "identity-foundation")
}

pub fn mir_identity_foundation_capability() -> CapabilityId {
    known_capability("org.scoop-lang.mir", "identity-foundation")
}

pub fn lir_identity_foundation_capability() -> CapabilityId {
    known_capability("org.scoop-lang.lir", "identity-foundation")
}

fn known_capability(namespace: &str, name: &str) -> CapabilityId {
    CapabilityId::new(namespace, name, 1).expect("built-in capability id is valid")
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct ArtifactCapabilityProfile;

impl ArtifactCapabilityProfile {
    pub const IDENTITY_FOUNDATION: Self = Self;

    pub fn id(self) -> ArtifactCapabilityProfileId {
        ArtifactCapabilityProfileId::identity_foundation()
    }

    pub fn descriptor(self) -> ArtifactCapabilityProfileDescriptor {
        ArtifactCapabilityProfileDescriptor {
            id: self.id(),
            required_manifest: Vec::new(),
            required_hir: vec![hir_identity_foundation_capability()],
            required_mir: vec![mir_identity_foundation_capability()],
            required_lir: vec![lir_identity_foundation_capability()],
            code_requirement: FingerprintAvailabilityRequirement::MustBeUnavailable,
            runtime_requirement: FingerprintAvailabilityRequirement::MustBeUnavailable,
            publication_class: PublicationClass::FoundationOnly,
            validation_policy: ArtifactValidationPolicy {
                odr: OdrValidationPolicy::IdentityOnlyNonPublishable,
                extra_sections: ExtraSectionPolicy::AllowPurposeDisjointOpaqueAndEnvelopeOptional,
                decode_cost_model: SlibDecodeCostModel::DeterministicLogicalCost,
                link_proof: LinkProofPolicy::Forbidden,
            },
        }
    }

    pub fn fingerprint(self) -> Result<ArtifactCapabilityProfileFingerprint, HashError> {
        ArtifactCapabilityProfileFingerprint::from_descriptor(&self.descriptor())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArtifactCapabilityProfileDescriptor {
    id: ArtifactCapabilityProfileId,
    required_manifest: Vec<CapabilityId>,
    required_hir: Vec<CapabilityId>,
    required_mir: Vec<CapabilityId>,
    required_lir: Vec<CapabilityId>,
    code_requirement: FingerprintAvailabilityRequirement,
    runtime_requirement: FingerprintAvailabilityRequirement,
    publication_class: PublicationClass,
    validation_policy: ArtifactValidationPolicy,
}

impl ArtifactCapabilityProfileDescriptor {
    pub const fn id(&self) -> &ArtifactCapabilityProfileId {
        &self.id
    }

    pub fn required_manifest(&self) -> &[CapabilityId] {
        &self.required_manifest
    }

    pub fn required_hir(&self) -> &[CapabilityId] {
        &self.required_hir
    }

    pub fn required_mir(&self) -> &[CapabilityId] {
        &self.required_mir
    }

    pub fn required_lir(&self) -> &[CapabilityId] {
        &self.required_lir
    }

    pub const fn code_requirement(&self) -> FingerprintAvailabilityRequirement {
        self.code_requirement
    }

    pub const fn runtime_requirement(&self) -> FingerprintAvailabilityRequirement {
        self.runtime_requirement
    }

    pub const fn publication_class(&self) -> PublicationClass {
        self.publication_class
    }

    pub const fn validation_policy(&self) -> ArtifactValidationPolicy {
        self.validation_policy
    }
}

impl WireEncode for ArtifactCapabilityProfileDescriptor {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(9)?;
        encoder.field(1)?;
        self.id.encode(encoder)?;
        encode_capabilities(encoder, 2, &self.required_manifest)?;
        encode_capabilities(encoder, 3, &self.required_hir)?;
        encode_capabilities(encoder, 4, &self.required_mir)?;
        encode_capabilities(encoder, 5, &self.required_lir)?;
        encoder.field(6)?;
        self.code_requirement.encode(encoder)?;
        encoder.field(7)?;
        self.runtime_requirement.encode(encoder)?;
        encoder.field(8)?;
        self.publication_class.encode(encoder)?;
        encoder.field(9)?;
        self.validation_policy.encode(encoder)
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ArtifactCapabilityProfileFingerprint([u8; 32]);

impl ArtifactCapabilityProfileFingerprint {
    pub fn from_descriptor(
        descriptor: &ArtifactCapabilityProfileDescriptor,
    ) -> Result<Self, HashError> {
        domain_separated_cbor_hash(ARTIFACT_PROFILE_DOMAIN, descriptor)
            .map(|digest| Self(*digest.as_array()))
    }

    pub const fn as_array(&self) -> &[u8; 32] {
        &self.0
    }
}

impl WireEncode for ArtifactCapabilityProfileFingerprint {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.bytes(&self.0)
    }
}

impl fmt::Display for ArtifactCapabilityProfileFingerprint {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in self.0 {
            write!(formatter, "{byte:02x}")?;
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum FingerprintAvailabilityRequirement {
    MustBeUnavailable,
    MustBeAvailable,
}

impl WireEncode for FingerprintAvailabilityRequirement {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.unsigned(match self {
            Self::MustBeUnavailable => 1,
            Self::MustBeAvailable => 2,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum PublicationClass {
    FoundationOnly,
    Publishable,
}

impl WireEncode for PublicationClass {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.unsigned(match self {
            Self::FoundationOnly => 1,
            Self::Publishable => 2,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum OdrValidationPolicy {
    IdentityOnlyNonPublishable,
    RejectAll,
    RequireCompleteDefinitionProof,
}

impl WireEncode for OdrValidationPolicy {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.unsigned(match self {
            Self::IdentityOnlyNonPublishable => 1,
            Self::RejectAll => 2,
            Self::RequireCompleteDefinitionProof => 3,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ExtraSectionPolicy {
    AllowPurposeDisjointOpaqueAndEnvelopeOptional,
}

impl WireEncode for ExtraSectionPolicy {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.unsigned(1)
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum SlibDecodeCostModel {
    DeterministicLogicalCost,
}

impl WireEncode for SlibDecodeCostModel {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.unsigned(1)
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum LinkProofPolicy {
    Forbidden,
    Required,
}

impl WireEncode for LinkProofPolicy {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.unsigned(match self {
            Self::Forbidden => 1,
            Self::Required => 2,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct ArtifactValidationPolicy {
    odr: OdrValidationPolicy,
    extra_sections: ExtraSectionPolicy,
    decode_cost_model: SlibDecodeCostModel,
    link_proof: LinkProofPolicy,
}

impl ArtifactValidationPolicy {
    pub const fn odr(self) -> OdrValidationPolicy {
        self.odr
    }

    pub const fn extra_sections(self) -> ExtraSectionPolicy {
        self.extra_sections
    }

    pub const fn decode_cost_model(self) -> SlibDecodeCostModel {
        self.decode_cost_model
    }

    pub const fn link_proof(self) -> LinkProofPolicy {
        self.link_proof
    }
}

impl WireEncode for ArtifactValidationPolicy {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(4)?;
        encoder.field(1)?;
        self.odr.encode(encoder)?;
        encoder.field(2)?;
        self.extra_sections.encode(encoder)?;
        encoder.field(3)?;
        self.decode_cost_model.encode(encoder)?;
        encoder.field(4)?;
        self.link_proof.encode(encoder)
    }
}

fn encode_capabilities(
    encoder: &mut Encoder,
    field: u32,
    capabilities: &[CapabilityId],
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.field(field)?;
    encoder.array(capabilities.len() as u64)?;
    for capability in capabilities {
        capability.encode(encoder)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use scoop_wire::encode;

    use super::*;

    #[test]
    fn identity_foundation_profile_has_the_fixed_descriptor_and_fingerprint() {
        let profile = ArtifactCapabilityProfile::IDENTITY_FOUNDATION;
        let descriptor = profile.descriptor();
        assert_eq!(
            hex(&encode(&descriptor).unwrap()),
            "a901a301781b6f72672e73636f6f702d6c616e672e736c69622d70726f66696c6502736964656e746974792d666f756e646174696f6e030102800381a301726f72672e73636f6f702d6c616e672e68697202736964656e746974792d666f756e646174696f6e03010481a301726f72672e73636f6f702d6c616e672e6d697202736964656e746974792d666f756e646174696f6e03010581a301726f72672e73636f6f702d6c616e672e6c697202736964656e746974792d666f756e646174696f6e030106010701080109a40101020103010401"
        );
        assert_eq!(
            profile.fingerprint().unwrap().to_string(),
            "6461601c81a77ecbd34698c708d9035732f0a75f6e5ec98d2c561250ab0111f9"
        );
        assert_eq!(
            descriptor.publication_class(),
            PublicationClass::FoundationOnly
        );
        assert_eq!(
            descriptor.validation_policy().link_proof(),
            LinkProofPolicy::Forbidden
        );
    }

    fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|byte| format!("{byte:02x}")).collect()
    }
}
