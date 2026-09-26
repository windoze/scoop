use super::*;

impl ShapeLinkProviderV1<'_> {
    pub(super) fn reject_legacy(
        &self,
        subject: ExternalStrongShapeSubjectV1,
    ) -> Result<(), ShapeLinkError> {
        Self::reject_legacy_subject(self.parts.ordinary, subject)
    }

    pub(crate) fn reject_legacy_subject(
        ordinary: &CrossConeLirBridgeSectionV1,
        subject: ExternalStrongShapeSubjectV1,
    ) -> Result<(), ShapeLinkError> {
        if let ExternalStrongShapeSubjectV1::Callable(target) = subject {
            if ordinary
                .exports()
                .iter()
                .any(|record| record.target() == target)
            {
                return Err(ShapeLinkError::LegacyPartition(subject));
            }
        }
        Ok(())
    }
}
