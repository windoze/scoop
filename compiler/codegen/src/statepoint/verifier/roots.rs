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

pub(super) fn verify_statepoint_shape(
    id: u64,
    expected: &ExpectedStatepoint,
    instruction: InstructionValue<'_>,
    roots: &[BasicValueEnum<'_>],
    function: &str,
) -> Result<(), CodegenError> {
    let expected_count = match expected {
        ExpectedStatepoint::Relocating(expected) => expected.groups.len(),
        ExpectedStatepoint::NativeTransition(_) | ExpectedStatepoint::ZeroLiveInvoke => 0,
    };
    if roots.len() != expected_count {
        // SAFETY: the statepoint shape check above has already validated the
        // fixed header, whose operand 2 is the actual callee pointer.
        let actual_callee =
            llvm_value_name(unsafe { LLVMGetOperand(instruction.as_value_ref(), 2) })?;
        return Err(CodegenError(format!(
            "statepoint {id} for `{actual_callee}` in `{function}` has a gc-live count that disagrees with final GC plan: expected {expected_count} from {expected:?}, observed {} ({roots:?})",
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
