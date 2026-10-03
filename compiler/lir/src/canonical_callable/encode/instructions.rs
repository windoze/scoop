use super::*;

mod boxing;
mod integers;

impl Writer<'_, '_> {
    pub(super) fn instruction(&mut self, instruction: &Instruction) -> Result {
        match instruction {
            Instruction::PublishReleaseReady { object } => {
                record!(self, 62; self.value(*object))
            }
            Instruction::ReleaseFieldLoad { out, offset } => {
                record!(self, 61; self.temp(*out), self.u(*offset))
            }
            Instruction::BoxValue {
                out,
                payload,
                safepoint,
                live,
            } => {
                record!(self, 1; self.temp(*out), self.box_payload(payload), self.safepoint(*safepoint), self.live(live))
            }
            Instruction::UnboxValue { object, result } => {
                record!(self, 2; self.value(*object), self.unbox_result(result))
            }
            Instruction::BinOp { out, op, lhs, rhs } => {
                record!(self, 3; self.temp(*out), self.binop(*op), self.value(*lhs), self.value(*rhs))
            }
            Instruction::UnaryOp { out, op, operand } => {
                let op = match op {
                    UnOp::Not => 1,
                };
                record!(self, 4; self.temp(*out), self.u(op), self.value(*operand))
            }
            Instruction::IntegerUnary {
                out,
                kind,
                operation,
                operand,
            } => {
                record!(self, 5; self.temp(*out), self.integer_kind(*kind), self.integer_unary(*operation), self.value(*operand))
            }
            Instruction::IntegerBinary {
                out,
                kind,
                operation,
                lhs,
                rhs,
            } => {
                record!(self, 6; self.temp(*out), self.integer_kind(*kind), self.integer_binary(*operation), self.value(*lhs), self.value(*rhs))
            }
            Instruction::SafeIntegerDivRem {
                out,
                kind,
                operation,
                lhs,
                rhs,
            } => {
                record!(self, 7; self.temp(*out), self.integer_kind(*kind), self.integer_divrem(*operation), self.value(*lhs), self.value(*rhs))
            }
            Instruction::IntegerCompare {
                out,
                kind,
                comparison,
                lhs,
                rhs,
            } => {
                record!(self, 8; self.temp(*out), self.integer_kind(*kind), self.integer_comparison(*comparison), self.value(*lhs), self.value(*rhs))
            }
            Instruction::IntegerCompareTo {
                out,
                operand_kind,
                lhs,
                rhs,
            } => {
                record!(self, 9; self.temp(*out), self.integer_kind(*operand_kind), self.value(*lhs), self.value(*rhs))
            }
            Instruction::IntegerShift {
                out,
                kind,
                operation,
                value,
                normalized_count,
            } => {
                record!(self, 10; self.temp(*out), self.integer_kind(*kind), self.integer_shift(*operation), self.value(*value), self.value(*normalized_count))
            }
            Instruction::IntegerConvert {
                out,
                source_kind,
                target_kind,
                operand,
            } => {
                record!(self, 11; self.temp(*out), self.integer_kind(*source_kind), self.integer_kind(*target_kind), self.value(*operand))
            }
            Instruction::MakeZstValue { out, value } => {
                record!(self, 12; self.temp(*out), self.logical_zst(value))
            }
            Instruction::MakeAggregate { out, elements } => {
                record!(self, 13; self.temp(*out), self.values(elements))
            }
            Instruction::ExtractValue {
                out,
                aggregate,
                index,
            } => {
                record!(self, 14; self.temp(*out), self.value(*aggregate), self.u(u64::from(*index)))
            }
            Instruction::HeapLoad {
                out,
                object,
                offset,
            } => record!(self, 15; self.temp(*out), self.value(*object), self.u(*offset)),
            Instruction::MachineHeapLoad {
                out,
                kind,
                object,
                offset,
            } => {
                record!(self, 16; self.temp(*out), self.machine_kind(*kind), self.value(*object), self.u(*offset))
            }
            Instruction::AtomicLoad {
                out,
                kind,
                object,
                offset,
            } => {
                record!(self, 17; self.temp(*out), self.machine_kind(*kind), self.value(*object), self.u(*offset))
            }
            Instruction::Store { local, value } => {
                record!(self, 18; self.local(*local), self.value(*value))
            }
            Instruction::GlobalLoad { out, global } => {
                record!(self, 19; self.temp(*out), self.global(*global))
            }
            Instruction::GlobalStore { global, value } => {
                record!(self, 20; self.global(*global), self.value(*value))
            }
            Instruction::GlobalAddress { out, global } => {
                record!(self, 21; self.temp(*out), self.global(*global))
            }
            Instruction::NativeGlobalLoad {
                out,
                global,
                protocol: NativeStorageProtocol::NativeSafe { safepoint, roots },
            } => {
                record!(self, 22; self.temp(*out), self.native_global(*global), self.safepoint(*safepoint), self.caller_roots(roots.as_slice()))
            }
            Instruction::NativeGlobalStore {
                global,
                value,
                protocol: NativeStorageProtocol::NativeSafe { safepoint, roots },
            } => {
                record!(self, 23; self.native_global(*global), self.value(*value), self.safepoint(*safepoint), self.caller_roots(roots.as_slice()))
            }
            Instruction::NativeGlobalAddress {
                out,
                global,
                protocol: NativeStorageProtocol::NativeSafe { safepoint, roots },
            } => {
                record!(self, 24; self.temp(*out), self.native_global(*global), self.safepoint(*safepoint), self.caller_roots(roots.as_slice()))
            }
            Instruction::NativeGlobalLoad {
                out,
                global,
                protocol: NativeStorageProtocol::NoTransition,
            } => record!(self, 63; self.temp(*out), self.native_global(*global)),
            Instruction::NativeGlobalStore {
                global,
                value,
                protocol: NativeStorageProtocol::NoTransition,
            } => record!(self, 64; self.native_global(*global), self.value(*value)),
            Instruction::NativeGlobalAddress {
                out,
                global,
                protocol: NativeStorageProtocol::NoTransition,
            } => record!(self, 65; self.temp(*out), self.native_global(*global)),
            Instruction::HeapStore {
                object,
                offset,
                value,
            } => record!(self, 25; self.value(*object), self.u(*offset), self.value(*value)),
            Instruction::MachineHeapStore {
                kind,
                object,
                offset,
                value,
            } => {
                record!(self, 26; self.machine_kind(*kind), self.value(*object), self.u(*offset), self.value(*value))
            }
            Instruction::AtomicStore {
                kind,
                object,
                offset,
                value,
            } => {
                record!(self, 27; self.machine_kind(*kind), self.value(*object), self.u(*offset), self.value(*value))
            }
            Instruction::AtomicCompareExchange {
                out,
                kind,
                object,
                offset,
                expected,
                replacement,
            } => {
                record!(self, 28; self.temp(*out), self.machine_kind(*kind), self.value(*object), self.u(*offset), self.value(*expected), self.value(*replacement))
            }
            Instruction::FunctionAddress { out, target } => {
                record!(self, 29; self.temp(*out), self.function_address(*target))
            }
            Instruction::ForeignCallbackRegister {
                out,
                bridge,
                closure,
            } => {
                record!(self, 30; self.temp(*out), self.callback_bridge(*bridge), self.value(*closure))
            }
            Instruction::ForeignCallbackOperation(operation) => {
                record!(self, 31; self.callback_operation(*operation))
            }
            Instruction::ULongToPtr { out, value } => {
                record!(self, 32; self.temp(*out), self.value(*value))
            }
            Instruction::PtrToULong { out, value } => {
                record!(self, 33; self.temp(*out), self.value(*value))
            }
            Instruction::RawLoad {
                out,
                pointer,
                pointee,
            } => record!(self, 34; self.temp(*out), self.value(*pointer), self.abi_value(pointee)),
            Instruction::RawStore {
                pointer,
                value,
                pointee,
            } => {
                record!(self, 35; self.value(*pointer), self.value(*value), self.abi_value(pointee))
            }
            Instruction::PtrOffset {
                out,
                pointer,
                element_offset,
                element_size,
                subtract,
            } => {
                record!(self, 36; self.temp(*out), self.value(*pointer), self.value(*element_offset), self.u(element_size.get()), self.boolean(*subtract))
            }
            Instruction::LocalAddress { out, local } => {
                record!(self, 37; self.temp(*out), self.local(*local))
            }
            Instruction::Call { site } => record!(self, 38; self.call(site)),
            Instruction::ManagedPoll { site } => record!(self, 39; self.poll(site)),
            Instruction::Invoke { site } => record!(self, 40; self.invoke(site)),
            Instruction::LandingPad { record, raw } => {
                record!(self, 41; self.temp(*record), self.temp(*raw))
            }
            Instruction::CleanupPad { record, raw } => {
                record!(self, 42; self.temp(*record), self.temp(*raw))
            }
            Instruction::BeginCatch { out, raw } => {
                record!(self, 43; self.temp(*out), self.value(*raw))
            }
            Instruction::EndCatch => record!(self, 44;),
            Instruction::Throw { exception } => record!(self, 45; self.value(*exception)),
            Instruction::ArrayAlloc {
                out,
                elements,
                array_type,
                safepoint,
                live,
            } => {
                record!(self, 46; self.temp(*out), self.values(elements), self.array_type(*array_type), self.safepoint(*safepoint), self.live(live))
            }
            Instruction::ArrayAssembly {
                out,
                parts,
                overflow_message,
                array_type,
                safepoint,
                live,
            } => {
                // Tag 47 omitted the callable-owned overflow message.
                record!(self, 57; self.temp(*out), self.array_parts(parts), self.array_type(*array_type), self.safepoint(*safepoint), self.live(live), self.global(*overflow_message))
            }
            Instruction::ArrayLen {
                out,
                operand,
                array_type,
            } => {
                record!(self, 48; self.temp(*out), self.value(*operand), self.array_type(*array_type))
            }
            Instruction::ArrayGet {
                out,
                array,
                index,
                array_type,
            } => {
                record!(self, 49; self.temp(*out), self.value(*array), self.value(*index), self.array_type(*array_type))
            }
            Instruction::ArraySet {
                array,
                index,
                value,
                array_type,
            } => {
                record!(self, 50; self.value(*array), self.value(*index), self.value(*value), self.array_type(*array_type))
            }
            Instruction::ArrayClone {
                out,
                operand,
                source_type,
                array_type,
                safepoint,
                live,
            } => {
                record!(self, 51; self.temp(*out), self.value(*operand), self.array_type(*source_type), self.array_type(*array_type), self.safepoint(*safepoint), self.live(live))
            }
            Instruction::EnumWrap {
                out,
                variant,
                fields,
            } => record!(self, 52; self.temp(*out), self.variant(*variant), self.values(fields)),
            Instruction::EnumTag {
                out,
                enum_id,
                operand,
            } => {
                record!(self, 53; self.temp(*out), self.id(&self.module.enums[*enum_id].exact_type), self.value(*operand))
            }
            Instruction::EnumField {
                out,
                enum_id,
                variant,
                index,
                operand,
            } => {
                record!(self, 54; self.temp(*out), self.id(&self.module.enums[*enum_id].exact_type), self.u(u64::from(*variant)), self.u(u64::from(*index)), self.value(*operand))
            }
            Instruction::VariantTest {
                out,
                operand,
                variant,
            } => record!(self, 55; self.temp(*out), self.value(*operand), self.variant(*variant)),
            Instruction::VariantPayloadProject {
                out,
                operand,
                field,
            } => record!(self, 56; self.temp(*out), self.value(*operand), self.field(*field)),
        }
    }

    fn array_parts(&mut self, parts: &[ArrayAssemblyPart]) -> Result {
        self.e.array(parts.len() as u64)?;
        for part in parts {
            match part {
                ArrayAssemblyPart::Element(value) => record!(self, 1; self.value(*value))?,
                ArrayAssemblyPart::CopyArray(value) => record!(self, 2; self.value(*value))?,
            }
        }
        Ok(())
    }

    fn callback_operation(&mut self, operation: ForeignCallbackOperation) -> Result {
        match operation {
            ForeignCallbackOperation::Retain {
                family,
                out,
                callback,
            } => {
                record!(self, 1; self.temp(out), self.callback_family(family), self.value(callback))
            }
            ForeignCallbackOperation::Release { family, callback } => {
                record!(self, 2; self.callback_family(family), self.value(callback))
            }
            ForeignCallbackOperation::State {
                family,
                out,
                callback,
            } => {
                record!(self, 3; self.temp(out), self.callback_family(family), self.value(callback))
            }
            ForeignCallbackOperation::Failure {
                family,
                out,
                callback,
            } => {
                record!(self, 4; self.temp(out), self.callback_family(family), self.value(callback))
            }
        }
    }
}
