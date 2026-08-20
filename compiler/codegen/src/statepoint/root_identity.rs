//! Zero-code LLVM uses retain logical leaf identities across SROA and RS4GC.

use super::*;

pub(crate) fn mark_root_identity<'ctx>(
    context: &'ctx Context,
    module: &LlvmModule<'ctx>,
    builder: &Builder<'ctx>,
    value: PointerValue<'ctx>,
    safepoint: scoop_lir::SafepointId,
    source: scoop_lir::CallerRootSource,
    byte_offset: u64,
) -> Result<(), CodegenError> {
    let intrinsic = inkwell::intrinsics::Intrinsic::find("llvm.fake.use")
        .and_then(|intrinsic| intrinsic.get_declaration(module, &[]))
        .ok_or_else(|| CodegenError("LLVM lacks required llvm.fake.use intrinsic".into()))?;
    let instruction = builder
        .build_call(intrinsic, &[value.into()], "")
        .map_err(|error| CodegenError(format!("retain GC leaf use: {error}")))?
        .try_as_basic_value()
        .expect_instruction("llvm.fake.use returns void");
    let (source_kind, source_index) = match source {
        scoop_lir::CallerRootSource::Param(index) => (0, index),
        scoop_lir::CallerRootSource::Local(id) => (1, id.into_raw().into_u32()),
        scoop_lir::CallerRootSource::Temp(id) => (2, id.into_raw().into_u32()),
    };
    let i64_type = context.i64_type();
    instruction
        .set_metadata(
            context.metadata_node(&[
                i64_type.const_int(safepoint.get(), false).into(),
                i64_type.const_int(source_kind, false).into(),
                i64_type.const_int(source_index.into(), false).into(),
                i64_type.const_int(byte_offset, false).into(),
            ]),
            context.get_kind_id(STATEPOINT_ROOT_IDENTITY_METADATA),
        )
        .map_err(|error| {
            CodegenError(format!(
                "mark statepoint {} root identity: {error}",
                safepoint.get()
            ))
        })
}

pub(super) fn read(
    instruction: InstructionValue<'_>,
    metadata_kind: u32,
) -> Result<Option<(u64, ExpectedRoot)>, CodegenError> {
    let Some(metadata) = instruction.get_metadata(metadata_kind) else {
        return Ok(None);
    };
    if instruction.get_opcode() != InstructionOpcode::Call {
        return Err(CodegenError(
            "typed root identity is not attached to llvm.fake.use".into(),
        ));
    }
    // SAFETY: the opcode check establishes a CallBase.
    let call = unsafe { CallSiteValue::new(instruction.as_value_ref()) };
    if call
        .get_called_fn_value()
        .is_none_or(|function| function.get_name().to_bytes() != b"llvm.fake.use")
        || call.count_arguments() != 1
    {
        return Err(CodegenError(
            "typed root use has the wrong intrinsic or arity".into(),
        ));
    }
    let values = metadata
        .get_node_values()
        .ok_or_else(|| CodegenError("statepoint root identity metadata is not a node".into()))?;
    let [
        BasicMetadataValueEnum::IntValue(site),
        BasicMetadataValueEnum::IntValue(kind),
        BasicMetadataValueEnum::IntValue(index),
        BasicMetadataValueEnum::IntValue(offset),
    ] = values.as_slice()
    else {
        return Err(CodegenError(
            "statepoint root identity metadata has an invalid shape".into(),
        ));
    };
    let site = constant(*site)?;
    let index = u32::try_from(constant(*index)?)
        .map_err(|_| CodegenError("root source index exceeds u32::MAX".into()))?;
    let source = match constant(*kind)? {
        0 => scoop_lir::CallerRootSource::Param(index),
        1 => scoop_lir::CallerRootSource::Local(scoop_lir::LocalId::from_raw(RawIdx::from_u32(
            index,
        ))),
        2 => {
            scoop_lir::CallerRootSource::Temp(scoop_lir::TempId::from_raw(RawIdx::from_u32(index)))
        }
        kind => {
            return Err(CodegenError(format!(
                "statepoint {site} has unknown root source kind {kind}"
            )));
        }
    };
    Ok(Some((
        site,
        ExpectedRoot {
            source,
            byte_offset: constant(*offset)?,
        },
    )))
}

pub(super) fn restored_value<'ctx>(
    instruction: InstructionValue<'ctx>,
    space: ManagedAddressSpace,
) -> Result<PointerValue<'ctx>, CodegenError> {
    // SAFETY: read() only accepts a verified llvm.fake.use with one argument.
    let value = unsafe { LLVMGetOperand(instruction.as_value_ref(), 0) };
    if !provenance::is_managed_pointer(value, space) {
        return Err(CodegenError(format!(
            "root restoration is not an AS{} managed pointer",
            space.llvm()
        )));
    }
    Ok(unsafe { PointerValue::new(value) })
}

fn constant(value: inkwell::values::IntValue<'_>) -> Result<u64, CodegenError> {
    if unsafe { LLVMIsAConstantInt(value.as_value_ref()) }.is_null() {
        return Err(CodegenError(
            "root identity metadata is not a constant integer".into(),
        ));
    }
    Ok(unsafe { LLVMConstIntGetZExtValue(value.as_value_ref()) })
}
