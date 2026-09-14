use std::fmt;

use scoop_identity::{ArtifactCapabilityProfileId, CapabilityId};
use scoop_wire::{
    Encoder, HashError, WireEncode, domain_separated_cbor_hash,
    domain_separated_cbor_hash_stream_length,
};

use crate::MemberPurposeSet;

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

pub fn manifest_single_cone_production_capability() -> CapabilityId {
    known_capability("org.scoop-lang.manifest", "single-cone-production")
}

pub fn hir_core_bootstrap_interface_capability() -> CapabilityId {
    known_capability("org.scoop-lang.hir", "core-bootstrap-interface")
}

pub fn mir_core_bootstrap_bridge_capability() -> CapabilityId {
    known_capability("org.scoop-lang.mir", "core-bootstrap-bridge")
}

pub fn lir_strong_production_capability() -> CapabilityId {
    known_capability("org.scoop-lang.lir", "strong-production")
}

pub fn lir_link_identity_closure_capability() -> CapabilityId {
    known_capability("org.scoop-lang.lir", "link-identity-closure")
}

fn known_capability(namespace: &str, name: &str) -> CapabilityId {
    CapabilityId::new(namespace, name, 1).expect("built-in capability id is valid")
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct ArtifactCapabilityProfile(ArtifactCapabilityProfileKind);

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
enum ArtifactCapabilityProfileKind {
    IdentityFoundation,
    SingleConeStrong,
}

impl ArtifactCapabilityProfile {
    pub const IDENTITY_FOUNDATION: Self = Self(ArtifactCapabilityProfileKind::IdentityFoundation);
    pub const SINGLE_CONE_STRONG: Self = Self(ArtifactCapabilityProfileKind::SingleConeStrong);

    pub fn id(self) -> ArtifactCapabilityProfileId {
        match self.0 {
            ArtifactCapabilityProfileKind::IdentityFoundation => {
                ArtifactCapabilityProfileId::identity_foundation()
            }
            ArtifactCapabilityProfileKind::SingleConeStrong => {
                ArtifactCapabilityProfileId::single_cone_strong()
            }
        }
    }

    pub fn from_id(id: &ArtifactCapabilityProfileId) -> Option<Self> {
        if id == &ArtifactCapabilityProfileId::identity_foundation() {
            Some(Self::IDENTITY_FOUNDATION)
        } else if id == &ArtifactCapabilityProfileId::single_cone_strong() {
            Some(Self::SINGLE_CONE_STRONG)
        } else {
            None
        }
    }

    pub fn descriptor(self) -> ArtifactCapabilityProfileDescriptor {
        match self.0 {
            ArtifactCapabilityProfileKind::IdentityFoundation => {
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
                        extra_sections:
                            ExtraSectionPolicy::AllowPurposeDisjointOpaqueAndEnvelopeOptional,
                        decode_cost_model: SlibDecodeCostModel::DeterministicLogicalCost,
                        link_proof: LinkProofPolicy::Forbidden,
                    },
                }
            }
            ArtifactCapabilityProfileKind::SingleConeStrong => {
                ArtifactCapabilityProfileDescriptor {
                    id: self.id(),
                    required_manifest: vec![manifest_single_cone_production_capability()],
                    required_hir: vec![
                        hir_core_bootstrap_interface_capability(),
                        hir_identity_foundation_capability(),
                    ],
                    required_mir: vec![
                        mir_core_bootstrap_bridge_capability(),
                        mir_identity_foundation_capability(),
                    ],
                    required_lir: vec![
                        lir_identity_foundation_capability(),
                        lir_link_identity_closure_capability(),
                        lir_strong_production_capability(),
                    ],
                    code_requirement: FingerprintAvailabilityRequirement::MustBeAvailable,
                    runtime_requirement: FingerprintAvailabilityRequirement::MustBeAvailable,
                    publication_class: PublicationClass::Publishable,
                    validation_policy: ArtifactValidationPolicy {
                        odr: OdrValidationPolicy::RejectAll,
                        extra_sections:
                            ExtraSectionPolicy::AllowPurposeDisjointOpaqueAndEnvelopeOptional,
                        decode_cost_model: SlibDecodeCostModel::DeterministicLogicalCost,
                        link_proof: LinkProofPolicy::Required,
                    },
                }
            }
        }
    }

    pub fn fingerprint(self) -> Result<ArtifactCapabilityProfileFingerprint, HashError> {
        ArtifactCapabilityProfileFingerprint::from_descriptor(&self.descriptor())
    }

    pub(crate) fn validate_link_manifest_inventory(
        self,
        sections: &[crate::ManifestSection],
    ) -> Result<(), ArtifactProfileLinkInventoryError> {
        let descriptor = self.descriptor();
        for (index, section) in sections.iter().enumerate() {
            if section.required_for().contains(MemberPurposeSet::LINK)
                && !descriptor
                    .required_manifest()
                    .contains(section.capability())
            {
                return Err(
                    ArtifactProfileLinkInventoryError::UnsupportedRequiredCapability {
                        location: SectionLocation::Manifest,
                        index,
                        capability: section.capability().clone(),
                    },
                );
            }
        }
        for capability in descriptor.required_manifest().iter().filter(|capability| {
            CapabilityContractRegistry::contract(capability)
                .is_some_and(|contract| contract.required_for().contains(MemberPurposeSet::LINK))
        }) {
            if !sections
                .iter()
                .any(|section| section.capability() == capability)
            {
                return Err(
                    ArtifactProfileLinkInventoryError::MissingRequiredCapability {
                        location: SectionLocation::Manifest,
                        capability: capability.clone(),
                    },
                );
            }
        }
        Ok(())
    }

    pub(crate) fn validate_link_metadata_inventory(
        self,
        location: crate::MetadataLocation,
        sections: &[crate::DecodedMetadataSection<'_>],
    ) -> Result<(), ArtifactProfileLinkInventoryError> {
        let descriptor = self.descriptor();
        let expected = match location {
            crate::MetadataLocation::Hir => descriptor.required_hir(),
            crate::MetadataLocation::Mir => descriptor.required_mir(),
            crate::MetadataLocation::Lir => descriptor.required_lir(),
        };
        let section_location = match location {
            crate::MetadataLocation::Hir => SectionLocation::Hir,
            crate::MetadataLocation::Mir => SectionLocation::Mir,
            crate::MetadataLocation::Lir => SectionLocation::Lir,
        };
        for (index, section) in sections.iter().enumerate() {
            if section.required_for().contains(MemberPurposeSet::LINK)
                && !expected.contains(section.capability())
            {
                return Err(
                    ArtifactProfileLinkInventoryError::UnsupportedRequiredCapability {
                        location: section_location,
                        index,
                        capability: section.capability().clone(),
                    },
                );
            }
        }
        for capability in expected.iter().filter(|capability| {
            CapabilityContractRegistry::contract(capability)
                .is_some_and(|contract| contract.required_for().contains(MemberPurposeSet::LINK))
        }) {
            if !sections
                .iter()
                .any(|section| section.capability() == capability)
            {
                return Err(
                    ArtifactProfileLinkInventoryError::MissingRequiredCapability {
                        location: section_location,
                        capability: capability.clone(),
                    },
                );
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ArtifactProfileLinkInventoryError {
    MissingRequiredCapability {
        location: SectionLocation,
        capability: CapabilityId,
    },
    UnsupportedRequiredCapability {
        location: SectionLocation,
        index: usize,
        capability: CapabilityId,
    },
}

impl fmt::Display for ArtifactProfileLinkInventoryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingRequiredCapability {
                location,
                capability,
            } => write!(
                formatter,
                "artifact profile requires {location} capability {}/{}/{} for Link",
                capability.namespace(),
                capability.name(),
                capability.major_version(),
            ),
            Self::UnsupportedRequiredCapability {
                location,
                index,
                capability,
            } => write!(
                formatter,
                "{location} section {index} requires unsupported Link capability {}/{}/{}",
                capability.namespace(),
                capability.name(),
                capability.major_version(),
            ),
        }
    }
}

impl std::error::Error for ArtifactProfileLinkInventoryError {}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum SectionLocation {
    Manifest,
    Hir,
    Mir,
    Lir,
}

impl fmt::Display for SectionLocation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Manifest => "Manifest",
            Self::Hir => "HIR",
            Self::Mir => "MIR",
            Self::Lir => "LIR",
        })
    }
}

impl WireEncode for SectionLocation {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.unsigned(match self {
            Self::Manifest => 1,
            Self::Hir => 2,
            Self::Mir => 3,
            Self::Lir => 4,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum FingerprintSink {
    Hir,
    Mir,
    Lir,
    Code,
    RuntimeImage,
    EnvelopeOnly,
    LinkValidationOnly,
}

impl FingerprintSink {
    const fn bit(self) -> u8 {
        match self {
            Self::Hir => 0x01,
            Self::Mir => 0x02,
            Self::Lir => 0x04,
            Self::Code => 0x08,
            Self::RuntimeImage => 0x10,
            Self::EnvelopeOnly => 0x20,
            Self::LinkValidationOnly => 0x40,
        }
    }
}

impl WireEncode for FingerprintSink {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.unsigned(match self {
            Self::Hir => 1,
            Self::Mir => 2,
            Self::Lir => 3,
            Self::Code => 4,
            Self::RuntimeImage => 5,
            Self::EnvelopeOnly => 6,
            Self::LinkValidationOnly => 7,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct FingerprintSinkSet(u8);

impl FingerprintSinkSet {
    pub const HIR: Self = Self(FingerprintSink::Hir.bit());
    pub const MIR: Self = Self(FingerprintSink::Mir.bit());
    pub const LIR: Self = Self(FingerprintSink::Lir.bit());
    pub const CODE: Self = Self(FingerprintSink::Code.bit());
    pub const RUNTIME_IMAGE: Self = Self(FingerprintSink::RuntimeImage.bit());
    pub const ENVELOPE_ONLY: Self = Self(FingerprintSink::EnvelopeOnly.bit());
    pub const LINK_VALIDATION_ONLY: Self = Self(FingerprintSink::LinkValidationOnly.bit());

    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    pub const fn contains(self, sink: FingerprintSink) -> bool {
        self.0 & sink.bit() != 0
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CapabilityContract {
    capability: CapabilityId,
    location: SectionLocation,
    required_for: MemberPurposeSet,
    sinks: FingerprintSinkSet,
}

impl CapabilityContract {
    pub const fn capability(&self) -> &CapabilityId {
        &self.capability
    }

    pub const fn location(&self) -> SectionLocation {
        self.location
    }

    pub const fn required_for(&self) -> MemberPurposeSet {
        self.required_for
    }

    pub const fn sinks(&self) -> FingerprintSinkSet {
        self.sinks
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct CapabilityContractRegistry;

impl CapabilityContractRegistry {
    pub fn contract(capability: &CapabilityId) -> Option<CapabilityContract> {
        let (location, required_for, sinks) = match (
            capability.namespace(),
            capability.name(),
            capability.major_version(),
        ) {
            ("org.scoop-lang.hir", "identity-foundation", 1) => (
                SectionLocation::Hir,
                MemberPurposeSet::COMPILE,
                FingerprintSinkSet::HIR,
            ),
            ("org.scoop-lang.mir", "identity-foundation", 1) => (
                SectionLocation::Mir,
                MemberPurposeSet::COMPILE,
                FingerprintSinkSet::MIR,
            ),
            ("org.scoop-lang.lir", "identity-foundation", 1) => (
                SectionLocation::Lir,
                MemberPurposeSet::COMPILE,
                FingerprintSinkSet::LIR,
            ),
            ("org.scoop-lang.manifest", "single-cone-production", 1) => (
                SectionLocation::Manifest,
                MemberPurposeSet::LINK,
                FingerprintSinkSet::CODE
                    .union(FingerprintSinkSet::RUNTIME_IMAGE)
                    .union(FingerprintSinkSet::LINK_VALIDATION_ONLY),
            ),
            ("org.scoop-lang.hir", "core-bootstrap-interface", 1) => (
                SectionLocation::Hir,
                MemberPurposeSet::COMPILE,
                FingerprintSinkSet::HIR,
            ),
            ("org.scoop-lang.mir", "core-bootstrap-bridge", 1) => (
                SectionLocation::Mir,
                MemberPurposeSet::COMPILE,
                FingerprintSinkSet::MIR,
            ),
            ("org.scoop-lang.lir", "strong-production", 1) => (
                SectionLocation::Lir,
                MemberPurposeSet::COMPILE_AND_LINK,
                FingerprintSinkSet::LIR
                    .union(FingerprintSinkSet::CODE)
                    .union(FingerprintSinkSet::RUNTIME_IMAGE),
            ),
            ("org.scoop-lang.lir", "link-identity-closure", 1) => (
                SectionLocation::Lir,
                MemberPurposeSet::LINK,
                FingerprintSinkSet::LINK_VALIDATION_ONLY,
            ),
            _ => return None,
        };
        Some(CapabilityContract {
            capability: capability.clone(),
            location,
            required_for,
            sinks,
        })
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

    #[test]
    fn single_cone_strong_profile_has_the_fixed_descriptor_and_fingerprint() {
        let profile = ArtifactCapabilityProfile::SINGLE_CONE_STRONG;
        let descriptor = profile.descriptor();
        assert_eq!(
            hex(&encode(&descriptor).unwrap()),
            "a901a301781b6f72672e73636f6f702d6c616e672e736c69622d70726f66696c65027273696e676c652d636f6e652d7374726f6e6703010281a301776f72672e73636f6f702d6c616e672e6d616e6966657374027673696e676c652d636f6e652d70726f64756374696f6e03010382a301726f72672e73636f6f702d6c616e672e686972027818636f72652d626f6f7473747261702d696e746572666163650301a301726f72672e73636f6f702d6c616e672e68697202736964656e746974792d666f756e646174696f6e03010482a301726f72672e73636f6f702d6c616e672e6d69720275636f72652d626f6f7473747261702d6272696467650301a301726f72672e73636f6f702d6c616e672e6d697202736964656e746974792d666f756e646174696f6e03010583a301726f72672e73636f6f702d6c616e672e6c697202736964656e746974792d666f756e646174696f6e0301a301726f72672e73636f6f702d6c616e672e6c697202756c696e6b2d6964656e746974792d636c6f737572650301a301726f72672e73636f6f702d6c616e672e6c697202717374726f6e672d70726f64756374696f6e030106020702080209a40102020103010402"
        );
        assert_eq!(
            profile.fingerprint().unwrap().to_string(),
            "456662e4eaee376624c54caa9068a91f8b369ea054453f884d61695a96da143f"
        );
        assert_eq!(
            ArtifactCapabilityProfile::from_id(descriptor.id()),
            Some(profile)
        );

        assert_eq!(
            descriptor.required_manifest(),
            &[manifest_single_cone_production_capability()]
        );
        assert_eq!(
            descriptor.required_hir(),
            &[
                hir_core_bootstrap_interface_capability(),
                hir_identity_foundation_capability(),
            ]
        );
        assert_eq!(
            descriptor.required_mir(),
            &[
                mir_core_bootstrap_bridge_capability(),
                mir_identity_foundation_capability(),
            ]
        );
        assert_eq!(
            descriptor.required_lir(),
            &[
                lir_identity_foundation_capability(),
                lir_link_identity_closure_capability(),
                lir_strong_production_capability(),
            ]
        );
        assert_eq!(
            descriptor.code_requirement(),
            FingerprintAvailabilityRequirement::MustBeAvailable
        );
        assert_eq!(
            descriptor.runtime_requirement(),
            FingerprintAvailabilityRequirement::MustBeAvailable
        );
        assert_eq!(
            descriptor.publication_class(),
            PublicationClass::Publishable
        );
        assert_eq!(
            descriptor.validation_policy().odr(),
            OdrValidationPolicy::RejectAll
        );
        assert_eq!(
            descriptor.validation_policy().link_proof(),
            LinkProofPolicy::Required
        );
    }

    #[test]
    fn capability_registry_has_the_fixed_location_purpose_and_sink_matrix() {
        let code_runtime_link = FingerprintSinkSet::CODE
            .union(FingerprintSinkSet::RUNTIME_IMAGE)
            .union(FingerprintSinkSet::LINK_VALIDATION_ONLY);
        let lir_code_runtime = FingerprintSinkSet::LIR
            .union(FingerprintSinkSet::CODE)
            .union(FingerprintSinkSet::RUNTIME_IMAGE);
        for (capability, location, purpose, sinks) in [
            (
                hir_identity_foundation_capability(),
                SectionLocation::Hir,
                MemberPurposeSet::COMPILE,
                FingerprintSinkSet::HIR,
            ),
            (
                mir_identity_foundation_capability(),
                SectionLocation::Mir,
                MemberPurposeSet::COMPILE,
                FingerprintSinkSet::MIR,
            ),
            (
                lir_identity_foundation_capability(),
                SectionLocation::Lir,
                MemberPurposeSet::COMPILE,
                FingerprintSinkSet::LIR,
            ),
            (
                manifest_single_cone_production_capability(),
                SectionLocation::Manifest,
                MemberPurposeSet::LINK,
                code_runtime_link,
            ),
            (
                hir_core_bootstrap_interface_capability(),
                SectionLocation::Hir,
                MemberPurposeSet::COMPILE,
                FingerprintSinkSet::HIR,
            ),
            (
                mir_core_bootstrap_bridge_capability(),
                SectionLocation::Mir,
                MemberPurposeSet::COMPILE,
                FingerprintSinkSet::MIR,
            ),
            (
                lir_strong_production_capability(),
                SectionLocation::Lir,
                MemberPurposeSet::COMPILE_AND_LINK,
                lir_code_runtime,
            ),
            (
                lir_link_identity_closure_capability(),
                SectionLocation::Lir,
                MemberPurposeSet::LINK,
                FingerprintSinkSet::LINK_VALIDATION_ONLY,
            ),
        ] {
            let contract = CapabilityContractRegistry::contract(&capability).unwrap();
            assert_eq!(contract.capability(), &capability);
            assert_eq!(contract.location(), location);
            assert_eq!(contract.required_for(), purpose);
            assert_eq!(contract.sinks(), sinks);
        }

        assert!(
            CapabilityContractRegistry::contract(
                &CapabilityId::new("org.scoop-lang.test", "unknown", 1).unwrap()
            )
            .is_none()
        );
    }

    #[test]
    fn manifest_inventory_is_closed_by_artifact_profile() {
        let production = crate::ManifestSection::new(
            manifest_single_cone_production_capability(),
            MemberPurposeSet::LINK,
            Vec::new(),
        )
        .unwrap();
        assert_eq!(
            ArtifactCapabilityProfile::SINGLE_CONE_STRONG.validate_link_manifest_inventory(&[]),
            Err(
                ArtifactProfileLinkInventoryError::MissingRequiredCapability {
                    location: SectionLocation::Manifest,
                    capability: manifest_single_cone_production_capability()
                }
            )
        );
        assert!(
            ArtifactCapabilityProfile::SINGLE_CONE_STRONG
                .validate_link_manifest_inventory(std::slice::from_ref(&production))
                .is_ok()
        );
        assert!(matches!(
            ArtifactCapabilityProfile::IDENTITY_FOUNDATION
                .validate_link_manifest_inventory(std::slice::from_ref(&production)),
            Err(
                ArtifactProfileLinkInventoryError::UnsupportedRequiredCapability {
                    location: SectionLocation::Manifest,
                    index: 0,
                    ..
                }
            )
        ));

        let optional = crate::ManifestSection::new(
            CapabilityId::new("org.scoop-lang.test", "optional", 1).unwrap(),
            MemberPurposeSet::NONE,
            Vec::new(),
        )
        .unwrap();
        assert!(
            ArtifactCapabilityProfile::IDENTITY_FOUNDATION
                .validate_link_manifest_inventory(&[optional])
                .is_ok()
        );
    }

    fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|byte| format!("{byte:02x}")).collect()
    }
}
