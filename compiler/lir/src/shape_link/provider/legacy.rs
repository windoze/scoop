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
            self.parts.production.core_shapes(),
            subject,
            meter,
        )
    }

    pub(crate) fn reject_legacy_subject(
        ordinary: &CrossConeLirBridgeSectionV1,
        core: &CoreLirBridgeBranchV1,
        core_shapes: &CoreShapeSupportPlanV1,
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
                meter.charge_work(core.callables().len() as u64 + 1, &path)?;
                if core
                    .callables()
                    .iter()
                    .any(|record| record.target() == target)
                    || core.initialization_cycle_thrower().target() == target
                {
                    return Err(ShapeLinkError::LegacyPartition(subject));
                }
            }
        }
        if let Some(core) = core_shapes.core() {
            meter.charge_work((core.closures().len() as u64).saturating_mul(16), &path)?;
            if core
                .closures()
                .iter()
                .any(|closure| legacy_shape(closure.roles(), subject))
            {
                return Err(ShapeLinkError::LegacyPartition(subject));
            }
        }
        Ok(())
    }
}

fn legacy_shape(
    roles: &ParamFreeShapeSupportRolesV1,
    subject: ExternalStrongShapeSubjectV1,
) -> bool {
    use ExternalStrongShapeSubjectV1 as Subject;
    let root = match subject {
        Subject::Layout(id) => roles
            .value_layout()
            .available()
            .is_some_and(|record| record.semantic_id() == id),
        Subject::Scan(id) => roles
            .ref_scan()
            .available()
            .is_some_and(|record| record.semantic_id() == id),
        Subject::TypeDescriptor(id) => roles
            .type_descriptor()
            .available()
            .is_some_and(|record| record.semantic_id() == id),
        Subject::TypeRegistration(id) => roles
            .type_registration()
            .available()
            .is_some_and(|record| record.semantic_id() == id),
        _ => false,
    };
    root || [
        roles.boxed_value(),
        roles.coroutine_step(),
        roles.coroutine_slot(),
    ]
    .into_iter()
    .filter_map(ShapeSupportAvailabilityV1::available)
    .any(|helper| match subject {
        Subject::Layout(id) => helper.layout().semantic_id() == id,
        Subject::Scan(id) => helper.scan().semantic_id() == id,
        Subject::TypeDescriptor(id) => helper.descriptor().semantic_id() == id,
        Subject::TypeRegistration(id) => helper.registration().semantic_id() == id,
        _ => false,
    })
}
