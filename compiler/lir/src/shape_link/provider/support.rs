use super::*;
use crate::shape_link::ShapeLinkSupportSourceV1;

impl<'a> ShapeLinkProviderV1<'a> {
    pub(super) fn support_contract(
        &self,
        subject: ExternalStrongShapeSubjectV1,
        physical: StrongShapeDefinitionRefV1,
        support: &dyn ShapeLinkSupportAuthorityV1<'a>,
        meter: &mut BudgetMeter,
    ) -> Result<ShapeLinkContractV1<'a>, ShapeLinkError> {
        use ExternalStrongShapeSubjectV1 as Subject;
        let source = support
            .support_source(self.provider(), subject, meter)?
            .ok_or(ShapeLinkError::SupportRelation(subject))?;
        let path = WirePath::root();
        let unit = match source {
            ShapeLinkSupportSourceV1::Initialization { unit }
            | ShapeLinkSupportSourceV1::StaticStorage { unit, .. } => unit,
        };
        meter.charge_work(
            self.parts.production.units().registrations().len() as u64,
            &path,
        )?;
        let registration = self
            .parts
            .production
            .units()
            .registrations()
            .iter()
            .find(|registration| registration.semantic().unit() == unit.unit())
            .ok_or(ShapeLinkError::SupportRelation(subject))?;
        // The hook names a relation, not a replacement payload. The contract
        // always borrows the actual provider's complete V2 semantic plan.
        charge_unit(unit, meter)?;
        charge_unit(registration.semantic(), meter)?;
        if unit != registration.semantic() {
            return Err(ShapeLinkError::SupportRelation(subject));
        }
        match (subject, source) {
            (
                Subject::InitializationCell(id) | Subject::InitializationDescriptor(id),
                ShapeLinkSupportSourceV1::Initialization { .. },
            ) => {
                let (definition, symbol, atom) =
                    if matches!(subject, Subject::InitializationCell(_)) {
                        (
                            registration.cell_definition_plan(),
                            registration.cell_symbol(),
                            registration.cell_primary_atom(),
                        )
                    } else {
                        (
                            registration.descriptor_definition_plan(),
                            registration.descriptor_symbol(),
                            registration.descriptor_primary_atom(),
                        )
                    };
                if id != unit.unit()
                    || definition != physical.definition()
                    || symbol != physical.symbol()
                    || atom != physical.primary()
                {
                    return Err(ShapeLinkError::DefinitionRelation(subject));
                }
                Ok(ShapeLinkContractV1::Initialization {
                    unit_projection: registration.semantic(),
                })
            }
            (
                Subject::StaticStorage(id) | Subject::StaticStorageRegistration(id),
                ShapeLinkSupportSourceV1::StaticStorage { storage, .. },
            ) => {
                if storage.storage() != id || (unit.storage() != id && unit.failure_root() != id) {
                    return Err(ShapeLinkError::SupportRelation(subject));
                }
                meter.charge_work(
                    self.parts.production.storages().registrations().len() as u64,
                    &path,
                )?;
                let registration = self
                    .parts
                    .production
                    .storages()
                    .registrations()
                    .iter()
                    .find(|registration| registration.semantic().storage() == id)
                    .ok_or(ShapeLinkError::SupportRelation(subject))?;
                let (definition, symbol, atom) = if matches!(subject, Subject::StaticStorage(_)) {
                    (
                        registration.storage_definition_plan(),
                        registration.semantic().symbol(),
                        registration.storage_primary_atom(),
                    )
                } else {
                    (
                        registration.registration_definition_plan(),
                        registration.registration_symbol(),
                        registration.registration_primary_atom(),
                    )
                };
                charge_storage(storage, meter)?;
                charge_storage(registration.semantic(), meter)?;
                if storage != registration.semantic()
                    || definition != physical.definition()
                    || symbol != physical.symbol()
                    || atom != physical.primary()
                {
                    return Err(ShapeLinkError::DefinitionRelation(subject));
                }
                Ok(ShapeLinkContractV1::StaticStorage {
                    storage_projection: registration.semantic(),
                })
            }
            _ => Err(ShapeLinkError::SupportRelation(subject)),
        }
    }
}

fn charge_unit(
    unit: &StrongInitializationUnitSemanticPlanV2,
    meter: &mut BudgetMeter,
) -> Result<(), ShapeLinkError> {
    meter.charge_work(
        (unit.diagnostic_path().len() as u64)
            .saturating_add((unit.dependencies().len() as u64).saturating_mul(32))
            .saturating_add(8),
        &WirePath::root(),
    )?;
    Ok(())
}
fn charge_storage(
    storage: &StrongStaticStorageSemanticPlanV1,
    meter: &mut BudgetMeter,
) -> Result<(), ShapeLinkError> {
    super::types::charge_scan(storage.scan_program(), 1, meter)?;
    meter.charge_work(
        (storage.initial_state().initial_template().len() as u64)
            .saturating_add(
                (storage.initial_state().immortal_relocations().len() as u64).saturating_mul(3),
            )
            .saturating_add(10),
        &WirePath::root(),
    )?;
    Ok(())
}
