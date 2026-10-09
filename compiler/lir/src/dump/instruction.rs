use super::*;

pub(super) fn dump_instruction(
    module: &Module,
    function: &Function,
    instruction: &Instruction,
    buf: &mut String,
) {
    match instruction {
        Instruction::MaybeUninit {
            out,
            wrapper,
            operation,
        } => {
            let (kind, operand) = match operation {
                MaybeUninitOperation::Uninit => ("uninit", String::new()),
                MaybeUninitOperation::Initialized(value) => {
                    ("initialized", format!(" {}", value_name(*value)))
                }
                MaybeUninitOperation::AssumeInit(value) => {
                    ("assume_init", format!(" {}", value_name(*value)))
                }
            };
            buf.push_str(&format!(
                "    local{} = maybe_uninit.{kind} {}{operand} : {}\n",
                out.into_raw(),
                module.structs[*wrapper].name,
                function.locals[*out].ty().dump()
            ));
        }
        Instruction::AtomicLoad { .. }
        | Instruction::AtomicStore { .. }
        | Instruction::AtomicRmw { .. }
        | Instruction::AtomicCmpXchg { .. } => super::atomics::dump_atomic(instruction, buf),
        Instruction::PushPinFrame { out, object } => buf.push_str(&format!(
            "    t{} = push_pin_frame {}\n",
            out.into_raw(),
            value_name(*object)
        )),
        Instruction::PopPinFrame { frame } => {
            buf.push_str(&format!("    pop_pin_frame {}\n", value_name(*frame)))
        }
        Instruction::ArrayDataPointer {
            out,
            object,
            array_type,
        } => buf.push_str(&format!(
            "    t{} = array_data_pointer {} array{}\n",
            out.into_raw(),
            value_name(*object),
            array_type.into_raw()
        )),
        Instruction::StringDataPointer {
            out,
            object,
            byte_offset,
        } => buf.push_str(&format!(
            "    t{} = string_data_pointer {} +{}\n",
            out.into_raw(),
            value_name(*object),
            byte_offset
        )),
        Instruction::BorrowDataLength {
            out,
            object,
            byte_offset,
        } => buf.push_str(&format!(
            "    t{} = borrow_data_length {} +{}\n",
            out.into_raw(),
            value_name(*object),
            byte_offset
        )),
        Instruction::FloatUnary {
            out,
            kind,
            operation,
            operand,
        } => buf.push_str(&format!(
            "    t{} = float_{operation:?}<{kind:?}> {} : {}\n",
            out.into_raw(),
            value_name(*operand),
            function.temps[*out].ty.dump()
        )),
        Instruction::FloatBinary {
            out,
            kind,
            operation,
            lhs,
            rhs,
        } => buf.push_str(&format!(
            "    t{} = float_{operation:?}<{kind:?}> {}, {} : {}\n",
            out.into_raw(),
            value_name(*lhs),
            value_name(*rhs),
            function.temps[*out].ty.dump()
        )),
        Instruction::FloatConversion {
            out,
            conversion,
            operand,
        } => buf.push_str(&format!(
            "    t{} = float_convert<{conversion:?}> {} : {}\n",
            out.into_raw(),
            value_name(*operand),
            function.temps[*out].ty.dump()
        )),
        Instruction::BoxValue { .. } | Instruction::UnboxValue { .. } => {
            super::boxing::dump_boxing(function, instruction, buf)
        }
        Instruction::BinOp { out, op, lhs, rhs } => buf.push_str(&format!(
            "    t{} = {:?} {}, {} : {}\n",
            out.into_raw(),
            op,
            value_name(*lhs),
            value_name(*rhs),
            function.temps[*out].ty.dump()
        )),
        Instruction::UnaryOp { out, op, operand } => buf.push_str(&format!(
            "    t{} = {:?} {} : {}\n",
            out.into_raw(),
            op,
            value_name(*operand),
            function.temps[*out].ty.dump()
        )),
        Instruction::IntegerUnary {
            out,
            kind,
            operation,
            operand,
        } => buf.push_str(&format!(
            "    t{} = integer_{:?}<{}> {} : {}\n",
            out.into_raw(),
            operation,
            kind.canonical_name(),
            value_name(*operand),
            function.temps[*out].ty.dump()
        )),
        Instruction::IntegerBinary {
            out,
            kind,
            operation,
            lhs,
            rhs,
        } => buf.push_str(&format!(
            "    t{} = integer_{:?}<{}> {}, {} : {}\n",
            out.into_raw(),
            operation,
            kind.canonical_name(),
            value_name(*lhs),
            value_name(*rhs),
            function.temps[*out].ty.dump()
        )),
        Instruction::SafeIntegerDivRem {
            out,
            kind,
            operation,
            lhs,
            rhs,
        } => buf.push_str(&format!(
            "    t{} = safe_integer_{:?}<{}> {}, {} : {}\n",
            out.into_raw(),
            operation,
            kind.canonical_name(),
            value_name(*lhs),
            value_name(*rhs),
            function.temps[*out].ty.dump()
        )),
        Instruction::IntegerCompare {
            out,
            kind,
            comparison,
            lhs,
            rhs,
        } => buf.push_str(&format!(
            "    t{} = integer_compare_{:?}<{}> {}, {} : {}\n",
            out.into_raw(),
            comparison,
            kind.canonical_name(),
            value_name(*lhs),
            value_name(*rhs),
            function.temps[*out].ty.dump()
        )),
        Instruction::IntegerCompareTo {
            out,
            operand_kind,
            lhs,
            rhs,
        } => buf.push_str(&format!(
            "    t{} = integer_compare_to<{}> {}, {} : {}\n",
            out.into_raw(),
            operand_kind.canonical_name(),
            value_name(*lhs),
            value_name(*rhs),
            function.temps[*out].ty.dump()
        )),
        Instruction::IntegerShift {
            out,
            kind,
            operation,
            value,
            normalized_count,
        } => buf.push_str(&format!(
            "    t{} = integer_shift_{:?}<{}> {}, normalized={} : {}\n",
            out.into_raw(),
            operation,
            kind.canonical_name(),
            value_name(*value),
            value_name(*normalized_count),
            function.temps[*out].ty.dump()
        )),
        Instruction::IntegerConvert {
            out,
            source_kind,
            target_kind,
            operand,
        } => buf.push_str(&format!(
            "    t{} = integer_convert<{}->{}> {} : {}\n",
            out.into_raw(),
            source_kind.canonical_name(),
            target_kind.canonical_name(),
            value_name(*operand),
            function.temps[*out].ty.dump()
        )),
        Instruction::MakeZstValue { out, value } => buf.push_str(&format!(
            "    t{} = zst_value exact={} align {} : {}\n",
            out.into_raw(),
            value.exact(),
            value.representation().layout().alignment(),
            value.representation().storage_type().dump()
        )),
        Instruction::MakeAggregate { out, elements } => {
            let elements: Vec<String> = elements.iter().map(|e| value_name(*e)).collect();
            buf.push_str(&format!(
                "    t{} = aggregate ({}) : {}\n",
                out.into_raw(),
                elements.join(", "),
                function.temps[*out].ty.dump()
            ))
        }
        Instruction::ExtractValue {
            out,
            aggregate,
            index,
        } => buf.push_str(&format!(
            "    t{} = extract {}, {} : {}\n",
            out.into_raw(),
            value_name(*aggregate),
            index,
            function.temps[*out].ty.dump()
        )),
        Instruction::PublishReleaseReady { object } => buf.push_str(&format!(
            "    publish_release_ready {}\n",
            value_name(*object)
        )),
        Instruction::ReleaseFieldLoad { out, offset } => buf.push_str(&format!(
            "    t{} = release_field_load +{offset} : {}\n",
            out.into_raw(),
            function.temps[*out].ty.dump()
        )),
        Instruction::HeapLoad {
            out,
            object,
            offset,
        } => buf.push_str(&format!(
            "    t{} = heap_load {} +{} : {}\n",
            out.into_raw(),
            value_name(*object),
            offset,
            function.temps[*out].ty.dump()
        )),
        Instruction::MachineHeapLoad {
            out,
            kind,
            object,
            offset,
        } => buf.push_str(&format!(
            "    t{} = machine_heap_load {} {} +{} : {}\n",
            out.into_raw(),
            kind.name(),
            value_name(*object),
            offset,
            function.temps[*out].ty.dump()
        )),
        Instruction::MachineAtomicLoad {
            out,
            kind,
            object,
            offset,
        } => buf.push_str(&format!(
            "    t{} = atomic_load acquire {} {} +{} : {}\n",
            out.into_raw(),
            kind.name(),
            value_name(*object),
            offset,
            function.temps[*out].ty.dump()
        )),
        Instruction::GlobalLoad { out, global } => buf.push_str(&format!(
            "    t{} = global_load global{} : {}\n",
            out.into_raw(),
            global.into_raw(),
            function.temps[*out].ty.dump()
        )),
        Instruction::GlobalStore { global, value } => buf.push_str(&format!(
            "    global_store global{}, {}\n",
            global.into_raw(),
            value_name(*value)
        )),
        Instruction::GlobalAddress { out, global } => buf.push_str(&format!(
            "    t{} = global_address global{}\n",
            out.into_raw(),
            global.into_raw()
        )),
        Instruction::NativeGlobalLoad {
            out,
            global,
            protocol,
        } => buf.push_str(&format!(
            "    t{} = native_global_load ng{} {} : {}\n",
            out.into_raw(),
            global.into_raw(),
            native_storage_protocol_name(function, protocol),
            function.temps[*out].ty.dump()
        )),
        Instruction::NativeGlobalStore {
            global,
            value,
            protocol,
        } => buf.push_str(&format!(
            "    native_global_store ng{}, {} {}\n",
            global.into_raw(),
            value_name(*value),
            native_storage_protocol_name(function, protocol),
        )),
        Instruction::NativeGlobalAddress {
            out,
            global,
            protocol,
        } => buf.push_str(&format!(
            "    t{} = native_global_address ng{} {}\n",
            out.into_raw(),
            global.into_raw(),
            native_storage_protocol_name(function, protocol),
        )),
        Instruction::HeapStore {
            object,
            offset,
            value,
        } => buf.push_str(&format!(
            "    heap_store {} +{} {}\n",
            value_name(*object),
            offset,
            value_name(*value)
        )),
        Instruction::MachineHeapStore {
            kind,
            object,
            offset,
            value,
        } => buf.push_str(&format!(
            "    machine_heap_store {} {} +{} {}\n",
            kind.name(),
            value_name(*object),
            offset,
            value_name(*value)
        )),
        Instruction::MachineAtomicStore {
            kind,
            object,
            offset,
            value,
        } => buf.push_str(&format!(
            "    atomic_store release {} {} +{} {}\n",
            kind.name(),
            value_name(*object),
            offset,
            value_name(*value)
        )),
        Instruction::MachineAtomicCompareExchange {
            out,
            kind,
            object,
            offset,
            expected,
            replacement,
        } => buf.push_str(&format!(
            "    t{} = atomic_cmpxchg acq_rel/acquire {} {} +{} expected={} replacement={} : {}\n",
            out.into_raw(),
            kind.name(),
            value_name(*object),
            offset,
            value_name(*expected),
            value_name(*replacement),
            function.temps[*out].ty.dump()
        )),
        Instruction::FunctionAddress { out, target } => buf.push_str(&format!(
            "    t{} = function_address @{} : ptr\n",
            out.into_raw(),
            match target {
                FunctionAddressTarget::Local(reference) =>
                    module.functions[reference.declaration().into_u32() as usize].symbol(),
                FunctionAddressTarget::CallbackTrampoline(bridge) =>
                    module.callback_bridges[*bridge].trampoline.entry().symbol(),
            }
        )),
        Instruction::ForeignCallbackRegister {
            out,
            bridge,
            closure,
        } => buf.push_str(&format!(
            "    t{} = foreign_callback_register fcb{} {} : {}\n",
            out.into_raw(),
            bridge.into_raw(),
            value_name(*closure),
            function.temps[*out].ty.dump()
        )),
        Instruction::ForeignCallbackOperation(operation) => match *operation {
            ForeignCallbackOperation::Retain {
                family,
                out,
                callback,
            } => buf.push_str(&format!(
                "    t{} = foreign_callback_{} family{} {} : {}\n",
                out.into_raw(),
                "Retain",
                family.into_raw(),
                value_name(callback),
                function.temps[out].ty.dump()
            )),
            ForeignCallbackOperation::State {
                family,
                out,
                callback,
            } => buf.push_str(&format!(
                "    t{} = foreign_callback_{} family{} {} : {}\n",
                out.into_raw(),
                "State",
                family.into_raw(),
                value_name(callback),
                function.temps[out].ty.dump()
            )),
            ForeignCallbackOperation::Failure {
                family,
                out,
                callback,
            } => buf.push_str(&format!(
                "    t{} = foreign_callback_{} family{} {} : {}\n",
                out.into_raw(),
                "Failure",
                family.into_raw(),
                value_name(callback),
                function.temps[out].ty.dump()
            )),
            ForeignCallbackOperation::Release { family, callback } => buf.push_str(&format!(
                "    foreign_callback_Release family{} {}\n",
                family.into_raw(),
                value_name(callback)
            )),
        },
        Instruction::ULongToPtr { out, value } => buf.push_str(&format!(
            "    t{} = ulong_to_ptr {} : ptr\n",
            out.into_raw(),
            value_name(*value)
        )),
        Instruction::PtrToULong { out, value } => buf.push_str(&format!(
            "    t{} = ptr_to_ulong {} : i64\n",
            out.into_raw(),
            value_name(*value)
        )),
        Instruction::RawLoad {
            out,
            pointer,
            pointee,
        } => buf.push_str(&format!(
            "    t{} = raw_load {} align {} : {}\n",
            out.into_raw(),
            value_name(*pointer),
            pointee.layout().alignment(),
            function.temps[*out].ty.dump()
        )),
        Instruction::RawStore {
            pointer,
            value,
            pointee,
        } => buf.push_str(&format!(
            "    raw_store {} {} align {}\n",
            value_name(*pointer),
            value_name(*value),
            pointee.layout().alignment()
        )),
        Instruction::PtrOffset {
            out,
            pointer,
            element_offset,
            element_size,
            subtract,
        } => buf.push_str(&format!(
            "    t{} = ptr_offset {} element-offset={} element-size={} subtract={} : ptr\n",
            out.into_raw(),
            value_name(*pointer),
            value_name(*element_offset),
            element_size,
            subtract,
        )),
        Instruction::LocalAddress { out, local } => buf.push_str(&format!(
            "    t{} = local_address local{} : ptr\n",
            out.into_raw(),
            local.into_raw()
        )),
        Instruction::Store { local, value } => buf.push_str(&format!(
            "    store {} -> local{}\n",
            value_name(*value),
            local.into_raw()
        )),
        Instruction::Call { site } => {
            buf.push_str(&format!("    call {}\n", call_site_name(function, site)));
        }
        Instruction::ManagedPoll { site } => buf.push_str(&format!(
            "    poll managed-void-target{} sp{} live=[{}]\n",
            site.target.into_raw(),
            safepoint_name(function, site.safepoint),
            live_set_name(&site.live)
        )),
        Instruction::Invoke { site } => {
            buf.push_str(&format!(
                "    invoke {}\n",
                invoke_site_name(function, site)
            ));
        }
        Instruction::LandingPad { record, raw } => buf.push_str(&format!(
            "    (t{}, t{}) = landingpad : ({}, {})\n",
            record.into_raw(),
            raw.into_raw(),
            function.temps[*record].ty.dump(),
            function.temps[*raw].ty.dump()
        )),
        Instruction::CleanupPad { record, raw } => buf.push_str(&format!(
            "    (t{}, t{}) = cleanup_pad : ({}, {})\n",
            record.into_raw(),
            raw.into_raw(),
            function.temps[*record].ty.dump(),
            function.temps[*raw].ty.dump()
        )),
        Instruction::BeginCatch { out, raw } => buf.push_str(&format!(
            "    t{} = begin_catch {} : {}\n",
            out.into_raw(),
            value_name(*raw),
            function.temps[*out].ty.dump()
        )),
        Instruction::EndCatch => buf.push_str("    end_catch\n"),
        Instruction::Throw { exception } => {
            buf.push_str(&format!("    throw {}\n", value_name(*exception)))
        }
        Instruction::ArrayAllocDynamic {
            out,
            count,
            array_type,
            safepoint,
            live,
            overflow_message,
        } => buf.push_str(&format!(
            "    t{} = array_alloc_dynamic array{} count {} overflow {} sp{} live {} : {}\n",
            out.into_raw(),
            array_type.into_raw(),
            value_name(*count),
            value_name(Value::Global(*overflow_message)),
            safepoint_name(function, *safepoint),
            live_set_name(live),
            function.temps[*out].ty.dump()
        )),
        Instruction::ArrayAlloc {
            out,
            elements,
            array_type,
            safepoint,
            live,
        } => {
            let elements: Vec<String> = elements.iter().map(|e| value_name(*e)).collect();
            buf.push_str(&format!(
                "    t{} = array_alloc array{} ({}) sp{} live {} : {}\n",
                out.into_raw(),
                array_type.into_raw(),
                elements.join(", "),
                safepoint_name(function, *safepoint),
                live_set_name(live),
                function.temps[*out].ty.dump()
            ))
        }
        Instruction::ArrayAssembly {
            out,
            parts,
            overflow_message,
            array_type,
            safepoint,
            live,
        } => {
            let parts = parts
                .iter()
                .map(|part| match part {
                    ArrayAssemblyPart::Element(value) => {
                        format!("element {}", value_name(*value))
                    }
                    ArrayAssemblyPart::CopyArray(value) => {
                        format!("copy {}", value_name(*value))
                    }
                })
                .collect::<Vec<_>>();
            buf.push_str(&format!(
                "    t{} = array_assembly array{} ({}) overflow {} sp{} live {} : {}\n",
                out.into_raw(),
                array_type.into_raw(),
                parts.join(", "),
                value_name(Value::Global(*overflow_message)),
                safepoint_name(function, *safepoint),
                live_set_name(live),
                function.temps[*out].ty.dump()
            ))
        }
        Instruction::ArrayLen {
            out,
            operand,
            array_type,
        } => buf.push_str(&format!(
            "    t{} = array_len array{} {} : {}\n",
            out.into_raw(),
            array_type.into_raw(),
            value_name(*operand),
            function.temps[*out].ty.dump()
        )),
        Instruction::ArrayGet {
            out,
            array,
            index,
            array_type,
        } => buf.push_str(&format!(
            "    t{} = array_get array{} {} {} : {}\n",
            out.into_raw(),
            array_type.into_raw(),
            value_name(*array),
            value_name(*index),
            function.temps[*out].ty.dump()
        )),
        Instruction::ArraySet {
            array,
            index,
            value,
            array_type,
        } => buf.push_str(&format!(
            "    array_set array{} {} {} {}\n",
            array_type.into_raw(),
            value_name(*array),
            value_name(*index),
            value_name(*value)
        )),
        Instruction::ArrayClone {
            source_type,
            out,
            operand,
            array_type,
            safepoint,
            live,
        } => buf.push_str(&format!(
            "    t{} = array_clone array{} -> array{} {} sp{} live {} : {}\n",
            out.into_raw(),
            source_type.into_raw(),
            array_type.into_raw(),
            value_name(*operand),
            safepoint_name(function, *safepoint),
            live_set_name(live),
            function.temps[*out].ty.dump()
        )),
        Instruction::EnumWrap {
            out,
            variant,
            fields,
        } => {
            let fields: Vec<String> = fields.iter().map(|f| value_name(*f)).collect();
            buf.push_str(&format!(
                "    t{} = enum_wrap e{} v{} ({}) : {}\n",
                out.into_raw(),
                variant.definition().into_raw(),
                variant.index(),
                fields.join(", "),
                function.temps[*out].ty.dump()
            ))
        }
        Instruction::EnumTag {
            out,
            enum_id,
            operand,
        } => buf.push_str(&format!(
            "    t{} = enum_tag e{} {} : {}\n",
            out.into_raw(),
            enum_id.into_raw(),
            value_name(*operand),
            function.temps[*out].ty.dump()
        )),
        Instruction::EnumField {
            out,
            enum_id,
            variant,
            index,
            operand,
        } => buf.push_str(&format!(
            "    t{} = enum_field e{} v{} f{} {} : {}\n",
            out.into_raw(),
            enum_id.into_raw(),
            variant,
            index,
            value_name(*operand),
            function.temps[*out].ty.dump()
        )),
        Instruction::VariantTest {
            out,
            operand,
            variant,
        } => buf.push_str(&format!(
            "    t{} = variant_test e{} v{} {} : {}\n",
            out.into_raw(),
            variant.definition().into_raw(),
            variant.index(),
            value_name(*operand),
            function.temps[*out].ty.dump()
        )),
        Instruction::VariantPayloadProject {
            out,
            operand,
            field,
        } => buf.push_str(&format!(
            "    t{} = variant_payload_project e{} v{} f{} {} : {}\n",
            out.into_raw(),
            field.definition().into_raw(),
            field.variant().index(),
            field.index(),
            value_name(*operand),
            function.temps[*out].ty.dump()
        )),
    }
}

fn native_storage_protocol_name(function: &Function, protocol: &NativeStorageProtocol) -> String {
    match protocol {
        NativeStorageProtocol::NoTransition => "no_transition".into(),
        NativeStorageProtocol::NativeSafe { safepoint, roots } => format!(
            "sp{} roots=[{}]",
            safepoint_name(function, *safepoint),
            caller_roots_name(roots.as_slice()),
        ),
    }
}
