//! Link-view section inventory and atomic payload decoding.

use std::collections::BTreeSet;
use std::fmt;

use scoop_hir::{
    DecodedCoreBootstrapInterfaceSectionV1, DecodedHirFoundation, OdrFreeHirFoundation,
};
use scoop_identity::{
    ArtifactCapabilityProfileId, CapabilityId, ConeCoordinate, ConeIdentity,
    IdentityValidationError, ValidatedIdentityGraph,
};
use scoop_lir::{
    CBridgeProductionValidationError, CBridgeToolchainProfileV1,
    CanonicalNativeExternalRequirementBuildError, CanonicalNativeExternalRequirementSurfaceV1,
    DecodedLirFoundation, DecodedStrongProductionSectionV1, OdrFreeLirFoundation,
    StrongExternalLirBridgeReconstructionError, StrongExternalLirBridgeSurfaceV1,
    StrongProducerUnitPartitionError, StrongProducerUnitPartitionV1,
};
use scoop_mir::{DecodedCoreBootstrapBridgeSectionV1, DecodedMirFoundation, OdrFreeMirFoundation};
use scoop_wire::{DecodeUsage, WireDecode, WireError, WirePath, decode_canonical_with_meter};

use crate::compile_decode::validate_foundation_identity_graph_with_authorities;
use crate::strong_compile_decode::{
    DecodedStrongProfileProductionSet, OdrFreeStrongFoundationSet, StrongProfileFoundationError,
    StrongProfileProductionError, validate_cross_cone_strong_profile_foundations,
    validate_strong_profile_foundations, validate_strong_profile_production,
};
use crate::{
    ArtifactCapabilityProfile, ArtifactFingerprint, ArtifactProfileInventoryError,
    BuiltinObjectExternalRequirementClosureError, BuiltinObjectSetValidationError,
    CBridgeCheckedSingleConeProductionManifestV1, CBridgeProductionEnvelopeValidationError,
    CBridgeTargetSupportRequirementValidationError, CanonicalDefinedLinkSymbolOwnerSetV1,
    CanonicalUndefinedSymbolRequirementSetV1, CodeFingerprintError,
    CodeLinkObjectMemberValidationError, ConeImageValidationError, ConeKind, ConeSourceForm,
    CoreStrongRequirementValidationError, CrossConeLinkClosureSectionV1,
    CrossConeLinkClosureSectionValidationError, CrossConeStrongRequirementValidationError,
    CurrentConeUndefinedRequirementValidationError, DecodedCrossConeLinkClosureSectionV1,
    DecodedLinkIdentityClosureSectionV1, DecodedMetadataEnvelope,
    DecodedSingleConeProductionManifestV1, DefinedLinkSymbolOwnerBuildError,
    DigestPatchSiteValidationError, EntryPatchError, EntryProductionValidationError,
    FinalObjectNormalizationError, FinalizedUndefinedSymbolRequirementPartitionsV1,
    GeneratedCBridgeObjectCandidateV1, GeneratedCBridgeSemanticValidationError,
    LinkDigestPatchInputValidationError, LinkIdentityClosureSectionV1,
    LinkIdentityClosureSectionValidationError, LinkObjectMaterializationValidationError,
    LinkObjectProjectionValidationError, LinkSymbolProjectionValidationError, ManifestSection,
    MaterializationCheckedLinkIdentityClosureSectionV1, MetadataLocation, MetadataReadError,
    ObjectProjectionCheckedLinkIdentityClosureSectionV1, PlannedStrongObjectSymbolSetV1,
    ProductionCodeProjectionError, RuntimeAndEhRequirementValidationError,
    RuntimeImageFingerprintError, RuntimeImagePatchError, ScoopLirObjectCandidateV1,
    ScoopLirStackmapValidationError, SemanticFingerprintError, SemanticFingerprintRecord,
    SingleConeProductionManifestV1, SingleConeProductionManifestValidationError, SlibMemberId,
    SlibMemberRecord, SlibMemberRole, SourceExternalRequirementValidationError,
    StrongCallableBodyFingerprintError, StrongCallableFingerprintError,
    StrongCallableRegistrationObjectFingerprintError, StrongCallableRegistrationValidationError,
    StrongImmortalObjectDefinitionFingerprintError, StrongImmortalObjectFingerprintError,
    StrongImmortalObjectRegistrationObjectFingerprintError,
    StrongImmortalObjectRegistrationValidationError,
    StrongInitializationDefinitionFingerprintError, StrongInitializationFingerprintError,
    StrongInitializationRegistrationObjectFingerprintError,
    StrongInitializationRegistrationValidationError, StrongObjectSymbolPlanningError,
    StrongRegistrationPatchError, StrongSafepointFingerprintError,
    StrongSafepointRegistrationValidationError, StrongStaticStorageDefinitionFingerprintError,
    StrongStaticStorageFingerprintError, StrongStaticStorageRegistrationObjectFingerprintError,
    StrongStaticStorageRegistrationValidationError, StrongStaticStorageShapeFingerprintError,
    StrongTypeDependencyFingerprintError, StrongTypeFingerprintError,
    StrongTypeRegistrationObjectFingerprintError, StrongTypeRegistrationValidationError,
    SymbolProjectionCheckedLinkIdentityClosureSectionV1,
    UndefinedSymbolRequirementFinalizationError, ValidatedGraphArtifact,
    ValidatedSingleConeStrongProduction, VerifiedBuiltinObjectStrongRelocationSetV1,
    VerifiedCBridgeProductionEnvelopeSetV1, VerifiedEntryPatchSetV1,
    VerifiedNormalizedProvisionalScoopLirObjectSetV1, VerifiedScoopLirDigestPatchSiteSetV1,
    VerifiedScoopLirStackmapSetV1, VerifiedStrongCallableFingerprintSetV1,
    VerifiedStrongCallableRegistrationObjectFingerprintSetV1,
    VerifiedStrongCallableRegistrationSetV1, VerifiedStrongImmortalObjectFingerprintSetV1,
    VerifiedStrongImmortalObjectRegistrationObjectFingerprintSetV1,
    VerifiedStrongImmortalObjectRegistrationSetV1, VerifiedStrongInitializationFingerprintSetV1,
    VerifiedStrongInitializationRegistrationObjectFingerprintSetV1,
    VerifiedStrongInitializationRegistrationSetV1, VerifiedStrongSafepointFingerprintSetV1,
    VerifiedStrongSafepointRegistrationSetV1, VerifiedStrongStaticStorageFingerprintSetV1,
    VerifiedStrongStaticStorageRegistrationObjectFingerprintSetV1,
    VerifiedStrongStaticStorageRegistrationSetV1, VerifiedStrongTypeFingerprintSetV1,
    VerifiedStrongTypeRegistrationObjectFingerprintSetV1, VerifiedStrongTypeRegistrationSetV1,
    hir_core_bootstrap_interface_capability, hir_identity_foundation_capability,
    lir_cross_cone_link_closure_capability, lir_identity_foundation_capability,
    lir_link_identity_closure_capability, lir_strong_production_capability,
    manifest_single_cone_production_capability, mir_core_bootstrap_bridge_capability,
    mir_identity_foundation_capability,
};

const LINK_SECTION_HANDLER_BASE_WORK: u64 = 64;

mod states;
pub use states::*;

mod cross_cone;
mod decode;
pub use cross_cone::*;
mod fingerprinting;
mod object_validation;

pub fn validate_single_cone_strong_link_artifact<'input>(
    graph: ValidatedGraphArtifact<'input>,
    expected_external_bridges: &StrongExternalLirBridgeSurfaceV1,
    core_owners: &CanonicalDefinedLinkSymbolOwnerSetV1,
    c_bridge_profile: &CBridgeToolchainProfileV1,
) -> Result<ValidatedSingleConeStrongLinkArtifact<'input>, StrongLinkArtifactValidationError> {
    graph
        .decode_single_cone_link_sections()
        .map_err(|error| StrongLinkArtifactValidationError::Decode(Box::new(error)))?
        .validate_identities()
        .map_err(|error| StrongLinkArtifactValidationError::Identities(Box::new(error)))?
        .validate_foundation_structure()
        .map_err(|error| StrongLinkArtifactValidationError::Foundations(Box::new(error)))?
        .validate_production(expected_external_bridges)
        .map_err(|error| StrongLinkArtifactValidationError::Production(Box::new(error)))?
        .validate_materializations()
        .map_err(|error| StrongLinkArtifactValidationError::Materializations(Box::new(error)))?
        .validate_c_bridge_envelopes(c_bridge_profile)
        .map_err(|error| StrongLinkArtifactValidationError::CBridge(Box::new(error)))?
        .validate_builtin_objects()
        .map_err(|error| StrongLinkArtifactValidationError::BuiltinObjects(Box::new(error)))?
        .validate_digest_patch_sites()
        .map_err(|error| StrongLinkArtifactValidationError::DigestPatches(Box::new(error)))?
        .validate_registration_objects()
        .map_err(|error| StrongLinkArtifactValidationError::RegistrationObjects(Box::new(error)))?
        .fingerprint_registration_leaves()
        .map_err(|error| StrongLinkArtifactValidationError::RegistrationLeaves(Box::new(error)))?
        .validate_link_symbol_requirements(core_owners, c_bridge_profile)
        .map_err(|error| StrongLinkArtifactValidationError::Symbols(Box::new(error)))?
        .fingerprint_registration_dependencies()
        .map_err(|error| {
            StrongLinkArtifactValidationError::RegistrationDependencies(Box::new(error))
        })?
        .finalize_strong_objects()
        .map_err(|error| StrongLinkArtifactValidationError::ObjectFinalization(Box::new(error)))?
        .validate_code_and_closure()
        .map_err(|error| StrongLinkArtifactValidationError::FinalProof(Box::new(error)))
}

/// Validates a previously published artifact without compiler-side IR by
/// reconstructing its typed external bridge surface from the validated Link
/// identity graph before replaying every remaining Link proof.
pub fn validate_self_describing_single_cone_strong_link_artifact<'input>(
    graph: ValidatedGraphArtifact<'input>,
    core_owners: &CanonicalDefinedLinkSymbolOwnerSetV1,
    c_bridge_profile: &CBridgeToolchainProfileV1,
) -> Result<ValidatedSingleConeStrongLinkArtifact<'input>, StrongLinkArtifactValidationError> {
    let mut front = graph
        .decode_single_cone_link_sections()
        .map_err(|error| StrongLinkArtifactValidationError::Decode(Box::new(error)))?
        .validate_identities()
        .map_err(|error| StrongLinkArtifactValidationError::Identities(Box::new(error)))?
        .validate_foundation_structure()
        .map_err(|error| StrongLinkArtifactValidationError::Foundations(Box::new(error)))?;
    let external_bridges = front
        .reconstruct_external_bridges()
        .map_err(|error| StrongLinkArtifactValidationError::ExternalBridges(Box::new(error)))?;
    front
        .validate_production(&external_bridges)
        .map_err(|error| StrongLinkArtifactValidationError::Production(Box::new(error)))?
        .validate_materializations()
        .map_err(|error| StrongLinkArtifactValidationError::Materializations(Box::new(error)))?
        .validate_c_bridge_envelopes(c_bridge_profile)
        .map_err(|error| StrongLinkArtifactValidationError::CBridge(Box::new(error)))?
        .validate_builtin_objects()
        .map_err(|error| StrongLinkArtifactValidationError::BuiltinObjects(Box::new(error)))?
        .validate_digest_patch_sites()
        .map_err(|error| StrongLinkArtifactValidationError::DigestPatches(Box::new(error)))?
        .validate_registration_objects()
        .map_err(|error| StrongLinkArtifactValidationError::RegistrationObjects(Box::new(error)))?
        .fingerprint_registration_leaves()
        .map_err(|error| StrongLinkArtifactValidationError::RegistrationLeaves(Box::new(error)))?
        .validate_link_symbol_requirements(core_owners, c_bridge_profile)
        .map_err(|error| StrongLinkArtifactValidationError::Symbols(Box::new(error)))?
        .fingerprint_registration_dependencies()
        .map_err(|error| {
            StrongLinkArtifactValidationError::RegistrationDependencies(Box::new(error))
        })?
        .finalize_strong_objects()
        .map_err(|error| StrongLinkArtifactValidationError::ObjectFinalization(Box::new(error)))?
        .validate_code_and_closure()
        .map_err(|error| StrongLinkArtifactValidationError::FinalProof(Box::new(error)))
}

fn validate_object_directory<'input>(
    graph: &ValidatedGraphArtifact<'input>,
    plan: &crate::PlannedLinkObjectMemberSetV1,
) -> Result<
    (
        Vec<ScoopLirObjectCandidateV1<'input>>,
        Vec<GeneratedCBridgeObjectCandidateV1<'input>>,
    ),
    StrongLinkMaterializationError,
> {
    let expected = plan
        .scoop_lir_members()
        .iter()
        .map(|member| member.member_id())
        .chain(
            plan.generated_bridge_members()
                .iter()
                .map(|member| member.member_id()),
        )
        .collect::<BTreeSet<_>>();
    let actual = graph
        .envelope
        .manifest()
        .members()
        .iter()
        .filter(|member| matches!(member.role(), SlibMemberRole::LinkObject { .. }))
        .map(SlibMemberRecord::id)
        .collect::<BTreeSet<_>>();
    if let Some(member) = actual.difference(&expected).next() {
        return Err(StrongLinkMaterializationError::UnexpectedObjectMember(
            *member,
        ));
    }
    if let Some(member) = expected.difference(&actual).next() {
        return Err(StrongLinkMaterializationError::MissingObjectMember(*member));
    }

    let scoop_objects = plan
        .scoop_lir_members()
        .iter()
        .map(|member| {
            required_object_payload(
                graph,
                member.member_id(),
                member.stable_key(),
                member.role(),
            )
            .map(|bytes| ScoopLirObjectCandidateV1::new(member.member_id(), bytes))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let generated_bridge_objects = plan
        .generated_bridge_members()
        .iter()
        .map(|member| {
            required_object_payload(
                graph,
                member.member_id(),
                member.stable_key(),
                member.role(),
            )
            .map(|bytes| GeneratedCBridgeObjectCandidateV1::new(member.member_id(), bytes))
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok((scoop_objects, generated_bridge_objects))
}

fn verify_reconstructed_scoop_objects(
    normalized: &VerifiedNormalizedProvisionalScoopLirObjectSetV1,
    reconstructed: &VerifiedEntryPatchSetV1,
) -> Result<(), ReconstructedScoopObjectError> {
    if normalized.objects().len() != reconstructed.objects().len() {
        return Err(ReconstructedScoopObjectError::ObjectCount {
            expected: normalized.objects().len(),
            actual: reconstructed.objects().len(),
        });
    }
    for (expected, actual) in normalized.objects().iter().zip(reconstructed.objects()) {
        if expected.member() != actual.member() {
            return Err(ReconstructedScoopObjectError::ObjectOrder {
                expected: expected.member(),
                actual: actual.member(),
            });
        }
        if expected.final_bytes() != actual.bytes() {
            return Err(ReconstructedScoopObjectError::ByteMismatch(
                expected.member(),
            ));
        }
    }
    Ok(())
}

fn required_object_payload<'input>(
    graph: &ValidatedGraphArtifact<'input>,
    member: SlibMemberId,
    stable_key: &crate::MemberStableKey,
    role: &SlibMemberRole,
) -> Result<&'input [u8], StrongLinkMaterializationError> {
    let record = graph
        .envelope
        .manifest()
        .members()
        .binary_search_by_key(&member, SlibMemberRecord::id)
        .ok()
        .map(|index| &graph.envelope.manifest().members()[index])
        .ok_or(StrongLinkMaterializationError::MissingObjectMember(member))?;
    if record.stable_key() != stable_key || record.role() != role {
        return Err(StrongLinkMaterializationError::ObjectRecordMismatch(member));
    }
    graph
        .envelope
        .member(member)
        .ok_or(StrongLinkMaterializationError::MissingObjectPayload(member))
}

fn require_strong_profile(
    graph: &ValidatedGraphArtifact<'_>,
    expected: ArtifactCapabilityProfile,
) -> Result<ArtifactCapabilityProfile, SingleConeLinkSectionDecodeError> {
    let actual = graph.envelope.manifest().compatibility().artifact_profile();
    if actual != &expected.id() {
        return Err(SingleConeLinkSectionDecodeError::WrongProfile {
            actual: actual.clone(),
        });
    }
    Ok(expected)
}

fn decode_production_manifest(
    graph: &mut ValidatedGraphArtifact<'_>,
    profile: ArtifactCapabilityProfile,
) -> Result<DecodedSingleConeProductionManifestV1, SingleConeLinkSectionDecodeError> {
    let (manifest, meter) = graph.envelope.manifest_and_meter();
    profile
        .validate_link_manifest_inventory(manifest.sections())
        .map_err(SingleConeLinkSectionDecodeError::Inventory)?;
    let section = required_manifest_section(
        manifest.sections(),
        manifest_single_cone_production_capability(),
    )?;
    meter
        .charge_work(LINK_SECTION_HANDLER_BASE_WORK, &Default::default())
        .map_err(SingleConeLinkSectionDecodeError::Resource)?;
    decode_canonical_with_meter::<DecodedSingleConeProductionManifestV1>(section.payload(), meter)
        .map_err(|source| SingleConeLinkSectionDecodeError::InnerSection {
            location: None,
            capability: manifest_single_cone_production_capability(),
            source,
        })
}

fn metadata_member_id(
    graph: &ValidatedGraphArtifact<'_>,
    location: MetadataLocation,
) -> Result<SlibMemberId, SingleConeLinkSectionDecodeError> {
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
        .map(SlibMemberRecord::id)
        .ok_or(SingleConeLinkSectionDecodeError::MissingMetadataMember { location })
}

fn member_payload<'input>(
    graph: &ValidatedGraphArtifact<'input>,
    location: MetadataLocation,
    member: SlibMemberId,
) -> Result<&'input [u8], SingleConeLinkSectionDecodeError> {
    graph
        .envelope
        .member(member)
        .ok_or(SingleConeLinkSectionDecodeError::MissingMetadataMemberPayload { location, member })
}

fn decode_metadata_envelope<'input>(
    graph: &mut ValidatedGraphArtifact<'input>,
    payload: &'input [u8],
    location: MetadataLocation,
) -> Result<DecodedMetadataEnvelope<'input>, SingleConeLinkSectionDecodeError> {
    DecodedMetadataEnvelope::decode_with_meter(payload, location, graph.envelope.meter_mut())
        .map_err(|source| SingleConeLinkSectionDecodeError::OuterEnvelope { location, source })
}

fn required_manifest_section(
    sections: &[ManifestSection],
    capability: CapabilityId,
) -> Result<&ManifestSection, SingleConeLinkSectionDecodeError> {
    sections
        .iter()
        .find(|section| section.capability() == &capability)
        .ok_or(SingleConeLinkSectionDecodeError::MissingSection {
            location: None,
            capability,
        })
}

fn required_metadata_section<'input>(
    envelope: &DecodedMetadataEnvelope<'input>,
    capability: &CapabilityId,
) -> Result<&'input [u8], SingleConeLinkSectionDecodeError> {
    envelope
        .sections()
        .iter()
        .find(|section| section.capability() == capability)
        .map(crate::DecodedMetadataSection::payload)
        .ok_or_else(|| SingleConeLinkSectionDecodeError::MissingSection {
            location: Some(envelope.location()),
            capability: capability.clone(),
        })
}

fn decode_inner<T: WireDecode>(
    graph: &mut ValidatedGraphArtifact<'_>,
    location: MetadataLocation,
    capability: CapabilityId,
    payload: &[u8],
) -> Result<T, SingleConeLinkSectionDecodeError> {
    let meter = graph.envelope.meter_mut();
    meter
        .charge_work(LINK_SECTION_HANDLER_BASE_WORK, &WirePath::root())
        .map_err(|source| SingleConeLinkSectionDecodeError::InnerSection {
            location: Some(location),
            capability: capability.clone(),
            source,
        })?;
    decode_canonical_with_meter(payload, meter).map_err(|source| {
        SingleConeLinkSectionDecodeError::InnerSection {
            location: Some(location),
            capability,
            source,
        }
    })
}

fn validate_semantic_fingerprints(
    graph: &mut ValidatedGraphArtifact<'_>,
    hir: &DecodedMetadataEnvelope<'_>,
    mir: &DecodedMetadataEnvelope<'_>,
    lir: &DecodedMetadataEnvelope<'_>,
) -> Result<(), SingleConeLinkSectionDecodeError> {
    let actual = {
        let (manifest, meter) = graph.envelope.manifest_and_meter();
        SemanticFingerprintRecord::from_decoded_compile_metadata_sections(
            manifest.compatibility(),
            manifest.direct_dependencies(),
            hir.sections(),
            mir.sections(),
            lir.sections(),
            meter,
        )
    }
    .map_err(SingleConeLinkSectionDecodeError::SemanticFingerprints)?;
    let expected = graph.envelope.manifest().semantic_fingerprints();
    require_fingerprint(
        MetadataLocation::Hir,
        expected.hir().as_array(),
        actual.hir().as_array(),
    )?;
    require_fingerprint(
        MetadataLocation::Mir,
        expected.mir().as_array(),
        actual.mir().as_array(),
    )?;
    require_fingerprint(
        MetadataLocation::Lir,
        expected.lir().as_array(),
        actual.lir().as_array(),
    )
}

fn require_fingerprint(
    location: MetadataLocation,
    expected: &[u8; 32],
    actual: &[u8; 32],
) -> Result<(), SingleConeLinkSectionDecodeError> {
    if expected == actual {
        Ok(())
    } else {
        Err(
            SingleConeLinkSectionDecodeError::SemanticFingerprintMismatch {
                location,
                expected: *expected,
                actual: *actual,
            },
        )
    }
}

#[derive(Debug)]
pub enum StrongLinkMaterializationError {
    ProducerUnits(StrongProducerUnitPartitionError),
    Closure(LinkObjectMaterializationValidationError),
    UnexpectedObjectMember(SlibMemberId),
    MissingObjectMember(SlibMemberId),
    ObjectRecordMismatch(SlibMemberId),
    MissingObjectPayload(SlibMemberId),
}

impl fmt::Display for StrongLinkMaterializationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "invalid strong Link materialization: {self:?}")
    }
}

impl std::error::Error for StrongLinkMaterializationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::ProducerUnits(error) => Some(error),
            Self::Closure(error) => Some(error),
            Self::UnexpectedObjectMember(_)
            | Self::MissingObjectMember(_)
            | Self::ObjectRecordMismatch(_)
            | Self::MissingObjectPayload(_) => None,
        }
    }
}

#[derive(Debug)]
pub enum StrongLinkCBridgeError {
    ManifestProduction(CBridgeProductionValidationError),
    ObjectEnvelopes(CBridgeProductionEnvelopeValidationError),
}

impl fmt::Display for StrongLinkCBridgeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "invalid strong Link C bridge production: {self:?}"
        )
    }
}

impl std::error::Error for StrongLinkCBridgeError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(match self {
            Self::ManifestProduction(error) => error,
            Self::ObjectEnvelopes(error) => error,
        })
    }
}

#[derive(Debug)]
pub enum StrongLinkBuiltinObjectError {
    ClosureInput(LinkDigestPatchInputValidationError),
    Normalization(FinalObjectNormalizationError),
    SymbolPlan(StrongObjectSymbolPlanningError),
    ObjectSet(BuiltinObjectSetValidationError),
}

impl fmt::Display for StrongLinkBuiltinObjectError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "invalid strong Link built-in object set: {self:?}"
        )
    }
}

impl std::error::Error for StrongLinkBuiltinObjectError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(match self {
            Self::ClosureInput(error) => error,
            Self::Normalization(error) => error,
            Self::SymbolPlan(error) => error,
            Self::ObjectSet(error) => error,
        })
    }
}

#[derive(Debug)]
pub enum StrongLinkDigestPatchError {
    ObjectSites(DigestPatchSiteValidationError),
    ClosureProjection(LinkObjectProjectionValidationError),
}

#[derive(Debug)]
pub enum StrongLinkRegistrationObjectError {
    Stackmaps(ScoopLirStackmapValidationError),
    Safepoints(StrongSafepointRegistrationValidationError),
    Callables(StrongCallableRegistrationValidationError),
    Types(StrongTypeRegistrationValidationError),
    ImmortalObjects(StrongImmortalObjectRegistrationValidationError),
    StaticStorages(StrongStaticStorageRegistrationValidationError),
    InitializationUnits(StrongInitializationRegistrationValidationError),
}

impl fmt::Display for StrongLinkRegistrationObjectError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "invalid strong Link registration object set: {self:?}"
        )
    }
}

impl std::error::Error for StrongLinkRegistrationObjectError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(match self {
            Self::Stackmaps(error) => error,
            Self::Safepoints(error) => error,
            Self::Callables(error) => error,
            Self::Types(error) => error,
            Self::ImmortalObjects(error) => error,
            Self::StaticStorages(error) => error,
            Self::InitializationUnits(error) => error,
        })
    }
}

#[derive(Debug)]
pub enum StrongLinkRegistrationLeafFingerprintError {
    Safepoints(StrongSafepointFingerprintError),
    Callables(StrongCallableRegistrationObjectFingerprintError),
    Types(StrongTypeRegistrationObjectFingerprintError),
    ImmortalObjects(StrongImmortalObjectRegistrationObjectFingerprintError),
    StaticStorages(StrongStaticStorageRegistrationObjectFingerprintError),
    InitializationUnits(StrongInitializationRegistrationObjectFingerprintError),
}

impl fmt::Display for StrongLinkRegistrationLeafFingerprintError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "failed to fingerprint strong Link registration leaves: {self:?}"
        )
    }
}

impl std::error::Error for StrongLinkRegistrationLeafFingerprintError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(match self {
            Self::Safepoints(error) => error,
            Self::Callables(error) => error,
            Self::Types(error) => error,
            Self::ImmortalObjects(error) => error,
            Self::StaticStorages(error) => error,
            Self::InitializationUnits(error) => error,
        })
    }
}

#[derive(Debug)]
pub enum StrongLinkSymbolRequirementError {
    DefinedSymbols(DefinedLinkSymbolOwnerBuildError),
    CurrentCone(CurrentConeUndefinedRequirementValidationError),
    NativeSurface(CanonicalNativeExternalRequirementBuildError),
    Core(CoreStrongRequirementValidationError),
    CrossCone(CrossConeStrongRequirementValidationError),
    SourceExternal(SourceExternalRequirementValidationError),
    RuntimeAndEh(RuntimeAndEhRequirementValidationError),
    GeneratedBridgeSemantics(GeneratedCBridgeSemanticValidationError),
    CBridgeTargetSupport(CBridgeTargetSupportRequirementValidationError),
    UnclassifiedExternal(BuiltinObjectExternalRequirementClosureError),
    UndefinedSymbols(UndefinedSymbolRequirementFinalizationError),
    CrossConeClosure(CrossConeLinkClosureSectionValidationError),
    ClosureProjection(LinkSymbolProjectionValidationError),
}

impl fmt::Display for StrongLinkSymbolRequirementError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "invalid strong Link symbol closure: {self:?}")
    }
}

impl std::error::Error for StrongLinkSymbolRequirementError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(match self {
            Self::DefinedSymbols(error) => error,
            Self::CurrentCone(error) => error,
            Self::NativeSurface(error) => error,
            Self::Core(error) => error,
            Self::CrossCone(error) => error,
            Self::SourceExternal(error) => error,
            Self::RuntimeAndEh(error) => error,
            Self::GeneratedBridgeSemantics(error) => error,
            Self::CBridgeTargetSupport(error) => error,
            Self::UnclassifiedExternal(error) => error,
            Self::UndefinedSymbols(error) => error,
            Self::CrossConeClosure(error) => error,
            Self::ClosureProjection(error) => error,
        })
    }
}

#[derive(Debug)]
pub enum StrongLinkRegistrationDependencyFingerprintError {
    CallableBodies(StrongCallableBodyFingerprintError),
    Callables(StrongCallableFingerprintError),
    TypeDependencies(StrongTypeDependencyFingerprintError),
    Types(StrongTypeFingerprintError),
    ImmortalObjectDefinitions(StrongImmortalObjectDefinitionFingerprintError),
    ImmortalObjects(StrongImmortalObjectFingerprintError),
    StaticStorageDefinitions(StrongStaticStorageDefinitionFingerprintError),
    StaticStorageShapes(StrongStaticStorageShapeFingerprintError),
    StaticStorages(StrongStaticStorageFingerprintError),
    InitializationDefinitions(StrongInitializationDefinitionFingerprintError),
    Initializations(StrongInitializationFingerprintError),
}

#[derive(Debug)]
pub enum StrongLinkObjectFinalizationError {
    ImageValidation(ConeImageValidationError),
    EntryValidation(EntryProductionValidationError),
    RegistrationPatch(StrongRegistrationPatchError),
    ImageFingerprint(RuntimeImageFingerprintError),
    ImagePatch(RuntimeImagePatchError),
    EntryPatch(EntryPatchError),
    FinalObjectMismatch(ReconstructedScoopObjectError),
}

impl fmt::Display for StrongLinkObjectFinalizationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "failed to finalize strong Link objects: {self:?}"
        )
    }
}

impl std::error::Error for StrongLinkObjectFinalizationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(match self {
            Self::ImageValidation(error) => error,
            Self::EntryValidation(error) => error,
            Self::RegistrationPatch(error) => error,
            Self::ImageFingerprint(error) => error,
            Self::ImagePatch(error) => error,
            Self::EntryPatch(error) => error,
            Self::FinalObjectMismatch(error) => error,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ReconstructedScoopObjectError {
    ObjectCount {
        expected: usize,
        actual: usize,
    },
    ObjectOrder {
        expected: SlibMemberId,
        actual: SlibMemberId,
    },
    ByteMismatch(SlibMemberId),
}

impl fmt::Display for ReconstructedScoopObjectError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "reconstructed Scoop object differs from final archive bytes: {self:?}"
        )
    }
}

impl std::error::Error for ReconstructedScoopObjectError {}

#[derive(Debug)]
pub enum StrongLinkFinalValidationError {
    NativeSurface(CanonicalNativeExternalRequirementBuildError),
    LinkObjectMembers(CodeLinkObjectMemberValidationError),
    ProductionProjection(ProductionCodeProjectionError),
    CodeFingerprint(CodeFingerprintError),
    SemanticFingerprintMismatch,
    ProductionManifest(SingleConeProductionManifestValidationError),
    LinkIdentityClosure(LinkIdentityClosureSectionValidationError),
    CrossConeLinkClosure(CrossConeLinkClosureSectionValidationError),
}

impl fmt::Display for StrongLinkFinalValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "invalid final strong Link proof: {self:?}")
    }
}

impl std::error::Error for StrongLinkFinalValidationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::NativeSurface(error) => Some(error),
            Self::LinkObjectMembers(error) => Some(error),
            Self::ProductionProjection(error) => Some(error),
            Self::CodeFingerprint(error) => Some(error),
            Self::SemanticFingerprintMismatch => None,
            Self::ProductionManifest(error) => Some(error),
            Self::LinkIdentityClosure(error) => Some(error),
            Self::CrossConeLinkClosure(error) => Some(error),
        }
    }
}

#[derive(Debug)]
pub enum StrongLinkArtifactValidationError {
    Decode(Box<SingleConeLinkSectionDecodeError>),
    Identities(Box<IdentityValidationError>),
    Foundations(Box<StrongProfileFoundationError>),
    ExternalBridges(Box<StrongExternalLirBridgeReconstructionError>),
    Production(Box<StrongProfileProductionError>),
    Materializations(Box<StrongLinkMaterializationError>),
    CBridge(Box<StrongLinkCBridgeError>),
    BuiltinObjects(Box<StrongLinkBuiltinObjectError>),
    DigestPatches(Box<StrongLinkDigestPatchError>),
    RegistrationObjects(Box<StrongLinkRegistrationObjectError>),
    RegistrationLeaves(Box<StrongLinkRegistrationLeafFingerprintError>),
    Symbols(Box<StrongLinkSymbolRequirementError>),
    RegistrationDependencies(Box<StrongLinkRegistrationDependencyFingerprintError>),
    ObjectFinalization(Box<StrongLinkObjectFinalizationError>),
    FinalProof(Box<StrongLinkFinalValidationError>),
}

impl fmt::Display for StrongLinkArtifactValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "invalid strong Link artifact: {self:?}")
    }
}

impl std::error::Error for StrongLinkArtifactValidationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(match self {
            Self::Decode(error) => error.as_ref(),
            Self::Identities(error) => error.as_ref(),
            Self::Foundations(error) => error.as_ref(),
            Self::ExternalBridges(error) => error.as_ref(),
            Self::Production(error) => error.as_ref(),
            Self::Materializations(error) => error.as_ref(),
            Self::CBridge(error) => error.as_ref(),
            Self::BuiltinObjects(error) => error.as_ref(),
            Self::DigestPatches(error) => error.as_ref(),
            Self::RegistrationObjects(error) => error.as_ref(),
            Self::RegistrationLeaves(error) => error.as_ref(),
            Self::Symbols(error) => error.as_ref(),
            Self::RegistrationDependencies(error) => error.as_ref(),
            Self::ObjectFinalization(error) => error.as_ref(),
            Self::FinalProof(error) => error.as_ref(),
        })
    }
}

impl fmt::Display for StrongLinkRegistrationDependencyFingerprintError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "failed to fingerprint strong Link registration dependencies: {self:?}"
        )
    }
}

impl std::error::Error for StrongLinkRegistrationDependencyFingerprintError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(match self {
            Self::CallableBodies(error) => error,
            Self::Callables(error) => error,
            Self::TypeDependencies(error) => error,
            Self::Types(error) => error,
            Self::ImmortalObjectDefinitions(error) => error,
            Self::ImmortalObjects(error) => error,
            Self::StaticStorageDefinitions(error) => error,
            Self::StaticStorageShapes(error) => error,
            Self::StaticStorages(error) => error,
            Self::InitializationDefinitions(error) => error,
            Self::Initializations(error) => error,
        })
    }
}

impl fmt::Display for StrongLinkDigestPatchError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "invalid strong Link digest patch set: {self:?}")
    }
}

impl std::error::Error for StrongLinkDigestPatchError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(match self {
            Self::ObjectSites(error) => error,
            Self::ClosureProjection(error) => error,
        })
    }
}

#[derive(Debug)]
pub enum SingleConeLinkSectionDecodeError {
    WrongProfile {
        actual: ArtifactCapabilityProfileId,
    },
    Inventory(ArtifactProfileInventoryError),
    MissingMetadataMember {
        location: MetadataLocation,
    },
    MissingMetadataMemberPayload {
        location: MetadataLocation,
        member: SlibMemberId,
    },
    OuterEnvelope {
        location: MetadataLocation,
        source: MetadataReadError,
    },
    MissingSection {
        location: Option<MetadataLocation>,
        capability: CapabilityId,
    },
    InnerSection {
        location: Option<MetadataLocation>,
        capability: CapabilityId,
        source: WireError,
    },
    Resource(WireError),
    SemanticFingerprints(SemanticFingerprintError),
    SemanticFingerprintMismatch {
        location: MetadataLocation,
        expected: [u8; 32],
        actual: [u8; 32],
    },
}

impl fmt::Display for SingleConeLinkSectionDecodeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "cannot decode strong Link sections: {self:?}")
    }
}

impl std::error::Error for SingleConeLinkSectionDecodeError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Inventory(error) => Some(error),
            Self::OuterEnvelope { source, .. } => Some(source),
            Self::InnerSection { source, .. } | Self::Resource(source) => Some(source),
            Self::SemanticFingerprints(error) => Some(error),
            Self::WrongProfile { .. }
            | Self::MissingMetadataMember { .. }
            | Self::MissingMetadataMemberPayload { .. }
            | Self::MissingSection { .. }
            | Self::SemanticFingerprintMismatch { .. } => None,
        }
    }
}

#[cfg(test)]
pub(crate) mod tests;

#[cfg(test)]
pub(crate) fn strong_production_fixture_for_test(
    coordinate: ConeCoordinate,
) -> (
    scoop_lir::CanonicalLirFoundation,
    scoop_lir::StrongProductionSectionV1,
) {
    tests::strong_production_fixture(coordinate)
}

#[cfg(test)]
pub(crate) fn complete_strong_artifact_for_test(corrupt_final_image_digest: bool) -> Vec<u8> {
    tests::complete_artifact(corrupt_final_image_digest)
}

#[cfg(test)]
pub(crate) fn complete_strong_artifact_identity_for_test() -> ConeIdentity {
    tests::cone().identity()
}

#[cfg(test)]
pub(crate) fn c_bridge_profile_for_test() -> CBridgeToolchainProfileV1 {
    tests::c_bridge_profile()
}
