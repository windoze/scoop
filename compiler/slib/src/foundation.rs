use std::fmt;

use scoop_hir::CanonicalHirFoundation;
use scoop_identity::ConeIdentity;
use scoop_lir::CanonicalLirFoundation;
use scoop_mir::CanonicalMirFoundation;
use scoop_wire::{WireEncode, encode};

use crate::{
    MemberPurposeSet, MemberStableKey, MetadataEnvelope, MetadataEnvelopeError, MetadataLocation,
    MetadataSection, MetadataSectionError, SlibMember, SlibMemberRecordError, SlibMemberRole,
    hir_identity_foundation_capability, lir_identity_foundation_capability,
    mir_identity_foundation_capability,
};

/// The three canonical identity-foundation metadata products of one Cone.
///
/// Construction encodes every IR-owned foundation as the payload of its one
/// required Compile section, then seals that section in the layer-specific
/// outer envelope. Callers cannot pair a HIR payload with a MIR envelope or
/// publish a bare inner foundation as a metadata member.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IdentityFoundationMetadata {
    hir: LayerMetadata,
    mir: LayerMetadata,
    lir: LayerMetadata,
}

impl IdentityFoundationMetadata {
    pub fn new(
        hir: &CanonicalHirFoundation,
        mir: &CanonicalMirFoundation,
        lir: &CanonicalLirFoundation,
    ) -> Result<Self, IdentityFoundationMetadataError> {
        Ok(Self {
            hir: LayerMetadata::new(
                MetadataLocation::Hir,
                hir_identity_foundation_capability(),
                hir,
            )?,
            mir: LayerMetadata::new(
                MetadataLocation::Mir,
                mir_identity_foundation_capability(),
                mir,
            )?,
            lir: LayerMetadata::new(
                MetadataLocation::Lir,
                lir_identity_foundation_capability(),
                lir,
            )?,
        })
    }

    pub const fn hir_section(&self) -> &MetadataSection {
        &self.hir.section
    }

    pub const fn mir_section(&self) -> &MetadataSection {
        &self.mir.section
    }

    pub const fn lir_section(&self) -> &MetadataSection {
        &self.lir.section
    }

    pub fn hir_envelope(&self) -> &[u8] {
        &self.hir.envelope
    }

    pub fn mir_envelope(&self) -> &[u8] {
        &self.mir.envelope
    }

    pub fn lir_envelope(&self) -> &[u8] {
        &self.lir.envelope
    }

    /// Bind the three envelopes to the canonical metadata member identities
    /// of `cone`. Exactly three members are returned, one for each IR layer.
    pub fn into_members(
        self,
        cone: ConeIdentity,
    ) -> Result<Vec<SlibMember>, IdentityFoundationMetadataError> {
        let mut members = Vec::new();
        members
            .try_reserve_exact(3)
            .map_err(|_| IdentityFoundationMetadataError::Allocation)?;
        members.push(self.hir.into_member(
            cone,
            MemberStableKey::HirMetadata,
            SlibMemberRole::HirMetadata,
        )?);
        members.push(self.mir.into_member(
            cone,
            MemberStableKey::MirMetadata,
            SlibMemberRole::MirMetadata,
        )?);
        members.push(self.lir.into_member(
            cone,
            MemberStableKey::LirMetadata,
            SlibMemberRole::LirMetadata,
        )?);
        Ok(members)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct LayerMetadata {
    section: MetadataSection,
    envelope: Vec<u8>,
}

impl LayerMetadata {
    fn new<T: WireEncode>(
        location: MetadataLocation,
        capability: scoop_identity::CapabilityId,
        foundation: &T,
    ) -> Result<Self, IdentityFoundationMetadataError> {
        let payload = encode(foundation)
            .map_err(|source| IdentityFoundationMetadataError::Encoding { location, source })?;
        let section =
            MetadataSection::new(location, capability, MemberPurposeSet::COMPILE, payload)
                .map_err(|source| IdentityFoundationMetadataError::Section { location, source })?;
        let envelope = MetadataEnvelope::new(location, vec![section.clone()])
            .map_err(|source| IdentityFoundationMetadataError::Envelope { location, source })?;
        let envelope = encode(&envelope)
            .map_err(|source| IdentityFoundationMetadataError::Encoding { location, source })?;
        Ok(Self { section, envelope })
    }

    fn into_member(
        self,
        cone: ConeIdentity,
        stable_key: MemberStableKey,
        role: SlibMemberRole,
    ) -> Result<SlibMember, IdentityFoundationMetadataError> {
        let location = self.section.location();
        SlibMember::new(cone, stable_key, role, self.envelope)
            .map_err(|source| IdentityFoundationMetadataError::Member { location, source })
    }
}

#[derive(Debug)]
pub enum IdentityFoundationMetadataError {
    Encoding {
        location: MetadataLocation,
        source: scoop_wire::cbor::EncodeError,
    },
    Section {
        location: MetadataLocation,
        source: MetadataSectionError,
    },
    Envelope {
        location: MetadataLocation,
        source: MetadataEnvelopeError,
    },
    Member {
        location: MetadataLocation,
        source: SlibMemberRecordError,
    },
    Allocation,
}

impl fmt::Display for IdentityFoundationMetadataError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Encoding { location, source } => {
                write!(
                    formatter,
                    "cannot encode {location} identity foundation: {source}"
                )
            }
            Self::Section { location, source } => {
                write!(
                    formatter,
                    "invalid {location} identity-foundation section: {source}"
                )
            }
            Self::Envelope { location, source } => {
                write!(formatter, "invalid {location} metadata envelope: {source}")
            }
            Self::Member { location, source } => {
                write!(
                    formatter,
                    "cannot build {location} metadata member: {source}"
                )
            }
            Self::Allocation => {
                formatter.write_str("failed to allocate identity-foundation metadata members")
            }
        }
    }
}

impl std::error::Error for IdentityFoundationMetadataError {}

#[cfg(test)]
mod tests {
    use scoop_hir::ValidatedHirFoundationWire;
    use scoop_identity::ConeCoordinate;
    use scoop_lir::ValidatedLirFoundationWire;
    use scoop_mir::ValidatedMirFoundationWire;
    use scoop_wire::{DecodeLimits, decode_canonical};

    use super::*;
    use crate::{DecodedMetadataEnvelope, MetadataLocation};

    #[test]
    fn canonical_foundations_are_sealed_in_their_typed_metadata_members() {
        let foundations = IdentityFoundationMetadata::new(
            &CanonicalHirFoundation::empty(),
            &CanonicalMirFoundation::empty(),
            &CanonicalLirFoundation::empty(),
        )
        .unwrap();

        assert_layer::<ValidatedHirFoundationWire>(
            foundations.hir_envelope(),
            foundations.hir_section(),
            MetadataLocation::Hir,
        );
        assert_layer::<ValidatedMirFoundationWire>(
            foundations.mir_envelope(),
            foundations.mir_section(),
            MetadataLocation::Mir,
        );
        assert_layer::<ValidatedLirFoundationWire>(
            foundations.lir_envelope(),
            foundations.lir_section(),
            MetadataLocation::Lir,
        );

        let members = foundations
            .into_members(ConeCoordinate::reserved_core().identity().unwrap())
            .unwrap();
        assert_eq!(members.len(), 3);
        assert!(matches!(
            members[0].record().role(),
            SlibMemberRole::HirMetadata
        ));
        assert!(matches!(
            members[1].record().role(),
            SlibMemberRole::MirMetadata
        ));
        assert!(matches!(
            members[2].record().role(),
            SlibMemberRole::LirMetadata
        ));
    }

    fn assert_layer<T: scoop_wire::WireDecode>(
        envelope: &[u8],
        expected_section: &MetadataSection,
        location: MetadataLocation,
    ) {
        let decoded =
            DecodedMetadataEnvelope::decode(envelope, location, DecodeLimits::default()).unwrap();
        let [section] = decoded.sections() else {
            panic!("foundation envelope must contain exactly one section")
        };
        assert_eq!(section.capability(), expected_section.capability());
        assert_eq!(section.required_for(), MemberPurposeSet::COMPILE);
        assert_eq!(section.payload(), expected_section.payload());
        decode_canonical::<T>(section.payload(), DecodeLimits::default()).unwrap();
    }
}
