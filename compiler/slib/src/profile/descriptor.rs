use super::*;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArtifactCapabilityProfileDescriptor {
    pub(super) id: ArtifactCapabilityProfileId,
    pub(super) required_manifest: Vec<CapabilityId>,
    pub(super) required_hir: Vec<CapabilityId>,
    pub(super) required_mir: Vec<CapabilityId>,
    pub(super) required_lir: Vec<CapabilityId>,
    pub(super) code_requirement: FingerprintAvailabilityRequirement,
    pub(super) runtime_requirement: FingerprintAvailabilityRequirement,
    pub(super) publication_class: PublicationClass,
    pub(super) validation_policy: ArtifactValidationPolicy,
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

    pub(crate) fn hash_stream_length(
        descriptor: &ArtifactCapabilityProfileDescriptor,
    ) -> Result<u64, HashError> {
        domain_separated_cbor_hash_stream_length(ARTIFACT_PROFILE_DOMAIN, descriptor)
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
    pub(super) odr: OdrValidationPolicy,
    pub(super) extra_sections: ExtraSectionPolicy,
    pub(super) decode_cost_model: SlibDecodeCostModel,
    pub(super) link_proof: LinkProofPolicy,
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
