use super::scans::flatten_scan;
use super::*;

pub(super) fn live_value_ty(value: LiveValue, function: &lir::Function) -> &lir::LirType {
    match value {
        LiveValue::Param(index) => {
            function.signature.arguments()[index as usize].logical_storage_type()
        }
        LiveValue::Local(id) => function.locals[id].ty(),
        LiveValue::Temp(id) => &function.temps[id].ty,
    }
}

pub(super) fn sorted_live(live: &HashSet<LiveValue>) -> Vec<LiveValue> {
    let mut values = live.iter().copied().collect::<Vec<_>>();
    values.sort_by_key(|value| value.sort_key());
    values
}

pub(super) fn caller_roots(
    context: &LoweringContext,
    live: &HashSet<LiveValue>,
    function: &lir::Function,
    structs: &lir::StructDefs,
    enums: &lir::EnumDefs,
) -> StorageResult<Vec<lir::CallerRoot>> {
    let mut roots = Vec::new();
    for value in sorted_live(live) {
        if let Some(root) = caller_root(context, value, function, structs, enums)? {
            roots.push(root);
        }
    }
    Ok(roots)
}

pub(super) fn caller_root(
    context: &LoweringContext,
    value: LiveValue,
    function: &lir::Function,
    structs: &lir::StructDefs,
    enums: &lir::EnumDefs,
) -> StorageResult<Option<lir::CallerRoot>> {
    let scan = root_scan(context, live_value_ty(value, function), structs, enums, 0)?;
    Ok(lir::NonEmptyRefScan::new(scan).map(|scan| lir::CallerRoot {
        source: value.source(),
        scan,
    }))
}

pub(super) fn statepoint_live_set(
    context: &LoweringContext,
    live: &HashSet<LiveValue>,
    function: &lir::Function,
    structs: &lir::StructDefs,
    enums: &lir::EnumDefs,
) -> StorageResult<lir::StatepointLiveSet> {
    let mut values = Vec::new();
    for value in sorted_live(live) {
        let ty = live_value_ty(value, function).clone();
        let scan = root_scan(context, &ty, structs, enums, 0)?;
        let mut offsets = Vec::new();
        flatten_scan(&scan, &mut offsets)?;
        offsets.sort_unstable();
        offsets.dedup();
        if offsets.is_empty() {
            continue;
        }
        let leaves = lir::ManagedLeafPaths::new(
            offsets
                .into_iter()
                .map(|byte_offset| lir::ManagedLeafPath { byte_offset })
                .collect(),
        )
        .ok_or(StorageLoweringError::InvalidRepresentation(
            "managed leaf paths must be nonempty and strictly ordered",
        ))?;
        values.push(lir::StatepointLiveValue {
            source: value.source(),
            ty,
            leaves,
        });
    }
    lir::StatepointLiveSet::new(values).ok_or(StorageLoweringError::InvalidRepresentation(
        "statepoint sources must be strictly ordered",
    ))
}

pub(super) fn include_managed_operands(
    context: &LoweringContext,
    roots: &mut HashSet<LiveValue>,
    operands: impl IntoIterator<Item = lir::Value>,
    function: &lir::Function,
    structs: &lir::StructDefs,
    enums: &lir::EnumDefs,
) -> StorageResult<()> {
    for operand in operands {
        if let Some(value) = LiveValue::from_value(operand)
            && root_scan(context, live_value_ty(value, function), structs, enums, 0)?
                .contains_reference()
        {
            roots.insert(value);
        }
    }
    Ok(())
}

pub(super) fn exceptional_root_set(
    context: &LoweringContext,
    instruction: &lir::Instruction,
    site: &lir::ManagedInvokeSite,
    live_in: &[HashSet<LiveValue>],
    function: &lir::Function,
    structs: &lir::StructDefs,
    enums: &lir::EnumDefs,
) -> StorageResult<lir::ExceptionalRootSet> {
    let mut normal = live_in[arena_index(site.normal)].clone();
    for definition in instruction_defs(instruction) {
        normal.remove(&definition);
    }
    let unwind = &live_in[arena_index(site.unwind)];
    let mut roots = normal.union(unwind).copied().collect::<HashSet<_>>();
    include_managed_operands(
        context,
        &mut roots,
        site.call
            .args()
            .iter()
            .map(|argument| argument.logical_value()),
        function,
        structs,
        enums,
    )?;
    let mut exceptional = Vec::new();
    for value in sorted_live(&roots) {
        if let Some(root) = caller_root(context, value, function, structs, enums)? {
            exceptional.push(lir::ExceptionalRoot {
                root,
                normal_live: normal.contains(&value),
                unwind_live: unwind.contains(&value),
            });
        }
    }
    Ok(lir::ExceptionalRootSet::new(exceptional))
}
