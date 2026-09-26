use super::*;
use scoop_identity::{PersistentDispatchTableId, PersistentExactTypeId};

impl<'a> ShapeLinkProviderV1<'a> {
    pub(super) fn descriptor(
        &self,
        exact: PersistentExactTypeId,
        subject: ExternalStrongShapeSubjectV1,
        physical: StrongShapeDefinitionRefV1,
    ) -> Result<&'a ExactDescriptorExportV1, ShapeLinkError> {
        let record = self
            .parts
            .descriptors
            .get(exact)
            .ok_or(ShapeLinkError::MissingSubject(subject))?;
        let registration = self
            .parts
            .production
            .type_registrations()
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
        )
        .map_err(ShapeLinkError::DescriptorPlan)?;

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
        self.dispatch_slots(semantic.vtable().table(), semantic.vtable().slots())?;
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
            self.dispatch_slots(expected.table(), expected.slots())?;
        }
        Ok(record)
    }

    pub(super) fn dispatch(
        &self,
        table: PersistentDispatchTableId,
        physical: StrongShapeDefinitionRefV1,
    ) -> Result<&'a ExactDispatchExportV1, ShapeLinkError> {
        let subject = ExternalStrongShapeSubjectV1::DispatchTable(table);

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
            .type_registrations()
            .registrations()
            .iter()
            .find(|registration| registration.exact_type() == record.owner_exact())
            .ok_or(ShapeLinkError::DefinitionRelation(subject))?;
        let semantic = owner.semantic();
        match record.role() {
            ExactDispatchRoleV1::Vtable if semantic.vtable().table() == table => {
                self.dispatch_slots(table, semantic.vtable().slots())?
            }
            ExactDispatchRoleV1::Itable { interface_exact } => {
                let itable = semantic
                    .itables()
                    .iter()
                    .find(|item| {
                        item.table() == table && item.interface().exact_type() == interface_exact
                    })
                    .ok_or(ShapeLinkError::DefinitionRelation(subject))?;
                self.dispatch_slots(table, itable.slots())?;
            }
            _ => return Err(ShapeLinkError::DefinitionRelation(subject)),
        }
        Ok(record)
    }

    fn dispatch_slots(
        &self,
        table: PersistentDispatchTableId,
        slots: &[StrongTypeDispatchCallableRefV2],
    ) -> Result<(), ShapeLinkError> {
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
