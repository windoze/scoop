use super::*;

pub(super) fn instruction_uses(
    instruction: &lir::Instruction,
    function: &lir::Function,
) -> Vec<lir::Value> {
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

pub(super) fn call_uses(args: &[lir::Value], destination: lir::CallDestination) -> Vec<lir::Value> {
    let mut values = args.to_vec();
    if let lir::CallDestination::Dispatch { table, .. } = destination {
        values.push(table);
    }
    values
}

pub(super) fn instruction_defs(instruction: &lir::Instruction) -> Vec<LiveValue> {
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

pub(super) fn call_defs(result: lir::TypedCallResult) -> Vec<LiveValue> {
    match result {
        lir::TypedCallResult::Void => Vec::new(),
        lir::TypedCallResult::Direct(out) => vec![LiveValue::Temp(out)],
        lir::TypedCallResult::IndirectResult(storage) => vec![LiveValue::Local(storage)],
    }
}

pub(super) fn terminator_uses(terminator: &lir::Terminator, mut use_value: impl FnMut(lir::Value)) {
    match terminator {
        lir::Terminator::CondBr { cond, .. } => use_value(*cond),
        lir::Terminator::Return { value: Some(value) } => use_value(*value),
        lir::Terminator::Resume { exception } => use_value(*exception),
        lir::Terminator::Br(_)
        | lir::Terminator::Return { value: None }
        | lir::Terminator::Unreachable => {}
    }
}

pub(super) fn block_successors(block: &lir::BasicBlock) -> Vec<lir::BlockId> {
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
