use scoop_hir::{DecodedHirFoundation, ValidatedHirFoundation};
use scoop_identity::{
    CapabilityId, ConeCoordinate, ConeIdentity, IdentityValidationError, PendingIdentityValidation,
    ValidatedIdentityGraph,
};
use scoop_lir::{DecodedLirFoundation, ValidatedLirFoundation};
use scoop_mir::{DecodedMirFoundation, ValidatedMirFoundation};
use scoop_wire::{DecodeUsage, WirePath, decode_canonical_with_meter};

use crate::{
    ArtifactFingerprint, DecodedMetadataEnvelope, DependencyRecord, MemberPurposeSet,
    MetadataLocation, SemanticFingerprintRecord, SlibMemberRecord, SlibMemberRole,
    ValidatedGraphArtifact, hir_identity_foundation_capability, lir_identity_foundation_capability,
    mir_identity_foundation_capability,
};

const IDENTITY_FOUNDATION_HANDLER_BASE_WORK: u64 = 64;

mod error;
pub use error::{FoundationStructureValidationError, IdentityFoundationDecodeError};
mod commit;
pub use commit::{
    CompileCapabilityProfile, CompileCommitError, CrossConeSemanticsStrongProfile,
    IdentityFoundationProfile, SingleConeStrongProfile, ValidatedCompileArtifact,
};
pub(crate) use commit::{charge_identity_import, commit_identity_graph, semantic_identity_import};
mod native_boundary;
pub(crate) use native_boundary::{
    AbiReplayDependency, NativeBoundaryFoundationView, replay_canonical_scoop_abi,
    validate_native_boundary_parts,
};
pub use native_boundary::{
    NativeBoundaryCompileError, NativeBoundarySourceValidatedFoundations,
    NativeBoundaryTargetError, NativeBoundaryValidatedFoundations,
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

/// Foundation payloads whose identities and all per-layer structural
/// relations passed as one HIR-to-LIR transaction.
///
/// Native-boundary closure, semantic-world import, and the final Compile view
/// remain separate proofs and are intentionally not implied by this type.
pub struct StructurallyValidatedFoundations<'input> {
    graph: ValidatedGraphArtifact<'input>,
    identities: ValidatedIdentityGraph,
    hir: ValidatedHirFoundation,
    mir: ValidatedMirFoundation,
    lir: ValidatedLirFoundation,
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
            foundation_handler_meter(&mut self.envelope, MetadataLocation::Hir)?,
        )
        .map_err(|source| IdentityFoundationDecodeError::InnerFoundation {
            location: MetadataLocation::Hir,
            source,
        })?;
        let mir = decode_canonical_with_meter::<DecodedMirFoundation>(
            mir_section.payload(),
            foundation_handler_meter(&mut self.envelope, MetadataLocation::Mir)?,
        )
        .map_err(|source| IdentityFoundationDecodeError::InnerFoundation {
            location: MetadataLocation::Mir,
            source,
        })?;
        let lir = decode_canonical_with_meter::<DecodedLirFoundation>(
            lir_section.payload(),
            foundation_handler_meter(&mut self.envelope, MetadataLocation::Lir)?,
        )
        .map_err(|source| IdentityFoundationDecodeError::InnerFoundation {
            location: MetadataLocation::Lir,
            source,
        })?;

        let actual = {
            let (manifest, meter) = self.envelope.manifest_and_meter();
            SemanticFingerprintRecord::from_decoded_compile_metadata_sections(
                manifest.compatibility(),
                manifest.direct_dependencies(),
                hir_envelope.sections(),
                mir_envelope.sections(),
                lir_envelope.sections(),
                meter,
            )
        }
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

fn foundation_handler_meter<'envelope, 'input>(
    envelope: &'envelope mut crate::DecodedSlibEnvelope<'input>,
    location: MetadataLocation,
) -> Result<&'envelope mut scoop_wire::BudgetMeter, IdentityFoundationDecodeError> {
    let meter = envelope.meter_mut();
    meter
        .charge_work(IDENTITY_FOUNDATION_HANDLER_BASE_WORK, &WirePath::root())
        .map_err(|source| IdentityFoundationDecodeError::InnerFoundation { location, source })?;
    Ok(meter)
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
        mut graph,
        hir,
        mir,
        lir,
    } = foundations;
    let identities = validate_foundation_identity_graph(&mut graph, &hir, &mir, &lir)?;
    Ok(IdentityCheckedFoundations {
        graph,
        identities,
        hir,
        mir,
        lir,
    })
}

pub(crate) fn validate_foundation_identity_graph(
    graph: &mut ValidatedGraphArtifact<'_>,
    hir: &DecodedHirFoundation,
    mir: &DecodedMirFoundation,
    lir: &DecodedLirFoundation,
) -> Result<ValidatedIdentityGraph, IdentityValidationError> {
    validate_foundation_identity_graph_with_authorities(graph, hir, mir, lir, std::iter::empty())
}

pub(crate) fn validate_foundation_identity_graph_with_authorities<'authority>(
    graph: &mut ValidatedGraphArtifact<'_>,
    hir: &DecodedHirFoundation,
    mir: &DecodedMirFoundation,
    lir: &DecodedLirFoundation,
    external_authorities: impl IntoIterator<Item = &'authority ValidatedIdentityGraph>,
) -> Result<ValidatedIdentityGraph, IdentityValidationError> {
    let producer = graph.identity();
    let (manifest, meter) = graph.envelope.manifest_and_meter();
    let mut validation = PendingIdentityValidation::with_meter(meter);
    validation.register_authority(ConeIdentity::CORE)?;
    if producer != ConeIdentity::CORE {
        validation.register_authority(producer)?;
    }
    for dependency in manifest.direct_dependencies() {
        let authority = dependency.identity();
        if authority != ConeIdentity::CORE && authority != producer {
            validation.register_authority(authority)?;
        }
    }

    hir.register_identities(&mut validation)?;
    mir.register_identities(&mut validation)?;
    lir.register_identities(&mut validation)?;
    for authority in external_authorities {
        validation.register_external_graph_authorities(authority)?;
    }

    hir.resolve_identities(&mut validation)?;
    mir.resolve_identities(&mut validation)?;
    lir.resolve_identities(&mut validation)?;

    validation.finish()
}

impl<'input> IdentityCheckedFoundations<'input> {
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

    pub fn declared_identity_count(&self) -> usize {
        self.identities.declared_identity_count()
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

    /// Validates HIR, MIR, and LIR structure in dependency order and only
    /// publishes a proof after every layer succeeds.
    pub fn validate_structure(
        self,
    ) -> Result<StructurallyValidatedFoundations<'input>, FoundationStructureValidationError> {
        let Self {
            mut graph,
            mut identities,
            hir,
            mir,
            lir,
        } = self;
        let producer = graph.identity();
        let (manifest, meter) = graph.envelope.manifest_and_meter();
        let coordinate = manifest.cone().coordinate();
        let hir = hir
            .validate(coordinate, &mut identities, meter)
            .map_err(FoundationStructureValidationError::Hir)?;
        let mir = mir
            .validate(&mut identities, meter)
            .map_err(FoundationStructureValidationError::Mir)?;
        let lir = lir
            .validate(producer, &mut identities, meter)
            .map_err(FoundationStructureValidationError::Lir)?;
        Ok(StructurallyValidatedFoundations {
            graph,
            identities,
            hir,
            mir,
            lir,
        })
    }
}

impl StructurallyValidatedFoundations<'_> {
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

    pub fn declared_identity_count(&self) -> usize {
        self.identities.declared_identity_count()
    }

    pub const fn hir(&self) -> &ValidatedHirFoundation {
        &self.hir
    }

    pub const fn mir(&self) -> &ValidatedMirFoundation {
        &self.mir
    }

    pub const fn lir(&self) -> &ValidatedLirFoundation {
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

#[cfg(test)]
mod tests;
