//! Code projection backed by the completed layout and native objects.

use scoop_lir::ConeProductionSectionV2;

use super::{
    ConeRecord, ProductionCodeProjectionError, SingleConeProductionCodeProjectionV1,
    production::{ProductionPlanInputs, build_production_code_projection_common},
};
use crate::link_object::FinalCodeObjectSetV2;

/// V2 metadata and final object bytes producing the shared Code projection.
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
