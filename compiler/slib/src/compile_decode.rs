use std::fmt;

use scoop_hir::ValidatedHirFoundationWire;
use scoop_identity::{CapabilityId, ConeCoordinate, ConeIdentity};
use scoop_lir::ValidatedLirFoundationWire;
use scoop_mir::ValidatedMirFoundationWire;
use scoop_wire::{DecodeUsage, WireError, decode_canonical_with_meter};

use crate::{
    ArtifactFingerprint, DecodedMetadataEnvelope, DependencyRecord, MemberPurposeSet,
    MetadataLocation, MetadataReadError, SemanticFingerprintError, SemanticFingerprintRecord,
    SlibMemberRecord, SlibMemberRole, ValidatedGraphArtifact, hir_identity_foundation_capability,
    lir_identity_foundation_capability, mir_identity_foundation_capability,
};

/// Canonically decoded foundation payloads whose artifact, profile inventory,
/// outer envelopes, inner wire products, and semantic fingerprints agree.
///
/// Persistent identities and cross-layer references have not yet been
/// committed to a semantic world, so this is deliberately not a Compile
/// proof and exposes no imported identity lookup.
#[derive(Debug)]
pub struct DecodedIdentityFoundations<'input> {
    graph: ValidatedGraphArtifact<'input>,
    hir: ValidatedHirFoundationWire,
    mir: ValidatedMirFoundationWire,
    lir: ValidatedLirFoundationWire,
}

impl<'input> ValidatedGraphArtifact<'input> {
    pub fn decode_identity_foundations(
        mut self,
    ) -> Result<DecodedIdentityFoundations<'input>, IdentityFoundationDecodeError> {
        reject_compile_required_manifest_sections(&self)?;

        let hir_member = metadata_member(&self, MetadataLocation::Hir)?;
        let mir_member = metadata_member(&self, MetadataLocation::Mir)?;
        let lir_member = metadata_member(&self, MetadataLocation::Lir)?;
        let hir_payload = self.envelope.member(hir_member.id()).ok_or(
            IdentityFoundationDecodeError::MissingMemberPayload {
                location: MetadataLocation::Hir,
                member: hir_member.id(),
            },
        )?;
        let mir_payload = self.envelope.member(mir_member.id()).ok_or(
            IdentityFoundationDecodeError::MissingMemberPayload {
                location: MetadataLocation::Mir,
                member: mir_member.id(),
            },
        )?;
        let lir_payload = self.envelope.member(lir_member.id()).ok_or(
            IdentityFoundationDecodeError::MissingMemberPayload {
                location: MetadataLocation::Lir,
                member: lir_member.id(),
            },
        )?;

        let hir_envelope = DecodedMetadataEnvelope::decode_with_meter(
            hir_payload,
            MetadataLocation::Hir,
            self.envelope.meter_mut(),
        )
        .map_err(|source| IdentityFoundationDecodeError::OuterEnvelope {
            location: MetadataLocation::Hir,
            source,
        })?;
        let mir_envelope = DecodedMetadataEnvelope::decode_with_meter(
            mir_payload,
            MetadataLocation::Mir,
            self.envelope.meter_mut(),
        )
        .map_err(|source| IdentityFoundationDecodeError::OuterEnvelope {
            location: MetadataLocation::Mir,
            source,
        })?;
        let lir_envelope = DecodedMetadataEnvelope::decode_with_meter(
            lir_payload,
            MetadataLocation::Lir,
            self.envelope.meter_mut(),
        )
        .map_err(|source| IdentityFoundationDecodeError::OuterEnvelope {
            location: MetadataLocation::Lir,
            source,
        })?;

        let hir_section =
            required_foundation_section(&hir_envelope, hir_identity_foundation_capability())?;
        let mir_section =
            required_foundation_section(&mir_envelope, mir_identity_foundation_capability())?;
        let lir_section =
            required_foundation_section(&lir_envelope, lir_identity_foundation_capability())?;

        let hir = decode_canonical_with_meter::<ValidatedHirFoundationWire>(
            hir_section.payload(),
            self.envelope.meter_mut(),
        )
        .map_err(|source| IdentityFoundationDecodeError::InnerFoundation {
            location: MetadataLocation::Hir,
            source,
        })?;
        let mir = decode_canonical_with_meter::<ValidatedMirFoundationWire>(
            mir_section.payload(),
            self.envelope.meter_mut(),
        )
        .map_err(|source| IdentityFoundationDecodeError::InnerFoundation {
            location: MetadataLocation::Mir,
            source,
        })?;
        let lir = decode_canonical_with_meter::<ValidatedLirFoundationWire>(
            lir_section.payload(),
            self.envelope.meter_mut(),
        )
        .map_err(|source| IdentityFoundationDecodeError::InnerFoundation {
            location: MetadataLocation::Lir,
            source,
        })?;

        let actual = SemanticFingerprintRecord::decoded_identity_foundation(
            self.envelope.manifest().compatibility(),
            self.direct_dependencies(),
            hir_section,
            mir_section,
            lir_section,
        )
        .map_err(IdentityFoundationDecodeError::SemanticFingerprints)?;
        let expected = self.envelope.manifest().semantic_fingerprints();
        require_semantic_fingerprint(
            MetadataLocation::Hir,
            expected.hir().as_array(),
            actual.hir().as_array(),
        )?;
        require_semantic_fingerprint(
            MetadataLocation::Mir,
            expected.mir().as_array(),
            actual.mir().as_array(),
        )?;
        require_semantic_fingerprint(
            MetadataLocation::Lir,
            expected.lir().as_array(),
            actual.lir().as_array(),
        )?;

        Ok(DecodedIdentityFoundations {
            graph: self,
            hir,
            mir,
            lir,
        })
    }
}

fn require_semantic_fingerprint(
    location: MetadataLocation,
    expected: &[u8; 32],
    actual: &[u8; 32],
) -> Result<(), IdentityFoundationDecodeError> {
    if actual == expected {
        Ok(())
    } else {
        Err(IdentityFoundationDecodeError::SemanticFingerprintMismatch {
            location,
            expected: *expected,
            actual: *actual,
        })
    }
}

impl DecodedIdentityFoundations<'_> {
    pub const fn coordinate(&self) -> &ConeCoordinate {
        self.graph.coordinate()
    }

    pub const fn identity(&self) -> ConeIdentity {
        self.graph.identity()
    }

    pub fn direct_dependencies(&self) -> &[DependencyRecord] {
        self.graph.direct_dependencies()
    }

    pub const fn artifact_fingerprint(&self) -> ArtifactFingerprint {
        self.graph.artifact_fingerprint()
    }

    pub const fn decode_usage(&self) -> DecodeUsage {
        self.graph.decode_usage()
    }

    pub const fn hir_wire(&self) -> &ValidatedHirFoundationWire {
        &self.hir
    }

    pub const fn mir_wire(&self) -> &ValidatedMirFoundationWire {
        &self.mir
    }

    pub const fn lir_wire(&self) -> &ValidatedLirFoundationWire {
        &self.lir
    }
}

fn reject_compile_required_manifest_sections(
    graph: &ValidatedGraphArtifact<'_>,
) -> Result<(), IdentityFoundationDecodeError> {
    if let Some((index, section)) = graph
        .envelope
        .manifest()
        .sections()
        .iter()
        .enumerate()
        .find(|(_, section)| section.required_for().contains(MemberPurposeSet::COMPILE))
    {
        return Err(IdentityFoundationDecodeError::UnknownCompileCapability {
            location: None,
            index,
            capability: section.capability().clone(),
        });
    }
    Ok(())
}

fn metadata_member<'graph>(
    graph: &'graph ValidatedGraphArtifact<'_>,
    location: MetadataLocation,
) -> Result<&'graph SlibMemberRecord, IdentityFoundationDecodeError> {
    graph
        .envelope
        .manifest()
        .members()
        .iter()
        .find(|member| {
            matches!(
                (location, member.role()),
                (MetadataLocation::Hir, SlibMemberRole::HirMetadata)
                    | (MetadataLocation::Mir, SlibMemberRole::MirMetadata)
                    | (MetadataLocation::Lir, SlibMemberRole::LirMetadata)
            )
        })
        .ok_or(IdentityFoundationDecodeError::MissingMetadataMember { location })
}

fn required_foundation_section<'envelope, 'input>(
    envelope: &'envelope DecodedMetadataEnvelope<'input>,
    required: CapabilityId,
) -> Result<&'envelope crate::DecodedMetadataSection<'input>, IdentityFoundationDecodeError> {
    let mut foundation = None;
    for (index, section) in envelope.sections().iter().enumerate() {
        if section.capability() == &required {
            foundation = Some(section);
        } else if section.required_for().contains(MemberPurposeSet::COMPILE) {
            return Err(IdentityFoundationDecodeError::UnknownCompileCapability {
                location: Some(envelope.location()),
                index,
                capability: section.capability().clone(),
            });
        }
    }
    foundation.ok_or(IdentityFoundationDecodeError::MissingFoundationSection {
        location: envelope.location(),
        capability: required,
    })
}

#[derive(Debug)]
pub enum IdentityFoundationDecodeError {
    MissingMetadataMember {
        location: MetadataLocation,
    },
    MissingMemberPayload {
        location: MetadataLocation,
        member: crate::SlibMemberId,
    },
    OuterEnvelope {
        location: MetadataLocation,
        source: MetadataReadError,
    },
    MissingFoundationSection {
        location: MetadataLocation,
        capability: CapabilityId,
    },
    UnknownCompileCapability {
        location: Option<MetadataLocation>,
        index: usize,
        capability: CapabilityId,
    },
    InnerFoundation {
        location: MetadataLocation,
        source: WireError,
    },
    SemanticFingerprints(SemanticFingerprintError),
    SemanticFingerprintMismatch {
        location: MetadataLocation,
        expected: [u8; 32],
        actual: [u8; 32],
    },
}

impl fmt::Display for IdentityFoundationDecodeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingMetadataMember { location } => {
                write!(formatter, "artifact has no {location} metadata member")
            }
            Self::MissingMemberPayload { location, member } => {
                write!(
                    formatter,
                    "{location} metadata member {member} has no payload range"
                )
            }
            Self::OuterEnvelope { location, source } => {
                write!(formatter, "invalid {location} metadata envelope: {source}")
            }
            Self::MissingFoundationSection {
                location,
                capability,
            } => write!(
                formatter,
                "{location} metadata is missing required capability {}/{}/{}",
                capability.namespace(),
                capability.name(),
                capability.major_version(),
            ),
            Self::UnknownCompileCapability {
                location,
                index,
                capability,
            } => {
                if let Some(location) = location {
                    write!(formatter, "{location} metadata section {index}")?;
                } else {
                    write!(formatter, "manifest section {index}")?;
                }
                write!(
                    formatter,
                    " requires unknown Compile capability {}/{}/{}",
                    capability.namespace(),
                    capability.name(),
                    capability.major_version(),
                )
            }
            Self::InnerFoundation { location, source } => {
                write!(
                    formatter,
                    "invalid {location} identity foundation: {source}"
                )
            }
            Self::SemanticFingerprints(source) => source.fmt(formatter),
            Self::SemanticFingerprintMismatch {
                location,
                expected,
                actual,
            } => {
                write!(
                    formatter,
                    "{location} semantic fingerprint mismatch: expected "
                )?;
                write_hex(expected, formatter)?;
                formatter.write_str(", found ")?;
                write_hex(actual, formatter)
            }
        }
    }
}

impl std::error::Error for IdentityFoundationDecodeError {}

fn write_hex(bytes: &[u8; 32], formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    for byte in bytes {
        write!(formatter, "{byte:02x}")?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use scoop_hir::CanonicalHirFoundation;
    use scoop_identity::{CapabilityId, ConeCoordinate};
    use scoop_lir::{CanonicalLirFoundation, ValidatedLirTargetSelection};
    use scoop_mir::CanonicalMirFoundation;
    use scoop_wire::{DecodeLimits, encode};

    use super::*;
    use crate::{
        ConeKind, ConeRecord, ConeSourceForm, IdentityFoundationArtifact,
        IdentityFoundationArtifactInput, ManifestSection, ProducerRecord,
    };

    #[test]
    fn graph_decodes_the_three_foundation_envelopes_with_one_cumulative_budget() {
        let hir = CanonicalHirFoundation::empty();
        let mir = CanonicalMirFoundation::empty();
        let lir = CanonicalLirFoundation::empty();
        let selection = ValidatedLirTargetSelection::DARWIN_AARCH64_LLVM_22_1;
        let cone = ConeRecord::new(
            ConeCoordinate::reserved_core(),
            ConeKind::Library,
            ConeSourceForm::Manifest,
        )
        .unwrap();
        let artifact = IdentityFoundationArtifact::write(IdentityFoundationArtifactInput::new(
            ProducerRecord::new("test").unwrap(),
            cone.clone(),
            selection,
            &hir,
            &mir,
            &lir,
        ))
        .unwrap();
        let envelope = crate::DecodedSlibEnvelope::open(
            artifact.as_bytes(),
            DecodeLimits::default(),
            selection,
        )
        .unwrap();
        let envelope_usage = envelope.decode_usage();
        let decoded = envelope
            .validate_graph()
            .unwrap()
            .decode_identity_foundations()
            .unwrap();

        assert_eq!(decoded.identity(), cone.identity());
        assert_eq!(encode(decoded.hir_wire()).unwrap(), encode(&hir).unwrap());
        assert_eq!(encode(decoded.mir_wire()).unwrap(), encode(&mir).unwrap());
        assert_eq!(encode(decoded.lir_wire()).unwrap(), encode(&lir).unwrap());
        assert!(decoded.decode_usage().decoded_nodes > envelope_usage.decoded_nodes);
        assert!(decoded.decode_usage().logical_heap_bytes > envelope_usage.logical_heap_bytes);
    }

    #[test]
    fn unknown_compile_required_manifest_capability_stops_foundation_decode() {
        let hir = CanonicalHirFoundation::empty();
        let mir = CanonicalMirFoundation::empty();
        let lir = CanonicalLirFoundation::empty();
        let selection = ValidatedLirTargetSelection::DARWIN_AARCH64_LLVM_22_1;
        let cone = ConeRecord::new(
            ConeCoordinate::reserved_core(),
            ConeKind::Library,
            ConeSourceForm::Manifest,
        )
        .unwrap();
        let capability = CapabilityId::new("org.scoop-lang.test", "compile", 1).unwrap();
        let metadata = crate::IdentityFoundationMetadata::new(&hir, &mir, &lir).unwrap();
        let compatibility = crate::CompatibilityRecord::identity_foundation(selection).unwrap();
        let fingerprints = SemanticFingerprintRecord::identity_foundation(
            &compatibility,
            &[],
            metadata.hir_section(),
            metadata.mir_section(),
            metadata.lir_section(),
        )
        .unwrap();
        let members = metadata.into_members(cone.identity()).unwrap();
        let manifest = crate::BootstrapManifest::new(
            ProducerRecord::new("test").unwrap(),
            compatibility,
            cone,
            Vec::new(),
            &members,
            fingerprints,
            vec![
                ManifestSection::new(capability.clone(), MemberPurposeSet::COMPILE, Vec::new())
                    .unwrap(),
            ],
        )
        .unwrap();
        let archive = crate::CanonicalSlibArchive::write_bootstrap(&manifest, members).unwrap();
        let graph = crate::DecodedSlibEnvelope::open(
            archive.as_bytes(),
            DecodeLimits::default(),
            selection,
        )
        .unwrap()
        .validate_graph()
        .unwrap();

        assert!(matches!(
            graph.decode_identity_foundations(),
            Err(IdentityFoundationDecodeError::UnknownCompileCapability {
                location: None,
                index: 0,
                capability: actual,
            }) if actual == capability
        ));
    }
}
