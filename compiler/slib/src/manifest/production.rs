//! Shared production-manifest projection for one Cone's final definitions.

use std::fmt;

use scoop_identity::{
    MainCallableBodyId, ObjectDefinitionPlanId, PersistentCallableBodyId,
    PersistentStaticStorageId, SourceSignatureFingerprint,
};
use scoop_lir::{
    CBridgeProductionSetV1, CanonicalNativeLibraryRequirementV1, ConeProductionSectionV1,
    EntryProductionPlanV1, RegistrationIdentitySurfaceV1, RegistrationIdentityV1,
};
use scoop_wire::{Encoder, WireEncode};

use super::{ConeKind, ConeRecord, ConeSourceForm, DependencyRecord, RuntimeImageFingerprint};
use crate::SlibMemberId;
use crate::link_object::{
    CanonicalOdrMemberDirectoryV1, ObjectDefinitionFingerprintV1,
    OdrMemberDirectoryProjectionError, VerifiedCodeFingerprintV1,
    VerifiedCodeLinkObjectMemberSetV1, VerifiedEntryProductionBranchV1,
    VerifiedStrongRegistrationPatchSetV1,
};

mod projection;
pub(crate) use projection::distribution;
pub(crate) use projection::{ProductionPlanInputs, verify_production_code_projection_common};

mod wire;
pub use wire::{
    CBridgeCheckedSingleConeProductionManifestV1, CodeProductionProjectionError,
    DecodedSingleConeProductionManifestV1, RuntimeProductionProjectionError,
    SingleConeProductionManifestValidationError,
};

#[cfg(test)]
pub(crate) fn encoded_library_production_manifest_for_test() -> Vec<u8> {
    wire::tests::library_manifest_bytes()
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ArtifactDistributionClassV1 {
    DistributableCone,
    LocalExecutableRoot,
}

impl WireEncode for ArtifactDistributionClassV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(1)?;
        encoder.field(0)?;
        encoder.unsigned(match self {
            Self::DistributableCone => 1,
            Self::LocalExecutableRoot => 2,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExecutableRootProjectionV1 {
    main: MainCallableBodyId,
    source_signature_fingerprint: SourceSignatureFingerprint,
    gateway: PersistentCallableBodyId,
    gateway_definition_fingerprint: ObjectDefinitionFingerprintV1,
    failure_root: PersistentStaticStorageId,
    entry_owner_member: SlibMemberId,
}

impl ExecutableRootProjectionV1 {
    pub const fn main(self) -> MainCallableBodyId {
        self.main
    }

    pub const fn source_signature_fingerprint(self) -> SourceSignatureFingerprint {
        self.source_signature_fingerprint
    }

    pub const fn gateway(self) -> PersistentCallableBodyId {
        self.gateway
    }

    pub const fn gateway_definition_fingerprint(self) -> ObjectDefinitionFingerprintV1 {
        self.gateway_definition_fingerprint
    }

    pub const fn failure_root(self) -> PersistentStaticStorageId {
        self.failure_root
    }

    pub const fn entry_owner_member(self) -> SlibMemberId {
        self.entry_owner_member
    }
}

impl WireEncode for ExecutableRootProjectionV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(6)?;
        encoder.field(1)?;
        self.main.encode(encoder)?;
        encoder.field(2)?;
        self.source_signature_fingerprint.encode(encoder)?;
        encoder.field(3)?;
        self.gateway.encode(encoder)?;
        encoder.field(4)?;
        self.gateway_definition_fingerprint.encode(encoder)?;
        encoder.field(5)?;
        self.failure_root.encode(encoder)?;
        encoder.field(6)?;
        self.entry_owner_member.encode(encoder)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SingleConeProductionOutputV1 {
    Library,
    Executable(Box<ExecutableRootProjectionV1>),
}

impl WireEncode for SingleConeProductionOutputV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Library => encode_empty_sum(encoder, 1),
            Self::Executable(root) => {
                encoder.map(2)?;
                encoder.field(0)?;
                encoder.unsigned(2)?;
                encoder.field(1)?;
                root.encode(encoder)
            }
        }
    }
}

/// The manifest contribution to `CodeFingerprint`; it cannot contain that
/// fingerprint and therefore cannot form a self-reference.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SingleConeProductionCodeProjectionV1 {
    distribution: ArtifactDistributionClassV1,
    output: SingleConeProductionOutputV1,
    image_owner_member: SlibMemberId,
    runtime_registration_projection: RegistrationIdentitySurfaceV1,

    runtime_image_fingerprint: RuntimeImageFingerprint,
    odr_members: CanonicalOdrMemberDirectoryV1,
    optimization: scoop_lir::OptimizationMode,
}

impl SingleConeProductionCodeProjectionV1 {
    pub fn with_optimization(mut self, mode: scoop_lir::OptimizationMode) -> Self {
        self.optimization = mode;
        self
    }

    pub const fn optimization(&self) -> scoop_lir::OptimizationMode {
        self.optimization
    }

    pub const fn distribution(&self) -> ArtifactDistributionClassV1 {
        self.distribution
    }

    pub const fn output(&self) -> &SingleConeProductionOutputV1 {
        &self.output
    }

    pub const fn image_owner_member(&self) -> SlibMemberId {
        self.image_owner_member
    }

    pub const fn runtime_registration_projection(&self) -> &RegistrationIdentitySurfaceV1 {
        &self.runtime_registration_projection
    }

    pub const fn runtime_image_fingerprint(&self) -> RuntimeImageFingerprint {
        self.runtime_image_fingerprint
    }

    pub const fn odr_members(&self) -> &CanonicalOdrMemberDirectoryV1 {
        &self.odr_members
    }
}

impl WireEncode for SingleConeProductionCodeProjectionV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(7)?;
        encoder.field(1)?;
        self.distribution.encode(encoder)?;
        encoder.field(2)?;
        self.output.encode(encoder)?;
        encoder.field(3)?;
        self.image_owner_member.encode(encoder)?;
        encoder.field(4)?;
        self.runtime_registration_projection.encode(encoder)?;
        encoder.field(6)?;
        self.runtime_image_fingerprint.encode(encoder)?;
        encoder.field(11)?;
        self.odr_members.encode(encoder)?;
        encoder.field(12)?;
        self.optimization.encode(encoder)
    }
}

/// Strong LIR metadata and finalized object bytes proven to describe the same
/// code-sink manifest projection.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedSingleConeProductionCodeProjectionV1 {
    strong_production: ConeProductionSectionV1,
    link_objects: VerifiedCodeLinkObjectMemberSetV1,
    projection: SingleConeProductionCodeProjectionV1,
}

impl VerifiedSingleConeProductionCodeProjectionV1 {
    pub fn with_optimization(mut self, mode: scoop_lir::OptimizationMode) -> Self {
        self.projection.optimization = mode;
        self
    }

    pub const fn strong_production(&self) -> &ConeProductionSectionV1 {
        &self.strong_production
    }

    pub const fn link_objects(&self) -> &VerifiedCodeLinkObjectMemberSetV1 {
        &self.link_objects
    }

    pub const fn projection(&self) -> &SingleConeProductionCodeProjectionV1 {
        &self.projection
    }
}

/// The complete eleven-field manifest payload. All fields remain derived from
/// the owned code proof, so repeated digests and projections cannot diverge.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SingleConeProductionManifestV1 {
    code: VerifiedCodeFingerprintV1,
}

impl SingleConeProductionManifestV1 {
    pub const fn optimization(&self) -> scoop_lir::OptimizationMode {
        self.projection().optimization()
    }

    pub const fn from_verified_code(code: VerifiedCodeFingerprintV1) -> Self {
        Self { code }
    }

    pub const fn code_proof(&self) -> &VerifiedCodeFingerprintV1 {
        &self.code
    }

    pub const fn distribution(&self) -> ArtifactDistributionClassV1 {
        self.projection().distribution()
    }

    pub const fn output(&self) -> &SingleConeProductionOutputV1 {
        self.projection().output()
    }

    pub const fn image_owner_member(&self) -> SlibMemberId {
        self.projection().image_owner_member()
    }

    pub const fn runtime_registration_projection(&self) -> &RegistrationIdentitySurfaceV1 {
        self.projection().runtime_registration_projection()
    }

    pub const fn runtime_image_fingerprint(&self) -> RuntimeImageFingerprint {
        self.projection().runtime_image_fingerprint()
    }

    pub const fn odr_members(&self) -> &CanonicalOdrMemberDirectoryV1 {
        self.projection().odr_members()
    }

    pub const fn code_fingerprint(&self) -> super::CodeFingerprint {
        self.code.fingerprint()
    }

    pub const fn native_contracts(
        &self,
    ) -> &crate::link_object::CanonicalNativeExternalContractCodeSetV1 {
        self.code.native_contracts()
    }

    pub fn native_cxx(&self) -> bool {
        self.code.native_requirements().cxx()
    }

    pub fn native_library_requirements(&self) -> &[CanonicalNativeLibraryRequirementV1] {
        self.code.native_requirements().library_requirements()
    }

    pub const fn c_bridge_production(&self) -> &CBridgeProductionSetV1 {
        self.code.c_bridge_production()
    }

    const fn projection(&self) -> &SingleConeProductionCodeProjectionV1 {
        self.code.production().projection()
    }
}

impl WireEncode for SingleConeProductionManifestV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(12)?;
        encoder.field(1)?;
        self.distribution().encode(encoder)?;
        encoder.field(2)?;
        self.output().encode(encoder)?;
        encoder.field(3)?;
        self.image_owner_member().encode(encoder)?;
        encoder.field(4)?;
        self.runtime_registration_projection().encode(encoder)?;
        encoder.field(6)?;
        self.runtime_image_fingerprint().encode(encoder)?;
        encoder.field(7)?;
        self.code_fingerprint().encode(encoder)?;
        encoder.field(8)?;
        self.native_contracts().encode(encoder)?;
        encoder.field(9)?;
        encode_array(encoder, self.native_library_requirements())?;
        encoder.field(10)?;
        self.c_bridge_production().encode(encoder)?;
        encoder.field(11)?;
        self.odr_members().encode(encoder)?;
        encoder.field(12)?;
        self.optimization().encode(encoder)?;
        encoder.field(13)?;
        encoder.unsigned(u64::from(self.native_cxx()))
    }
}

pub fn verify_single_cone_production_code_projection_v1(
    cone: &ConeRecord,
    direct_dependencies: &[DependencyRecord],
    source_count: usize,
    strong_production: ConeProductionSectionV1,
    link_objects: VerifiedCodeLinkObjectMemberSetV1,
) -> Result<VerifiedSingleConeProductionCodeProjectionV1, ProductionCodeProjectionError> {
    let dependency_identities = direct_dependencies
        .iter()
        .map(DependencyRecord::identity)
        .collect::<Vec<_>>();
    verify_production_code_projection_v1(
        cone,
        &dependency_identities,
        source_count,
        strong_production,
        link_objects,
    )
}

/// Verifies the ordinary cross-Cone image against the same direct dependency
/// inventory used by the artifact graph and semantic/link closures.
pub fn verify_cross_cone_production_code_projection_v1(
    cone: &ConeRecord,
    direct_dependencies: &[DependencyRecord],
    source_count: usize,
    strong_production: ConeProductionSectionV1,
    link_objects: VerifiedCodeLinkObjectMemberSetV1,
) -> Result<VerifiedSingleConeProductionCodeProjectionV1, ProductionCodeProjectionError> {
    verify_single_cone_production_code_projection_v1(
        cone,
        direct_dependencies,
        source_count,
        strong_production,
        link_objects,
    )
}

fn verify_production_code_projection_v1(
    cone: &ConeRecord,
    dependency_identities: &[scoop_identity::ConeIdentity],
    source_count: usize,
    strong_production: ConeProductionSectionV1,
    link_objects: VerifiedCodeLinkObjectMemberSetV1,
) -> Result<VerifiedSingleConeProductionCodeProjectionV1, ProductionCodeProjectionError> {
    projection::distribution(cone, dependency_identities, source_count)?;
    let projection = verify_production_code_projection_common(
        cone,
        dependency_identities,
        ProductionPlanInputs::from(&strong_production),
        &link_objects,
    )?;
    Ok(VerifiedSingleConeProductionCodeProjectionV1 {
        strong_production,
        link_objects,
        projection,
    })
}

fn encode_empty_sum(encoder: &mut Encoder, tag: u64) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.map(1)?;
    encoder.field(0)?;
    encoder.unsigned(tag)
}

fn encode_array(
    encoder: &mut Encoder,
    values: &[impl WireEncode],
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.array(values.len() as u64)?;
    for value in values {
        value.encode(encoder)?;
    }
    Ok(())
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProductionCodeProjectionError {
    InvalidSingleFileLinkRoot,
    ConeMismatch,
    DependencyMismatch,
    DigestPlanMismatch,
    ImagePlanMismatch,
    EntryPlanMismatch,
    GeneratedBridgePlanMismatch,
    RegistrationIdentityMismatch,
    InvalidSingleFileRoot {
        kind: ConeKind,
        source_count: usize,
        dependencies: Vec<scoop_identity::ConeIdentity>,
    },
    OutputMismatch,
    MissingGatewayFingerprint(PersistentCallableBodyId),

    OdrMembers(OdrMemberDirectoryProjectionError),
}

impl fmt::Display for ProductionCodeProjectionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "invalid production code projection: {self:?}")
    }
}

impl std::error::Error for ProductionCodeProjectionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::OdrMembers(source) => Some(source),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests;
