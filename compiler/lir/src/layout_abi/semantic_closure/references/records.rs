use super::resolve::{descriptor_ref, embedded_callable, instance_layout, target, value_layout};
use super::*;

pub(super) fn descriptor(
    record: &crate::ExactDescriptorExportV1,
    owner: usize,
    depth: u64,
    views: &[&LayoutAbiExportConstituentsV1],
    index: &LayoutAbiTargetIndex,
    pending: &mut Vec<Pending>,
    meter: &mut BudgetMeter,
) -> Result<(), LayoutAbiSemanticClosureError> {
    value_layout(record.value_layout(), depth, views, index, pending, meter)?;
    instance_layout(
        record.instance_layout(),
        depth,
        views,
        index,
        pending,
        meter,
    )?;
    if let Some(parent) = record.ancestry().parent() {
        descriptor_ref(
            parent,
            views[owner].provider(),
            depth,
            views,
            index,
            pending,
            meter,
        )?;
    }
    for interface in record.ancestry().interfaces() {
        descriptor_ref(
            *interface,
            views[owner].provider(),
            depth,
            views,
            index,
            pending,
            meter,
        )?;
    }
    target(
        LayoutAbiSemanticTargetV1::Dispatch(record.dispatch().vtable()),
        Some(views[owner].provider()),
        depth,
        views,
        index,
        pending,
        meter,
    )?;
    for table in record.dispatch().itables() {
        descriptor_ref(
            table.interface(),
            views[owner].provider(),
            depth,
            views,
            index,
            pending,
            meter,
        )?;
        target(
            LayoutAbiSemanticTargetV1::Dispatch(table.table()),
            Some(views[owner].provider()),
            depth,
            views,
            index,
            pending,
            meter,
        )?;
    }
    Ok(())
}

pub(super) fn dispatch(
    record: &crate::ExactDispatchExportV1,
    owner: usize,
    depth: u64,
    views: &[&LayoutAbiExportConstituentsV1],
    index: &LayoutAbiTargetIndex,
    pending: &mut Vec<Pending>,
    meter: &mut BudgetMeter,
) -> Result<(), LayoutAbiSemanticClosureError> {
    target(
        LayoutAbiSemanticTargetV1::Descriptor(record.owner_exact()),
        Some(views[owner].provider()),
        depth,
        views,
        index,
        pending,
        meter,
    )?;
    if let crate::ExactDispatchRoleV1::Itable { interface_exact } = record.role() {
        target(
            LayoutAbiSemanticTargetV1::Descriptor(interface_exact),
            None,
            depth,
            views,
            index,
            pending,
            meter,
        )?;
    }
    for entry in record.entries() {
        callable(entry.callable_abi(), depth, views, index, pending, meter)?;
        if let Some(layout) = entry.slot_receiver_layout() {
            value_layout(layout, depth, views, index, pending, meter)?;
        }
    }
    Ok(())
}

pub(super) fn callable(
    record: &crate::ExactCallableAbiExportV1,
    depth: u64,
    views: &[&LayoutAbiExportConstituentsV1],
    index: &LayoutAbiTargetIndex,
    pending: &mut Vec<Pending>,
    meter: &mut BudgetMeter,
) -> Result<(), LayoutAbiSemanticClosureError> {
    embedded_callable(record, depth, views, index, pending, meter)?;
    if let Some(value) = record.layout_dependencies().receiver().value() {
        value_layout(value, depth, views, index, pending, meter)?;
    }
    for value in record.layout_dependencies().parameters() {
        value_layout(value, depth, views, index, pending, meter)?;
    }
    value_layout(
        record.layout_dependencies().result(),
        depth,
        views,
        index,
        pending,
        meter,
    )
}

pub(super) fn shape_support(
    record: &crate::ParamFreeShapeSupportExportV1,
    depth: u64,
    views: &[&LayoutAbiExportConstituentsV1],
    index: &LayoutAbiTargetIndex,
    pending: &mut Vec<Pending>,
    meter: &mut BudgetMeter,
) -> Result<(), LayoutAbiSemanticClosureError> {
    let provider = record.provider();
    let roles = record.roles();
    if let Some(layout) = roles.value_layout().available() {
        target(
            LayoutAbiSemanticTargetV1::Layout(layout.semantic_id()),
            Some(provider),
            depth,
            views,
            index,
            pending,
            meter,
        )?;
    }
    if let Some(descriptor) = roles.type_descriptor().available() {
        target(
            LayoutAbiSemanticTargetV1::Descriptor(descriptor.semantic_id()),
            Some(provider),
            depth,
            views,
            index,
            pending,
            meter,
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
            depth,
            views,
            index,
            pending,
            meter,
        )?;
        target(
            LayoutAbiSemanticTargetV1::Descriptor(support.descriptor().semantic_id()),
            Some(provider),
            depth,
            views,
            index,
            pending,
            meter,
        )?;
    }
    Ok(())
}
