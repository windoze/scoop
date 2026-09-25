use std::fmt;

use scoop_hir::CanonicalHirFoundation;
use scoop_identity::ConeIdentity;
use scoop_lir::CanonicalLirFoundation;
use scoop_mir::CanonicalMirFoundation;
use scoop_wire::{HashError, WireEncode, encode};

use crate::{
    ArtifactFingerprint, BootstrapManifest, BootstrapManifestError, CanonicalSlibArchive,
    CompatibilityRecord, ConeRecord, DependencyRecord, ManifestSection, MemberPurposeSet,
    MemberStableKey, MetadataEnvelope, MetadataEnvelopeError, MetadataLocation, MetadataSection,
    MetadataSectionError, ProducerRecord, SemanticFingerprintError, SemanticFingerprintRecord,
    SlibMember, SlibMemberId, SlibMemberRecordError, SlibMemberRole, SlibWriteError,
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

/// Complete input for the deliberately non-publishable foundation artifact.
///
/// The builder owns only graph-envelope data and typed opaque members. Its IR
/// payloads remain borrowed from the stage-owned canonical projections.
pub struct IdentityFoundationArtifactInput<'foundation> {
    producer: ProducerRecord,
    cone: ConeRecord,
    selection: scoop_lir::ValidatedLirTargetSelection,
    hir: &'foundation CanonicalHirFoundation,
    mir: &'foundation CanonicalMirFoundation,
    lir: &'foundation CanonicalLirFoundation,
    direct_dependencies: Vec<DependencyRecord>,
    auxiliary_members: Vec<SlibMember>,
    manifest_sections: Vec<ManifestSection>,
}

impl<'foundation> IdentityFoundationArtifactInput<'foundation> {
    pub const fn new(
        producer: ProducerRecord,
        cone: ConeRecord,
        selection: scoop_lir::ValidatedLirTargetSelection,
        hir: &'foundation CanonicalHirFoundation,
        mir: &'foundation CanonicalMirFoundation,
        lir: &'foundation CanonicalLirFoundation,
    ) -> Self {
        Self {
            producer,
            cone,
            selection,
            hir,
            mir,
            lir,
            direct_dependencies: Vec::new(),
            auxiliary_members: Vec::new(),
            manifest_sections: Vec::new(),
        }
    }

    pub fn with_direct_dependencies(mut self, dependencies: Vec<DependencyRecord>) -> Self {
        self.direct_dependencies = dependencies;
        self
    }

    pub fn with_auxiliary_members(mut self, members: Vec<SlibMember>) -> Self {
        self.auxiliary_members = members;
        self
    }

    pub fn with_manifest_sections(mut self, sections: Vec<ManifestSection>) -> Self {
        self.manifest_sections = sections;
        self
    }
}

/// Canonical bytes of an identity-foundation artifact.
///
/// This type intentionally exposes neither a publication marker nor a Link
/// proof. The only semantic promotion available to a consumer starts again at
/// [`crate::DecodedSlibEnvelope`].
#[derive(Debug, Eq, PartialEq)]
pub struct IdentityFoundationArtifact {
    archive: CanonicalSlibArchive,
    artifact_fingerprint: ArtifactFingerprint,
}

impl IdentityFoundationArtifact {
    pub fn write(
        input: IdentityFoundationArtifactInput<'_>,
    ) -> Result<Self, IdentityFoundationArtifactError> {
        let IdentityFoundationArtifactInput {
            producer,
            cone,
            selection,
            hir,
            mir,
            lir,
            direct_dependencies,
            mut auxiliary_members,
            manifest_sections,
        } = input;
        validate_manifest_sections(&manifest_sections)?;
        validate_auxiliary_members(cone.identity(), &auxiliary_members)?;

        let compatibility = CompatibilityRecord::new(
            selection,
            crate::ArtifactCapabilityProfile::IDENTITY_FOUNDATION,
        )
        .map_err(IdentityFoundationArtifactError::Compatibility)?;
        let metadata = IdentityFoundationMetadata::new(hir, mir, lir)
            .map_err(IdentityFoundationArtifactError::Metadata)?;
        let semantic_fingerprints = SemanticFingerprintRecord::from_metadata_sections(
            &compatibility,
            &direct_dependencies,
            std::slice::from_ref(metadata.hir_section()),
            std::slice::from_ref(metadata.mir_section()),
            std::slice::from_ref(metadata.lir_section()),
        )
        .map_err(IdentityFoundationArtifactError::SemanticFingerprints)?;
        let mut members = metadata
            .into_members(cone.identity())
            .map_err(IdentityFoundationArtifactError::Metadata)?;
        members
            .try_reserve_exact(auxiliary_members.len())
            .map_err(|_| IdentityFoundationArtifactError::Allocation)?;
        members.append(&mut auxiliary_members);

        let manifest = BootstrapManifest::new(
            producer,
            compatibility,
            cone,
            direct_dependencies,
            &members,
            semantic_fingerprints,
            manifest_sections,
        )
        .map_err(IdentityFoundationArtifactError::Manifest)?;
        let artifact_fingerprint = manifest.artifact_fingerprint();
        let archive = CanonicalSlibArchive::write_bootstrap(&manifest, members)
            .map_err(IdentityFoundationArtifactError::Archive)?;
        Ok(Self {
            archive,
            artifact_fingerprint,
        })
    }

    pub const fn artifact_fingerprint(&self) -> ArtifactFingerprint {
        self.artifact_fingerprint
    }

    pub fn as_bytes(&self) -> &[u8] {
        self.archive.as_bytes()
    }
}

fn validate_manifest_sections(
    sections: &[ManifestSection],
) -> Result<(), IdentityFoundationArtifactError> {
    if let Some((index, section)) = sections
        .iter()
        .enumerate()
        .find(|(_, section)| section.required_for().contains(MemberPurposeSet::COMPILE))
    {
        return Err(
            IdentityFoundationArtifactError::CompileRequiredManifestSection {
                index,
                capability: section.capability().clone(),
            },
        );
    }
    Ok(())
}

fn validate_auxiliary_members(
    cone: ConeIdentity,
    members: &[SlibMember],
) -> Result<(), IdentityFoundationArtifactError> {
    for (index, member) in members.iter().enumerate() {
        if let Some(location) = match member.record().role() {
            SlibMemberRole::HirMetadata => Some(MetadataLocation::Hir),
            SlibMemberRole::MirMetadata => Some(MetadataLocation::Mir),
            SlibMemberRole::LirMetadata => Some(MetadataLocation::Lir),
            SlibMemberRole::LinkObject { .. }
            | SlibMemberRole::DiagnosticAttachment { .. }
            | SlibMemberRole::ExtensionBlob { .. } => None,
        } {
            return Err(IdentityFoundationArtifactError::ReservedMetadataMember {
                index,
                location,
            });
        }
        let expected = member
            .expected_id(cone)
            .map_err(IdentityFoundationArtifactError::MemberIdentity)?;
        let actual = member.record().id();
        if actual != expected {
            return Err(IdentityFoundationArtifactError::MemberConeMismatch {
                index,
                expected,
                actual,
            });
        }
    }
    Ok(())
}

#[derive(Debug)]
pub enum IdentityFoundationArtifactError {
    Compatibility(HashError),
    Metadata(IdentityFoundationMetadataError),
    SemanticFingerprints(SemanticFingerprintError),
    CompileRequiredManifestSection {
        index: usize,
        capability: scoop_identity::CapabilityId,
    },
    ReservedMetadataMember {
        index: usize,
        location: MetadataLocation,
    },
    MemberIdentity(HashError),
    MemberConeMismatch {
        index: usize,
        expected: SlibMemberId,
        actual: SlibMemberId,
    },
    Manifest(BootstrapManifestError),
    Archive(SlibWriteError),
    Allocation,
}

impl fmt::Display for IdentityFoundationArtifactError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Compatibility(source) => {
                write!(
                    formatter,
                    "cannot derive foundation compatibility: {source}"
                )
            }
            Self::Metadata(source) => source.fmt(formatter),
            Self::SemanticFingerprints(source) => {
                write!(
                    formatter,
                    "cannot fingerprint foundation metadata: {source}"
                )
            }
            Self::CompileRequiredManifestSection { index, capability } => write!(
                formatter,
                "foundation manifest section {index} ({}/{}/{}) requires Compile",
                capability.namespace(),
                capability.name(),
                capability.major_version(),
            ),
            Self::ReservedMetadataMember { index, location } => write!(
                formatter,
                "auxiliary member {index} attempts to replace the canonical {location} metadata member"
            ),
            Self::MemberIdentity(source) => {
                write!(
                    formatter,
                    "cannot derive auxiliary member identity: {source}"
                )
            }
            Self::MemberConeMismatch {
                index,
                expected,
                actual,
            } => write!(
                formatter,
                "auxiliary member {index} belongs to another Cone: expected {expected}, found {actual}"
            ),
            Self::Manifest(source) => write!(formatter, "invalid foundation manifest: {source}"),
            Self::Archive(source) => write!(formatter, "cannot write foundation archive: {source}"),
            Self::Allocation => {
                formatter.write_str("failed to allocate foundation artifact member table")
            }
        }
    }
}

impl std::error::Error for IdentityFoundationArtifactError {}

#[cfg(test)]
mod tests {
    use scoop_hir::DecodedHirFoundation;
    use scoop_identity::{CapabilityId, ConeCoordinate};
    use scoop_lir::DecodedLirFoundation;
    use scoop_mir::DecodedMirFoundation;
    use scoop_wire::decode_canonical;

    use super::*;
    use crate::{
        ConeKind, ConeSourceForm, DecodedMetadataEnvelope, DecodedSlibEnvelope,
        ExtensionRequirement, LogicalMemberKey, ManifestSection, MetadataLocation,
    };

    #[test]
    fn canonical_foundations_are_sealed_in_their_typed_metadata_members() {
        let foundations = IdentityFoundationMetadata::new(
            &CanonicalHirFoundation::empty(),
            &CanonicalMirFoundation::empty(),
            &CanonicalLirFoundation::empty(),
        )
        .unwrap();

        assert_layer::<DecodedHirFoundation>(
            foundations.hir_envelope(),
            foundations.hir_section(),
            MetadataLocation::Hir,
        );
        assert_layer::<DecodedMirFoundation>(
            foundations.mir_envelope(),
            foundations.mir_section(),
            MetadataLocation::Mir,
        );
        assert_layer::<DecodedLirFoundation>(
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

    #[test]
    fn foundation_writer_produces_a_reproducible_graph_artifact() {
        let hir = CanonicalHirFoundation::empty();
        let mir = CanonicalMirFoundation::empty();
        let lir = CanonicalLirFoundation::empty();
        let selection = scoop_lir::ValidatedLirTargetSelection::DARWIN_AARCH64_LLVM_22_1;
        let cone = ConeRecord::new(
            ConeCoordinate::reserved_core(),
            ConeKind::Library,
            ConeSourceForm::Manifest,
        )
        .unwrap();
        let diagnostic_capability = CapabilityId::new("org.scoop-lang.test", "dump", 1).unwrap();
        let extension_capability = CapabilityId::new("org.scoop-lang.test", "opaque", 1).unwrap();
        let diagnostic = SlibMember::new(
            cone.identity(),
            MemberStableKey::DiagnosticAttachment {
                capability: diagnostic_capability.clone(),
                logical_key: LogicalMemberKey::new(b"dump".to_vec()).unwrap(),
            },
            SlibMemberRole::DiagnosticAttachment {
                capability: diagnostic_capability,
            },
            b"diagnostic".to_vec(),
        )
        .unwrap();
        let extension = SlibMember::new(
            cone.identity(),
            MemberStableKey::ExtensionBlob {
                capability: extension_capability.clone(),
                logical_key: LogicalMemberKey::new(b"opaque".to_vec()).unwrap(),
            },
            SlibMemberRole::ExtensionBlob {
                capability: extension_capability,
                requirement: ExtensionRequirement::Optional,
            },
            b"opaque".to_vec(),
        )
        .unwrap();
        let write = |members| {
            IdentityFoundationArtifact::write(
                IdentityFoundationArtifactInput::new(
                    ProducerRecord::new("test").unwrap(),
                    cone.clone(),
                    selection,
                    &hir,
                    &mir,
                    &lir,
                )
                .with_auxiliary_members(members),
            )
            .unwrap()
        };

        let forward = write(vec![diagnostic.clone(), extension.clone()]);
        let reverse = write(vec![extension, diagnostic]);
        assert_eq!(forward, reverse);
        let graph = DecodedSlibEnvelope::open(forward.as_bytes(), selection)
            .unwrap()
            .validate_graph()
            .unwrap();
        assert_eq!(graph.identity(), cone.identity());
        assert_eq!(graph.artifact_fingerprint(), forward.artifact_fingerprint());
    }

    #[test]
    fn foundation_writer_rejects_inputs_that_cannot_form_its_profile() {
        let hir = CanonicalHirFoundation::empty();
        let mir = CanonicalMirFoundation::empty();
        let lir = CanonicalLirFoundation::empty();
        let selection = scoop_lir::ValidatedLirTargetSelection::DARWIN_AARCH64_LLVM_22_1;
        let cone = ConeRecord::new(
            ConeCoordinate::reserved_core(),
            ConeKind::Library,
            ConeSourceForm::Manifest,
        )
        .unwrap();
        let other_cone = ConeCoordinate::reserved_single_file().identity().unwrap();
        let capability = CapabilityId::new("org.scoop-lang.test", "opaque", 1).unwrap();
        let foreign_member = SlibMember::new(
            other_cone,
            MemberStableKey::ExtensionBlob {
                capability: capability.clone(),
                logical_key: LogicalMemberKey::new(b"foreign".to_vec()).unwrap(),
            },
            SlibMemberRole::ExtensionBlob {
                capability: capability.clone(),
                requirement: ExtensionRequirement::Optional,
            },
            Vec::new(),
        )
        .unwrap();
        let input = || {
            IdentityFoundationArtifactInput::new(
                ProducerRecord::new("test").unwrap(),
                cone.clone(),
                selection,
                &hir,
                &mir,
                &lir,
            )
        };

        assert!(matches!(
            IdentityFoundationArtifact::write(input().with_auxiliary_members(vec![foreign_member])),
            Err(IdentityFoundationArtifactError::MemberConeMismatch { index: 0, .. })
        ));

        let metadata_member = SlibMember::new(
            cone.identity(),
            MemberStableKey::HirMetadata,
            SlibMemberRole::HirMetadata,
            Vec::new(),
        )
        .unwrap();
        assert!(matches!(
            IdentityFoundationArtifact::write(
                input().with_auxiliary_members(vec![metadata_member])
            ),
            Err(IdentityFoundationArtifactError::ReservedMetadataMember {
                index: 0,
                location: MetadataLocation::Hir,
            })
        ));

        let compile_section =
            ManifestSection::new(capability, MemberPurposeSet::COMPILE, Vec::new()).unwrap();
        assert!(matches!(
            IdentityFoundationArtifact::write(
                input().with_manifest_sections(vec![compile_section])
            ),
            Err(IdentityFoundationArtifactError::CompileRequiredManifestSection { index: 0, .. })
        ));
    }

    fn assert_layer<T: scoop_wire::WireDecode>(
        envelope: &[u8],
        expected_section: &MetadataSection,
        location: MetadataLocation,
    ) {
        let decoded = DecodedMetadataEnvelope::decode(envelope, location).unwrap();
        let [section] = decoded.sections() else {
            panic!("foundation envelope must contain exactly one section")
        };
        assert_eq!(section.capability(), expected_section.capability());
        assert_eq!(section.required_for(), MemberPurposeSet::COMPILE);
        assert_eq!(section.payload(), expected_section.payload());
        decode_canonical::<T>(section.payload()).unwrap();
    }
}
