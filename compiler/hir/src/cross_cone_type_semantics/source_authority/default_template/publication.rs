use super::*;
use crate::{
    ProtectedDefaultReferenceSetV1, ProtectedDefaultTemplateBuildError, ProtectedDefaultTemplateV1,
};

impl DefaultSourceTemplateV1 {
    /// Preserve the source body and provider envelope while publishing the
    /// canonical reference occurrences for the current parameter owner.
    pub(crate) fn into_protected_template(
        self,
        references: ProtectedDefaultReferenceSetV1,
    ) -> Result<ProtectedDefaultTemplateV1, ProtectedDefaultTemplateBuildError> {
        ProtectedDefaultTemplateV1::try_new(
            self.key,
            self.definition_root,
            self.definition_path,
            self.locals,
            self.body,
            self.result,
            self.allows_suspend,
            self.type_parameters,
            self.receiver,
            self.value_parameters,
            references,
            self.definition_origin,
        )
    }
}
