use super::resolve::{descriptor_ref, instance_layout, target, value_layout};
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
        descriptor_ref(parent, views[owner].provider(), index, pending)?;
    }
    for interface in record.ancestry().interfaces() {
        descriptor_ref(*interface, views[owner].provider(), index, pending)?;
    }
    target(
        LayoutAbiSemanticTargetV1::Dispatch(record.dispatch().vtable()),
        Some(views[owner].provider()),
        index,
        pending,
    )?;
    for table in record.dispatch().itables() {
        descriptor_ref(table.interface(), views[owner].provider(), index, pending)?;
        target(
            LayoutAbiSemanticTargetV1::Dispatch(table.table()),
            Some(views[owner].provider()),
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
        index,
        pending,
    )?;
    if let crate::ExactDispatchRoleV1::Itable { interface_exact } = record.role() {
        target(
            LayoutAbiSemanticTargetV1::Descriptor(interface_exact),
            None,
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
            index,
            pending,
        )?;
        if let Some(layout) = entry.slot_receiver_layout() {
            value_layout(layout, views, index, pending)?;
        }
    }
    Ok(())
}

pub(super) fn shape_support(
    record: &crate::ParamFreeShapeSupportExportV1,
    index: &LayoutAbiTargetIndex,
    pending: &mut Vec<Pending>,
) -> Result<(), LayoutAbiSemanticClosureError> {
    let provider = record.provider();
    let roles = record.roles();
    if let Some(layout) = roles.value_layout().available() {
        target(
            LayoutAbiSemanticTargetV1::Layout(layout.semantic_id()),
            Some(provider),
            index,
            pending,
        )?;
    }
    if let Some(descriptor) = roles.type_descriptor().available() {
        target(
            LayoutAbiSemanticTargetV1::Descriptor(descriptor.semantic_id()),
            Some(provider),
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
            index,
            pending,
        )?;
        target(
            LayoutAbiSemanticTargetV1::Descriptor(support.descriptor().semantic_id()),
            Some(provider),
            index,
            pending,
        )?;
    }
    Ok(())
}
