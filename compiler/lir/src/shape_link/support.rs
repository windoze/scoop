use scoop_identity::ConeIdentity;
use scoop_wire::BudgetMeter;

use super::ShapeLinkError;
use crate::{
    ExternalStrongShapeSubjectV1, StrongInitializationUnitSemanticPlanV2,
    StrongStaticStorageSemanticPlanV1,
};

pub(crate) mod sealed {
    pub trait Sealed {}
}

/// Implemented by the complete section after its exported object/unit and
/// terminal selection joins. A query does not itself grant an import.
pub trait ShapeLinkSupportAuthorityV1<'a>: sealed::Sealed {
    fn support_source(
        &self,
        provider: ConeIdentity,
        subject: ExternalStrongShapeSubjectV1,
        meter: &mut BudgetMeter,
    ) -> Result<Option<ShapeLinkSupportSourceV1<'a>>, ShapeLinkError>;
}

#[derive(Clone, Copy, Debug)]
pub enum ShapeLinkSupportSourceV1<'a> {
    StaticStorage {
        unit: &'a StrongInitializationUnitSemanticPlanV2,
        storage: &'a StrongStaticStorageSemanticPlanV1,
    },
    Initialization {
        unit: &'a StrongInitializationUnitSemanticPlanV2,
    },
}
