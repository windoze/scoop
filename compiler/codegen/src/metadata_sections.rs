//! Relocatable metadata becomes read-only after the platform loader fixes it up.

use inkwell::{module::Module, values::BasicValueEnum};

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
        global.set_section(Some(profile.read_only_metadata_section()));
    }
}
