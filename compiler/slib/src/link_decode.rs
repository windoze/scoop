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
    StrongExternalLirBridgeSurfaceV1, StrongProducerUnitPartitionError,
    StrongProducerUnitPartitionV1,
};
use scoop_mir::{DecodedCoreBootstrapBridgeSectionV1, DecodedMirFoundation, OdrFreeMirFoundation};
use scoop_wire::{DecodeUsage, WireDecode, WireError, WirePath, decode_canonical_with_meter};

use crate::compile_decode::validate_foundation_identity_graph;
use crate::strong_compile_decode::{
    DecodedStrongProfileProductionSet, OdrFreeStrongFoundationSet, StrongProfileFoundationError,
    StrongProfileProductionError, validate_strong_profile_foundations,
    validate_strong_profile_production,
};
use crate::{
    ArtifactCapabilityProfile, ArtifactFingerprint, ArtifactProfileInventoryError,
    BuiltinObjectExternalRequirementClosureError, BuiltinObjectSetValidationError,
    CBridgeCheckedSingleConeProductionManifestV1, CBridgeProductionEnvelopeValidationError,
    CBridgeTargetSupportRequirementValidationError, CanonicalDefinedLinkSymbolOwnerSetV1,
    CanonicalUndefinedSymbolRequirementSetV1, CodeFingerprintError,
    CodeLinkObjectMemberValidationError, ConeImageValidationError, ConeKind, ConeSourceForm,
    CoreStrongRequirementValidationError, CurrentConeUndefinedRequirementValidationError,
    DecodedLinkIdentityClosureSectionV1, DecodedMetadataEnvelope,
    DecodedSingleConeProductionManifestV1, DefinedLinkSymbolOwnerBuildError,
    DigestPatchSiteValidationError, EntryPatchError, EntryProductionValidationError,
    FinalObjectNormalizationError, GeneratedCBridgeObjectCandidateV1,
    GeneratedCBridgeSemanticValidationError, LinkDigestPatchInputValidationError,
    LinkIdentityClosureSectionV1, LinkIdentityClosureSectionValidationError,
    LinkObjectMaterializationValidationError, LinkObjectProjectionValidationError,
    LinkSymbolProjectionValidationError, ManifestSection,
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
    lir_identity_foundation_capability, lir_link_identity_closure_capability,
    lir_strong_production_capability, manifest_single_cone_production_capability,
    mir_core_bootstrap_bridge_capability, mir_identity_foundation_capability,
};

const LINK_SECTION_HANDLER_BASE_WORK: u64 = 64;

/// The three Link payloads decoded atomically from one strong-profile graph
/// artifact. No identity, fingerprint, object, or manifest value has yet been
/// promoted to a Link proof.
#[derive(Debug)]
pub struct DecodedSingleConeLinkSections<'input> {
    graph: ValidatedGraphArtifact<'input>,
    hir_foundation: DecodedHirFoundation,
    hir_production: DecodedCoreBootstrapInterfaceSectionV1,
    mir_foundation: DecodedMirFoundation,
    mir_production: DecodedCoreBootstrapBridgeSectionV1,
    lir_foundation: DecodedLirFoundation,
    strong_production: DecodedStrongProductionSectionV1,
    link_identity_closure: DecodedLinkIdentityClosureSectionV1,
    production_manifest: DecodedSingleConeProductionManifestV1,
}

/// Link sections whose complete HIR-to-LIR foundation identity graph passed
/// one transaction. Foundation structure and the strong profile's ODR policy
/// remain unproven.
pub struct IdentityCheckedSingleConeLinkSections<'input> {
    graph: ValidatedGraphArtifact<'input>,
    identities: ValidatedIdentityGraph,
    hir_foundation: DecodedHirFoundation,
    hir_production: DecodedCoreBootstrapInterfaceSectionV1,
    mir_foundation: DecodedMirFoundation,
    mir_production: DecodedCoreBootstrapBridgeSectionV1,
    lir_foundation: DecodedLirFoundation,
    strong_production: DecodedStrongProductionSectionV1,
    link_identity_closure: DecodedLinkIdentityClosureSectionV1,
    production_manifest: DecodedSingleConeProductionManifestV1,
}

/// Link sections backed by structurally valid foundations that satisfy the
/// `SingleConeStrongProfile` `RejectAll` ODR policy.
pub struct OdrCheckedSingleConeLinkFoundations<'input> {
    graph: ValidatedGraphArtifact<'input>,
    identities: ValidatedIdentityGraph,
    foundations: OdrFreeStrongFoundationSet,
    production: DecodedStrongProfileProductionSet,
    link_identity_closure: DecodedLinkIdentityClosureSectionV1,
    production_manifest: DecodedSingleConeProductionManifestV1,
}

/// Link sections whose HIR, MIR, and LIR production surfaces were rebuilt
/// from the same ODR-free foundations. Object, closure, and manifest proofs
/// remain separate Link obligations.
pub struct ProductionValidatedSingleConeLinkSections<'input> {
    graph: ValidatedGraphArtifact<'input>,
    identities: ValidatedIdentityGraph,
    foundations: OdrFreeStrongFoundationSet,
    production: ValidatedSingleConeStrongProduction,
    link_identity_closure: DecodedLinkIdentityClosureSectionV1,
    production_manifest: DecodedSingleConeProductionManifestV1,
}

/// Link sections whose decoded materializations and archive directory were
/// proven to describe the same complete built-in object member plan. Object
/// bytes remain unverified candidates.
pub struct MaterializationCheckedSingleConeLinkSections<'input> {
    graph: ValidatedGraphArtifact<'input>,
    identities: ValidatedIdentityGraph,
    foundations: OdrFreeStrongFoundationSet,
    production: ValidatedSingleConeStrongProduction,
    link_identity_closure: MaterializationCheckedLinkIdentityClosureSectionV1,
    scoop_objects: Vec<ScoopLirObjectCandidateV1<'input>>,
    generated_bridge_objects: Vec<GeneratedCBridgeObjectCandidateV1<'input>>,
    production_manifest: DecodedSingleConeProductionManifestV1,
}

/// Link sections whose manifest C-bridge branch and every generated-C object
/// envelope were checked against the same strong bridge plan and request
/// toolchain profile. Scoop object semantics remain unverified.
pub struct CBridgeCheckedSingleConeLinkSections<'input> {
    graph: ValidatedGraphArtifact<'input>,
    identities: ValidatedIdentityGraph,
    foundations: OdrFreeStrongFoundationSet,
    production: ValidatedSingleConeStrongProduction,
    link_identity_closure: MaterializationCheckedLinkIdentityClosureSectionV1,
    scoop_objects: Vec<ScoopLirObjectCandidateV1<'input>>,
    generated_bridge_objects: Vec<GeneratedCBridgeObjectCandidateV1<'input>>,
    c_bridge_production: VerifiedCBridgeProductionEnvelopeSetV1,
    production_manifest: CBridgeCheckedSingleConeProductionManifestV1,
}

/// Every built-in object member through exact symbol, atom-range,
/// relocation, and current-Cone strong-target closure validation. Digest,
/// registration, image, entry, and final fingerprint proofs remain pending.
pub struct BuiltinObjectCheckedSingleConeLinkSections<'input> {
    graph: ValidatedGraphArtifact<'input>,
    identities: ValidatedIdentityGraph,
    foundations: OdrFreeStrongFoundationSet,
    production: ValidatedSingleConeStrongProduction,
    link_identity_closure: crate::DigestPatchInputCheckedLinkIdentityClosureSectionV1,
    scoop_objects: VerifiedNormalizedProvisionalScoopLirObjectSetV1,
    builtin_objects: VerifiedBuiltinObjectStrongRelocationSetV1,
    production_manifest: CBridgeCheckedSingleConeProductionManifestV1,
}

/// Link sections whose decoded digest materializations were bound to the
/// validated plan and proven against the exact Scoop object bytes. Later
/// registration, image, entry, and final fingerprint proofs remain pending.
pub struct DigestPatchCheckedSingleConeLinkSections<'input> {
    graph: ValidatedGraphArtifact<'input>,
    identities: ValidatedIdentityGraph,
    foundations: OdrFreeStrongFoundationSet,
    production: ValidatedSingleConeStrongProduction,
    link_identity_closure: ObjectProjectionCheckedLinkIdentityClosureSectionV1,
    scoop_objects: VerifiedNormalizedProvisionalScoopLirObjectSetV1,
    digest_patch_sites: VerifiedScoopLirDigestPatchSiteSetV1,
    production_manifest: CBridgeCheckedSingleConeProductionManifestV1,
}

/// Link sections whose normalized stackmaps and all six strong registration
/// object tables were proven against the complete LIR registration-production
/// surface and the exact Scoop object bytes. Image, entry, and final
/// fingerprint proofs remain pending.
pub struct RegistrationObjectCheckedSingleConeLinkSections<'input> {
    graph: ValidatedGraphArtifact<'input>,
    identities: ValidatedIdentityGraph,
    foundations: OdrFreeStrongFoundationSet,
    production: ValidatedSingleConeStrongProduction,
    link_identity_closure: ObjectProjectionCheckedLinkIdentityClosureSectionV1,
    scoop_objects: VerifiedNormalizedProvisionalScoopLirObjectSetV1,
    safepoint_registrations: VerifiedStrongSafepointRegistrationSetV1,
    callable_registrations: VerifiedStrongCallableRegistrationSetV1,
    type_registrations: VerifiedStrongTypeRegistrationSetV1,
    immortal_object_registrations: VerifiedStrongImmortalObjectRegistrationSetV1,
    static_storage_registrations: VerifiedStrongStaticStorageRegistrationSetV1,
    initialization_registrations: VerifiedStrongInitializationRegistrationSetV1,
    production_manifest: CBridgeCheckedSingleConeProductionManifestV1,
}

/// Link sections whose six registration-object leaves were canonically
/// fingerprinted from the verified provisional object bytes. Safepoints also
/// carry their final strong-registration fingerprints because their only
/// non-object input is the already normalized stackmap proof. Remaining
/// registration dependency leaves are still unproven.
pub struct RegistrationLeafFingerprintedSingleConeLinkSections<'input> {
    graph: ValidatedGraphArtifact<'input>,
    identities: ValidatedIdentityGraph,
    foundations: OdrFreeStrongFoundationSet,
    production: ValidatedSingleConeStrongProduction,
    link_identity_closure: ObjectProjectionCheckedLinkIdentityClosureSectionV1,
    scoop_objects: VerifiedNormalizedProvisionalScoopLirObjectSetV1,
    safepoints: VerifiedStrongSafepointFingerprintSetV1,
    callable_registration_objects: VerifiedStrongCallableRegistrationObjectFingerprintSetV1,
    type_registration_objects: VerifiedStrongTypeRegistrationObjectFingerprintSetV1,
    immortal_object_registration_objects:
        VerifiedStrongImmortalObjectRegistrationObjectFingerprintSetV1,
    static_storage_registration_objects:
        VerifiedStrongStaticStorageRegistrationObjectFingerprintSetV1,
    initialization_registration_objects:
        VerifiedStrongInitializationRegistrationObjectFingerprintSetV1,
    production_manifest: CBridgeCheckedSingleConeProductionManifestV1,
}

/// Link sections whose member-aware defined owners and every undefined use
/// were rebuilt from verified object relocations and the authoritative core,
/// source-native, runtime/EH, and generated-C requirement registries. The two
/// decoded closure tables have matched those rebuilt values exactly.
pub struct LinkSymbolCheckedSingleConeLinkSections<'input> {
    graph: ValidatedGraphArtifact<'input>,
    identities: ValidatedIdentityGraph,
    foundations: OdrFreeStrongFoundationSet,
    production: ValidatedSingleConeStrongProduction,
    link_identity_closure: SymbolProjectionCheckedLinkIdentityClosureSectionV1,
    scoop_objects: VerifiedNormalizedProvisionalScoopLirObjectSetV1,
    defined_symbols: CanonicalDefinedLinkSymbolOwnerSetV1,
    undefined_symbols: CanonicalUndefinedSymbolRequirementSetV1,
    safepoints: VerifiedStrongSafepointFingerprintSetV1,
    callable_registration_objects: VerifiedStrongCallableRegistrationObjectFingerprintSetV1,
    type_registration_objects: VerifiedStrongTypeRegistrationObjectFingerprintSetV1,
    immortal_object_registration_objects:
        VerifiedStrongImmortalObjectRegistrationObjectFingerprintSetV1,
    static_storage_registration_objects:
        VerifiedStrongStaticStorageRegistrationObjectFingerprintSetV1,
    initialization_registration_objects:
        VerifiedStrongInitializationRegistrationObjectFingerprintSetV1,
    production_manifest: CBridgeCheckedSingleConeProductionManifestV1,
}

/// Link sections whose complete registration dependencies and final strong
/// fingerprints have been computed. The six tables are ready for one atomic
/// patch transaction.
pub struct RegistrationDependencyFingerprintedSingleConeLinkSections<'input> {
    graph: ValidatedGraphArtifact<'input>,
    identities: ValidatedIdentityGraph,
    foundations: OdrFreeStrongFoundationSet,
    production: ValidatedSingleConeStrongProduction,
    link_identity_closure: SymbolProjectionCheckedLinkIdentityClosureSectionV1,
    scoop_objects: VerifiedNormalizedProvisionalScoopLirObjectSetV1,
    defined_symbols: CanonicalDefinedLinkSymbolOwnerSetV1,
    undefined_symbols: CanonicalUndefinedSymbolRequirementSetV1,
    safepoints: VerifiedStrongSafepointFingerprintSetV1,
    callables: VerifiedStrongCallableFingerprintSetV1,
    types: VerifiedStrongTypeFingerprintSetV1,
    immortal_objects: VerifiedStrongImmortalObjectFingerprintSetV1,
    static_storages: VerifiedStrongStaticStorageFingerprintSetV1,
    initializations: VerifiedStrongInitializationFingerprintSetV1,
    production_manifest: CBridgeCheckedSingleConeProductionManifestV1,
}

/// Link sections whose six strong-registration tables, runtime image, and
/// entry branch have been patched into copied object bytes and revalidated.
/// Code/member fingerprints and the final Link identity closure remain.
pub struct FinalizedStrongLinkObjectSections<'input> {
    graph: ValidatedGraphArtifact<'input>,
    identities: ValidatedIdentityGraph,
    foundations: OdrFreeStrongFoundationSet,
    production: ValidatedSingleConeStrongProduction,
    link_identity_closure: SymbolProjectionCheckedLinkIdentityClosureSectionV1,
    defined_symbols: CanonicalDefinedLinkSymbolOwnerSetV1,
    undefined_symbols: CanonicalUndefinedSymbolRequirementSetV1,
    final_objects: VerifiedEntryPatchSetV1,
    production_manifest: CBridgeCheckedSingleConeProductionManifestV1,
}

/// Fully validated Link view for one `SingleConeStrongProfile` artifact.
/// The owned production manifest retains the complete Code and final-object
/// proof; no earlier provisional state can be recovered from this value.
pub struct ValidatedSingleConeStrongLinkArtifact<'input> {
    graph: ValidatedGraphArtifact<'input>,
    identities: ValidatedIdentityGraph,
    foundations: OdrFreeStrongFoundationSet,
    production: ValidatedSingleConeStrongProduction,
    link_identity_closure: LinkIdentityClosureSectionV1,
    production_manifest: SingleConeProductionManifestV1,
}

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

impl<'input> ValidatedGraphArtifact<'input> {
    pub fn decode_single_cone_link_sections(
        mut self,
    ) -> Result<DecodedSingleConeLinkSections<'input>, SingleConeLinkSectionDecodeError> {
        let profile = require_strong_profile(&self)?;
        let production_manifest = decode_production_manifest(&mut self, profile)?;

        let hir_member_id = metadata_member_id(&self, MetadataLocation::Hir)?;
        let mir_member_id = metadata_member_id(&self, MetadataLocation::Mir)?;
        let lir_member_id = metadata_member_id(&self, MetadataLocation::Lir)?;
        let hir_payload = member_payload(&self, MetadataLocation::Hir, hir_member_id)?;
        let mir_payload = member_payload(&self, MetadataLocation::Mir, mir_member_id)?;
        let lir_payload = member_payload(&self, MetadataLocation::Lir, lir_member_id)?;

        let hir_envelope = decode_metadata_envelope(&mut self, hir_payload, MetadataLocation::Hir)?;
        let mir_envelope = decode_metadata_envelope(&mut self, mir_payload, MetadataLocation::Mir)?;
        let lir_envelope = decode_metadata_envelope(&mut self, lir_payload, MetadataLocation::Lir)?;
        profile
            .validate_link_metadata_inventory(MetadataLocation::Hir, hir_envelope.sections())
            .map_err(SingleConeLinkSectionDecodeError::Inventory)?;
        profile
            .validate_link_metadata_inventory(MetadataLocation::Mir, mir_envelope.sections())
            .map_err(SingleConeLinkSectionDecodeError::Inventory)?;
        profile
            .validate_link_metadata_inventory(MetadataLocation::Lir, lir_envelope.sections())
            .map_err(SingleConeLinkSectionDecodeError::Inventory)?;

        let hir_foundation_capability = hir_identity_foundation_capability();
        let hir_production_capability = hir_core_bootstrap_interface_capability();
        let mir_foundation_capability = mir_identity_foundation_capability();
        let mir_production_capability = mir_core_bootstrap_bridge_capability();
        let lir_foundation_capability = lir_identity_foundation_capability();
        let strong_production_capability = lir_strong_production_capability();
        let link_identity_closure_capability = lir_link_identity_closure_capability();
        let hir_foundation_payload =
            required_metadata_section(&hir_envelope, &hir_foundation_capability)?;
        let hir_production_payload =
            required_metadata_section(&hir_envelope, &hir_production_capability)?;
        let mir_foundation_payload =
            required_metadata_section(&mir_envelope, &mir_foundation_capability)?;
        let mir_production_payload =
            required_metadata_section(&mir_envelope, &mir_production_capability)?;
        let lir_foundation_payload =
            required_metadata_section(&lir_envelope, &lir_foundation_capability)?;
        let strong_payload =
            required_metadata_section(&lir_envelope, &strong_production_capability)?;
        let closure_payload =
            required_metadata_section(&lir_envelope, &link_identity_closure_capability)?;

        let hir_foundation = decode_inner(
            &mut self,
            MetadataLocation::Hir,
            hir_foundation_capability,
            hir_foundation_payload,
        )?;
        let hir_production = decode_inner(
            &mut self,
            MetadataLocation::Hir,
            hir_production_capability,
            hir_production_payload,
        )?;
        let mir_foundation = decode_inner(
            &mut self,
            MetadataLocation::Mir,
            mir_foundation_capability,
            mir_foundation_payload,
        )?;
        let mir_production = decode_inner(
            &mut self,
            MetadataLocation::Mir,
            mir_production_capability,
            mir_production_payload,
        )?;
        let lir_foundation = decode_inner(
            &mut self,
            MetadataLocation::Lir,
            lir_foundation_capability,
            lir_foundation_payload,
        )?;
        let strong_production = decode_inner(
            &mut self,
            MetadataLocation::Lir,
            strong_production_capability,
            strong_payload,
        )?;
        let link_identity_closure = decode_inner(
            &mut self,
            MetadataLocation::Lir,
            link_identity_closure_capability,
            closure_payload,
        )?;
        validate_semantic_fingerprints(&mut self, &hir_envelope, &mir_envelope, &lir_envelope)?;

        Ok(DecodedSingleConeLinkSections {
            graph: self,
            hir_foundation,
            hir_production,
            mir_foundation,
            mir_production,
            lir_foundation,
            strong_production,
            link_identity_closure,
            production_manifest,
        })
    }
}

impl<'input> DecodedSingleConeLinkSections<'input> {
    pub const fn coordinate(&self) -> &ConeCoordinate {
        self.graph.coordinate()
    }

    pub const fn identity(&self) -> ConeIdentity {
        self.graph.identity()
    }

    pub const fn artifact_fingerprint(&self) -> ArtifactFingerprint {
        self.graph.artifact_fingerprint()
    }

    pub const fn decode_usage(&self) -> DecodeUsage {
        self.graph.decode_usage()
    }

    pub const fn hir_foundation_wire(&self) -> &DecodedHirFoundation {
        &self.hir_foundation
    }

    pub const fn mir_foundation_wire(&self) -> &DecodedMirFoundation {
        &self.mir_foundation
    }

    pub const fn hir_production_wire(&self) -> &DecodedCoreBootstrapInterfaceSectionV1 {
        &self.hir_production
    }

    pub const fn mir_production_wire(&self) -> &DecodedCoreBootstrapBridgeSectionV1 {
        &self.mir_production
    }

    pub const fn lir_foundation_wire(&self) -> &DecodedLirFoundation {
        &self.lir_foundation
    }

    pub const fn strong_production_wire(&self) -> &DecodedStrongProductionSectionV1 {
        &self.strong_production
    }

    pub const fn link_identity_closure_wire(&self) -> &DecodedLinkIdentityClosureSectionV1 {
        &self.link_identity_closure
    }

    pub const fn production_manifest_wire(&self) -> &DecodedSingleConeProductionManifestV1 {
        &self.production_manifest
    }

    /// Registers and resolves every foundation identity before any Link
    /// payload can use a persistent identity as trusted input.
    pub fn validate_identities(
        self,
    ) -> Result<IdentityCheckedSingleConeLinkSections<'input>, IdentityValidationError> {
        let Self {
            mut graph,
            hir_foundation,
            hir_production,
            mir_foundation,
            mir_production,
            lir_foundation,
            strong_production,
            link_identity_closure,
            production_manifest,
        } = self;
        let identities = validate_foundation_identity_graph(
            &mut graph,
            &hir_foundation,
            &mir_foundation,
            &lir_foundation,
        )?;
        Ok(IdentityCheckedSingleConeLinkSections {
            graph,
            identities,
            hir_foundation,
            hir_production,
            mir_foundation,
            mir_production,
            lir_foundation,
            strong_production,
            link_identity_closure,
            production_manifest,
        })
    }
}

impl<'input> IdentityCheckedSingleConeLinkSections<'input> {
    pub const fn coordinate(&self) -> &ConeCoordinate {
        self.graph.coordinate()
    }

    pub const fn identity(&self) -> ConeIdentity {
        self.graph.identity()
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

    pub const fn strong_production_wire(&self) -> &DecodedStrongProductionSectionV1 {
        &self.strong_production
    }

    pub const fn hir_production_wire(&self) -> &DecodedCoreBootstrapInterfaceSectionV1 {
        &self.hir_production
    }

    pub const fn mir_production_wire(&self) -> &DecodedCoreBootstrapBridgeSectionV1 {
        &self.mir_production
    }

    pub const fn link_identity_closure_wire(&self) -> &DecodedLinkIdentityClosureSectionV1 {
        &self.link_identity_closure
    }

    pub const fn production_manifest_wire(&self) -> &DecodedSingleConeProductionManifestV1 {
        &self.production_manifest
    }

    pub fn validate_foundation_structure(
        self,
    ) -> Result<OdrCheckedSingleConeLinkFoundations<'input>, StrongProfileFoundationError> {
        let Self {
            mut graph,
            mut identities,
            hir_foundation,
            hir_production,
            mir_foundation,
            mir_production,
            lir_foundation,
            strong_production,
            link_identity_closure,
            production_manifest,
        } = self;
        let foundations = validate_strong_profile_foundations(
            &mut graph,
            &mut identities,
            hir_foundation,
            mir_foundation,
            lir_foundation,
        )?;
        Ok(OdrCheckedSingleConeLinkFoundations {
            graph,
            identities,
            foundations,
            production: DecodedStrongProfileProductionSet {
                hir: hir_production,
                mir: mir_production,
                lir: strong_production,
            },
            link_identity_closure,
            production_manifest,
        })
    }
}

impl<'input> OdrCheckedSingleConeLinkFoundations<'input> {
    pub const fn coordinate(&self) -> &ConeCoordinate {
        self.graph.coordinate()
    }

    pub const fn identity(&self) -> ConeIdentity {
        self.graph.identity()
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

    pub const fn hir_foundation(&self) -> &OdrFreeHirFoundation {
        &self.foundations.hir
    }

    pub const fn mir_foundation(&self) -> &OdrFreeMirFoundation {
        &self.foundations.mir
    }

    pub const fn lir_foundation(&self) -> &OdrFreeLirFoundation {
        &self.foundations.lir
    }

    pub const fn strong_production_wire(&self) -> &DecodedStrongProductionSectionV1 {
        &self.production.lir
    }

    pub const fn hir_production_wire(&self) -> &DecodedCoreBootstrapInterfaceSectionV1 {
        &self.production.hir
    }

    pub const fn mir_production_wire(&self) -> &DecodedCoreBootstrapBridgeSectionV1 {
        &self.production.mir
    }

    pub const fn link_identity_closure_wire(&self) -> &DecodedLinkIdentityClosureSectionV1 {
        &self.link_identity_closure
    }

    pub const fn production_manifest_wire(&self) -> &DecodedSingleConeProductionManifestV1 {
        &self.production_manifest
    }

    pub fn validate_production(
        self,
        expected_external_bridges: &StrongExternalLirBridgeSurfaceV1,
    ) -> Result<ProductionValidatedSingleConeLinkSections<'input>, StrongProfileProductionError>
    {
        let Self {
            graph,
            mut identities,
            foundations,
            production,
            link_identity_closure,
            production_manifest,
        } = self;
        let production = validate_strong_profile_production(
            &graph,
            &mut identities,
            &foundations,
            production,
            expected_external_bridges,
        )?;
        Ok(ProductionValidatedSingleConeLinkSections {
            graph,
            identities,
            foundations,
            production,
            link_identity_closure,
            production_manifest,
        })
    }
}

impl<'input> ProductionValidatedSingleConeLinkSections<'input> {
    pub const fn coordinate(&self) -> &ConeCoordinate {
        self.graph.coordinate()
    }

    pub const fn identity(&self) -> ConeIdentity {
        self.graph.identity()
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

    pub const fn hir_foundation(&self) -> &OdrFreeHirFoundation {
        &self.foundations.hir
    }

    pub const fn mir_foundation(&self) -> &OdrFreeMirFoundation {
        &self.foundations.mir
    }

    pub const fn lir_foundation(&self) -> &OdrFreeLirFoundation {
        &self.foundations.lir
    }

    pub const fn production(&self) -> &ValidatedSingleConeStrongProduction {
        &self.production
    }

    pub const fn link_identity_closure_wire(&self) -> &DecodedLinkIdentityClosureSectionV1 {
        &self.link_identity_closure
    }

    pub const fn production_manifest_wire(&self) -> &DecodedSingleConeProductionManifestV1 {
        &self.production_manifest
    }

    pub fn validate_materializations(
        self,
    ) -> Result<MaterializationCheckedSingleConeLinkSections<'input>, StrongLinkMaterializationError>
    {
        let Self {
            graph,
            identities,
            foundations,
            production,
            link_identity_closure,
            production_manifest,
        } = self;
        let partition = StrongProducerUnitPartitionV1::from_odr_free_foundation(&foundations.lir)
            .map_err(StrongLinkMaterializationError::ProducerUnits)?;
        let link_identity_closure = link_identity_closure
            .validate_materializations(&partition)
            .map_err(StrongLinkMaterializationError::Closure)?;
        let (scoop_objects, generated_bridge_objects) =
            validate_object_directory(&graph, link_identity_closure.member_plan())?;
        Ok(MaterializationCheckedSingleConeLinkSections {
            graph,
            identities,
            foundations,
            production,
            link_identity_closure,
            scoop_objects,
            generated_bridge_objects,
            production_manifest,
        })
    }
}

impl<'input> MaterializationCheckedSingleConeLinkSections<'input> {
    pub const fn coordinate(&self) -> &ConeCoordinate {
        self.graph.coordinate()
    }

    pub const fn identity(&self) -> ConeIdentity {
        self.graph.identity()
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

    pub const fn hir_foundation(&self) -> &OdrFreeHirFoundation {
        &self.foundations.hir
    }

    pub const fn mir_foundation(&self) -> &OdrFreeMirFoundation {
        &self.foundations.mir
    }

    pub const fn lir_foundation(&self) -> &OdrFreeLirFoundation {
        &self.foundations.lir
    }

    pub const fn production(&self) -> &ValidatedSingleConeStrongProduction {
        &self.production
    }

    pub const fn materializations(&self) -> &MaterializationCheckedLinkIdentityClosureSectionV1 {
        &self.link_identity_closure
    }

    pub fn scoop_objects(&self) -> &[ScoopLirObjectCandidateV1<'_>] {
        &self.scoop_objects
    }

    pub fn generated_bridge_objects(&self) -> &[GeneratedCBridgeObjectCandidateV1<'_>] {
        &self.generated_bridge_objects
    }

    pub const fn production_manifest_wire(&self) -> &DecodedSingleConeProductionManifestV1 {
        &self.production_manifest
    }

    pub fn validate_c_bridge_envelopes(
        self,
        profile: &CBridgeToolchainProfileV1,
    ) -> Result<CBridgeCheckedSingleConeLinkSections<'input>, StrongLinkCBridgeError> {
        let Self {
            graph,
            identities,
            foundations,
            production,
            link_identity_closure,
            scoop_objects,
            generated_bridge_objects,
            production_manifest,
        } = self;
        let bridge_plan = production.lir().generated_bridge_plan();
        let production_manifest = production_manifest
            .validate_c_bridge_production(bridge_plan, profile)
            .map_err(StrongLinkCBridgeError::ManifestProduction)?;
        let c_bridge_production = crate::verify_c_bridge_production_envelopes_v1(
            bridge_plan.clone(),
            production_manifest.c_bridge_production().clone(),
            profile,
            link_identity_closure.member_plan(),
            &generated_bridge_objects,
        )
        .map_err(StrongLinkCBridgeError::ObjectEnvelopes)?;
        Ok(CBridgeCheckedSingleConeLinkSections {
            graph,
            identities,
            foundations,
            production,
            link_identity_closure,
            scoop_objects,
            generated_bridge_objects,
            c_bridge_production,
            production_manifest,
        })
    }
}

impl<'input> CBridgeCheckedSingleConeLinkSections<'input> {
    pub const fn coordinate(&self) -> &ConeCoordinate {
        self.graph.coordinate()
    }

    pub const fn identity(&self) -> ConeIdentity {
        self.graph.identity()
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

    pub const fn hir_foundation(&self) -> &OdrFreeHirFoundation {
        &self.foundations.hir
    }

    pub const fn mir_foundation(&self) -> &OdrFreeMirFoundation {
        &self.foundations.mir
    }

    pub const fn lir_foundation(&self) -> &OdrFreeLirFoundation {
        &self.foundations.lir
    }

    pub const fn production(&self) -> &ValidatedSingleConeStrongProduction {
        &self.production
    }

    pub const fn materializations(&self) -> &MaterializationCheckedLinkIdentityClosureSectionV1 {
        &self.link_identity_closure
    }

    pub fn scoop_objects(&self) -> &[ScoopLirObjectCandidateV1<'_>] {
        &self.scoop_objects
    }

    pub fn generated_bridge_objects(&self) -> &[GeneratedCBridgeObjectCandidateV1<'_>] {
        &self.generated_bridge_objects
    }

    pub const fn c_bridge_production(&self) -> &VerifiedCBridgeProductionEnvelopeSetV1 {
        &self.c_bridge_production
    }

    pub const fn production_manifest(&self) -> &CBridgeCheckedSingleConeProductionManifestV1 {
        &self.production_manifest
    }

    pub fn validate_builtin_objects(
        self,
    ) -> Result<BuiltinObjectCheckedSingleConeLinkSections<'input>, StrongLinkBuiltinObjectError>
    {
        let Self {
            graph,
            identities,
            foundations,
            production,
            link_identity_closure,
            scoop_objects: final_scoop_objects,
            generated_bridge_objects,
            c_bridge_production,
            production_manifest,
        } = self;
        let link_identity_closure = link_identity_closure
            .validate_digest_patch_inputs(production.lir().digest_finalization_plan())
            .map_err(StrongLinkBuiltinObjectError::ClosureInput)?;
        let scoop_objects = crate::normalize_final_scoop_lir_objects_v1(
            link_identity_closure.member_plan(),
            &final_scoop_objects,
            link_identity_closure.provisional_patch_sites(),
        )
        .map_err(StrongLinkBuiltinObjectError::Normalization)?;
        let scoop_candidates = scoop_objects.candidates();
        let symbol_plan = PlannedStrongObjectSymbolSetV1::new(
            graph.target_selection().target(),
            production.lir().canonical_definitions(),
            link_identity_closure.member_plan(),
        )
        .map_err(StrongLinkBuiltinObjectError::SymbolPlan)?;
        let builtin_objects = crate::verify_builtin_object_strong_relocations_v1(
            link_identity_closure.member_plan(),
            &symbol_plan,
            &scoop_candidates,
            c_bridge_production,
            &generated_bridge_objects,
        )
        .map_err(StrongLinkBuiltinObjectError::ObjectSet)?;
        Ok(BuiltinObjectCheckedSingleConeLinkSections {
            graph,
            identities,
            foundations,
            production,
            link_identity_closure,
            scoop_objects,
            builtin_objects,
            production_manifest,
        })
    }
}

impl<'input> BuiltinObjectCheckedSingleConeLinkSections<'input> {
    pub const fn coordinate(&self) -> &ConeCoordinate {
        self.graph.coordinate()
    }

    pub const fn identity(&self) -> ConeIdentity {
        self.graph.identity()
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

    pub const fn hir_foundation(&self) -> &OdrFreeHirFoundation {
        &self.foundations.hir
    }

    pub const fn mir_foundation(&self) -> &OdrFreeMirFoundation {
        &self.foundations.mir
    }

    pub const fn lir_foundation(&self) -> &OdrFreeLirFoundation {
        &self.foundations.lir
    }

    pub const fn production(&self) -> &ValidatedSingleConeStrongProduction {
        &self.production
    }

    pub const fn digest_patch_inputs(
        &self,
    ) -> &crate::DigestPatchInputCheckedLinkIdentityClosureSectionV1 {
        &self.link_identity_closure
    }

    pub const fn scoop_objects(&self) -> &VerifiedNormalizedProvisionalScoopLirObjectSetV1 {
        &self.scoop_objects
    }

    pub const fn builtin_objects(&self) -> &VerifiedBuiltinObjectStrongRelocationSetV1 {
        &self.builtin_objects
    }

    pub const fn production_manifest(&self) -> &CBridgeCheckedSingleConeProductionManifestV1 {
        &self.production_manifest
    }

    pub fn validate_digest_patch_sites(
        self,
    ) -> Result<DigestPatchCheckedSingleConeLinkSections<'input>, StrongLinkDigestPatchError> {
        let Self {
            graph,
            identities,
            foundations,
            production,
            link_identity_closure,
            scoop_objects,
            builtin_objects,
            production_manifest,
        } = self;
        let digest_plan = production.lir().digest_finalization_plan();
        let scoop_candidates = scoop_objects.candidates();
        let digest_patch_sites = crate::verify_scoop_lir_digest_patch_sites_v1(
            builtin_objects,
            &foundations.lir,
            digest_plan.clone(),
            &scoop_candidates,
            link_identity_closure.provisional_patch_sites(),
        )
        .map_err(StrongLinkDigestPatchError::ObjectSites)?;
        let link_identity_closure = link_identity_closure
            .validate_object_projections(&digest_patch_sites)
            .map_err(StrongLinkDigestPatchError::ClosureProjection)?;
        Ok(DigestPatchCheckedSingleConeLinkSections {
            graph,
            identities,
            foundations,
            production,
            link_identity_closure,
            scoop_objects,
            digest_patch_sites,
            production_manifest,
        })
    }
}

impl<'input> DigestPatchCheckedSingleConeLinkSections<'input> {
    pub const fn coordinate(&self) -> &ConeCoordinate {
        self.graph.coordinate()
    }

    pub const fn identity(&self) -> ConeIdentity {
        self.graph.identity()
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

    pub const fn hir_foundation(&self) -> &OdrFreeHirFoundation {
        &self.foundations.hir
    }

    pub const fn mir_foundation(&self) -> &OdrFreeMirFoundation {
        &self.foundations.mir
    }

    pub const fn lir_foundation(&self) -> &OdrFreeLirFoundation {
        &self.foundations.lir
    }

    pub const fn production(&self) -> &ValidatedSingleConeStrongProduction {
        &self.production
    }

    pub const fn object_projections(&self) -> &ObjectProjectionCheckedLinkIdentityClosureSectionV1 {
        &self.link_identity_closure
    }

    pub const fn scoop_objects(&self) -> &VerifiedNormalizedProvisionalScoopLirObjectSetV1 {
        &self.scoop_objects
    }

    pub const fn digest_patch_sites(&self) -> &VerifiedScoopLirDigestPatchSiteSetV1 {
        &self.digest_patch_sites
    }

    pub const fn production_manifest(&self) -> &CBridgeCheckedSingleConeProductionManifestV1 {
        &self.production_manifest
    }

    pub fn validate_registration_objects(
        self,
    ) -> Result<
        RegistrationObjectCheckedSingleConeLinkSections<'input>,
        StrongLinkRegistrationObjectError,
    > {
        let Self {
            graph,
            identities,
            foundations,
            production,
            link_identity_closure,
            scoop_objects,
            digest_patch_sites,
            production_manifest,
        } = self;
        let registration_production = production.lir().registration_production();
        let safepoint_semantics = registration_production.safepoint_semantics();
        let safepoint_plan = registration_production.safepoints().clone();
        let callable_plan = registration_production.callables().clone();
        let type_plan = registration_production.types().clone();
        let immortal_object_plan = registration_production.immortal_objects().clone();
        let static_storage_plan = registration_production.static_storages().clone();
        let initialization_plan = registration_production.initialization_units().clone();
        let scoop_candidates = scoop_objects.candidates();

        let stackmaps = crate::verify_scoop_lir_stackmaps_v1(
            digest_patch_sites.builtins().clone(),
            safepoint_semantics,
            &scoop_candidates,
        )
        .map_err(StrongLinkRegistrationObjectError::Stackmaps)?;
        let safepoint_registrations = crate::verify_strong_safepoint_registrations_v1(
            stackmaps,
            digest_patch_sites.clone(),
            safepoint_plan,
            &scoop_candidates,
        )
        .map_err(StrongLinkRegistrationObjectError::Safepoints)?;
        let callable_registrations = crate::verify_strong_callable_registrations_v1(
            digest_patch_sites.clone(),
            callable_plan,
            &scoop_candidates,
        )
        .map_err(StrongLinkRegistrationObjectError::Callables)?;
        let type_registrations = crate::verify_strong_type_registrations_v1(
            digest_patch_sites.clone(),
            type_plan,
            &scoop_candidates,
        )
        .map_err(StrongLinkRegistrationObjectError::Types)?;
        let immortal_object_registrations = crate::verify_strong_immortal_object_registrations_v1(
            digest_patch_sites.clone(),
            immortal_object_plan,
            &scoop_candidates,
        )
        .map_err(StrongLinkRegistrationObjectError::ImmortalObjects)?;
        let static_storage_registrations = crate::verify_strong_static_storage_registrations_v1(
            digest_patch_sites.clone(),
            static_storage_plan,
            &scoop_candidates,
        )
        .map_err(StrongLinkRegistrationObjectError::StaticStorages)?;
        let initialization_registrations = crate::verify_strong_initialization_registrations_v1(
            digest_patch_sites,
            initialization_plan,
            &scoop_candidates,
        )
        .map_err(StrongLinkRegistrationObjectError::InitializationUnits)?;

        Ok(RegistrationObjectCheckedSingleConeLinkSections {
            graph,
            identities,
            foundations,
            production,
            link_identity_closure,
            scoop_objects,
            safepoint_registrations,
            callable_registrations,
            type_registrations,
            immortal_object_registrations,
            static_storage_registrations,
            initialization_registrations,
            production_manifest,
        })
    }
}

impl<'input> RegistrationObjectCheckedSingleConeLinkSections<'input> {
    pub const fn coordinate(&self) -> &ConeCoordinate {
        self.graph.coordinate()
    }

    pub const fn identity(&self) -> ConeIdentity {
        self.graph.identity()
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

    pub const fn hir_foundation(&self) -> &OdrFreeHirFoundation {
        &self.foundations.hir
    }

    pub const fn mir_foundation(&self) -> &OdrFreeMirFoundation {
        &self.foundations.mir
    }

    pub const fn lir_foundation(&self) -> &OdrFreeLirFoundation {
        &self.foundations.lir
    }

    pub const fn production(&self) -> &ValidatedSingleConeStrongProduction {
        &self.production
    }

    pub const fn object_projections(&self) -> &ObjectProjectionCheckedLinkIdentityClosureSectionV1 {
        &self.link_identity_closure
    }

    pub const fn scoop_objects(&self) -> &VerifiedNormalizedProvisionalScoopLirObjectSetV1 {
        &self.scoop_objects
    }

    pub const fn stackmaps(&self) -> &VerifiedScoopLirStackmapSetV1 {
        self.safepoint_registrations.stackmaps()
    }

    pub const fn safepoint_registrations(&self) -> &VerifiedStrongSafepointRegistrationSetV1 {
        &self.safepoint_registrations
    }

    pub const fn callable_registrations(&self) -> &VerifiedStrongCallableRegistrationSetV1 {
        &self.callable_registrations
    }

    pub const fn type_registrations(&self) -> &VerifiedStrongTypeRegistrationSetV1 {
        &self.type_registrations
    }

    pub const fn immortal_object_registrations(
        &self,
    ) -> &VerifiedStrongImmortalObjectRegistrationSetV1 {
        &self.immortal_object_registrations
    }

    pub const fn static_storage_registrations(
        &self,
    ) -> &VerifiedStrongStaticStorageRegistrationSetV1 {
        &self.static_storage_registrations
    }

    pub const fn initialization_registrations(
        &self,
    ) -> &VerifiedStrongInitializationRegistrationSetV1 {
        &self.initialization_registrations
    }

    pub const fn production_manifest(&self) -> &CBridgeCheckedSingleConeProductionManifestV1 {
        &self.production_manifest
    }

    pub fn fingerprint_registration_leaves(
        self,
    ) -> Result<
        RegistrationLeafFingerprintedSingleConeLinkSections<'input>,
        StrongLinkRegistrationLeafFingerprintError,
    > {
        let Self {
            graph,
            identities,
            foundations,
            production,
            link_identity_closure,
            scoop_objects,
            safepoint_registrations,
            callable_registrations,
            type_registrations,
            immortal_object_registrations,
            static_storage_registrations,
            initialization_registrations,
            production_manifest,
        } = self;
        let scoop_candidates = scoop_objects.candidates();
        let safepoints = crate::compute_strong_safepoint_fingerprints_v1(
            safepoint_registrations,
            &scoop_candidates,
        )
        .map_err(StrongLinkRegistrationLeafFingerprintError::Safepoints)?;
        let callable_registration_objects =
            crate::compute_strong_callable_registration_object_fingerprints_v1(
                callable_registrations,
                &scoop_candidates,
            )
            .map_err(StrongLinkRegistrationLeafFingerprintError::Callables)?;
        let type_registration_objects =
            crate::compute_strong_type_registration_object_fingerprints_v1(
                type_registrations,
                &scoop_candidates,
            )
            .map_err(StrongLinkRegistrationLeafFingerprintError::Types)?;
        let immortal_object_registration_objects =
            crate::compute_strong_immortal_object_registration_object_fingerprints_v1(
                immortal_object_registrations,
                &scoop_candidates,
            )
            .map_err(StrongLinkRegistrationLeafFingerprintError::ImmortalObjects)?;
        let static_storage_registration_objects =
            crate::compute_strong_static_storage_registration_object_fingerprints_v1(
                static_storage_registrations,
                &scoop_candidates,
            )
            .map_err(StrongLinkRegistrationLeafFingerprintError::StaticStorages)?;
        let initialization_registration_objects =
            crate::compute_strong_initialization_registration_object_fingerprints_v1(
                initialization_registrations,
                &scoop_candidates,
            )
            .map_err(StrongLinkRegistrationLeafFingerprintError::InitializationUnits)?;

        Ok(RegistrationLeafFingerprintedSingleConeLinkSections {
            graph,
            identities,
            foundations,
            production,
            link_identity_closure,
            scoop_objects,
            safepoints,
            callable_registration_objects,
            type_registration_objects,
            immortal_object_registration_objects,
            static_storage_registration_objects,
            initialization_registration_objects,
            production_manifest,
        })
    }
}

impl<'input> RegistrationLeafFingerprintedSingleConeLinkSections<'input> {
    pub const fn coordinate(&self) -> &ConeCoordinate {
        self.graph.coordinate()
    }

    pub const fn identity(&self) -> ConeIdentity {
        self.graph.identity()
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

    pub const fn hir_foundation(&self) -> &OdrFreeHirFoundation {
        &self.foundations.hir
    }

    pub const fn mir_foundation(&self) -> &OdrFreeMirFoundation {
        &self.foundations.mir
    }

    pub const fn lir_foundation(&self) -> &OdrFreeLirFoundation {
        &self.foundations.lir
    }

    pub const fn production(&self) -> &ValidatedSingleConeStrongProduction {
        &self.production
    }

    pub const fn object_projections(&self) -> &ObjectProjectionCheckedLinkIdentityClosureSectionV1 {
        &self.link_identity_closure
    }

    pub const fn scoop_objects(&self) -> &VerifiedNormalizedProvisionalScoopLirObjectSetV1 {
        &self.scoop_objects
    }

    pub const fn safepoints(&self) -> &VerifiedStrongSafepointFingerprintSetV1 {
        &self.safepoints
    }

    pub const fn callable_registration_objects(
        &self,
    ) -> &VerifiedStrongCallableRegistrationObjectFingerprintSetV1 {
        &self.callable_registration_objects
    }

    pub const fn type_registration_objects(
        &self,
    ) -> &VerifiedStrongTypeRegistrationObjectFingerprintSetV1 {
        &self.type_registration_objects
    }

    pub const fn immortal_object_registration_objects(
        &self,
    ) -> &VerifiedStrongImmortalObjectRegistrationObjectFingerprintSetV1 {
        &self.immortal_object_registration_objects
    }

    pub const fn static_storage_registration_objects(
        &self,
    ) -> &VerifiedStrongStaticStorageRegistrationObjectFingerprintSetV1 {
        &self.static_storage_registration_objects
    }

    pub const fn initialization_registration_objects(
        &self,
    ) -> &VerifiedStrongInitializationRegistrationObjectFingerprintSetV1 {
        &self.initialization_registration_objects
    }

    pub const fn production_manifest(&self) -> &CBridgeCheckedSingleConeProductionManifestV1 {
        &self.production_manifest
    }

    pub fn validate_link_symbol_requirements(
        self,
        core_owners: &CanonicalDefinedLinkSymbolOwnerSetV1,
        c_bridge_profile: &CBridgeToolchainProfileV1,
    ) -> Result<LinkSymbolCheckedSingleConeLinkSections<'input>, StrongLinkSymbolRequirementError>
    {
        let Self {
            graph,
            identities,
            foundations,
            production,
            link_identity_closure,
            scoop_objects,
            safepoints,
            callable_registration_objects,
            type_registration_objects,
            immortal_object_registration_objects,
            static_storage_registration_objects,
            initialization_registration_objects,
            production_manifest,
        } = self;
        let selection = graph.target_selection();
        let patch_sites = callable_registration_objects
            .registrations()
            .patch_sites()
            .clone();
        let strong_closure = patch_sites.builtins().strong_relocations().clone();
        let defined_symbols =
            CanonicalDefinedLinkSymbolOwnerSetV1::from_verified_strong_closure(&strong_closure)
                .map_err(StrongLinkSymbolRequirementError::DefinedSymbols)?;
        let bridge_plan = production.lir().generated_bridge_plan().clone();
        let current_cone = crate::verify_current_cone_undefined_requirements_v1(
            strong_closure.clone(),
            bridge_plan.clone(),
        )
        .map_err(StrongLinkSymbolRequirementError::CurrentCone)?;
        let native_requirements = CanonicalNativeExternalRequirementSurfaceV1::from_foundation(
            selection.target(),
            &foundations.lir,
        )
        .map_err(StrongLinkSymbolRequirementError::NativeSurface)?;
        let core = crate::verify_core_strong_requirements_v1(
            selection.target(),
            strong_closure,
            production.lir().external_bridges().clone(),
            core_owners.clone(),
        )
        .map_err(StrongLinkSymbolRequirementError::Core)?;
        let source =
            crate::verify_source_external_requirements_v1(core, native_requirements.clone())
                .map_err(StrongLinkSymbolRequirementError::SourceExternal)?;
        let runtime_and_eh = crate::verify_runtime_and_eh_requirements_v1(source, selection)
            .map_err(StrongLinkSymbolRequirementError::RuntimeAndEh)?;
        let bridge_semantics = crate::verify_generated_c_bridge_semantics_v1(
            patch_sites,
            bridge_plan,
            native_requirements,
            c_bridge_profile,
        )
        .map_err(StrongLinkSymbolRequirementError::GeneratedBridgeSemantics)?;
        let external =
            crate::verify_c_bridge_target_support_requirements_v1(runtime_and_eh, bridge_semantics)
                .map_err(StrongLinkSymbolRequirementError::CBridgeTargetSupport)?;
        let external = crate::seal_builtin_object_external_requirements_v1(external)
            .map_err(StrongLinkSymbolRequirementError::UnclassifiedExternal)?;
        let undefined_symbols =
            crate::finalize_undefined_symbol_requirements_v1(current_cone, external)
                .map_err(StrongLinkSymbolRequirementError::UndefinedSymbols)?;
        let link_identity_closure = link_identity_closure
            .validate_symbol_projections(&defined_symbols, &undefined_symbols)
            .map_err(StrongLinkSymbolRequirementError::ClosureProjection)?;

        Ok(LinkSymbolCheckedSingleConeLinkSections {
            graph,
            identities,
            foundations,
            production,
            link_identity_closure,
            scoop_objects,
            defined_symbols,
            undefined_symbols,
            safepoints,
            callable_registration_objects,
            type_registration_objects,
            immortal_object_registration_objects,
            static_storage_registration_objects,
            initialization_registration_objects,
            production_manifest,
        })
    }
}

impl<'input> LinkSymbolCheckedSingleConeLinkSections<'input> {
    pub const fn coordinate(&self) -> &ConeCoordinate {
        self.graph.coordinate()
    }

    pub const fn identity(&self) -> ConeIdentity {
        self.graph.identity()
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

    pub const fn hir_foundation(&self) -> &OdrFreeHirFoundation {
        &self.foundations.hir
    }

    pub const fn mir_foundation(&self) -> &OdrFreeMirFoundation {
        &self.foundations.mir
    }

    pub const fn lir_foundation(&self) -> &OdrFreeLirFoundation {
        &self.foundations.lir
    }

    pub const fn production(&self) -> &ValidatedSingleConeStrongProduction {
        &self.production
    }

    pub const fn symbol_projections(&self) -> &SymbolProjectionCheckedLinkIdentityClosureSectionV1 {
        &self.link_identity_closure
    }

    pub const fn scoop_objects(&self) -> &VerifiedNormalizedProvisionalScoopLirObjectSetV1 {
        &self.scoop_objects
    }

    pub const fn defined_symbols(&self) -> &CanonicalDefinedLinkSymbolOwnerSetV1 {
        &self.defined_symbols
    }

    pub const fn undefined_symbols(&self) -> &CanonicalUndefinedSymbolRequirementSetV1 {
        &self.undefined_symbols
    }

    pub const fn safepoints(&self) -> &VerifiedStrongSafepointFingerprintSetV1 {
        &self.safepoints
    }

    pub const fn callable_registration_objects(
        &self,
    ) -> &VerifiedStrongCallableRegistrationObjectFingerprintSetV1 {
        &self.callable_registration_objects
    }

    pub const fn type_registration_objects(
        &self,
    ) -> &VerifiedStrongTypeRegistrationObjectFingerprintSetV1 {
        &self.type_registration_objects
    }

    pub const fn immortal_object_registration_objects(
        &self,
    ) -> &VerifiedStrongImmortalObjectRegistrationObjectFingerprintSetV1 {
        &self.immortal_object_registration_objects
    }

    pub const fn static_storage_registration_objects(
        &self,
    ) -> &VerifiedStrongStaticStorageRegistrationObjectFingerprintSetV1 {
        &self.static_storage_registration_objects
    }

    pub const fn initialization_registration_objects(
        &self,
    ) -> &VerifiedStrongInitializationRegistrationObjectFingerprintSetV1 {
        &self.initialization_registration_objects
    }

    pub const fn production_manifest(&self) -> &CBridgeCheckedSingleConeProductionManifestV1 {
        &self.production_manifest
    }

    pub fn fingerprint_registration_dependencies(
        self,
    ) -> Result<
        RegistrationDependencyFingerprintedSingleConeLinkSections<'input>,
        StrongLinkRegistrationDependencyFingerprintError,
    > {
        let Self {
            graph,
            identities,
            foundations,
            production,
            link_identity_closure,
            scoop_objects,
            defined_symbols,
            undefined_symbols,
            safepoints,
            callable_registration_objects,
            type_registration_objects,
            immortal_object_registration_objects,
            static_storage_registration_objects,
            initialization_registration_objects,
            production_manifest,
        } = self;
        let scoop_candidates = scoop_objects.candidates();

        let stackmaps = safepoints.registrations().stackmaps().clone();
        let callable_bodies = crate::compute_strong_callable_body_object_fingerprints_v1(
            callable_registration_objects,
            stackmaps,
            undefined_symbols.clone(),
            &scoop_candidates,
        )
        .map_err(StrongLinkRegistrationDependencyFingerprintError::CallableBodies)?;
        let callables = crate::compute_strong_callable_fingerprints_v1(callable_bodies)
            .map_err(StrongLinkRegistrationDependencyFingerprintError::Callables)?;

        let type_dependencies = crate::compute_strong_type_dependency_fingerprints_v1(
            type_registration_objects,
            &scoop_candidates,
        )
        .map_err(StrongLinkRegistrationDependencyFingerprintError::TypeDependencies)?;
        let types = crate::compute_strong_type_fingerprints_v1(type_dependencies)
            .map_err(StrongLinkRegistrationDependencyFingerprintError::Types)?;

        let immortal_object_definitions =
            crate::compute_strong_immortal_object_definition_fingerprints_v1(
                immortal_object_registration_objects,
                undefined_symbols.clone(),
                &scoop_candidates,
            )
            .map_err(StrongLinkRegistrationDependencyFingerprintError::ImmortalObjectDefinitions)?;
        let immortal_objects =
            crate::compute_strong_immortal_object_fingerprints_v1(immortal_object_definitions)
                .map_err(StrongLinkRegistrationDependencyFingerprintError::ImmortalObjects)?;

        let static_storage_definitions =
            crate::compute_strong_static_storage_definition_fingerprints_v1(
                static_storage_registration_objects,
                &scoop_candidates,
            )
            .map_err(StrongLinkRegistrationDependencyFingerprintError::StaticStorageDefinitions)?;
        let static_storage_shapes =
            crate::compute_strong_static_storage_shape_fingerprints_v1(static_storage_definitions)
                .map_err(StrongLinkRegistrationDependencyFingerprintError::StaticStorageShapes)?;
        let static_storages =
            crate::compute_strong_static_storage_fingerprints_v1(static_storage_shapes)
                .map_err(StrongLinkRegistrationDependencyFingerprintError::StaticStorages)?;

        let initialization_definitions =
            crate::compute_strong_initialization_definition_fingerprints_v1(
                initialization_registration_objects,
                &scoop_candidates,
            )
            .map_err(StrongLinkRegistrationDependencyFingerprintError::InitializationDefinitions)?;
        let initializations = crate::compute_strong_initialization_fingerprints_v1(
            initialization_definitions,
            callables.body_objects(),
        )
        .map_err(StrongLinkRegistrationDependencyFingerprintError::Initializations)?;

        Ok(RegistrationDependencyFingerprintedSingleConeLinkSections {
            graph,
            identities,
            foundations,
            production,
            link_identity_closure,
            scoop_objects,
            defined_symbols,
            undefined_symbols,
            safepoints,
            callables,
            types,
            immortal_objects,
            static_storages,
            initializations,
            production_manifest,
        })
    }
}

impl<'input> RegistrationDependencyFingerprintedSingleConeLinkSections<'input> {
    pub const fn coordinate(&self) -> &ConeCoordinate {
        self.graph.coordinate()
    }

    pub const fn identity(&self) -> ConeIdentity {
        self.graph.identity()
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

    pub const fn hir_foundation(&self) -> &OdrFreeHirFoundation {
        &self.foundations.hir
    }

    pub const fn mir_foundation(&self) -> &OdrFreeMirFoundation {
        &self.foundations.mir
    }

    pub const fn lir_foundation(&self) -> &OdrFreeLirFoundation {
        &self.foundations.lir
    }

    pub const fn production(&self) -> &ValidatedSingleConeStrongProduction {
        &self.production
    }

    pub const fn symbol_projections(&self) -> &SymbolProjectionCheckedLinkIdentityClosureSectionV1 {
        &self.link_identity_closure
    }

    pub const fn scoop_objects(&self) -> &VerifiedNormalizedProvisionalScoopLirObjectSetV1 {
        &self.scoop_objects
    }

    pub const fn defined_symbols(&self) -> &CanonicalDefinedLinkSymbolOwnerSetV1 {
        &self.defined_symbols
    }

    pub const fn undefined_symbols(&self) -> &CanonicalUndefinedSymbolRequirementSetV1 {
        &self.undefined_symbols
    }

    pub const fn safepoints(&self) -> &VerifiedStrongSafepointFingerprintSetV1 {
        &self.safepoints
    }

    pub const fn callables(&self) -> &VerifiedStrongCallableFingerprintSetV1 {
        &self.callables
    }

    pub const fn types(&self) -> &VerifiedStrongTypeFingerprintSetV1 {
        &self.types
    }

    pub const fn immortal_objects(&self) -> &VerifiedStrongImmortalObjectFingerprintSetV1 {
        &self.immortal_objects
    }

    pub const fn static_storages(&self) -> &VerifiedStrongStaticStorageFingerprintSetV1 {
        &self.static_storages
    }

    pub const fn initializations(&self) -> &VerifiedStrongInitializationFingerprintSetV1 {
        &self.initializations
    }

    pub const fn production_manifest(&self) -> &CBridgeCheckedSingleConeProductionManifestV1 {
        &self.production_manifest
    }

    pub fn finalize_strong_objects(
        self,
    ) -> Result<FinalizedStrongLinkObjectSections<'input>, StrongLinkObjectFinalizationError> {
        let Self {
            graph,
            identities,
            foundations,
            production,
            link_identity_closure,
            scoop_objects,
            defined_symbols,
            undefined_symbols,
            safepoints,
            callables,
            types,
            immortal_objects,
            static_storages,
            initializations,
            production_manifest,
        } = self;
        let scoop_candidates = scoop_objects.candidates();

        let patch_sites = safepoints.registrations().patch_sites().clone();
        let image = crate::verify_cone_image_v1(
            patch_sites.clone(),
            production.lir().image_plan().clone(),
            &scoop_candidates,
        )
        .map_err(StrongLinkObjectFinalizationError::ImageValidation)?;
        let entry = crate::verify_entry_production_v1(
            patch_sites,
            production.lir().entry_plan().clone(),
            &scoop_candidates,
        )
        .map_err(StrongLinkObjectFinalizationError::EntryValidation)?;
        let registrations = crate::patch_strong_registration_fingerprints_v1(
            safepoints,
            callables,
            types,
            immortal_objects,
            static_storages,
            initializations,
            &scoop_candidates,
        )
        .map_err(StrongLinkObjectFinalizationError::RegistrationPatch)?;
        let image_fingerprint = crate::compute_runtime_image_fingerprint_v1(
            image,
            registrations,
            graph.envelope.manifest().compatibility().clone(),
        )
        .map_err(StrongLinkObjectFinalizationError::ImageFingerprint)?;
        let runtime_images = crate::patch_runtime_image_fingerprint_v1(image_fingerprint)
            .map_err(StrongLinkObjectFinalizationError::ImagePatch)?;
        let final_objects = crate::patch_entry_production_v1(runtime_images, entry)
            .map_err(StrongLinkObjectFinalizationError::EntryPatch)?;
        verify_reconstructed_scoop_objects(&scoop_objects, &final_objects)
            .map_err(StrongLinkObjectFinalizationError::FinalObjectMismatch)?;

        Ok(FinalizedStrongLinkObjectSections {
            graph,
            identities,
            foundations,
            production,
            link_identity_closure,
            defined_symbols,
            undefined_symbols,
            final_objects,
            production_manifest,
        })
    }
}

impl<'input> FinalizedStrongLinkObjectSections<'input> {
    pub const fn coordinate(&self) -> &ConeCoordinate {
        self.graph.coordinate()
    }

    pub const fn identity(&self) -> ConeIdentity {
        self.graph.identity()
    }

    pub fn identity_count(&self) -> usize {
        self.identities.identity_count()
    }

    pub const fn hir_foundation(&self) -> &OdrFreeHirFoundation {
        &self.foundations.hir
    }

    pub const fn mir_foundation(&self) -> &OdrFreeMirFoundation {
        &self.foundations.mir
    }

    pub const fn lir_foundation(&self) -> &OdrFreeLirFoundation {
        &self.foundations.lir
    }

    pub const fn production(&self) -> &ValidatedSingleConeStrongProduction {
        &self.production
    }

    pub const fn symbol_projections(&self) -> &SymbolProjectionCheckedLinkIdentityClosureSectionV1 {
        &self.link_identity_closure
    }

    pub const fn defined_symbols(&self) -> &CanonicalDefinedLinkSymbolOwnerSetV1 {
        &self.defined_symbols
    }

    pub const fn undefined_symbols(&self) -> &CanonicalUndefinedSymbolRequirementSetV1 {
        &self.undefined_symbols
    }

    pub const fn final_objects(&self) -> &VerifiedEntryPatchSetV1 {
        &self.final_objects
    }

    pub const fn production_manifest(&self) -> &CBridgeCheckedSingleConeProductionManifestV1 {
        &self.production_manifest
    }

    pub fn validate_code_and_closure(
        self,
    ) -> Result<ValidatedSingleConeStrongLinkArtifact<'input>, StrongLinkFinalValidationError> {
        let Self {
            graph,
            identities,
            foundations,
            production,
            link_identity_closure,
            defined_symbols,
            undefined_symbols,
            final_objects,
            production_manifest,
        } = self;
        let manifest = graph.envelope.manifest();
        let native_requirements = CanonicalNativeExternalRequirementSurfaceV1::from_foundation(
            graph.target_selection().target(),
            &foundations.lir,
        )
        .map_err(StrongLinkFinalValidationError::NativeSurface)?;
        let link_objects =
            crate::verify_code_link_object_members_v1(final_objects, manifest.members())
                .map_err(StrongLinkFinalValidationError::LinkObjectMembers)?;
        let code_projection = crate::verify_single_cone_production_code_projection_v1(
            manifest.cone(),
            manifest.direct_dependencies(),
            foundations.hir.as_canonical().counts().sources,
            production.lir().clone(),
            link_objects,
        )
        .map_err(StrongLinkFinalValidationError::ProductionProjection)?;
        let code = crate::compute_code_fingerprint_v1(
            code_projection,
            native_requirements,
            defined_symbols,
            undefined_symbols,
        )
        .map_err(StrongLinkFinalValidationError::CodeFingerprint)?;
        let runtime_image = code
            .production()
            .link_objects()
            .final_objects()
            .runtime_images()
            .fingerprint()
            .fingerprint();
        let semantic = manifest.semantic_fingerprints();
        if semantic.code() != crate::FingerprintAvailability::Available(code.fingerprint())
            || semantic.runtime_image() != crate::FingerprintAvailability::Available(runtime_image)
        {
            return Err(StrongLinkFinalValidationError::SemanticFingerprintMismatch);
        }
        let production_manifest = production_manifest
            .validate(&code)
            .map_err(StrongLinkFinalValidationError::ProductionManifest)?;
        let link_identity_closure = link_identity_closure
            .validate(&code)
            .map_err(StrongLinkFinalValidationError::LinkIdentityClosure)?;
        Ok(ValidatedSingleConeStrongLinkArtifact {
            graph,
            identities,
            foundations,
            production,
            link_identity_closure,
            production_manifest,
        })
    }
}

impl ValidatedSingleConeStrongLinkArtifact<'_> {
    pub const fn coordinate(&self) -> &ConeCoordinate {
        self.graph.coordinate()
    }

    pub const fn identity(&self) -> ConeIdentity {
        self.graph.identity()
    }

    pub const fn kind(&self) -> ConeKind {
        self.graph.kind()
    }

    pub const fn source_form(&self) -> ConeSourceForm {
        self.graph.source_form()
    }

    pub const fn target_selection(&self) -> scoop_lir::ValidatedLirTargetSelection {
        self.graph.target_selection()
    }

    pub const fn artifact_fingerprint(&self) -> ArtifactFingerprint {
        self.graph.artifact_fingerprint()
    }

    pub const fn semantic_fingerprints(&self) -> SemanticFingerprintRecord {
        self.graph.envelope.manifest().semantic_fingerprints()
    }

    pub const fn decode_usage(&self) -> DecodeUsage {
        self.graph.decode_usage()
    }

    pub fn identity_count(&self) -> usize {
        self.identities.identity_count()
    }

    pub const fn hir_foundation(&self) -> &OdrFreeHirFoundation {
        &self.foundations.hir
    }

    pub const fn mir_foundation(&self) -> &OdrFreeMirFoundation {
        &self.foundations.mir
    }

    pub const fn lir_foundation(&self) -> &OdrFreeLirFoundation {
        &self.foundations.lir
    }

    pub const fn production(&self) -> &ValidatedSingleConeStrongProduction {
        &self.production
    }

    pub const fn link_identity_closure(&self) -> &LinkIdentityClosureSectionV1 {
        &self.link_identity_closure
    }

    pub const fn production_manifest(&self) -> &SingleConeProductionManifestV1 {
        &self.production_manifest
    }
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
) -> Result<ArtifactCapabilityProfile, SingleConeLinkSectionDecodeError> {
    let actual = graph.envelope.manifest().compatibility().artifact_profile();
    if actual != &ArtifactCapabilityProfileId::single_cone_strong() {
        return Err(SingleConeLinkSectionDecodeError::WrongProfile {
            actual: actual.clone(),
        });
    }
    Ok(ArtifactCapabilityProfile::SINGLE_CONE_STRONG)
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
    SourceExternal(SourceExternalRequirementValidationError),
    RuntimeAndEh(RuntimeAndEhRequirementValidationError),
    GeneratedBridgeSemantics(GeneratedCBridgeSemanticValidationError),
    CBridgeTargetSupport(CBridgeTargetSupportRequirementValidationError),
    UnclassifiedExternal(BuiltinObjectExternalRequirementClosureError),
    UndefinedSymbols(UndefinedSymbolRequirementFinalizationError),
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
            Self::SourceExternal(error) => error,
            Self::RuntimeAndEh(error) => error,
            Self::GeneratedBridgeSemantics(error) => error,
            Self::CBridgeTargetSupport(error) => error,
            Self::UnclassifiedExternal(error) => error,
            Self::UndefinedSymbols(error) => error,
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
        }
    }
}

#[derive(Debug)]
pub enum StrongLinkArtifactValidationError {
    Decode(Box<SingleConeLinkSectionDecodeError>),
    Identities(Box<IdentityValidationError>),
    Foundations(Box<StrongProfileFoundationError>),
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
        write!(
            formatter,
            "invalid SingleConeStrong Link artifact: {self:?}"
        )
    }
}

impl std::error::Error for StrongLinkArtifactValidationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(match self {
            Self::Decode(error) => error.as_ref(),
            Self::Identities(error) => error.as_ref(),
            Self::Foundations(error) => error.as_ref(),
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
        write!(
            formatter,
            "cannot decode single-Cone Link sections: {self:?}"
        )
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
