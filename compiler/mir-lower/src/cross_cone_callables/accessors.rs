use super::*;
use scoop_identity::PersistentPropertyAccessorId;
use std::collections::BTreeSet;

pub(super) fn project(
    public: &hir::CrossConeHirInterfaceSectionV1,
    source: &hir::CrossConeTypeSemanticsProductionV1,
    required: &mut BTreeMap<Declaration, SourceContract>,
    meter: &mut BudgetMeter,
) -> Result<(), Error> {
    let mut extra = property_accessors(source, meter)?;
    let callables = public.callable_interfaces();
    for property in public.property_interfaces().all_declarations() {
        for accessor in std::iter::once(property.accessors().getter_source())
            .chain(property.accessors().setter_source())
        {
            let declaration = accessor.accessor();
            let key = Declaration::PropertyAccessor(declaration);
            work(
                u64::from(extra.len().checked_ilog2().unwrap_or(0))
                    + u64::from(required.len().checked_ilog2().unwrap_or(0))
                    + 3,
                meter,
            )?;
            if !extra.remove(&declaration) && !required.contains_key(&key) {
                continue;
            }
            if !accessor.implementation().requires_body() {
                required.remove(&key);
                continue;
            }
            work(
                u64::from(callables.declaration_count().max(1).ilog2()) + 1,
                meter,
            )?;
            let callable = callables
                .declaration(CallableTemplateOrigin::Accessor(declaration))
                .ok_or(Error::MissingSourceContract(key))?;
            inventory::insert(
                required,
                key,
                SourceContract::new(callable.effects(), callable.modality()),
                meter,
            )?;
        }
    }
    if let Some(missing) = extra.first() {
        return Err(Error::MissingSourceContract(Declaration::PropertyAccessor(
            *missing,
        )));
    }
    Ok(())
}

fn property_accessors(
    source: &hir::CrossConeTypeSemanticsProductionV1,
    meter: &mut BudgetMeter,
) -> Result<BTreeSet<PersistentPropertyAccessorId>, Error> {
    let mut required = BTreeSet::new();
    for property in source.source_properties().records() {
        work(1, meter)?;
        let hir::NominalSupportPropertyPayloadV1::Runtime { interface } = property.payload() else {
            continue;
        };
        if visible(property.declaration_access().declared_visibility()) {
            insert(&mut required, interface.getter(), meter)?;
        }
        if let hir::ProtectedPropertyMutabilityV1::ReadWrite {
            setter,
            setter_access,
        } = interface.mutability()
            && visible(setter_access.declared_visibility())
        {
            insert(&mut required, *setter, meter)?;
        }
    }
    Ok(required)
}

fn visible(visibility: hir::DeclaredVisibilityV1) -> bool {
    matches!(
        visibility,
        hir::DeclaredVisibilityV1::Public | hir::DeclaredVisibilityV1::Protected
    )
}

fn insert(
    required: &mut BTreeSet<PersistentPropertyAccessorId>,
    id: PersistentPropertyAccessorId,
    meter: &mut BudgetMeter,
) -> Result<(), Error> {
    work(
        2 * (u64::from(required.len().checked_ilog2().unwrap_or(0)) + 1),
        meter,
    )?;
    if !required.contains(&id) {
        meter.charge_collection_slots(1, &WirePath::root())?;
        meter.charge_owned_bytes(
            std::mem::size_of::<PersistentPropertyAccessorId>() as u64,
            &WirePath::root(),
        )?;
        required.insert(id);
    }
    Ok(())
}
