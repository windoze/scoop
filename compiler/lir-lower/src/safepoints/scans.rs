use super::*;

pub(super) fn shift_scan(scan: &lir::RefScan, base: u64) -> lir::RefScan {
    match scan {
        lir::RefScan::None => lir::RefScan::None,
        lir::RefScan::References(offsets) => {
            lir::RefScan::References(offsets.iter().map(|offset| base + offset).collect())
        }
        lir::RefScan::Sequence(parts) => {
            lir::RefScan::Sequence(parts.iter().map(|part| shift_scan(part, base)).collect())
        }
    }
}

pub(super) fn lir_size_align(
    ty: &lir::LirType,
    structs: &Arena<lir::StructDef>,
    enums: &Arena<lir::EnumDef>,
) -> (u64, u64) {
    match ty {
        lir::LirType::Void => (0, 1),
        lir::LirType::I1 => (1, 1),
        lir::LirType::I64 | lir::LirType::Ptr(_) => (8, 8),
        lir::LirType::ExceptionRecord => (16, 8),
        lir::LirType::Aggregate(fields) => {
            let (_, size, align) = lir_aggregate_shape(fields, structs, enums);
            (size, align)
        }
        lir::LirType::Struct(id) => (structs[*id].size, structs[*id].align),
        lir::LirType::Enum(id) => repr_shape(&enums[*id].repr),
    }
}

pub(super) fn lir_aggregate_shape(
    fields: &[lir::LirType],
    structs: &Arena<lir::StructDef>,
    enums: &Arena<lir::EnumDef>,
) -> (Vec<u64>, u64, u64) {
    let mut offsets = Vec::with_capacity(fields.len());
    let mut size = 0u64;
    let mut align = 1u64;
    for field in fields {
        let (field_size, field_align) = lir_size_align(field, structs, enums);
        size = size.next_multiple_of(field_align);
        offsets.push(size);
        size += field_size;
        align = align.max(field_align);
    }
    (offsets, size.next_multiple_of(align), align)
}

pub(crate) fn root_scan(
    ty: &lir::LirType,
    structs: &Arena<lir::StructDef>,
    enums: &Arena<lir::EnumDef>,
    base: u64,
) -> lir::RefScan {
    match ty {
        lir::LirType::Ptr(lir::PointerKind::Managed) => lir::RefScan::References(vec![base]),
        lir::LirType::Aggregate(fields) => {
            let (offsets, _, _) = lir_aggregate_shape(fields, structs, enums);
            sequence(
                fields
                    .iter()
                    .zip(offsets)
                    .map(|(field, offset)| root_scan(field, structs, enums, base + offset)),
            )
        }
        lir::LirType::Struct(id) => sequence(
            structs[*id]
                .fields
                .iter()
                .map(|field| root_scan(&field.ty, structs, enums, base + field.layout.offset)),
        ),
        lir::LirType::Enum(id) => shift_scan(&enums[*id].scan, base),
        lir::LirType::Void
        | lir::LirType::I1
        | lir::LirType::I64
        | lir::LirType::Ptr(_)
        | lir::LirType::ExceptionRecord => lir::RefScan::None,
    }
}

pub(super) fn live_value_ty(value: LiveValue, function: &lir::Function) -> &lir::LirType {
    match value {
        LiveValue::Param(index) => &function.params[index as usize],
        LiveValue::Local(id) => &function.locals[id].ty,
        LiveValue::Temp(id) => &function.temps[id].ty,
    }
}

pub(super) fn sorted_live(live: &HashSet<LiveValue>) -> Vec<LiveValue> {
    let mut values = live.iter().copied().collect::<Vec<_>>();
    values.sort_by_key(|value| value.sort_key());
    values
}

pub(super) fn caller_roots(
    live: &HashSet<LiveValue>,
    function: &lir::Function,
    structs: &Arena<lir::StructDef>,
    enums: &Arena<lir::EnumDef>,
) -> Vec<lir::CallerRoot> {
    sorted_live(live)
        .into_iter()
        .filter_map(|value| caller_root(value, function, structs, enums))
        .collect()
}

pub(super) fn caller_root(
    value: LiveValue,
    function: &lir::Function,
    structs: &Arena<lir::StructDef>,
    enums: &Arena<lir::EnumDef>,
) -> Option<lir::CallerRoot> {
    let scan = root_scan(live_value_ty(value, function), structs, enums, 0);
    lir::NonEmptyRefScan::new(scan).map(|scan| lir::CallerRoot {
        source: value.source(),
        scan,
    })
}

pub(super) fn statepoint_live_set(
    live: &HashSet<LiveValue>,
    function: &lir::Function,
    structs: &Arena<lir::StructDef>,
    enums: &Arena<lir::EnumDef>,
) -> lir::StatepointLiveSet {
    let values = sorted_live(live)
        .into_iter()
        .filter_map(|value| {
            let ty = live_value_ty(value, function).clone();
            let scan = root_scan(&ty, structs, enums, 0);
            let mut offsets = Vec::new();
            flatten_scan(&scan, &mut offsets);
            offsets.sort_unstable();
            offsets.dedup();
            let leaves = lir::ManagedLeafPaths::new(
                offsets
                    .into_iter()
                    .map(|byte_offset| lir::ManagedLeafPath { byte_offset })
                    .collect(),
            )?;
            Some(lir::StatepointLiveValue {
                source: value.source(),
                ty,
                leaves,
            })
        })
        .collect();
    lir::StatepointLiveSet::new(values)
        .expect("sorted_live produces one strictly ordered entry per source")
}

pub(super) fn include_managed_operands(
    roots: &mut HashSet<LiveValue>,
    operands: impl IntoIterator<Item = lir::Value>,
    function: &lir::Function,
    structs: &Arena<lir::StructDef>,
    enums: &Arena<lir::EnumDef>,
) {
    for operand in operands {
        if let Some(value) = LiveValue::from_value(operand)
            && root_scan(live_value_ty(value, function), structs, enums, 0).contains_reference()
        {
            roots.insert(value);
        }
    }
}

pub(super) fn exceptional_root_set(
    instruction: &lir::Instruction,
    site: &lir::ManagedInvokeSite,
    live_in: &[HashSet<LiveValue>],
    function: &lir::Function,
    structs: &Arena<lir::StructDef>,
    enums: &Arena<lir::EnumDef>,
) -> lir::ExceptionalRootSet {
    let mut normal = live_in[arena_index(site.normal)].clone();
    for definition in instruction_defs(instruction) {
        normal.remove(&definition);
    }
    let unwind = &live_in[arena_index(site.unwind)];
    let mut roots = normal.union(unwind).copied().collect::<HashSet<_>>();
    include_managed_operands(
        &mut roots,
        site.call.args().iter().copied(),
        function,
        structs,
        enums,
    );
    lir::ExceptionalRootSet::new(
        sorted_live(&roots)
            .into_iter()
            .filter_map(|value| {
                caller_root(value, function, structs, enums).map(|root| lir::ExceptionalRoot {
                    root,
                    normal_live: normal.contains(&value),
                    unwind_live: unwind.contains(&value),
                })
            })
            .collect(),
    )
}

pub(super) fn flatten_scan(scan: &lir::RefScan, offsets: &mut Vec<u64>) {
    match scan {
        lir::RefScan::None => {}
        lir::RefScan::References(references) => offsets.extend(references),
        lir::RefScan::Sequence(parts) => {
            for part in parts {
                flatten_scan(part, offsets);
            }
        }
    }
}
