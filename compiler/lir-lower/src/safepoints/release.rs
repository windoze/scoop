use super::*;

mod values;

pub(crate) fn validate(module: &lir::Module) -> StorageResult<()> {
    for (_, hook) in module.release_hooks.iter() {
        validate_body(module, &hook.code)?;
    }
    Ok(())
}

fn invalid(reason: &'static str) -> StorageLoweringError {
    StorageLoweringError::InvalidRepresentation(reason)
}

fn validate_body(module: &lir::Module, function: &lir::Function) -> StorageResult<()> {
    if function.callable_body.release_owner().is_none()
        || function.gc_effect != lir::GcEffect::NoGc
        || function.signature.arguments().len() != 1
        || !matches!(&function.signature.arguments()[0],
            lir::AbiArgument::Direct(argument) if argument.storage_type() == &lir::RAW_PTR)
        || !matches!(function.signature.result(), lir::AbiReturn::UnitVoid)
        || !function.safepoints.is_empty()
    {
        return Err(invalid(
            "release machine body requires its private NoGc void(AS0) ABI",
        ));
    }
    let mut values = values::MachineValues::new(module);
    for (_, local) in function.locals.iter() {
        if matches!(local.storage(), lir::LocalStorage::NonZero(value) if value.scan() != &lir::RefScan::None)
        {
            return Err(invalid(
                "release machine locals cannot contain managed values",
            ));
        }
    }
    for (_, temporary) in function.temps.iter() {
        if !values.gc_free(&temporary.ty) {
            return Err(invalid(
                "release machine temporaries cannot contain managed values",
            ));
        }
    }
    for (_, block) in function.blocks.iter() {
        if !matches!(
            block.terminator,
            lir::Terminator::Br(_)
                | lir::Terminator::CondBr { .. }
                | lir::Terminator::Return { value: None }
                | lir::Terminator::Unreachable
        ) {
            return Err(invalid(
                "release machine control flow cannot unwind or return a value",
            ));
        }
        let mut valid_operands = true;
        terminator_uses(&block.terminator, |value| {
            valid_operands &= operand(module, value)
        });
        for instruction in &block.instructions {
            if !allowed_instruction(module, function, instruction) {
                return Err(invalid(
                    "operation is outside the release machine body's GC-free subset",
                ));
            }
            valid_operands &= instruction_uses(instruction, function)
                .into_iter()
                .all(|value| operand(module, value));
        }
        if !valid_operands {
            return Err(invalid(
                "release raw receiver and runtime metadata cannot be used as ordinary values",
            ));
        }
    }
    Ok(())
}

fn operand(module: &lir::Module, value: lir::Value) -> bool {
    match value {
        lir::Value::Param(_)
        | lir::Value::TypeDescriptor(_)
        | lir::Value::RootScan(_)
        | lir::Value::InitializationUnit(_)
        | lir::Value::NullPointer(lir::PointerKind::Managed) => false,
        lir::Value::Global(global) => global_storage(module, global),
        _ => true,
    }
}

fn global_storage(module: &lir::Module, global: lir::GlobalId) -> bool {
    let global = &module.globals[global];
    global.address_kind == lir::PointerKind::Raw
        && global.scan == lir::RefScan::None
        && !matches!(
            global.init,
            lir::GlobalInit::RawStorage {
                thread_local: true,
                ..
            } | lir::GlobalInit::StringConst { .. }
                | lir::GlobalInit::CString { .. }
        )
}

fn allowed_instruction(
    module: &lir::Module,
    function: &lir::Function,
    instruction: &lir::Instruction,
) -> bool {
    use lir::Instruction as I;
    match instruction {
        I::Call {
            site: site @ lir::CallSite::ReleaseScoop(_),
        } => matches!(
            site.destination(&function.call_targets),
            lir::CallDestination::Local(_) | lir::CallDestination::External(_)
        ),
        I::Call {
            site: lir::CallSite::ReleaseNativeLeaf(_),
        } => true,
        I::GlobalLoad { global, .. }
        | I::GlobalStore { global, .. }
        | I::GlobalAddress { global, .. } => global_storage(module, *global),
        I::NativeGlobalLoad {
            global, protocol, ..
        }
        | I::NativeGlobalStore {
            global, protocol, ..
        }
        | I::NativeGlobalAddress {
            global, protocol, ..
        } => {
            !module.native_globals[*global].thread_local
                && matches!(protocol, lir::NativeStorageProtocol::NoTransition)
        }
        I::ReleaseFieldLoad { offset, .. } => *offset >= 16,
        I::BinOp { .. }
        | I::UnaryOp { .. }
        | I::IntegerUnary { .. }
        | I::IntegerBinary { .. }
        | I::IntegerCompare { .. }
        | I::IntegerCompareTo { .. }
        | I::IntegerShift { .. }
        | I::IntegerConvert { .. }
        | I::MakeZstValue { .. }
        | I::MakeAggregate { .. }
        | I::ExtractValue { .. }
        | I::Store { .. }
        | I::ULongToPtr { .. }
        | I::PtrToULong { .. }
        | I::RawLoad { .. }
        | I::RawStore { .. }
        | I::PtrOffset { .. }
        | I::LocalAddress { .. }
        | I::EnumWrap { .. }
        | I::EnumTag { .. }
        | I::EnumField { .. }
        | I::VariantTest { .. }
        | I::VariantPayloadProject { .. } => true,
        _ => false,
    }
}
