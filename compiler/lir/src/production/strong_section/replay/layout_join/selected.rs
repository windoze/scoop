use super::*;
use crate::{
    ExternalStrongShapeSubjectV1 as Subject, LayoutAbiSemanticTargetV1 as Target,
    StrongInitializationDependencyKindV2,
};
use scoop_wire::WirePath;

pub(super) fn validate(
    production: &ReplayedStrongProductionSectionV2,
    selected: &crate::SelectedDependencyLayoutAbiSetV1<'_>,
    meter: &mut BudgetMeter,
) -> Result<(), StrongProductionLayoutJoinError> {
    meter.charge_work(1, &WirePath::root())?;
    if selected.consumer() != production.type_registrations().producer() {
        return Err(StrongProductionLayoutJoinError::Provider {
            expected: production.type_registrations().producer(),
            actual: selected.consumer(),
            component: "selected_layout_abi",
        });
    }
    for registration in production.type_registrations().registrations() {
        let semantic = registration.semantic();
        let itable_slots = semantic.itables().iter().fold(0_u64, |count, table| {
            count.saturating_add(table.slots().len() as u64)
        });
        meter.charge_work(
            (semantic.itables().len() as u64)
                .saturating_add(semantic.vtable().slots().len() as u64)
                .saturating_add(itable_slots)
                .saturating_add(1),
            &WirePath::root(),
        )?;
        if let Some(reference) = semantic.parent() {
            descriptor(reference, selected, meter)?;
        }
        for table in semantic.itables() {
            descriptor(table.interface(), selected, meter)?;
        }
        for reference in semantic
            .vtable()
            .slots()
            .iter()
            .chain(semantic.itables().iter().flat_map(|table| table.slots()))
        {
            callable(*reference, selected, meter)?;
        }
    }
    for registration in production.initialization_registrations().registrations() {
        meter.charge_work(
            (registration.semantic().dependencies().len() as u64).saturating_add(1),
            &WirePath::root(),
        )?;
        for dependency in registration.semantic().dependencies() {
            initialization(dependency, selected, meter)?;
        }
    }
    Ok(())
}

fn descriptor(
    reference: crate::StrongTypeDescriptorRefV2,
    selected: &crate::SelectedDependencyLayoutAbiSetV1<'_>,
    meter: &mut BudgetMeter,
) -> Result<(), StrongProductionLayoutJoinError> {
    let crate::StrongTypeDescriptorRefV2::DependencyExternal { provider, exact } = reference else {
        return Ok(());
    };
    meter.charge_work(selected.len() as u64, &WirePath::root())?;
    if selected
        .reference(provider, Target::Descriptor(exact))
        .is_none()
    {
        return Err(StrongProductionLayoutJoinError::MissingSelectedDescriptor { provider, exact });
    }
    meter.charge_work(
        selected.physical_imports().records().len() as u64,
        &WirePath::root(),
    )?;
    let found = selected.physical_imports().records().iter().any(|import| {
        import.provider() == provider && import.subject() == Subject::TypeDescriptor(exact)
    });
    if !found {
        return Err(StrongProductionLayoutJoinError::MissingPhysicalDescriptor { provider, exact });
    }
    Ok(())
}

fn callable(
    reference: crate::StrongTypeDispatchCallableRefV2,
    selected: &crate::SelectedDependencyLayoutAbiSetV1<'_>,
    meter: &mut BudgetMeter,
) -> Result<(), StrongProductionLayoutJoinError> {
    let crate::StrongTypeDispatchCallableRefV2::DependencyExternal { provider, body } = reference
    else {
        return Ok(());
    };
    meter.charge_work(
        selected.physical_imports().records().len() as u64,
        &WirePath::root(),
    )?;
    let mut owner = None;
    let mut matches = 0_u32;
    for import in selected.physical_imports().records() {
        let Subject::Callable(candidate) = import.subject() else {
            continue;
        };
        if import.provider() == provider
            && PersistentCallableBodyId::from_key(&CallableBodyKey::strong(candidate))? == body
        {
            owner = Some(candidate);
            matches = matches.saturating_add(1);
        }
    }
    let owner = match (matches, owner) {
        (0, _) => {
            return Err(StrongProductionLayoutJoinError::MissingPhysicalCallable {
                provider,
                body,
            });
        }
        (1, Some(owner)) => owner,
        _ => {
            return Err(StrongProductionLayoutJoinError::AmbiguousPhysicalCallable {
                provider,
                body,
            });
        }
    };
    meter.charge_work(selected.len() as u64, &WirePath::root())?;
    if selected
        .reference(provider, Target::Callable(owner))
        .is_none()
    {
        return Err(StrongProductionLayoutJoinError::MissingSelectedCallable { provider, body });
    }
    Ok(())
}

fn initialization(
    reference: &crate::StrongInitializationDependencyRefV2,
    selected: &crate::SelectedDependencyLayoutAbiSetV1<'_>,
    meter: &mut BudgetMeter,
) -> Result<(), StrongProductionLayoutJoinError> {
    let StrongInitializationDependencyKindV2::DependencyExternalUnit { provider, unit_ref } =
        reference.kind()
    else {
        return Ok(());
    };
    let unit = unit_ref.unit();
    meter.charge_work(
        selected.physical_imports().records().len() as u64,
        &WirePath::root(),
    )?;
    let Some(import) = selected.physical_imports().records().iter().find(|import| {
        import.provider() == provider && import.subject() == Subject::InitializationDescriptor(unit)
    }) else {
        return Err(
            StrongProductionLayoutJoinError::MissingPhysicalInitialization { provider, unit },
        );
    };
    let crate::ShapeLinkContractV1::Initialization { unit_projection } = import.contract() else {
        return Err(StrongProductionLayoutJoinError::InitializationDefinition { provider, unit });
    };
    if unit_projection.unit() != unit
        || import.required_definition() != unit_ref.descriptor().plan()
        || import.expected_symbol() != unit_ref.descriptor().symbol()
    {
        return Err(StrongProductionLayoutJoinError::InitializationDefinition { provider, unit });
    }
    Ok(())
}

#[cfg(test)]
mod tests;
