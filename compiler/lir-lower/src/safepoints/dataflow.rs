use super::*;

pub(super) fn instruction_uses(
    instruction: &lir::Instruction,
    function: &lir::Function,
) -> Vec<lir::Value> {
    match instruction {
        lir::Instruction::BoxValue { payload, .. } => payload
            .source()
            .map(lir::Value::Local)
            .into_iter()
            .collect(),
        lir::Instruction::UnboxValue { object, .. } => vec![*object],
        lir::Instruction::PublishReleaseReady { object } => vec![*object],
        lir::Instruction::BinOp { lhs, rhs, .. }
        | lir::Instruction::FloatBinary { lhs, rhs, .. }
        | lir::Instruction::IntegerBinary { lhs, rhs, .. }
        | lir::Instruction::SafeIntegerDivRem { lhs, rhs, .. }
        | lir::Instruction::IntegerCompare { lhs, rhs, .. }
        | lir::Instruction::IntegerCompareTo { lhs, rhs, .. } => vec![*lhs, *rhs],
        lir::Instruction::IntegerShift {
            value,
            normalized_count,
            ..
        } => vec![*value, *normalized_count],
        lir::Instruction::UnaryOp { operand, .. }
        | lir::Instruction::FloatUnary { operand, .. }
        | lir::Instruction::FloatConversion { operand, .. }
        | lir::Instruction::IntegerUnary { operand, .. }
        | lir::Instruction::IntegerConvert { operand, .. }
        | lir::Instruction::ExtractValue {
            aggregate: operand, ..
        }
        | lir::Instruction::HeapLoad {
            object: operand, ..
        }
        | lir::Instruction::MachineHeapLoad {
            object: operand, ..
        }
        | lir::Instruction::AtomicLoad {
            object: operand, ..
        }
        | lir::Instruction::ULongToPtr { value: operand, .. }
        | lir::Instruction::PtrToULong { value: operand, .. }
        | lir::Instruction::RawLoad {
            pointer: operand, ..
        }
        | lir::Instruction::BeginCatch { raw: operand, .. }
        | lir::Instruction::Throw { exception: operand }
        | lir::Instruction::ArrayAllocDynamic { count: operand, .. }
        | lir::Instruction::ArrayLen { operand, .. }
        | lir::Instruction::ArrayClone { operand, .. }
        | lir::Instruction::EnumTag { operand, .. }
        | lir::Instruction::EnumField { operand, .. }
        | lir::Instruction::VariantTest { operand, .. }
        | lir::Instruction::VariantPayloadProject { operand, .. }
        | lir::Instruction::ForeignCallbackRegister {
            closure: operand, ..
        } => vec![*operand],
        lir::Instruction::ForeignCallbackOperation(operation) => vec![operation.callback()],
        lir::Instruction::MakeAggregate { elements, .. }
        | lir::Instruction::ArrayAlloc { elements, .. } => elements.clone(),
        lir::Instruction::ArrayAssembly { parts, .. } => parts
            .iter()
            .map(|part| match part {
                lir::ArrayAssemblyPart::Element(value)
                | lir::ArrayAssemblyPart::CopyArray(value) => *value,
            })
            .collect(),
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
        | lir::Instruction::MachineHeapStore { object, value, .. }
        | lir::Instruction::AtomicStore { object, value, .. } => vec![*object, *value],
        lir::Instruction::AtomicCompareExchange {
            object,
            expected,
            replacement,
            ..
        } => vec![*object, *expected, *replacement],
        lir::Instruction::RawStore { pointer, value, .. } => vec![*pointer, *value],
        lir::Instruction::PtrOffset {
            pointer,
            element_offset,
            ..
        } => vec![*pointer, *element_offset],
        lir::Instruction::LocalAddress { local, .. } => vec![lir::Value::Local(*local)],
        lir::Instruction::ArrayGet { array, index, .. } => vec![*array, *index],
        lir::Instruction::ArraySet {
            array,
            index,
            value,
            ..
        } => vec![*array, *index, *value],
        lir::Instruction::EnumWrap { fields, .. } => fields.clone(),
        lir::Instruction::ReleaseFieldLoad { .. }
        | lir::Instruction::MakeZstValue { .. }
        | lir::Instruction::GlobalLoad { .. }
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

pub(super) fn call_uses(
    args: &[lir::AbiCallArgument],
    destination: lir::CallDestination,
) -> Vec<lir::Value> {
    let mut values = args
        .iter()
        .map(|argument| argument.logical_value())
        .collect::<Vec<_>>();
    if let lir::CallDestination::Dispatch { table, .. } = destination {
        values.push(table);
    }
    values
}

pub(super) fn instruction_defs(instruction: &lir::Instruction) -> Vec<LiveValue> {
    let out = match instruction {
        lir::Instruction::UnboxValue { result, .. } => {
            return match result {
                lir::UnboxResult::ZeroSized { out, .. } => vec![LiveValue::Temp(*out)],
                lir::UnboxResult::NonZero(place) => vec![LiveValue::Local(place.local())],
            };
        }
        lir::Instruction::BoxValue { out, .. }
        | lir::Instruction::BinOp { out, .. }
        | lir::Instruction::UnaryOp { out, .. }
        | lir::Instruction::FloatUnary { out, .. }
        | lir::Instruction::FloatBinary { out, .. }
        | lir::Instruction::FloatConversion { out, .. }
        | lir::Instruction::IntegerUnary { out, .. }
        | lir::Instruction::IntegerBinary { out, .. }
        | lir::Instruction::SafeIntegerDivRem { out, .. }
        | lir::Instruction::IntegerCompare { out, .. }
        | lir::Instruction::IntegerCompareTo { out, .. }
        | lir::Instruction::IntegerShift { out, .. }
        | lir::Instruction::IntegerConvert { out, .. }
        | lir::Instruction::MakeAggregate { out, .. }
        | lir::Instruction::MakeZstValue { out, .. }
        | lir::Instruction::ExtractValue { out, .. }
        | lir::Instruction::ReleaseFieldLoad { out, .. }
        | lir::Instruction::HeapLoad { out, .. }
        | lir::Instruction::MachineHeapLoad { out, .. }
        | lir::Instruction::AtomicLoad { out, .. }
        | lir::Instruction::AtomicCompareExchange { out, .. }
        | lir::Instruction::GlobalLoad { out, .. }
        | lir::Instruction::GlobalAddress { out, .. }
        | lir::Instruction::NativeGlobalLoad { out, .. }
        | lir::Instruction::NativeGlobalAddress { out, .. }
        | lir::Instruction::FunctionAddress { out, .. }
        | lir::Instruction::ULongToPtr { out, .. }
        | lir::Instruction::PtrToULong { out, .. }
        | lir::Instruction::RawLoad { out, .. }
        | lir::Instruction::PtrOffset { out, .. }
        | lir::Instruction::LocalAddress { out, .. }
        | lir::Instruction::BeginCatch { out, .. }
        | lir::Instruction::ArrayAllocDynamic { out, .. }
        | lir::Instruction::ArrayAlloc { out, .. }
        | lir::Instruction::ArrayAssembly { out, .. }
        | lir::Instruction::ArrayLen { out, .. }
        | lir::Instruction::ArrayGet { out, .. }
        | lir::Instruction::ArrayClone { out, .. }
        | lir::Instruction::EnumWrap { out, .. }
        | lir::Instruction::EnumTag { out, .. }
        | lir::Instruction::EnumField { out, .. }
        | lir::Instruction::VariantTest { out, .. }
        | lir::Instruction::VariantPayloadProject { out, .. }
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
        | lir::Instruction::PublishReleaseReady { .. }
        | lir::Instruction::NativeGlobalStore { .. }
        | lir::Instruction::HeapStore { .. }
        | lir::Instruction::MachineHeapStore { .. }
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
        lir::TypedCallResult::ElidedZst(out) | lir::TypedCallResult::Direct(out) => {
            vec![LiveValue::Temp(out)]
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    fn callable_body() -> lir::CallableBodyIdentity {
        let site = scoop_identity::SourceDeclarationSite::new(
            scoop_identity::ConeIdentity::SINGLE_FILE,
            scoop_identity::PackagePath::root(),
            scoop_identity::DefinitionOwnerChain::top_level(),
            scoop_identity::DeclarationScope::ConeWide,
        )
        .unwrap();
        let declaration = scoop_identity::SourceDeclarationKey::function(
            site,
            scoop_identity::CanonicalIdentifier::new("dataflowTest").unwrap(),
            0,
            None,
            Vec::new(),
        );
        let function =
            scoop_identity::PersistentFunctionId::from_source_declaration(&declaration).unwrap();
        lir::CallableBodyIdentity::for_function(function).unwrap()
    }

    #[test]
    fn variant_primitives_track_their_operand_use_and_temp_definition() {
        let mut enums = lir::EnumDefs::default();
        let enum_id = enums.alloc(lir::EnumDef {
            exact_type: crate::tests::test_physical_exact(
                "Tracked",
                scoop_identity::SourceNominalKind::Enum,
            ),
            name: "Tracked".to_string(),
            repr: lir::EnumRepr::Tagged {
                variants: vec![lir::EnumVariantRepr {
                    fields: vec![lir::EnumFieldRepr {
                        ty: lir::MANAGED_PTR,
                        offset: 8,
                    }],
                    slot_offset: 8,
                    slot_size: 8,
                    slot_align: 8,
                    gc_free: false,
                }],
                size: 16,
                align: 8,
            },
            scan: lir::RefScan::References(vec![8]),
        });
        let variant = enums.variant_ref(enum_id, 0).expect("variant exists");
        let field = enums
            .variant_field_ref(variant, 0)
            .expect("payload field exists");

        let mut temps = Arena::default();
        let tested = temps.alloc(lir::Temp {
            ty: lir::LirType::I1,
        });
        let projected = temps.alloc(lir::Temp {
            ty: lir::MANAGED_PTR,
        });
        let mut blocks = Arena::default();
        let entry = blocks.alloc(lir::BasicBlock {
            name: "entry".to_string(),
            instructions: Vec::new(),
            terminator: lir::Terminator::Unreachable,
        });
        let function = lir::Function {
            callable_body: callable_body(),
            gc_effect: lir::GcEffect::Managed,
            signature: lir::ScoopAbiSignature::new(
                vec![lir::AbiArgument::Indirect(
                    lir::AbiValue::new(
                        lir::LirType::Enum(enum_id),
                        lir::AbiNonZeroLayout::new(16, 8).expect("test enum has a valid layout"),
                        lir::RefScan::References(vec![8]),
                    )
                    .expect("test enum is a non-void ABI value"),
                )],
                lir::AbiReturn::UnitVoid,
                lir::CallingConvention::Cdecl,
            ),
            call_targets: lir::CallTargets::default(),
            safepoints: lir::SafepointIdentities::default(),
            locals: Arena::default(),
            temps,
            blocks,
            entry,
        };
        let operand = lir::Value::Param(0);
        let test = lir::Instruction::VariantTest {
            out: tested,
            operand,
            variant,
        };
        let project = lir::Instruction::VariantPayloadProject {
            out: projected,
            operand,
            field,
        };

        assert_eq!(instruction_uses(&test, &function), vec![operand]);
        assert_eq!(instruction_defs(&test), vec![LiveValue::Temp(tested)]);
        assert_eq!(instruction_uses(&project, &function), vec![operand]);
        assert_eq!(instruction_defs(&project), vec![LiveValue::Temp(projected)]);
    }
}
