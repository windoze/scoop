//! Statepoint gc-live roots, typed identities, and exact LIR shape checks.

use super::llvm::llvm_value_name;
use super::*;

pub(super) fn gc_live_roots<'ctx>(
    raw: inkwell::llvm_sys::prelude::LLVMValueRef,
    id: u64,
    managed_address_space: ManagedAddressSpace,
) -> Result<Vec<BasicValueEnum<'ctx>>, CodegenError> {
    // SAFETY: the caller only passes a statepoint CallBase instruction.
    let call = unsafe { CallSiteValue::new(raw) };
    let mut roots = None;
    for bundle in call.get_operand_bundles() {
        let tag = bundle.get_tag().map_err(|error| {
            CodegenError(format!(
                "statepoint {id} operand-bundle tag is not UTF-8: {error}"
            ))
        })?;
        if tag != "gc-live" {
            return Err(CodegenError(format!(
                "statepoint {id} carries unsupported `{tag}` operand bundle"
            )));
        }
        if roots.is_some() {
            return Err(CodegenError(format!(
                "statepoint {id} carries more than one gc-live bundle"
            )));
        }
        roots = Some(bundle.get_args().collect::<Vec<_>>());
    }
    let roots = roots.unwrap_or_default();
    let mut identities = BTreeSet::new();
    for (index, root) in roots.iter().enumerate() {
        if !provenance::is_managed_pointer(root.as_value_ref(), managed_address_space) {
            return Err(CodegenError(format!(
                "statepoint {id} gc-live root {index} is not an AS{} managed pointer",
                managed_address_space.llvm()
            )));
        }
        if root
            .as_instruction_value()
            .is_some_and(|instruction| instruction.get_opcode() == InstructionOpcode::GetElementPtr)
        {
            return Err(CodegenError(format!(
                "statepoint {id} gc-live root {index} is a derived AS{} address",
                managed_address_space.llvm()
            )));
        }
        if !identities.insert(root.as_value_ref() as usize) {
            return Err(CodegenError(format!(
                "statepoint {id} repeats one LLVM root value in gc-live"
            )));
        }
    }
    Ok(roots)
}

pub(super) fn gc_live_root_identities(
    roots: &[BasicValueEnum<'_>],
    metadata_kind: u32,
    statepoint: u64,
) -> Result<Vec<ExpectedRoot>, CodegenError> {
    let mut identities = Vec::with_capacity(roots.len());
    let mut unique = BTreeSet::new();
    for (index, root) in roots.iter().enumerate() {
        let instruction = root.as_instruction_value().ok_or_else(|| {
            CodegenError(format!(
                "statepoint {statepoint} gc-live root {index} has no typed LIR identity: {root:?}"
            ))
        })?;
        if instruction.get_opcode() != InstructionOpcode::Load {
            return Err(CodegenError(format!(
                "statepoint {statepoint} gc-live root {index} is not loaded from typed root storage: {root:?}"
            )));
        }
        let storage = unsafe { LLVMGetOperand(instruction.as_value_ref(), 0) };
        if unsafe { LLVMIsAAllocaInst(storage) }.is_null() {
            return Err(CodegenError(format!(
                "statepoint {statepoint} gc-live root {index} does not use dedicated typed root storage: {root:?}"
            )));
        }
        // SAFETY: LLVMIsAAllocaInst above proves the operand is an instruction.
        let storage = unsafe { InstructionValue::new(storage) };
        let metadata = storage.get_metadata(metadata_kind).ok_or_else(|| {
            CodegenError(format!(
                "statepoint {statepoint} gc-live root {index} lacks typed LIR identity metadata: {root:?}"
            ))
        })?;
        let values = metadata.get_node_values().ok_or_else(|| {
            CodegenError(format!(
                "statepoint {statepoint} root identity metadata is not a node"
            ))
        })?;
        let [
            BasicMetadataValueEnum::IntValue(metadata_statepoint),
            BasicMetadataValueEnum::IntValue(source_kind),
            BasicMetadataValueEnum::IntValue(source_index),
            BasicMetadataValueEnum::IntValue(byte_offset),
        ] = values.as_slice()
        else {
            return Err(CodegenError(format!(
                "statepoint {statepoint} root identity metadata has an invalid shape"
            )));
        };
        let metadata_statepoint = constant_metadata_int(*metadata_statepoint, "safepoint id")?;
        if metadata_statepoint != statepoint {
            return Err(CodegenError(format!(
                "statepoint {statepoint} root {index} carries identity for statepoint {metadata_statepoint}"
            )));
        }
        let source_index =
            u32::try_from(constant_metadata_int(*source_index, "root source index")?).map_err(
                |_| {
                    CodegenError(format!(
                        "statepoint {statepoint} root source index exceeds u32::MAX"
                    ))
                },
            )?;
        let source = match constant_metadata_int(*source_kind, "root source kind")? {
            0 => scoop_lir::CallerRootSource::Param(source_index),
            1 => scoop_lir::CallerRootSource::Local(scoop_lir::LocalId::from_raw(
                RawIdx::from_u32(source_index),
            )),
            2 => scoop_lir::CallerRootSource::Temp(scoop_lir::TempId::from_raw(RawIdx::from_u32(
                source_index,
            ))),
            kind => {
                return Err(CodegenError(format!(
                    "statepoint {statepoint} root {index} carries unknown source kind {kind}"
                )));
            }
        };
        let identity = ExpectedRoot {
            source,
            byte_offset: constant_metadata_int(*byte_offset, "root byte offset")?,
        };
        if !unique.insert(identity.key()) {
            return Err(CodegenError(format!(
                "statepoint {statepoint} repeats typed root identity {identity:?}"
            )));
        }
        identities.push(identity);
    }
    Ok(identities)
}

fn constant_metadata_int(
    value: inkwell::values::IntValue<'_>,
    what: &str,
) -> Result<u64, CodegenError> {
    if unsafe { LLVMIsAConstantInt(value.as_value_ref()) }.is_null() {
        return Err(CodegenError(format!(
            "statepoint {what} metadata is not a constant integer"
        )));
    }
    Ok(unsafe { LLVMConstIntGetZExtValue(value.as_value_ref()) })
}

pub(super) fn verify_statepoint_shape(
    id: u64,
    expected: &ExpectedStatepoint,
    instruction: InstructionValue<'_>,
    roots: &[BasicValueEnum<'_>],
    identities: &[ExpectedRoot],
    function: &str,
) -> Result<(), CodegenError> {
    let expected_count = match expected {
        ExpectedStatepoint::Relocating(expected) => expected.len(),
        ExpectedStatepoint::NativeTransition(_) | ExpectedStatepoint::ZeroLiveInvoke => 0,
    };
    if roots.len() != expected_count {
        // SAFETY: the statepoint shape check above has already validated the
        // fixed header, whose operand 2 is the actual callee pointer.
        let actual_callee =
            llvm_value_name(unsafe { LLVMGetOperand(instruction.as_value_ref(), 2) })?;
        return Err(CodegenError(format!(
            "statepoint {id} for `{actual_callee}` in `{function}` has a gc-live count that disagrees with complete LIR: expected {expected_count} from {expected:?}, observed {} ({roots:?})",
            roots.len(),
        )));
    }
    if let ExpectedStatepoint::NativeTransition(expected_callee) = expected {
        // SAFETY: the statepoint shape check has validated operand 2 as the
        // actual callee pointer.
        let actual_callee =
            llvm_value_name(unsafe { LLVMGetOperand(instruction.as_value_ref(), 2) })?;
        if actual_callee != expected_callee.logical_symbol() {
            let expected_callee = expected_callee.logical_symbol();
            return Err(CodegenError(format!(
                "native transition statepoint {id} in `{function}` targets `{actual_callee}`, expected `{expected_callee}`"
            )));
        }
    }
    if let ExpectedStatepoint::Relocating(expected_roots) = expected {
        let expected_identities = expected_roots
            .iter()
            .copied()
            .map(ExpectedRoot::key)
            .collect::<BTreeSet<_>>();
        let observed_identities = identities
            .iter()
            .copied()
            .map(ExpectedRoot::key)
            .collect::<BTreeSet<_>>();
        if observed_identities != expected_identities {
            return Err(CodegenError(format!(
                "statepoint {id} in `{function}` has gc-live identities that disagree with complete LIR: expected {expected_roots:?}, observed {identities:?}"
            )));
        }
    } else if !identities.is_empty() {
        return Err(CodegenError(format!(
            "zero-live statepoint {id} unexpectedly carries typed root identities"
        )));
    }
    let expected_opcode = match expected {
        ExpectedStatepoint::Relocating(_) | ExpectedStatepoint::NativeTransition(_) => {
            InstructionOpcode::Call
        }
        ExpectedStatepoint::ZeroLiveInvoke => InstructionOpcode::Invoke,
    };
    if instruction.get_opcode() != expected_opcode {
        return Err(CodegenError(format!(
            "statepoint {id} has {:?} control shape, expected {expected_opcode:?}",
            instruction.get_opcode()
        )));
    }
    Ok(())
}
