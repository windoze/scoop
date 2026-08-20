//! Recognize selections whose possible pointer values are all LLVM constants.

use super::*;
use inkwell::llvm_sys::core::{
    LLVMCountIncoming, LLVMGetIncomingValue, LLVMIsAPHINode, LLVMIsASelectInst, LLVMIsConstant,
};

pub(super) fn is_constant_root(value: PointerValue<'_>) -> bool {
    let mut pending = vec![value.as_value_ref()];
    let mut visited = BTreeSet::new();
    while let Some(value) = pending.pop() {
        if !visited.insert(value as usize) {
            continue;
        }
        // SAFETY: every work item is a live value from the verified LLVM module.
        unsafe {
            if LLVMIsConstant(value) != 0 {
                continue;
            }
            if !LLVMIsAPHINode(value).is_null() {
                for index in 0..LLVMCountIncoming(value) {
                    pending.push(LLVMGetIncomingValue(value, index));
                }
            } else if !LLVMIsASelectInst(value).is_null() {
                pending.push(LLVMGetOperand(value, 1));
                pending.push(LLVMGetOperand(value, 2));
            } else {
                return false;
            }
        }
    }
    true
}
