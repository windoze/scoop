use super::*;
use crate::{
    CallableModalityV1, CheckedProtectedCallableSourceV1, ProtectedCallableSemanticAuthority,
};
use scoop_identity::{AccessorRole, CallableTemplateOrigin, PropertyOwner};

pub(super) fn validate_key<A: ProtectedCallableSemanticAuthority<E>, E>(
    property: PersistentPropertyId,
    accessor: PersistentPropertyAccessorId,
    role: AccessorRole,
    authority: &A,
    meter: &mut BudgetMeter,
) -> Result<(), ProtectedPropertySemanticError<E>> {
    use ProtectedPropertySemanticError as Error;
    let key = authority
        .property_accessor_key(accessor)
        .map_err(Error::Foundation)?;
    meter
        .charge_sha256(
            scoop_wire::encoded_length(key).map_err(Error::Encoding)?,
            &WirePath::root(),
        )
        .map_err(Error::Resource)?;
    if PersistentPropertyAccessorId::from_key(key).ok() != Some(accessor)
        || key.owner() != PropertyOwner::Property(property)
        || key.role() != role
    {
        return Err(Error::Accessor);
    }
    Ok(())
}

impl CheckedProtectedPropertySourceV1<'_> {
    /// Joins checked protected callable records. Restricted setters deliberately
    /// have no entry here; their source domain remains in the property payload.
    pub fn validate_accessor_contracts(
        &self,
        getter: CheckedProtectedCallableSourceV1<'_>,
        setter: Option<CheckedProtectedCallableSourceV1<'_>>,
        meter: &mut BudgetMeter,
    ) -> Result<(), ProtectedPropertyAccessorClosureError> {
        self.validate_accessor_views(
            AccessorView::checked(&getter),
            setter.as_ref().map(AccessorView::checked),
            meter,
        )
    }

    pub(in crate::cross_cone_type_semantics::protected_interfaces) fn validate_resolved_accessor_records(
        &self,
        getter: &crate::ProtectedCallableInterfaceV1,
        setter: Option<&crate::ProtectedCallableInterfaceV1>,
        meter: &mut BudgetMeter,
    ) -> Result<(), ProtectedPropertyAccessorClosureError> {
        self.validate_accessor_views(
            AccessorView::record(getter),
            setter.map(AccessorView::record),
            meter,
        )
    }

    fn validate_accessor_views(
        &self,
        getter: AccessorView<'_>,
        setter: Option<AccessorView<'_>>,
        meter: &mut BudgetMeter,
    ) -> Result<(), ProtectedPropertyAccessorClosureError> {
        use ProtectedPropertyAccessorClosureError as Error;
        let property = self.record().payload();
        meter
            .charge_nodes(1, &WirePath::root())
            .map_err(Error::Resource)?;
        meter
            .charge_work(
                scoop_wire::encoded_length(property.value_type())
                    .map_err(Error::Encoding)?
                    .saturating_mul(2),
                &WirePath::root(),
            )
            .map_err(Error::Resource)?;
        if getter.declaration != CallableTemplateOrigin::Accessor(property.getter()) {
            return Err(Error::Getter);
        }
        check_common(
            self.record(),
            getter,
            self.record().declaration_access(),
            meter,
        )?;
        if !getter.payload.parameters().is_empty()
            || getter.payload.result() != property.value_type()
        {
            return Err(Error::Signature);
        }
        match (property.mutability(), setter) {
            (
                ProtectedPropertyMutabilityV1::ReadWrite {
                    setter: expected,
                    setter_access,
                },
                Some(setter),
            ) if setter_access.declared_visibility() == DeclaredVisibilityV1::Protected => {
                if setter.declaration != CallableTemplateOrigin::Accessor(*expected) {
                    return Err(Error::Setter);
                }
                check_common(self.record(), setter, setter_access, meter)?;
                if setter.payload.parameters().parameters().len() != 1
                    || setter.payload.parameters().parameters()[0].value_type()
                        != property.value_type()
                {
                    return Err(Error::Signature);
                }
            }
            (ProtectedPropertyMutabilityV1::ReadOnly, None) => {}
            (ProtectedPropertyMutabilityV1::ReadWrite { setter_access, .. }, None)
                if setter_access.declared_visibility() != DeclaredVisibilityV1::Protected => {}
            _ => return Err(Error::Setter),
        }
        Ok(())
    }
}

fn check_common(
    property: &ProtectedPropertyInterfaceV1,
    accessor: AccessorView<'_>,
    source: &DeclarationAccessSourceV1,
    meter: &mut BudgetMeter,
) -> Result<(), ProtectedPropertyAccessorClosureError> {
    use ProtectedPropertyAccessorClosureError as Error;
    let slot_count = accessor.payload.slot_relations().slots().len() as u64;
    let table_count = property.payload().slot_relations().slots().len() as u64;
    let work = (source.lexical_owners().len() as u64)
        .saturating_add(slot_count.saturating_mul(table_count.saturating_add(1)));
    meter
        .charge_work(work, &WirePath::root())
        .map_err(Error::Resource)?;
    let actual = accessor.access;
    if accessor.payload.owner() != property.payload().owner()
        || actual.declared_visibility() != source.declared_visibility()
        || actual.lexical_owners() != source.lexical_owners()
        || actual.definition_origin().origin().source()
            != source.definition_origin().origin().source()
    {
        return Err(Error::Access);
    }
    if (property.payload().representation() == PropertyRepresentationV1::AbstractSlot)
        != (accessor.payload.modality() == CallableModalityV1::Abstract)
    {
        return Err(Error::Representation);
    }
    for slot in accessor.payload.slot_relations().slots() {
        if property
            .payload()
            .slot_relations()
            .slots()
            .binary_search(slot)
            .is_err()
        {
            return Err(Error::Slot);
        }
    }
    Ok(())
}

#[derive(Clone, Copy)]
struct AccessorView<'a> {
    declaration: CallableTemplateOrigin,
    payload: &'a crate::ProtectedCallablePayloadV1,
    access: &'a DeclarationAccessSourceV1,
}
impl<'a> AccessorView<'a> {
    fn checked(value: &'a CheckedProtectedCallableSourceV1<'a>) -> Self {
        Self {
            declaration: value.declaration(),
            payload: value.payload(),
            access: value.declaration_access().source(),
        }
    }
    fn record(value: &'a crate::ProtectedCallableInterfaceV1) -> Self {
        Self {
            declaration: value.declaration(),
            payload: value.payload(),
            access: value.declaration_access(),
        }
    }
}
