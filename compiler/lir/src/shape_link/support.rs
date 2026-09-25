use scoop_identity::ConeIdentity;

use super::ShapeLinkError;
use crate::{
    ExternalStrongShapeSubjectV1, StrongInitializationUnitSemanticPlanV2,
    StrongStaticStorageSemanticPlanV1,
};

/// A candidate relation lookup owned by the enclosing closure. Implementations
/// must join actual object/unit exports and selection; this interface supplies
/// no source, access, machine-selection or publication proof.
pub trait ShapeLinkSupportLookupV1<'a> {
    fn support_source(
        &self,
        provider: ConeIdentity,
        subject: ExternalStrongShapeSubjectV1,
    ) -> Result<Option<ShapeLinkSupportSourceV1<'a>>, ShapeLinkError>;
}

/// Explicitly grants no object or initialization support. General subjects
/// can use this authority without acquiring private storage access.
#[derive(Clone, Copy, Debug, Default)]
pub struct NoShapeLinkSupportV1;

impl<'a> ShapeLinkSupportLookupV1<'a> for NoShapeLinkSupportV1 {
    fn support_source(
        &self,
        _: ConeIdentity,
        subject: ExternalStrongShapeSubjectV1,
    ) -> Result<Option<ShapeLinkSupportSourceV1<'a>>, ShapeLinkError> {
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
