use std::collections::BTreeSet;
use std::fmt;

use scoop_hir::DecodedHirFoundation;
use scoop_identity::{
    CapabilityId, ConeCoordinate, ConeIdentity, IdentityValidationError, PendingIdentityValidation,
    ValidatedIdentityGraph,
};
use scoop_lir::DecodedLirFoundation;
use scoop_mir::DecodedMirFoundation;
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
    hir: DecodedHirFoundation,
    mir: DecodedMirFoundation,
    lir: DecodedLirFoundation,
}

/// Foundation graph whose complete HIR/MIR/LIR identity transaction passed.
///
/// This proof does not yet include the remaining per-layer structural checks,
/// typed remap, or semantic-world commit, so it is not a Compile proof.
pub struct IdentityCheckedFoundations<'input> {
    graph: ValidatedGraphArtifact<'input>,
    identities: ValidatedIdentityGraph,
    hir: DecodedHirFoundation,
    mir: DecodedMirFoundation,
    lir: DecodedLirFoundation,
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

        let hir = decode_canonical_with_meter::<DecodedHirFoundation>(
            hir_section.payload(),
            self.envelope.meter_mut(),
        )
        .map_err(|source| IdentityFoundationDecodeError::InnerFoundation {
            location: MetadataLocation::Hir,
            source,
        })?;
        let mir = decode_canonical_with_meter::<DecodedMirFoundation>(
            mir_section.payload(),
            self.envelope.meter_mut(),
        )
        .map_err(|source| IdentityFoundationDecodeError::InnerFoundation {
            location: MetadataLocation::Mir,
            source,
        })?;
        let lir = decode_canonical_with_meter::<DecodedLirFoundation>(
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

impl<'input> DecodedIdentityFoundations<'input> {
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

    pub const fn hir_wire(&self) -> &DecodedHirFoundation {
        &self.hir
    }

    pub const fn mir_wire(&self) -> &DecodedMirFoundation {
        &self.mir
    }

    pub const fn lir_wire(&self) -> &DecodedLirFoundation {
        &self.lir
    }

    /// Validates the complete cross-layer identity graph as one transaction.
    /// All three deltas are registered before HIR-to-LIR resolution begins.
    pub fn validate_identities(
        self,
    ) -> Result<IdentityCheckedFoundations<'input>, IdentityValidationError> {
        validate_foundation_identities(self)
    }
}

fn validate_foundation_identities<'input>(
    foundations: DecodedIdentityFoundations<'input>,
) -> Result<IdentityCheckedFoundations<'input>, IdentityValidationError> {
    let DecodedIdentityFoundations {
        graph,
        hir,
        mir,
        lir,
    } = foundations;
    let mut validation = PendingIdentityValidation::new();
    let mut authorities = BTreeSet::from([ConeIdentity::CORE, graph.identity()]);
    authorities.extend(
        graph
            .direct_dependencies()
            .iter()
            .map(DependencyRecord::identity),
    );
    for authority in authorities {
        validation.register_authority(authority)?;
    }

    hir.register_identities(&mut validation)?;
    mir.register_identities(&mut validation)?;
    lir.register_identities(&mut validation)?;

    hir.resolve_identities(&mut validation)?;
    mir.resolve_identities(&mut validation)?;
    lir.resolve_identities(&mut validation)?;

    let identities = validation.finish()?;
    Ok(IdentityCheckedFoundations {
        graph,
        identities,
        hir,
        mir,
        lir,
    })
}

impl IdentityCheckedFoundations<'_> {
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

    pub fn identity_count(&self) -> usize {
        self.identities.identity_count()
    }

    pub const fn hir_wire(&self) -> &DecodedHirFoundation {
        &self.hir
    }

    pub const fn mir_wire(&self) -> &DecodedMirFoundation {
        &self.mir
    }

    pub const fn lir_wire(&self) -> &DecodedLirFoundation {
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
    use scoop_identity::{
        CanonicalIdentifier, CapabilityId, CborIdentityRecord, ConeCoordinate, ConeIdentity,
        DeclarationScope, DefinitionOwnerChain, ExactTypeKey, LayoutKey, PackagePath,
        RepresentationRole, SourceDeclarationKey, SourceDeclarationSite, SourceNominalKind,
    };
    use scoop_lir::{CanonicalLirFoundation, ValidatedLirTargetSelection};
    use scoop_mir::CanonicalMirFoundation;
    use scoop_wire::{DecodeLimits, encode};

    use super::*;
    use crate::{
        ConeKind, ConeRecord, ConeSourceForm, IdentityFoundationArtifact,
        IdentityFoundationArtifactInput, ManifestSection, ProducerRecord,
    };

    #[test]
    fn graph_decodes_and_identity_checks_all_foundation_layers() {
        let selection = ValidatedLirTargetSelection::DARWIN_AARCH64_LLVM_22_1;
        let declaration = SourceDeclarationKey::nominal(
            SourceDeclarationSite::new(
                ConeIdentity::CORE,
                PackagePath::root(),
                DefinitionOwnerChain::top_level(),
                DeclarationScope::ConeWide,
            )
            .unwrap(),
            CanonicalIdentifier::new("Widget").unwrap(),
            SourceNominalKind::Struct,
            0,
        );
        let type_record = CborIdentityRecord::from_key(declaration).unwrap();
        let exact_record =
            CborIdentityRecord::from_key(ExactTypeKey::Nominal(type_record.id())).unwrap();
        let layout_record = CborIdentityRecord::from_key(LayoutKey::new(
            exact_record.id(),
            selection.target().wire_id(),
            RepresentationRole::ManagedValue,
        ))
        .unwrap();
        let mut hir = CanonicalHirFoundation::empty();
        hir.set_types(vec![type_record]).unwrap();
        let mut mir = CanonicalMirFoundation::empty();
        mir.set_exact_types(vec![exact_record]).unwrap();
        let mut lir = CanonicalLirFoundation::empty();
        lir.set_layouts(vec![layout_record]).unwrap();
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

        let checked = decoded.validate_identities().unwrap();
        assert_eq!(checked.identity(), cone.identity());
        assert_eq!(checked.identity_count(), 4);
        assert_eq!(encode(checked.hir_wire()).unwrap(), encode(&hir).unwrap());
        assert_eq!(encode(checked.mir_wire()).unwrap(), encode(&mir).unwrap());
        assert_eq!(encode(checked.lir_wire()).unwrap(), encode(&lir).unwrap());
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
