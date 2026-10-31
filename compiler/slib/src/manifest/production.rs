//! Shared Code projection for one Cone's final definitions.

use std::fmt;

use scoop_identity::{
    MainCallableBodyId, PersistentCallableBodyId, PersistentStaticStorageId,
    SourceSignatureFingerprint,
};
use scoop_lir::{ConeProductionSectionV1, RegistrationIdentitySurfaceV1};
use scoop_wire::{Encoder, WireEncode};

use super::{ConeKind, ConeRecord, ConeSourceForm, RuntimeImageFingerprint};
use crate::SlibMemberId;
use crate::link_object::{
    CanonicalOdrMemberDirectoryV1, EntryProductionBranchV1, FinalCodeObjectSetV1,
    ObjectDefinitionFingerprintV1, OdrMemberDirectoryProjectionError,
};

mod projection;
pub(crate) use projection::{ProductionPlanInputs, build_production_code_projection_common};

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

/// Strong LIR metadata and finalized object bytes producing the shared Code
/// projection.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedSingleConeProductionCodeProjectionV1 {
    strong_production: ConeProductionSectionV1,
    link_objects: FinalCodeObjectSetV1,
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

    pub const fn link_objects(&self) -> &FinalCodeObjectSetV1 {
        &self.link_objects
    }

    pub const fn projection(&self) -> &SingleConeProductionCodeProjectionV1 {
        &self.projection
    }
}

pub fn verify_single_cone_production_code_projection_v1(
    cone: &ConeRecord,
    strong_production: ConeProductionSectionV1,
    link_objects: FinalCodeObjectSetV1,
) -> Result<VerifiedSingleConeProductionCodeProjectionV1, ProductionCodeProjectionError> {
    verify_production_code_projection_v1(cone, strong_production, link_objects)
}

/// Projects the completed ordinary cross-Cone image and its runtime registrations.
pub fn verify_cross_cone_production_code_projection_v1(
    cone: &ConeRecord,
    strong_production: ConeProductionSectionV1,
    link_objects: FinalCodeObjectSetV1,
) -> Result<VerifiedSingleConeProductionCodeProjectionV1, ProductionCodeProjectionError> {
    verify_single_cone_production_code_projection_v1(cone, strong_production, link_objects)
}

fn verify_production_code_projection_v1(
    cone: &ConeRecord,
    strong_production: ConeProductionSectionV1,
    link_objects: FinalCodeObjectSetV1,
) -> Result<VerifiedSingleConeProductionCodeProjectionV1, ProductionCodeProjectionError> {
    let projection = build_production_code_projection_common(
        cone,
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProductionCodeProjectionError {
    InvalidSingleFileLinkRoot,
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
