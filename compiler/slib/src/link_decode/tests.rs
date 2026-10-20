use scoop_identity::{
    CapabilityId, CborIdentityRecord, ConeCoordinate, ConeIdentity, ConeImageSupportRole,
    DefinitionAtomRole, DefinitionAtomSubkey, DigestNodeKey, LinkageClass, ObjectDefinitionAtomId,
    ObjectDefinitionAtomKey, ObjectDefinitionPlanId, ObjectDefinitionPlanKey, PersistentSymbolKey,
    PersistentSymbolRequest, PersistentSymbolRequestTable, StrongDefinitionEntity,
    StrongDefinitionRole,
};
use scoop_lir::{
    AppleClangCompilerIdentityV1, CBridgeProductionSetV1, CBridgeToolchainProfileV1,
    CanonicalLirFoundation, ConeLirFoundation, ConeProductionSectionV1,
    DarwinCBridgeDeploymentContractV1, DarwinPackedVersionV1, DigestFinalizationPlanV1,
    EntryProductionSourceV1, ObjectSymbolSurfaceV1, ProducerUnitPartitionV1,
    ValidatedLirTargetSelection,
};
use scoop_wire::encode;

use crate::{
    ArtifactCapabilityProfile, BootstrapManifest, CanonicalSlibArchive, CompatibilityRecord,
    ConeKind, ConeRecord, ConeSourceForm, DependencyRecord, HirFingerprint, ManifestSection,
    MemberPurposeSet, MemberStableKey, MetadataEnvelope, MetadataLocation, MetadataSection,
    PlannedLinkObjectMemberSetV1, PlannedStrongObjectSymbolSetV1, ProducerRecord,
    SemanticFingerprintRecord, SlibMember, SlibMemberRole, hir_core_bootstrap_interface_capability,
    hir_identity_foundation_capability, lir_identity_foundation_capability,
    lir_link_identity_closure_capability, lir_strong_production_capability,
    manifest_single_cone_production_capability, mir_core_bootstrap_bridge_capability,
    mir_identity_foundation_capability,
};
use scoop_lir::CanonicalNativeExternalRequirementSurfaceV1;

pub(crate) mod layout_link_support;

mod archive;
mod extension_members;
mod objects;
mod production;
mod sections;

pub(super) use archive::complete_artifact;
use objects::{finalized_link_object_fixture, link_object_fixture, link_object_plan};
use production::selection;
pub(super) use production::{c_bridge_profile, cone};
pub(crate) use production::{image_atoms, strong_production_fixture};
use sections::{empty_hir_library_section, empty_mir_library_section};

#[test]
fn canonical_link_artifact_is_byte_reproducible() {
    assert_eq!(complete_artifact(), complete_artifact());
}
