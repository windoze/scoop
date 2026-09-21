use super::*;

impl ShapeLinkProviderV1<'_> {
    pub(super) fn reject_legacy(
        &self,
        subject: ExternalStrongShapeSubjectV1,
        meter: &mut BudgetMeter,
    ) -> Result<(), ShapeLinkError> {
        Self::reject_legacy_subject(
            self.parts.ordinary,
            self.parts.production.core(),
            subject,
            meter,
        )
    }

    pub(crate) fn reject_legacy_subject(
        ordinary: &CrossConeLirBridgeSectionV1,
        core: &CoreLirBridgeBranchV1,
        subject: ExternalStrongShapeSubjectV1,
        meter: &mut BudgetMeter,
    ) -> Result<(), ShapeLinkError> {
        let path = WirePath::root();
        if let ExternalStrongShapeSubjectV1::Callable(target) = subject {
            meter.charge_work(ordinary.exports().len() as u64, &path)?;
            if ordinary
                .exports()
                .iter()
                .any(|record| record.target() == target)
            {
                return Err(ShapeLinkError::LegacyPartition(subject));
            }
            if let Some(core) = core.core() {
                meter.charge_work(1, &path)?;
                if core.initialization_cycle_thrower().target() == target {
                    return Err(ShapeLinkError::LegacyPartition(subject));
                }
            }
        }
        Ok(())
    }
}
