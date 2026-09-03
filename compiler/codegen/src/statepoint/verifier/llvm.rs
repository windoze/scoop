//! Small checked wrappers around LLVM's CallBase and value-name C APIs.

use super::*;

pub(super) fn call_constant(
    raw: inkwell::llvm_sys::prelude::LLVMValueRef,
    index: u32,
    what: &str,
) -> Result<u64, CodegenError> {
    // SAFETY: CallBase argument counts and operands come from verified IR.
    let argument_count = unsafe { LLVMGetNumArgOperands(raw) };
    if index >= argument_count {
        return Err(CodegenError(format!("{what} operand is missing")));
    }
    let operand = unsafe { LLVMGetOperand(raw, index) };
    if unsafe { LLVMIsAConstantInt(operand) }.is_null() {
        return Err(CodegenError(format!("{what} is not a constant integer")));
    }
    Ok(unsafe { LLVMConstIntGetZExtValue(operand) })
}

pub(super) fn llvm_value_name(
    raw: inkwell::llvm_sys::prelude::LLVMValueRef,
) -> Result<String, CodegenError> {
    let mut length = 0usize;
    // SAFETY: LLVM owns the returned byte span for the lifetime of `raw`.
    let pointer = unsafe { LLVMGetValueName2(raw, &mut length) };
    if pointer.is_null() {
        return Ok(String::new());
    }
    let bytes = unsafe { slice::from_raw_parts(pointer.cast::<u8>(), length) };
    String::from_utf8(bytes.to_vec())
        .map_err(|error| CodegenError(format!("LLVM value name is not UTF-8: {error}")))
}
