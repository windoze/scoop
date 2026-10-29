//! Apply ELF visibility and COMDAT ownership from the existing symbol plans.

use inkwell::GlobalVisibility;
use inkwell::module::{Linkage, Module};
use inkwell::values::{AsValueRef, GlobalValue};
use llvm_sys::core::LLVMGetFirstUse;

use crate::CodegenError;
use crate::llvm_definitions::definition_owner;

mod plans;

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
