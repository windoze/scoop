use super::resolve::{descriptor_ref, embedded_callable, instance_layout, target, value_layout};
use super::*;

pub(super) fn descriptor(
    record: &crate::ExactDescriptorExportV1,
    owner: usize,
    views: &[&LayoutAbiExportConstituentsV1],
    index: &LayoutAbiTargetIndex,
    pending: &mut Vec<Pending>,
) -> Result<(), LayoutAbiSemanticClosureError> {
    value_layout(record.value_layout(), views, index, pending)?;
    instance_layout(record.instance_layout(), views, index, pending)?;
    if let Some(parent) = record.ancestry().parent() {
        descriptor_ref(parent, views[owner].provider(), views, index, pending)?;
    }
    for interface in record.ancestry().interfaces() {
        descriptor_ref(*interface, views[owner].provider(), views, index, pending)?;
    }
    target(
        LayoutAbiSemanticTargetV1::Dispatch(record.dispatch().vtable()),
        Some(views[owner].provider()),
        views,
        index,
        pending,
    )?;
    for table in record.dispatch().itables() {
        descriptor_ref(
            table.interface(),
            views[owner].provider(),
            views,
            index,
            pending,
        )?;
        target(
            LayoutAbiSemanticTargetV1::Dispatch(table.table()),
            Some(views[owner].provider()),
            views,
            index,
            pending,
        )?;
    }
    Ok(())
}

pub(super) fn dispatch(
    record: &crate::ExactDispatchExportV1,
    owner: usize,
    views: &[&LayoutAbiExportConstituentsV1],
    index: &LayoutAbiTargetIndex,
    pending: &mut Vec<Pending>,
) -> Result<(), LayoutAbiSemanticClosureError> {
    target(
        LayoutAbiSemanticTargetV1::Descriptor(record.owner_exact()),
        Some(views[owner].provider()),
        views,
        index,
        pending,
    )?;
    if let crate::ExactDispatchRoleV1::Itable { interface_exact } = record.role() {
        target(
            LayoutAbiSemanticTargetV1::Descriptor(interface_exact),
            None,
            views,
            index,
            pending,
        )?;
    }
    for entry in record.entries() {
        let provider = match entry.abi() {
            crate::StrongTypeDispatchCallableRefV2::Local(_) => views[owner].provider(),
            crate::StrongTypeDispatchCallableRefV2::DependencyExternal { provider, .. } => provider,
            crate::StrongTypeDispatchCallableRefV2::Runtime(_) => continue,
        };
        target(
            LayoutAbiSemanticTargetV1::Callable(entry.implementation().target()),
            Some(provider),
            views,
            index,
            pending,
        )?;
        if let Some(layout) = entry.slot_receiver_layout() {
            value_layout(layout, views, index, pending)?;
        }
    }
    Ok(())
}

pub(super) fn direct_callable(
    record: &crate::ParamFreeLirCallableExportV1,
    views: &[&LayoutAbiExportConstituentsV1],
    index: &LayoutAbiTargetIndex,
    pending: &mut Vec<Pending>,
) -> Result<(), LayoutAbiSemanticClosureError> {
    let signature = record.abi_signature().signature();
    for exact in signature
        .receiver()
        .into_option()
        .into_iter()
        .chain(signature.parameters().iter().copied())
        .chain(std::iter::once(signature.result()))
    {
        let mut found = None;
        for view in views {
            if let Some(layout) = view
                .layouts()
                .find_exact_role(exact, scoop_identity::RepresentationRole::ManagedValue)
            {
                if found.replace(layout).is_some() {
                    return Err(LayoutAbiSemanticClosureError::DuplicateValueLayout(exact));
                }
            }
        }
        let layout = found.ok_or(LayoutAbiSemanticClosureError::MissingValueLayout(exact))?;
        target(
            LayoutAbiSemanticTargetV1::Layout(layout.identity().layout()),
            Some(layout.identity().physical_definition().provider()),
            views,
            index,
            pending,
        )?;
    }
    Ok(())
}

pub(super) fn callable(
    record: &crate::ExactCallableAbiExportV1,
    views: &[&LayoutAbiExportConstituentsV1],
    index: &LayoutAbiTargetIndex,
    pending: &mut Vec<Pending>,
) -> Result<(), LayoutAbiSemanticClosureError> {
    embedded_callable(record, views, index, pending)?;
    if let Some(value) = record.layout_dependencies().receiver().value() {
        value_layout(value, views, index, pending)?;
    }
    for value in record.layout_dependencies().parameters() {
        value_layout(value, views, index, pending)?;
    }
    value_layout(record.layout_dependencies().result(), views, index, pending)
}

pub(super) fn shape_support(
    record: &crate::ParamFreeShapeSupportExportV1,
    views: &[&LayoutAbiExportConstituentsV1],
    index: &LayoutAbiTargetIndex,
    pending: &mut Vec<Pending>,
) -> Result<(), LayoutAbiSemanticClosureError> {
    let provider = record.provider();
    let roles = record.roles();
    if let Some(layout) = roles.value_layout().available() {
        target(
            LayoutAbiSemanticTargetV1::Layout(layout.semantic_id()),
            Some(provider),
            views,
            index,
            pending,
        )?;
    }
    if let Some(descriptor) = roles.type_descriptor().available() {
        target(
            LayoutAbiSemanticTargetV1::Descriptor(descriptor.semantic_id()),
            Some(provider),
            views,
            index,
            pending,
        )?;
    }
    for support in [
        roles.boxed_value().available(),
        roles.coroutine_step().available(),
        roles.coroutine_slot().available(),
    ]
    .into_iter()
    .flatten()
    {
        target(
            LayoutAbiSemanticTargetV1::Layout(support.layout().semantic_id()),
            Some(provider),
            views,
            index,
            pending,
        )?;
        target(
            LayoutAbiSemanticTargetV1::Descriptor(support.descriptor().semantic_id()),
            Some(provider),
            views,
            index,
            pending,
        )?;
    }
    Ok(())
}
