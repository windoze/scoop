//! Production-manifest projection backed by M23-6 Strong V2 semantics.

use scoop_lir::{
    CBridgeProductionSetV1, CanonicalNativeLibraryRequirementV1, ConeProductionSectionV2,
    RegistrationIdentitySurfaceV1,
};
use scoop_wire::{Encoder, WireEncode};

use super::{
    ConeRecord, DependencyRecord, ProductionCodeProjectionError,
    SingleConeProductionCodeProjectionV1,
    production::{ProductionPlanInputs, verify_production_code_projection_common},
};
use crate::link_object::VerifiedCodeLinkObjectMemberSetV2;
use crate::{CodeFingerprint, RuntimeImageFingerprint, SlibMemberId, VerifiedCodeFingerprintV2};

/// V2 metadata and final object bytes producing the shared manifest projection.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedSingleConeProductionCodeProjectionV2 {
    strong_production: ConeProductionSectionV2,
    link_objects: VerifiedCodeLinkObjectMemberSetV2,
    projection: SingleConeProductionCodeProjectionV1,
}

impl VerifiedSingleConeProductionCodeProjectionV2 {
    pub const fn strong_production(&self) -> &ConeProductionSectionV2 {
        &self.strong_production
    }

    pub const fn link_objects(&self) -> &VerifiedCodeLinkObjectMemberSetV2 {
        &self.link_objects
    }

    pub const fn projection(&self) -> &SingleConeProductionCodeProjectionV1 {
        &self.projection
    }
}

/// Verifies the layout image against the artifact's complete direct dependency
/// inventory before accepting its object and runtime fingerprint projection.
pub fn verify_cross_cone_layout_production_code_projection_v1(
    cone: &ConeRecord,
    direct_dependencies: &[DependencyRecord],
    source_count: usize,
    strong_production: ConeProductionSectionV2,
    link_objects: VerifiedCodeLinkObjectMemberSetV2,
) -> Result<VerifiedSingleConeProductionCodeProjectionV2, ProductionCodeProjectionError> {
    let dependency_identities = direct_dependencies
        .iter()
        .map(DependencyRecord::identity)
        .collect::<Vec<_>>();
    let projection = verify_production_code_projection_common(
        cone,
        &dependency_identities,
        source_count,
        ProductionPlanInputs::from(&strong_production),
        &link_objects,
    )?;
    Ok(VerifiedSingleConeProductionCodeProjectionV2 {
        strong_production,
        link_objects,
        projection,
    })
}

/// The shared eleven-field production manifest, derived from a Code result
/// whose production input is V2.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CrossConeLayoutProductionManifestV1 {
    code: VerifiedCodeFingerprintV2,
}

impl CrossConeLayoutProductionManifestV1 {
    pub const fn from_verified_code(code: VerifiedCodeFingerprintV2) -> Self {
        Self { code }
    }

    pub const fn code_proof(&self) -> &VerifiedCodeFingerprintV2 {
        &self.code
    }

    pub const fn distribution(&self) -> super::ArtifactDistributionClassV1 {
        self.projection().distribution()
    }

    pub const fn output(&self) -> &super::SingleConeProductionOutputV1 {
        self.projection().output()
    }

    pub const fn image_owner_member(&self) -> SlibMemberId {
        self.projection().image_owner_member()
    }

    pub const fn runtime_registration_projection(&self) -> &RegistrationIdentitySurfaceV1 {
        self.projection().runtime_registration_projection()
    }

    pub const fn strong_registration_set(
        &self,
    ) -> &crate::CanonicalStrongRegistrationFingerprintSetV1 {
        self.projection().strong_registration_set()
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

    pub const fn native_contracts(&self) -> &crate::CanonicalNativeExternalContractCodeSetV1 {
        self.code.native_contracts()
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

impl WireEncode for CrossConeLayoutProductionManifestV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(11)?;
        encoder.field(1)?;
        self.distribution().encode(encoder)?;
        encoder.field(2)?;
        self.output().encode(encoder)?;
        encoder.field(3)?;
        self.image_owner_member().encode(encoder)?;
        encoder.field(4)?;
        self.runtime_registration_projection().encode(encoder)?;
        encoder.field(5)?;
        self.strong_registration_set().encode(encoder)?;
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
        self.odr_members().encode(encoder)
    }
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
