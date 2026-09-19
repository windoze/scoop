use super::*;
use scoop_identity::{PersistentDispatchTableId, PersistentExactTypeId};

impl<'a> ShapeLinkProviderV1<'a> {
    pub(super) fn descriptor(
        &self,
        exact: PersistentExactTypeId,
        subject: ExternalStrongShapeSubjectV1,
        physical: StrongShapeDefinitionRefV1,
        meter: &mut BudgetMeter,
    ) -> Result<&'a ExactDescriptorExportV1, ShapeLinkError> {
        let path = WirePath::root();
        meter.charge_work(
            (self.parts.descriptors.records().len() as u64)
                .saturating_add(self.parts.production.types().registrations().len() as u64),
            &path,
        )?;
        let record = self
            .parts
            .descriptors
            .get(exact)
            .ok_or(ShapeLinkError::MissingSubject(subject))?;
        let registration = self
            .parts
            .production
            .types()
            .registrations()
            .iter()
            .find(|registration| registration.exact_type() == exact)
            .ok_or(ShapeLinkError::DefinitionRelation(subject))?;
        let semantic = registration.semantic();
        let (definition, symbol, atom) =
            if matches!(subject, ExternalStrongShapeSubjectV1::TypeDescriptor(_)) {
                (
                    registration.descriptor_definition_plan(),
                    registration.descriptor_symbol(),
                    registration.descriptor_primary_atom(),
                )
            } else {
                (
                    registration.definition_plan(),
                    registration.symbol(),
                    registration.primary_atom(),
                )
            };
        if definition != physical.definition()
            || symbol != physical.symbol()
            || atom != physical.primary()
        {
            return Err(ShapeLinkError::DefinitionRelation(subject));
        }
        crate::exact_descriptor::validate_registration_plan(
            registration,
            self.parts.layouts,
            self.parts.foundation,
            meter,
        )
        .map_err(ShapeLinkError::DescriptorPlan)?;
        meter.charge_work(
            (self.parts.layouts.records().len() as u64)
                .saturating_mul(2)
                .saturating_add(semantic.diagnostic_name().len() as u64)
                .saturating_add(record.diagnostic_name().as_str().len() as u64)
                .saturating_add(semantic.itables().len() as u64),
            &path,
        )?;
        let instance_record = self
            .parts
            .layouts
            .get(record.instance_layout().identity().layout())
            .ok_or(ShapeLinkError::DefinitionRelation(subject))?;
        let value_record = self
            .parts
            .layouts
            .get(record.value_layout().identity().layout())
            .ok_or(ShapeLinkError::DefinitionRelation(subject))?;
        let ExactLayoutBodyKindV1::Instance(instance) = instance_record.kind() else {
            return Err(ShapeLinkError::DefinitionRelation(subject));
        };
        if !matches!(value_record.kind(), ExactLayoutBodyKindV1::Value(_))
            || value_record.identity().exact() != exact
            || instance_record.identity().exact() != exact
        {
            return Err(ShapeLinkError::DefinitionRelation(subject));
        }
        charge_scan(semantic.instance_shape().object_scan(), 1, meter)?;
        charge_scan(record.shape().object_scan(), 1, meter)?;
        charge_scan(semantic.instance_shape().inline_scan(), 1, meter)?;
        charge_scan(record.shape().inline_scan(), 1, meter)?;
        if semantic.instance_layout() != instance_record.identity().layout()
            || semantic.instance_scan() != instance_record.scan()
            || semantic.instance_shape() != instance.shape()
            || record.shape() != instance.shape()
            || record.object_scan() != instance.shape().object_scan()
            || semantic.diagnostic_name() != record.diagnostic_name().as_str()
            || semantic.parent() != record.ancestry().parent()
            || semantic.vtable().table() != record.dispatch().vtable()
            || semantic.itables().len() != record.dispatch().itables().len()
            || semantic.itables().len() != record.ancestry().interfaces().len()
        {
            return Err(ShapeLinkError::Contract);
        }
        crate::exact_descriptor::validate_inline_scan(
            semantic,
            instance_record,
            instance,
            self.parts.layouts,
        )
        .map_err(ShapeLinkError::DescriptorPlan)?;
        self.dispatch_slots(semantic.vtable().table(), semantic.vtable().slots(), meter)?;
        for ((actual, expected), interface) in record
            .dispatch()
            .itables()
            .iter()
            .zip(semantic.itables())
            .zip(record.ancestry().interfaces())
        {
            if actual.interface() != expected.interface()
                || actual.table() != expected.table()
                || *interface != expected.interface()
            {
                return Err(ShapeLinkError::Contract);
            }
            self.dispatch_slots(expected.table(), expected.slots(), meter)?;
        }
        Ok(record)
    }

    pub(super) fn dispatch(
        &self,
        table: PersistentDispatchTableId,
        physical: StrongShapeDefinitionRefV1,
        meter: &mut BudgetMeter,
    ) -> Result<&'a ExactDispatchExportV1, ShapeLinkError> {
        let subject = ExternalStrongShapeSubjectV1::DispatchTable(table);
        let path = WirePath::root();
        meter.charge_work(
            (self.parts.dispatch.records().len() as u64)
                .saturating_add(self.parts.production.types().registrations().len() as u64),
            &path,
        )?;
        let record = self
            .parts
            .dispatch
            .get(table)
            .ok_or(ShapeLinkError::MissingSubject(subject))?;
        if record.physical_definition() != physical {
            return Err(ShapeLinkError::DefinitionRelation(subject));
        }
        let owner = self
            .parts
            .production
            .types()
            .registrations()
            .iter()
            .find(|registration| registration.exact_type() == record.owner_exact())
            .ok_or(ShapeLinkError::DefinitionRelation(subject))?;
        let semantic = owner.semantic();
        match record.role() {
            ExactDispatchRoleV1::Vtable if semantic.vtable().table() == table => {
                self.dispatch_slots(table, semantic.vtable().slots(), meter)?
            }
            ExactDispatchRoleV1::Itable { interface_exact } => {
                meter.charge_work(semantic.itables().len() as u64, &path)?;
                let itable = semantic
                    .itables()
                    .iter()
                    .find(|item| {
                        item.table() == table && item.interface().exact_type() == interface_exact
                    })
                    .ok_or(ShapeLinkError::DefinitionRelation(subject))?;
                self.dispatch_slots(table, itable.slots(), meter)?;
            }
            _ => return Err(ShapeLinkError::DefinitionRelation(subject)),
        }
        Ok(record)
    }

    fn dispatch_slots(
        &self,
        table: PersistentDispatchTableId,
        slots: &[StrongTypeDispatchCallableRefV2],
        meter: &mut BudgetMeter,
    ) -> Result<(), ShapeLinkError> {
        meter.charge_work(
            (slots.len() as u64).saturating_add(self.parts.dispatch.records().len() as u64),
            &WirePath::root(),
        )?;
        let record = self
            .parts
            .dispatch
            .get(table)
            .ok_or(ShapeLinkError::MissingSubject(
                ExternalStrongShapeSubjectV1::DispatchTable(table),
            ))?;
        if record.entries().len() != slots.len()
            || !record
                .entries()
                .iter()
                .zip(slots)
                .all(|(entry, slot)| entry.abi() == *slot)
        {
            return Err(ShapeLinkError::Contract);
        }
        Ok(())
    }
}

pub(super) fn charge_scan(
    scan: &RefScan,
    depth: u64,
    meter: &mut BudgetMeter,
) -> Result<(), ShapeLinkError> {
    let path = WirePath::root();
    meter.check_semantic_depth(depth, &path)?;
    meter.charge_nodes(1, &path)?;
    meter.charge_work(1, &path)?;
    match scan {
        RefScan::None => {}
        RefScan::References(offsets) => meter.charge_work(offsets.len() as u64, &path)?,
        RefScan::Sequence(parts) => {
            for part in parts {
                charge_scan(part, depth.saturating_add(1), meter)?;
            }
        }
        RefScan::Array { element, .. } => {
            charge_scan(element.as_ref_scan(), depth.saturating_add(1), meter)?
        }
    }
    Ok(())
}
