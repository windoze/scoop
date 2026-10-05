//! Relocatable metadata becomes read-only after the platform loader fixes it up.

use std::ffi::CString;

use inkwell::{
    module::Module,
    values::{AsValueRef, BasicValueEnum, GlobalValue},
};

use crate::target::ValidatedBackendProfile;

pub(crate) fn place_immutable_metadata(llvm: &Module<'_>, profile: ValidatedBackendProfile) {
    for global in llvm.get_globals() {
        let Some(initializer) = global.get_initializer() else {
            continue;
        };
        if !global.is_constant()
            || global.get_section().is_some()
            || global.get_name().to_bytes().starts_with(b"llvm.")
            || matches!(initializer, BasicValueEnum::ArrayValue(array) if array.is_const_string())
        {
            continue;
        }
        set_section(global, profile.read_only_metadata_section());
    }
}

pub(crate) fn set_section(global: GlobalValue<'_>, section: &str) {
    let section = CString::new(section).expect("target section names contain no NUL");
    // Inkwell adds a Mach-O segment separator on macOS hosts, even for ELF
    // target modules. The backend profile already supplies the exact spelling.
    unsafe { llvm_sys::core::LLVMSetSection(global.as_value_ref(), section.as_ptr()) };
}
