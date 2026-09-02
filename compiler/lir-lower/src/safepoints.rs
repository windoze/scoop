//! Explicit safepoint placement and complete root-plan construction.
//!
//! This pass runs after one function's CFG and physical LIR types are final.
//! It inserts managed entry/back-edge polls, computes backward liveness once,
//! and fills every protocol-specific root plan. Codegen consumes these plans;
//! it never rediscovers CFG safepoints or source liveness.

use std::collections::HashSet;

use la_arena::{Arena, Idx};
use scoop_lir as lir;

use super::metadata::{repr_shape, sequence};

#[derive(Debug)]
pub(super) struct SafepointIds {
    next: u64,
}

impl Default for SafepointIds {
    fn default() -> Self {
        Self { next: 1 }
    }
}

impl SafepointIds {
    pub(super) fn allocate(&mut self) -> lir::SafepointId {
        let raw = self.next;
        self.next = raw
            .checked_add(1)
            .expect("a Scoop image cannot contain u64::MAX safepoints");
        lir::SafepointId::new(raw).expect("SafepointIds starts at one")
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum LiveValue {
    Param(u32),
    Local(lir::LocalId),
    Temp(lir::TempId),
}

impl LiveValue {
    fn from_value(value: lir::Value) -> Option<Self> {
        match value {
            lir::Value::Param(index) => Some(Self::Param(index)),
            lir::Value::Local(id) => Some(Self::Local(id)),
            lir::Value::Temp(id) => Some(Self::Temp(id)),
            lir::Value::IntConst(_)
            | lir::Value::BoolConst(_)
            | lir::Value::NullPointer(_)
            | lir::Value::TypeDescriptor(_)
            | lir::Value::RootScan(_)
            | lir::Value::Global(_) => None,
        }
    }

    fn source(self) -> lir::CallerRootSource {
        match self {
            Self::Param(index) => lir::CallerRootSource::Param(index),
            Self::Local(id) => lir::CallerRootSource::Local(id),
            Self::Temp(id) => lir::CallerRootSource::Temp(id),
        }
    }

    fn sort_key(self) -> (u8, u32) {
        match self {
            Self::Param(index) => (0, index),
            Self::Local(id) => (1, id.into_raw().into_u32()),
            Self::Temp(id) => (2, id.into_raw().into_u32()),
        }
    }
}

pub(super) fn complete_function(
    function: &mut lir::Function,
    structs: &Arena<lir::StructDef>,
    enums: &Arena<lir::EnumDef>,
    ids: &mut SafepointIds,
) {
    fold_constant_branches(function);
    prune_unreachable_blocks(function);
    insert_polls(function, ids);
    annotate_root_plans(function, structs, enums);
}

fn fold_constant_branches(function: &mut lir::Function) {
    for block in function.blocks.values_mut() {
        let target = match &block.terminator {
            lir::Terminator::CondBr {
                cond: lir::Value::BoolConst(true),
                then_block,
                ..
            } => *then_block,
            lir::Terminator::CondBr {
                cond: lir::Value::BoolConst(false),
                else_block,
                ..
            } => *else_block,
            _ => continue,
        };
        block.terminator = lir::Terminator::Br(target);
    }
}

/// Canonicalize the final LIR CFG before assigning any safepoint identity.
/// MIR lowering can leave detached EH and coroutine continuation blocks after
/// control-flow simplification. LLVM is allowed to delete those blocks, so
/// retaining them would make the supposedly exact LIR statepoint manifest
/// describe code that cannot occur in the emitted image.
fn prune_unreachable_blocks(function: &mut lir::Function) {
    let mut reachable = vec![false; function.blocks.len()];
    let mut worklist = vec![function.entry];
    while let Some(block_id) = worklist.pop() {
        let index = arena_index(block_id);
        if reachable[index] {
            continue;
        }
        reachable[index] = true;
        worklist.extend(block_successors(&function.blocks[block_id]));
    }

    if reachable.iter().all(|reachable| *reachable) {
        return;
    }

    let old_blocks = std::mem::take(&mut function.blocks);
    let mut new_blocks = Arena::with_capacity(reachable.iter().filter(|value| **value).count());
    let mut block_map = vec![None; reachable.len()];
    for (old_id, block) in old_blocks {
        if reachable[arena_index(old_id)] {
            let new_id = new_blocks.alloc(block);
            block_map[arena_index(old_id)] = Some(new_id);
        }
    }

    function.entry = remap_block(function.entry, &block_map);
    for block in new_blocks.values_mut() {
        remap_block_targets(block, &block_map);
    }
    function.blocks = new_blocks;
}

fn remap_block(old: lir::BlockId, block_map: &[Option<lir::BlockId>]) -> lir::BlockId {
    block_map[arena_index(old)].expect("every edge from a reachable block is reachable")
}

fn remap_block_targets(block: &mut lir::BasicBlock, block_map: &[Option<lir::BlockId>]) {
    match &mut block.terminator {
        lir::Terminator::Br(target) => *target = remap_block(*target, block_map),
        lir::Terminator::CondBr {
            then_block,
            else_block,
            ..
        } => {
            *then_block = remap_block(*then_block, block_map);
            *else_block = remap_block(*else_block, block_map);
        }
        lir::Terminator::Return { .. }
        | lir::Terminator::Resume { .. }
        | lir::Terminator::Unreachable => {}
    }

    for instruction in &mut block.instructions {
        let lir::Instruction::Invoke { site } = instruction else {
            continue;
        };
        match site {
            lir::InvokeSite::Managed(site) => {
                site.normal = remap_block(site.normal, block_map);
                site.unwind = remap_block(site.unwind, block_map);
            }
            lir::InvokeSite::NoGc(site) => {
                site.normal = remap_block(site.normal, block_map);
                site.unwind = remap_block(site.unwind, block_map);
            }
        }
    }
}

fn insert_polls(function: &mut lir::Function, ids: &mut SafepointIds) {
    if function.gc_effect == lir::GcEffect::NoGc {
        return;
    }
    let headers = loop_headers(function);
    let signature = function
        .call_targets
        .void_signatures
        .alloc(lir::VoidCallSignature {
            params: Vec::new(),
            calling_convention: lir::CallingConvention::Cdecl,
        });
    let poll_target = function
        .call_targets
        .managed_targets
        .void
        .alloc(lir::CallTarget {
            destination: lir::ManagedCallDestination::runtime(
                lir::ManagedRuntimeFunction::Safepoint,
            ),
            signature,
        });
    for (block_id, block) in function.blocks.iter_mut() {
        if block_id != function.entry && !headers[arena_index(block_id)] {
            continue;
        }
        let insertion = usize::from(matches!(
            block.instructions.first(),
            Some(lir::Instruction::LandingPad { .. } | lir::Instruction::CleanupPad { .. })
        ));
        block.instructions.insert(
            insertion,
            lir::Instruction::ManagedPoll {
                site: lir::ManagedPollSite {
                    target: poll_target,
                    safepoint: ids.allocate(),
                    live: lir::StatepointLiveSet::default(),
                },
            },
        );
    }
}

fn loop_headers(function: &lir::Function) -> Vec<bool> {
    let len = function.blocks.len();
    let mut successors = vec![Vec::new(); len];
    for (id, block) in function.blocks.iter() {
        let from = arena_index(id);
        match block.terminator {
            lir::Terminator::Br(target) => successors[from].push(arena_index(target)),
            lir::Terminator::CondBr {
                then_block,
                else_block,
                ..
            } => {
                successors[from].push(arena_index(then_block));
                successors[from].push(arena_index(else_block));
            }
            lir::Terminator::Return { .. }
            | lir::Terminator::Resume { .. }
            | lir::Terminator::Unreachable => {}
        }
        if let Some(lir::Instruction::Invoke { site }) = block.instructions.last() {
            successors[from].push(arena_index(site.unwind()));
        }
    }

    let mut predecessors = vec![Vec::new(); len];
    for (from, targets) in successors.iter().enumerate() {
        for &to in targets {
            predecessors[to].push(from);
        }
    }

    let entry = arena_index(function.entry);
    let mut dominators: Vec<HashSet<usize>> = vec![(0..len).collect(); len];
    dominators[entry] = [entry].into_iter().collect();
    loop {
        let mut changed = false;
        for block in 0..len {
            if block == entry {
                continue;
            }
            let mut next: HashSet<usize> = match predecessors[block].as_slice() {
                [] => [block].into_iter().collect(),
                [first, rest @ ..] => {
                    let mut set = dominators[*first].clone();
                    for predecessor in rest {
                        set.retain(|candidate| dominators[*predecessor].contains(candidate));
                    }
                    set
                }
            };
            next.insert(block);
            if next != dominators[block] {
                dominators[block] = next;
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }

    let mut headers = vec![false; len];
    for (from, targets) in successors.iter().enumerate() {
        for &to in targets {
            if dominators[from].contains(&to) {
                headers[to] = true;
            }
        }
    }
    headers
}

fn instruction_uses(instruction: &lir::Instruction, function: &lir::Function) -> Vec<lir::Value> {
    match instruction {
        lir::Instruction::BinOp { lhs, rhs, .. } => vec![*lhs, *rhs],
        lir::Instruction::UnaryOp { operand, .. }
        | lir::Instruction::ExtractValue {
            aggregate: operand, ..
        }
        | lir::Instruction::HeapLoad {
            object: operand, ..
        }
        | lir::Instruction::AtomicLoad {
            object: operand, ..
        }
        | lir::Instruction::IntToPtr { value: operand, .. }
        | lir::Instruction::PtrToInt { value: operand, .. }
        | lir::Instruction::RawLoad {
            pointer: operand, ..
        }
        | lir::Instruction::BeginCatch { raw: operand, .. }
        | lir::Instruction::Throw { exception: operand }
        | lir::Instruction::ArrayLen { operand, .. }
        | lir::Instruction::ArrayClone { operand, .. }
        | lir::Instruction::EnumTag { operand, .. }
        | lir::Instruction::EnumField { operand, .. }
        | lir::Instruction::ForeignCallbackRegister {
            closure: operand, ..
        } => vec![*operand],
        lir::Instruction::ForeignCallbackOperation(operation) => vec![operation.callback()],
        lir::Instruction::MakeAggregate { elements, .. }
        | lir::Instruction::ArrayAlloc { elements, .. } => elements.clone(),
        lir::Instruction::Store { value, .. }
        | lir::Instruction::GlobalStore { value, .. }
        | lir::Instruction::NativeGlobalStore { value, .. } => vec![*value],
        lir::Instruction::Call { site } => {
            call_uses(site.args(), site.destination(&function.call_targets))
        }
        lir::Instruction::Invoke { site } => {
            call_uses(site.args(), site.destination(&function.call_targets))
        }
        lir::Instruction::HeapStore { object, value, .. }
        | lir::Instruction::AtomicStore { object, value, .. } => vec![*object, *value],
        lir::Instruction::AtomicCompareExchange {
            object,
            expected,
            replacement,
            ..
        } => vec![*object, *expected, *replacement],
        lir::Instruction::RawStore { pointer, value, .. } => vec![*pointer, *value],
        lir::Instruction::PtrOffset { pointer, bytes, .. } => vec![*pointer, *bytes],
        lir::Instruction::LocalAddress { local, .. } => vec![lir::Value::Local(*local)],
        lir::Instruction::ArrayGet { array, index, .. } => vec![*array, *index],
        lir::Instruction::ArraySet {
            array,
            index,
            value,
            ..
        } => vec![*array, *index, *value],
        lir::Instruction::EnumWrap { fields, .. } => fields.clone(),
        lir::Instruction::GlobalLoad { .. }
        | lir::Instruction::GlobalAddress { .. }
        | lir::Instruction::NativeGlobalLoad { .. }
        | lir::Instruction::NativeGlobalAddress { .. }
        | lir::Instruction::FunctionAddress { .. }
        | lir::Instruction::ManagedPoll { .. }
        | lir::Instruction::LandingPad { .. }
        | lir::Instruction::CleanupPad { .. }
        | lir::Instruction::EndCatch => Vec::new(),
    }
}

fn call_uses(args: &[lir::Value], destination: lir::CallDestination) -> Vec<lir::Value> {
    let mut values = args.to_vec();
    if let lir::CallDestination::Dispatch { table, .. } = destination {
        values.push(table);
    }
    values
}

fn instruction_defs(instruction: &lir::Instruction) -> Vec<LiveValue> {
    let out = match instruction {
        lir::Instruction::BinOp { out, .. }
        | lir::Instruction::UnaryOp { out, .. }
        | lir::Instruction::MakeAggregate { out, .. }
        | lir::Instruction::ExtractValue { out, .. }
        | lir::Instruction::HeapLoad { out, .. }
        | lir::Instruction::AtomicLoad { out, .. }
        | lir::Instruction::AtomicCompareExchange { out, .. }
        | lir::Instruction::GlobalLoad { out, .. }
        | lir::Instruction::GlobalAddress { out, .. }
        | lir::Instruction::NativeGlobalLoad { out, .. }
        | lir::Instruction::NativeGlobalAddress { out, .. }
        | lir::Instruction::FunctionAddress { out, .. }
        | lir::Instruction::IntToPtr { out, .. }
        | lir::Instruction::PtrToInt { out, .. }
        | lir::Instruction::RawLoad { out, .. }
        | lir::Instruction::PtrOffset { out, .. }
        | lir::Instruction::LocalAddress { out, .. }
        | lir::Instruction::BeginCatch { out, .. }
        | lir::Instruction::ArrayAlloc { out, .. }
        | lir::Instruction::ArrayLen { out, .. }
        | lir::Instruction::ArrayGet { out, .. }
        | lir::Instruction::ArrayClone { out, .. }
        | lir::Instruction::EnumWrap { out, .. }
        | lir::Instruction::EnumTag { out, .. }
        | lir::Instruction::EnumField { out, .. }
        | lir::Instruction::ForeignCallbackRegister { out, .. } => Some(*out),
        lir::Instruction::Call { site } => return call_defs(site.result()),
        lir::Instruction::Invoke { site } => return call_defs(site.result()),
        lir::Instruction::ForeignCallbackOperation(operation) => operation.out(),
        lir::Instruction::Store { local, .. } => return vec![LiveValue::Local(*local)],
        lir::Instruction::LandingPad { record, raw }
        | lir::Instruction::CleanupPad { record, raw } => {
            return vec![LiveValue::Temp(*record), LiveValue::Temp(*raw)];
        }
        lir::Instruction::GlobalStore { .. }
        | lir::Instruction::NativeGlobalStore { .. }
        | lir::Instruction::HeapStore { .. }
        | lir::Instruction::AtomicStore { .. }
        | lir::Instruction::RawStore { .. }
        | lir::Instruction::ArraySet { .. }
        | lir::Instruction::ManagedPoll { .. }
        | lir::Instruction::EndCatch
        | lir::Instruction::Throw { .. } => None,
    };
    out.map_or_else(Vec::new, |out| vec![LiveValue::Temp(out)])
}

fn call_defs(result: lir::TypedCallResult) -> Vec<LiveValue> {
    match result {
        lir::TypedCallResult::Void => Vec::new(),
        lir::TypedCallResult::Direct(out) => vec![LiveValue::Temp(out)],
        lir::TypedCallResult::IndirectResult(storage) => vec![LiveValue::Local(storage)],
    }
}

fn terminator_uses(terminator: &lir::Terminator, mut use_value: impl FnMut(lir::Value)) {
    match terminator {
        lir::Terminator::CondBr { cond, .. } => use_value(*cond),
        lir::Terminator::Return { value: Some(value) } => use_value(*value),
        lir::Terminator::Resume { exception } => use_value(*exception),
        lir::Terminator::Br(_)
        | lir::Terminator::Return { value: None }
        | lir::Terminator::Unreachable => {}
    }
}

fn block_successors(block: &lir::BasicBlock) -> Vec<lir::BlockId> {
    let mut successors = match block.terminator {
        lir::Terminator::Br(target) => vec![target],
        lir::Terminator::CondBr {
            then_block,
            else_block,
            ..
        } => vec![then_block, else_block],
        lir::Terminator::Return { .. }
        | lir::Terminator::Resume { .. }
        | lir::Terminator::Unreachable => Vec::new(),
    };
    if let Some(lir::Instruction::Invoke { site }) = block.instructions.last() {
        if !successors.contains(&site.normal()) {
            successors.push(site.normal());
        }
        if !successors.contains(&site.unwind()) {
            successors.push(site.unwind());
        }
    }
    successors
}

fn shift_scan(scan: &lir::RefScan, base: u64) -> lir::RefScan {
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

fn lir_size_align(
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

fn lir_aggregate_shape(
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

pub(super) fn root_scan(
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

fn live_value_ty(value: LiveValue, function: &lir::Function) -> &lir::LirType {
    match value {
        LiveValue::Param(index) => &function.params[index as usize],
        LiveValue::Local(id) => &function.locals[id].ty,
        LiveValue::Temp(id) => &function.temps[id].ty,
    }
}

fn sorted_live(live: &HashSet<LiveValue>) -> Vec<LiveValue> {
    let mut values = live.iter().copied().collect::<Vec<_>>();
    values.sort_by_key(|value| value.sort_key());
    values
}

fn caller_roots(
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

fn caller_root(
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

fn statepoint_live_set(
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

fn include_managed_operands(
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

fn exceptional_root_set(
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

fn flatten_scan(scan: &lir::RefScan, offsets: &mut Vec<u64>) {
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

#[derive(Debug, Clone)]
enum RootPlan {
    None,
    Statepoint(lir::StatepointLiveSet),
    Exceptional(lir::ExceptionalRootSet),
    NativeSafe(lir::NativeSafeRootSet),
    NativeBorrowed(Vec<lir::CallerRoot>),
}

fn annotate_root_plans(
    function: &mut lir::Function,
    structs: &Arena<lir::StructDef>,
    enums: &Arena<lir::EnumDef>,
) {
    let block_count = function.blocks.len();
    let mut uses = vec![HashSet::new(); block_count];
    let mut defs = vec![HashSet::new(); block_count];
    let mut successors = vec![Vec::new(); block_count];

    for (id, block) in function.blocks.iter() {
        let index = arena_index(id);
        let mut block_defs = HashSet::new();
        let mut block_uses = HashSet::new();
        for instruction in &block.instructions {
            for value in instruction_uses(instruction, function) {
                if let Some(value) = LiveValue::from_value(value)
                    && !block_defs.contains(&value)
                {
                    block_uses.insert(value);
                }
            }
            block_defs.extend(instruction_defs(instruction));
        }
        terminator_uses(&block.terminator, |value| {
            if let Some(value) = LiveValue::from_value(value)
                && !block_defs.contains(&value)
            {
                block_uses.insert(value);
            }
        });
        uses[index] = block_uses;
        defs[index] = block_defs;
        successors[index] = block_successors(block);
    }

    let mut live_in = vec![HashSet::new(); block_count];
    let mut live_out = vec![HashSet::new(); block_count];
    loop {
        let mut changed = false;
        for index in (0..block_count).rev() {
            let mut next_out = HashSet::new();
            for successor in &successors[index] {
                next_out.extend(live_in[arena_index(*successor)].iter().copied());
            }
            let mut next_in = uses[index].clone();
            next_in.extend(next_out.difference(&defs[index]).copied());
            if next_out != live_out[index] || next_in != live_in[index] {
                live_out[index] = next_out;
                live_in[index] = next_in;
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }

    let mut plans: Vec<Vec<RootPlan>> = function
        .blocks
        .iter()
        .map(|(_, block)| vec![RootPlan::None; block.instructions.len()])
        .collect();
    for (id, block) in function.blocks.iter() {
        let block_index = arena_index(id);
        let mut live = live_out[block_index].clone();
        terminator_uses(&block.terminator, |value| {
            if let Some(value) = LiveValue::from_value(value) {
                live.insert(value);
            }
        });
        for (instruction_index, instruction) in block.instructions.iter().enumerate().rev() {
            for definition in instruction_defs(instruction) {
                live.remove(&definition);
            }
            plans[block_index][instruction_index] = match instruction {
                lir::Instruction::ManagedPoll { .. } => {
                    RootPlan::Statepoint(statepoint_live_set(&live, function, structs, enums))
                }
                lir::Instruction::ArrayAlloc { elements, .. } => {
                    // Array allocation is expanded in codegen: element values
                    // remain live after the collecting slow-path call until
                    // they are stored into the new object.
                    let mut allocation_live = live.clone();
                    for element in elements {
                        if let Some(value) = LiveValue::from_value(*element)
                            && root_scan(live_value_ty(value, function), structs, enums, 0)
                                .contains_reference()
                        {
                            allocation_live.insert(value);
                        }
                    }
                    RootPlan::Statepoint(statepoint_live_set(
                        &allocation_live,
                        function,
                        structs,
                        enums,
                    ))
                }
                lir::Instruction::ArrayClone { operand, .. } => {
                    let mut roots = live.clone();
                    include_managed_operands(&mut roots, [*operand], function, structs, enums);
                    RootPlan::Statepoint(statepoint_live_set(&roots, function, structs, enums))
                }
                lir::Instruction::Call { site } => match site {
                    lir::CallSite::Managed(_) => {
                        let mut roots = live.clone();
                        include_managed_operands(
                            &mut roots,
                            site.args().iter().copied(),
                            function,
                            structs,
                            enums,
                        );
                        RootPlan::Statepoint(statepoint_live_set(&roots, function, structs, enums))
                    }
                    lir::CallSite::NoGc(_) => RootPlan::None,
                    lir::CallSite::NativeSafe(_) => RootPlan::NativeSafe(
                        lir::NativeSafeRootSet::new(caller_roots(&live, function, structs, enums)),
                    ),
                    lir::CallSite::NativeBorrowed(site) => {
                        let mut borrowed_live = live.clone();
                        for argument in site.call.args() {
                            if let Some(value) = LiveValue::from_value(*argument)
                                && root_scan(live_value_ty(value, function), structs, enums, 0)
                                    .contains_reference()
                            {
                                borrowed_live.insert(value);
                            }
                        }
                        RootPlan::NativeBorrowed(caller_roots(
                            &borrowed_live,
                            function,
                            structs,
                            enums,
                        ))
                    }
                },
                lir::Instruction::Invoke { site } => match site {
                    lir::InvokeSite::Managed(site) => RootPlan::Exceptional(exceptional_root_set(
                        instruction,
                        site,
                        &live_in,
                        function,
                        structs,
                        enums,
                    )),
                    lir::InvokeSite::NoGc(_) => RootPlan::None,
                },
                lir::Instruction::NativeGlobalLoad { .. }
                | lir::Instruction::NativeGlobalStore { .. }
                | lir::Instruction::NativeGlobalAddress { .. } => RootPlan::NativeSafe(
                    lir::NativeSafeRootSet::new(caller_roots(&live, function, structs, enums)),
                ),
                _ => RootPlan::None,
            };
            for value in instruction_uses(instruction, function) {
                if let Some(value) = LiveValue::from_value(value) {
                    live.insert(value);
                }
            }
        }
    }

    for (id, block) in function.blocks.iter_mut() {
        let block_index = arena_index(id);
        for (instruction_index, instruction) in block.instructions.iter_mut().enumerate() {
            let plan =
                std::mem::replace(&mut plans[block_index][instruction_index], RootPlan::None);
            match (instruction, plan) {
                (lir::Instruction::ManagedPoll { site }, RootPlan::Statepoint(live)) => {
                    site.live = live;
                }
                (
                    lir::Instruction::ArrayAlloc {
                        live: instruction_live,
                        ..
                    }
                    | lir::Instruction::ArrayClone {
                        live: instruction_live,
                        ..
                    },
                    RootPlan::Statepoint(live),
                ) => *instruction_live = live,
                (
                    lir::Instruction::Call {
                        site: lir::CallSite::Managed(site),
                    },
                    RootPlan::Statepoint(live),
                ) => site.live = live,
                (
                    lir::Instruction::Call {
                        site: lir::CallSite::NativeSafe(site),
                    },
                    RootPlan::NativeSafe(roots),
                ) => site.roots = roots,
                (
                    lir::Instruction::Call {
                        site: lir::CallSite::NativeBorrowed(site),
                    },
                    RootPlan::NativeBorrowed(roots),
                ) => site.roots = lir::NativeBorrowedRootSet::new(roots),
                (
                    lir::Instruction::Invoke {
                        site: lir::InvokeSite::Managed(site),
                    },
                    RootPlan::Exceptional(roots),
                ) => site.roots = roots,
                (
                    lir::Instruction::NativeGlobalLoad { roots, .. }
                    | lir::Instruction::NativeGlobalStore { roots, .. }
                    | lir::Instruction::NativeGlobalAddress { roots, .. },
                    RootPlan::NativeSafe(computed),
                ) => *roots = computed,
                (
                    lir::Instruction::Call {
                        site: lir::CallSite::NoGc(_),
                    },
                    RootPlan::None,
                )
                | (
                    lir::Instruction::Invoke {
                        site: lir::InvokeSite::NoGc(_),
                    },
                    RootPlan::None,
                )
                | (_, RootPlan::None) => {}
                (_, unexpected) => {
                    panic!("root-plan kind does not match LIR instruction: {unexpected:?}")
                }
            }
        }
    }
}

fn arena_index<T>(id: Idx<T>) -> usize {
    id.into_raw().into_u32() as usize
}
