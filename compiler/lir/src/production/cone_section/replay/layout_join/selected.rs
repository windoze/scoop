use super::*;
use crate::{
    ExternalStrongShapeSubjectV1 as Subject, LayoutAbiSemanticTargetV1 as Target,
    StrongInitializationDependencyKindV2,
};

mod view;
pub(super) use view::Selection;

pub(super) fn validate(
    production: &ConeProductionSectionV2,
    selected: Selection<'_, '_>,
) -> Result<(), StrongProductionLayoutJoinError> {
    if selected.consumer() != production.type_registrations().producer() {
        return Err(StrongProductionLayoutJoinError::Provider {
            expected: production.type_registrations().producer(),
            actual: selected.consumer(),
            component: "selected_layout_abi",
        });
    }
    for registration in production.type_registrations().registrations() {
        let semantic = registration.semantic();

        if let Some(reference) = semantic.parent() {
            descriptor(reference, selected)?;
        }
        for table in semantic.itables() {
            descriptor(table.interface(), selected)?;
        }
        for reference in semantic
            .vtable()
            .slots()
            .iter()
            .chain(semantic.itables().iter().flat_map(|table| table.slots()))
        {
            callable(*reference, selected)?;
        }
    }
    for registration in production.static_storage_registrations().registrations() {
        let semantic = registration.semantic();
        if let crate::StaticStorageLayout::External(value) = semantic.value_layout() {
            let provider = semantic.layout_provider();
            let scan = value.scan_definition();
            if !selected.contains(provider, Target::Layout(semantic.layout()))
                || !selected.physical().records().iter().any(|import| {
                    import.provider() == provider
                        && import.subject() == Subject::Scan(semantic.scan())
                        && import.expected_symbol() == scan.symbol()
                        && import.required_definition() == scan.definition()
                })
            {
                return Err(StrongProductionLayoutJoinError::StaticStorageLayout {
                    provider,
                    layout: semantic.layout(),
                });
            }
        }
    }
    for registration in production.initialization_registrations().registrations() {
        for dependency in registration.semantic().dependencies() {
            initialization(dependency, selected)?;
        }
    }
    Ok(())
}

fn descriptor(
    reference: crate::StrongTypeDescriptorRefV2,
    selected: Selection<'_, '_>,
) -> Result<(), StrongProductionLayoutJoinError> {
    let crate::StrongTypeDescriptorRefV2::DependencyExternal { provider, exact } = reference else {
        return Ok(());
    };

    if !selected.contains(provider, Target::Descriptor(exact)) {
        return Err(StrongProductionLayoutJoinError::MissingSelectedDescriptor { provider, exact });
    }

    let found = selected.physical().records().iter().any(|import| {
        import.provider() == provider && import.subject() == Subject::TypeDescriptor(exact)
    });
    if !found {
        return Err(StrongProductionLayoutJoinError::MissingPhysicalDescriptor { provider, exact });
    }
    Ok(())
}

fn callable(
    reference: crate::StrongTypeDispatchCallableRefV2,
    selected: Selection<'_, '_>,
) -> Result<(), StrongProductionLayoutJoinError> {
    let crate::StrongTypeDispatchCallableRefV2::DependencyExternal { provider, body } = reference
    else {
        return Ok(());
    };

    let mut owner = None;
    let mut matches = 0_u32;
    for import in selected.physical().records() {
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
    match (matches, owner) {
        (0, _) => {
            return Err(StrongProductionLayoutJoinError::MissingPhysicalCallable {
                provider,
                body,
            });
        }
        (1, Some(_)) => {}
        _ => {
            return Err(StrongProductionLayoutJoinError::AmbiguousPhysicalCallable {
                provider,
                body,
            });
        }
    };

    Ok(())
}

fn initialization(
    reference: &crate::StrongInitializationDependencyRefV2,
    selected: Selection<'_, '_>,
) -> Result<(), StrongProductionLayoutJoinError> {
    let StrongInitializationDependencyKindV2::DependencyExternalUnit { provider, unit_ref } =
        reference.kind()
    else {
        return Ok(());
    };
    let unit = unit_ref.unit();

    let Some(import) = selected.physical().records().iter().find(|import| {
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
