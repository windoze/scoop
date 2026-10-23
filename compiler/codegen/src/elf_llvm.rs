//! Apply ELF visibility and COMDAT ownership from the existing symbol plans.

use inkwell::GlobalVisibility;
use inkwell::module::{Linkage, Module};
use inkwell::values::{AsValueRef, GlobalValue};
use llvm_sys::core::{
    LLVMAliasGetAliasee, LLVMGetConstOpcode, LLVMGetFirstUse, LLVMGetOperand, LLVMIsAConstantExpr,
    LLVMIsAGlobalAlias, LLVMIsAGlobalObject,
};
use scoop_lir::ObjectSymbolSurfaceV1;

use crate::CodegenError;

mod plans;
pub(crate) use plans::SymbolPlanIndex;

pub(crate) fn prepare(
    llvm: &Module<'_>,
    surface: &ObjectSymbolSurfaceV1,
) -> Result<(), CodegenError> {
    SymbolPlanIndex::new(surface).prepare(llvm)
}

pub(crate) fn apply_visibility(value: GlobalValue<'_>) {
    // Unused hidden declarations emit NOTYPE symbols even without relocations.
    // Besides conflicting with TLS definitions, those symbols make a shared
    // ODR body depend on unrelated declarations in its consumer's module.
    let unused =
        value.is_declaration() && unsafe { LLVMGetFirstUse(value.as_value_ref()) }.is_null();
    value.set_visibility(if unused {
        GlobalVisibility::Default
    } else {
        GlobalVisibility::Hidden
    });
}

fn definition_owner(value: GlobalValue<'_>) -> Result<GlobalValue<'_>, CodegenError> {
    let mut raw = value.as_value_ref();
    // Boundaries may refer to a parent-table alias, which in turn projects a
    // complete array. All of those views belong to the array's ELF section.
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
                            "ELF atom alias is not a direct owner/extent".into(),
                        ));
                    }
                }
            } else {
                break;
            }
        }
        if LLVMIsAGlobalObject(raw).is_null() {
            return Err(CodegenError(
                "ELF atom alias does not resolve to an LLVM global object".into(),
            ));
        }
        Ok(GlobalValue::new(raw))
    }
}
