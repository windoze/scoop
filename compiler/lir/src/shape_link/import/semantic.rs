use super::*;
use crate::{CanonicalExactCallableAbiExportsV1, CanonicalExactLayoutExportsV1};
impl ExternalShapeLinkImportV1 {
    pub(crate) fn semantic_target(
        &self,
        layouts: &CanonicalExactLayoutExportsV1,
        callables: &CanonicalExactCallableAbiExportsV1,
    ) -> Result<Option<crate::LayoutAbiSemanticTargetV1>, ShapeLinkError> {
        semantic_target(self.subject(), layouts, callables)
    }
}

pub(in crate::shape_link) fn semantic_target(
    subject: ExternalStrongShapeSubjectV1,
    layouts: &CanonicalExactLayoutExportsV1,
    callables: &CanonicalExactCallableAbiExportsV1,
) -> Result<Option<crate::LayoutAbiSemanticTargetV1>, ShapeLinkError> {
    use crate::{ExternalStrongShapeSubjectV1 as Subject, LayoutAbiSemanticTargetV1 as Target};

    Ok(match subject {
        Subject::Callable(owner) => callables.get(owner).map(|_| Target::Callable(owner)),
        Subject::Layout(id) => Some(Target::Layout(id)),
        Subject::Scan(id) => Some(Target::Layout(
            layouts
                .records()
                .iter()
                .find(|record| record.scan() == id)
                .ok_or(ShapeLinkError::MissingSubject(subject))?
                .identity()
                .layout(),
        )),
        Subject::TypeDescriptor(id) | Subject::TypeRegistration(id) => Some(Target::Descriptor(id)),
        Subject::DispatchTable(id) => Some(Target::Dispatch(id)),
        Subject::ImmortalObject(_)
        | Subject::StaticStorage(_)
        | Subject::StaticStorageRegistration(_)
        | Subject::InitializationCell(_)
        | Subject::InitializationRegistration(_) => None,
    })
}
