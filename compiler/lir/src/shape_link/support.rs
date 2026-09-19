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

/// Explicitly grants no object or initialization support. General subjects
/// can use this authority without acquiring private storage access.
#[derive(Clone, Copy, Debug, Default)]
pub struct NoShapeLinkSupportV1;

impl sealed::Sealed for NoShapeLinkSupportV1 {}

impl<'a> ShapeLinkSupportAuthorityV1<'a> for NoShapeLinkSupportV1 {
    fn support_source(
        &self,
        _: ConeIdentity,
        subject: ExternalStrongShapeSubjectV1,
        meter: &mut BudgetMeter,
    ) -> Result<Option<ShapeLinkSupportSourceV1<'a>>, ShapeLinkError> {
        meter.charge_work(1, &scoop_wire::WirePath::root())?;
        use ExternalStrongShapeSubjectV1 as Subject;
        match subject {
            Subject::Callable(_)
            | Subject::Layout(_)
            | Subject::Scan(_)
            | Subject::TypeDescriptor(_)
            | Subject::DispatchTable(_)
            | Subject::TypeRegistration(_) => Ok(None),
            Subject::StaticStorage(_)
            | Subject::StaticStorageRegistration(_)
            | Subject::InitializationCell(_)
            | Subject::InitializationDescriptor(_) => Err(ShapeLinkError::SupportRelation(subject)),
        }
    }
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
