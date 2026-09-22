use super::*;
use scoop_identity::PersistentPropertyAccessorId;
use std::collections::BTreeSet;

pub(super) fn project(
    export: &hir::ExportHir,
    source: &hir::CrossConeTypeSemanticsProductionV1,
    required: &mut BTreeMap<Declaration, SourceContract>,
    meter: &mut BudgetMeter,
) -> Result<(), Error> {
    let mut extra = property_accessors(source, meter)?;
    for (declaration, implementation, attributes) in export
        .property_getters
        .iter()
        .map(|(id, getter)| {
            (
                export.property_accessor_identities[id].id(),
                getter.implementation,
                getter.attributes,
            )
        })
        .chain(export.property_setters.iter().map(|(id, setter)| {
            (
                export.property_accessor_identities[id].id(),
                setter.implementation,
                setter.attributes,
            )
        }))
    {
        let key = Declaration::PropertyAccessor(declaration);
        work(
            u64::from(extra.len().checked_ilog2().unwrap_or(0))
                + u64::from(required.len().checked_ilog2().unwrap_or(0))
                + 3,
            meter,
        )?;
        let selected = extra.remove(&declaration) || required.contains_key(&key);
        if !selected {
            continue;
        }
        let function = match implementation {
            hir::PropertyAccessorImplementation::Storage
            | hir::PropertyAccessorImplementation::Constant => {
                required.remove(&key);
                continue;
            }
            hir::PropertyAccessorImplementation::Body(function)
            | hir::PropertyAccessorImplementation::AbstractSlot(function) => function,
        };
        let function = &export.functions[function];
        let contract = SourceContract {
            execution: scoop_identity::Effect::Ordinary,
            gc: match attributes.gc_effect {
                hir::GcEffect::Managed => mir::GcEffect::Managed,
                hir::GcEffect::NoGc => mir::GcEffect::NoGc,
            },
            modality: modality(export, function),
        };
        inventory::insert(required, key, contract, meter)?;
    }
    if let Some(missing) = extra.first() {
        return Err(Error::MissingSourceContract(Declaration::PropertyAccessor(
            *missing,
        )));
    }
    Ok(())
}

fn modality(export: &hir::ExportHir, function: &hir::Function) -> hir::CallableModalityV1 {
    let Some(method) = function.method else {
        return hir::CallableModalityV1::Final;
    };
    if let hir::MethodDispatch::Interface(id) = method.dispatch {
        return match export.interface_methods[id].implementation {
            hir::InterfaceMemberImplementation::Body => hir::CallableModalityV1::InterfaceDefault,
            hir::InterfaceMemberImplementation::AbstractSlot => hir::CallableModalityV1::Abstract,
        };
    }
    match method.modifier {
        hir::MethodModifier::Final => hir::CallableModalityV1::Final,
        hir::MethodModifier::Open => hir::CallableModalityV1::Open,
        hir::MethodModifier::Abstract => hir::CallableModalityV1::Abstract,
    }
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
