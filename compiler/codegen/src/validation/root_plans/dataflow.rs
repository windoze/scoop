//! Protocol-neutral use/def and CFG facts for root-plan liveness.

use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(super) enum LiveValue {
    Param(u32),
    Local(scoop_lir::LocalId),
    Temp(scoop_lir::TempId),
}

impl LiveValue {
    pub(super) fn from_value(value: Value) -> Option<Self> {
        match value {
            Value::Param(index) => Some(Self::Param(index)),
            Value::Local(id) => Some(Self::Local(id)),
            Value::Temp(id) => Some(Self::Temp(id)),
            Value::CArgumentStorage(storage) => Some(Self::Local(storage.local())),
            Value::IntegerConst(_)
            | Value::MachineScalar(_)
            | Value::BoolConst(_)
            | Value::NullPointer(_)
            | Value::TypeDescriptor(_)
            | Value::RootScan(_)
            | Value::Global(_)
            | Value::InitializationUnit(_) => None,
        }
    }

    pub(super) fn source(self) -> scoop_lir::CallerRootSource {
        match self {
            Self::Param(index) => scoop_lir::CallerRootSource::Param(index),
            Self::Local(id) => scoop_lir::CallerRootSource::Local(id),
            Self::Temp(id) => scoop_lir::CallerRootSource::Temp(id),
        }
    }

    pub(super) fn sort_key(self) -> (u8, u32) {
        match self {
            Self::Param(index) => (0, index),
            Self::Local(id) => (1, id.into_raw().into_u32()),
            Self::Temp(id) => (2, id.into_raw().into_u32()),
        }
    }
}

pub(super) fn instruction_uses(instruction: &Instruction, function: &Function) -> Vec<Value> {
    match instruction {
        Instruction::BoxValue { payload, .. } => {
            payload.source().map(Value::Local).into_iter().collect()
        }
        Instruction::UnboxValue { object, .. } => vec![*object],
        Instruction::BinOp { lhs, rhs, .. }
        | Instruction::IntegerBinary { lhs, rhs, .. }
        | Instruction::SafeIntegerDivRem { lhs, rhs, .. }
        | Instruction::IntegerCompare { lhs, rhs, .. }
        | Instruction::IntegerCompareTo { lhs, rhs, .. } => vec![*lhs, *rhs],
        Instruction::IntegerShift {
            value,
            normalized_count,
            ..
        } => vec![*value, *normalized_count],
        Instruction::UnaryOp { operand, .. }
        | Instruction::IntegerUnary { operand, .. }
        | Instruction::IntegerConvert { operand, .. }
        | Instruction::ExtractValue {
            aggregate: operand, ..
        }
        | Instruction::HeapLoad {
            object: operand, ..
        }
        | Instruction::MachineHeapLoad {
            object: operand, ..
        }
        | Instruction::AtomicLoad {
            object: operand, ..
        }
        | Instruction::ULongToPtr { value: operand, .. }
        | Instruction::PtrToULong { value: operand, .. }
        | Instruction::RawLoad {
            pointer: operand, ..
        }
        | Instruction::BeginCatch { raw: operand, .. }
        | Instruction::Throw { exception: operand }
        | Instruction::ArrayLen { operand, .. }
        | Instruction::ArrayClone { operand, .. }
        | Instruction::EnumTag { operand, .. }
        | Instruction::EnumField { operand, .. }
        | Instruction::VariantTest { operand, .. }
        | Instruction::VariantPayloadProject { operand, .. }
        | Instruction::ForeignCallbackRegister {
            closure: operand, ..
        } => vec![*operand],
        Instruction::ForeignCallbackOperation(operation) => vec![operation.callback()],
        Instruction::MakeAggregate { elements, .. } | Instruction::ArrayAlloc { elements, .. } => {
            elements.clone()
        }
        Instruction::ArrayAssembly { parts, .. } => parts
            .iter()
            .map(|part| match part {
                scoop_lir::ArrayAssemblyPart::Element(value)
                | scoop_lir::ArrayAssemblyPart::CopyArray(value) => *value,
            })
            .collect(),
        Instruction::Store { value, .. }
        | Instruction::GlobalStore { value, .. }
        | Instruction::NativeGlobalStore { value, .. } => vec![*value],
        Instruction::Call { site } => {
            call_uses(site.args(), site.destination(&function.call_targets))
        }
        Instruction::Invoke { site } => {
            call_uses(site.args(), site.destination(&function.call_targets))
        }
        Instruction::HeapStore { object, value, .. }
        | Instruction::MachineHeapStore { object, value, .. }
        | Instruction::AtomicStore { object, value, .. } => vec![*object, *value],
        Instruction::AtomicCompareExchange {
            object,
            expected,
            replacement,
            ..
        } => vec![*object, *expected, *replacement],
        Instruction::RawStore { pointer, value, .. } => vec![*pointer, *value],
        Instruction::PtrOffset {
            pointer,
            element_offset,
            ..
        } => vec![*pointer, *element_offset],
        Instruction::LocalAddress { local, .. } => vec![Value::Local(*local)],
        Instruction::ArrayGet { array, index, .. } => vec![*array, *index],
        Instruction::ArraySet {
            array,
            index,
            value,
            ..
        } => vec![*array, *index, *value],
        Instruction::EnumWrap { fields, .. } => fields.clone(),
        Instruction::GlobalLoad { .. }
        | Instruction::GlobalAddress { .. }
        | Instruction::NativeGlobalLoad { .. }
        | Instruction::NativeGlobalAddress { .. }
        | Instruction::FunctionAddress { .. }
        | Instruction::ManagedPoll { .. }
        | Instruction::LandingPad { .. }
        | Instruction::CleanupPad { .. }
        | Instruction::EndCatch => Vec::new(),
    }
}

fn call_uses(
    arguments: &[scoop_lir::AbiCallArgument],
    destination: scoop_lir::CallDestination,
) -> Vec<Value> {
    let mut values = arguments
        .iter()
        .map(|argument| argument.logical_value())
        .collect::<Vec<_>>();
    if let scoop_lir::CallDestination::Dispatch { table, .. } = destination {
        values.push(table);
    }
    values
}

pub(super) fn instruction_defs(instruction: &Instruction) -> Vec<LiveValue> {
    let out = match instruction {
        Instruction::UnboxValue { result, .. } => {
            return match result {
                scoop_lir::UnboxResult::ZeroSized { out, .. } => vec![LiveValue::Temp(*out)],
                scoop_lir::UnboxResult::NonZero(place) => vec![LiveValue::Local(place.local())],
            };
        }
        Instruction::BoxValue { out, .. }
        | Instruction::BinOp { out, .. }
        | Instruction::UnaryOp { out, .. }
        | Instruction::IntegerUnary { out, .. }
        | Instruction::IntegerBinary { out, .. }
        | Instruction::SafeIntegerDivRem { out, .. }
        | Instruction::IntegerCompare { out, .. }
        | Instruction::IntegerCompareTo { out, .. }
        | Instruction::IntegerShift { out, .. }
        | Instruction::IntegerConvert { out, .. }
        | Instruction::MakeAggregate { out, .. }
        | Instruction::ExtractValue { out, .. }
        | Instruction::HeapLoad { out, .. }
        | Instruction::MachineHeapLoad { out, .. }
        | Instruction::AtomicLoad { out, .. }
        | Instruction::AtomicCompareExchange { out, .. }
        | Instruction::GlobalLoad { out, .. }
        | Instruction::GlobalAddress { out, .. }
        | Instruction::NativeGlobalLoad { out, .. }
        | Instruction::NativeGlobalAddress { out, .. }
        | Instruction::FunctionAddress { out, .. }
        | Instruction::ULongToPtr { out, .. }
        | Instruction::PtrToULong { out, .. }
        | Instruction::RawLoad { out, .. }
        | Instruction::PtrOffset { out, .. }
        | Instruction::LocalAddress { out, .. }
        | Instruction::BeginCatch { out, .. }
        | Instruction::ArrayAlloc { out, .. }
        | Instruction::ArrayAssembly { out, .. }
        | Instruction::ArrayLen { out, .. }
        | Instruction::ArrayGet { out, .. }
        | Instruction::ArrayClone { out, .. }
        | Instruction::EnumWrap { out, .. }
        | Instruction::EnumTag { out, .. }
        | Instruction::EnumField { out, .. }
        | Instruction::VariantTest { out, .. }
        | Instruction::VariantPayloadProject { out, .. }
        | Instruction::ForeignCallbackRegister { out, .. } => Some(*out),
        Instruction::Call { site } => return call_defs(site.result()),
        Instruction::Invoke { site } => return call_defs(site.result()),
        Instruction::ForeignCallbackOperation(operation) => operation.out(),
        Instruction::Store { local, .. } => return vec![LiveValue::Local(*local)],
        Instruction::LandingPad { record, raw } | Instruction::CleanupPad { record, raw } => {
            return vec![LiveValue::Temp(*record), LiveValue::Temp(*raw)];
        }
        Instruction::GlobalStore { .. }
        | Instruction::NativeGlobalStore { .. }
        | Instruction::HeapStore { .. }
        | Instruction::MachineHeapStore { .. }
        | Instruction::AtomicStore { .. }
        | Instruction::RawStore { .. }
        | Instruction::ArraySet { .. }
        | Instruction::ManagedPoll { .. }
        | Instruction::EndCatch
        | Instruction::Throw { .. } => None,
    };
    out.map_or_else(Vec::new, |out| vec![LiveValue::Temp(out)])
}

fn call_defs(result: scoop_lir::TypedCallResult) -> Vec<LiveValue> {
    match result {
        scoop_lir::TypedCallResult::Void => Vec::new(),
        scoop_lir::TypedCallResult::ElidedZst(out) | scoop_lir::TypedCallResult::Direct(out) => {
            vec![LiveValue::Temp(out)]
        }
        scoop_lir::TypedCallResult::IndirectResult(storage) => vec![LiveValue::Local(storage)],
    }
}

pub(super) fn terminator_uses(terminator: &Terminator, mut use_value: impl FnMut(Value)) {
    match terminator {
        Terminator::CondBr { cond, .. } => use_value(*cond),
        Terminator::Return { value: Some(value) } => use_value(*value),
        Terminator::Resume { exception } => use_value(*exception),
        Terminator::Br(_) | Terminator::Return { value: None } | Terminator::Unreachable => {}
    }
}

pub(super) fn block_successors(block: &scoop_lir::BasicBlock) -> Vec<scoop_lir::BlockId> {
    let mut successors = match block.terminator {
        Terminator::Br(target) => vec![target],
        Terminator::CondBr {
            then_block,
            else_block,
            ..
        } => vec![then_block, else_block],
        Terminator::Return { .. } | Terminator::Resume { .. } | Terminator::Unreachable => {
            Vec::new()
        }
    };
    if let Some(Instruction::Invoke { site }) = block.instructions.last() {
        if !successors.contains(&site.normal()) {
            successors.push(site.normal());
        }
        if !successors.contains(&site.unwind()) {
            successors.push(site.unwind());
        }
    }
    successors
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn indirect_result_storage_is_a_call_definition() {
        let storage = scoop_lir::LocalId::from_raw(3.into());
        assert_eq!(
            call_defs(scoop_lir::TypedCallResult::IndirectResult(storage)),
            vec![LiveValue::Local(storage)]
        );
    }
}
