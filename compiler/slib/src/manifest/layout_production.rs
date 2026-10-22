//! Production-manifest projection backed by M23-6 Strong V2 semantics.

use scoop_lir::ConeProductionSectionV2;
use scoop_wire::{Encoder, WireEncode};

use super::{
    ConeRecord, ProductionCodeProjectionError, SingleConeProductionCodeProjectionV1,
    production::{ProductionPlanInputs, build_production_code_projection_common},
};
use crate::link_object::FinalCodeObjectSetV2;
use crate::{CodeFingerprint, CodeFingerprintDataV2, RuntimeImageFingerprint};

/// V2 metadata and final object bytes producing the shared manifest projection.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedSingleConeProductionCodeProjectionV2 {
    strong_production: ConeProductionSectionV2,
    link_objects: FinalCodeObjectSetV2,
    projection: SingleConeProductionCodeProjectionV1,
}

impl VerifiedSingleConeProductionCodeProjectionV2 {
    pub fn with_optimization(mut self, mode: scoop_lir::OptimizationMode) -> Self {
        self.projection = self.projection.with_optimization(mode);
        self
    }

    pub const fn strong_production(&self) -> &ConeProductionSectionV2 {
        &self.strong_production
    }

    pub const fn link_objects(&self) -> &FinalCodeObjectSetV2 {
        &self.link_objects
    }

    pub const fn projection(&self) -> &SingleConeProductionCodeProjectionV1 {
        &self.projection
    }
}

/// Projects the completed layout image, final objects, and runtime fingerprints.
pub fn verify_cross_cone_layout_production_code_projection_v1(
    cone: &ConeRecord,
    strong_production: ConeProductionSectionV2,
    link_objects: FinalCodeObjectSetV2,
) -> Result<VerifiedSingleConeProductionCodeProjectionV2, ProductionCodeProjectionError> {
    let projection = build_production_code_projection_common(
        cone,
        ProductionPlanInputs::from(&strong_production),
        &link_objects,
    )?;
    Ok(VerifiedSingleConeProductionCodeProjectionV2 {
        strong_production,
        link_objects,
        projection,
    })
}

/// The shared production manifest, derived from a Code result
/// whose production input is V2.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CrossConeLayoutProductionManifestV1 {
    code: CodeFingerprintDataV2,
}

impl CrossConeLayoutProductionManifestV1 {
    pub const fn from_verified_code(code: CodeFingerprintDataV2) -> Self {
        Self { code }
    }

    pub const fn code_proof(&self) -> &CodeFingerprintDataV2 {
        &self.code
    }

    pub const fn runtime_image_fingerprint(&self) -> RuntimeImageFingerprint {
        self.projection().runtime_image_fingerprint()
    }

    pub const fn odr_members(&self) -> &crate::CanonicalOdrMemberDirectoryV1 {
        self.projection().odr_members()
    }

    pub const fn code_fingerprint(&self) -> CodeFingerprint {
        self.code.fingerprint()
    }

    const fn projection(&self) -> &SingleConeProductionCodeProjectionV1 {
        self.code.production().projection()
    }
}

impl WireEncode for CrossConeLayoutProductionManifestV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(1)?;
        encoder.field(11)?;
        self.odr_members().encode(encoder)
    }
}
