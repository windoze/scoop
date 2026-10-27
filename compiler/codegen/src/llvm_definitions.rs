//! Resolve atom aliases to the LLVM object owning their native storage.

use inkwell::values::{AsValueRef, GlobalValue};
use llvm_sys::core::{
    LLVMAliasGetAliasee, LLVMGetConstOpcode, LLVMGetOperand, LLVMIsAConstantExpr,
    LLVMIsAGlobalAlias, LLVMIsAGlobalObject,
};

use crate::CodegenError;

pub(crate) fn definition_owner(value: GlobalValue<'_>) -> Result<GlobalValue<'_>, CodegenError> {
    let mut raw = value.as_value_ref();
    // Boundaries may refer to a parent-table alias, which in turn projects a
    // complete array. All of those views belong to the array's native section.
    unsafe {
        loop {
            if !LLVMIsAGlobalAlias(raw).is_null() {
                raw = LLVMAliasGetAliasee(raw);
            } else if !LLVMIsAConstantExpr(raw).is_null() {
                match LLVMGetConstOpcode(raw) {
                    llvm_sys::LLVMOpcode::LLVMGetElementPtr | llvm_sys::LLVMOpcode::LLVMBitCast => {
                        raw = LLVMGetOperand(raw, 0)
                    }
                    _ => {
                        return Err(CodegenError(
                            "atom alias is not a direct owner/extent".into(),
                        ));
                    }
                }
            } else {
                break;
            }
        }
        if LLVMIsAGlobalObject(raw).is_null() {
            return Err(CodegenError(
                "atom alias does not resolve to an LLVM global object".into(),
            ));
        }
        Ok(GlobalValue::new(raw))
    }
}
