use super::*;

impl ShapeLinkProviderV1<'_> {
    pub(super) fn reject_legacy(
        &self,
        subject: ExternalStrongShapeSubjectV1,
    ) -> Result<(), ShapeLinkError> {
        Self::reject_legacy_subject(
            self.parts.ordinary,
            self.parts.production.initialization_cycle_abi(),
            subject,
        )
    }

    pub(crate) fn reject_legacy_subject(
        ordinary: &CrossConeLirBridgeSectionV1,
        initialization_abi: Option<&CallableAbiRecordV1>,
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
            if let Some(abi) = initialization_abi {
                if abi.target() == target {
                    return Err(ShapeLinkError::LegacyPartition(subject));
                }
            }
        }
        Ok(())
    }
}
